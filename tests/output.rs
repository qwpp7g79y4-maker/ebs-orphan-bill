//! `--output FILE`: the report goes to a file, written whole or not at all.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

const SAMPLES: [&str; 3] = [
    "examples/volumes.json",
    "examples/snapshots.json",
    "examples/images.json",
];

fn run(extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ebs-wees-bill"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["--as-of", "2026-11-09"])
        .args(extra)
        .args(SAMPLES)
        .output()
        .expect("run binary")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("utf-8")
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "ebs-wees-bill-out-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir(&path).expect("scratch dir");
        Self(path)
    }

    fn file(&self, name: &str) -> String {
        self.0.join(name).to_str().expect("utf8").to_string()
    }

    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(&self.0)
            .expect("read dir")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn the_report_goes_to_the_file_and_not_to_stdout() {
    for format in [None, Some("--json"), Some("--md")] {
        let dir = Scratch::new("write");
        let target = dir.file("report.out");
        let mut args = vec![];
        args.extend(format);
        let direct = run(&args);
        args.extend(["--output", &target]);
        let out = run(&args);
        assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
        assert!(out.stdout.is_empty(), "{}", text(&out.stdout));
        assert_eq!(fs::read(&target).expect("read"), direct.stdout);
        assert_eq!(dir.names(), ["report.out"], "no temporary file is left");
    }
}

#[test]
fn an_existing_file_is_replaced_and_the_threshold_exit_stays() {
    let dir = Scratch::new("replace");
    let target = dir.file("report.out");
    fs::write(&target, "old contents that are longer than nothing").expect("write");
    let out = run(&["--fail-usd", "15.99", "--output", &target]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
    let body = fs::read_to_string(&target).expect("read");
    assert!(body.contains("estimated_usd_month=21.50"), "{body}");
    assert!(!body.contains("old contents"));
}

#[test]
fn a_missing_parent_directory_is_exit_2_and_is_not_created() {
    let dir = Scratch::new("missing");
    let target = dir.file("no-such-dir/report.out");
    let out = run(&["--output", &target]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    let stderr = text(&out.stderr);
    assert!(stderr.contains("cannot write"), "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
    assert!(dir.names().is_empty());
}

#[test]
fn a_directory_or_a_device_as_target_is_refused() {
    let dir = Scratch::new("dir");
    let target = dir.file("sub");
    fs::create_dir(&target).expect("sub");
    let out = run(&["--output", &target]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        text(&out.stderr).contains("not a regular file"),
        "{}",
        text(&out.stderr)
    );
    assert_eq!(dir.names(), ["sub"]);
    let out = run(&["--output", "/dev/null"]);
    assert_eq!(out.status.code(), Some(2));
}

#[cfg(unix)]
#[test]
fn a_symlink_as_target_is_refused_and_not_followed() {
    let dir = Scratch::new("symlink");
    let real = dir.file("real.txt");
    fs::write(&real, "keep me").expect("write");
    let link = dir.file("link.out");
    std::os::unix::fs::symlink(&real, &link).expect("symlink");
    let out = run(&["--output", &link]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        text(&out.stderr).contains("symlink"),
        "{}",
        text(&out.stderr)
    );
    assert_eq!(fs::read_to_string(&real).expect("read"), "keep me");
    assert_eq!(dir.names(), ["link.out", "real.txt"]);
}

#[cfg(unix)]
#[test]
fn a_directory_that_cannot_be_written_is_exit_2() {
    use std::os::unix::fs::PermissionsExt;
    let dir = Scratch::new("readonly");
    let sub = dir.0.join("ro");
    fs::create_dir(&sub).expect("sub");
    fs::set_permissions(&sub, fs::Permissions::from_mode(0o555)).expect("chmod");
    let target = sub.join("report.out");
    let out = run(&["--output", target.to_str().expect("utf8")]);
    let writable = fs::write(sub.join("probe"), "x").is_ok();
    fs::set_permissions(&sub, fs::Permissions::from_mode(0o755)).expect("chmod back");
    if writable {
        return; // running as a user that ignores directory permissions
    }
    assert_eq!(out.status.code(), Some(2));
    assert!(
        text(&out.stderr).contains("cannot write"),
        "{}",
        text(&out.stderr)
    );
    assert!(!text(&out.stderr).contains("panicked"));
}
