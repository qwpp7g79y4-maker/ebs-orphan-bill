# Quality

This commit changes documentation only; the measurements below apply to code commit 6118c69.

Suite: 197 passed, 0 ignored, 0 failed (Linux, debug and release, stable and Rust 1.85).

Record of 2026-11-24. The test runs below ran on the commit 6118c69f99d0266b34ae0d36103e61dddd4dee92 (branch `poets-ebs`), the last commit that changes a test. The commits after it change documents only. Toolchains: stable rustc 1.99.0 / cargo 1.99.0, and rustc 1.85.1 / cargo 1.85.1 (the declared minimum). Linux x86_64. No tag, no publish. Only the four `cargo test` rows and the test counts are measurements of that commit. Every other row of the Commands table (format, clippy, package, licenses, about, glibc, audit) was not run on it; those last ran on 1815bb6 on 2026-11-20 and are marked "not run on 6118c69". The cross-compile rows (Windows and macOS targets) were not run either and are listed under "Not run".

Package check: not run on 6118c69. On 1815bb6 (2026-11-20) `cargo package --offline --allow-dirty --no-verify` listed 60 files and `cargo test --offline` in the unpacked crate gave 195 passed, 0 failed.

## Commands

Every command ran with `-j 2` and, for tests, `--test-threads 2`.

| Command | Exit |
| --- | ---: |
| `cargo fmt --check` | not run on 6118c69 |
| `cargo clippy --offline --all-targets --target x86_64-pc-windows-gnu -- -D warnings` | not run on 6118c69 |
| `cargo clippy --offline --all-targets --target x86_64-apple-darwin -- -D warnings` | not run on 6118c69 |
| `cargo clippy --locked --offline --all-targets -- -D warnings` (stable) | not run on 6118c69 |
| `cargo +1.85 clippy --locked --offline --all-targets -- -D warnings` | not run on 6118c69 |
| `cargo test --locked --offline` (debug) | 0 |
| `cargo test --locked --offline --release` | 0 |
| `cargo +1.85 test --locked --offline` (debug) | 0 |
| `cargo +1.85 test --locked --offline --release` | 0 |
| `cargo package --offline --allow-dirty --no-verify`, then `cargo test --offline` in the unpacked crate | not run on 6118c69 |
| `cargo deny --locked check licenses` | not run on 6118c69 |
| `cargo about generate --locked --offline about.hbs` (22 license sections) | not run on 6118c69 |
| `bash scripts/check-glibc.sh target/release/ebs-wees-bill 2.34` (and `2.30`, which must fail) | not run on 6118c69 |
| `cargo audit --no-fetch` (local advisory database) | not run on 6118c69 |


## Test result per target

`cargo test --locked --offline -- --test-threads 2` in debug and in release, and the same on Rust 1.85 in debug and in release, give the same counts.

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

Debug: 197 passed, 0 failed, 0 ignored. Release: 197 passed, 0 failed, 0 ignored. Rust 1.85 debug: 197 passed, 0 failed, 0 ignored. Rust 1.85 release: 197 passed, 0 failed, 0 ignored. Counted on 2026-11-24 from `cargo test --locked --offline -j 2 -- --test-threads 2` and the same three other runs.

Gating: in `tests/cli.rs` 4 tests are `#[cfg(unix)]` (symlinks, FIFOs) and 6 are `#[cfg(target_os = "linux")]` (closed and full standard output, a file name with a control character, two file names that are not UTF-8, and usage errors for arguments that are not UTF-8). `tests/sigint.rs` (2 tests) is `#[cfg(unix)]`, and 2 tests of `tests/output.rs` (a symlink target, a directory that cannot be written) are `#[cfg(unix)]`. By that gating a Unix system that is not Linux runs 189 and a system with neither runs 181. Both were measured on 1815bb6 on 2026-11-20 (195 tests then) and not counted again on 6118c69 (two tests were added since, which this gating does not touch) by building a copy with the gating changed (`target_os = "linux"` set to a name no system has, then also `cfg(unix)` set to `cfg(windows)`) and counting `cargo test -- --list`; the tests were not run on those systems.

## Standard output

