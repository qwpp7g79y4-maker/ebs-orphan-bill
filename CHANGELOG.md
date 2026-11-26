# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-11-19

Changes made after 2026-11-19; the version number did not change.

### Added

- `THIRD-PARTY-LICENSES.html`: the licenses of the third-party crates in the binary, generated with `cargo about generate --locked about.hbs`.
- Documentation standard: `README.md` with Installation, Quick start, Aan de slag in 5 minuten, Options, Example, Exit codes, Prices and responsibility, Maintenance, Support, and Contact sections; `README.nl.md`; `CONTRIBUTING.md`; `QUALITY.nl.md`; `about.hbs` and `about.toml` for the third-party license file of a release archive; `.github/workflows/release.yml`; `examples/README.md` with `examples/report.expected`. `SECURITY.md` and `QUALITY.md` are updated (the suite is 195 tests).
- `build.rs`: a debug build made in a git checkout prints the commit after the version in `--version`; a release build, a tree without git, or an unpacked package prints the version only.
- `"schema_version": 1` as the first key of every JSON report.
- `--output FILE` writes the report to a file through a temporary file and a rename; a symlink, a directory, or another non-regular target is refused (exit 2), a write error exits 2, and SIGINT removes the temporary file.
- SIGINT on Unix: exit 130, one line on standard error, nothing on standard output (new module `src/interrupt.rs`, tested with a real SIGINT).
- `--md` prints a Markdown report with the same amounts as text and JSON (pipes and line breaks in cells are escaped); with `--json` it is a usage error. `Options` has a new field `markdown`, and the library exports `render_markdown`.
- `--quiet` (no warnings) and `--verbose` (progress and one `context:` line on standard error); together they are a usage error (exit 2). `Options` has a new field `quiet`, and `Output` a new field `context`.
- `tests/real_shape.rs` with three fixtures in `tests/fixtures/` in the shape the AWS CLI reference documents for `describe-volumes`, `describe-snapshots`, and `describe-images` (optional fields left out, empty lists, several values, special characters in tags and names); the total is checked to the cent. These tests pass on the previous commit as well: they pin behaviour that was already right.
- A warning when the `date` of the price data is more than 180 days before `--as-of` (or before today without `--as-of`); the exit code stays 0. The binary also writes it to standard error in every mode except with `--quiet`, and it warns for a price date before 1970 too. `Options` has a new field `today_ms`, set only by the binary.

### Changed

- The monthly cost is labelled as an estimate. The text totals are `estimated_usd_month=21.50`, `volume estimated_usd_month=...` and `snapshot estimated_usd_month=...`, one line says the figure is an estimate from list prices and not the invoice, and the JSON has `"estimate": true` and `estimate_note`. The JSON totals `usd_month`, `volume_usd_month`, and `snapshot_usd_month` are now `estimated_usd_month`, `volume_estimated_usd_month`, and `snapshot_estimated_usd_month`. The per-row `usd_month` is unchanged.


### Added

- CI workflow on four systems with Rust stable (fmt, clippy, `cargo test --locked`, `cargo test --release --locked`) and on Rust 1.85, plus `cargo audit` and `cargo deny --locked check licenses` with a permissive-license list in `deny.toml`.
- `QUALITY.md` with the measured test result (162 passed, 0 ignored, 0 failed on Linux, in debug and in release, on stable and on Rust 1.85), a "Not run" list, known limits, and a table from each `README.md` claim to the test that covers it.
- `SECURITY.md` with the supported versions, what the tool does with its input, and how to report a problem.
- `.github/dependabot.yml`, and a 30-minute timeout on the CI test and MSRV jobs.
- Tests for an exact product and sum, for the `--fail-usd` length and zero rules, for `VolumeType` case, and for control characters in the price `source`.
- Tests for the 256 MiB input limit (a file, standard input, and gzip, each at the limit and one byte over it), for a symlink named on the command line, and for the split of warnings between standard error and the JSON object.
- The library also exports `escape_hidden`, `is_hidden`, and `loses_digits`.
- `man/ebs-wees-bill.1`, a man page that holds the `--help` text and the same long options. `tests/handbook_help.rs` checks that.
- `tests/docs_clean.rs` (22 patterns) fails when `README.md`, `QUALITY.md`, `CHANGELOG.md`, `SECURITY.md`, or `man/` holds a local absolute path, a wrapper option, the name of a working-notes file, an internal tool name, or the name of an AI system. A second test scans the manifest, the workflow files, `scripts/`, `src/`, `tests/`, `examples/`, and `prices/` for the same words. The test source writes the words in pieces, so it does not hold them.
- `scripts/check-glibc.sh` and a CI job `glibc` (Ubuntu 22.04) that fails when the release binary needs a glibc symbol newer than 2.34.
- Tests for the limit on JSON values (`a_document_with_too_many_json_values_is_refused`, a unit test of the count) and for a single object with a `VolumeId` and an `ImageId`.
- `.gitattributes` sets `* text=auto eol=lf`, so the man page, the examples, and the price file keep their bytes in a CRLF checkout. `man/` and `SECURITY.md` are in the package.

