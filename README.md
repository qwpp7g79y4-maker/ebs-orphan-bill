# ebs-wees-bill

Find loose EBS volumes and snapshots older than N days with no AMI, and the monthly cost of each one. Reads files only. No network.

## Why

An unattached volume and a snapshot that no image uses keep billing after the instance is gone. `ebs-wees-bill` reads JSON you already saved from `aws ec2 describe-volumes`, `aws ec2 describe-snapshots`, and `aws ec2 describe-images`. It lists the loose volumes and the old standard snapshots, with a monthly USD figure for each priced row.

The report identifies orphans and monthly cost. It does not delete volumes or snapshots.

## Quality

`cargo test --locked` passes 197 tests and fails 0 on Linux, in debug and in release, on stable and on Rust 1.85. Some tests run on Linux or Unix only, so the count is lower on macOS and Windows. See `QUALITY.md`.

## Installation

Build from source.

Rust 1.85 or newer. From a clone of this crate:

```sh
cargo install --locked --path .
ebs-wees-bill --help
```

The binary is `ebs-wees-bill`. Version 1.0.0 is not published and has no git tag. The command is built with clap. There is no download: build from source. A binary built on Linux needs the glibc of the machine that built it; the release build checked for this page needed glibc 2.34, and a CI job fails when the highest glibc symbol of a release build is newer than that. Other libc builds (for example musl) and the oldest macOS or Windows version that works were not measured. macOS and Windows builds are checked by compiling only (see `QUALITY.md`).

## Quick start

The sample files are `examples/volumes.json`, `examples/snapshots.json`, and `examples/images.json`. The command does not call AWS. `--older-than-days` defaults to 30. The sample snapshot file has a `NextToken`. EC2 uses that key, and a lowercase `nextToken` is the same warning. The report says the page may be partial. The token value is not printed.

<!-- quick-start:start -->
```console
$ ebs-wees-bill --as-of 2026-11-09 --older-than-days 30 examples/volumes.json examples/snapshots.json examples/images.json
warning: examples/snapshots.json: page may be partial; nextToken was not followed
warning: vol-44444444444444444: volume type standard is not priced
warning: snap-44444444444444444: snapshot tier archive is not priced
prices: USD, source https://aws.amazon.com/ebs/pricing/, date 2026-11-09, region us-east-1
meter published: 2026-11-07
as-of: 2026-11-09
older-than-days: 30
free tier not applied: the pricing page includes 30 GB of storage, 2000000 I/Os, and 1 GB of snapshot storage for the account, and this export is not the whole account
not priced: magnetic standard volumes (0.05 USD per GB-month), snapshot archive storage (0.0125 USD per GB-month) and archive retrieval (0.03 USD per GB), fast snapshot restore, direct APIs, time-based copy, volume initialization, and volume clones. Priced types are gp2, gp3, io1, io2, st1, sc1, and standard snapshots
gp3 throughput is 0.04 USD per MB/s-month, the meter 40.96 USD per GiBps-month divided by 1024. The pricing page example still says 0.06
io2 IOPS are tiered per volume: the first 32000 at 0.065 USD, then to 64000 at 0.0455 USD, then 0.03185 USD. The pricing page example rounds the middle meter to 0.046; this tool uses the meter
snapshot size is an upper bound: FullSnapshotSizeInBytes when present (basis full-snapshot-bytes), otherwise VolumeSize (basis volume-size). describe-snapshots does not return changed blocks
this report identifies orphans and monthly cost. It does not delete volumes or snapshots
Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)

volumes
id                     type      size_gb  iops  mib_s  storage_usd  iops_usd  throughput_usd  usd_month
vol-11111111111111111  gp3           100  4000    200         8.00      5.00            3.00      16.00
vol-22222222222222222  gp2            50                      5.00                                 5.00
vol-44444444444444444  standard       20                  unpriced  unpriced        unpriced   unpriced
volume estimated_usd_month=21.00

snapshots
id                      age_days  gb  usd_month  size_basis   tier
snap-11111111111111111        31  10       0.50  volume-size  standard
snap-44444444444444444        31  80   unpriced  volume-size  archive
snapshot estimated_usd_month=0.50

estimated_usd_month=21.50
note: this report identifies orphans and monthly cost. It does not delete volumes or snapshots
```
<!-- quick-start:end -->

## Aan de slag in 5 minuten

