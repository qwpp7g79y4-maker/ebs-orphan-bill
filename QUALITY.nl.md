# Kwaliteit

Meting van 2026-11-25. Gemeten op commit a483cd8cf9e0e4e653cd9d57914f4412ee77a493 ("Do not price creating, deleting, deleted or error volumes as a full month of orphan cost"), in een schone worktree. Deze commit voegt alleen documentatie toe; de meting hieronder geldt voor codecommit a483cd8.

Nederlandse samenvatting van `QUALITY.md`; de Engelse tekst is leidend. Toolchain: stable rustc 1.99.0 / cargo 1.99.0, Linux x86_64. Geen tag, geen push. Alleen de vier commando's in de tabel hieronder zijn uitgevoerd. De rest is niet uitgevoerd (not run).

## Uitgevoerde controles

| Commando | Exit |
| --- | ---: |
| `cargo fmt --check` | 0 |
| `cargo clippy --all-targets --locked -- -D warnings` | 0 |
| `cargo test --locked` (debug) | 0 (198 geslaagd, 0 mislukt, 0 genegeerd) |
| `cargo test --release --locked` | 0 (198 geslaagd, 0 mislukt, 0 genegeerd) |

Gemeten op 2026-11-25 met toolchain 1.85.1 (`rustc 1.85.1`, `cargo +1.85`, de gedeclareerde ondergrens) op codecommit a483cd8: `cargo +1.85 fmt --check` exit 0; `cargo +1.85 clippy --all-targets --locked -- -D warnings` exit 0; `cargo +1.85 test --locked` exit 0 (198 geslaagd, 0 mislukt, 0 genegeerd); `cargo +1.85 test --release --locked` exit 0 (198 geslaagd, 0 mislukt, 0 genegeerd).

Niet uitgevoerd (not run) in deze meting: clippy voor de Windows- en macOS-doelen, `cargo package` met de testrun in de uitgepakte crate, `cargo deny`, `cargo about`, `scripts/check-glibc.sh`, `cargo audit`, en de run van het programma met gesloten of volle standaarduitvoer. Hierover doet dit bestand geen uitspraak.

## Testresultaat per doel

Aantallen van de debug-run. De release-run gaf in totaal 198 geslaagd, 0 mislukt, 0 genegeerd.

| Doel | Geslaagd | Mislukt | Genegeerd |
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
| totaal | 198 | 0 | 0 |
