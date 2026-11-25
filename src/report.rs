use std::fmt::Write;

use crate::bill::{Bill, SnapshotRow, VolumeRow};
use crate::money::{format_dec, format_usd, Amount};
use crate::price::Prices;

pub const SNAPSHOT_LINE: &str = "snapshot size is an upper bound: FullSnapshotSizeInBytes when present (basis full-snapshot-bytes), otherwise VolumeSize (basis volume-size). describe-snapshots does not return changed blocks";

pub const ESTIMATE_LINE: &str =
    "Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)";

pub const NO_DELETE_LINE: &str =
    "this report identifies orphans and monthly cost. It does not delete volumes or snapshots";

#[must_use]
pub fn free_tier_line(prices: &Prices) -> String {
    let unused = &prices.not_applied;
    format!(
        "free tier not applied: the pricing page includes {} GB of storage, {} I/Os, and {} GB of snapshot storage for the account, and this export is not the whole account",
        unused.free_tier_storage_gb, unused.free_tier_io_count, unused.free_tier_snapshot_gb
    )
}

#[must_use]
pub fn not_applied_line(prices: &Prices) -> String {
    let unused = &prices.not_applied;
    format!(
        "not priced: magnetic standard volumes ({} USD per GB-month), snapshot archive storage ({} USD per GB-month) and archive retrieval ({} USD per GB), fast snapshot restore, direct APIs, time-based copy, volume initialization, and volume clones. Priced types are gp2, gp3, io1, io2, st1, sc1, and standard snapshots",
        format_dec(unused.magnetic_standard_volume_per_gb_month),
        format_dec(unused.snapshot_archive_storage_per_gb_month),
        format_dec(unused.snapshot_archive_retrieval_per_gb)
    )
}

#[must_use]
pub fn throughput_line(prices: &Prices) -> String {
    format!(
        "gp3 throughput is {} USD per MB/s-month, the meter {} USD per GiBps-month divided by 1024. The pricing page example still says 0.06",
        format_dec(prices.gp3_throughput_per_mib_month),
        format_dec(prices.gp3_throughput_per_gibps_month)
    )
}

#[must_use]
pub fn io2_line(prices: &Prices) -> String {
    format!(
        "io2 IOPS are tiered per volume: the first {} at {} USD, then to {} at {} USD, then {} USD. The pricing page example rounds the middle meter to 0.046; this tool uses the meter",
        prices.io2_iops_tier1_cap,
        format_dec(prices.io2_iops_tier1_per_iops_month),
        prices.io2_iops_tier2_cap,
        format_dec(prices.io2_iops_tier2_per_iops_month),
        format_dec(prices.io2_iops_tier3_per_iops_month)
    )
}

pub fn render(bill: &Bill, json_output: bool) -> String {
    if json_output {
        render_json(bill)
    } else {
        render_text(bill)
    }
}

/// Markdown report: the same amounts as the text report, in tables.
#[must_use]
pub fn render_markdown(bill: &Bill) -> String {
    let prices = &bill.prices;
    let mut out = String::new();
    let _ = writeln!(out, "# EBS orphan cost (estimate)\n");
    let _ = writeln!(out, "{}\n", md_text(ESTIMATE_LINE));
    let _ = writeln!(
        out,
        "- prices: {}, source {}, date {}, region {}",
        md_text(&prices.currency),
        md_text(&prices.source),
        md_text(&prices.date),
        md_text(&prices.region)
    );
    let _ = writeln!(out, "- meter published: {}", prices.meter_published);
    let as_of = bill.as_of.as_deref().unwrap_or("not set");
    let _ = writeln!(out, "- as-of: {as_of}");
    let _ = writeln!(out, "- older-than-days: {}", bill.older_than_days);
    if let Some(limit) = bill.fail_usd {
        let yn = if bill.threshold_exceeded { "yes" } else { "no" };
        let _ = writeln!(out, "- fail-usd: {}", format_dec(limit));
        let _ = writeln!(out, "- threshold exceeded: {yn}");
    }
    for line in [
        free_tier_line(prices),
        not_applied_line(prices),
        throughput_line(prices),
        io2_line(prices),
        SNAPSHOT_LINE.to_string(),
        NO_DELETE_LINE.to_string(),
    ] {
        let _ = writeln!(out, "- {}", md_text(&line));
    }
    if !bill.warnings.is_empty() {
        let _ = writeln!(out, "\n## Warnings\n");
        for warning in &bill.warnings {
            let _ = writeln!(out, "- {}", md_text(warning));
        }
    }
    let _ = writeln!(out, "\n## Volumes\n");
    push_md_table(
        &mut out,
        &volume_table(bill),
        &[false, false, true, true, true, true, true, true, true],
    );
    let _ = writeln!(
        out,
        "\nvolume estimated_usd_month={}",
        format_usd(bill.volume_usd)
    );
    let _ = writeln!(out, "\n## Snapshots\n");
    push_md_table(
        &mut out,
        &snapshot_table(bill),
        &[false, true, true, true, false, false],
    );
    let _ = writeln!(
        out,
        "\nsnapshot estimated_usd_month={}",
        format_usd(bill.snapshot_usd)
    );
    let _ = writeln!(out, "\n## Total\n");
    let _ = writeln!(out, "estimated_usd_month={}", format_usd(bill.usd_month));
    out
}

