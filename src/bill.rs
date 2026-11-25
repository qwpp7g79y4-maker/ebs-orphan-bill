use rust_decimal::Decimal;

use crate::error::Error;
use crate::money::{exact_add, exact_mul, Amount};
use crate::price::Prices;

/// Volume types this tool prices. Anything else is listed and not priced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeKind {
    Gp2,
    Gp3,
    Io1,
    Io2,
    St1,
    Sc1,
    Unpriced,
}

#[must_use]
pub fn kind_of(volume_type: &str) -> VolumeKind {
    match volume_type {
        "gp2" => VolumeKind::Gp2,
        "gp3" => VolumeKind::Gp3,
        "io1" => VolumeKind::Io1,
        "io2" => VolumeKind::Io2,
        "st1" => VolumeKind::St1,
        "sc1" => VolumeKind::Sc1,
        _ => VolumeKind::Unpriced,
    }
}

/// Why a gp3 or io field was filled in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteNote {
    Gp3IopsBaseline,
    Gp3ThroughputBaseline,
    IopsNotPriced,
}

/// One loose volume's monthly parts. `None` means that part is not priced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quote {
    pub storage_usd: Option<Amount>,
    pub iops_usd: Option<Amount>,
    pub throughput_usd: Option<Amount>,
    pub usd_month: Option<Amount>,
    pub priced: bool,
    pub iops: Option<u64>,
    pub throughput_mib: Option<u64>,
    pub notes: Vec<QuoteNote>,
}

/// How a snapshot's gigabytes were chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotBasis {
    FullSnapshotBytes,
    VolumeSize,
}

impl SnapshotBasis {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FullSnapshotBytes => "full-snapshot-bytes",
            Self::VolumeSize => "volume-size",
        }
    }
}

fn mul(left: Decimal, right: Decimal) -> Result<Amount, Error> {
    kept(exact_mul(left, right))
}

fn add(left: Amount, right: Amount) -> Result<Amount, Error> {
    kept(exact_add(left, right))
}

/// Sum of three amounts. Only the final sum must fit in cents, so an
/// intermediate sum that does not fit cannot fail a total that does.
fn add3(first: Amount, second: Amount, third: Amount) -> Result<Amount, Error> {
    let partial = exact_add(first, second).ok_or_else(|| Error::new("amount overflowed"))?;
    add(partial, third)
}

fn kept(amount: Option<Amount>) -> Result<Amount, Error> {
    match amount {
        Some(amount) if amount.cents_fit() => Ok(amount),
        _ => Err(Error::new("amount overflowed")),
    }
}

pub fn quote_volume(
    kind: VolumeKind,
    size_gb: u64,
    iops: Option<u64>,
    throughput_mib: Option<u64>,
    prices: &Prices,
) -> Result<Quote, Error> {
    let size = Decimal::from(size_gb);
    match kind {
        VolumeKind::Unpriced => Ok(Quote::unpriced(iops, throughput_mib)),
        VolumeKind::Gp2 => Ok(Quote::storage_only(mul(
            size,
            prices.gp2_storage_per_gb_month,
        )?)),
        VolumeKind::St1 => Ok(Quote::storage_only(mul(
            size,
            prices.st1_storage_per_gb_month,
        )?)),
        VolumeKind::Sc1 => Ok(Quote::storage_only(mul(
            size,
            prices.sc1_storage_per_gb_month,
        )?)),
        VolumeKind::Gp3 => quote_gp3(size, iops, throughput_mib, prices),
        VolumeKind::Io1 => quote_io(
            mul(size, prices.io1_storage_per_gb_month)?,
            iops,
            prices,
            false,
        ),
        VolumeKind::Io2 => quote_io(
            mul(size, prices.io2_storage_per_gb_month)?,
            iops,
            prices,
            true,
        ),
    }
}

fn quote_gp3(
    size: Decimal,
    iops: Option<u64>,
    throughput: Option<u64>,
    prices: &Prices,
) -> Result<Quote, Error> {
    let mut notes = Vec::new();
    let iops = match iops {
        Some(value) => value,
        None => {
            notes.push(QuoteNote::Gp3IopsBaseline);
            prices.gp3_iops_baseline
        }
    };
    let throughput = match throughput {
        Some(value) => value,
        None => {
            notes.push(QuoteNote::Gp3ThroughputBaseline);
            prices.gp3_throughput_baseline_mib
        }
    };
    let extra_iops = iops.saturating_sub(prices.gp3_iops_baseline);
    let extra_mib = throughput.saturating_sub(prices.gp3_throughput_baseline_mib);
    let storage = mul(size, prices.gp3_storage_per_gb_month)?;
    let iops_usd = mul(Decimal::from(extra_iops), prices.gp3_iops_per_iops_month)?;
    let throughput_usd = mul(
        Decimal::from(extra_mib),
        prices.gp3_throughput_per_mib_month,
    )?;
    let usd_month = add3(storage, iops_usd, throughput_usd)?;
    Ok(Quote {
        storage_usd: Some(storage),
        iops_usd: Some(iops_usd),
        throughput_usd: Some(throughput_usd),
        usd_month: Some(usd_month),
        priced: true,
        iops: Some(iops),
        throughput_mib: Some(throughput),
        notes,
    })
}

