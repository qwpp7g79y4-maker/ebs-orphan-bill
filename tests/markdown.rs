//! `--md`: a Markdown report with the same amounts as the text and JSON output.

use std::process::Command;

const SAMPLES: [&str; 3] = [
    "examples/volumes.json",
    "examples/snapshots.json",
    "examples/images.json",
];
const ESTIMATE: &str =
    "Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)";

fn run(extra: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ebs-orphan-bill"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["--as-of", "2026-11-09"])
        .args(extra)
        .args(SAMPLES)
        .output()
        .expect("run binary");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The cells of the table rows that follow `heading`, without the two header rows.
fn table_after<'a>(md: &'a str, heading: &str) -> Vec<Vec<&'a str>> {
    let start = md.find(heading).expect("heading") + heading.len();
    md[start..]
        .lines()
        .skip_while(|line| !line.starts_with('|'))
        .take_while(|line| line.starts_with('|'))
        .skip(2)
        .map(|line| line.trim_matches('|').split(" | ").map(str::trim).collect())
        .collect()
}

#[test]
fn markdown_has_the_same_amounts_as_text_and_json() {
    let (code, md, err) = run(&["--md"]);
    assert_eq!(code, 0, "{err}");
    let (_, text, _) = run(&[]);
    let (_, json, _) = run(&["--json"]);
    let json: serde_json::Value = serde_json::from_str(&json).expect("json");
    assert_eq!(md.matches(ESTIMATE).count(), 1, "{md}");
    for line in [
        "estimated_usd_month=21.50",
        "volume estimated_usd_month=21.00",
        "snapshot estimated_usd_month=0.50",
    ] {
        assert!(md.contains(line), "{md}");
        assert!(text.contains(line), "{text}");
    }
    assert_eq!(json["estimated_usd_month"], "21.50");
    let volumes = table_after(&md, "## Volumes");
    let json_volumes = json["volumes"].as_array().expect("volumes");
    assert_eq!(volumes.len(), json_volumes.len());
    for (cells, row) in volumes.iter().zip(json_volumes) {
        assert_eq!(cells[0], row["id"].as_str().expect("id"));
        let amount = row["usd_month"].as_str().unwrap_or("unpriced");
        assert_eq!(cells[8], amount, "{cells:?}");
        assert!(text.contains(amount), "{amount}");
    }
    let snapshots = table_after(&md, "## Snapshots");
    let json_snapshots = json["snapshots"].as_array().expect("snapshots");
    assert_eq!(snapshots.len(), json_snapshots.len());
    for (cells, row) in snapshots.iter().zip(json_snapshots) {
        assert_eq!(cells[0], row["id"].as_str().expect("id"));
        assert_eq!(cells[3], row["usd_month"].as_str().unwrap_or("unpriced"));
    }
}

#[test]
fn markdown_escapes_pipes_and_line_breaks_in_cells_and_lines() {
    let result = run(&["--md", "--prices", "tests/fixtures/prices-with-pipe.json"]);
    let (code, md, err) = result;
    assert_eq!(code, 0, "{err}");
    assert!(md.contains("a\\|b"), "{md}");
    assert!(!md.contains("a|b"), "{md}");
}

#[test]
fn markdown_and_json_together_are_a_usage_error() {
    let (code, stdout, stderr) = run(&["--md", "--json"]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty());
}

#[test]
fn markdown_keeps_the_threshold_exit_and_quiet_drops_warnings() {
    let (code, md, _) = run(&["--md", "--fail-usd", "15.99"]);
    assert_eq!(code, 1);
    assert!(md.contains("## Warnings"), "{md}");
    let (qcode, quiet, _) = run(&["--md", "--quiet"]);
    assert_eq!(qcode, 0);
    assert!(!quiet.contains("## Warnings"), "{quiet}");
}