1. Build the tool: `cargo install --locked --path .` (Rust 1.85 or newer). Check it with `ebs-wees-bill --version`.
2. Save your own exports (the command only reads files; it never calls AWS): `aws ec2 describe-volumes > volumes.json`, `aws ec2 describe-snapshots --owner-ids self > snapshots.json`, and `aws ec2 describe-images --owners self > images.json`.
3. Run `ebs-wees-bill --as-of "$(date -u +%F)" volumes.json snapshots.json images.json`. Without `--as-of` the volumes are priced and the snapshots are not classified.
4. Read the last lines: `estimated_usd_month=` is the estimate for the loose volumes and old snapshots, from list prices and not from your invoice.
5. Use it in a script: `--fail-usd 50` exits 1 when one orphan costs more than 50 USD per month, `--json` and `--md` change the format, and `--output report.md` writes a file.

## Options

```text
ebs-wees-bill [OPTIONS] <FILE>...

  <FILE>...                 describe-volumes, describe-snapshots, or describe-images JSON: files, gzip, directories, or - for standard input
  --prices <FILE>           Price file. The default is the built-in US East (N. Virginia) EBS list
  --as-of <YYYY-MM-DD>      Date ages are measured to, midnight UTC. Without it, snapshots are not classified
  --older-than-days <DAYS>  A snapshot is old when its age is strictly greater than this many days [default: 30]
  --fail-usd <AMOUNT>       Exit 1 when a priced orphan's exact monthly USD is strictly greater than this
  --json                    Print JSON instead of the text table
  --md                      Print a Markdown report (tables) instead of the text table
  --output <FILE>           Write the report to this file instead of standard output
  --quiet                   Hide the warnings; print only the result and errors
  --verbose                 Print progress on standard error
  -h, --help                Print help
  -V, --version             Print version
```

`--version` prints the crate version. A debug build made in a git checkout also prints the commit, as `ebs-wees-bill 1.0.0 (<40-character sha>)`; a release build, or an unpacked package without git, prints the version only. `--help` and `man/ebs-wees-bill.1` hold the full text. The notes under "How it counts" explain each option in detail.

## Example

`vol-11111111111111111` is a loose gp3 volume of 100 GB with 4000 IOPS and 200 MB/s. Storage is `100 * 0.08 = 8.00`. IOPS above 3000 are `(4000 - 3000) * 0.005 = 5.00`. Throughput above 125 MB/s is `(200 - 125) * 0.04 = 3.00`. The row is 16.00 USD per month.

`vol-22222222222222222` is a loose gp2 volume of 50 GB. Storage is `50 * 0.10 = 5.00`. gp2 I/O is included in that storage price.

`vol-33333333333333333` is attached, so it is not an orphan. Its instance id is not printed.

`vol-44444444444444444` is a loose magnetic `standard` volume of 20 GB. That previous-generation rate is 0.05 USD per GB-month and is not applied, so the row is unpriced. `20 * 0.05 = 1.00` is not the figure.

`snap-11111111111111111` started at `2026-10-09T00:00:00.000Z`. On 2026-11-09 that is 31 days, which is strictly greater than 30. No image references it. `VolumeSize` is 10 GB and `FullSnapshotSizeInBytes` is absent, so the basis is `volume-size` and the cost is `10 * 0.05 = 0.50`.

`snap-22222222222222222` is old and standard, and `ami-11111111111111111` references it in `BlockDeviceMappings[].Ebs.SnapshotId`. It is not an orphan. A description that mentions CreateImage is not AMI linkage and is not printed.

`snap-33333333333333333` started at `2026-10-10T00:00:00.000Z`. That is exactly 30 days before 2026-11-09, so it is not older than 30. `2026-10-09T23:59:59Z` is 30 days plus one second and does qualify.

`snap-44444444444444444` is an archive snapshot, 80 GB, and old enough. Archive storage is 0.0125 USD per GB-month and is not applied. `80 * 0.0125 = 1.00` is not the figure.

Priced volume orphans total `16.00 + 5.00 = 21.00`. The priced snapshot total is 0.50. The grand total is 21.50 USD per month, printed as `estimated_usd_month=21.50`. Every report says it is an estimate from list prices: the text has one line `Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)`, and the JSON has `"estimate": true` at the top level (with the same sentence in `estimate_note`). The totals are `estimated_usd_month`, `volume_estimated_usd_month`, and `snapshot_estimated_usd_month` in JSON.

