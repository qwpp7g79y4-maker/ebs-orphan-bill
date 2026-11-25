use ebs_wees_bill::{
    format_usd, free_tier_line, io2_iops_usd, io2_line, kind_of, not_applied_line, quote_snapshot,
    quote_volume, throughput_line, VolumeKind,
};
use rust_decimal::Decimal;

fn readme() -> &'static str {
    include_str!("../README.md")
}

fn price_file() -> &'static str {
    include_str!("../prices/ebs.json")
}

fn dec(text: &str) -> Decimal {
    text.parse().unwrap()
}

#[test]
fn readme_sections_are_in_order() {
    let text = readme();
    let mut last = 0;
    for heading in [
        "## Why",
        "## Installation",
        "## Quick start",
        "## Aan de slag in 5 minuten",
        "## Options",
        "## Example",
        "## Exit codes",
        "## How it counts",
        "## Limitations",
        "## Support",
        "## License",
        "## Contact",
    ] {
        let at = text
            .find(heading)
            .unwrap_or_else(|| panic!("{heading} missing"));
        assert!(at >= last, "{heading} is out of order");
        last = at;
    }
    let contact = &text[text.find("## Contact").expect("Contact")..];
    for needle in [
        "pep_ai@icloud.com",
        "https://www.pepai.nl",
        "https://x.com/pepai_nl",
        "https://ko-fi.com/pepai",
    ] {
        assert!(contact.contains(needle), "{needle} missing from Contact");
    }
}

#[test]
fn readme_and_price_file_cite_the_page() {
    let text = readme();
    let prices = price_file();
    for needle in [
        "https://aws.amazon.com/ebs/pricing/",
        "2026-11-09",
        "2026-11-07",
        "us-east-1",
        "0.08",
        "0.10",
        "0.125",
        "0.065",
        "0.0455",
        "0.03185",
        "0.045",
        "0.015",
        "0.05",
        "0.04",
        "40.96",
        "0.0125",
        "1073741824",
    ] {
        assert!(text.contains(needle), "{needle} missing from README");
        assert!(prices.contains(needle), "{needle} missing from price file");
    }
    assert!(prices.contains("\"apply_not_applied\": false"));
    assert!(!prices.contains("\"0.06\""));
    assert!(!prices.contains("0.046"));
    assert!(text.contains("0.06"));
    assert!(text.contains("0.046"));
}

#[test]
fn throughput_identity_and_the_example_rate_are_separated() {
    let per_mib = dec("40.96") / Decimal::from(1024);
    assert_eq!(per_mib, dec("0.04"));
    let prices = ebs_wees_bill::prices_builtin();
    assert_eq!(prices.gp3_throughput_per_mib_month, per_mib);
    let bill = include_str!("../src/bill.rs");
    assert!(!bill.contains("0.06"));
    assert!(!bill.contains("0.046"));
    assert!(!bill.contains("3.625"));
    assert!(readme().contains("210.00"));
    assert!(readme().contains("3.625"));
    assert!(readme().contains("22.50"));
}

#[test]
fn page_full_months_match_the_readme() {
    let prices = ebs_wees_bill::prices_builtin();
    let gp3 = quote_volume(VolumeKind::Gp3, 2000, Some(10_000), Some(500), &prices).unwrap();
    assert_eq!(format_usd(gp3.usd_month.unwrap()), "210.00");
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
    assert_eq!(
        format_usd(io2_iops_usd(60_000, &prices).unwrap()),
        "3354.00"
    );
    assert_ne!(
        format_usd(io2_iops_usd(60_000, &prices).unwrap()),
        format_usd(dec("32000") * dec("0.065") + dec("28000") * dec("0.046"))
    );
    assert_eq!(
        format_usd(
            quote_volume(VolumeKind::Io1, 2000, Some(1000), None, &prices)
                .unwrap()
                .usd_month
                .unwrap()
        ),
        "315.00"
    );
    assert_eq!(
        format_usd(io2_iops_usd(32_001, &prices).unwrap()),
        "2080.05"
    );
    assert_eq!(
        format_usd(io2_iops_usd(64_001, &prices).unwrap()),
        "3536.03"
    );
    let text = readme();
    for needle in [
        "210.00", "200.00", "90.00", "30.00", "3354.00", "315.00", "2080.05", "3536.03", "3368.00",
    ] {
        assert!(text.contains(needle), "{needle}");
    }
    assert_eq!(kind_of("standard"), VolumeKind::Unpriced);
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
        format_usd(quote_snapshot(Decimal::ONE, &prices).unwrap()),
        "0.05"
    );
    assert!(text.contains("3.00"));
    assert!(text.contains("not free"));
    assert_eq!(format_usd(dec("0.005")), "0.01");
    assert_eq!(format_usd(dec("0.004")), "0.00");
    assert!(text.contains("0.005 is shown as 0.01"));
    assert!(text.contains("0.004 is shown as 0.00"));
}

