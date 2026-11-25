use rust_decimal::Decimal;
use serde_json::{Map, Value};

use crate::error::Error;
use crate::json_scan::{top_level_keys, KeyFault, MAX_JSON_ITEMS};
use crate::money::{loses_digits, parse_money, too_precise, trim_ascii_ws, valid_date};

/// Rates recorded from the pricing page and not used to price a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotApplied {
    pub snapshot_archive_storage_per_gb_month: Decimal,
    pub snapshot_archive_retrieval_per_gb: Decimal,
    pub magnetic_standard_volume_per_gb_month: Decimal,
    pub free_tier_storage_gb: u64,
    pub free_tier_snapshot_gb: u64,
    pub free_tier_io_count: u64,
}

/// One region's EBS quote. `not_applied` is stored so a report can name it.
/// Billing does not read those fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prices {
    pub currency: String,
    pub source: String,
    pub date: String,
    pub meter_published: String,
    pub region: String,
    pub gb_bytes: u64,
    pub gp3_storage_per_gb_month: Decimal,
    pub gp3_iops_per_iops_month: Decimal,
    pub gp3_iops_baseline: u64,
    pub gp3_throughput_per_gibps_month: Decimal,
    pub gp3_throughput_per_mib_month: Decimal,
    pub gp3_throughput_baseline_mib: u64,
    pub gp2_storage_per_gb_month: Decimal,
    pub io1_storage_per_gb_month: Decimal,
    pub io1_iops_per_iops_month: Decimal,
    pub io2_storage_per_gb_month: Decimal,
    pub io2_iops_tier1_per_iops_month: Decimal,
    pub io2_iops_tier2_per_iops_month: Decimal,
    pub io2_iops_tier3_per_iops_month: Decimal,
    pub io2_iops_tier1_cap: u64,
    pub io2_iops_tier2_cap: u64,
    pub st1_storage_per_gb_month: Decimal,
    pub sc1_storage_per_gb_month: Decimal,
    pub snapshot_standard_per_gb_month: Decimal,
    pub not_applied: NotApplied,
}

const BUILTIN_JSON: &str = include_str!("../prices/ebs.json");

/// US East (N. Virginia) rates from the AWS EBS pricing page, read 2026-11-09.
///
/// The page's price file was published 2026-11-07. gp3 throughput per MB/s is
/// the GiBps meter divided by 1024. Magnetic volumes, archive snapshots, and
/// the account free tier are on the page and are not applied.
///
/// Source: <https://aws.amazon.com/ebs/pricing/>
#[must_use]
pub fn prices_builtin() -> Prices {
    load_prices_str(BUILTIN_JSON).expect("built-in prices")
}