`src/stdout_probe.rs` is a module of the binary (`src/main.rs`, `mod stdout_probe;`). It is one of two modules of the binary with unsafe code; the other is `src/interrupt.rs`, which registers the SIGINT handler on Unix (the handler calls only `write`, `_exit`, and atomics). Rust replaces a closed fd 0, 1, or 2 with `/dev/null` before `main`, so a closed standard output can no longer be told from `1<>/dev/null`. On Linux the probe is a constructor in `.init_array` that calls `fcntl(1, F_GETFD)` before that replacement and records whether fd 1 was already closed at process start. It does not use `/proc`. The binary then exits 2 with `ebs-wees-bill: could not write output`. On a platform other than Linux the probe never reports a closed stdout.

Measured on 2026-11-19 (one script, run once per binary) with the release binary and with the debug binary, valid input `--as-of 2026-11-09 examples/volumes.json examples/snapshots.json examples/images.json`. Both binaries gave the same result on every line:

| Case | Exit | Standard error |
| --- | ---: | --- |
| `>&-` (closed) | 2 | `ebs-wees-bill: could not write output` |
| `>/dev/full` | 2 | `ebs-wees-bill: could not write output` |
| `--help >&-` | 2 | `ebs-wees-bill: could not write output` |
| `--version >&-` | 2 | `ebs-wees-bill: could not write output` |
| `--help >/dev/full` | 2 | `ebs-wees-bill: could not write output` |
| `--version >/dev/full` | 2 | `ebs-wees-bill: could not write output` |
| `>/dev/null` | 0 | empty |
| `1<>/dev/null` | 0 | empty |
| stdout to Python `subprocess.DEVNULL` | 0 | empty |
| `\| true` | 2 | `ebs-wees-bill: could not write output` |
| `--help \| true` | 2 | `ebs-wees-bill: could not write output` |
| `--help 1<>/dev/null` | 0 | empty |
| `>/dev/null` with an empty `/proc` (new mount namespace, tmpfs on `/proc`) | 0 | empty |
| `>&-` with an empty `/proc` | 2 | `ebs-wees-bill: could not write output` |
| output to a file with an empty `/proc` | 0 | empty, 2399 bytes written |

No panic in any case.

## Usage errors

Measured with the release binary. An argument with an ESC sequence and a newline is escaped before clap builds the message, so the message stays on clap's own lines:

- `--older-than-days $'1\x1b[31m\nFAKE: line'` gives `error: invalid value '1\u{1b}[31m\u{a}FAKE: line' for '--older-than-days <DAYS>': invalid digit found in string`, a blank line, and `For more information, try '--help'.` Exit 2, no raw ESC, CR, or newline from the argument.
- `--as-of $'\xff' $'f\x1b.json'` (an argument that is not UTF-8 next to one with an ESC) gives `error: invalid UTF-8 was detected in one or more arguments`, the usage, and the `--help` hint. Exit 2.
- `--older-than-days $'1\xc2\xa0X'` (a no-break space) gives `error: invalid value '1\u{a0}X' for ...`. Exit 2.
- `$'-\x1b[31m\nFAKE'` (a short option that holds a hidden character) gives `error: unexpected argument '-\' found`: clap shows only the first character of the escaped text. No loose line, no raw ESC. Exit 2.
- `$'--x\x1b[31m\nFAKE: line'` gives `error: unexpected argument '--x\u{1b}[31m\u{a}FAKE: line' found` and the `tip:` line with the same escaped text. Exit 2.

## Not run

- A fuzz campaign.
- GitHub CI and the release workflow (`ci.yml` and `release.yml`): not run, because the GitHub account has a payment problem and no tag exists. This covers the CI job `glibc` (`scripts/check-glibc.sh` was run on this machine), the smoke test of `--version` in a release archive, and GitHub Actions in general. `about.hbs` and `about.toml` were run with `cargo about` on this machine only.
- A fuzz directory: this crate has none.
- Tests on Windows and macOS, and `cargo clippy` for `aarch64-apple-darwin`: not run. Clippy for `x86_64-pc-windows-gnu` and `x86_64-apple-darwin` ran (see Commands); they check that the code compiles and lints there, not that the tests pass. The SIGINT handler (`src/interrupt.rs`) is `#[cfg(unix)]`, so Windows builds without it.
- A mutation run.
- macOS and Windows: compiled and linted only (rows above). No test ran there, so the behaviour of a closed standard output on those systems is not measured.
- `cargo audit` with a fresh advisory database (`--no-fetch` was used).
- The pricing page was not read again: the rates are those read on 2026-11-09.
- `cargo install --locked --path .` (R28).
- A libc other than glibc (for example musl), and the oldest macOS or Windows version that works.
- `git commit`, `git push`, `git tag`, and `cargo publish`.

