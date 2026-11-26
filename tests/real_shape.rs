//! The input shapes of `aws ec2 describe-volumes`, `describe-snapshots` and
//! `describe-images` as the AWS CLI reference documents them:
//! <https://docs.aws.amazon.com/cli/latest/reference/ec2/describe-volumes.html>,
//! <https://docs.aws.amazon.com/cli/latest/reference/ec2/describe-snapshots.html>,
//! <https://docs.aws.amazon.com/cli/latest/reference/ec2/describe-images.html>.
//! The fixtures reproduce that shape from memory, with the variants that occur
//! in practice: optional fields left out, empty lists, several values, and names
//! with special characters. No network is used.

use std::process::{Command, Output};

const VOLUMES: &str = "tests/fixtures/describe-volumes.json";
const SNAPSHOTS: &str = "tests/fixtures/describe-snapshots.json";
const IMAGES: &str = "tests/fixtures/describe-images.json";

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ebs-orphan-bill"))
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("run binary")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("utf-8")
}

#[test]
fn the_documented_shape_prices_to_the_exact_amount() {
    let out = run(&[
        "--as-of",
        "2026-11-09",
        "--json",
        VOLUMES,
        SNAPSHOTS,
        IMAGES,
    ]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let value: serde_json::Value = serde_json::from_str(&text(&out.stdout)).expect("json");
    // gp3 125 GB 10.00, gp2 8 GB 0.80, io1 100 GB + 1000 IOPS 12.50 + 65.00,
    // io2 10 GB + 100 IOPS 1.25 + 6.50, st1 500 GB 22.50, sc1 500 GB 7.50,
    // gp3 1 GB 0.08, gp2 1 GB 0.10; the attached io2 volume is not an orphan.
    assert_eq!(value["volume_estimated_usd_month"], "126.23");
    // 8 GB 0.40, 4 GB from FullSnapshotSizeInBytes 0.20, 2 GB 0.10. The AMI
    // snapshot, the young one, the pending one, and the archive one add nothing.
    assert_eq!(value["snapshot_estimated_usd_month"], "0.70");
    assert_eq!(value["estimated_usd_month"], "126.93");
    let volumes = value["volumes"].as_array().expect("volumes");
    assert_eq!(volumes.len(), 8);
    let snapshots = value["snapshots"].as_array().expect("snapshots");
    let ids: Vec<&str> = snapshots
        .iter()
        .map(|row| row["id"].as_str().expect("id"))
        .collect();
    assert_eq!(
        ids,
        [
            "snap-0a1b2c3d4e5f60718",
            "snap-0b1b2c3d4e5f60718",
            "snap-0c1b2c3d4e5f60718",
            "snap-0e1b2c3d4e5f60718"
        ]
    );
    let stdout = text(&out.stdout);
    for secret in [
        "123456789012",
        "i-0123456789abcdef0",
        "arn:aws:kms",
        "données",
    ] {
        assert!(!stdout.contains(secret), "{secret}");
    }
}

#[test]
fn the_same_shape_in_text_mode_shows_the_same_totals() {
    let out = run(&["--as-of", "2026-11-09", VOLUMES, SNAPSHOTS, IMAGES]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let stdout = text(&out.stdout);
    assert!(
        stdout.contains("\nvolume estimated_usd_month=126.23\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains("\nsnapshot estimated_usd_month=0.70\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains("\nestimated_usd_month=126.93\n"),
        "{stdout}"
    );
}

#[test]
fn without_describe_images_snapshots_are_not_orphans() {
    let out = run(&["--as-of", "2026-11-09", "--json", VOLUMES, SNAPSHOTS]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let value: serde_json::Value = serde_json::from_str(&text(&out.stdout)).expect("json");
    assert_eq!(value["estimated_usd_month"], "126.23");
    assert_eq!(value["snapshots"].as_array().expect("snapshots").len(), 0);
}

#[test]
fn empty_lists_are_a_zero_report() {
    let out = run(&["--as-of", "2026-11-09", "--json", "-"]);
    // Standard input is empty here: that is not JSON, so it is exit 2.
    assert_eq!(out.status.code(), Some(2));
    for (name, body) in [
        ("volumes", r#"{"Volumes": []}"#),
        ("snapshots", r#"{"Snapshots": []}"#),
        ("images", r#"{"Images": []}"#),
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_ebs-orphan-bill"))
            .args(["--json", "-"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("spawn");
        std::io::Write::write_all(&mut child.stdin.take().expect("stdin"), body.as_bytes())
            .expect("write");
        let out = child.wait_with_output().expect("wait");
        assert_eq!(out.status.code(), Some(0), "{name}: {}", text(&out.stderr));
        let value: serde_json::Value = serde_json::from_str(&text(&out.stdout)).expect("json");
        assert_eq!(value["estimated_usd_month"], "0.00", "{name}");
    }
}
