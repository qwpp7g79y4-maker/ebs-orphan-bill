use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use rust_decimal::Decimal;

use crate::bill::{
    kind_of, older_than, quote_snapshot, quote_volume, Bill, QuoteNote, SnapshotBasis, SnapshotRow,
    VolumeKind, VolumeRow,
};
use crate::error::Error;
use crate::money::{
    epoch_millis, exact_add, gb_from_bytes, parse_ymd, ymd_to_epoch_millis, Amount,
};
use crate::parse::{parse_document, Image, Snapshot, SnapshotState, SnapshotTier, Volume};
use crate::price::{prices_builtin, Prices};
use crate::report::{render, render_markdown};
use crate::MAX_FILE_BYTES;

/// Path text for a message. Control characters are escaped. A path that is
/// not valid UTF-8 is shown as escaped raw bytes, so no byte is lost.
pub fn shown_path(path: &Path) -> String {
    match path.to_str() {
        Some(text) => {
            let mut shown = String::new();
            for ch in text.chars() {
                if crate::hidden::is_hidden(ch) && !ch.is_control() {
                    shown.extend(ch.escape_unicode());
                } else {
                    shown.extend(ch.escape_debug());
                }
            }
            shown
        }
        None => path_bytes(path).escape_ascii().to_string(),
    }
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(not(unix))]
fn path_bytes(path: &Path) -> Vec<u8> {
    path.to_string_lossy().into_owned().into_bytes()
}

/// Price data older than this many days before the reference date gets a warning.
const PRICE_MAX_AGE_DAYS: i64 = 180;

/// What to price, and how to print it.
#[derive(Debug, Clone)]
pub struct Options {
    pub prices: Prices,
    pub as_of: Option<String>,
    pub older_than_days: u32,
    pub fail_usd: Option<Decimal>,
    pub json: bool,
    /// Print Markdown instead of the text report (ignored when `json` is set).
    pub markdown: bool,
    /// Drop the warnings from the text report and from standard error.
    pub quiet: bool,
    /// Midnight UTC of today in Unix milliseconds, for the price-age check when
    /// `--as-of` is not set. `None` skips that check; only `main` reads the clock.
    pub today_ms: Option<i64>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            prices: prices_builtin(),
            as_of: None,
            older_than_days: 30,
            fail_usd: None,
            json: false,
            markdown: false,
            quiet: false,
            today_ms: None,
        }
    }
}

/// One file, or one member of a directory walk (`strict` is false for members).
pub struct NamedBytes {
    pub name: String,
    pub bytes: Vec<u8>,
    pub strict: bool,
}

/// Start of the warning about old price data; the binary also writes it to standard error in every mode.
const PRICE_AGE_PREFIX: &str = "price data is ";

/// Rendered report. `warnings` is also included in `stdout` for text output.
#[derive(Debug)]
pub struct Output {
    pub stdout: String,
    pub warnings: Vec<String>,
    pub threshold_exceeded: bool,
    /// One line of counts for `--verbose`, without a trailing newline.
    pub context: String,
}

pub fn bill_from_inputs(inputs: &[NamedBytes], options: &Options) -> Result<Bill, Error> {
    let as_of_ms = match &options.as_of {
        Some(text) => Some(ymd_to_epoch_millis(text).ok_or_else(|| {
            Error::new("invalid --as-of: expected YYYY-MM-DD, 1970-01-01 through 2199-12-31")
        })?),
        None => None,
    };
    let mut warnings = Vec::new();
    let mut volumes = Vec::new();
    let mut snapshots = Vec::new();
    let mut images = Vec::new();
    let mut saw_snapshots = false;
    let mut saw_images = false;
    for input in inputs {
        let parsed = parse_document(&input.name, &input.bytes, input.strict)?;
        warnings.extend(parsed.warnings);
        saw_snapshots |= parsed.saw_snapshots;
        saw_images |= parsed.saw_images;
        volumes.extend(parsed.volumes);
        snapshots.extend(parsed.snapshots);
        images.extend(parsed.images);
    }
    price_all(
        Collected {
            volumes,
            snapshots,
            images,
            saw_snapshots,
            saw_images,
            warnings,
        },
        options,
        as_of_ms,
    )
}

struct Collected {
    volumes: Vec<Volume>,
    snapshots: Vec<Snapshot>,
    images: Vec<Image>,
    saw_snapshots: bool,
    saw_images: bool,
    warnings: Vec<String>,
}

