# Security policy

## Supported versions

| Version | Supported |
| --- | --- |
| 1.0.x | Yes |

Security fixes go into the latest release only. The crate is not published and has no git tag.

## What ebs-orphan-bill does

It reads the files and arguments it is given, treats all of them as untrusted, and writes its report to standard output and its messages to standard error, or to one file when `--output` is given (a temporary file in the same directory, renamed over the target; a symlink or a non-regular target is refused). The code in `src/` has no call that starts a program or opens a network connection, and writes a file only for `--output`. A text scan of `src/` and the dependency list check the first two in part (see `QUALITY.md`). Only the build script `build.rs` starts a program (`git`, at build time, to name the commit in a debug build). The input limit is 268435456 bytes per file, also after decompression, and a document may hold at most 16777216 JSON values plus object keys. Memory use is a multiple of the file size and, for one document, is bounded at about 5 GiB by that value limit (`--json` holds its output once and stays within the same bound, measured in `README.md`); all files of one run are held together (see Known limits in `QUALITY.md`).

## Reporting a vulnerability

Please report security problems privately, not in a public issue: open the **Security** tab of this repository and choose **Report a vulnerability**. If that is not possible, e-mail pep_ai@icloud.com. Examples are a file name, JSON string, or price source that can add a fake line or a terminal escape to the report or to an error, an error that echoes a value it should hide, a way to make ebs-orphan-bill write, run, or connect to something, a file that makes it read without the stated size limit or hang, and input that changes an amount without an error.

Memory use that grows with the size of a file or of a directory, within the measured bounds of the README, is a documented limit (see the README, Limitations), not a vulnerability by itself.

Please include:

- the ebs-orphan-bill version (the `version` field in `Cargo.toml`, or the commit you built) and your operating system;
- the exact command;
- minimal synthetic `describe-volumes`, `describe-snapshots`, or `describe-images` JSON that reproduces the problem, never a real account export;
- what ebs-orphan-bill printed and what you expected.

We aim to respond within 7 days (best effort). Once a fix is released, the report can be credited in the changelog if you wish.