Exit 0 is a priced report. Exit 1 means `--fail-usd` is set and some priced orphan's exact monthly USD is strictly greater. Exit 2 is bad input, bad usage, a standard output that cannot be written, or an `--output` file that cannot be written. `--fail-usd 16` matches the gp3 row, so that command exits 0. `--fail-usd 15.99` exits 1. An attached volume, a young snapshot, an AMI snapshot, an archive snapshot, and an unpriced magnetic volume do not trip the threshold. `--json` prints one JSON object on standard output. Warnings go to standard error in JSON mode and are also a `warnings` array in the object. Text mode prints warnings on standard output.

```sh
ebs-wees-bill --as-of 2026-11-09 --fail-usd 16 examples/volumes.json examples/snapshots.json examples/images.json
ebs-wees-bill --as-of 2026-11-09 --fail-usd 15.99 --json examples/volumes.json examples/snapshots.json examples/images.json
```

## Exit codes

| Code | Meaning |
| ---: | --- |
| 0 | A priced report was written. |
| 1 | `--fail-usd` is set and some priced orphan's exact monthly USD is strictly greater. |
| 2 | Bad input, bad usage, a standard output that cannot be written, or an `--output` file that cannot be written. |
| 130 | SIGINT (Ctrl-C) on Unix. Nothing is on standard output and no half-written file is left. |

`--help` and `--version` exit 0. A usage error from clap exits 2.

## How it counts

Rates are the US East (N. Virginia) rows on <https://aws.amazon.com/ebs/pricing/>, read on 2026-11-09. The price file that page loads was published 2026-11-07 (`hawkFilePublicationDate` 2026-11-07T00:28:52Z, source `ebs`, currency USD). The built-in quote is `prices/ebs.json`.