pub fn execute_paths(paths: &[PathBuf], options: &Options) -> Result<Output, Error> {
    let mut named = Vec::new();
    let mut warnings = Vec::new();
    for path in paths {
        if path.as_os_str() == "-" {
            let mut buf = Vec::new();
            std::io::stdin()
                .take(MAX_FILE_BYTES + 1)
                .read_to_end(&mut buf)
                .map_err(|err| Error::new(format!("-: {err}")))?;
            if buf.len() as u64 > MAX_FILE_BYTES {
                return Err(Error::new("-: file is larger than 268435456 bytes"));
            }
            named.push(NamedBytes {
                name: "-".to_string(),
                bytes: buf,
                strict: true,
            });
            continue;
        }
        push_explicit(path, &mut named, &mut warnings)?;
    }
    let mut bill = bill_from_inputs(&named, options)?;
    // The input bytes are not needed once the bill exists.
    drop(named);
    let mut all = warnings;
    all.append(&mut bill.warnings);
    bill.warnings = all;
    let threshold_exceeded = bill.threshold_exceeded;
    let context = format!(
        "context: orphan volumes {}, old snapshots {}, warnings {}",
        bill.volumes.len(),
        bill.snapshots.len(),
        bill.warnings.len()
    );
    if options.quiet && !options.json {
        bill.warnings.clear();
    }
    let stdout = if options.markdown && !options.json {
        render_markdown(&bill)
    } else {
        render(&bill, options.json)
    };
    let mut warnings = std::mem::take(&mut bill.warnings);
    if options.quiet {
        warnings.clear();
    }
    drop(bill);
    Ok(Output {
        stdout,
        warnings,
        threshold_exceeded,
        context,
    })
}

fn push_explicit(
    path: &Path,
    named: &mut Vec<NamedBytes>,
    warnings: &mut Vec<String>,
) -> Result<(), Error> {
    let symlink_meta = fs::symlink_metadata(path)
        .map_err(|err| Error::new(format!("{}: {err}", shown_path(path))))?;
    let meta = if symlink_meta.file_type().is_symlink() {
        fs::metadata(path).map_err(|err| Error::new(format!("{}: {err}", shown_path(path))))?
    } else {
        symlink_meta
    };
    if meta.is_dir() {
        return collect_dir(path, 1, named, warnings);
    }
    if !meta.is_file() {
        return Err(Error::new(format!("{}: not a file", shown_path(path))));
    }
    let bytes = read_limited(path)?;
    named.push(NamedBytes {
        name: shown_path(path),
        bytes,
        strict: true,
    });
    Ok(())
}

fn collect_dir(
    dir: &Path,
    depth: usize,
    named: &mut Vec<NamedBytes>,
    warnings: &mut Vec<String>,
) -> Result<(), Error> {
    if depth > 64 {
        return Err(Error::new(format!(
            "{}: directory is nested too deeply",
            shown_path(dir)
        )));
    }
    let entries =
        fs::read_dir(dir).map_err(|err| Error::new(format!("{}: {err}", shown_path(dir))))?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|err| Error::new(format!("{}: {err}", shown_path(dir))))?;
        paths.push(entry.path());
    }
    paths.sort();
    for path in paths {
        let meta = fs::symlink_metadata(&path)
            .map_err(|err| Error::new(format!("{}: {err}", shown_path(&path))))?;
        let kind = meta.file_type();
        if kind.is_symlink() {
            warnings.push(format!("skipped symlink {}", shown_path(&path)));
            continue;
        }
        if kind.is_dir() {
            collect_dir(&path, depth + 1, named, warnings)?;
        } else if kind.is_file() {
            let bytes = read_limited(&path)?;
            named.push(NamedBytes {
                name: shown_path(&path),
                bytes,
                strict: false,
            });
        } else {
            warnings.push(format!("skipped non-regular file {}", shown_path(&path)));
        }
    }
    Ok(())
}

fn read_limited(path: &Path) -> Result<Vec<u8>, Error> {
    let meta =
        fs::metadata(path).map_err(|err| Error::new(format!("{}: {err}", shown_path(path))))?;
    if meta.len() > MAX_FILE_BYTES {
        return Err(Error::new(format!(
            "{}: file is larger than 268435456 bytes",
            shown_path(path)
        )));
    }
    let file =
        fs::File::open(path).map_err(|err| Error::new(format!("{}: {err}", shown_path(path))))?;
    let mut buf = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut buf)
        .map_err(|err| Error::new(format!("{}: {err}", shown_path(path))))?;
    if buf.len() as u64 > MAX_FILE_BYTES {
        return Err(Error::new(format!(
            "{}: file is larger than 268435456 bytes",
            shown_path(path)
        )));
    }
    Ok(buf)
}