fn quote_io(
    storage: Amount,
    iops: Option<u64>,
    prices: &Prices,
    io2: bool,
) -> Result<Quote, Error> {
    let Some(iops) = iops else {
        return Ok(Quote {
            storage_usd: Some(storage),
            iops_usd: None,
            throughput_usd: None,
            usd_month: Some(storage),
            priced: true,
            iops: None,
            throughput_mib: None,
            notes: vec![QuoteNote::IopsNotPriced],
        });
    };
    let iops_usd = if io2 {
        io2_iops_usd(iops, prices)?
    } else {
        mul(Decimal::from(iops), prices.io1_iops_per_iops_month)?
    };
    let usd_month = add(storage, iops_usd)?;
    Ok(Quote {
        storage_usd: Some(storage),
        iops_usd: Some(iops_usd),
        throughput_usd: None,
        usd_month: Some(usd_month),
        priced: true,
        iops: Some(iops),
        throughput_mib: None,
        notes: Vec::new(),
    })
}

/// io2 IOPS tiers apply to one volume. Caps and rates come from `prices`.
pub fn io2_iops_usd(iops: u64, prices: &Prices) -> Result<Amount, Error> {
    let cap1 = prices.io2_iops_tier1_cap;
    let cap2 = prices.io2_iops_tier2_cap;
    let tier1 = iops.min(cap1);
    let tier2 = iops.saturating_sub(cap1).min(cap2.saturating_sub(cap1));
    let tier3 = iops.saturating_sub(cap2);
    let first = mul(Decimal::from(tier1), prices.io2_iops_tier1_per_iops_month)?;
    let second = mul(Decimal::from(tier2), prices.io2_iops_tier2_per_iops_month)?;
    let third = mul(Decimal::from(tier3), prices.io2_iops_tier3_per_iops_month)?;
    add3(first, second, third)
}

/// Standard-snapshot storage. `gb` is already in the page's GB unit.
pub fn quote_snapshot(gb: Decimal, prices: &Prices) -> Result<Amount, Error> {
    mul(gb, prices.snapshot_standard_per_gb_month)
}

/// Age is strictly greater than `days` when `age_ms` is positive milliseconds.
#[must_use]
pub fn older_than(age_ms: i64, days: u32) -> bool {
    if age_ms < 0 {
        return false;
    }
    let limit = i128::from(days) * 86_400_000;
    i128::from(age_ms) > limit
}

impl Quote {
    fn unpriced(iops: Option<u64>, throughput_mib: Option<u64>) -> Self {
        Self {
            storage_usd: None,
            iops_usd: None,
            throughput_usd: None,
            usd_month: None,
            priced: false,
            iops,
            throughput_mib,
            notes: Vec::new(),
        }
    }

    fn storage_only(storage: Amount) -> Self {
        Self {
            storage_usd: Some(storage),
            iops_usd: None,
            throughput_usd: None,
            usd_month: Some(storage),
            priced: true,
            iops: None,
            throughput_mib: None,
            notes: Vec::new(),
        }
    }
}

/// One loose volume on the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeRow {
    pub id: String,
    pub volume_type: String,
    pub size_gb: u64,
    pub iops: Option<u64>,
    pub throughput_mib: Option<u64>,
    pub storage_usd: Option<Amount>,
    pub iops_usd: Option<Amount>,
    pub throughput_usd: Option<Amount>,
    pub usd_month: Option<Amount>,
    pub priced: bool,
    pub iops_unpriced: bool,
}

/// One old snapshot on the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotRow {
    pub id: String,
    pub age_days: Decimal,
    pub gb: Decimal,
    pub usd_month: Option<Amount>,
    pub basis: SnapshotBasis,
    pub tier: String,
    pub priced: bool,
}

