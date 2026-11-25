# Examples

Three saved exports in the shape of `aws ec2 describe-volumes`, `describe-snapshots`, and `describe-images`. The values are made up. Nothing here calls AWS.

| File | What it holds |
| --- | --- |
| `volumes.json` | four volumes: a loose gp3, a loose gp2, an attached one, and a loose magnetic `standard` volume |
| `snapshots.json` | four snapshots: a standard one older than 30 days, one that an image uses, one exactly 30 days old, and an archive snapshot; the file has a `NextToken` |
| `images.json` | one image that uses `snap-22222222222222222` |
| `report.expected` | the exact standard output of the command below |

Run it from the repository root (the command reads the files and writes only to standard output):

```sh
ebs-wees-bill --as-of 2026-11-09 --older-than-days 30 examples/volumes.json examples/snapshots.json examples/images.json
```

The output is `report.expected`: `estimated_usd_month=21.50`, which is 21.00 for the volumes and 0.50 for the snapshots. `--json` and `--md` print the same amounts in other formats. A test (`tests/examples.rs`) runs the command and compares the output with `report.expected`, so the file cannot drift from the tool.