#[test]
fn readme_prints_the_builtin_note_lines() {
    let prices = ebs_wees_bill::prices_builtin();
    let text = readme();
    assert!(text.contains(&free_tier_line(&prices)));
    assert!(text.contains(&not_applied_line(&prices)));
    assert!(text.contains(&throughput_line(&prices)));
    assert!(text.contains(&io2_line(&prices)));
    assert!(text.contains("full-snapshot-bytes"));
    assert!(text.contains("volume-size"));
    assert!(text.contains("does not delete volumes or snapshots"));
    assert!(!text.contains("delete-volume"));
    assert!(!text.contains("delete-snapshot"));
    assert!(text.contains("Storage General Purpose gp3 GB Mo"));
    assert!(text.contains("Provisioned Throughput gp3 per GiBps mo"));
    assert!(text.contains("16.00"));
    assert!(text.contains("21.50"));
    assert!(text.contains("--fail-usd 16"));
    assert!(text.contains("--fail-usd 15.99"));
    assert!(text.contains("exits 0"));
    assert!(text.contains("exits 1"));
    assert!(text.contains("Exit 0"));
    assert!(text.contains("Exit 2"));
    assert!(text.contains("--json"));
}

#[test]
fn changelog_licenses_and_manifest() {
    let changelog = include_str!("../CHANGELOG.md");
    assert!(changelog.contains("## [1.0.0] - 2026-11-19"));
    assert!(changelog.contains("## [0.1.0] - 2026-11-09"));
    assert!(changelog.contains("https://aws.amazon.com/ebs/pricing/"));
    assert!(changelog.contains("0.08"));
    assert!(changelog.contains("0.0455"));
    assert!(changelog.contains("not applied"));
    assert!(changelog.contains("does not delete"));
    assert!(changelog.contains("This version is not published and has no git tag."));
    assert!(!changelog.contains("delete-volume"));
    let mit = include_str!("../LICENSE-MIT");
    let apache = include_str!("../LICENSE-APACHE");
    assert!(mit.contains("Copyright (c) 2026 PEPAI"));
    assert!(mit.contains("MIT License"));
    assert!(apache.contains("Apache License"));
    assert!(apache.contains("Version 2.0"));
    let manifest = include_str!("../Cargo.toml");
    assert!(manifest.contains("license = \"MIT OR Apache-2.0\""));
    assert!(manifest.contains("clap"));
    assert!(manifest.contains("rust-version = \"1.85\""));
    assert!(manifest.contains("name = \"ebs-wees-bill\""));
    assert!(manifest.contains("version = \"1.0.0\""));
    assert!(!manifest.contains("aws-sdk"));
    assert!(!manifest.contains("reqwest"));
    assert!(readme().contains("MIT OR Apache-2.0"));
    assert!(readme().contains("PEPAI"));
    assert!(readme().contains("clap"));
}

#[test]
fn source_does_not_spawn_aws_or_link_an_sdk() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = 0;
    for entry in std::fs::read_dir(&root).expect("src") {
        let entry = entry.expect("entry");
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.ends_with(".rs") {
            continue;
        }
        files += 1;
        let text = std::fs::read_to_string(entry.path()).expect("read");
        assert!(!text.contains("Command::new"), "{name}");
        assert!(!text.contains("aws-sdk"), "{name}");
        assert!(!text.contains("reqwest"), "{name}");
        assert!(!text.contains("hyper::"), "{name}");
        assert!(!text.contains("std::net"), "{name}");
        assert!(!text.contains("delete-volume"), "{name}");
        assert!(!text.contains("delete-snapshot"), "{name}");
    }
    assert!(files >= 8);
    let bill = include_str!("../src/bill.rs");
    assert!(!bill.contains("not_applied"));
    assert!(!bill.contains("0.0125"));
}

fn headings(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| line.starts_with("## "))
        .collect()
}

fn quick_start_block(text: &str) -> &str {
    let start = text
        .find("<!-- quick-start:start -->")
        .expect("start marker");
    let end = text.find("<!-- quick-start:end -->").expect("end marker");
    &text[start..end]
}

#[test]
fn the_dutch_readme_follows_the_english_one() {
    let nl = include_str!("../README.nl.md");
    let en = readme();
    // Same number of sections, in the same order, with the shared heading and the real output.
    assert_eq!(headings(nl).len(), headings(en).len(), "{:?}", headings(nl));
    assert!(nl.contains("## Aan de slag in 5 minuten"));
    assert_eq!(quick_start_block(nl), quick_start_block(en));
    for needle in [
        "estimated_usd_month=21.50",
        "Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)",
        "\"schema_version\": 1",
        "180 dagen",
        "--output",
        "--quiet",
        "--verbose",
        "--md",
        "130",
        "pep_ai@icloud.com",
        "https://www.pepai.nl",
        "https://x.com/pepai_nl",
        "https://ko-fi.com/pepai",
    ] {
        assert!(nl.contains(needle), "{needle} missing from README.nl.md");
    }
    let contact = &nl[nl.find("## Contact").expect("Contact")..];
    assert!(contact.contains("https://ko-fi.com/pepai"));
}

#[test]
fn changelog_names_the_gb_rounding_change() {
    let changelog = include_str!("../CHANGELOG.md");
    assert!(changelog.contains("This changes the output for a size in bytes"));
    assert!(changelog
        .contains("0.0000000018626451492309570312 and now shows 0.0000000018626451492309570313"));
}