### Changed

- `--json` writes the report straight into one string instead of building a tree of values first; the output is byte for byte the same. At 2320000 volumes it took 9.67 s and 2.4 GiB, where it took 20.87 s and 10.8 GiB; with two warnings per volume (1800000 volumes) 17.00 s and 3.2 GiB, where it took 27.48 s and 9.7 GiB. The run also frees the input bytes before it renders and does not copy the warnings. `README.md` and `QUALITY.md` hold the measurements.
- The documented exact amount for 1001 GB at a rate of 0.4106043956043956043956043956 is 411.0149999999999999999999999956 (28 decimals); the text had one 9 too many.
- A document may hold at most 16777216 JSON values plus object keys; a document or a price file with more exits 2 with `too many JSON values (at most 16777216)` before the tree is built. Before, only the 256 MiB size limit applied: arrays nested 120 deep took about 150 times the file size in memory (a 64 MiB file about 10 GB, and a file at the size limit tens of GB), and a 33554290-byte file of that shape now takes 6 s and 4.8 GiB at most. A very large realistic export (about 400000 volumes with three tags each, or more) is refused and has to be split.
- A single object that holds both a `VolumeId` and an `ImageId` is a mixed document (exit 2), as the same object inside an array already was. It used to be read as a volume with exit 0.
- A row and the report total are the exact sum of the unrounded products, rounded once, half away from zero. `--fail-usd` compares that exact amount. `Decimal` used to round a product that needs more than 28 digits, without a message: a size of 18446744073709551615 GB at a rate of 123456789.41249 showed `...180.70` instead of `...180.67`, and two volumes at a rate of 500000000000000000000000000.01 showed a total of `1000000000000000000000000000.00`. Such a run exits 2 with `amount overflowed` when the exact amount does not fit in the accumulator, or when the once-rounded cent amount does not fit in a decimal. A sum of already rounded products used to land on the wrong cent: two volumes of 1000000000000003 GB at a rate of 20.05736282578990000041152263 showed `40114725651579920.35` and now show `40114725651579920.34`. 1001 GB at a rate of 0.4106043956043956043956043956 (exactly 411.0149999999999999999999999956) used to show 411.02 and then to exit 2; it now shows 411.01. GB from a size in bytes is rounded once, half away from zero. This changes the output for a size in bytes whose exact GB value ends in a tie at the 28th decimal: `Decimal` division used to round such a tie to the even digit, so 2 bytes showed 0.0000000018626451492309570312 and now shows 0.0000000018626451492309570313, and 10 bytes showed 0.0000000093132257461547851562 and now shows 0.0000000093132257461547851563. 1 byte is not a tie and still shows 0.0000000009313225746154785156. Monthly amounts in the library are an exact `Amount`; `format_usd` rounds one.
- `--help` says that exit 2 also covers a standard output that cannot be written. The man page holds the same text.
- Version 1.0.0. The price file and the prices are the same as 0.1.0. The input and output checks below changed.
- A closed standard output (`>&-`) and a full one (`>/dev/full`) exit 2 with `ebs-wees-bill: could not write output`. `--help` and `--version` are written the same way, so they exit 2 on a closed or full standard output. `>/dev/null` and `1<>/dev/null` still exit 0. A closed standard output is recognised on Linux only (`src/stdout_probe.rs`).
- `--as-of` must be 1970-01-01 through 2199-12-31, or the command exits 2.
- `--fail-usd` with more than 28 decimals exits 2. A rate in a `--prices` file with more than 28 decimals is an error ("does not fit"). The same holds for an amount or a rate with more digits than a decimal holds exactly: `--fail-usd 1844674407370955161.49999999999` used to be rounded to `1844674407370955161.5`, so a row of exactly that amount passed, and now exits 2. Leading and trailing zeros do not count, and the largest rate, 79228162514264337593543950335, still works.
- Only space, tab, CR, and LF are trimmed around numbers, `VolumeType`, `StorageTier`, and `State`, and in price strings. A no-break space, U+2028, or U+3000 is not trimmed, so such a value is rejected or not recognised. A file that holds only Unicode whitespace is not an empty file and is an error.
- A gzip input is one member. Bytes after the first member, including a second member, are an error.
- A snapshot in state `pending` or `error` is skipped with a warning before its age is checked, so a pending snapshot that starts after `--as-of` is skipped instead of failing with "StartTime is after --as-of".
- The price `source` is shown with invisible and direction-changing characters as `\u{..}` escapes, in text and in `--json`; a control character in it is refused with exit 2 (`bad source`). In a usage error, every argument is escaped before clap builds the message, so an argument with an ESC sequence, a lone ESC, an OSC sequence, a carriage return, or a newline gives one `error:` message with `\u{..}` escapes and no loose line. This holds for a bad value (`--older-than-days`), an unknown option, and `--json=VALUE`. A file name with the Braille blank U+2800 or U+FFFC shows it escaped, and a file name that is not UTF-8 shows its bytes escaped.
- One list of hidden characters serves the price `source`, file names, and usage errors. In a usage error, U+2028, U+200B, U+2800, U+061C, and U+00AD in an argument used to be shown as themselves. In the price `source`, U+E0100 through U+E01EF (variation selectors supplement) used to be shown as themselves. Both are escaped now. The list also holds every white-space character other than the plain space (for example U+00A0, U+1680, U+2003, U+3000), U+0600 through U+0605, U+06DD, U+070F, U+08E2, U+1D173 through U+1D17A, and U+FFFC. A usage error and the price `source` used to show those as themselves.
- A usage error for an argument that is not UTF-8, when another argument holds a hidden character, says `invalid UTF-8 was detected in one or more arguments`. It said `invalid arguments`.
- The `--as-of` error names the range: `invalid --as-of: expected YYYY-MM-DD, 1970-01-01 through 2199-12-31`.
- Table columns are aligned by characters, not bytes, and only ASCII whitespace is trimmed from the end of a line.
- `homepage` in `Cargo.toml` is `https://www.pepai.nl`.
- `README.md` says GB and age are Decimal values with at most 28 decimal places, not exact decimals. It also gives the `--fail-usd` rules for length (40 characters) and for zeros in the decimals, the measured memory for three input shapes and for several files, that the price `source` refuses a control character (exit 2) and escapes the other hidden ones, and that `VolumeType` is matched without regard to case.

This version is not published and has no git tag.

## [0.1.0] - 2026-11-09

### Added

- `ebs-wees-bill` reads local `aws ec2 describe-volumes`, `describe-snapshots`, and `describe-images` JSON and prices loose volumes and snapshots older than N days that no AMI references.
- Built-in US East (N. Virginia) rates from <https://aws.amazon.com/ebs/pricing/>, read on 2026-11-09. The page price file was published 2026-11-07. gp3 storage is 0.08 USD per GB-month, gp2 is 0.10, io1 and io2 storage are 0.125, io1 IOPS and io2 tier 1 are 0.065, io2 tier 2 is 0.0455, io2 tier 3 is 0.03185, st1 is 0.045, sc1 is 0.015, and standard snapshots are 0.05. gp3 throughput is 0.04 USD per MB/s-month, the 40.96 USD per GiBps-month meter divided by 1024.
- Magnetic standard volumes (0.05), snapshot archive storage (0.0125) and retrieval (0.03), and the account free tier are recorded and not applied.
- Text table and `--json`. `--fail-usd` exits 1 when a priced orphan's exact monthly USD is strictly greater. The report does not delete volumes or snapshots.

This version is not published and has no git tag.
