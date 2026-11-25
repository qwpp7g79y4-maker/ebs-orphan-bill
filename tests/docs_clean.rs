//! The published files must not carry internal names, local paths, or tool prefixes.
//!
//! The words are written in pieces, so that this file does not contain them either.

use std::fs;
use std::path::{Path, PathBuf};

/// Lower-case patterns that must not appear in a published file.
const FORBIDDEN: [&str; 22] = [
    concat!("/me", "dia/"),
    concat!("/ho", "me/"),
    concat!("~", "/"),
    concat!("/us", "ers/"),
    concat!("c:", "\\"),
    concat!("--max", "-gb"),
    concat!("--na", "am"),
    concat!("ta", "sk.md"),
    concat!("blue", "print.md"),
    concat!("geheugen", "wachter"),
    concat!("bouw", "-tmp"),
    concat!("gr", "ok"),
    concat!("cla", "ude"),
    concat!("anthr", "opic"),
    concat!("open", "ai"),
    concat!("co", "dex"),
    concat!("brug", "-prompts"),
    concat!("son", "net"),
    concat!("op", "us"),
    concat!("open", "bot"),
    concat!("co-auth", "ored"),
    concat!("gem", "ini"),
];

fn collect(path: &Path, out: &mut Vec<PathBuf>) {
    if !path.exists() {
        return;
    }
    if path.is_dir() {
        let mut entries: Vec<PathBuf> = fs::read_dir(path)
            .unwrap_or_else(|e| panic!("read dir {}: {e}", path.display()))
            .map(|entry| entry.expect("entry").path())
            .collect();
        entries.sort();
        for entry in entries {
            collect(&entry, out);
        }
    } else {
        out.push(path.to_path_buf());
    }
}

/// The patterns found in `text`, each with its 1-based line number. Case does not matter.
fn hits(text: &str) -> Vec<(usize, &'static str)> {
    let mut found = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let lower = line.to_lowercase();
        for word in FORBIDDEN {
            if lower.contains(word) {
                found.push((index + 1, word));
            }
        }
    }
    found
}

fn problems_in(files: &[PathBuf]) -> Vec<String> {
    let mut problems = Vec::new();
    for file in files {
        if !file.is_file() {
            continue;
        }
        let text =
            fs::read_to_string(file).unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
        assert!(!text.trim().is_empty(), "{} is empty", file.display());
        for (line, word) in hits(&text) {
            problems.push(format!("{}:{line}: {word}", file.display()));
        }
    }
    problems
}

#[test]
fn public_docs_have_no_local_paths_wrapper_options_or_internal_names() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files: Vec<PathBuf> = [
        "README.md",
        "README.nl.md",
        "QUALITY.md",
        "QUALITY.nl.md",
        "CHANGELOG.md",
        "SECURITY.md",
        "CONTRIBUTING.md",
        "THIRD-PARTY-LICENSES.html",
    ]
    .iter()
    .map(|name| root.join(name))
    .collect();
    collect(&root.join("man"), &mut files);
    assert!(files.len() >= 5, "man/ holds no page: {files:?}");
    let problems = problems_in(&files);
    assert!(
        problems.is_empty(),
        "internal text in public files:\n{}",
        problems.join("\n")
    );
}

/// The ignore file is left out: it lists the two local working files by name, on purpose.
#[test]
fn package_metadata_workflows_sources_and_tests_have_no_internal_names_either() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files: Vec<PathBuf> = ["Cargo.toml", "deny.toml", ".gitattributes"]
        .iter()
        .map(|name| root.join(name))
        .collect();
    for dir in [".github", "scripts", "src", "tests", "examples", "prices"] {
        collect(&root.join(dir), &mut files);
    }
    assert!(files.len() >= 20, "too few files found: {files:?}");
    let problems = problems_in(&files);
    assert!(
        problems.is_empty(),
        "internal text in package files:\n{}",
        problems.join("\n")
    );
}

#[test]
fn a_package_tree_without_git_metadata_is_clean() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let absent_attr = root.join("src").join(".gitattributes");
    let absent_meta = root.join("src").join(".github");
    assert!(!absent_attr.exists(), "{}", absent_attr.display());
    assert!(!absent_meta.exists(), "{}", absent_meta.display());
    let mut files = vec![root.join("Cargo.toml"), root.join("deny.toml"), absent_attr];
    collect(&absent_meta, &mut files);
    collect(&root.join("src"), &mut files);
    let problems = problems_in(&files);
    assert!(problems.is_empty(), "{problems:?}");
    assert!(
        files.iter().any(|path| path.ends_with("lib.rs")),
        "sources were not read: {files:?}"
    );
}

#[test]
fn the_check_itself_finds_each_forbidden_pattern() {
    for word in FORBIDDEN {
        let text = format!("fine line\nbefore {word} after\n");
        assert_eq!(hits(&text), vec![(2, word)], "{word}");
        let upper = format!("fine line\nbefore {} after\n", word.to_uppercase());
        assert_eq!(hits(&upper), vec![(2, word)], "{word} in capitals");
    }
    assert!(hits("plain text, a `cargo test` command, a/b/c.json").is_empty());
}