## Known limits

- This page records checks. It is not a guarantee and it is not a fuzz report.
- "No network call" and "reads only" are checked by `source_does_not_spawn_aws_or_link_an_sdk` (a text scan of `src/` for starting a program and for network code) and by the dependency list. No test watches the system calls. The only file the tool writes is the `--output` target (and its temporary file), covered by `tests/output.rs`; no test scans `src/` for other file writes.
- Closed standard output is detected on Linux only (see above).
- Memory use is a multiple of the input size, and the multiple depends on the shape (release binary, `/usr/bin/time`, 2026-11-19). A document may hold at most 16777216 JSON values plus object keys; more is refused before the tree is built. Realistic export, 64652 loose volumes with three tags each, 33554410 bytes: 1.01 s and 349160 KiB (about 10 times). 6000000 numbers in one array, 12000023 bytes: 0.86 s and 634232 KiB (about 54 times). 258189 such volumes, 134000113 bytes: 5.18 s and 1388328 KiB. At the item limit, about 16.7 million items: one array of numbers (33554442 bytes) 2.80 s and 1771195 KiB; `{"a":1}` objects in an array (44739246 bytes) 6.22 s and 2928348 KiB; arrays nested 120 deep (33554290 bytes, the worst shape tried) 6.07 s and 5019844 KiB (about 150 times). So one document needs at most about 5 GiB. A file of 268000204 bytes with 516378 such volumes holds about 21.7 million items and is refused after 1.56 s and 525320 KiB. The files of one directory are read before any is parsed, so their bytes add up: 8 files of 16777735 bytes took 2.96 s and 381644 KiB (one such file: 175992 KiB). The limit on items is tested at its edge by a unit test (`values_and_keys_are_counted_against_the_maximum`) and one item over it through the binary (`a_document_with_too_many_json_values_is_refused`); the memory figures themselves are measured by hand, not by a test. `--json` is written straight into one string, so its memory is the output plus the same tree as text mode, not a second tree of values. Measured on Linux with the release binary (2026-11-20, one run each) on files of 580000, 1160000, and 2320000 volumes (33514733, 67029453, and 134058893 bytes, at most 16.2 million items): `--json` took 2.10 s, 4.11 s, and 9.67 s and 632728, 1263384, and 2523300 KiB (2.4 GiB for the largest; text mode took 2.11 s, 4.10 s, and 9.24 s and 651136, 1262636, and 2524368 KiB). The same file at 2320000 volumes took 20.87 s and 11293720 KiB (10.8 GiB) in `--json` mode before this change. With two warnings per volume (`io1` volumes `in-use` without `Iops`; files of 450000, 900000, and 1800000 volumes, 33652813, 67305613, and 134611213 bytes) `--json` took 4.33 s, 8.56 s, and 17.00 s and 843208, 1683972, and 3363948 KiB (3.2 GiB for the largest; text mode 2.48 s, 4.63 s, and 8.81 s and 754088, 1507436, and 3011280 KiB), where it took 27.48 s and 10170228 KiB (9.7 GiB) before. Time and memory double with the input in both modes. The `--json` time is above the text time when there are many warnings, because each is written to standard error and into the object. `json_report_peak_is_the_output_plus_a_small_constant` (`tests/json_peak.rs`) counts the heap while the `--json` report is built and fails when the peak is more than twice the output plus 4096 bytes (at 2000 volumes it was 7718398 bytes for 600299 bytes of output before this change); the times are not tested.
- The documented test total (here, in `README.md`, and in `CHANGELOG.md`) is not checked by a test. It was counted from the `cargo test` output.
- A row and the report total are the exact sum of the unrounded products, rounded once, half away from zero. `--fail-usd` compares that exact amount. If the exact amount does not fit in the accumulator, or the once-rounded cent amount does not fit in a decimal, the run exits 2 with `amount overflowed`. GB from a size in bytes is rounded once, half away from zero, to at most 28 decimal places (1 byte shows 0.0000000009313225746154785156, 2 bytes shows 0.0000000018626451492309570313). See `totals_are_the_exact_sum_of_unrounded_products_not_of_rounded_rows` (three rows and three snapshots at 0.102 each: rows 0.10, each class 0.31, total 0.61; three mutations that add rounded rows or rounded classes fail it), `a_total_of_rounded_products_uses_the_exact_sum`, `a_row_sums_exact_parts_that_meet_on_a_half_cent`, `fail_usd_compares_the_exact_amount`, `one_product_near_a_half_cent_rounds_once`, `gb_from_snapshot_bytes_rounds_half_away_from_zero`, `a_product_or_sum_that_would_lose_digits_is_an_error`, and `exact_math_refuses_what_does_not_fit`.
- `--fail-usd` reads at most 40 characters, and a zero at the end of the 29th decimal is refused (`0.50000000000000000000000000000`).
- The price file's `source` refuses a control character (exit 2) and shows the other hidden characters as escapes.
- The binary built on this machine needs glibc 2.34 (`objdump -T`). No other libc was measured.
- The set of escaped characters is a list (`src/hidden.rs`), not a Unicode category. A format or spacing character that is not on it is shown as itself.
- `tests/claims.rs` checks in part that fragments of text are in `README.md` and `prices/ebs.json`. That blocks a price or version change; it is not proof of behaviour.
- These tests are also green on the commit before this record, so they pin behaviour that was already right and show no fix: `usage_error_escapes_del_and_backspace_in_the_argument`, `input_at_the_size_limit_is_read_and_one_byte_more_is_refused`, `a_symlink_named_on_the_command_line_is_followed`, `json_mode_sends_warnings_to_standard_error_and_into_the_object`, `nesting_limit_and_input_tolerance_are_as_the_readme_says`, `fail_usd_length_and_zero_rules_are_as_the_readme_says`, `volume_type_case_and_price_source_controls_are_as_the_readme_says`, and the first two tests of `tests/docs_clean.rs`. Others of the same kind (for example `shown_path_escapes_invisible_and_variation_selector_characters`, the `OPTIONS` check of `tests/handbook_help.rs`, and the directory and FIFO tests). They keep it from changing; they do not show a fix. Two tests of this record are red on commit bff1732 and green here: `a_document_with_too_many_json_values_is_refused` (exit 0 there, exit 2 here) and `a_single_object_with_a_volume_and_an_image_id_is_a_mixed_document` (exit 0 there, exit 2 here).