/// Makes text safe for one Markdown line or table cell: hidden characters and
/// line breaks become `\u{..}` escapes, and `|` gets a backslash.
fn md_text(text: &str) -> String {
    visible_text(text).replace('|', "\\|")
}

fn push_md_table(out: &mut String, rows: &[Vec<String>], right: &[bool]) {
    for (index, row) in rows.iter().enumerate() {
        let cells: Vec<String> = row.iter().map(|cell| md_text(cell)).collect();
        let _ = writeln!(out, "| {} |", cells.join(" | "));
        if index == 0 {
            let rule: Vec<&str> = right
                .iter()
                .map(|r| if *r { "---:" } else { "---" })
                .collect();
            let _ = writeln!(out, "| {} |", rule.join(" | "));
        }
    }
}

/// Shows invisible and direction-changing characters as `\u{..}` escapes.
fn visible_text(text: &str) -> String {
    crate::hidden::escape_hidden(text)
}

fn render_text(bill: &Bill) -> String {
    let mut out = String::new();
    for warning in &bill.warnings {
        let _ = writeln!(out, "warning: {warning}");
    }
    let prices = &bill.prices;
    let _ = writeln!(
        out,
        "prices: {}, source {}, date {}, region {}",
        prices.currency,
        visible_text(&prices.source),
        prices.date,
        prices.region
    );
    let _ = writeln!(out, "meter published: {}", prices.meter_published);
    match &bill.as_of {
        Some(date) => {
            let _ = writeln!(out, "as-of: {date}");
        }
        None => {
            let _ = writeln!(out, "as-of: not set");
        }
    }
    let _ = writeln!(out, "older-than-days: {}", bill.older_than_days);
    if let Some(limit) = bill.fail_usd {
        let yn = if bill.threshold_exceeded { "yes" } else { "no" };
        let _ = writeln!(out, "fail-usd: {}", format_dec(limit));
        let _ = writeln!(out, "threshold exceeded: {yn}");
    }
    let _ = writeln!(out, "{}", free_tier_line(prices));
    let _ = writeln!(out, "{}", not_applied_line(prices));
    let _ = writeln!(out, "{}", throughput_line(prices));
    let _ = writeln!(out, "{}", io2_line(prices));
    let _ = writeln!(out, "{SNAPSHOT_LINE}");
    let _ = writeln!(out, "{NO_DELETE_LINE}");
    let _ = writeln!(out, "{ESTIMATE_LINE}");
    out.push('\n');
    let _ = writeln!(out, "volumes");
    push_table(
        &mut out,
        &volume_table(bill),
        &[false, false, true, true, true, true, true, true, true],
    );
    let _ = writeln!(
        out,
        "volume estimated_usd_month={}",
        format_usd(bill.volume_usd)
    );
    out.push('\n');
    let _ = writeln!(out, "snapshots");
    push_table(
        &mut out,
        &snapshot_table(bill),
        &[false, true, true, true, false, false],
    );
    let _ = writeln!(
        out,
        "snapshot estimated_usd_month={}",
        format_usd(bill.snapshot_usd)
    );
    out.push('\n');
    let _ = writeln!(out, "estimated_usd_month={}", format_usd(bill.usd_month));
    let _ = writeln!(out, "note: {NO_DELETE_LINE}");
    out
}

fn volume_table(bill: &Bill) -> Vec<Vec<String>> {
    let mut rows = vec![vec![
        "id".to_string(),
        "type".to_string(),
        "size_gb".to_string(),
        "iops".to_string(),
        "mib_s".to_string(),
        "storage_usd".to_string(),
        "iops_usd".to_string(),
        "throughput_usd".to_string(),
        "usd_month".to_string(),
    ]];
    for row in &bill.volumes {
        rows.push(volume_cells(row));
    }
    rows
}

