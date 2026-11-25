# Kwaliteit

Deze commit wijzigt alleen documentatie; de meting hieronder geldt voor codecommit 6118c69.

Nederlandse samenvatting van `QUALITY.md`. De Engelse tekst is leidend; de tabel met claims en tests (R1 tot en met R53) staat daar, met de namen van de tests. De vorm van de invoer volgt de AWS CLI-referentie: <https://docs.aws.amazon.com/cli/latest/reference/ec2/describe-volumes.html>, <https://docs.aws.amazon.com/cli/latest/reference/ec2/describe-snapshots.html>, en <https://docs.aws.amazon.com/cli/latest/reference/ec2/describe-images.html>; de fixtures in `tests/fixtures/` zijn met de hand in die vorm geschreven en geen letterlijk overgenomen voorbeelduitvoer.

Reeks: 197 geslaagd, 0 genegeerd, 0 mislukt (Linux, debug en release, stable en Rust 1.85).

Meting van 2026-11-24. De testruns hieronder draaiden op de commit 6118c69f99d0266b34ae0d36103e61dddd4dee92 (tak `poets-ebs`), de laatste commit die een test wijzigt. De commits daarna wijzigen alleen documenten. Alleen de vier `cargo test`-rijen en de testaantallen zijn metingen van die commit; alle andere rijen van de tabel (format, clippy, pakket, licenties, about, glibc, audit) zijn op die commit niet uitgevoerd en laatst gedraaid op 1815bb6 op 2026-11-20. Toolchains: stable rustc 1.99.0 / cargo 1.99.0 en rustc 1.85.1 / cargo 1.85.1 (het gedeclareerde minimum). Linux x86_64. Geen tag, niet gepubliceerd.

## Uitgevoerde controles

Elk commando draaide met `-j 2` en, voor tests, `--test-threads 2`.

| Commando | Exit |
| --- | ---: |
| `cargo fmt --check` | niet uitgevoerd op 6118c69 |
| `cargo clippy --locked --offline --all-targets -- -D warnings` (stable) | niet uitgevoerd op 6118c69 |
| `cargo clippy --offline --all-targets --target x86_64-pc-windows-gnu -- -D warnings` | niet uitgevoerd op 6118c69 |
| `cargo clippy --offline --all-targets --target x86_64-apple-darwin -- -D warnings` | niet uitgevoerd op 6118c69 |
| `cargo +1.85 clippy --locked --offline --all-targets -- -D warnings` | niet uitgevoerd op 6118c69 |
| `cargo test --locked --offline` (debug) | 0 (197 geslaagd) |
| `cargo test --locked --offline --release` | 0 (197 geslaagd) |
| `cargo +1.85 test --locked --offline` (debug) | 0 (197 geslaagd) |
| `cargo +1.85 test --locked --offline --release` | 0 (197 geslaagd) |
| `cargo package --offline --allow-dirty --no-verify`, daarna `cargo test --offline` in de uitgepakte crate | niet uitgevoerd op 6118c69 |
| `cargo deny --locked check licenses` | niet uitgevoerd op 6118c69 |
| `cargo about generate --locked --offline about.hbs` (22 licentiesecties) | niet uitgevoerd op 6118c69 |
| `bash scripts/check-glibc.sh target/release/ebs-wees-bill 2.34` (en `2.30`, dat moet falen) | niet uitgevoerd op 6118c69 |
| `cargo audit --no-fetch` (lokale advisory-database) | niet uitgevoerd op 6118c69 |

## Testresultaat per doel

Lib 85, claims 9, cli 71, docs_clean 4, examples 3, handbook_help 1, json_peak 1, markdown 4, output 6, real_shape 4, schema_version 2, sigint 2, verbosity 4, version 1, doc-tests 0: samen 197 geslaagd, 0 mislukt, 0 genegeerd.

Beperking per systeem: 4 tests in `tests/cli.rs` zijn alleen Unix en 6 alleen Linux; `tests/sigint.rs` (2 tests) en 2 tests van `tests/output.rs` zijn alleen Unix. Een Unix-systeem dat geen Linux is draait er daardoor 189 en een systeem zonder beide 181. Beide zijn op 1815bb6 op 2026-11-20 gemeten (toen 195 tests) en op 6118c69 niet opnieuw geteld (er zijn twee tests bijgekomen, die deze beperking niet raakt), door een kopie te bouwen met de beperking gewijzigd (`target_os = "linux"` vervangen door een naam die geen systeem heeft, daarna ook `cfg(unix)` door `cfg(windows)`) en `cargo test -- --list` te tellen; de tests zijn op die systemen niet gedraaid.

## Niet uitgevoerd

- Een fuzz-campagne en een mutatierun.
- GitHub CI en de releaseworkflow (`ci.yml` en `release.yml`): niet gedraaid wegens het betaalprobleem van het GitHub-account; er is ook geen tag. Dat omvat de CI-taak `glibc` (`scripts/check-glibc.sh` is op deze machine gedraaid) en de rooktest van het archief.
- Tests op Windows en macOS, en `cargo clippy` voor `aarch64-apple-darwin`: niet uitgevoerd. Clippy voor `x86_64-pc-windows-gnu` en `x86_64-apple-darwin` is wel gedraaid (zie de tabel); dat toont dat de code daar compileert en door clippy komt, niet dat de tests slagen. De SIGINT-handler (`src/interrupt.rs`) is alleen Unix, dus Windows bouwt zonder.
- macOS en Windows: geen test is er gedraaid.
- `cargo audit` met een verse advisory-database (`--no-fetch` is gebruikt).
- De prijspagina is niet opnieuw gelezen: de tarieven zijn die van 2026-11-09.
- Een andere libc dan glibc (bijvoorbeeld musl), en de oudste macOS- of Windows-versie die werkt.
- `git push`, `git tag` en `cargo publish`.

## Bekende grenzen

- Deze pagina legt controles vast. Het is geen garantie en geen fuzzrapport.
- "Geen netwerkaanroep" wordt gecontroleerd door `source_does_not_spawn_aws_or_link_an_sdk` (een tekstscan van `src/`) en de afhankelijkhedenlijst. Het enige bestand dat de tool schrijft is het `--output`-doel (en zijn tijdelijke bestand), gedekt door `tests/output.rs`.
- Een gesloten standaarduitvoer wordt alleen op Linux herkend.
- Het geheugengebruik is een veelvoud van de invoergrootte; de gemeten waarden staan in `README.md` en `QUALITY.md`.
- Het genoemde aantal tests is een meting van na de laatste codewijziging; geen test controleert het getal zelf.
- De set escapetekens is een lijst (`src/hidden.rs`), geen Unicode-categorie.