## Open points

These are choices, measured here and described in `README.md`. They are not defects the author has decided to change.

- A completed snapshot that starts on the `--as-of` day, after midnight UTC, is an error (exit 2) and the message names the snapshot id and not the file: `snap-11111111: StartTime is after --as-of`. A `pending` or `error` snapshot is skipped instead. Use the next day as `--as-of`.
- A number may be a JSON string (`"Size":"100"` is 100 GB). The price file's `currency` is not checked (`EUR` is accepted and shown). `--older-than-days +5` is accepted. AWS does not return these forms.
- One bad file in a directory stops the whole run with exit 2.
- JSON nested deeper than 126 arrays inside a document object is refused as not JSON (126 deep is read).
- The total is the exact sum, rounded once, so it can differ from the sum of the displayed rows by less than one cent per row.
- The limit of 16777216 items refuses a realistic export of more than about 400000 volumes with three tags each (about 200 MiB). Raising it raises the worst memory in step (about 300 bytes per item for nested arrays); lowering the 256 MiB size limit or reading in a streaming way is not done. The README states the measured figures.
- The release workflow and the glibc CI job exist but have not run on GitHub (see "Not run").
- A usage error for a short option that holds a hidden character shows only the first character of it (`'-\'`). It is safe and not pretty; clap builds that text.

## Claim → test

Every claim of `README.md`, with the test that covers it. "Gap" means no test proves it, or only part of it.

