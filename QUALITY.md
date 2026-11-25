# Quality

Record of 2026-11-25. Measured on commit a483cd8cf9e0e4e653cd9d57914f4412ee77a493 ("Do not price creating, deleting, deleted or error volumes as a full month of orphan cost"), in a clean worktree. This commit adds documentation only; the measurements below apply to code commit a483cd8.

Toolchain: stable rustc 1.99.0 / cargo 1.99.0, Linux x86_64. No tag, no publish, no push. Only the four commands in the table below were run. Everything else is marked "not run".

## Commands

| Command | Exit |
| --- | ---: |
| `cargo fmt --check` | 0 |
| `cargo clippy --all-targets --locked -- -D warnings` | 0 |
| `cargo test --locked` (debug) | 0 (198 passed, 0 failed, 0 ignored) |
| `cargo test --release --locked` | 0 (198 passed, 0 failed, 0 ignored) |

Measured on 2026-11-25 with toolchain 1.85.1 (`rustc 1.85.1`, `cargo +1.85`, the declared minimum) on code commit a483cd8: `cargo +1.85 fmt --check` exit 0; `cargo +1.85 clippy --all-targets --locked -- -D warnings` exit 0; `cargo +1.85 test --locked` exit 0 (198 passed, 0 failed, 0 ignored); `cargo +1.85 test --release --locked` exit 0 (198 passed, 0 failed, 0 ignored).

Not run in this measurement: clippy for the Windows and macOS targets, `cargo package` and the test run in the unpacked crate, `cargo deny`, `cargo about`, `scripts/check-glibc.sh`, `cargo audit`, and the run of the binary against a closed or full standard output. These are not run, so this file makes no claim about them.

## Test result per target

Counts from the debug run (`cargo test --locked`). The release run gave a total of 198 passed, 0 failed, 0 ignored.

| Target | Passed | Failed | Ignored |
| --- | ---: | ---: | ---: |
| unittests `src/lib.rs` | 86 | 0 | 0 |
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
| total | 198 | 0 | 0 |