pub fn load_prices_str(text: &str) -> Result<Prices, Error> {
    let text = strip_bom(text);
    match top_level_keys(text) {
        Ok(_) => {}
        Err(KeyFault::Duplicate) => return Err(Error::new("duplicate key")),
        Err(KeyFault::Bad) => return Err(Error::new("not JSON")),
        Err(KeyFault::TooMany) => {
            return Err(Error::new(format!(
                "too many JSON values (at most {MAX_JSON_ITEMS})"
            )))
        }
    }
    let value: Value = serde_json::from_str(text).map_err(|_| Error::new("not JSON"))?;
    let Some(obj) = value.as_object() else {
        return Err(Error::new("expected an object"));
    };
    reject_apply_flag(obj)?;
    let per_gibps = req_decimal(obj, "gp3_throughput_per_gibps_month")?;
    let per_mib = req_decimal(obj, "gp3_throughput_per_mib_month")?;
    if per_mib != per_gibps / Decimal::from(1024) {
        return Err(Error::new(
            "gp3 throughput per MiB does not match the GiBps meter divided by 1024",
        ));
    }
    let gb_bytes = req_u64(obj, "gb_bytes")?;
    if gb_bytes != 1_073_741_824 {
        return Err(Error::new("gb_bytes must be 1073741824"));
    }
    let tier1 = req_u64(obj, "io2_iops_tier1_cap")?;
    let tier2 = req_u64(obj, "io2_iops_tier2_cap")?;
    if tier2 <= tier1 {
        return Err(Error::new("io2 tier caps must increase"));
    }
    Ok(Prices {
        currency: req_currency(obj)?,
        source: req_source(obj)?,
        date: req_date(obj, "date")?,
        meter_published: req_date(obj, "meter_published")?,
        region: req_label(obj, "region")?,
        gb_bytes,
        gp3_storage_per_gb_month: req_decimal(obj, "gp3_storage_per_gb_month")?,
        gp3_iops_per_iops_month: req_decimal(obj, "gp3_iops_per_iops_month")?,
        gp3_iops_baseline: req_u64(obj, "gp3_iops_baseline")?,
        gp3_throughput_per_gibps_month: per_gibps,
        gp3_throughput_per_mib_month: per_mib,
        gp3_throughput_baseline_mib: req_u64(obj, "gp3_throughput_baseline_mib")?,
        gp2_storage_per_gb_month: req_decimal(obj, "gp2_storage_per_gb_month")?,
        io1_storage_per_gb_month: req_decimal(obj, "io1_storage_per_gb_month")?,
        io1_iops_per_iops_month: req_decimal(obj, "io1_iops_per_iops_month")?,
        io2_storage_per_gb_month: req_decimal(obj, "io2_storage_per_gb_month")?,
        io2_iops_tier1_per_iops_month: req_decimal(obj, "io2_iops_tier1_per_iops_month")?,
        io2_iops_tier2_per_iops_month: req_decimal(obj, "io2_iops_tier2_per_iops_month")?,
        io2_iops_tier3_per_iops_month: req_decimal(obj, "io2_iops_tier3_per_iops_month")?,
        io2_iops_tier1_cap: tier1,
        io2_iops_tier2_cap: tier2,
        st1_storage_per_gb_month: req_decimal(obj, "st1_storage_per_gb_month")?,
        sc1_storage_per_gb_month: req_decimal(obj, "sc1_storage_per_gb_month")?,
        snapshot_standard_per_gb_month: req_decimal(obj, "snapshot_standard_per_gb_month")?,
        not_applied: load_not_applied(obj)?,
    })
}

fn load_not_applied(obj: &Map<String, Value>) -> Result<NotApplied, Error> {
    let Some(value) = obj.get("not_applied") else {
        return Err(Error::new("missing not_applied"));
    };
    let Some(inner) = value.as_object() else {
        return Err(Error::new("bad not_applied"));
    };
    Ok(NotApplied {
        snapshot_archive_storage_per_gb_month: req_decimal(
            inner,
            "snapshot_archive_storage_per_gb_month",
        )?,
        snapshot_archive_retrieval_per_gb: req_decimal(inner, "snapshot_archive_retrieval_per_gb")?,
        magnetic_standard_volume_per_gb_month: req_decimal(
            inner,
            "magnetic_standard_volume_per_gb_month",
        )?,
        free_tier_storage_gb: req_u64(inner, "free_tier_storage_gb")?,
        free_tier_snapshot_gb: req_u64(inner, "free_tier_snapshot_gb")?,
        free_tier_io_count: req_u64(inner, "free_tier_io_count")?,
    })
}

fn reject_apply_flag(obj: &Map<String, Value>) -> Result<(), Error> {
    match obj.get("apply_not_applied") {
        None | Some(Value::Bool(false)) => Ok(()),
        Some(Value::Bool(true)) => Err(Error::new("apply_not_applied cannot be true")),
        Some(_) => Err(Error::new("bad apply_not_applied")),
    }
}

fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

fn req_currency(obj: &Map<String, Value>) -> Result<String, Error> {
    let text = req_string(obj, "currency")?;
    if text.len() == 3 && text.bytes().all(|b| b.is_ascii_uppercase()) {
        Ok(text)
    } else {
        Err(Error::new("bad currency"))
    }
}

fn req_source(obj: &Map<String, Value>) -> Result<String, Error> {
    let text = req_string(obj, "source")?;
    if (1..=500).contains(&text.chars().count()) && !text.chars().any(|c| c.is_control()) {
        Ok(text)
    } else {
        Err(Error::new("bad source"))
    }
}

fn req_date(obj: &Map<String, Value>, key: &str) -> Result<String, Error> {
    let text = req_string(obj, key)?;
    if valid_date(&text) {
        Ok(text)
    } else {
        Err(Error::new(format!("bad {key}")))
    }
}

fn req_label(obj: &Map<String, Value>, key: &str) -> Result<String, Error> {
    let text = req_string(obj, key)?;
    let ok = (1..=64).contains(&text.len())
        && text.chars().all(|c| c.is_ascii_graphic() || c == ' ')
        && !text.chars().any(|c| c.is_control());
    if ok {
        Ok(text)
    } else {
        Err(Error::new(format!("bad {key}")))
    }
}