| # | Claim | Test | Status |
| --- | --- | --- | --- |
| R1 | Quick start output is what the binary prints | `quick_start_block_matches_the_binary`, `quick_start_hides_secrets_and_prices_the_sample` (cli) | tested |
| R2 | Sample orphans total 21.50 USD per month | `example_orphans_total_21_50` | tested |
| R3 | Reads files only, no network, no `aws` call | `source_does_not_spawn_aws_or_link_an_sdk` | tested in part: text scan and dependency list, no network test |
| R4 | Does not delete volumes or snapshots | `builtin_lines_name_the_page_and_do_not_issue_a_delete`, `source_does_not_spawn_aws_or_link_an_sdk` | tested |
| R5 | README sections are in order, the page is cited | `readme_sections_are_in_order`, `readme_and_price_file_cite_the_page` | tested |
| R6 | gp3 storage, IOPS, throughput (0.04 per MB/s) and the full-month example | `gp3_sample_and_the_page_full_month`, `gp3_baseline_is_free_and_missing_fields_use_it`, `throughput_meter_divides_by_1024`, `throughput_identity_and_the_example_rate_are_separated` | tested |
| R7 | gp2, st1, sc1 full months | `gp2_st1_and_sc1_full_months`, `page_full_months_match_the_readme` | tested |
| R8 | io1 full month, missing `Iops` prices storage only | `io1_full_month_and_missing_iops`, `missing_io_iops_pending_and_in_use_loose` | tested |
| R9 | io2 tiers per volume, 32001, 60000, 64001 IOPS | `io2_tiers_use_the_meter_not_the_rounded_example` | tested |
| R10 | Standard snapshots, `FullSnapshotSizeInBytes` before `VolumeSize` | `snapshots_use_standard_storage_and_full_bytes`, `start_after_as_of_is_an_error_and_full_bytes_win` | tested |
| R11 | Magnetic, archive and other meters are not priced; the free tier is not applied | `magnetic_is_not_priced_at_the_recorded_rate`, `apply_flag_cannot_turn_unused_rates_on`, `unpriced_type_and_snapshot_skip_warning_texts`, `readme_prints_the_builtin_note_lines` | tested |
| R12 | Loose volume: no attachments; `in-use` without attachments is loose and warns | `loose_attached_and_single_objects`, `in_use_without_attachments_and_bad_id_is_hidden`, `missing_io_iops_pending_and_in_use_loose` | tested |
| R13 | Old snapshot: strictly older, no AMI link, tiers and states | `age_is_strict_and_display_rounds_half_away`, `empty_images_still_classifies_and_exact_age_does_not`, `without_images_or_as_of_snapshots_stay_out`, `snapshots_images_and_tiers`, `pending_snapshot_starting_today_is_skipped_not_an_error` | tested |
| R14 | Age: UTC `StartTime`, years 1970 to 2199, no leap second, start after `--as-of` is an error | `utc_timestamps_match_the_age_boundaries`, `utc_timestamps_reject_offsets_leap_seconds_and_years`, `as_of_year_range_is_1970_through_2199`, `invalid_as_of_and_fail_usd_exit_2` (cli), `multibyte_start_time_exits_2` (cli) | tested |
| R15 | Display rounding half away from zero, once, on the exact sum of the unrounded products | `usd_rounds_half_away_from_zero`, `usd_keeps_two_digits`, `one_byte_displays_as_zero_and_still_exceeds_a_zero_threshold`, `a_total_of_rounded_products_uses_the_exact_sum`, `a_row_sums_exact_parts_that_meet_on_a_half_cent`, `one_product_near_a_half_cent_rounds_once` (cli) | tested |
| R16 | GB from a size in bytes is rounded once, half away from zero, to at most 28 decimal places; 1 byte shows 0.0000000009313225746154785156 and 2 bytes shows 0.0000000018626451492309570313 | `one_byte_gb_is_a_28_digit_decimal_and_readme_does_not_call_it_exact`, `gb_from_snapshot_bytes_rounds_half_away_from_zero` (cli) | tested |
| R17 | `--fail-usd` strict, equal exits 0, plain digits, at most 40 characters, at most 28 decimals (zeros count), no digit is rounded away; the same for a price rate | `fail_usd_equal_exits_0_and_just_under_exits_1`, `fail_usd_with_29_decimals_exits_2_and_28_decimals_works`, `fail_usd_that_would_be_rounded_exits_2`, `price_rate_with_more_than_28_significant_digits_exits_2` (cli), `threshold_is_strict_on_priced_orphans_only`, `digits_that_do_not_fit_in_a_decimal_are_found`, `fail_usd_length_and_zero_rules_are_as_the_readme_says` (cli) | tested |
| R18 | Inputs: file, one gzip member, directory, `-`; bytes after the member are an error | `stdin_and_gzip`, `gzip_with_bytes_after_the_first_member_is_exit_2`, `gzip_round_trip_and_bad_snapshot_id_on_an_image` | tested |
| R19 | Directory: recursive to 64 levels, skips symlinks, non-regular, empty and non-UTF-8 files; one bad file stops the run | `directory_nested_64_levels_is_fine`, `directory_nested_65_levels_exits_2`, `directory_skips_symlinks`, `directory_skips_a_fifo_as_non_regular`, `directory_skips_junk_and_bad_json_is_an_error` | tested |
| R20 | One kind per file (a single object with a `VolumeId` and an `ImageId` is a mixed document), duplicate ids are an error, `nextToken` is not followed or printed | `duplicate_volume_id_across_inputs_exits_2`, `duplicate_snapshot_and_image_ids_are_errors`, `a_single_object_with_a_volume_and_an_image_id_is_a_mixed_document`, `describe_shapes_exit_0`, `quick_start_hides_secrets_and_prices_the_sample` (cli) | tested |
| R21 | Owner, KMS, instance ids and descriptions are not printed | `quick_start_hides_secrets_and_prices_the_sample` (cli) | tested |
| R22 | A symlink named on the command line is followed | `a_symlink_named_on_the_command_line_is_followed` (cli, Unix) | tested |
| R23 | Input file at most 268435456 bytes, also after decompression | `input_at_the_size_limit_is_read_and_one_byte_more_is_refused` (cli): a file, standard input, and gzip, each at the limit and one byte over | tested |
| R24 | `--json` is one object; warnings go to standard error in JSON mode | `json_output_escapes_bidi_in_the_prices_source` (cli), `quick_start_block_matches_the_binary`, `json_mode_sends_warnings_to_standard_error_and_into_the_object` (cli), `json_report_peak_is_the_output_plus_a_small_constant` (json_peak, memory), `json_strings_are_escaped_like_serde_json` (unit) | tested; the time is measured by hand |
| R25 | Exit 0, 1, 2; `--help` and `--version` exit 0; a clap usage error exits 2 | `help_and_version_exit_0`, `unknown_flag_exits_2`, `missing_file_exits_2`, `fail_usd_equal_exits_0_and_just_under_exits_1` (cli) | tested |
| R26 | `--prices` file in the same shape; a bad file exits 2 | `builtin_matches_the_pricing_page`, `bom_bad_date_and_missing_not_applied`, `rejects_exponent_negative_and_wrong_gb`, `oversized_prices_file_is_exit_2_without_reading_it` (cli) | tested |
| R27 | Prices are us-east-1, a full month, no discount | `builtin_matches_the_pricing_page`, `page_full_months_match_the_readme` | tested |
| R28 | Install with `cargo install --locked --path .` | none | gap |
| R29 | The documented test total is 197 on Linux, lower elsewhere | none | gap: the Linux total is measured on 6118c69; the numbers for other systems (189 and 181) were counted on 1815bb6, not on 6118c69, as test lists counted on a copy with the gating changed, and the tests were not run there |
| R30 | Not published, no git tag | `changelog_licenses_and_manifest` (the changelog says so only) | gap: the repository state is not tested |
| R31 | Licenses MIT OR Apache-2.0, uses clap | `changelog_licenses_and_manifest` | tested |
| R32 | The man page holds the `--help` text and the same long options | `help_long_options_match_the_man_page_and_unknown_option_is_not_a_panic` (`tests/handbook_help.rs`) | tested |
| R33 | Control and bidi characters in file names, the price source, and usage errors are escaped | `control_chars_in_a_short_file_name_stay_escaped`, `bidi_in_the_prices_source_is_escaped_in_text`, `line_splitting_and_blank_chars_in_the_prices_source_are_escaped`, `hangul_filler_in_the_prices_source_is_escaped_without_changing_the_amount`, `usage_error_escapes_bidi_and_newline_in_the_argument`, `usage_error_with_an_escape_sequence_and_newline_is_one_message`, `usage_error_with_a_lone_escape_an_osc_and_a_carriage_return_is_one_message`, `usage_error_escapes_invisible_characters_in_the_argument`, `visible_text_escapes_the_variation_selector_supplement`, `shown_path_escapes_invisible_and_variation_selector_characters`, `non_utf8_file_name_is_shown_as_escaped_bytes`, `usage_error_escapes_spacing_and_format_characters_in_the_argument`, `usage_error_escapes_del_and_backspace_in_the_argument`, `usage_error_keeps_the_reason_when_the_escaped_arguments_parse`, `spacing_and_format_characters_are_hidden`, `shown_path_escapes_spacing_and_format_characters` | tested |
| R34 | A closed or full standard output exits 2 with a message, `/dev/null` exits 0 | `closed_stdout_is_exit_2_and_dev_null_is_exit_0`, `help_and_version_to_closed_or_full_stdout_exit_2` (cli) | tested on Linux; not run on other systems |
| R35 | A closed standard output is recognised on Linux only; a full or broken one exits 2 | `closed_stdout_is_exit_2_and_dev_null_is_exit_0`, `help_and_version_to_closed_or_full_stdout_exit_2` (cli) | tested on Linux; the other systems are not run |
| R36 | A document holds at most 16777216 JSON values plus object keys, or exit 2; memory is a multiple of the file size (about 10, 54 and 150 times for three shapes), bounded at about 5 GiB for one document, and the bytes of all files add up | `a_document_with_too_many_json_values_is_refused` (cli, a document and a price file), `values_and_keys_are_counted_against_the_maximum` (unit) | tested in part: the limit is tested, the memory figures are measured by hand, see Known limits |
| R37 | A glibc 2.34 requirement for the Linux release build; no download; a CI job checks it | none | gap: `scripts/check-glibc.sh` ran here (exit 0 at 2.34, exit 1 at 2.30); the CI job has not run on GitHub |
| R38 | The `--as-of` error names the range 1970-01-01 through 2199-12-31 | `as_of_error_names_the_year_range` (cli), `invalid_as_of_does_not_echo_the_text_body` | tested |
| R39 | The README, QUALITY, CHANGELOG, SECURITY and man page hold no local path, wrapper option, notes file name, internal tool name, or AI name; neither do the manifest, workflow, sources, tests, examples, and prices. A package that has no git metadata still passes | `public_docs_have_no_local_paths_wrapper_options_or_internal_names`, `package_metadata_workflows_sources_and_tests_have_no_internal_names_either`, `a_package_tree_without_git_metadata_is_clean`, `the_check_itself_finds_each_forbidden_pattern` (docs_clean) | tested |
| R40 | JSON nested 126 deep is read and 127 deep is refused; numbers may be strings; the currency is not checked; `+5` is a number; a completed snapshot on the `--as-of` day is an error naming the snapshot | `nesting_limit_and_input_tolerance_are_as_the_readme_says` (cli) | tested |
| R41 | `SECURITY.md`: no call that starts a program or opens a network connection, and a file is written only for `--output` | `source_does_not_spawn_aws_or_link_an_sdk` | tested in part: the scan covers starting a program and network code; the `--output` behaviour is covered by `tests/output.rs`, and no test scans for other file writes |
| R42 | A row and the total are the exact sum of the unrounded products, rounded once, half away from zero. `--fail-usd` compares that exact amount. An amount that does not fit exits 2 with `amount overflowed` | `a_product_or_sum_that_would_lose_digits_is_an_error`, `a_rounded_product_never_shows_a_cent_the_exact_amount_does_not_have`, `a_total_of_rounded_products_uses_the_exact_sum`, `a_row_sums_exact_parts_that_meet_on_a_half_cent`, `fail_usd_compares_the_exact_amount`, `one_product_near_a_half_cent_rounds_once` (cli), `exact_math_refuses_what_does_not_fit`, `a_value_near_a_half_cent_rounds_once`, `extreme_rate_on_a_small_volume_is_an_error` | tested |
| R43 | `--help` and the man page say that exit 2 also covers a standard output that cannot be written | `help_says_exit_2_also_covers_standard_output_that_cannot_be_written` (cli), `help_long_options_match_the_man_page_and_unknown_option_is_not_a_panic` | tested |
| R44 | The price `source` refuses a control character (exit 2) and escapes the other hidden characters; `VolumeType` is matched without regard to case | `volume_type_case_and_price_source_controls_are_as_the_readme_says` (cli) | tested |
| R45 | For a snapshot priced from its size in bytes, `--fail-usd` is exact to about the 28th decimal | `one_byte_displays_as_zero_and_still_exceeds_a_zero_threshold` | tested in part: the 28th-decimal limit itself has no test |
| R46 | The monthly cost is labelled as an estimate: one estimate line in text, totals named `estimated_usd_month`, `"estimate": true` in JSON | `the_monthly_cost_is_labelled_as_an_estimate_in_text_and_json` (cli), `quick_start_block_matches_the_binary` (cli) | tested |
| R47 | Price data dated more than 180 days before `--as-of` (or today) warns and exits 0, on standard error as well as in the report (not with `--quiet`); exactly 180 days does not warn; a price date before 1970 warns too | `price_data_older_than_180_days_warns_and_still_exits_0`, `a_price_file_is_checked_for_age_with_its_own_date`, `price_age_warning_is_also_on_standard_error_in_text_and_markdown`, `price_age_warning_is_not_written_with_quiet`, `price_date_outside_1970_to_2199_still_warns` (cli) | tested |
| R48 | The describe-volumes, describe-snapshots, and describe-images shapes that the AWS CLI reference documents, at <https://docs.aws.amazon.com/cli/latest/reference/ec2/describe-volumes.html>, <https://docs.aws.amazon.com/cli/latest/reference/ec2/describe-snapshots.html>, and <https://docs.aws.amazon.com/cli/latest/reference/ec2/describe-images.html> (the fixtures in `tests/fixtures/` are written by hand in that shape, with the same fields; they are not copied example output) (optional fields missing, empty lists, several values, special characters) run without error and total 126.93 | `the_documented_shape_prices_to_the_exact_amount`, `the_same_shape_in_text_mode_shows_the_same_totals`, `without_describe_images_snapshots_are_not_orphans`, `empty_lists_are_a_zero_report` (real_shape; green on the previous commit too, so they pin behaviour and show no fix) | tested |
| R49 | `--quiet` hides the warnings, `--verbose` prints progress on standard error only, the result and exit code do not change, both together exit 2 | `help_lists_quiet_and_verbose`, `quiet_drops_warnings_and_keeps_the_result_and_exit_code`, `verbose_adds_progress_on_stderr_only`, `quiet_with_verbose_is_a_usage_error` (verbosity) | tested |
| R50 | `--md` prints the same amounts as text and JSON, escapes a pipe, cannot be combined with `--json`, and keeps the threshold exit | `markdown_has_the_same_amounts_as_text_and_json`, `markdown_escapes_pipes_and_line_breaks_in_cells_and_lines`, `markdown_and_json_together_are_a_usage_error`, `markdown_keeps_the_threshold_exit_and_quiet_drops_warnings` (markdown) | tested |
| R51 | SIGINT ends the run with exit 130, one line on standard error, and an empty standard output (Unix only) | `sigint_while_reading_stdin_exits_130_with_empty_stdout` (sigint, sends a real SIGINT with `kill`) | tested on Unix only |
| R52 | `--output` writes the whole report or leaves the old file, refuses a symlink and a non-regular target, exits 2 on a write error, and SIGINT keeps the old file and removes the temporary one | `the_report_goes_to_the_file_and_not_to_stdout`, `an_existing_file_is_replaced_and_the_threshold_exit_stays`, `a_missing_parent_directory_is_exit_2_and_is_not_created`, `a_directory_or_a_device_as_target_is_refused`, `a_symlink_as_target_is_refused_and_not_followed`, `a_directory_that_cannot_be_written_is_exit_2` (output), `sigint_keeps_an_existing_output_file_whole` (sigint) | tested on Unix; the last two of the output tests are Unix only |
| R53 | Every JSON report starts with `schema_version` 1, and a failing run prints no JSON | `every_json_path_has_schema_version`, `a_failing_json_run_prints_no_json` (schema_version; the second is green on the previous commit too and pins behaviour) | tested |

53 claims: 45 tested (R33, R34 and R35 on Linux only), 4 tested in part (R3, R36, R41, R45), 4 without a test that proves them (R28, R29, R30, R37).
