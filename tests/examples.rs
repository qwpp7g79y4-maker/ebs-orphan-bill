//! The files in `examples/` and the files that ship the release stay in line
//! with the tool and with each other.

use std::process::Command;

#[test]
fn the_example_command_prints_exactly_the_expected_report() {
    let out = Command::new(env!("CARGO_BIN_EXE_ebs-orphan-bill"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "--as-of",
            "2026-11-09",
            "--older-than-days",
            "30",
            "examples/volumes.json",
            "examples/snapshots.json",
            "examples/images.json",
        ])
        .output()
        .expect("run binary");
    assert_eq!(out.status.code(), Some(0));
    let expected = include_str!("../examples/report.expected").replace("\r\n", "\n");
    assert_eq!(String::from_utf8(out.stdout).expect("utf-8"), expected);
    assert!(expected.contains("estimated_usd_month=21.50"));
    let readme = include_str!("../examples/README.md");
    assert!(readme.contains("report.expected") && readme.contains("21.50"));
}

fn quoted_list(text: &str, key: &str) -> Vec<String> {
    let start = text.find(key).unwrap_or_else(|| panic!("{key} missing"));
    let rest = &text[start..];
    let open = rest.find('[').expect("open bracket");
    let close = rest.find(']').expect("close bracket");
    rest[open + 1..close]
        .lines()
        .map(|line| {
            line.trim()
                .trim_end_matches(',')
                .trim_matches('"')
                .to_string()
        })
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

#[test]
fn the_license_listing_accepts_what_deny_toml_allows() {
    let about = include_str!("../about.toml");
    let deny = include_str!("../deny.toml");
    assert_eq!(quoted_list(about, "accepted"), quoted_list(deny, "allow"));
    let template = include_str!("../about.hbs");
    for needle in ["{{#each licenses}}", "{{text}}", "{{#each overview}}"] {
        assert!(template.contains(needle), "{needle} missing from about.hbs");
    }
}

#[test]
fn the_release_workflow_builds_this_binary_from_a_tag_after_ci() {
    let release = include_str!("../.github/workflows/release.yml");
    for needle in [
        "tags: [\"v*\"]",
        "uses: ./.github/workflows/ci.yml",
        "cargo about generate --locked about.hbs",
        "ebs-orphan-bill-${TAG}-${{ matrix.target }}",
        "check-glibc.sh",
        "THIRD-PARTY-LICENSES.html",
        "needs: [ci, build]",
    ] {
        assert!(
            release.contains(needle),
            "{needle} missing from release.yml"
        );
    }
    let ci = include_str!("../.github/workflows/ci.yml");
    assert!(ci.contains("workflow_call:"), "ci.yml must be callable");
}