fn req_decimal(obj: &Map<String, Value>, key: &str) -> Result<Decimal, Error> {
    let Some(value) = obj.get(key) else {
        return Err(Error::new(format!("missing {key}")));
    };
    let raw = match value {
        Value::String(text) => trim_ascii_ws(text).to_string(),
        Value::Number(number) => number.to_string(),
        _ => return Err(Error::new(format!("bad {key}"))),
    };
    if too_precise(&raw) {
        return Err(Error::new(format!(
            "{key} does not fit: more than 28 decimals"
        )));
    }
    let value = parse_money(&raw).map_err(|()| Error::new(format!("bad {key}")))?;
    if loses_digits(&raw) {
        return Err(Error::new(format!(
            "{key} does not fit: more digits than a decimal holds exactly"
        )));
    }
    Ok(value)
}

fn req_u64(obj: &Map<String, Value>, key: &str) -> Result<u64, Error> {
    let Some(value) = obj.get(key) else {
        return Err(Error::new(format!("missing {key}")));
    };
    let text = match value {
        Value::Number(number) => number.to_string(),
        Value::String(text) => trim_ascii_ws(text).to_string(),
        _ => return Err(Error::new(format!("bad {key}"))),
    };
    if text.is_empty() || text.len() > 20 || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Error::new(format!("bad {key}")));
    }
    text.parse().map_err(|_| Error::new(format!("bad {key}")))
}