fn price_all(
    collected: Collected,
    options: &Options,
    as_of_ms: Option<i64>,
) -> Result<Bill, Error> {
    let Collected {
        volumes,
        snapshots,
        images,
        saw_snapshots,
        saw_images,
        mut warnings,
    } = collected;
    let mut volume_map: BTreeMap<String, Volume> = BTreeMap::new();
    for volume in volumes {
        let id = volume.id.clone();
        if volume_map.insert(id.clone(), volume).is_some() {
            return Err(Error::new(format!("duplicate volume {id}")));
        }
    }
    let mut snapshot_map: BTreeMap<String, Snapshot> = BTreeMap::new();
    for snapshot in snapshots {
        let id = snapshot.id.clone();
        if snapshot_map.insert(id.clone(), snapshot).is_some() {
            return Err(Error::new(format!("duplicate snapshot {id}")));
        }
    }
    let mut image_ids = BTreeSet::new();
    let mut ami_snapshots = BTreeSet::new();
    for image in images {
        if !image_ids.insert(image.id.clone()) {
            return Err(Error::new(format!("duplicate image {}", image.id)));
        }
        for snapshot_id in image.snapshot_ids {
            ami_snapshots.insert(snapshot_id);
        }
    }
    if let Some(reference_ms) = as_of_ms.or(options.today_ms) {
        if let Some((year, month, day)) = parse_ymd(&options.prices.date) {
            let price_ms = epoch_millis(year, month, day);
            let days = reference_ms.saturating_sub(price_ms) / 86_400_000;
            if days > PRICE_MAX_AGE_DAYS {
                warnings.push(format!(
                    "{PRICE_AGE_PREFIX}{days} days old (dated {}, more than {PRICE_MAX_AGE_DAYS} days before {}); check the prices",
                    options.prices.date,
                    if as_of_ms.is_some() { "--as-of" } else { "today" }
                ));
            }
        }
    }
    if saw_snapshots && options.as_of.is_none() {
        warnings.push("--as-of is not set; snapshots are not classified".to_string());
    }
    if saw_snapshots && !saw_images {
        warnings.push(
            "describe-images was not provided; snapshots are not classified as orphans".to_string(),
        );
    }
    let classify = as_of_ms.is_some() && saw_images;
    let (volume_rows, volume_usd) = price_volumes(&volume_map, options, &mut warnings)?;
    let (snapshot_rows, snapshot_usd) = price_snapshots(
        &snapshot_map,
        &ami_snapshots,
        options,
        as_of_ms,
        classify,
        &mut warnings,
    )?;
    let usd_month =
        exact_add(volume_usd, snapshot_usd).ok_or_else(|| Error::new("amount overflowed"))?;
    if !usd_month.cents_fit() {
        return Err(Error::new("amount overflowed"));
    }
    let threshold_exceeded = match options.fail_usd {
        Some(limit) => {
            volume_rows
                .iter()
                .any(|row| row.usd_month.is_some_and(|usd| usd.gt_decimal(limit)))
                || snapshot_rows
                    .iter()
                    .any(|row| row.usd_month.is_some_and(|usd| usd.gt_decimal(limit)))
        }
        None => false,
    };
    Ok(Bill {
        prices: options.prices.clone(),
        as_of: options.as_of.clone(),
        older_than_days: options.older_than_days,
        fail_usd: options.fail_usd,
        threshold_exceeded,
        images_checked: saw_images,
        snapshots_classified: classify,
        volumes: volume_rows,
        snapshots: snapshot_rows,
        volume_usd,
        snapshot_usd,
        usd_month,
        warnings,
    })
}

fn price_volumes(
    volumes: &BTreeMap<String, Volume>,
    options: &Options,
    warnings: &mut Vec<String>,
) -> Result<(Vec<VolumeRow>, Amount), Error> {
    let mut rows = Vec::new();
    let mut total = Amount::ZERO;
    let mut regions = BTreeSet::new();
    let mut az_warned = false;
    for volume in volumes.values() {
        if let Some(region) = &volume.region {
            if region != &options.prices.region && regions.insert(region.clone()) {
                warnings.push(format!(
                    "volume region {region} differs from price region {}",
                    options.prices.region
                ));
            }
        }
        if volume.az_unparsed && !az_warned {
            warnings.push("availability zone is not a region name".to_string());
            az_warned = true;
        }
        if !volume.loose {
            continue;
        }
        if let Some(state) = volume.transient_state {
            warnings.push(format!(
                "{}: state is {state} and attachments are empty; not priced as an orphan",
                volume.id
            ));
            continue;
        }
        if volume.state_in_use {
            warnings.push(format!(
                "{}: state is in-use and attachments are empty",
                volume.id
            ));
        }
        let kind = kind_of(&volume.volume_type);
        let quote = quote_volume(
            kind,
            volume.size_gb,
            volume.iops,
            volume.throughput_mib,
            &options.prices,
        )
        .map_err(|err| Error::new(format!("{}: {err}", volume.id)))?;
        for note in &quote.notes {
            let line = match note {
                QuoteNote::Gp3IopsBaseline => format!(
                    "{}: Iops missing; using gp3 baseline {}",
                    volume.id, options.prices.gp3_iops_baseline
                ),
                QuoteNote::Gp3ThroughputBaseline => format!(
                    "{}: Throughput missing; using gp3 baseline {}",
                    volume.id, options.prices.gp3_throughput_baseline_mib
                ),
                QuoteNote::IopsNotPriced => {
                    format!("{}: Iops missing; IOPS not priced", volume.id)
                }
            };
            warnings.push(line);
        }
        let label = type_label(&volume.volume_type);
        if kind == VolumeKind::Unpriced {
            if label == "unknown" {
                warnings.push(format!("{}: volume type is not priced", volume.id));
            } else {
                warnings.push(format!("{}: volume type {label} is not priced", volume.id));
            }
        }
        if let Some(usd) = quote.usd_month {
            total = add_total(total, usd, &volume.id)?;
        }
        rows.push(VolumeRow {
            id: volume.id.clone(),
            volume_type: label,
            size_gb: volume.size_gb,
            iops: quote.iops,
            throughput_mib: quote.throughput_mib,
            storage_usd: quote.storage_usd,
            iops_usd: quote.iops_usd,
            throughput_usd: quote.throughput_usd,
            usd_month: quote.usd_month,
            priced: quote.priced,
            iops_unpriced: quote.notes.contains(&QuoteNote::IopsNotPriced),
        });
    }
    Ok((rows, checked_total(total)?))
}