/// The whole report before it is rendered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bill {
    pub prices: Prices,
    pub as_of: Option<String>,
    pub older_than_days: u32,
    pub fail_usd: Option<Decimal>,
    pub threshold_exceeded: bool,
    pub images_checked: bool,
    pub snapshots_classified: bool,
    pub volumes: Vec<VolumeRow>,
    pub snapshots: Vec<SnapshotRow>,
    pub volume_usd: Amount,
    pub snapshot_usd: Amount,
    pub usd_month: Amount,
    pub warnings: Vec<String>,
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::{
        io2_iops_usd, kind_of, older_than, quote_snapshot, quote_volume, QuoteNote, SnapshotBasis,
        VolumeKind,
    };
    use crate::money::{format_usd, gb_from_bytes, Amount};
    use crate::price::prices_builtin;

    fn dec(text: &str) -> Decimal {
        text.parse().unwrap()
    }

    fn usd(text: &str) -> String {
        format_usd(dec(text))
    }

    #[test]
    fn gp3_sample_and_the_page_full_month() {
        let prices = prices_builtin();
        let sample = quote_volume(VolumeKind::Gp3, 100, Some(4000), Some(200), &prices).unwrap();
        assert_eq!(format_usd(sample.storage_usd.unwrap()), "8.00");
        assert_eq!(format_usd(sample.iops_usd.unwrap()), "5.00");
        assert_eq!(format_usd(sample.throughput_usd.unwrap()), "3.00");
        assert_eq!(format_usd(sample.usd_month.unwrap()), "16.00");
        let page = quote_volume(VolumeKind::Gp3, 2000, Some(10_000), Some(500), &prices).unwrap();
        assert_eq!(format_usd(page.storage_usd.unwrap()), "160.00");
        assert_eq!(format_usd(page.iops_usd.unwrap()), "35.00");
        assert_eq!(format_usd(page.throughput_usd.unwrap()), "15.00");
        assert_eq!(format_usd(page.usd_month.unwrap()), "210.00");
        assert_eq!(format_usd(page.throughput_usd.unwrap()), "15.00");
    }

    #[test]
    fn gp3_baseline_is_free_and_missing_fields_use_it() {
        let prices = prices_builtin();
        let under = quote_volume(VolumeKind::Gp3, 1, Some(2999), Some(125), &prices).unwrap();
        assert_eq!(under.iops_usd.unwrap(), Amount::ZERO);
        assert_eq!(under.throughput_usd.unwrap(), Amount::ZERO);
        assert!(under.notes.is_empty());
        let missing = quote_volume(VolumeKind::Gp3, 10, None, None, &prices).unwrap();
        assert_eq!(missing.iops, Some(3000));
        assert_eq!(missing.throughput_mib, Some(125));
        assert_eq!(missing.iops_usd.unwrap(), Amount::ZERO);
        assert_eq!(missing.throughput_usd.unwrap(), Amount::ZERO);
        assert_eq!(format_usd(missing.storage_usd.unwrap()), "0.80");
        assert_eq!(
            missing.notes,
            vec![QuoteNote::Gp3IopsBaseline, QuoteNote::Gp3ThroughputBaseline]
        );
    }

    #[test]
    fn gp2_st1_and_sc1_full_months() {
        let prices = prices_builtin();
        assert_eq!(
            format_usd(
                quote_volume(VolumeKind::Gp2, 2000, None, None, &prices)
                    .unwrap()
                    .usd_month
                    .unwrap()
            ),
            "200.00"
        );
        assert_eq!(
            format_usd(
                quote_volume(VolumeKind::Gp2, 50, None, None, &prices)
                    .unwrap()
                    .usd_month
                    .unwrap()
            ),
            "5.00"
        );
        assert_eq!(
            format_usd(
                quote_volume(VolumeKind::Gp2, 30, None, None, &prices)
                    .unwrap()
                    .usd_month
                    .unwrap()
            ),
            "3.00"
        );
        assert_eq!(
            format_usd(
                quote_volume(VolumeKind::St1, 2000, None, None, &prices)
                    .unwrap()
                    .usd_month
                    .unwrap()
            ),
            "90.00"
        );
        assert_eq!(
            format_usd(
                quote_volume(VolumeKind::Sc1, 2000, None, None, &prices)
                    .unwrap()
                    .usd_month
                    .unwrap()
            ),
            "30.00"
        );
    }

    #[test]
    fn io1_full_month_and_missing_iops() {
        let prices = prices_builtin();
        let quote = quote_volume(VolumeKind::Io1, 2000, Some(1000), None, &prices).unwrap();
        assert_eq!(format_usd(quote.storage_usd.unwrap()), "250.00");
        assert_eq!(format_usd(quote.iops_usd.unwrap()), "65.00");
        assert_eq!(format_usd(quote.usd_month.unwrap()), "315.00");
        let missing = quote_volume(VolumeKind::Io1, 8, None, None, &prices).unwrap();
        assert!(missing.iops_usd.is_none());
        assert_eq!(format_usd(missing.usd_month.unwrap()), "1.00");
        assert_eq!(missing.notes, vec![QuoteNote::IopsNotPriced]);
    }

    #[test]
    fn io2_tiers_use_the_meter_not_the_rounded_example() {
        let prices = prices_builtin();
        assert_eq!(
            format_usd(io2_iops_usd(32_000, &prices).unwrap()),
            "2080.00"
        );
        assert!(io2_iops_usd(32_001, &prices)
            .unwrap()
            .eq_decimal(dec("2080.0455")));
        assert_eq!(
            format_usd(io2_iops_usd(32_001, &prices).unwrap()),
            "2080.05"
        );
        assert_eq!(
            format_usd(io2_iops_usd(60_000, &prices).unwrap()),
            "3354.00"
        );
        assert!(io2_iops_usd(60_000, &prices)
            .unwrap()
            .eq_decimal(dec("3354")));
        assert_eq!(
            format_usd(io2_iops_usd(64_000, &prices).unwrap()),
            "3536.00"
        );
        assert!(io2_iops_usd(64_001, &prices)
            .unwrap()
            .eq_decimal(dec("3536.03185")));
        assert_eq!(
            format_usd(io2_iops_usd(64_001, &prices).unwrap()),
            "3536.03"
        );
        let one = io2_iops_usd(80_000, &prices).unwrap();
        let split = crate::money::exact_add(
            io2_iops_usd(40_000, &prices).unwrap(),
            io2_iops_usd(40_000, &prices).unwrap(),
        )
        .unwrap();
        assert_ne!(one, split);
        let with_storage =
            quote_volume(VolumeKind::Io2, 2000, Some(60_000), None, &prices).unwrap();
        assert_eq!(format_usd(with_storage.storage_usd.unwrap()), "250.00");
        assert_eq!(format_usd(with_storage.iops_usd.unwrap()), "3354.00");
    }

    #[test]
    fn magnetic_is_not_priced_at_the_recorded_rate() {
        let prices = prices_builtin();
        assert_eq!(kind_of("standard"), VolumeKind::Unpriced);
        assert_eq!(kind_of("gp3"), VolumeKind::Gp3);
        let quote = quote_volume(VolumeKind::Unpriced, 20, None, None, &prices).unwrap();
        assert!(!quote.priced);
        assert!(quote.usd_month.is_none());
        assert!(quote.storage_usd.is_none());
    }

    #[test]
    fn snapshots_use_standard_storage_and_full_bytes() {
        let prices = prices_builtin();
        assert_eq!(
            format_usd(quote_snapshot(dec("10"), &prices).unwrap()),
            "0.50"
        );
        assert_eq!(
            format_usd(quote_snapshot(Decimal::ONE, &prices).unwrap()),
            "0.05"
        );
        let one_byte = gb_from_bytes(Decimal::ONE);
        let tiny = quote_snapshot(one_byte, &prices).unwrap();
        assert_eq!(format_usd(tiny), "0.00");
        assert!(tiny.gt_decimal(Decimal::ZERO));
        assert_eq!(
            SnapshotBasis::FullSnapshotBytes.as_str(),
            "full-snapshot-bytes"
        );
        assert_eq!(SnapshotBasis::VolumeSize.as_str(), "volume-size");
        assert_eq!(
            format_usd(quote_snapshot(dec("80"), &prices).unwrap()),
            "4.00"
        );
    }

    #[test]
    fn age_is_strict_and_display_rounds_half_away() {
        let day = 86_400_000;
        assert!(!older_than(30 * day, 30));
        assert!(older_than(30 * day + 1_000, 30));
        assert!(older_than(31 * day, 30));
        assert!(!older_than(0, 0));
        assert!(older_than(1, 0));
        assert!(!older_than(-1, 30));
        assert_eq!(usd("0.005"), "0.01");
        assert_eq!(usd("0.004"), "0.00");
    }

    #[test]
    fn extreme_rate_on_a_small_volume_is_an_error() {
        let mut prices = prices_builtin();
        prices.gp2_storage_per_gb_month = Decimal::MAX;
        let err = quote_volume(VolumeKind::Gp2, 2, None, None, &prices).unwrap_err();
        assert_eq!(err.message(), "amount overflowed");
    }
}