- **Gigabyte.** The page says GB = 1024^3 bytes. `Size` and `VolumeSize` are already that GB. `FullSnapshotSizeInBytes / 1024^3` is the GB figure, as a Decimal. `gb_bytes` in the price file is 1073741824.
- **gp3.** Meter `Storage General Purpose gp3 GB Mo` is 0.08 USD per GB-month. Meter `Provisioned EBS IOPS gp3 Volumes per IOPS Mo` is 0.005 USD per IOPS-month above the 3000 IOPS baseline. Meter `Provisioned Throughput gp3 per GiBps mo` is 40.96 USD per GiBps-month. The page divides that meter by 1024 (`decimals=3`), which is 0.04 USD per MB/s-month above the 125 MB/s baseline. A missing `Iops` or `Throughput` uses that baseline, so the extra is 0. A value below the baseline also has extra 0.
- **gp3 worked example, full month.** The page's example 1 provisions 2000 GB, 10000 IOPS, and 500 MB/s for 12 hours and shows $3.625. This tool does not prorate those 12 hours. The full month is `2000 * 0.08 = 160.00`, `(10000 - 3000) * 0.005 = 35.00`, and `(500 - 125) * 0.04 = 15.00`, which is 210.00 USD. The same example still says 0.06 USD per MB/s. That would be `(500 - 125) * 0.06 = 22.50` and is not used.
- **gp2.** Meter `Storage General Purpose GB Mo` and `Storage General Purpose gp2 GB Mo` are 0.10 USD per GB-month. 2000 GB for a full month is 200.00 USD. The page's 12-hour example is $3.33 and is not the monthly figure.
- **io1.** Storage meters `Storage Provisioned IOPS GB Mo` and `Storage Provisioned IOPS io1 GB Mo` are 0.125 USD per GB-month. IOPS meter `Provisioned EBS IOPS Volumes per IOPS Mo` and `Provisioned EBS IOPS io1 Volumes per IOPS Mo` are 0.065 USD per IOPS-month. 2000 GB and 1000 IOPS for a full month is `250.00 + 65.00 = 315.00` USD. The page's 12-hour example is $5.25 and is not the monthly figure. A missing `Iops` prices storage only and warns that IOPS are not priced.
- **io2.** Storage meter `Storage Provisioned IOPS io2 GB month` is 0.125 USD per GB-month. IOPS are tiered per volume, not across the account. Tier 1 meter `Provisioned EBS IOPS io2 Volumes per IOPS Mo` is 0.065 up to 32000. Tier 2 meter `Provisioned EBS IOPS Tier 2 io2 Volumes per IOPS Mo` is 0.0455 up to 64000. Tier 3 meter `Provisioned EBS IOPS Tier 3 io2 Volumes per IOPS Mo` is 0.03185 above that. 32000 IOPS is 2080.00. 32001 IOPS is 2080.0455, shown as 2080.05. 60000 IOPS is `32000 * 0.065 + 28000 * 0.0455 = 3354.00`. The page example rounds the middle meter to 0.046 and a 12-hour charge of $56.133; `32000 * 0.065 + 28000 * 0.046 = 3368.00` is not the full-month figure. 64000 IOPS is 3536.00. 64001 IOPS is 3536.03185, shown as 3536.03. Ten volumes of 1000 IOPS are not one volume of 10000 IOPS.
- **st1 and sc1.** `Storage Throughput Optimized HDD GB Mo` is 0.045. 2000 GB is 90.00 USD per month. The page's 12-hour example is $1.50 and is not the monthly figure. `Storage Cold HDD GB Mo` is 0.015. 2000 GB is 30.00 USD per month. The page's 12-hour example is $0.50 and is not the monthly figure.
- **Standard snapshots.** Meter `Storage Snapshot Amazon S3 GB Mo` is 0.05 USD per GB-month. A missing `StorageTier` counts as standard. Billable snapshot size is changed blocks, which `describe-snapshots` does not return. `FullSnapshotSizeInBytes` is used when present (basis `full-snapshot-bytes`). Otherwise `VolumeSize` is used (basis `volume-size`). Both are an upper bound except a first full snapshot.
- **Not priced.** Magnetic standard volumes (`Storage Magnetic standard GB Mo`, 0.05), snapshot archive storage (0.0125) and archive retrieval (0.03), fast snapshot restore, direct APIs, time-based copy, volume initialization, and volume clones. The lines below are what the tool prints. `apply_not_applied` cannot be true.
- **Free tier.** The page includes 30 GB of storage, 2 million I/Os, and 1 GB of snapshot storage for the account. This export is not the whole account, so the waiver is not applied. A 30 GB gp2 volume is `30 * 0.10 = 3.00` USD, not free. A 1 GB standard snapshot is `1 * 0.05 = 0.05` USD, not free.
- **Loose volume.** `Attachments` missing or empty. A non-empty attachment list, including `detaching`, is attached and is not an orphan. `State` `in-use` with empty attachments is still loose, and the report warns.
- **Old snapshot.** `StorageTier` `standard` or missing, age strictly greater than `--older-than-days` (default 30), and not referenced by any image `BlockDeviceMappings[].Ebs.SnapshotId`. Description text that contains `ami-` is not linkage. Without a `describe-images` document, snapshots are not classified as orphans. `{"Images":[]}` means the images were checked and none reference a snapshot. `pending` and `error` are skipped with a warning. `completed` or a missing state is eligible. Archive and any other tier are listed when they would otherwise be orphans, and they are not priced.
- **Age.** With `--as-of YYYY-MM-DD`, age is milliseconds from `StartTime` to that date at midnight UTC. `StartTime` must be UTC (`Z` or `+00:00`), with an optional fraction, years 1970 through 2199, and no leap second. `--as-of` must be 1970-01-01 through 2199-12-31, or exit 2. Equal age does not qualify. A start time after `--as-of` is an error. Without `--as-of`, volumes are still priced and snapshots are not orphans.
- **Display.** USD is rounded once, half away from zero, to 2 decimal places. A half cent rounds away from zero: 0.005 is shown as 0.01, and 0.004 is shown as 0.00. A row and the report total are the exact sum of the unrounded products, and that exact amount is what is rounded. 1001 GB at a rate of 0.4106043956043956043956043956 is exactly 411.0149999999999999999999999956 and is shown as 411.01. If the exact amount does not fit in the accumulator, or the once-rounded cent amount does not fit in a decimal, the command exits 2 with `amount overflowed`. GB from a size in bytes is `bytes / 1024^3`, rounded once, half away from zero, to at most 28 decimal places. For 1 byte the true fraction ends in 15625, and the output shows 0.0000000009313225746154785156. For 2 bytes it shows 0.0000000018626451492309570313. Age is a decimal division and can still differ in the last digits. The threshold compares the exact monthly amount, before that display rounding. A row that shows 0.00 can still exceed a threshold of 0.
- **Price age.** When the `date` of the price file (the built-in one or `--prices`) is more than 180 days before `--as-of`, or before today when `--as-of` is not set, the report warns with that date (`price data is 181 days old (dated 2026-11-09, ...)`). The warning follows the other warnings (standard output in text mode, the `warnings` array in JSON mode) and is also written to standard error in every mode, except with `--quiet`. The exit code stays 0. Exactly 180 days does not warn. Any valid date warns when it is old enough, including one before 1970, such as `1900-01-01`.
- **Markdown.** `--md` prints a Markdown report instead of the text table: a heading, the estimate line, the same notes as a list, a `## Warnings` list (not with `--quiet`), a table for volumes and one for snapshots, and the totals as `estimated_usd_month=21.50`. The amounts are the same strings as in text and JSON, with no other rounding. A `|` in a cell or line is written `\|`, and line breaks and hidden characters are `\u{..}` escapes. `--md` with `--json` is a usage error (exit 2). The first lines of the table for the sample files are:

