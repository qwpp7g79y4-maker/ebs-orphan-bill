//! SIGINT (Ctrl-C) ends the run with exit 130, a one-line message, and nothing
//! on standard output, so an empty stdout is never mistaken for a success.

#![cfg(unix)]

use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};

#[test]
fn sigint_while_reading_stdin_exits_130_with_empty_stdout() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ebs-orphan-bill"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["--verbose", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let mut stderr = BufReader::new(child.stderr.take().expect("stderr"));
    // The progress line proves the process is inside `main`, past the point
    // where the handler is installed, and about to block on standard input.
    let mut line = String::new();
    stderr.read_line(&mut line).expect("progress line");
    assert_eq!(line, "reading -\n");
    let status = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .expect("run kill");
    assert!(status.success());
    let mut rest = String::new();
    stderr.read_to_string(&mut rest).expect("stderr");
    let mut stdout = String::new();
    child
        .stdout
        .take()
        .expect("stdout")
        .read_to_string(&mut stdout)
        .expect("stdout");
    let status = child.wait().expect("wait");
    assert_eq!(status.code(), Some(130), "{rest}");
    assert!(stdout.is_empty(), "{stdout}");
    assert_eq!(rest, "ebs-orphan-bill: interrupted by SIGINT\n");
}

#[test]
fn sigint_keeps_an_existing_output_file_whole() {
    let dir = std::env::temp_dir().join(format!("ebs-orphan-bill-sigint-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    let target = dir.join("report.out");
    std::fs::write(&target, "previous report").expect("write");
    let mut child = Command::new(env!("CARGO_BIN_EXE_ebs-orphan-bill"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["--verbose", "--output", target.to_str().expect("utf8"), "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let mut stderr = BufReader::new(child.stderr.take().expect("stderr"));
    let mut line = String::new();
    stderr.read_line(&mut line).expect("progress line");
    let status = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .expect("run kill");
    assert!(status.success());
    let mut rest = String::new();
    stderr.read_to_string(&mut rest).expect("stderr");
    assert_eq!(child.wait().expect("wait").code(), Some(130), "{rest}");
    assert_eq!(
        std::fs::read_to_string(&target).expect("read"),
        "previous report"
    );
    let names: Vec<_> = std::fs::read_dir(&dir).expect("dir").collect();
    assert_eq!(names.len(), 1, "no temporary file is left");
    let _ = std::fs::remove_dir_all(&dir);
}