fn volume_cells(row: &VolumeRow) -> Vec<String> {
    let whole_unpriced = !row.priced;
    vec![
        row.id.clone(),
        row.volume_type.clone(),
        row.size_gb.to_string(),
        opt_u64(row.iops),
        opt_u64(row.throughput_mib),
        money_cell(row.storage_usd, whole_unpriced),
        money_cell(row.iops_usd, whole_unpriced || row.iops_unpriced),
        money_cell(row.throughput_usd, whole_unpriced),
        money_cell(row.usd_month, whole_unpriced),
    ]
}

fn snapshot_table(bill: &Bill) -> Vec<Vec<String>> {
    let mut rows = vec![vec![
        "id".to_string(),
        "age_days".to_string(),
        "gb".to_string(),
        "usd_month".to_string(),
        "size_basis".to_string(),
        "tier".to_string(),
    ]];
    for row in &bill.snapshots {
        rows.push(snapshot_cells(row));
    }
    rows
}

fn snapshot_cells(row: &SnapshotRow) -> Vec<String> {
    vec![
        row.id.clone(),
        format_dec(row.age_days),
        format_dec(row.gb),
        money_cell(row.usd_month, !row.priced),
        row.basis.as_str().to_string(),
        row.tier.clone(),
    ]
}

fn opt_u64(value: Option<u64>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

fn money_cell(value: Option<Amount>, unpriced: bool) -> String {
    match value {
        Some(value) => format_usd(value),
        None if unpriced => "unpriced".to_string(),
        None => String::new(),
    }
}

fn push_table(out: &mut String, rows: &[Vec<String>], right: &[bool]) {
    let mut widths = vec![0; rows[0].len()];
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(cell.chars().count());
        }
    }
    for row in rows {
        let mut line = String::new();
        for (index, cell) in row.iter().enumerate() {
            if index > 0 {
                line.push_str("  ");
            }
            if right[index] {
                let _ = write!(line, "{:>width$}", cell, width = widths[index]);
            } else {
                let _ = write!(line, "{:<width$}", cell, width = widths[index]);
            }
        }
        out.push_str(line.trim_end_matches([' ', '\t', '\r', '\n']));
        out.push('\n');
    }
}

/// Writes the report as pretty JSON straight into one string, byte for byte what
/// `serde_json::to_string_pretty` gives for the same object, without building a
/// tree of values or cloning a row.
fn render_json(bill: &Bill) -> String {
    let prices = &bill.prices;
    // About 400 bytes per volume row and 250 per snapshot row, so the string
    // seldom has to grow while it is written.
    let mut out = String::with_capacity(
        4096 + bill.volumes.len().saturating_mul(400) + bill.snapshots.len().saturating_mul(250),
    );
    out.push_str("{\n");
    field_raw(&mut out, "schema_version", "1");
    field_str(&mut out, "version", env!("CARGO_PKG_VERSION"));
    field_raw(&mut out, "estimate", "true");
    field_str(&mut out, "estimate_note", ESTIMATE_LINE);
    field_str(&mut out, "currency", &prices.currency);
    field_str(&mut out, "source", &visible_text(&prices.source));
    field_str(&mut out, "date", &prices.date);
    field_str(&mut out, "meter_published", &prices.meter_published);
    field_str(&mut out, "region", &prices.region);
    field_opt_str(&mut out, "as_of", bill.as_of.as_deref());
    field_raw(
        &mut out,
        "older_than_days",
        &bill.older_than_days.to_string(),
    );
    field_opt_str(
        &mut out,
        "fail_usd",
        bill.fail_usd.map(format_dec).as_deref(),
    );
    field_raw(
        &mut out,
        "threshold_exceeded",
        bool_text(bill.threshold_exceeded),
    );
    field_raw(&mut out, "images_checked", bool_text(bill.images_checked));
    field_raw(
        &mut out,
        "snapshots_classified",
        bool_text(bill.snapshots_classified),
    );
    field_raw(&mut out, "free_tier_applied", "false");
    field_str(&mut out, "free_tier", &free_tier_line(prices));
    field_str(&mut out, "not_priced", &not_applied_line(prices));
    field_str(&mut out, "gp3_throughput", &throughput_line(prices));
    field_str(&mut out, "io2_iops", &io2_line(prices));
    field_str(&mut out, "snapshot_size", SNAPSHOT_LINE);
    field_str(&mut out, "note", NO_DELETE_LINE);
    out.push_str("  \"warnings\": ");
    if bill.warnings.is_empty() {
        out.push_str("[]");
    } else {
        out.push_str("[\n");
        for (index, warning) in bill.warnings.iter().enumerate() {
            out.push_str("    ");
            push_json_string(&mut out, warning);
            out.push_str(if index + 1 < bill.warnings.len() {
                ",\n"
            } else {
                "\n"
            });
        }
        out.push_str("  ]");
    }
    out.push_str(",\n  \"volumes\": ");
    if bill.volumes.is_empty() {
        out.push_str("[]");
    } else {
        out.push_str("[\n");
        for (index, row) in bill.volumes.iter().enumerate() {
            push_volume(&mut out, row);
            out.push_str(if index + 1 < bill.volumes.len() {
                ",\n"
            } else {
                "\n"
            });
        }
        out.push_str("  ]");
    }
    out.push_str(",\n  \"snapshots\": ");
    if bill.snapshots.is_empty() {
        out.push_str("[]");
    } else {
        out.push_str("[\n");
        for (index, row) in bill.snapshots.iter().enumerate() {
            push_snapshot(&mut out, row);
            out.push_str(if index + 1 < bill.snapshots.len() {
                ",\n"
            } else {
                "\n"
            });
        }
        out.push_str("  ]");
    }
    out.push_str(",\n");
    field_str(
        &mut out,
        "volume_estimated_usd_month",
        &format_usd(bill.volume_usd),
    );
    field_str(
        &mut out,
        "snapshot_estimated_usd_month",
        &format_usd(bill.snapshot_usd),
    );
    out.push_str("  \"estimated_usd_month\": ");
    push_json_string(&mut out, &format_usd(bill.usd_month));
    out.push_str("\n}\n");
    out
}