```markdown
| id | type | size_gb | iops | mib_s | storage_usd | iops_usd | throughput_usd | usd_month |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| vol-11111111111111111 | gp3 | 100 | 4000 | 200 | 8.00 | 5.00 | 3.00 | 16.00 |
```

- **SIGINT.** On Unix, Ctrl-C (SIGINT) ends the run with exit 130 and the line `ebs-wees-bill: interrupted by SIGINT` on standard error. Before the report is written nothing has gone to standard output, so an empty output is not a success; a SIGINT during the single write of the report lets that write finish (never half a report) and then exits 130. Windows has no such handler.
- **Output file.** `--output FILE` writes the report (text, `--json`, or `--md`) to a file and leaves standard output empty. The report goes to a temporary file in the same directory (`.NAME.tmp-PID-N`), which is renamed over `FILE`, so the file is the old one or the whole new one; SIGINT removes the temporary file. An existing regular file is replaced. A symlink is refused and not followed, and so is a directory or any other file that is not a regular file. A missing parent directory is not created. A write error exits 2 with `ebs-wees-bill: cannot write FILE: ...`. Warnings in JSON mode still go to standard error. Exit 1 for `--fail-usd` still applies after the file is written.
- **JSON schema version.** Every `--json` report starts with `"schema_version": 1`, a number, as its first key. It changes only when a key is removed or changes meaning; adding a key does not change it. The tool has no JSON error output: a failing run (exit 2) prints nothing on standard output.
- **Quiet and verbose.** `--quiet` hides the warnings (the `warning:` lines in text mode, the lines on standard error in JSON mode) and prints only the result and errors; the JSON `warnings` array stays and the exit code does not change. `--verbose` adds one `reading <file>` line per input and one `context:` line of counts on standard error; standard output is the same. Both together are a usage error and exit 2 (as for every other usage error).
- **Threshold.** `--fail-usd AMOUNT` exits 1 when some priced orphan's exact monthly USD is strictly greater than `AMOUNT`. Equal does not fail. `AMOUNT` is plain digits with an optional fraction, no sign and no exponent, with at most 28 decimals and no more digits than a decimal holds exactly (the largest is 79228162514264337593543950335), or exit 2. Leading and trailing zeros do not count towards those digits, but the text is at most 40 characters and its decimals are counted with their zeros: `0.50000000000000000000000000000` has 29 decimals and exits 2, and so does `79228162514264337593543950335.00000000000` (41 characters). An amount that would be rounded when read, such as `1844674407370955161.49999999999`, is refused instead. For a snapshot priced from `FullSnapshotSizeInBytes`, the GB figure is rounded once, half away from zero, to the digits a decimal holds. The monthly amount is that figure times the rate, kept exact, and the threshold compares that exact amount.
- **Inputs.** A file, a single gzip member (magic bytes `1f 8b`, not the file name; bytes after that member, including a second member, are an error), a directory, or `-` for standard input. A directory is read recursively, at most 64 levels deep. It skips symlinks and other non-regular files with a warning, and it skips empty files and non-UTF-8 files. A symlink named directly on the command line is followed. A file that is not JSON is an error, and one bad file in a directory stops the whole run with exit 2. JSON nested too deeply is not JSON: measured on Linux, an object that holds arrays nested 126 deep is read and 127 deep is refused. One file is one kind: volumes, snapshots, or images; a single object that holds both a `VolumeId` and an `ImageId` is a mixed document, as it is inside an array. Duplicate ids are an error. `nextToken` is not followed and its value is not printed. Owner ids, KMS key ids, instance ids, and descriptions are not printed. Line breaks, control characters, invisible characters, and direction-changing characters in a file name and in an argument that clap repeats in a usage error are shown as `\u{..}` escapes on one line. In the price `source`, a control character (a newline, ESC, DEL, U+0085) is refused with `bad source` and exit 2; the invisible and direction-changing characters are shown as `\u{..}` escapes. A file name that is not UTF-8 shows its bytes as `\xNN`. The escaped set is a list, not a Unicode category: controls, white space other than the plain space, the invisible and direction-changing characters, a few Arabic and musical format characters, and U+FFFC. A character that is not on it is shown as itself.
- **Input tolerance.** A number may be a JSON string: `"Size":"100"` and `"FullSnapshotSizeInBytes":"5"` are read as numbers. `VolumeType` is matched without regard to case: `"GP2"` is priced as gp2 (AWS returns lower case). The price file's `currency` is shown and not checked: a file that says `EUR` is accepted, and its amounts are still reported in columns named `usd_month`. `--older-than-days` accepts a leading plus (`+5`). A completed snapshot that starts on the `--as-of` day (after midnight UTC) is an error, and the message names the snapshot id and not the file.

