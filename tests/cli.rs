use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use flate2::write::GzEncoder;
use flate2::Compression;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_ebs-orphan-bill")
}

fn manifest() -> &'static str {
    env!("CARGO_MANIFEST_DIR")
}

fn run(args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .current_dir(manifest())
        .output()
        .expect("run binary")
}

fn run_stdin(args: &[&str], input: &[u8]) -> Output {
    let mut child = Command::new(bin())
        .args(args)
        .current_dir(manifest())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input)
        .expect("write stdin");
    child.wait_with_output().expect("wait")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("utf-8")
}

fn temp_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "ebs-orphan-bill-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir(&path).expect("temp dir");
    path
}

struct Cleanup(PathBuf);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn samples() -> [&'static str; 3] {
    [
        "examples/volumes.json",
        "examples/snapshots.json",
        "examples/images.json",
    ]
}

#[test]
fn help_and_version_exit_0() {
    let help = run(&["--help"]);
    assert_eq!(help.status.code(), Some(0));
    let stdout = text(&help.stdout);
    assert!(stdout.contains("--json"));
    assert!(stdout.contains("--fail-usd"));
    assert!(stdout.contains("--as-of"));
    assert!(stdout.contains("--older-than-days"));
    assert!(stdout.contains("--prices"));
    assert!(stdout.contains("no network"));
    assert!(stdout.contains("Exit codes"));
    assert!(help.stderr.is_empty());
    let version = run(&["--version"]);
    assert_eq!(version.status.code(), Some(0));
    assert!(text(&version.stdout).contains("1.0.2"));
}

#[test]
fn unknown_flag_exits_2() {
    let out = run(&["--no-such-flag"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert!(text(&out.stderr).contains("ebs-orphan-bill"));
}

#[test]
fn missing_file_exits_2() {
    let out = run(&["examples/does-not-exist.json"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    let err = text(&out.stderr);
    assert!(err.contains("ebs-orphan-bill:"));
    assert!(err.contains("examples/does-not-exist.json"));
}

#[test]
fn bad_json_exits_2_without_the_body() {
    let dir = Cleanup(temp_dir("bad-json"));
    let path = dir.0.join("in.json");
    fs::write(&path, b"{\"note\":\"SECRETJSON\" ").expect("write");
    let out = run(&[path.to_str().expect("utf8 path")]);
    assert_eq!(out.status.code(), Some(2));
    let err = text(&out.stderr);
    assert!(err.contains("not JSON"));
    assert!(!err.contains("SECRETJSON"));
    assert!(!err.contains("delete-volume"));
}

#[test]
fn fail_usd_with_29_decimals_exits_2_and_28_decimals_works() {
    let mut args = vec!["--fail-usd", "0.00000000000000000000000000001"];
    args.extend(samples());
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out.stderr).contains("does not fit"));
    let mut ok = vec!["--fail-usd", "0.0000000000000000000000000001"];
    ok.extend(samples());
    assert_ne!(run(&ok).status.code(), Some(2));
    let mut max = vec!["--fail-usd", "79228162514264337593543950335"];
    max.extend(samples());
    assert_eq!(run(&max).status.code(), Some(0));
}

#[test]
fn invalid_as_of_and_fail_usd_exit_2() {
    let mut args = vec!["--as-of", "2025-02-29"];
    args.extend(samples());
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2));
    let err = text(&out.stderr);
    assert!(err.contains("invalid --as-of"));
    assert!(!err.contains("SECRETKEY"));
    let mut bad = vec!["--fail-usd", "1e-2"];
    bad.extend(samples());
    let out = run(&bad);
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out.stderr).contains("invalid --fail-usd"));
    assert!(!text(&out.stderr).contains("1e-2"));
}

#[test]
fn duplicate_volume_id_across_inputs_exits_2() {
    let input = br#"{"VolumeId":"vol-01234567","Size":10,"VolumeType":"gp2","Attachments":[]}"#;
    let dir = Cleanup(temp_dir("dupvol"));
    let first = dir.0.join("a.json");
    let second = dir.0.join("b.json");
    fs::write(&first, input).expect("write");
    fs::write(&second, input).expect("write");
    let out = run(&[
        first.to_str().expect("utf8"),
        second.to_str().expect("utf8"),
    ]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    assert!(text(&out.stderr).contains("duplicate volume vol-01234567"));
    assert!(text(&out.stdout).is_empty());
}

#[test]
fn quick_start_hides_secrets_and_prices_the_sample() {
    let mut args = vec!["--as-of", "2026-11-09", "--older-than-days", "30"];
    args.extend(samples());
    let out = run(&args);
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stderr.is_empty());
    let stdout = text(&out.stdout);
    assert!(stdout.contains("21.50"));
    assert!(stdout.contains("21.00"));
    assert!(stdout.contains("16.00"));
    assert!(stdout.contains("5.00"));
    assert!(stdout.contains("0.50"));
    assert!(stdout.contains("vol-11111111111111111"));
    assert!(stdout.contains("vol-44444444444444444"));
    assert!(stdout.contains("snap-11111111111111111"));
    assert!(stdout.contains("snap-44444444444444444"));
    assert!(!stdout.contains("vol-33333333333333333"));
    assert!(!stdout.contains("snap-22222222222222222"));
    assert!(!stdout.contains("snap-33333333333333333"));
    assert!(!stdout.contains("123456789012"));
    assert!(!stdout.contains("SECRETKEY"));
    assert!(!stdout.contains("SECRETTOKEN"));
    assert!(!stdout.contains("CreateImage"));
    assert!(!stdout.contains("i-11111111111111111"));
    assert!(!stdout.contains("ami-11111111111111111"));
    assert!(!stdout.contains("delete-volume"));
    assert!(!stdout.contains("delete-snapshot"));
    assert!(stdout.contains("https://aws.amazon.com/ebs/pricing/"));
    assert!(stdout.contains("2026-11-09"));
    assert!(stdout.contains("2026-11-07"));
}

const ESTIMATE_LINE: &str =
    "Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)";

