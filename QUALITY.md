# Quality

Record of 2026-11-25. Measured on commit 8c0b33a5d0e9fa23be0bfd88661d02d108d7ca62 (`origin/main`, "Set bin_name so the usage line has no .exe on Windows"), in a clean worktree. This commit adds documentation only; the measurements below apply to code commit 8c0b33a.

Toolchain: stable rustc 1.99.0 / cargo 1.99.0, Linux x86_64. No tag, no publish, no push. Only the four commands in the table below were run. Everything else is marked "not run".

## Commands

| Command | Exit |
| --- | ---: |
| `cargo fmt --check` | 0 |
| `cargo clippy --all-targets --locked -- -D warnings` | 0 |
| `cargo test --locked` (debug) | 0 (197 passed, 0 failed, 0 ignored) |
| `cargo test --release --locked` | 0 (197 passed, 0 failed, 0 ignored) |

Not run in this measurement: `cargo +1.85` (declared minimum) clippy and tests, clippy for the Windows and macOS targets, `cargo package` and the test run in the unpacked crate, `cargo deny`, `cargo about`, `scripts/check-glibc.sh`, `cargo audit`, and the run of the binary against a closed or full standard output. These are not run, so this file makes no claim about them.

## Test result per target

Counts from the debug run (`cargo test --locked`). The release run gave a total of 197 passed, 0 failed, 0 ignored.

| Target | Passed | Failed | Ignored |
| --- | ---: | ---: | ---: |
| unittests `src/lib.rs` | 85 | 0 | 0 |
| unittests `src/main.rs` | 0 | 0 | 0 |
| `tests/claims.rs` | 9 | 0 | 0 |
| `tests/cli.rs` | 71 | 0 | 0 |
| `tests/docs_clean.rs` | 4 | 0 | 0 |
| `tests/examples.rs` | 3 | 0 | 0 |
| `tests/handbook_help.rs` | 1 | 0 | 0 |
| `tests/json_peak.rs` | 1 | 0 | 0 |
| `tests/markdown.rs` | 4 | 0 | 0 |
| `tests/output.rs` | 6 | 0 | 0 |
| `tests/real_shape.rs` | 4 | 0 | 0 |
| `tests/schema_version.rs` | 2 | 0 | 0 |
| `tests/sigint.rs` | 2 | 0 | 0 |
| `tests/verbosity.rs` | 4 | 0 | 0 |
| `tests/version.rs` | 1 | 0 | 0 |
| doc-tests | 0 | 0 | 0 |
| total | 197 | 0 | 0 |
