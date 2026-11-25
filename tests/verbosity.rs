//! `--quiet` and `--verbose`: quiet drops the warnings, verbose adds progress on
//! standard error, the result on standard output never changes, and both
//! together are a usage error (exit 2, like every other usage error here).

use std::process::{Command, Stdio};

const SAMPLES: [&str; 3] = [
    "examples/volumes.json",
    "examples/snapshots.json",
    "examples/images.json",
];

fn run(extra: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ebs-wees-bill"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["--as-of", "2026-11-09"])
        .args(extra)
        .args(SAMPLES)
        .stdin(Stdio::null())
        .output()
        .expect("run binary");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn help_lists_quiet_and_verbose() {
    let output = Command::new(env!("CARGO_BIN_EXE_ebs-wees-bill"))
        .arg("--help")
        .output()
        .expect("run binary");
    let help = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        help.contains("--quiet") && help.contains("--verbose"),
        "{help}"
    );
}

#[test]
fn quiet_drops_warnings_and_keeps_the_result_and_exit_code() {
    let (code, plain, _) = run(&[]);
    assert_eq!(code, 0);
    assert!(plain.contains("warning: "), "{plain}");
    let (qcode, quiet, qerr) = run(&["--quiet"]);
    assert_eq!(qcode, 0, "{qerr}");
    assert!(!quiet.contains("warning:"), "{quiet}");
    assert!(qerr.is_empty(), "{qerr}");
    let mut without_warnings = String::new();
    for line in plain.lines().filter(|line| !line.starts_with("warning: ")) {
        without_warnings.push_str(line);
        without_warnings.push('\n');
    }
    assert_eq!(quiet, without_warnings);
    let (jcode, _, jerr) = run(&["--quiet", "--json"]);
    assert_eq!(jcode, 0);
    assert!(jerr.is_empty(), "{jerr}");
    let (fcode, _, _) = run(&["--quiet", "--fail-usd", "15.99"]);
    assert_eq!(fcode, 1, "quiet does not hide the threshold exit");
}

#[test]
fn verbose_adds_progress_on_stderr_only() {
    let (code, plain, plain_err) = run(&[]);
    assert_eq!(code, 0);
    assert!(plain_err.is_empty(), "{plain_err}");
    let (vcode, verbose, verr) = run(&["--verbose"]);
    assert_eq!(vcode, 0, "{verr}");
    assert_eq!(verbose, plain, "verbose does not change the report");
    assert!(verr.contains("reading examples/volumes.json\n"), "{verr}");
    assert!(
        verr.trim_end()
            .ends_with("context: orphan volumes 3, old snapshots 2, warnings 3"),
        "{verr}"
    );
}

#[test]
fn quiet_with_verbose_is_a_usage_error() {
    for args in [["--quiet", "--verbose"], ["--verbose", "--quiet"]] {
        let (code, stdout, stderr) = run(&args);
        assert_eq!(code, 2, "{stderr}");
        assert!(stdout.is_empty(), "{stdout}");
        assert!(stderr.contains("cannot be used with"), "{stderr}");
    }
}