#[test]
fn the_monthly_cost_is_labelled_as_an_estimate_in_text_and_json() {
    let mut args = vec!["--as-of", "2026-11-09"];
    args.extend(samples());
    let out = run(&args);
    let stdout = text(&out.stdout);
    assert_eq!(stdout.matches(ESTIMATE_LINE).count(), 1, "{stdout}");
    assert!(stdout.contains("\nestimated_usd_month=21.50\n"), "{stdout}");
    assert!(
        stdout.contains("volume estimated_usd_month=21.00\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains("snapshot estimated_usd_month=0.50\n"),
        "{stdout}"
    );
    assert!(!stdout.contains("\nusd_month "), "{stdout}");
    args.push("--json");
    let out = run(&args);
    let value: serde_json::Value = serde_json::from_str(&text(&out.stdout)).expect("json");
    assert_eq!(value["estimate"], true);
    assert_eq!(value["estimate_note"], ESTIMATE_LINE);
    assert_eq!(value["estimated_usd_month"], "21.50");
    assert!(value.get("usd_month").is_none());
}

#[test]
fn price_data_older_than_180_days_warns_and_still_exits_0() {
    // The built-in prices are dated 2026-11-09; 180 days later is 2027-05-08.
    let mut base = vec!["--as-of", "2027-05-08"];
    base.extend(samples());
    let out = run(&base);
    assert_eq!(out.status.code(), Some(0));
    assert!(
        !text(&out.stdout).contains("price data is"),
        "180 days is not older"
    );
    let mut old = vec!["--as-of", "2027-05-09", "--json"];
    old.extend(samples());
    let out = run(&old);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let stderr = text(&out.stderr);
    assert!(
        stderr.contains("warning: price data is 181 days old (dated 2026-11-09"),
        "{stderr}"
    );
    let value: serde_json::Value = serde_json::from_str(&text(&out.stdout)).expect("json");
    assert!(value["warnings"]
        .as_array()
        .expect("warnings")
        .iter()
        .any(|w| w.as_str().is_some_and(|w| w.contains("181 days old"))));
    old.retain(|arg| *arg != "--json");
    let out = run(&old);
    assert_eq!(out.status.code(), Some(0));
    assert!(text(&out.stdout).contains("warning: price data is 181 days old"));
}

#[test]
fn a_price_file_is_checked_for_age_with_its_own_date() {
    let dir = Cleanup(temp_dir("old-prices"));
    let prices = include_str!("../prices/ebs.json")
        .replace("\"date\": \"2026-11-09\"", "\"date\": \"2025-01-01\"");
    assert!(prices.contains("2025-01-01"));
    let path = dir.0.join("prices.json");
    fs::write(&path, prices).expect("write");
    let prices_arg = path.to_str().expect("utf8");
    let mut args = vec!["--prices", prices_arg, "--as-of", "2026-11-09"];
    args.extend(samples());
    let out = run(&args);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(
        text(&out.stdout).contains("price data is 677 days old (dated 2025-01-01"),
        "{}",
        text(&out.stdout)
    );
}

#[test]
fn fail_usd_equal_exits_0_and_just_under_exits_1() {
    let mut equal = vec!["--as-of", "2026-11-09", "--fail-usd", "16"];
    equal.extend(samples());
    let out = run(&equal);
    assert_eq!(out.status.code(), Some(0));
    assert!(text(&out.stdout).contains("threshold exceeded: no"));
    let mut under = vec!["--as-of", "2026-11-09", "--fail-usd", "15.99", "--json"];
    under.extend(samples());
    let out = run(&under);
    assert_eq!(out.status.code(), Some(1));
    let stdout = text(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("json");
    assert_eq!(parsed["threshold_exceeded"], true);
    assert_eq!(parsed["fail_usd"], "15.99");
    assert_eq!(parsed["estimated_usd_month"], "21.50");
    assert_eq!(parsed["volume_estimated_usd_month"], "21.00");
    assert_eq!(parsed["snapshot_estimated_usd_month"], "0.50");
    assert_eq!(parsed["free_tier_applied"], false);
    assert_eq!(parsed["volumes"][2]["usd_month"], serde_json::Value::Null);
    assert_eq!(parsed["snapshots"][1]["priced"], false);
    assert_eq!(parsed["snapshots"][0]["size_basis"], "volume-size");
    assert!(stdout.contains("\"warnings\""));
    let stderr = text(&out.stderr);
    assert!(stderr.contains("warning:"));
    assert!(stderr.contains("nextToken was not followed"));
    assert!(!stderr.contains("SECRETTOKEN"));
    assert!(!stdout.contains("123456789012"));
    assert!(!stdout.contains("SECRETKEY"));
    assert!(!stdout.contains("delete-snapshot"));
}

#[test]
fn empty_volumes_exit_0_and_missing_images_skip_snapshots() {
    let dir = Cleanup(temp_dir("empty"));
    let path = dir.0.join("v.json");
    fs::write(&path, r#"{"Volumes":[]}"#).expect("write");
    let out = run(&[path.to_str().expect("utf8")]);
    assert_eq!(out.status.code(), Some(0));
    assert!(text(&out.stdout).contains("estimated_usd_month=0.00"));
    let out = run(&[
        "--as-of",
        "2026-11-09",
        "examples/volumes.json",
        "examples/snapshots.json",
    ]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = text(&out.stdout);
    assert!(stdout.contains("describe-images was not provided"));
    assert!(!stdout.contains("snap-11111111111111111"));
    assert!(stdout.contains("21.00"));
    assert!(!stdout.contains("21.50"));
}

#[test]
fn stdin_and_gzip() {
    let input = br#"{"VolumeId":"vol-01234567","Size":10,"VolumeType":"gp2","Attachments":[]}"#;
    let out = run_stdin(&["--as-of", "2026-11-09", "-"], input);
    assert_eq!(out.status.code(), Some(0));
    assert!(text(&out.stdout).contains("1.00"));
    assert!(text(&out.stderr).is_empty());
    let dir = Cleanup(temp_dir("gzip"));
    let path = dir.0.join("v.json.gz");
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(input).expect("gzip");
    fs::write(&path, encoder.finish().expect("finish")).expect("write");
    let out = run(&["--as-of", "2026-11-09", path.to_str().expect("utf8")]);
    assert_eq!(out.status.code(), Some(0));
    assert!(text(&out.stdout).contains("vol-01234567"));
    assert!(text(&out.stdout).contains("1.00"));
}

fn gzip_bytes(input: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(input).expect("gzip");
    encoder.finish().expect("finish")
}

#[test]
fn gzip_with_bytes_after_the_first_member_is_exit_2() {
    let a = gzip_bytes(br#"{"Volumes":[{"VolumeId":"vol-11111111","Size":1,"VolumeType":"gp2"}]}"#);
    let b =
        gzip_bytes(br#"{"Volumes":[{"VolumeId":"vol-22222222","Size":1000,"VolumeType":"gp2"}]}"#);
    let dir = Cleanup(temp_dir("gzmulti"));
    let mut two = a.clone();
    two.extend_from_slice(&b);
    let mut junk = a.clone();
    junk.extend_from_slice(b"junk");
    for (name, data) in [("ab.gz", two), ("trail.gz", junk)] {
        let path = dir.0.join(name);
        fs::write(&path, data).expect("write");
        let out = run(&[path.to_str().expect("utf8")]);
        assert_eq!(out.status.code(), Some(2), "{name}");
        assert!(
            text(&out.stderr).contains("after the first gzip member"),
            "{name}"
        );
        assert!(!text(&out.stdout).contains("0.10"), "{name}");
    }
}

#[test]
fn directory_skips_junk_and_bad_json_is_an_error() {
    let dir = Cleanup(temp_dir("dir"));
    fs::write(dir.0.join("empty.json"), b"").expect("empty");
    fs::write(dir.0.join("bad.bin"), [0xff, 0xfe, 0xfd]).expect("bin");
    fs::write(
        dir.0.join("v.json"),
        r#"{"Volumes":[{"VolumeId":"vol-01234567","Size":2,"VolumeType":"sc1"}]}"#,
    )
    .expect("json");
    let out = run(&[dir.0.to_str().expect("utf8")]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let stdout = text(&out.stdout);
    assert!(stdout.contains("skipped empty file"));
    assert!(stdout.contains("skipped non-utf8 file"));
    assert!(stdout.contains("vol-01234567"));
    assert!(stdout.contains("0.03"));
    fs::write(dir.0.join("nope.json"), b"{\"SECRETJSON\": ").expect("bad");
    let out = run(&[dir.0.to_str().expect("utf8")]);
    assert_eq!(out.status.code(), Some(2));
    let err = text(&out.stderr);
    assert!(err.contains("not JSON"));
    assert!(!err.contains("SECRETJSON"));
}

#[cfg(unix)]
#[test]
fn directory_skips_symlinks() {
    use std::os::unix::fs::symlink;
    let dir = Cleanup(temp_dir("dir-symlink"));
    fs::write(
        dir.0.join("v.json"),
        r#"{"Volumes":[{"VolumeId":"vol-01234567","Size":2,"VolumeType":"sc1"}]}"#,
    )
    .expect("json");
    symlink(dir.0.join("v.json"), dir.0.join("link.json")).expect("symlink");
    let out = run(&[dir.0.to_str().expect("utf8")]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(text(&out.stdout).contains("skipped symlink"));
}

#[test]
fn duplicate_keys_in_inner_objects_are_exit_2() {
    let cases = [
        (
            "volume",
            r#"{"Volumes":[{"VolumeId":"vol-01234567","Size":1,"Size":2,"VolumeType":"gp2"}]}"#,
        ),
        (
            "array",
            r#"[{"VolumeId":"vol-01234567","Size":1,"Size":2,"VolumeType":"gp2"}]"#,
        ),
        (
            "attachments",
            r#"{"Volumes":[{"VolumeId":"vol-01234567","Size":1,"VolumeType":"gp2","Attachments":[],"Attachments":[]}]}"#,
        ),
    ];
    for (label, json) in cases {
        let dir = Cleanup(temp_dir(label));
        let file = dir.0.join("d.json");
        fs::write(&file, json).expect("json");
        let out = run(&[file.to_str().expect("utf8")]);
        assert_eq!(out.status.code(), Some(2), "{label}");
        assert!(text(&out.stderr).contains("duplicate key"), "{label}");
    }
}

#[test]
fn quick_start_block_matches_the_binary() {
    let readme = include_str!("../README.md").replace("\r\n", "\n");
    let start = readme
        .find("<!-- quick-start:start -->")
        .expect("start marker");
    let end = readme.find("<!-- quick-start:end -->").expect("end marker");
    let block = &readme[start..end];
    let console = block
        .split("```console\n")
        .nth(1)
        .expect("console")
        .split("\n```")
        .next()
        .expect("console end");
    let mut lines = console.lines();
    let command = lines.next().expect("command");
    assert_eq!(
        command,
        "$ ebs-orphan-bill --as-of 2026-11-09 --older-than-days 30 examples/volumes.json examples/snapshots.json examples/images.json"
    );
    let mut expected = String::new();
    for line in lines {
        expected.push_str(line);
        expected.push('\n');
    }
    let out = run(&[
        "--as-of",
        "2026-11-09",
        "--older-than-days",
        "30",
        "examples/volumes.json",
        "examples/snapshots.json",
        "examples/images.json",
    ]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(text(&out.stdout), expected);
    assert!(expected.contains("21.50"));
    assert!(!expected.contains("123456789012"));
}

#[test]
fn describe_shapes_exit_0() {
    let dir = Cleanup(temp_dir("aws-shape"));
    let vol = dir.0.join("vol.json");
    let snap = dir.0.join("snap.json");
    fs::write(
        &vol,
        r#"{"Volumes":[{"VolumeId":"vol-0123456789abcdef0","Size":8,"VolumeType":"gp2","SnapshotId":"","Attachments":[]}]}"#,
    )
    .expect("vol");
    fs::write(
        &snap,
        r#"{"Snapshots":[{"SnapshotId":"snap-0123456789abcdef0","StartTime":"2026-01-01T00:00:00+00:00","State":"completed","VolumeId":"vol-0123456789abcdef0","VolumeSize":8,"StorageTier":"standard"}]}"#,
    )
    .expect("snap");
    let out = run(&[
        "--as-of",
        "2026-11-09",
        vol.to_str().expect("utf8"),
        snap.to_str().expect("utf8"),
    ]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let stdout = text(&out.stdout);
    assert!(stdout.contains("vol-0123456789abcdef0"));
    assert!(stdout.contains("0.80"));
    assert!(!stdout.contains("mixed document"));
}

#[test]
fn surrogate_pair_in_a_string_exits_0() {
    let dir = Cleanup(temp_dir("pair"));
    let path = dir.0.join("v.json");
    fs::write(
        &path,
        r#"{"VolumeId":"vol-01234567","Size":1,"VolumeType":"gp2","Note":"\ud83d\ude00"}"#,
    )
    .expect("write");
    let out = run(&[path.to_str().expect("utf8")]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(!text(&out.stderr).contains("not JSON"));
    assert!(text(&out.stdout).contains("vol-01234567"));
}

#[cfg(target_os = "linux")]
#[test]
fn control_chars_in_a_short_file_name_stay_escaped() {
    let dir = Cleanup(temp_dir("name"));
    let path = dir.0.join("a\u{1b}\nwarning:z");
    fs::write(&path, b"").expect("write");
    let listed = run(&[dir.0.to_str().expect("utf8")]);
    assert_eq!(listed.status.code(), Some(0), "{}", text(&listed.stderr));
    assert_no_raw_controls(&listed);
    let stdout = text(&listed.stdout);
    assert!(stdout.contains("skipped empty file"));
    assert!(stdout.contains("\\u{1b}\\nwarning:z"), "{stdout}");
    let direct = run(&[path.to_str().expect("utf8")]);
    assert_eq!(direct.status.code(), Some(2), "{}", text(&direct.stderr));
    assert_no_raw_controls(&direct);
    let err = text(&direct.stderr);
    assert!(err.contains("not JSON"));
    assert!(err.contains("\\u{1b}\\nwarning:z"), "{err}");
    assert_eq!(err.lines().count(), 1);
}

#[cfg(target_os = "linux")]
fn assert_no_raw_controls(out: &Output) {
    assert!(!out.stdout.contains(&0x1b));
    assert!(!out.stderr.contains(&0x1b));
    for bytes in [&out.stdout, &out.stderr] {
        for line in text(bytes).lines() {
            assert!(!line.starts_with("warning:z"), "{line:?}");
        }
    }
}

#[test]
fn deeply_nested_brackets_exit_2_quickly() {
    let dir = Cleanup(temp_dir("deep"));
    let path = dir.0.join("deep.json");
    fs::write(&path, format!("{{\"k\":{}", "[".repeat(1_000_000))).expect("write");
    let started = std::time::Instant::now();
    let out = run(&[path.to_str().expect("utf8")]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    assert!(text(&out.stderr).contains("not JSON"));
    assert!(started.elapsed().as_secs() < 10);
}

#[test]
fn bidi_in_the_prices_source_is_escaped_in_text() {
    let dir = Cleanup(temp_dir("bidi-source"));
    let prices = include_str!("../prices/ebs.json").replace(
        "https://aws.amazon.com/ebs/pricing/",
        "https://x.example/\\u202ep",
    );
    assert!(prices.contains("\\u202e"));
    let path = dir.0.join("prices.json");
    fs::write(&path, prices).expect("write");
    let prices_arg = path.to_str().expect("utf8");
    let mut args = vec!["--prices", prices_arg, "examples/volumes.json"];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let stdout = text(&out.stdout);
    assert!(
        stdout.contains("source https://x.example/\\u{202e}p"),
        "{stdout}"
    );
    assert!(!stdout.contains('\u{202e}'));
    args.insert(0, "--json");
    let json = run(&args);
    assert_eq!(json.status.code(), Some(0), "{}", text(&json.stderr));
    assert!(!text(&json.stdout).contains('\u{202e}'));
}

#[test]
fn json_output_escapes_bidi_in_the_prices_source() {
    let dir = Cleanup(temp_dir("bidi-source-json"));
    let prices = include_str!("../prices/ebs.json").replace(
        "https://aws.amazon.com/ebs/pricing/",
        "https://x.example/\\u202ep",
    );
    let path = dir.0.join("prices.json");
    fs::write(&path, prices).expect("write");
    let out = run(&[
        "--json",
        "--prices",
        path.to_str().expect("utf8"),
        "examples/volumes.json",
    ]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let stdout = text(&out.stdout);
    assert!(!stdout.contains('\u{202e}'), "{stdout}");
    assert!(stdout.contains("x.example/\\\\u{202e}p"), "{stdout}");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("json");
    assert_eq!(value["source"], "https://x.example/\\u{202e}p");
}

#[test]
fn line_splitting_and_blank_chars_in_the_prices_source_are_escaped() {
    let dir = Cleanup(temp_dir("blank-source"));
    let volume = dir.0.join("volume.json");
    fs::write(
        &volume,
        r#"{"VolumeId":"vol-01234567","Size":10,"VolumeType":"gp2","Attachments":[]}"#,
    )
    .expect("write volume");
    let plain_prices = include_str!("../prices/ebs.json");
    let odd_prices = plain_prices.replace(
        "https://aws.amazon.com/ebs/pricing/",
        "https://x.example/\\u2028a\\u3164b",
    );
    assert!(odd_prices.contains("\\u2028a\\u3164b"));
    let plain_path = dir.0.join("plain.json");
    let odd_path = dir.0.join("odd.json");
    fs::write(&plain_path, plain_prices).expect("write");
    fs::write(&odd_path, odd_prices).expect("write");
    let volume_arg = volume.to_str().expect("utf8");
    let plain_arg = plain_path.to_str().expect("utf8");
    let odd_arg = odd_path.to_str().expect("utf8");

    let plain_text = run(&["--prices", plain_arg, volume_arg]);
    let odd_text = run(&["--prices", odd_arg, volume_arg]);
    assert_eq!(
        odd_text.status.code(),
        Some(0),
        "{}",
        text(&odd_text.stderr)
    );
    let odd_out = text(&odd_text.stdout);
    assert!(!odd_out.contains('\u{2028}') && !odd_out.contains('\u{3164}'));
    assert!(odd_out.contains("\\u{2028}") && odd_out.contains("\\u{3164}"));
    let total = |s: &str| {
        s.lines()
            .filter(|l| !l.starts_with("prices:"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(total(&odd_out), total(&text(&plain_text.stdout)));

    let plain_json = run(&["--json", "--prices", plain_arg, volume_arg]);
    let odd_json = run(&["--json", "--prices", odd_arg, volume_arg]);
    assert_eq!(
        odd_json.status.code(),
        Some(0),
        "{}",
        text(&odd_json.stderr)
    );
    let odd_raw = text(&odd_json.stdout);
    assert!(!odd_raw.contains('\u{2028}') && !odd_raw.contains('\u{3164}'));
    let mut a: serde_json::Value = serde_json::from_str(&odd_raw).expect("json");
    let mut b: serde_json::Value = serde_json::from_str(&text(&plain_json.stdout)).expect("json");
    assert_eq!(a["source"], "https://x.example/\\u{2028}a\\u{3164}b");
    a["source"] = serde_json::Value::Null;
    b["source"] = serde_json::Value::Null;
    assert_eq!(a, b);
}

#[test]
fn hangul_filler_in_the_prices_source_is_escaped_without_changing_the_amount() {
    let dir = Cleanup(temp_dir("filler-source"));
    let volume = dir.0.join("volume.json");
    fs::write(
        &volume,
        r#"{"VolumeId":"vol-01234567","Size":10,"VolumeType":"gp2","Attachments":[]}"#,
    )
    .expect("write volume");
    let plain_prices = include_str!("../prices/ebs.json");
    let odd_prices = plain_prices.replace(
        "https://aws.amazon.com/ebs/pricing/",
        "https://x.example/\\u115Fz",
    );
    assert!(odd_prices.contains("\\u115Fz"));
    let plain_path = dir.0.join("plain.json");
    let odd_path = dir.0.join("odd.json");
    fs::write(&plain_path, plain_prices).expect("write");
    fs::write(&odd_path, odd_prices).expect("write");
    let volume_arg = volume.to_str().expect("utf8");
    let plain_arg = plain_path.to_str().expect("utf8");
    let odd_arg = odd_path.to_str().expect("utf8");

    let plain_text = run(&["--prices", plain_arg, volume_arg]);
    let odd_text = run(&["--prices", odd_arg, volume_arg]);
    assert_eq!(
        odd_text.status.code(),
        Some(0),
        "{}",
        text(&odd_text.stderr)
    );
    let odd_out = text(&odd_text.stdout);
    assert!(!odd_out.contains('\u{115F}'));
    assert!(odd_out.contains("\\u{115f}"));
    let total = |s: &str| {
        s.lines()
            .filter(|l| !l.starts_with("prices:"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(total(&odd_out), total(&text(&plain_text.stdout)));

    let plain_json = run(&["--json", "--prices", plain_arg, volume_arg]);
    let odd_json = run(&["--json", "--prices", odd_arg, volume_arg]);
    assert_eq!(
        odd_json.status.code(),
        Some(0),
        "{}",
        text(&odd_json.stderr)
    );
    let odd_raw = text(&odd_json.stdout);
    assert!(!odd_raw.contains('\u{115F}'));
    let mut a: serde_json::Value = serde_json::from_str(&odd_raw).expect("json");
    let mut b: serde_json::Value = serde_json::from_str(&text(&plain_json.stdout)).expect("json");
    assert_eq!(a["source"], "https://x.example/\\u{115f}z");
    a["source"] = serde_json::Value::Null;
    b["source"] = serde_json::Value::Null;
    assert_eq!(a, b);
}

#[test]
fn multibyte_start_time_exits_2() {
    let dir = Cleanup(temp_dir("start"));
    let path = dir.0.join("snap.json");
    fs::write(
        &path,
        r#"{"SnapshotId":"snap-0123456789abcdef0","VolumeSize":8,"StartTime":"2026-10-09T00:00:0éZ"}"#,
    )
    .expect("snap");
    let out = run(&[path.to_str().expect("utf8")]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    let err = text(&out.stderr);
    assert!(err.contains("bad StartTime"));
    assert!(!err.to_ascii_lowercase().contains("panic"));
}

#[test]
fn unicode_whitespace_in_a_price_string_exits_2() {
    for pad in ["\u{a0}", "\u{2028}", "\u{3000}"] {
        let dir = Cleanup(temp_dir("uniws"));
        let prices_path = dir.0.join("prices.json");
        let builtin =
            fs::read_to_string(PathBuf::from(manifest()).join("prices/ebs.json")).expect("prices");
        let edited = builtin.replace(
            "\"gp2_storage_per_gb_month\": \"0.10\"",
            &format!("\"gp2_storage_per_gb_month\": \"{pad}0.10\""),
        );
        assert_ne!(edited, builtin);
        fs::write(&prices_path, edited).expect("write prices");
        let vol = dir.0.join("vol.json");
        fs::write(
            &vol,
            r#"{"VolumeId":"vol-01234567","Size":2,"VolumeType":"gp2","Attachments":[]}"#,
        )
        .expect("vol");
        let out = run(&[
            "--prices",
            prices_path.to_str().expect("utf8"),
            vol.to_str().expect("utf8"),
        ]);
        assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
        assert!(out.stdout.is_empty());
        assert!(text(&out.stderr).contains("bad gp2_storage_per_gb_month"));
    }
}

#[test]
fn extreme_price_on_a_small_volume_exits_2() {
    let dir = Cleanup(temp_dir("rate"));
    let prices_path = dir.0.join("prices.json");
    let builtin =
        fs::read_to_string(PathBuf::from(manifest()).join("prices/ebs.json")).expect("prices");
    let edited = builtin.replace(
        "\"gp2_storage_per_gb_month\": \"0.10\"",
        "\"gp2_storage_per_gb_month\": \"79228162514264337593543950335\"",
    );
    assert_ne!(edited, builtin);
    fs::write(&prices_path, edited).expect("write prices");
    let vol = dir.0.join("vol.json");
    fs::write(
        &vol,
        r#"{"VolumeId":"vol-01234567","Size":2,"VolumeType":"gp2","Attachments":[]}"#,
    )
    .expect("vol");
    let out = run(&[
        "--prices",
        prices_path.to_str().expect("utf8"),
        vol.to_str().expect("utf8"),
    ]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    let err = text(&out.stderr);
    assert!(err.contains("amount overflowed"));
    assert!(!err.to_ascii_lowercase().contains("panic"));
}

#[cfg(unix)]
#[test]
fn prices_fifo_is_refused_without_a_read() {
    let dir = Cleanup(temp_dir("prices-fifo"));
    let fifo = dir.0.join("prices.pipe");
    let made = Command::new("mkfifo").arg(&fifo).status().expect("mkfifo");
    assert!(made.success(), "mkfifo");
    let mut child = Command::new(bin())
        .args([
            "--prices",
            fifo.to_str().expect("utf8"),
            "examples/volumes.json",
        ])
        .current_dir(manifest())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let start = std::time::Instant::now();
    let timed_out = loop {
        match child.try_wait() {
            Ok(Some(_)) => break false,
            Ok(None) if start.elapsed() >= std::time::Duration::from_secs(2) => {
                let _ = child.kill();
                break true;
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(20)),
            Err(err) => panic!("wait: {err}"),
        }
    };
    let out = child.wait_with_output().expect("output");
    assert!(!timed_out, "prices fifo read blocked");
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    let err = text(&out.stderr);
    assert!(err.contains("not a file"), "{err}");
    assert!(err.contains("prices.pipe"), "{err}");
    assert!(!err.to_ascii_lowercase().contains("panic"));
}

#[test]
fn oversized_prices_file_is_exit_2_without_reading_it() {
    let dir = Cleanup(temp_dir("prices-big"));
    let path = dir.0.join("prices.json");
    let file = fs::File::create(&path).expect("create");
    file.set_len(268_435_457).expect("sparse len");
    drop(file);
    let out = run(&[
        "--prices",
        path.to_str().expect("utf8 path"),
        "examples/volumes.json",
    ]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    assert!(out.stdout.is_empty());
    let err = text(&out.stderr);
    assert!(err.contains("file is larger than 268435456 bytes"), "{err}");
    assert!(err.contains("prices.json"), "{err}");
}

#[cfg(target_os = "linux")]
#[test]
fn closed_stdout_is_exit_2_and_dev_null_is_exit_0() {
    let dir = Cleanup(temp_dir("closedout"));
    let path = dir.0.join("v.json");
    fs::write(
        &path,
        br#"{"Volumes":[{"VolumeId":"vol-11111111","Size":1,"VolumeType":"gp2"}]}"#,
    )
    .expect("write");
    let run_sh = |redirect: &str| {
        Command::new("sh")
            .arg("-c")
            .arg(format!("exec \"$0\" \"$1\" {redirect}"))
            .arg(bin())
            .arg(&path)
            .output()
            .expect("sh")
    };
    let closed = run_sh(">&-");
    assert_eq!(closed.status.code(), Some(2));
    let err = text(&closed.stderr);
    assert!(err.contains("could not write output"), "{err}");
    assert!(!err.contains("panicked"), "{err}");
    let null = run_sh(">/dev/null");
    assert_eq!(null.status.code(), Some(0));
    let rw_null = run_sh("1<>/dev/null");
    assert_eq!(rw_null.status.code(), Some(0), "{}", text(&rw_null.stderr));
    let full = run_sh(">/dev/full");
    assert_eq!(full.status.code(), Some(2));
}

#[cfg(target_os = "linux")]
#[test]
fn help_and_version_to_closed_or_full_stdout_exit_2() {
    for flag in ["--help", "--version"] {
        for redirect in [">&-", ">/dev/full"] {
            let out = Command::new("sh")
                .arg("-c")
                .arg(format!("exec \"$0\" \"$1\" {redirect}"))
                .arg(bin())
                .arg(flag)
                .output()
                .expect("sh");
            assert_eq!(out.status.code(), Some(2), "{flag} {redirect}");
            assert!(out.stdout.is_empty(), "{flag} {redirect}");
            let err = text(&out.stderr);
            assert!(
                err.contains("could not write output"),
                "{flag} {redirect}: {err}"
            );
        }
    }
}

const ONE_VOLUME: &str = r#"{"Volumes":[{"VolumeId":"vol-01234567","Size":2,"VolumeType":"sc1"}]}"#;

#[test]
fn json_file_with_a_leading_bom_is_still_read() {
    let dir = Cleanup(temp_dir("bom"));
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(ONE_VOLUME.as_bytes());
    let file = dir.0.join("v.json");
    fs::write(&file, bytes).expect("json");
    let out = run(&[file.to_str().expect("utf8")]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(text(&out.stdout).contains("vol-01234567"));
}

fn nested_dir(levels: usize) -> Cleanup {
    let root = Cleanup(temp_dir("deep"));
    let mut deepest = root.0.clone();
    for _ in 1..levels {
        deepest.push("d");
    }
    fs::create_dir_all(&deepest).expect("nested dirs");
    fs::write(deepest.join("v.json"), ONE_VOLUME).expect("json");
    root
}

#[test]
fn directory_nested_64_levels_is_fine() {
    let dir = nested_dir(64);
    let out = run(&[dir.0.to_str().expect("utf8")]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(text(&out.stdout).contains("vol-01234567"));
}

#[test]
fn directory_nested_65_levels_exits_2() {
    let dir = nested_dir(65);
    let out = run(&[dir.0.to_str().expect("utf8")]);
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out.stderr).contains("nested too deeply"));
}

#[test]
fn one_byte_gb_is_a_28_digit_decimal_and_readme_does_not_call_it_exact() {
    let dir = temp_dir("onebyte");
    let _cleanup = Cleanup(dir.clone());
    let snaps = dir.join("snapshots.json");
    let images = dir.join("images.json");
    fs::write(
        &snaps,
        r#"{"Snapshots":[{"SnapshotId":"snap-44444444444444444","FullSnapshotSizeInBytes":1,"StartTime":"2026-01-01T00:00:00Z","StorageTier":"standard","State":"completed"}]}"#,
    )
    .expect("write snapshots");
    fs::write(&images, r#"{"Images":[]}"#).expect("write images");
    let out = run(&[
        "--as-of",
        "2026-11-09",
        snaps.to_str().expect("path"),
        images.to_str().expect("path"),
    ]);
    let stdout = text(&out.stdout);
    assert!(
        stdout.contains("0.0000000009313225746154785156"),
        "{stdout}"
    );
    assert!(!stdout.contains("0.00000000093132257461547851562"));

    let readme = include_str!("../README.md").replace("\r\n", "\n");
    assert!(!readme.contains("as an exact decimal"));
    assert!(!readme.contains("GB and age stay exact"));
    assert!(readme.contains("at most 28 decimal places"));
    assert!(readme.contains("15625"));
}

#[test]
fn usage_error_escapes_bidi_and_newline_in_the_argument() {
    let out = run(&["f", "--older-than-days", "x\u{202e}y\nz"]);
    assert_eq!(out.status.code(), Some(2));
    let err = text(&out.stderr);
    assert!(err.contains("x\\u{202e}y\\u{a}z"), "{err}");
    assert!(!err.contains('\u{202e}'), "{err}");
    assert!(!err.contains("y\nz"), "{err}");
}

#[cfg(unix)]
#[test]
fn directory_skips_a_fifo_as_non_regular() {
    let dir = Cleanup(temp_dir("dir-fifo"));
    fs::write(
        dir.0.join("v.json"),
        r#"{"Volumes":[{"VolumeId":"vol-01234567","Size":2,"VolumeType":"sc1"}]}"#,
    )
    .expect("json");
    let fifo = dir.0.join("pipe");
    let made = Command::new("mkfifo").arg(&fifo).status().expect("mkfifo");
    assert!(made.success(), "mkfifo");
    let out = run(&[dir.0.to_str().expect("utf8")]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(text(&out.stdout).contains("skipped non-regular file"));
}

#[cfg(target_os = "linux")]
#[test]
fn non_utf8_file_name_is_shown_as_escaped_bytes() {
    use std::os::unix::ffi::OsStrExt;
    let dir = Cleanup(temp_dir("non-utf8-name"));
    let file = dir.0.join(std::ffi::OsStr::from_bytes(b"bad\xff.json"));
    fs::write(&file, b"not json").expect("write");
    let out = Command::new(bin())
        .arg(&file)
        .current_dir(manifest())
        .output()
        .expect("run binary");
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    let err = text(&out.stderr);
    assert!(err.contains("bad\\xff.json"), "{err}");
    assert!(!err.contains('\u{fffd}'), "{err}");
}

#[cfg(target_os = "linux")]
#[test]
fn valid_volume_in_a_non_utf8_file_name_still_counts() {
    use std::os::unix::ffi::OsStrExt;
    let dir = Cleanup(temp_dir("non-utf8-ok"));
    let file = dir.0.join(std::ffi::OsStr::from_bytes(b"ok\xff.json"));
    fs::write(
        &file,
        r#"{"VolumeId":"vol-01234567","Size":10,"VolumeType":"gp2","Attachments":[]}"#,
    )
    .expect("write");
    let plain = dir.0.join("plain.json");
    fs::copy(&file, &plain).expect("copy");
    let odd = Command::new(bin())
        .arg(&file)
        .current_dir(manifest())
        .output()
        .expect("run binary");
    let base = run(&[plain.to_str().expect("utf8")]);
    assert_eq!(odd.status.code(), Some(0), "{}", text(&odd.stderr));
    assert_eq!(base.status.code(), Some(0));
    let stdout = text(&odd.stdout);
    assert!(stdout.contains("\nestimated_usd_month=1.00\n"), "{stdout}");
    assert!(stdout.contains("vol-01234567"), "{stdout}");
}

/// Number of lines clap prints for a plain, harmless bad argument of the same shape.
fn stderr_lines(args: &[&str]) -> usize {
    let out = run(args);
    assert_eq!(out.status.code(), Some(2));
    text(&out.stderr).lines().count()
}

fn assert_one_clap_message(args: &[&str], benign: &[&str]) {
    let out = run(args);
    assert_eq!(out.status.code(), Some(2), "{args:?}");
    assert!(out.stdout.is_empty(), "{args:?}");
    let err = text(&out.stderr);
    for bad in ['\u{1b}', '\r', '\u{7}', '\u{202e}', '\u{2028}'] {
        assert!(!err.contains(bad), "{args:?}: raw {bad:?} in {err:?}");
    }
    assert_eq!(
        err.lines().filter(|l| l.starts_with("error:")).count(),
        1,
        "{args:?}: {err:?}"
    );
    assert!(
        !err.lines().any(|line| line.starts_with("FAKE")),
        "{args:?}: {err:?}"
    );
    assert_eq!(
        err.lines().count(),
        stderr_lines(benign),
        "{args:?}: {err:?}"
    );
    assert!(err.contains("\\u{"), "{args:?}: {err:?}");
}

#[test]
fn usage_error_with_an_escape_sequence_and_newline_is_one_message() {
    assert_one_clap_message(
        &[
            "examples/volumes.json",
            "--older-than-days",
            "1\x1b[31m\nFAKE: line",
        ],
        &["examples/volumes.json", "--older-than-days", "abc"],
    );
    assert_one_clap_message(
        &["examples/volumes.json", "--x\x1b[31m\nFAKE: line"],
        &["examples/volumes.json", "--xyz"],
    );
}

#[test]
fn usage_error_with_a_lone_escape_an_osc_and_a_carriage_return_is_one_message() {
    let older = ["examples/volumes.json", "--older-than-days"];
    let benign = ["examples/volumes.json", "--older-than-days", "abc"];
    for value in [
        "1\x1b\nFAKE: line",
        "1\x1b]0;t\x07\nFAKE: line",
        "1\x1b[31m\rFAKE: line",
        "1\u{202e}\nFAKE: line",
    ] {
        let args = [older[0], older[1], value];
        assert_one_clap_message(&args, &benign);
    }
    let json = "--json=\x1b[2J\r\nerror: FAKE";
    let out = run(&["examples/volumes.json", json]);
    assert_eq!(out.status.code(), Some(2));
    let err = text(&out.stderr);
    assert_eq!(
        err.lines().filter(|l| l.starts_with("error:")).count(),
        1,
        "{err:?}"
    );
    assert!(!err.contains('\r') && !err.contains('\u{1b}'), "{err:?}");
    assert_eq!(
        err.lines().count(),
        stderr_lines(&["examples/volumes.json", "--json=abc"]),
        "{err:?}"
    );
}

#[test]
fn usage_error_escapes_invisible_characters_in_the_argument() {
    for ch in [
        '\u{2028}',
        '\u{200b}',
        '\u{2800}',
        '\u{61c}',
        '\u{ad}',
        '\u{e0100}',
    ] {
        let value = format!("1{ch}FAKE");
        let out = run(&["examples/volumes.json", "--older-than-days", &value]);
        assert_eq!(out.status.code(), Some(2));
        let err = text(&out.stderr);
        assert!(!err.contains(ch), "{:04X}: {err:?}", ch as u32);
        assert!(
            err.contains(&format!("\\u{{{:x}}}", ch as u32)),
            "{:04X}: {err:?}",
            ch as u32
        );
    }
}

#[test]
fn as_of_error_names_the_year_range() {
    for date in ["1969-01-01", "2200-01-01", "nonsense"] {
        let out = run(&["--as-of", date, "examples/volumes.json"]);
        assert_eq!(out.status.code(), Some(2), "{date}");
        let err = text(&out.stderr);
        assert!(
            err.contains("1970-01-01 through 2199-12-31"),
            "{date}: {err}"
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn usage_error_keeps_the_reason_when_the_escaped_arguments_parse() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let out = Command::new(bin())
        .arg("examples/volumes.json")
        .arg("--as-of")
        .arg(OsStr::from_bytes(b"\xff"))
        .arg(OsStr::from_bytes(b"f\x1b.json"))
        .current_dir(manifest())
        .output()
        .expect("run binary");
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    let err = text(&out.stderr);
    assert!(err.contains("invalid UTF-8"), "{err:?}");
    assert!(!err.contains("invalid arguments"), "{err:?}");
    assert!(!err.contains('\u{1b}'), "{err:?}");
    assert_eq!(
        err.lines().filter(|l| l.starts_with("error:")).count(),
        1,
        "{err:?}"
    );
}

#[test]
fn usage_error_escapes_spacing_and_format_characters_in_the_argument() {
    for ch in [
        '\u{a0}',
        '\u{1680}',
        '\u{2003}',
        '\u{202f}',
        '\u{3000}',
        '\u{600}',
        '\u{6dd}',
        '\u{70f}',
        '\u{8e2}',
        '\u{1d173}',
        '\u{fffc}',
    ] {
        let value = format!("1{ch}X");
        let out = run(&["examples/volumes.json", "--older-than-days", &value]);
        assert_eq!(out.status.code(), Some(2));
        let err = text(&out.stderr);
        assert!(!err.contains(ch), "{:04X}: {err:?}", ch as u32);
        assert!(
            err.contains(&format!("\\u{{{:x}}}", ch as u32)),
            "{:04X}: {err:?}",
            ch as u32
        );
    }
}

#[test]
fn usage_error_escapes_del_and_backspace_in_the_argument() {
    let out = run(&["examples/volumes.json", "--older-than-days", "1\x7fX\x08Y"]);
    assert_eq!(out.status.code(), Some(2));
    let err = text(&out.stderr);
    assert!(!err.contains('\x7f') && !err.contains('\x08'), "{err:?}");
    assert!(err.contains("\\u{7f}") && err.contains("\\u{8}"), "{err:?}");
}

const BIG_VOLUME: &str =
    r#"{"Volumes":[{"VolumeId":"vol-11111111","Size":18446744073709551615,"VolumeType":"gp2"}]}"#;

#[test]
fn fail_usd_that_would_be_rounded_exits_2() {
    let dir = Cleanup(temp_dir("failround"));
    let vol = dir.0.join("big.json");
    fs::write(&vol, BIG_VOLUME).expect("write");
    let vol = vol.to_str().expect("utf8");
    // The row is exactly 1844674407370955161.50. The threshold has 30 digits and
    // used to be rounded to ...161.5, so a row above it passed.
    for amount in [
        "1844674407370955161.49999999999",
        "79228162514264337593543950334.9",
        "1234567890123456789.1234567890123456789",
    ] {
        let out = run(&["--fail-usd", amount, vol]);
        assert_eq!(out.status.code(), Some(2), "{amount}");
        assert!(out.stdout.is_empty(), "{amount}");
        let err = text(&out.stderr);
        assert!(err.contains("does not fit"), "{amount}: {err}");
        assert!(!err.contains(amount), "{amount}: {err}");
    }
    // Digits that fit exactly, including leading and trailing zeros, still work.
    for (amount, code) in [
        ("1844674407370955161.5", 0),
        ("1844674407370955161.4999999999", 1),
        ("0001844674407370955161.50000000000", 0),
        ("79228162514264337593543950335", 0),
        ("0.0000000000000000000000000001", 1),
    ] {
        let out = run(&["--fail-usd", amount, vol]);
        assert_eq!(out.status.code(), Some(code), "{amount}");
    }
}

#[test]
fn price_rate_with_more_than_28_significant_digits_exits_2() {
    let dir = Cleanup(temp_dir("ratedigits"));
    let builtin =
        fs::read_to_string(PathBuf::from(manifest()).join("prices/ebs.json")).expect("prices");
    let vol = dir.0.join("vol.json");
    fs::write(
        &vol,
        r#"{"VolumeId":"vol-01234567","Size":1,"VolumeType":"gp2","Attachments":[]}"#,
    )
    .expect("vol");
    for (rate, code) in [
        ("1234567890123456789.1234567890123456789", 2),
        ("0.10000000000000000000000000000", 2),
        ("123456789012345678.9012345678", 0),
    ] {
        let edited = builtin.replace(
            "\"gp2_storage_per_gb_month\": \"0.10\"",
            &format!("\"gp2_storage_per_gb_month\": \"{rate}\""),
        );
        assert_ne!(edited, builtin);
        let prices = dir.0.join("prices.json");
        fs::write(&prices, edited).expect("write prices");
        let out = run(&[
            "--prices",
            prices.to_str().expect("utf8"),
            vol.to_str().expect("utf8"),
        ]);
        let want = if code == 0 { 0 } else { 2 };
        assert_eq!(
            out.status.code(),
            Some(want),
            "{rate}: {}",
            text(&out.stderr)
        );
        if want == 2 {
            assert!(text(&out.stderr).contains("does not fit"), "{rate}");
        }
    }
}

/// `{"Volumes":[]}` followed by spaces, `total` bytes long, written in pieces.
fn write_padded_empty_document(out: &mut dyn Write, total: u64) -> std::io::Result<()> {
    let head = br#"{"Volumes":[]}"#;
    out.write_all(head)?;
    let chunk = vec![b' '; 1 << 20];
    let mut left = total - head.len() as u64;
    while left > 0 {
        let n = left.min(chunk.len() as u64) as usize;
        out.write_all(&chunk[..n])?;
        left -= n as u64;
    }
    Ok(())
}

fn run_stdin_stream(total: u64) -> Output {
    let mut child = Command::new(bin())
        .arg("-")
        .current_dir(manifest())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let mut stdin = child.stdin.take().expect("stdin");
    // The program stops reading one byte past the limit; a broken pipe is expected then.
    let _ = write_padded_empty_document(&mut stdin, total);
    drop(stdin);
    child.wait_with_output().expect("wait")
}

#[test]
fn input_at_the_size_limit_is_read_and_one_byte_more_is_refused() {
    const LIMIT: u64 = 268_435_456;
    let dir = Cleanup(temp_dir("limit"));
    let path = dir.0.join("exact.json");
    let mut file = fs::File::create(&path).expect("create");
    write_padded_empty_document(&mut file, LIMIT).expect("write");
    drop(file);
    assert_eq!(fs::metadata(&path).expect("meta").len(), LIMIT);
    let exact = run(&[path.to_str().expect("utf8")]);
    assert_eq!(exact.status.code(), Some(0), "{}", text(&exact.stderr));
    fs::remove_file(&path).expect("remove");

    // One byte more: the size is read from the file system, so a sparse file will do.
    let over = dir.0.join("over.json");
    let file = fs::File::create(&over).expect("create");
    file.set_len(LIMIT + 1).expect("len");
    drop(file);
    let out = run(&[over.to_str().expect("utf8")]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert!(text(&out.stderr).contains("file is larger than 268435456 bytes"));
    fs::remove_file(&over).expect("remove");

    for (total, code) in [(LIMIT, 0), (LIMIT + 1, 2)] {
        let out = run_stdin_stream(total);
        assert_eq!(out.status.code(), Some(code), "stdin {total}");
        if code == 2 {
            assert!(out.stdout.is_empty(), "stdin {total}");
            assert!(
                text(&out.stderr).contains("file is larger than 268435456 bytes"),
                "stdin {total}: {}",
                text(&out.stderr)
            );
        }
    }

    for (total, code) in [(LIMIT, 0), (LIMIT + 1, 2)] {
        let gz = dir.0.join("padded.json.gz");
        let file = fs::File::create(&gz).expect("create");
        let mut encoder = GzEncoder::new(file, Compression::fast());
        write_padded_empty_document(&mut encoder, total).expect("gzip");
        encoder.finish().expect("finish");
        let out = run(&[gz.to_str().expect("utf8")]);
        assert_eq!(out.status.code(), Some(code), "gzip {total}");
        if code == 2 {
            assert!(
                text(&out.stderr).contains("larger than 268435456 bytes"),
                "gzip {total}: {}",
                text(&out.stderr)
            );
        }
        fs::remove_file(&gz).expect("remove");
    }
}

#[cfg(unix)]
#[test]
fn a_symlink_named_on_the_command_line_is_followed() {
    let dir = Cleanup(temp_dir("cmdlink"));
    let real = dir.0.join("real.json");
    fs::write(
        &real,
        r#"{"VolumeId":"vol-01234567","Size":10,"VolumeType":"gp2","Attachments":[]}"#,
    )
    .expect("write");
    let link = dir.0.join("link.json");
    std::os::unix::fs::symlink(&real, &link).expect("symlink");
    let out = run(&[link.to_str().expect("utf8")]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(text(&out.stdout).contains("vol-01234567"));
}

#[test]
fn json_mode_sends_warnings_to_standard_error_and_into_the_object() {
    let mut args = vec!["--json", "--as-of", "2026-11-09"];
    args.extend(samples());
    let out = run(&args);
    assert_eq!(out.status.code(), Some(0));
    let stdout = text(&out.stdout);
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(!stdout.contains("\nwarning:"), "{stdout}");
    assert!(stdout.contains("\"warnings\""), "{stdout}");
    let err = text(&out.stderr);
    assert_eq!(
        err.lines().filter(|l| l.starts_with("warning: ")).count(),
        3,
        "{err}"
    );
    assert!(err.contains("warning: vol-44444444444444444: volume type standard is not priced"));
}

#[test]
fn nesting_limit_and_input_tolerance_are_as_the_readme_says() {
    let dir = Cleanup(temp_dir("tolerance"));
    let path = |name: &str| dir.0.join(name);
    let p = |name: &str| path(name).to_str().expect("utf8").to_string();

    // An object that holds arrays nested 126 deep is read; 127 deep is not JSON.
    for (depth, code) in [(126, 0), (127, 2)] {
        let doc = format!(
            r#"{{"Volumes":[],"X":{}{}}}"#,
            "[".repeat(depth),
            "]".repeat(depth)
        );
        fs::write(path("deep.json"), doc).expect("write");
        let out = run(&[&p("deep.json")]);
        assert_eq!(out.status.code(), Some(code), "depth {depth}");
        if code == 2 {
            assert!(text(&out.stderr).contains("not JSON"), "depth {depth}");
        }
    }

    // A number may be a string.
    fs::write(
        path("str.json"),
        r#"{"Volumes":[{"VolumeId":"vol-11111111","Size":"100","VolumeType":"gp2"}]}"#,
    )
    .expect("write");
    let out = run(&[&p("str.json")]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(text(&out.stdout).contains("\nestimated_usd_month=10.00\n"));
    fs::write(
        path("snap.json"),
        r#"{"Snapshots":[{"SnapshotId":"snap-11111111","StartTime":"2026-09-09T12:00:00.000Z","State":"completed","VolumeSize":1,"FullSnapshotSizeInBytes":"5"}]}"#,
    )
    .expect("write");
    fs::write(path("img.json"), r#"{"Images":[]}"#).expect("write");
    let out = run(&["--as-of", "2026-11-09", &p("snap.json"), &p("img.json")]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(text(&out.stdout).contains("full-snapshot-bytes"));

    // The currency is shown and not checked.
    let builtin =
        fs::read_to_string(PathBuf::from(manifest()).join("prices/ebs.json")).expect("prices");
    let eur = builtin.replace("\"currency\": \"USD\"", "\"currency\": \"EUR\"");
    assert_ne!(eur, builtin);
    fs::write(path("eur.json"), eur).expect("write");
    let out = run(&["--prices", &p("eur.json"), &p("str.json")]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(text(&out.stdout).contains("prices: EUR,"));

    // A leading plus is a number to clap.
    let out = run(&["--older-than-days", "+5", &p("str.json")]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));

    // A completed snapshot that starts on the --as-of day is an error that names the snapshot, not the file.
    fs::write(
        path("today.json"),
        r#"{"Snapshots":[{"SnapshotId":"snap-11111111","StartTime":"2026-11-09T12:00:00.000Z","State":"completed","VolumeSize":1}]}"#,
    )
    .expect("write");
    let out = run(&["--as-of", "2026-11-09", &p("today.json")]);
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out.stderr).contains("snap-11111111: StartTime is after --as-of"));
}

/// Runs the binary with the built-in prices where one key has another value.
fn run_with_rate(dir: &std::path::Path, key: &str, rate: &str, files: &[&str]) -> Output {
    let builtin =
        fs::read_to_string(PathBuf::from(manifest()).join("prices/ebs.json")).expect("prices");
    let mut found = false;
    let mut edited = String::new();
    for line in builtin.lines() {
        if line.trim_start().starts_with(&format!("\"{key}\"")) {
            found = true;
            edited.push_str(&format!("  \"{key}\": \"{rate}\",\n"));
        } else {
            edited.push_str(line);
            edited.push('\n');
        }
    }
    assert!(found, "{key}");
    let prices = dir.join("rate-prices.json");
    fs::write(&prices, edited).expect("write prices");
    let mut args = vec!["--prices", prices.to_str().expect("utf8")];
    args.extend_from_slice(files);
    run(&args)
}

#[test]
fn a_product_or_sum_that_would_lose_digits_is_an_error() {
    let dir = Cleanup(temp_dir("exactmath"));
    let one = dir.0.join("one.json");
    fs::write(
        &one,
        r#"{"VolumeId":"vol-01234567","Size":18446744073709551615,"VolumeType":"gp2","Attachments":[]}"#,
    )
    .expect("write");
    let two = dir.0.join("two.json");
    fs::write(
        &two,
        r#"[{"VolumeId":"vol-01234567","Size":1,"VolumeType":"gp2","Attachments":[]},{"VolumeId":"vol-76543210","Size":1,"VolumeType":"gp2","Attachments":[]}]"#,
    )
    .expect("write");
    let gp3 = dir.0.join("gp3.json");
    fs::write(
        &gp3,
        r#"{"VolumeId":"vol-01234567","Size":1,"VolumeType":"gp3","Iops":3001,"Throughput":125,"Attachments":[]}"#,
    )
    .expect("write");
    let one = one.to_str().expect("utf8");
    let two = two.to_str().expect("utf8");
    let gp3 = gp3.to_str().expect("utf8");
    // A product with 33 digits used to be rounded to 28 and shown as ...180.70.
    let out = run_with_rate(
        &dir.0,
        "gp2_storage_per_gb_month",
        "123456789.41249",
        &[one],
    );
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stdout));
    assert!(text(&out.stderr).contains("amount overflowed"));
    // The row fits, the total of two rows needs 30 digits: it used to lose the cents.
    let out = run_with_rate(
        &dir.0,
        "gp2_storage_per_gb_month",
        "500000000000000000000000000.01",
        &[two],
    );
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stdout));
    assert!(text(&out.stderr).contains("amount overflowed"));
    // A sum inside one gp3 volume: 79228162514264337593543950335 + 0.08.
    let out = run_with_rate(
        &dir.0,
        "gp3_iops_per_iops_month",
        "79228162514264337593543950335",
        &[gp3],
    );
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stdout));
    assert!(text(&out.stderr).contains("amount overflowed"));
    // Sums and products that fit exactly are unchanged, with trailing zeros that cancel.
    let out = run_with_rate(
        &dir.0,
        "gp2_storage_per_gb_month",
        "500000000000000000000000000.25",
        &[two],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(text(&out.stdout).contains("1000000000000000000000000000.50"));
}

#[test]
fn a_rounded_product_never_shows_a_cent_the_exact_amount_does_not_have() {
    let dir = Cleanup(temp_dir("doubleround"));
    let write = |name: &str, size: u64| -> String {
        let path = dir.0.join(name);
        fs::write(
            &path,
            format!(
                r#"{{"Volumes":[{{"VolumeId":"vol-01234567","Size":{size},"VolumeType":"gp2","State":"available","Attachments":[]}}]}}"#
            ),
        )
        .expect("write");
        path.to_str().expect("utf8").to_string()
    };
    // Exact: 411.0149999999999999999999999956, so 411.01. Rounding first gave 411.02.
    // Exact: 1000000000000000.01499999999995657..., so ...00.01, never ...00.02.
    let cases = [
        (1001_u64, "0.4106043956043956043956043956", "411.02"),
        (
            1_000_000_000_000_038,
            "0.9999999999999620150000000014",
            "1000000000000000.02",
        ),
    ];
    for (size, rate, wrong) in cases {
        let file = write("v.json", size);
        for json in [false, true] {
            let mut files = vec![file.as_str()];
            if json {
                files.insert(0, "--json");
            }
            let out = run_with_rate(&dir.0, "gp2_storage_per_gb_month", rate, &files);
            let shown = text(&out.stdout);
            let right = match size {
                1001 => "411.01",
                _ => "1000000000000000.01",
            };
            assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
            assert!(shown.contains(right), "{size} {rate}: {shown}");
            assert!(!shown.contains(wrong), "{size} {rate}: {shown}");
        }
    }
    // The same input one GB smaller is exact and unchanged.
    let file = write("w.json", 1000);
    let out = run_with_rate(
        &dir.0,
        "gp2_storage_per_gb_month",
        "0.4106043956043956043956043956",
        &[file.as_str()],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(text(&out.stdout).contains("410.60"));
}

#[test]
fn help_says_exit_2_also_covers_standard_output_that_cannot_be_written() {
    let out = run(&["--help"]);
    assert_eq!(out.status.code(), Some(0));
    let help = text(&out.stdout);
    assert!(
        help.contains("2 input or usage error, or standard output that cannot be written."),
        "{help}"
    );
}

#[test]
fn fail_usd_length_and_zero_rules_are_as_the_readme_says() {
    let dir = Cleanup(temp_dir("zeros"));
    let vol = dir.0.join("vol.json");
    fs::write(
        &vol,
        r#"{"VolumeId":"vol-01234567","Size":10,"VolumeType":"gp2","Attachments":[]}"#,
    )
    .expect("write");
    let vol = vol.to_str().expect("utf8");
    for (amount, code) in [
        // 40 characters are read, with leading zeros that do not count as digits.
        ("0000000000000000000000000000000000000001", 0),
        ("79228162514264337593543950335.0", 0),
        // 41 characters, or a zero in the 29th decimal, is refused.
        ("00000000000000000000000000000000000000001", 2),
        ("79228162514264337593543950335.00000000000", 2),
        ("0.50000000000000000000000000000", 2),
    ] {
        let out = run(&["--fail-usd", amount, vol]);
        assert_eq!(out.status.code(), Some(code), "{amount}");
    }
}

#[test]
fn volume_type_case_and_price_source_controls_are_as_the_readme_says() {
    let dir = Cleanup(temp_dir("source"));
    let vol = dir.0.join("vol.json");
    fs::write(
        &vol,
        r#"{"VolumeId":"vol-01234567","Size":10,"VolumeType":"GP2","Attachments":[]}"#,
    )
    .expect("write");
    let vol = vol.to_str().expect("utf8");
    let out = run(&[vol]);
    assert_eq!(out.status.code(), Some(0));
    assert!(text(&out.stdout).contains("estimated_usd_month=1.00"));
    let builtin =
        fs::read_to_string(PathBuf::from(manifest()).join("prices/ebs.json")).expect("prices");
    assert!(builtin.contains("\"source\": \"https://aws.amazon.com/ebs/pricing/\""));
    for (source, code) in [
        ("a\\nb", 2),
        ("a\\u001b[31mb", 2),
        ("a\\u007fb", 2),
        ("a\\u0085b", 2),
        ("a\\u202eb", 0),
        ("a\\u00a0b", 0),
    ] {
        let edited = builtin.replace(
            "\"source\": \"https://aws.amazon.com/ebs/pricing/\"",
            &format!("\"source\": \"{source}\""),
        );
        let prices = dir.0.join("prices.json");
        fs::write(&prices, edited).expect("write");
        let out = run(&["--prices", prices.to_str().expect("utf8"), vol]);
        assert_eq!(out.status.code(), Some(code), "{source}");
        if code == 2 {
            assert!(text(&out.stderr).contains("bad source"), "{source}");
        } else {
            assert!(text(&out.stdout).contains("\\u{"), "{source}");
        }
    }
}

/// The scanner counts every JSON value and every object key. More than 16777216 of them
/// is refused before the tree is built (the README gives the limit and its measured cost).
const JSON_ITEM_LIMIT: usize = 16_777_216;

fn document_with_items(items: usize) -> String {
    // `{"Volumes":[],"T":[` holds the top object, two keys, the empty array, and the
    // array of `T`: 5 items before the numbers.
    let mut doc = String::with_capacity(items * 2 + 32);
    doc.push_str(r#"{"Volumes":[],"T":["#);
    doc.push_str(&"0,".repeat(items - 5 - 1));
    doc.push_str("0]}");
    doc
}

#[test]
fn a_document_with_too_many_json_values_is_refused() {
    let dir = Cleanup(temp_dir("too-many-values"));
    let path = dir.0.join("many.json");
    fs::write(&path, document_with_items(JSON_ITEM_LIMIT + 1)).expect("write");
    let out = run(&[path.to_str().expect("utf8 path")]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    assert!(out.stdout.is_empty());
    let err = text(&out.stderr);
    assert!(
        err.contains("too many JSON values (at most 16777216)"),
        "{err}"
    );
    assert!(err.contains("many.json"), "{err}");

    // The same overload in a price file.
    let prices = dir.0.join("prices.json");
    let builtin = fs::read_to_string(format!("{}/prices/ebs.json", manifest())).expect("builtin");
    let body = builtin.trim_end().trim_end_matches('}');
    let extra = format!("{body},\"T\":[{}0]}}", "0,".repeat(JSON_ITEM_LIMIT));
    fs::write(&prices, extra).expect("write");
    let out = run(&[
        "--prices",
        prices.to_str().expect("utf8 path"),
        samples()[0],
    ]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    assert!(text(&out.stderr).contains("too many JSON values"));
}

/// Copy the built-in price file and replace the named rates.
fn prices_with(dir: &std::path::Path, pairs: &[(&str, &str)]) -> String {
    let mut text =
        fs::read_to_string(PathBuf::from(manifest()).join("prices/ebs.json")).expect("prices");
    for (key, rate) in pairs {
        let needle = format!("\"{key}\": \"");
        let start = text.find(&needle).unwrap_or_else(|| panic!("{key}"));
        let value_at = start + needle.len();
        let end = text[value_at..].find('"').expect("rate end");
        text.replace_range(value_at..value_at + end, rate);
    }
    let path = dir.join("prices.json");
    fs::write(&path, text).expect("write prices");
    path.to_str().expect("utf8").to_string()
}

#[test]
fn a_total_of_rounded_products_uses_the_exact_sum() {
    // 2 * 1000000000000003 * 20.05736282578990000041152263
    // = 40114725651579920.3449999999994..., which is 40114725651579920.34
    // once, half away from zero. The sum of the rounded products is ...20.345
    // and shows ...20.35.
    let dir = Cleanup(temp_dir("exact-sum"));
    let prices = prices_with(
        &dir.0,
        &[("gp2_storage_per_gb_month", "20.05736282578990000041152263")],
    );
    let volumes = dir.0.join("volumes.json");
    fs::write(
        &volumes,
        r#"{"Volumes":[{"VolumeId":"vol-01234567","Size":1000000000000003,"VolumeType":"gp2","Attachments":[]},{"VolumeId":"vol-76543210","Size":1000000000000003,"VolumeType":"gp2","Attachments":[]}]}"#,
    )
    .expect("write");
    let volumes = volumes.to_str().expect("utf8");
    for json in [false, true] {
        let mut args = vec![prices.as_str(), volumes];
        if json {
            args.insert(0, "--json");
        } else {
            args.insert(0, "--prices");
        }
        if json {
            args.insert(1, "--prices");
        }
        let out = run(&args);
        let shown = text(&out.stdout);
        assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
        assert!(shown.contains("40114725651579920.34"), "{shown}");
        assert!(!shown.contains("40114725651579920.35"), "{shown}");
    }
}

#[test]
fn a_row_sums_exact_parts_that_meet_on_a_half_cent() {
    // Storage is the product above (exact fraction just under ...60.1725).
    // One extra IOPS at 0.1725 is exact. The exact sum is ...60.3449999999997,
    // which is ...60.34 once. The rounded parts add to ...60.345 and show ...60.35.
    let dir = Cleanup(temp_dir("exact-row"));
    let prices = prices_with(
        &dir.0,
        &[
            ("gp3_storage_per_gb_month", "20.05736282578990000041152263"),
            ("gp3_iops_per_iops_month", "0.1725"),
        ],
    );
    let volume = dir.0.join("volume.json");
    fs::write(
        &volume,
        r#"{"VolumeId":"vol-01234567","Size":1000000000000003,"VolumeType":"gp3","Iops":3001,"Throughput":125,"Attachments":[]}"#,
    )
    .expect("write");
    let out = run(&["--prices", prices.as_str(), volume.to_str().expect("utf8")]);
    let shown = text(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(shown.contains("20057362825789960.34"), "{shown}");
    assert!(!shown.contains("20057362825789960.35"), "{shown}");
}

#[test]
fn fail_usd_compares_the_exact_amount() {
    // 3 * 2.6409387504754779197847983464 = 7.9228162514264337593543950392,
    // which is greater than 7.922816251426433759354395039. The product rounded
    // into a decimal equals that limit, so the rounded value does not exceed it.
    let dir = Cleanup(temp_dir("exact-limit"));
    let prices = prices_with(
        &dir.0,
        &[("gp2_storage_per_gb_month", "2.6409387504754779197847983464")],
    );
    let volume = dir.0.join("volume.json");
    fs::write(
        &volume,
        r#"{"VolumeId":"vol-01234567","Size":3,"VolumeType":"gp2","Attachments":[]}"#,
    )
    .expect("write");
    let out = run(&[
        "--prices",
        prices.as_str(),
        "--fail-usd",
        "7.922816251426433759354395039",
        volume.to_str().expect("utf8"),
    ]);
    let shown = text(&out.stdout);
    assert_eq!(out.status.code(), Some(1), "{shown}{}", text(&out.stderr));
    assert!(shown.contains("7.92"), "{shown}");
    assert!(shown.contains("threshold exceeded: yes"), "{shown}");
}

#[test]
fn one_product_near_a_half_cent_rounds_once() {
    // 1001 * 0.4106043956043956043956043956
    // = 411.0149999999999999999999999956, which is 411.01 once.
    let dir = Cleanup(temp_dir("exact-once"));
    let prices = prices_with(
        &dir.0,
        &[("gp2_storage_per_gb_month", "0.4106043956043956043956043956")],
    );
    let volume = dir.0.join("volume.json");
    fs::write(
        &volume,
        r#"{"VolumeId":"vol-01234567","Size":1001,"VolumeType":"gp2","Attachments":[]}"#,
    )
    .expect("write");
    let out = run(&["--prices", prices.as_str(), volume.to_str().expect("utf8")]);
    let shown = text(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(shown.contains("411.01"), "{shown}");
    assert!(!shown.contains("411.02"), "{shown}");
}

#[test]
fn gb_from_snapshot_bytes_rounds_half_away_from_zero() {
    // 2 / 1024^3 rounded half away from zero at 28 decimals ends in 313.
    // 10 / 1024^3 ends in 563. Half to even ends in 312 and 562.
    let dir = Cleanup(temp_dir("exact-gb"));
    let images = dir.0.join("images.json");
    fs::write(&images, r#"{"Images":[]}"#).expect("images");
    let images = images.to_str().expect("utf8").to_string();
    let cases = [
        (2_u64, "0.0000000018626451492309570313"),
        (10, "0.0000000093132257461547851563"),
    ];
    for (bytes, want) in cases {
        let snaps = dir.0.join(format!("s{bytes}.json"));
        fs::write(
            &snaps,
            format!(
                r#"{{"Snapshots":[{{"SnapshotId":"snap-44444444444444444","FullSnapshotSizeInBytes":{bytes},"StartTime":"2026-01-01T00:00:00Z","StorageTier":"standard","State":"completed"}}]}}"#
            ),
        )
        .expect("snaps");
        let out = run(&[
            "--as-of",
            "2026-11-09",
            snaps.to_str().expect("utf8"),
            images.as_str(),
        ]);
        let shown = text(&out.stdout);
        assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
        assert!(shown.contains(want), "{bytes}: {shown}");
    }
}

#[test]
fn a_single_object_with_a_volume_and_an_image_id_is_a_mixed_document() {
    let dir = Cleanup(temp_dir("single-mixed"));
    let one = r#"{"VolumeId":"vol-01234567","ImageId":"ami-01234567","Size":1,"VolumeType":"gp2"}"#;
    let single = dir.0.join("single.json");
    let array = dir.0.join("array.json");
    fs::write(&single, one).expect("write");
    fs::write(&array, format!("[{one}]")).expect("write");
    for path in [&single, &array] {
        let out = run(&[path.to_str().expect("utf8 path")]);
        assert_eq!(out.status.code(), Some(2), "{path:?}");
        assert!(text(&out.stderr).contains("mixed document"), "{path:?}");
        assert!(out.stdout.is_empty());
    }
}

#[test]
fn totals_are_the_exact_sum_of_unrounded_products_not_of_rounded_rows() {
    // Rate 0.102 per GB for gp2 and for standard snapshots, 1 GB each, three
    // volumes and three snapshots. Each row is 0.102 and shows 0.10. Fixed
    // vectors (Python fractions): a class is 0.306, which is 0.31 (rounded rows
    // would give 0.30), and the report is 0.612, which is 0.61 (adding the
    // rounded classes would give 0.62, adding rounded rows 0.60).
    let dir = Cleanup(temp_dir("exact-total"));
    let prices = prices_with(
        &dir.0,
        &[
            ("gp2_storage_per_gb_month", "0.102"),
            ("snapshot_standard_per_gb_month", "0.102"),
        ],
    );
    let volumes = dir.0.join("volumes.json");
    fs::write(
        &volumes,
        r#"{"Volumes":[{"VolumeId":"vol-00000001","Size":1,"VolumeType":"gp2","Attachments":[]},{"VolumeId":"vol-00000002","Size":1,"VolumeType":"gp2","Attachments":[]},{"VolumeId":"vol-00000003","Size":1,"VolumeType":"gp2","Attachments":[]}]}"#,
    )
    .expect("write");
    let mut snapshot_rows = Vec::new();
    for n in 1..=3 {
        snapshot_rows.push(format!(
            r#"{{"SnapshotId":"snap-{n:017}","VolumeSize":1,"StartTime":"2026-01-01T00:00:00Z","StorageTier":"standard","State":"completed"}}"#
        ));
    }
    let snapshots = dir.0.join("snapshots.json");
    fs::write(
        &snapshots,
        format!(r#"{{"Snapshots":[{}]}}"#, snapshot_rows.join(",")),
    )
    .expect("write");
    let images = dir.0.join("images.json");
    fs::write(&images, r#"{"Images":[]}"#).expect("write");
    let args = [
        "--prices",
        prices.as_str(),
        "--as-of",
        "2026-11-09",
        "--json",
        volumes.to_str().expect("utf8"),
        snapshots.to_str().expect("utf8"),
        images.to_str().expect("utf8"),
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    for row in value["volumes"].as_array().expect("volumes") {
        assert_eq!(row["usd_month"], "0.10");
    }
    for row in value["snapshots"].as_array().expect("snapshots") {
        assert_eq!(row["usd_month"], "0.10");
    }
    assert_eq!(value["volume_estimated_usd_month"], "0.31");
    assert_eq!(value["snapshot_estimated_usd_month"], "0.31");
    assert_eq!(value["estimated_usd_month"], "0.61");
    // The text report shows the same exact totals.
    let out = run(&[
        "--prices",
        prices.as_str(),
        "--as-of",
        "2026-11-09",
        volumes.to_str().expect("utf8"),
        snapshots.to_str().expect("utf8"),
        images.to_str().expect("utf8"),
    ]);
    let shown = text(&out.stdout);
    assert!(shown.contains("0.31"), "{shown}");
    assert!(shown.contains("0.61"), "{shown}");
}

fn stderr_of_price_age(extra: &[&str], as_of: &str) -> (String, String) {
    let mut args = vec!["--as-of", as_of];
    args.extend(extra);
    args.extend(samples());
    let out = run(&args);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    (text(&out.stdout), text(&out.stderr))
}

#[test]
fn price_age_warning_is_also_on_standard_error_in_text_and_markdown() {
    for extra in [&[][..], &["--md"][..]] {
        let (stdout, stderr) = stderr_of_price_age(extra, "2027-05-09");
        assert!(
            stderr.contains("warning: price data is 181 days old (dated 2026-11-09"),
            "{extra:?}: {stderr}"
        );
        assert!(stdout.contains("price data is 181 days old"), "{stdout}");
    }
}

#[test]
fn price_age_warning_is_not_written_with_quiet() {
    let (stdout, stderr) = stderr_of_price_age(&["--quiet"], "2027-05-09");
    assert!(!stdout.contains("price data is"), "{stdout}");
    assert!(!stderr.contains("price data is"), "{stderr}");
}

#[test]
fn price_date_outside_1970_to_2199_still_warns() {
    let dir = Cleanup(temp_dir("odd-price-dates"));
    for date in ["1900-01-01", "0001-01-01", "0000-01-01", "1969-12-31"] {
        let prices = include_str!("../prices/ebs.json")
            .replace("\"date\": \"2026-11-09\"", &format!("\"date\": \"{date}\""));
        assert!(prices.contains(date));
        let path = dir.0.join(format!("prices-{date}.json"));
        fs::write(&path, prices).expect("write");
        let mut args = vec!["--prices", path.to_str().expect("utf8")];
        args.extend(["--as-of", "2026-11-09"]);
        args.extend(samples());
        let out = run(&args);
        assert_eq!(out.status.code(), Some(0), "{date}: {}", text(&out.stderr));
        let stderr = text(&out.stderr);
        assert!(
            stderr.contains("warning: price data is ")
                && stderr.contains(&format!("(dated {date}")),
            "{date}: {stderr}"
        );
    }
}
