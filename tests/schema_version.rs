//! Every JSON report starts with `"schema_version": 1`. The tool has no JSON
//! error path: a failing run prints nothing on standard output.

use std::io::Write;
use std::process::{Command, Stdio};

fn run(args: &[&str], stdin: &[u8]) -> (i32, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ebs-orphan-bill"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(stdin)
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8(out.stdout).expect("utf-8"),
    )
}

fn assert_versioned(stdout: &str) {
    let value: serde_json::Value = serde_json::from_str(stdout).expect("json");
    assert_eq!(value["schema_version"], 1, "{stdout}");
    assert!(
        stdout.starts_with("{\n  \"schema_version\": 1,\n"),
        "schema_version comes first: {stdout}"
    );
}

#[test]
fn every_json_path_has_schema_version() {
    let samples = [
        "examples/volumes.json",
        "examples/snapshots.json",
        "examples/images.json",
    ];
    let cases: Vec<Vec<&str>> = vec![
        [&["--json", "--as-of", "2026-11-09"][..], &samples].concat(),
        [&["--json"][..], &samples].concat(),
        [
            &["--json", "--quiet", "--as-of", "2026-11-09"][..],
            &samples,
        ]
        .concat(),
        [
            &["--json", "--verbose", "--as-of", "2026-11-09"][..],
            &samples,
        ]
        .concat(),
        [
            &["--json", "--fail-usd", "1", "--as-of", "2026-11-09"][..],
            &samples,
        ]
        .concat(),
        vec!["--json", "tests/fixtures/describe-volumes.json"],
        vec!["--json", "-"],
    ];
    for args in &cases {
        let (code, stdout) = run(args, br#"{"Volumes":[]}"#);
        assert!(code == 0 || code == 1, "{args:?}: {code}");
        assert_versioned(&stdout);
    }
    let (_, empty) = run(&["--json", "-"], br#"{"Snapshots":[]}"#);
    assert_versioned(&empty);
}

#[test]
fn a_failing_json_run_prints_no_json() {
    for args in [
        vec!["--json", "no/such/file.json"],
        vec!["--json", "--as-of", "2026-13-40", "examples/volumes.json"],
        vec!["--json", "--md", "examples/volumes.json"],
    ] {
        let (code, stdout) = run(&args, b"");
        assert_eq!(code, 2, "{args:?}");
        assert!(stdout.is_empty(), "{args:?}: {stdout}");
    }
}
