//! `--version` prints the crate version. A debug build made in a git checkout
//! adds the commit; a release build, or a tree without git (an unpacked
//! package), prints the version only.

use std::path::Path;
use std::process::Command;

#[test]
fn version_is_the_crate_version_with_the_commit_only_in_a_debug_checkout() {
    let out = Command::new(env!("CARGO_BIN_EXE_ebs-orphan-bill"))
        .arg("--version")
        .output()
        .expect("run binary");
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8(out.stdout).expect("utf-8");
    let line = text.trim_end();
    let base = format!("ebs-orphan-bill {}", env!("CARGO_PKG_VERSION"));
    assert!(line.starts_with(&base), "{line}");
    let rest = &line[base.len()..];
    let in_checkout = Path::new(env!("CARGO_MANIFEST_DIR")).join(".git").exists();
    if cfg!(debug_assertions) && in_checkout {
        let sha = rest
            .strip_prefix(" (")
            .and_then(|r| r.strip_suffix(')'))
            .unwrap_or_else(|| panic!("no commit after the version: {line}"));
        assert_eq!(sha.len(), 40, "{line}");
        assert!(sha.bytes().all(|b| b.is_ascii_hexdigit()), "{line}");
        let head = Command::new("git")
            .args(["rev-parse", "--verify", "HEAD"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output();
        if let Some(head) = head.ok().filter(|o| o.status.success()) {
            assert_eq!(
                String::from_utf8_lossy(&head.stdout).trim(),
                sha,
                "stale commit"
            );
        }
    } else if !cfg!(debug_assertions) {
        assert_eq!(rest, "", "a release build prints the version only");
    } else {
        assert!(rest.is_empty() || rest.starts_with(" ("), "{line}");
    }
}