fn price_snapshots(
    snapshots: &BTreeMap<String, Snapshot>,
    ami_snapshots: &BTreeSet<String>,
    options: &Options,
    as_of_ms: Option<i64>,
    classify: bool,
    warnings: &mut Vec<String>,
) -> Result<(Vec<SnapshotRow>, Amount), Error> {
    let mut rows = Vec::new();
    let mut total = Amount::ZERO;
    for snapshot in snapshots.values() {
        let Some(start_ms) = snapshot.start_ms else {
            if as_of_ms.is_some() {
                warnings.push(format!(
                    "{}: StartTime is missing; snapshot is not classified",
                    snapshot.id
                ));
            }
            continue;
        };
        if let Some(as_of_ms) = as_of_ms {
            let Some(age_ms) = as_of_ms.checked_sub(start_ms) else {
                return Err(Error::new(format!("{}: bad StartTime", snapshot.id)));
            };
            if skip_state(snapshot, warnings) {
                continue;
            }
            if age_ms < 0 {
                return Err(Error::new(format!(
                    "{}: StartTime is after --as-of",
                    snapshot.id
                )));
            }
            if !classify || !older_than(age_ms, options.older_than_days) {
                continue;
            }
            if ami_snapshots.contains(&snapshot.id) {
                continue;
            }
            let (gb, basis) = snapshot_gb(snapshot)?;
            let priced = snapshot.tier == SnapshotTier::Standard;
            let usd_month = if priced {
                Some(
                    quote_snapshot(gb, &options.prices)
                        .map_err(|err| Error::new(format!("{}: {err}", snapshot.id)))?,
                )
            } else {
                None
            };
            if !priced {
                warnings.push(unpriced_tier_warning(snapshot));
            }
            if let Some(usd) = usd_month {
                total = add_total(total, usd, &snapshot.id)?;
            }
            let age_days = Decimal::from(age_ms) / Decimal::from(86_400_000_i64);
            rows.push(SnapshotRow {
                id: snapshot.id.clone(),
                age_days,
                gb,
                usd_month,
                basis,
                tier: tier_name(snapshot.tier).to_string(),
                priced,
            });
        } else {
            let _ = skip_state(snapshot, warnings);
        }
    }
    Ok((rows, checked_total(total)?))
}

fn skip_state(snapshot: &Snapshot, warnings: &mut Vec<String>) -> bool {
    let line = match snapshot.state {
        SnapshotState::Completed => return false,
        SnapshotState::Pending => format!("{}: snapshot state is pending; not priced", snapshot.id),
        SnapshotState::Error => format!("{}: snapshot state is error; not priced", snapshot.id),
        SnapshotState::Other => format!(
            "{}: snapshot state is not completed; not priced",
            snapshot.id
        ),
    };
    warnings.push(line);
    true
}

fn unpriced_tier_warning(snapshot: &Snapshot) -> String {
    if snapshot.tier == SnapshotTier::Archive {
        format!("{}: snapshot tier archive is not priced", snapshot.id)
    } else {
        format!("{}: snapshot tier is not priced", snapshot.id)
    }
}

fn add_total(total: Amount, usd: Amount, id: &str) -> Result<Amount, Error> {
    let sum =
        exact_add(total, usd).ok_or_else(|| Error::new(format!("{id}: amount overflowed")))?;
    Ok(sum)
}

/// Only the finished total must fit in cents: a running sum that does not fit
/// may still be followed by a total that does, whatever the order of the ids.
fn checked_total(total: Amount) -> Result<Amount, Error> {
    if total.cents_fit() {
        Ok(total)
    } else {
        Err(Error::new("amount overflowed"))
    }
}

fn snapshot_gb(snapshot: &Snapshot) -> Result<(Decimal, SnapshotBasis), Error> {
    if let Some(bytes) = snapshot.full_bytes {
        return Ok((
            gb_from_bytes(Decimal::from(bytes)),
            SnapshotBasis::FullSnapshotBytes,
        ));
    }
    if let Some(size) = snapshot.volume_size_gb {
        return Ok((Decimal::from(size), SnapshotBasis::VolumeSize));
    }
    Err(Error::new(format!("{}: missing size", snapshot.id)))
}

fn tier_name(tier: SnapshotTier) -> &'static str {
    match tier {
        SnapshotTier::Standard => "standard",
        SnapshotTier::Archive => "archive",
        SnapshotTier::Other => "other",
    }
}