The tool prints these lines from the built-in quote:

free tier not applied: the pricing page includes 30 GB of storage, 2000000 I/Os, and 1 GB of snapshot storage for the account, and this export is not the whole account

not priced: magnetic standard volumes (0.05 USD per GB-month), snapshot archive storage (0.0125 USD per GB-month) and archive retrieval (0.03 USD per GB), fast snapshot restore, direct APIs, time-based copy, volume initialization, and volume clones. Priced types are gp2, gp3, io1, io2, st1, sc1, and standard snapshots

gp3 throughput is 0.04 USD per MB/s-month, the meter 40.96 USD per GiBps-month divided by 1024. The pricing page example still says 0.06

io2 IOPS are tiered per volume: the first 32000 at 0.065 USD, then to 64000 at 0.0455 USD, then 0.03185 USD. The pricing page example rounds the middle meter to 0.046; this tool uses the meter

snapshot size is an upper bound: FullSnapshotSizeInBytes when present (basis full-snapshot-bytes), otherwise VolumeSize (basis volume-size). describe-snapshots does not return changed blocks

this report identifies orphans and monthly cost. It does not delete volumes or snapshots

Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)

## Limitations

- Offline and read-only. There is no AWS SDK and no network call. Nothing in this crate runs `aws`. The report does not delete volumes or snapshots.
- Prices are the us-east-1 meters from the page above, for a full month of the provisioned size. Other regions differ. Pass `--prices` with your own quote in the same shape. The tool does not download prices. Real bills are per second with a 60-second minimum. The 12-hour examples on the page are not used.
- No contract discount and no free-tier waiver. The 30 GB, 2 million I/Os, and 1 GB snapshot waiver is for the account, and one export is not the whole account.
- Magnetic `standard` volumes and archive snapshots are not priced. Fast snapshot restore, direct APIs, time-based copy, volume initialization, and volume clones are not priced.
- Snapshot cost is an upper bound. Changed blocks are not in `describe-snapshots`. A later incremental snapshot can be smaller than `VolumeSize`.
- An AMI link is only `describe-images` `BlockDeviceMappings[].Ebs.SnapshotId`. Without that document, snapshots are not called orphans.
- A partial page has a `nextToken`. The token is not followed. The report warns that the page may be partial.
- USD on each row is rounded separately. A total can differ from the sum of the displayed rows by less than one cent per row. `--fail-usd` uses the exact amount.
- One input file may be at most 268435456 bytes, gzip included, after decompression as well. A document may also hold at most 16777216 JSON values plus object keys (an object counts as one value, each key as one more), or the command exits 2 with `too many JSON values (at most 16777216)` before the tree is built; the same holds for a price file. A file is read whole and parsed in memory, so memory use is a multiple of the file size, and the multiple depends on the shape. Measured on Linux with the release binary: a realistic export of 64652 loose volumes with three tags each (33554410 bytes) took 1.01 s and about 341 MiB (about 10 times the file size); a valid 12000023-byte file with 6000000 numbers in one array took 0.86 s and about 619 MiB (about 54 times); a file of 134000113 bytes with 258189 such volumes took 5.18 s and about 1.3 GiB. At the item limit, three shapes of the same count (about 16.7 million items) took: one array of numbers (33554442 bytes) 2.80 s and 1.7 GiB; objects `{"a":1}` in an array (44739246 bytes) 6.22 s and 2.8 GiB; arrays nested 120 deep (33554290 bytes, the worst shape tried) 6.07 s and 4.8 GiB, which is about 150 times the file size. So the memory of one document is bounded at about 5 GiB, where the byte limit alone would have allowed tens of GB. `--json` is written straight into one string, so its memory is the output plus the same tree as text mode, not a second tree of values. Measured on Linux with the release binary (2026-11-20, one run each) on files of 580000, 1160000, and 2320000 volumes (33514733, 67029453, and 134058893 bytes, at most 16.2 million items): `--json` took 2.10 s, 4.11 s, and 9.67 s and 632728, 1263384, and 2523300 KiB (2.4 GiB for the largest; text mode took 2.11 s, 4.10 s, and 9.24 s and 651136, 1262636, and 2524368 KiB). The same file at 2320000 volumes took 20.87 s and 11293720 KiB (10.8 GiB) in `--json` mode before this change. With two warnings per volume (`io1` volumes `in-use` without `Iops`; files of 450000, 900000, and 1800000 volumes, 33652813, 67305613, and 134611213 bytes) `--json` took 4.33 s, 8.56 s, and 17.00 s and 843208, 1683972, and 3363948 KiB (3.2 GiB for the largest; text mode 2.48 s, 4.63 s, and 8.81 s and 754088, 1507436, and 3011280 KiB), where it took 27.48 s and 10170228 KiB (9.7 GiB) before. Time and memory double with the input in both modes. The `--json` time is above the text time when there are many warnings, because each is written to standard error and into the object. The item limit refuses a large realistic export: a file of 268000204 bytes with 516378 volumes with three tags each holds about 21.7 million items and is refused after 1.56 s; split such an export (about 400000 volumes of that shape fit). The size limit applies to each file. All files of one run are read before any is parsed, so their bytes add up: 8 files of 16 MiB (about 134 MB together) took 2.96 s and about 373 MiB. Split a very large export.
- A closed standard output (`>&-`) is recognised on Linux only, where the command then exits 2. On other systems the check is not made. A full or broken standard output (`>/dev/full`, a closed pipe) exits 2, measured on Linux.
- The test total is a Linux figure. Tests for symlinks and FIFOs run on Unix only, and tests that need `/dev/full`, a closed descriptor, a file name with a control character, or a file name that is not UTF-8 run on Linux only, so macOS and Windows run fewer tests.
- `--json` is one pretty JSON object on standard output. Exit 0 is ok. Exit 1 is a priced orphan above `--fail-usd`. Exit 2 is bad input, bad usage, or a standard output that cannot be written. Exit 130 is SIGINT on Unix. `ebs-wees-bill --help` and `--version` exit 0. A usage error from clap exits 2.

## Prices and responsibility

The built-in prices are a snapshot: the us-east-1 list prices from <https://aws.amazon.com/ebs/pricing/>, dated 2026-11-09 (the `date` in `prices/ebs.json`). They are not updated by the tool. Check the current prices yourself before you rely on a figure, and pass your own price file with `--prices <FILE>` when they differ. You are responsible for every decision you take based on the output; the report is an estimate, not an invoice.

## Maintenance

Report bugs through GitHub Issues of this repository. After a release the tool is dormant: a new version appears only for a reported bug, a request, or a security problem, and security always comes first.

## Support

Questions, bug reports, and ideas: mail pep_ai@icloud.com, or open an issue on the repository. For a security problem, read `SECURITY.md` first.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option. Copyright 2026 PEPAI.

MIT OR Apache-2.0.

The licenses of the third-party crates built into the binary are listed in [THIRD-PARTY-LICENSES.html](THIRD-PARTY-LICENSES.html).

## Contact

- E-mail: pep_ai@icloud.com
- Web: https://www.pepai.nl
- X: https://x.com/pepai_nl
- Ko-fi: https://ko-fi.com/pepai
