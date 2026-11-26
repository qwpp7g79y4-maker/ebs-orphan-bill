//! Asks git for the commit of this tree. A debug build prints it after the
//! version in `--version`. Nothing here invents a commit: no git, a failed
//! command, a tree that is not the top of its own repository (for example an
//! unpacked package inside another checkout), or a value that is not a full sha
//! leaves the version unchanged. A release build does not receive the commit.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rustc-check-cfg=cfg(ebs_orphan_bill_git_sha)");
    println!("cargo:rerun-if-changed=build.rs");
    track_git_head();
    if std::env::var("PROFILE").unwrap_or_default() != "debug" {
        return;
    }
    let Some(sha) = git_sha() else {
        return;
    };
    println!("cargo:rustc-cfg=ebs_orphan_bill_git_sha");
    println!("cargo:rustc-env=EBS_ORPHAN_BILL_GIT_SHA={sha}");
}

/// Rebuild when the commit changes: `.git/HEAD` names the branch, and the
/// branch file (or `packed-refs`) holds the commit.
fn track_git_head() {
    println!("cargo:rerun-if-changed=.git/HEAD");
    let head = manifest_dir().join(".git").join("HEAD");
    if let Ok(text) = std::fs::read_to_string(head) {
        if let Some(name) = text.trim().strip_prefix("ref:") {
            println!("cargo:rerun-if-changed=.git/{}", name.trim());
            println!("cargo:rerun-if-changed=.git/packed-refs");
        }
    }
}

fn manifest_dir() -> PathBuf {
    PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string()))
}

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(manifest_dir())
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8(output.stdout).ok()?.trim().to_string())
}

fn git_sha() -> Option<String> {
    // Only a checkout whose top directory is this crate may name its commit.
    let top = PathBuf::from(git(&["rev-parse", "--show-toplevel"])?);
    if top.canonicalize().ok()? != manifest_dir().canonicalize().ok()? {
        return None;
    }
    let sha = git(&["rev-parse", "--verify", "HEAD"])?;
    if sha.len() == 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Some(sha)
    } else {
        None
    }
}