fn type_label(volume_type: &str) -> String {
    let safe = (1..=16).contains(&volume_type.len())
        && volume_type
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit());
    if safe {
        volume_type.to_string()
    } else {
        "unknown".to_string()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn shown_path_escapes_invisible_and_variation_selector_characters() {
        for ch in [
            '\u{200B}',
            '\u{2060}',
            '\u{FE0F}',
            '\u{E0100}',
            '\u{E01EF}',
            '\u{3164}',
        ] {
            let shown = super::shown_path(std::path::Path::new(&format!("a{ch}b")));
            assert_eq!(
                shown,
                format!("a\\u{{{:x}}}b", ch as u32),
                "{:X}",
                ch as u32
            );
        }
    }

    #[test]
    fn shown_path_escapes_spacing_and_format_characters() {
        for ch in ['\u{A0}', '\u{3000}', '\u{2003}', '\u{FFFC}', '\u{600}'] {
            let shown = super::shown_path(std::path::Path::new(&format!("a{ch}b")));
            assert_eq!(
                shown,
                format!("a\\u{{{:x}}}b", ch as u32),
                "{:X}",
                ch as u32
            );
        }
        assert_eq!(
            super::shown_path(std::path::Path::new("a b\u{e9}.json")),
            "a b\u{e9}.json"
        );
    }

    #[test]
    fn shown_path_escapes_controls_and_keeps_colons() {
        let shown = super::shown_path(std::path::Path::new("a\u{1b}\nb:c"));
        assert_eq!(shown, "a\\u{1b}\\nb:c");
    }

    #[test]
    fn braille_blank_in_file_name_is_escaped_and_volume_still_counts() {
        let shown = super::shown_path(std::path::Path::new("x\u{2800}y"));
        assert_eq!(shown, "x\\u{2800}y");

        let dir = std::env::temp_dir().join(format!("ebs-braille-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bad = dir.join("bad\u{2800}.json");
        let good = dir.join("good\u{2800}.json");
        std::fs::write(&bad, "not json").unwrap();
        std::fs::write(
            &good,
            r#"{"Volumes":[{"VolumeId":"vol-0123456789abcdef0","Size":100,"VolumeType":"gp3","State":"available","Attachments":[]}]}"#,
        )
        .unwrap();
        let plain = dir.join("plain.json");
        std::fs::copy(&good, &plain).unwrap();

        let err =
            super::execute_paths(std::slice::from_ref(&bad), &Options::default()).unwrap_err();
        let text = err.to_string();
        std::fs::remove_file(&bad).unwrap();
        let with = super::execute_paths(std::slice::from_ref(&good), &Options::default());
        let without = super::execute_paths(std::slice::from_ref(&plain), &Options::default());
        std::fs::remove_dir_all(&dir).unwrap();

        assert!(text.contains("\\u{2800}"), "{text}");
        assert!(!text.contains('\u{2800}'), "{text}");
        let with = with.unwrap();
        let without = without.unwrap();
        assert_eq!(with.stdout, without.stdout);
    }

    use super::{bill_from_inputs, NamedBytes, Options};
    use crate::money::format_usd;
    use crate::report::render;

    fn file(name: &str, json: &str) -> NamedBytes {
        NamedBytes {
            name: name.to_string(),
            bytes: json.as_bytes().to_vec(),
            strict: true,
        }
    }

    fn opts(as_of: Option<&str>, fail: Option<&str>) -> Options {
        Options {
            as_of: as_of.map(str::to_string),
            fail_usd: fail.map(|text| text.parse().unwrap()),
            ..Options::default()
        }
    }

    fn examples() -> Vec<NamedBytes> {
        vec![
            file(
                "examples/volumes.json",
                include_str!("../examples/volumes.json"),
            ),
            file(
                "examples/snapshots.json",
                include_str!("../examples/snapshots.json"),
            ),
            file(
                "examples/images.json",
                include_str!("../examples/images.json"),
            ),
        ]
    }

    #[test]
    fn running_total_that_does_not_fit_may_end_in_a_total_that_fits() {
        use super::{add_total, checked_total};
        use crate::money::Amount;
        use rust_decimal::Decimal;
        let big = Amount::from_decimal(Decimal::from_i128_with_scale((1_i128 << 96) - 1, 2))
            .expect("amount");
        let cent = Amount::from_decimal(Decimal::new(1, 2)).expect("amount");
        let four = Amount::from_decimal(Decimal::new(4, 2)).expect("amount");
        let forward = [big, cent, four]
            .into_iter()
            .try_fold(Amount::ZERO, |sum, usd| add_total(sum, usd, "x"))
            .and_then(checked_total);
        let backward = [four, cent, big]
            .into_iter()
            .try_fold(Amount::ZERO, |sum, usd| add_total(sum, usd, "x"))
            .and_then(checked_total);
        assert!(forward.is_ok());
        assert!(backward.is_ok());
    }

    #[test]
    fn example_orphans_total_21_50() {
        let bill = bill_from_inputs(&examples(), &opts(Some("2026-11-09"), None)).unwrap();
        assert_eq!(bill.volumes.len(), 3);
        assert_eq!(bill.volumes[0].id, "vol-11111111111111111");
        assert_eq!(format_usd(bill.volumes[0].usd_month.unwrap()), "16.00");
        assert_eq!(format_usd(bill.volumes[1].usd_month.unwrap()), "5.00");
        assert!(!bill.volumes[2].priced);
        assert_eq!(bill.volumes[2].id, "vol-44444444444444444");
        assert!(bill
            .snapshots
            .iter()
            .any(|row| row.id == "snap-11111111111111111"));
        assert!(bill
            .snapshots
            .iter()
            .any(|row| row.id == "snap-44444444444444444" && !row.priced));
        assert!(!bill
            .snapshots
            .iter()
            .any(|row| row.id == "snap-22222222222222222"));
        assert!(!bill
            .snapshots
            .iter()
            .any(|row| row.id == "snap-33333333333333333"));
        assert!(!bill
            .volumes
            .iter()
            .any(|row| row.id == "vol-33333333333333333"));
        assert_eq!(format_usd(bill.volume_usd), "21.00");
        assert_eq!(format_usd(bill.snapshot_usd), "0.50");
        assert_eq!(format_usd(bill.usd_month), "21.50");
        assert_eq!(format_usd(bill.snapshots[0].usd_month.unwrap()), "0.50");
        assert_eq!(format_usd(bill.snapshots[0].age_days), "31.00");
        let text = render(&bill, false);
        assert!(text.contains("21.50"));
        assert!(!text.contains("123456789012"));
        assert!(!text.contains("SECRETKEY"));
        assert!(!text.contains("SECRETTOKEN"));
        assert!(!text.contains("CreateImage"));
        assert!(!text.contains("i-11111111111111111"));
        assert!(!text.contains("ami-11111111111111111"));
        assert!(!text.contains("aws ec2"));
        assert!(text.contains("unpriced"));
        assert!(text.contains("nextToken was not followed"));
    }

    #[test]
    fn threshold_is_strict_on_priced_orphans_only() {
        let equal = bill_from_inputs(&examples(), &opts(Some("2026-11-09"), Some("16"))).unwrap();
        assert!(!equal.threshold_exceeded);
        let under =
            bill_from_inputs(&examples(), &opts(Some("2026-11-09"), Some("15.99"))).unwrap();
        assert!(under.threshold_exceeded);
        let high = bill_from_inputs(&examples(), &opts(Some("2026-11-09"), Some("21.50"))).unwrap();
        assert!(!high.threshold_exceeded);
        let json = render(&under, true);
        assert!(json.contains("\"threshold_exceeded\": true"));
        assert!(json.contains("15.99"));
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["estimated_usd_month"], "21.50");
        assert_eq!(parsed["volumes"][2]["usd_month"], serde_json::Value::Null);
        assert_eq!(parsed["snapshots"][1]["priced"], false);
    }

    #[test]
    fn without_images_or_as_of_snapshots_stay_out() {
        let volumes = file(
            "examples/volumes.json",
            include_str!("../examples/volumes.json"),
        );
        let snapshots = file(
            "examples/snapshots.json",
            include_str!("../examples/snapshots.json"),
        );
        let no_images =
            bill_from_inputs(&[volumes, snapshots], &opts(Some("2026-11-09"), None)).unwrap();
        assert!(no_images.snapshots.is_empty());
        assert!(!no_images.snapshots_classified);
        assert!(no_images
            .warnings
            .iter()
            .any(|line| line.contains("describe-images was not provided")));
        assert_eq!(format_usd(no_images.volume_usd), "21.00");
        let no_date = bill_from_inputs(&examples(), &opts(None, None)).unwrap();
        assert!(no_date.snapshots.is_empty());
        assert!(no_date
            .warnings
            .iter()
            .any(|line| line.contains("--as-of is not set")));
        assert_eq!(format_usd(no_date.usd_month), "21.00");
    }

    #[test]
    fn empty_images_still_classifies_and_exact_age_does_not() {
        let snaps = file(
            "s.json",
            r#"{"Snapshots":[
                {"SnapshotId":"snap-11111111111111111","VolumeSize":1,"StartTime":"2026-10-10T00:00:00Z"},
                {"SnapshotId":"snap-22222222222222222","VolumeSize":1,"StartTime":"2026-10-09T23:59:59Z"}
            ]}"#,
        );
        let images = file("i.json", r#"{"Images":[]}"#);
        let bill = bill_from_inputs(&[snaps, images], &opts(Some("2026-11-09"), None)).unwrap();
        assert!(bill.images_checked);
        assert!(bill.snapshots_classified);
        assert_eq!(bill.snapshots.len(), 1);
        assert_eq!(bill.snapshots[0].id, "snap-22222222222222222");
        assert_eq!(format_usd(bill.snapshots[0].usd_month.unwrap()), "0.05");
    }

    #[test]
    fn pending_snapshot_starting_today_is_skipped_not_an_error() {
        let snaps = file(
            "s.json",
            r#"{"Snapshots":[
                {"SnapshotId":"snap-11111111","VolumeSize":1,"StartTime":"2026-09-01T00:00:00Z"},
                {"SnapshotId":"snap-22222222","VolumeSize":1,"StartTime":"2026-11-09T12:00:00.000Z","State":"pending"}
            ]}"#,
        );
        let images = file("i.json", r#"{"Images":[]}"#);
        let bill = bill_from_inputs(&[snaps, images], &opts(Some("2026-11-09"), None)).unwrap();
        assert_eq!(bill.snapshots.len(), 1);
        assert_eq!(bill.snapshots[0].id, "snap-11111111");
    }

    #[test]
    fn start_after_as_of_is_an_error_and_full_bytes_win() {
        let future = file(
            "s.json",
            r#"{"SnapshotId":"snap-11111111111111111","VolumeSize":1,"StartTime":"2026-11-10T00:00:00Z","State":"completed"}"#,
        );
        let err = bill_from_inputs(&[future], &opts(Some("2026-11-09"), None)).unwrap_err();
        assert_eq!(
            err.message(),
            "snap-11111111111111111: StartTime is after --as-of"
        );
        let both = file(
            "s.json",
            r#"{"SnapshotId":"snap-11111111111111111","VolumeSize":99,"FullSnapshotSizeInBytes":1073741824,"StartTime":"2026-10-09T00:00:00Z"}"#,
        );
        let images = file("i.json", r#"{"Images":[]}"#);
        let bill = bill_from_inputs(&[both, images], &opts(Some("2026-11-09"), None)).unwrap();
        assert_eq!(bill.snapshots[0].basis.as_str(), "full-snapshot-bytes");
        assert_eq!(format_usd(bill.snapshots[0].gb), "1.00");
        assert_eq!(format_usd(bill.snapshots[0].usd_month.unwrap()), "0.05");
    }

    #[test]
    fn one_byte_displays_as_zero_and_still_exceeds_a_zero_threshold() {
        let snap = file(
            "s.json",
            r#"{"SnapshotId":"snap-11111111111111111","FullSnapshotSizeInBytes":1,"StartTime":"2026-01-01T00:00:00Z"}"#,
        );
        let images = file("i.json", r#"{"Images":[]}"#);
        let bill = bill_from_inputs(&[snap, images], &opts(Some("2026-11-09"), Some("0"))).unwrap();
        assert_eq!(format_usd(bill.snapshots[0].usd_month.unwrap()), "0.00");
        assert!(bill.threshold_exceeded);
        let exact = bill.snapshots[0].usd_month.unwrap();
        assert!(exact.gt_decimal(rust_decimal::Decimal::ZERO));
        assert!(!exact.gt_amount(exact));
    }

    #[test]
    fn duplicates_region_mismatch_and_hidden_zone() {
        let dup = file(
            "a.json",
            r#"[{"VolumeId":"vol-01234567","Size":1,"VolumeType":"gp2"},{"VolumeId":"vol-01234567","Size":2,"VolumeType":"gp2"}]"#,
        );
        assert_eq!(
            bill_from_inputs(&[dup], &opts(None, None))
                .unwrap_err()
                .message(),
            "duplicate volume vol-01234567"
        );
        let zones = file(
            "a.json",
            r#"{"Volumes":[
                {"VolumeId":"vol-11111111111111111","Size":1,"VolumeType":"gp2","AvailabilityZone":"eu-west-1b"},
                {"VolumeId":"vol-22222222222222222","Size":1,"VolumeType":"gp2","AvailabilityZone":"SECRETZONE"}
            ]}"#,
        );
        let bill = bill_from_inputs(&[zones], &opts(None, None)).unwrap();
        let text = render(&bill, false);
        assert!(text.contains("volume region eu-west-1 differs from price region us-east-1"));
        assert!(text.contains("availability zone is not a region name"));
        assert!(!text.contains("SECRETZONE"));
    }

    #[test]
    fn creating_deleting_deleted_and_error_volumes_are_not_priced_as_orphans() {
        for state in ["creating", "deleting", "deleted", "error"] {
            let json = format!(
                r#"{{"Volumes":[{{"VolumeId":"vol-11111111111111111","Size":100,"VolumeType":"gp2","State":"{state}","Attachments":[]}}]}}"#
            );
            let bill = bill_from_inputs(&[file("v.json", &json)], &opts(Some("2026-11-09"), None))
                .unwrap();
            assert!(bill.volumes.is_empty(), "{state}");
            assert!(!bill.threshold_exceeded, "{state}");
            assert!(
                bill.warnings.iter().any(|line| line.contains(&format!(
                    "vol-11111111111111111: state is {state} and attachments are empty; not priced"
                ))),
                "{state}: {:?}",
                bill.warnings
            );
        }
        for state in ["available", "in-use"] {
            let json = format!(
                r#"{{"Volumes":[{{"VolumeId":"vol-11111111111111111","Size":100,"VolumeType":"gp2","State":"{state}","Attachments":[]}}]}}"#
            );
            let bill = bill_from_inputs(&[file("v.json", &json)], &opts(Some("2026-11-09"), None))
                .unwrap();
            assert_eq!(bill.volumes.len(), 1, "{state}");
        }
    }

    #[test]
    fn missing_io_iops_pending_and_in_use_loose() {
        let volumes = file(
            "v.json",
            r#"{"Volumes":[
                {"VolumeId":"vol-11111111111111111","Size":8,"VolumeType":"io2","State":"in-use","Attachments":[]},
                {"VolumeId":"vol-22222222222222222","Size":10,"VolumeType":"gp3"}
            ]}"#,
        );
        let snaps = file(
            "s.json",
            r#"{"Snapshots":[
                {"SnapshotId":"snap-11111111111111111","VolumeSize":4,"StartTime":"2026-01-01T00:00:00Z","State":"pending"},
                {"SnapshotId":"snap-22222222222222222","VolumeSize":4,"StartTime":"2026-01-01T00:00:00Z","StorageTier":"SECRET"}
            ]}"#,
        );
        let images = file("i.json", r#"{"Images":[]}"#);
        let bill =
            bill_from_inputs(&[volumes, snaps, images], &opts(Some("2026-11-09"), None)).unwrap();
        assert_eq!(format_usd(bill.volumes[0].usd_month.unwrap()), "1.00");
        assert!(bill.volumes[0].iops_unpriced);
        assert!(bill
            .warnings
            .iter()
            .any(|line| line.contains("Iops missing; IOPS not priced")));
        assert!(bill
            .warnings
            .iter()
            .any(|line| line.contains("state is in-use and attachments are empty")));
        assert!(bill
            .warnings
            .iter()
            .any(|line| line.contains("using gp3 baseline 3000")));
        assert!(bill
            .warnings
            .iter()
            .any(|line| line.contains("using gp3 baseline 125")));
        assert_eq!(format_usd(bill.volumes[1].usd_month.unwrap()), "0.80");
        assert!(bill
            .warnings
            .iter()
            .any(|line| line.contains("snapshot state is pending")));
        assert!(!bill
            .snapshots
            .iter()
            .any(|row| row.id == "snap-11111111111111111"));
        let other = bill
            .snapshots
            .iter()
            .find(|row| row.id == "snap-22222222222222222")
            .unwrap();
        assert!(!other.priced);
        assert_eq!(other.tier, "other");
        assert!(bill
            .warnings
            .iter()
            .any(|line| line.contains("snapshot tier is not priced")));
        assert!(!render(&bill, false).contains("SECRET"));
    }

    #[test]
    fn empty_volume_list_is_a_zero_bill() {
        let bill = bill_from_inputs(
            &[file("v.json", r#"{"Volumes":[]}"#)],
            &opts(Some("2026-11-09"), None),
        )
        .unwrap();
        assert!(bill.volumes.is_empty());
        assert_eq!(format_usd(bill.usd_month), "0.00");
        assert!(!bill.threshold_exceeded);
    }

    #[test]
    fn invalid_as_of_does_not_echo_the_text_body() {
        let err = bill_from_inputs(
            &[file("v.json", r#"{"Volumes":[]}"#)],
            &opts(Some("2025-02-29"), None),
        )
        .unwrap_err();
        assert_eq!(
            err.message(),
            "invalid --as-of: expected YYYY-MM-DD, 1970-01-01 through 2199-12-31"
        );
    }

    #[test]
    fn as_of_year_range_is_1970_through_2199() {
        let run = |d: &str| {
            bill_from_inputs(&[file("v.json", r#"{"Volumes":[]}"#)], &opts(Some(d), None))
        };
        assert!(run("1970-01-01").is_ok());
        assert!(run("2199-12-31").is_ok());
        assert!(run("0000-01-01").is_err());
        assert!(run("9999-12-31").is_err());
    }

    #[test]
    fn duplicate_snapshot_and_image_ids_are_errors() {
        let snaps = file(
            "s.json",
            r#"[{"SnapshotId":"snap-01234567","VolumeSize":1},{"SnapshotId":"snap-01234567","VolumeSize":2}]"#,
        );
        assert_eq!(
            bill_from_inputs(&[snaps], &opts(None, None))
                .unwrap_err()
                .message(),
            "duplicate snapshot snap-01234567"
        );
        let images = file(
            "i.json",
            r#"[{"ImageId":"ami-01234567"},{"ImageId":"ami-01234567"}]"#,
        );
        assert_eq!(
            bill_from_inputs(&[images], &opts(None, None))
                .unwrap_err()
                .message(),
            "duplicate image ami-01234567"
        );
    }

    #[test]
    fn unpriced_type_and_snapshot_skip_warning_texts() {
        let volumes = file(
            "v.json",
            r#"{"Volumes":[
                {"VolumeId":"vol-11111111111111111","Size":1,"VolumeType":"standard"},
                {"VolumeId":"vol-22222222222222222","Size":1,"VolumeType":"a-b"}
            ]}"#,
        );
        let snaps = file(
            "s.json",
            r#"{"Snapshots":[
                {"SnapshotId":"snap-11111111111111111","VolumeSize":1},
                {"SnapshotId":"snap-22222222222222222","VolumeSize":1,"StartTime":"2026-01-01T00:00:00Z","State":"error"},
                {"SnapshotId":"snap-33333333333333333","VolumeSize":1,"StartTime":"2026-01-01T00:00:00Z","State":"weird"},
                {"SnapshotId":"snap-44444444444444444","VolumeSize":1,"StartTime":"2026-01-01T00:00:00Z","StorageTier":"archive"}
            ]}"#,
        );
        let images = file("i.json", r#"{"Images":[]}"#);
        let bill =
            bill_from_inputs(&[volumes, snaps, images], &opts(Some("2026-11-09"), None)).unwrap();
        for expected in [
            "vol-11111111111111111: volume type standard is not priced",
            "vol-22222222222222222: volume type is not priced",
            "snap-11111111111111111: StartTime is missing; snapshot is not classified",
            "snap-22222222222222222: snapshot state is error; not priced",
            "snap-33333333333333333: snapshot state is not completed; not priced",
            "snap-44444444444444444: snapshot tier archive is not priced",
        ] {
            assert!(
                bill.warnings.iter().any(|line| line == expected),
                "{expected} missing from {:?}",
                bill.warnings
            );
        }
    }
}