fn bool_text(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

fn field_raw(out: &mut String, key: &str, raw: &str) {
    let _ = writeln!(out, "  \"{key}\": {raw},");
}

fn field_str(out: &mut String, key: &str, value: &str) {
    let _ = write!(out, "  \"{key}\": ");
    push_json_string(out, value);
    out.push_str(",\n");
}

fn field_opt_str(out: &mut String, key: &str, value: Option<&str>) {
    match value {
        Some(value) => field_str(out, key, value),
        None => field_raw(out, key, "null"),
    }
}

fn push_volume(out: &mut String, row: &VolumeRow) {
    out.push_str("    {\n");
    obj_str(out, "id", &row.id, true);
    obj_str(out, "type", &row.volume_type, true);
    obj_raw(out, "size_gb", &row.size_gb.to_string(), true);
    obj_raw(out, "iops", &opt_number(row.iops), true);
    obj_raw(out, "throughput_mib", &opt_number(row.throughput_mib), true);
    obj_usd(out, "storage_usd", row.storage_usd, true);
    obj_usd(out, "iops_usd", row.iops_usd, true);
    obj_usd(out, "throughput_usd", row.throughput_usd, true);
    obj_usd(out, "usd_month", row.usd_month, true);
    obj_raw(out, "priced", bool_text(row.priced), true);
    obj_raw(out, "iops_unpriced", bool_text(row.iops_unpriced), false);
    out.push_str("    }");
}

fn push_snapshot(out: &mut String, row: &SnapshotRow) {
    out.push_str("    {\n");
    obj_str(out, "id", &row.id, true);
    obj_str(out, "age_days", &format_dec(row.age_days), true);
    obj_str(out, "gb", &format_dec(row.gb), true);
    obj_usd(out, "usd_month", row.usd_month, true);
    obj_str(out, "size_basis", row.basis.as_str(), true);
    obj_str(out, "tier", &row.tier, true);
    obj_raw(out, "priced", bool_text(row.priced), false);
    out.push_str("    }");
}

fn opt_number(value: Option<u64>) -> String {
    value.map_or_else(|| "null".to_string(), |number| number.to_string())
}

fn obj_raw(out: &mut String, key: &str, raw: &str, comma: bool) {
    let _ = writeln!(
        out,
        "      \"{key}\": {raw}{}",
        if comma { "," } else { "" }
    );
}

fn obj_str(out: &mut String, key: &str, value: &str, comma: bool) {
    let _ = write!(out, "      \"{key}\": ");
    push_json_string(out, value);
    out.push_str(if comma { ",\n" } else { "\n" });
}

fn obj_usd(out: &mut String, key: &str, value: Option<Amount>, comma: bool) {
    match value {
        Some(value) => obj_str(out, key, &format_usd(value), comma),
        None => obj_raw(out, key, "null", comma),
    }
}

/// A JSON string with the escapes `serde_json` uses: `\"`, `\\`, `\b`, `\f`,
/// `\n`, `\r`, `\t`, and `\u00xx` for the other control characters below U+0020.
fn push_json_string(out: &mut String, text: &str) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if (ch as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", ch as u32);
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    #[test]
    fn json_strings_are_escaped_like_serde_json() {
        let mut all = String::new();
        for code in 0..0x80_u32 {
            all.push(char::from_u32(code).expect("char"));
        }
        all.push_str("é\u{2028}\u{202e}\u{1f600}\"\\");
        for text in ["", "plain", all.as_str()] {
            let mut mine = String::new();
            super::push_json_string(&mut mine, text);
            assert_eq!(mine, serde_json::to_string(text).expect("serde"));
        }
    }

    use super::{free_tier_line, io2_line, not_applied_line, throughput_line, NO_DELETE_LINE};
    use crate::price::prices_builtin;

    #[test]
    fn visible_text_escapes_the_variation_selector_supplement() {
        for ch in ['\u{E0100}', '\u{E0150}', '\u{E01EF}', '\u{2028}'] {
            let shown = super::visible_text(&format!("a{ch}b"));
            assert_eq!(
                shown,
                format!("a\\u{{{:x}}}b", ch as u32),
                "{:X}",
                ch as u32
            );
        }
        assert_eq!(super::visible_text("a\u{E01F0}b"), "a\u{E01F0}b");
    }

    #[test]
    fn visible_text_escapes_bidi_and_keeps_plain_text() {
        assert_eq!(super::visible_text("a\u{202E}b"), "a\\u{202e}b");
        assert_eq!(
            super::visible_text("https://x.example/p"),
            "https://x.example/p"
        );
    }

    #[test]
    fn visible_text_escapes_each_added_invisible_char() {
        let added = [
            '\u{115F}', '\u{1160}', '\u{17B4}', '\u{17B5}', '\u{180B}', '\u{180C}', '\u{180D}',
            '\u{FE00}', '\u{FE01}', '\u{FE02}', '\u{FE03}', '\u{FE04}', '\u{FE05}', '\u{FE06}',
            '\u{FE07}', '\u{FE08}', '\u{FE09}', '\u{FE0A}', '\u{FE0B}', '\u{FE0C}', '\u{FE0D}',
            '\u{FE0E}', '\u{FFA0}', '\u{FFF9}', '\u{FFFA}', '\u{FFFB}',
        ];
        for c in added {
            let got = super::visible_text(&format!("a{c}b"));
            assert_eq!(got, format!("a{}b", c.escape_unicode()), "{:x}", c as u32);
            assert!(!got.contains(c));
        }
    }

    #[test]
    fn push_table_aligns_columns_by_chars_not_bytes() {
        let rows = vec![
            vec!["name".to_string(), "n".to_string()],
            vec!["é-ü".to_string(), "1".to_string()],
            vec!["abc".to_string(), "22".to_string()],
        ];
        let mut out = String::new();
        super::push_table(&mut out, &rows, &[false, true]);
        assert_eq!(out, "name   n\né-ü    1\nabc   22\n");
    }

    #[test]
    fn push_table_keeps_trailing_nbsp_and_strips_ascii_space() {
        let rows = vec![
            vec!["a".to_string(), "x\u{a0}".to_string()],
            vec!["b".to_string(), String::new()],
        ];
        let mut out = String::new();
        super::push_table(&mut out, &rows, &[false, false]);
        assert_eq!(out, "a  x\u{a0}\nb\n");
    }

    #[test]
    fn builtin_lines_name_the_page_and_do_not_issue_a_delete() {
        let prices = prices_builtin();
        let free = free_tier_line(&prices);
        assert!(free.contains("30 GB"));
        assert!(free.contains("2000000 I/Os"));
        assert!(free.contains("1 GB"));
        assert!(free.contains("not applied"));
        let unused = not_applied_line(&prices);
        assert!(unused.contains("0.05"));
        assert!(unused.contains("0.0125"));
        assert!(unused.contains("0.03"));
        assert!(unused.contains("gp3"));
        let throughput = throughput_line(&prices);
        assert!(throughput.contains("0.04"));
        assert!(throughput.contains("40.96"));
        assert!(throughput.contains("0.06"));
        let io2 = io2_line(&prices);
        assert!(io2.contains("0.0455"));
        assert!(io2.contains("0.046"));
        assert!(io2.contains("0.03185"));
        assert!(NO_DELETE_LINE.contains("does not delete"));
        assert!(!NO_DELETE_LINE.contains('-'));
        let blob = format!("{free}\n{unused}\n{throughput}\n{io2}\n{NO_DELETE_LINE}");
        assert!(!blob.contains("aws ec2"));
    }
}