fn req_string(obj: &Map<String, Value>, key: &str) -> Result<String, Error> {
    let Some(value) = obj.get(key) else {
        return Err(Error::new(format!("missing {key}")));
    };
    value
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| Error::new(format!("bad {key}")))
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::{load_prices_str, prices_builtin};

    fn builtin_text() -> &'static str {
        include_str!("../prices/ebs.json")
    }

    fn dec(text: &str) -> Decimal {
        text.parse().unwrap()
    }

    #[test]
    fn builtin_matches_the_pricing_page() {
        let prices = prices_builtin();
        assert_eq!(prices.currency, "USD");
        assert_eq!(prices.source, "https://aws.amazon.com/ebs/pricing/");
        assert_eq!(prices.date, "2026-11-09");
        assert_eq!(prices.meter_published, "2026-11-07");
        assert_eq!(prices.region, "us-east-1");
        assert_eq!(prices.gb_bytes, 1_073_741_824);
        assert_eq!(prices.gp3_storage_per_gb_month, dec("0.08"));
        assert_eq!(prices.gp3_iops_per_iops_month, dec("0.005"));
        assert_eq!(prices.gp3_iops_baseline, 3000);
        assert_eq!(prices.gp3_throughput_per_gibps_month, dec("40.96"));
        assert_eq!(prices.gp3_throughput_per_mib_month, dec("0.04"));
        assert_eq!(prices.gp3_throughput_baseline_mib, 125);
        assert_eq!(prices.gp2_storage_per_gb_month, dec("0.10"));
        assert_eq!(prices.io1_storage_per_gb_month, dec("0.125"));
        assert_eq!(prices.io1_iops_per_iops_month, dec("0.065"));
        assert_eq!(prices.io2_storage_per_gb_month, dec("0.125"));
        assert_eq!(prices.io2_iops_tier1_per_iops_month, dec("0.065"));
        assert_eq!(prices.io2_iops_tier2_per_iops_month, dec("0.0455"));
        assert_eq!(prices.io2_iops_tier3_per_iops_month, dec("0.03185"));
        assert_eq!(prices.io2_iops_tier1_cap, 32_000);
        assert_eq!(prices.io2_iops_tier2_cap, 64_000);
        assert_eq!(prices.st1_storage_per_gb_month, dec("0.045"));
        assert_eq!(prices.sc1_storage_per_gb_month, dec("0.015"));
        assert_eq!(prices.snapshot_standard_per_gb_month, dec("0.05"));
        assert_eq!(
            prices.not_applied.snapshot_archive_storage_per_gb_month,
            dec("0.0125")
        );
        assert_eq!(
            prices.not_applied.snapshot_archive_retrieval_per_gb,
            dec("0.03")
        );
        assert_eq!(
            prices.not_applied.magnetic_standard_volume_per_gb_month,
            dec("0.05")
        );
        assert_eq!(prices.not_applied.free_tier_storage_gb, 30);
        assert_eq!(prices.not_applied.free_tier_snapshot_gb, 1);
        assert_eq!(prices.not_applied.free_tier_io_count, 2_000_000);
        let text = builtin_text();
        assert!(text.contains("https://aws.amazon.com/ebs/pricing/"));
        assert!(text.contains("2026-11-09"));
        assert!(text.contains("2026-11-07"));
        assert!(text.contains("\"apply_not_applied\": false"));
        assert!(!text.contains("\"0.06\""));
        assert!(!text.contains("0.046"));
    }

    #[test]
    fn throughput_meter_divides_by_1024() {
        let prices = prices_builtin();
        let per_mib = prices.gp3_throughput_per_gibps_month / Decimal::from(1024);
        assert_eq!(per_mib, dec("0.04"));
        assert_eq!(prices.gp3_throughput_per_mib_month, per_mib);
    }

    #[test]
    fn rejects_a_throughput_that_does_not_match_the_meter() {
        let text = builtin_text().replace(
            "\"gp3_throughput_per_mib_month\": \"0.04\"",
            "\"gp3_throughput_per_mib_month\": \"0.06\"",
        );
        let err = load_prices_str(&text).unwrap_err();
        assert_eq!(
            err.message(),
            "gp3 throughput per MiB does not match the GiBps meter divided by 1024"
        );
    }

    #[test]
    fn rejects_exponent_negative_and_wrong_gb() {
        let exp = builtin_text().replace("\"0.08\"", "\"1e-7\"");
        let err = load_prices_str(&exp).unwrap_err();
        assert_eq!(err.message(), "bad gp3_storage_per_gb_month");
        assert!(!err.message().contains("1e-7"));
        let neg = builtin_text().replace("\"0.08\"", "\"-0.08\"");
        assert_eq!(
            load_prices_str(&neg).unwrap_err().message(),
            "bad gp3_storage_per_gb_month"
        );
        let gb = builtin_text().replace("1073741824", "1000000000");
        assert_eq!(
            load_prices_str(&gb).unwrap_err().message(),
            "gb_bytes must be 1073741824"
        );
        assert_eq!(
            load_prices_str("[]").unwrap_err().message(),
            "expected an object"
        );
        assert_eq!(load_prices_str("{").unwrap_err().message(), "not JSON");
    }

    #[test]
    fn duplicate_key_and_secret_are_not_echoed() {
        let text = "{\"currency\":\"USD\",\"currency\":\"SECRETTOKEN\"}";
        let err = load_prices_str(text).unwrap_err();
        assert_eq!(err.message(), "duplicate key");
        assert!(!err.message().contains("SECRETTOKEN"));
    }

    #[test]
    fn apply_flag_cannot_turn_unused_rates_on() {
        let applied = builtin_text().replace(
            "\"apply_not_applied\": false",
            "\"apply_not_applied\": true",
        );
        assert_eq!(
            load_prices_str(&applied).unwrap_err().message(),
            "apply_not_applied cannot be true"
        );
        let bad = builtin_text().replace(
            "\"apply_not_applied\": false",
            "\"apply_not_applied\": \"SECRET\"",
        );
        let err = load_prices_str(&bad).unwrap_err();
        assert_eq!(err.message(), "bad apply_not_applied");
        assert!(!err.message().contains("SECRET"));
    }

    #[test]
    fn bom_bad_date_and_missing_not_applied() {
        let text = format!("\u{feff}{}", builtin_text());
        assert_eq!(load_prices_str(&text).unwrap().date, "2026-11-09");
        let date = builtin_text().replace("\"2026-11-09\"", "\"2025-02-29\"");
        assert_eq!(load_prices_str(&date).unwrap_err().message(), "bad date");
        let missing = builtin_text().replace("\"not_applied\":", "\"unused\":");
        assert_eq!(
            load_prices_str(&missing).unwrap_err().message(),
            "missing not_applied"
        );
        let currency = builtin_text().replace("\"USD\"", "\"usd\"");
        assert_eq!(
            load_prices_str(&currency).unwrap_err().message(),
            "bad currency"
        );
    }

    #[test]
    fn bad_not_applied_bad_source_and_missing_key() {
        let not_applied =
            builtin_text().replace("\"not_applied\": {", "\"not_applied\": [], \"x\": {");
        assert_eq!(
            load_prices_str(&not_applied).unwrap_err().message(),
            "bad not_applied"
        );
        let empty = builtin_text().replace(
            "\"source\": \"https://aws.amazon.com/ebs/pricing/\"",
            "\"source\": \"\"",
        );
        assert_eq!(load_prices_str(&empty).unwrap_err().message(), "bad source");
        let control = builtin_text().replace(
            "\"source\": \"https://aws.amazon.com/ebs/pricing/\"",
            "\"source\": \"https://a\\u0007b\"",
        );
        assert_eq!(
            load_prices_str(&control).unwrap_err().message(),
            "bad source"
        );
        let missing = builtin_text().replace("\"gp2_storage_per_gb_month\":", "\"unused\":");
        assert_eq!(
            load_prices_str(&missing).unwrap_err().message(),
            "missing gp2_storage_per_gb_month"
        );
    }

    #[test]
    fn remaining_bad_and_missing_texts_are_reported() {
        let cases: [(&str, &str, &str); 12] = [
            (
                "\"meter_published\": \"2026-11-07\"",
                "\"meter_published\": \"2026-13-07\"",
                "bad meter_published",
            ),
            (
                "\"region\": \"us-east-1\"",
                "\"region\": \"\"",
                "bad region",
            ),
            ("\"region\": \"us-east-1\"", "\"region\": 5", "bad region"),
            (
                "\"region\": \"us-east-1\"",
                "\"unused\": 5",
                "missing region",
            ),
            ("\"currency\": \"USD\"", "\"currency\": 5", "bad currency"),
            ("\"currency\": \"USD\"", "\"unused\": 5", "missing currency"),
            (
                "\"gp3_storage_per_gb_month\": \"0.08\"",
                "\"gp3_storage_per_gb_month\": true",
                "bad gp3_storage_per_gb_month",
            ),
            (
                "\"gb_bytes\": 1073741824",
                "\"unused\": 1",
                "missing gb_bytes",
            ),
            (
                "\"gb_bytes\": 1073741824",
                "\"gb_bytes\": 1.5",
                "bad gb_bytes",
            ),
            (
                "\"gp3_iops_baseline\": 3000",
                "\"gp3_iops_baseline\": null",
                "bad gp3_iops_baseline",
            ),
            (
                "\"free_tier_io_count\": 2000000",
                "\"unused\": 2000000",
                "missing free_tier_io_count",
            ),
            (
                "\"io2_iops_tier1_cap\": 32000",
                "\"io2_iops_tier1_cap\": \"\"",
                "bad io2_iops_tier1_cap",
            ),
        ];
        for (from, to, want) in cases {
            assert!(builtin_text().contains(from), "{from}");
            let text = builtin_text().replace(from, to);
            assert_eq!(load_prices_str(&text).unwrap_err().message(), want);
        }
        let long = format!("\"source\": \"{}\"", "a".repeat(501));
        let text =
            builtin_text().replace("\"source\": \"https://aws.amazon.com/ebs/pricing/\"", &long);
        assert_eq!(load_prices_str(&text).unwrap_err().message(), "bad source");
    }

    #[test]
    fn rate_with_29_decimals_does_not_fit() {
        let long = builtin_text().replace("\"0.08\"", "\"0.00000000000000000000000000001\"");
        let err = load_prices_str(&long).unwrap_err();
        assert!(err.message().contains("does not fit"), "{}", err.message());
    }

    #[test]
    fn tier_caps_must_increase() {
        let text = builtin_text().replace(
            "\"io2_iops_tier2_cap\": 64000",
            "\"io2_iops_tier2_cap\": 32000",
        );
        assert_eq!(
            load_prices_str(&text).unwrap_err().message(),
            "io2 tier caps must increase"
        );
    }

    #[test]
    fn u64_string_trims_only_ascii_whitespace() {
        let ok = builtin_text().replace(
            "\"gp3_iops_baseline\": 3000",
            "\"gp3_iops_baseline\": \" \\t3000\\r\\n\"",
        );
        assert_eq!(load_prices_str(&ok).unwrap().gp3_iops_baseline, 3000);
        for ws in ["\\u00a0", "\\u2028", "\\u3000"] {
            for text in [format!("{ws}3000"), format!("3000{ws}")] {
                let bad = builtin_text().replace(
                    "\"gp3_iops_baseline\": 3000",
                    &format!("\"gp3_iops_baseline\": \"{text}\""),
                );
                assert_eq!(
                    load_prices_str(&bad).unwrap_err().message(),
                    "bad gp3_iops_baseline"
                );
            }
        }
    }
}
