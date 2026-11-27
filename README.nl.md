# ebs-orphan-bill

[![crates.io](https://img.shields.io/crates/v/ebs-orphan-bill.svg)](https://crates.io/crates/ebs-orphan-bill) [![downloads](https://img.shields.io/crates/d/ebs-orphan-bill.svg)](https://crates.io/crates/ebs-orphan-bill) [![CI](https://github.com/qwpp7g79y4-maker/ebs-orphan-bill/actions/workflows/ci.yml/badge.svg)](https://github.com/qwpp7g79y4-maker/ebs-orphan-bill/actions/workflows/ci.yml) [![MSRV 1.85](https://img.shields.io/badge/MSRV-1.85-blue.svg)](https://github.com/qwpp7g79y4-maker/ebs-orphan-bill/blob/main/Cargo.toml) [![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/qwpp7g79y4-maker/ebs-orphan-bill#license)

Vind losse EBS-volumes en snapshots die ouder zijn dan N dagen en geen AMI hebben, met de maandkosten van elk. Leest alleen bestanden. Geen netwerk.

Dit is de Nederlandse versie van `README.md`; de Engelse tekst is leidend.

## Waarom

Een losgekoppeld volume en een snapshot die geen image gebruikt blijven kosten, ook als de instance weg is. `ebs-orphan-bill` leest JSON die je al hebt opgeslagen van `aws ec2 describe-volumes`, `aws ec2 describe-snapshots` en `aws ec2 describe-images`. Het toont de losse volumes en de oude standaardsnapshots, met een maandbedrag in USD voor elke geprijsde regel.

Het rapport vindt wezen en maandkosten. Het verwijdert geen volumes of snapshots. Het bedrag is een schatting uit lijstprijzen, niet uit je factuur.

## Kwaliteit

`cargo test --locked` slaagt voor 198 tests en faalt voor 0 op Linux, in debug en in release, op stable en op Rust 1.85. Sommige tests draaien alleen op Linux of Unix, dus op macOS en Windows is het aantal lager. Zie `QUALITY.md`.

## Installatie

Bouw vanuit de broncode. Rust 1.85 of nieuwer. Vanuit een kloon van deze crate:

```sh
cargo install --locked --path .
ebs-orphan-bill --help
```

De binary heet `ebs-orphan-bill`. Er is geen download: bouw vanuit de broncode. Een binary die op Linux is gebouwd heeft de glibc nodig van de machine die hem bouwde; de releasebuild voor deze pagina had glibc 2.34 nodig, en een CI-taak faalt als het hoogste glibc-symbool van een releasebuild nieuwer is. Andere libc-builds (bijvoorbeeld musl) en de oudste werkende macOS- of Windows-versie zijn niet gemeten. macOS- en Windows-builds worden alleen door compileren gecontroleerd (zie `QUALITY.md`).

## Snelstart

De voorbeeldbestanden zijn `examples/volumes.json`, `examples/snapshots.json` en `examples/images.json`. Het commando roept AWS niet aan. `--older-than-days` is standaard 30. Het voorbeeldbestand met snapshots heeft een `NextToken`; het rapport zegt dan dat de pagina onvolledig kan zijn. De waarde van het token wordt niet getoond.

<!-- quick-start:start -->
```console
$ ebs-orphan-bill --as-of 2026-11-09 --older-than-days 30 examples/volumes.json examples/snapshots.json examples/images.json
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

1. Bouw de tool: `cargo install --locked --path .` (Rust 1.85 of nieuwer). Controleer met `ebs-orphan-bill --version`.
2. Sla je eigen exports op (het commando leest alleen bestanden en roept AWS nooit aan): `aws ec2 describe-volumes > volumes.json`, `aws ec2 describe-snapshots --owner-ids self > snapshots.json` en `aws ec2 describe-images --owners self > images.json`.
3. Draai `ebs-orphan-bill --as-of "$(date -u +%F)" volumes.json snapshots.json images.json`. Zonder `--as-of` worden de volumes geprijsd en de snapshots niet beoordeeld.
4. Lees de laatste regels: `estimated_usd_month=` is de schatting voor de losse volumes en oude snapshots, uit lijstprijzen en niet uit je factuur.
5. Gebruik het in een script: `--fail-usd 50` geeft exit 1 als één wees meer dan 50 USD per maand kost, `--json` en `--md` veranderen het formaat en `--output rapport.md` schrijft een bestand.

## Opties

```text
ebs-orphan-bill [OPTIES] <BESTAND>...

  <BESTAND>...              describe-volumes-, describe-snapshots- of describe-images-JSON: bestanden, gzip, mappen, of - voor standaardinvoer
  --prices <BESTAND>        Prijsbestand. Standaard de ingebouwde US East (N. Virginia) EBS-lijst
  --as-of <JJJJ-MM-DD>      Datum waarop leeftijden worden gemeten, middernacht UTC. Zonder deze optie worden snapshots niet beoordeeld
  --older-than-days <DAGEN> Een snapshot is oud als zijn leeftijd strikt groter is dan dit aantal dagen [standaard: 30]
  --fail-usd <BEDRAG>       Exit 1 als de exacte maandprijs in USD van een geprijsde wees strikt groter is
  --json                    Toon JSON in plaats van de teksttabel
  --md                      Toon een Markdown-rapport (tabellen) in plaats van de teksttabel
  --output <BESTAND>        Schrijf het rapport naar dit bestand in plaats van naar standaarduitvoer
  --quiet                   Verberg de waarschuwingen; toon alleen het resultaat en fouten
  --verbose                 Toon voortgang op standaardfout
  -h, --help                Toon hulp
  -V, --version             Toon de versie
```

`--version` toont de crateversie. Een debugbuild uit een git-kloon toont ook de commit, als `ebs-orphan-bill 1.0.2 (<sha van 40 tekens>)`; een releasebuild, of een uitgepakt pakket zonder git, toont alleen de versie. `--help` en `man/ebs-orphan-bill.1` bevatten de volledige (Engelse) tekst. De opmerkingen onder "Hoe het telt" leggen elke optie uitgebreider uit.

## Voorbeeld

`vol-11111111111111111` is een los gp3-volume van 100 GB met 4000 IOPS en 200 MB/s. Opslag is `100 * 0.08 = 8.00`. IOPS boven 3000 zijn `(4000 - 3000) * 0.005 = 5.00`. Doorvoer boven 125 MB/s is `(200 - 125) * 0.04 = 3.00`. De regel is 16.00 USD per maand.

`vol-22222222222222222` is een los gp2-volume van 50 GB. Opslag is `50 * 0.10 = 5.00`. De gp2-I/O zit in die opslagprijs.

`vol-33333333333333333` is gekoppeld en dus geen wees. Zijn instance-id wordt niet getoond.

`vol-44444444444444444` is een los magnetisch `standard`-volume van 20 GB. Dat tarief van de vorige generatie (0.05 USD per GB-maand) wordt niet toegepast, dus de regel is ongeprijsd. `20 * 0.05 = 1.00` is niet het bedrag.

`snap-11111111111111111` begon op `2026-10-09T00:00:00.000Z`. Op 2026-11-09 is dat 31 dagen, strikt meer dan 30. Geen image verwijst ernaar. `VolumeSize` is 10 GB en `FullSnapshotSizeInBytes` ontbreekt, dus de basis is `volume-size` en de kosten zijn `10 * 0.05 = 0.50`.

`snap-22222222222222222` is oud en standaard, maar `ami-11111111111111111` verwijst ernaar in `BlockDeviceMappings[].Ebs.SnapshotId`. Het is geen wees. Een beschrijving die CreateImage noemt is geen AMI-koppeling en wordt niet getoond.

`snap-33333333333333333` begon op `2026-10-10T00:00:00.000Z`. Dat is precies 30 dagen vóór 2026-11-09, dus niet ouder dan 30. `2026-10-09T23:59:59Z` is 30 dagen plus één seconde en telt wel.

`snap-44444444444444444` is een archiefsnapshot van 80 GB en oud genoeg. Archiefopslag (0.0125 USD per GB-maand) wordt niet toegepast. `80 * 0.0125 = 1.00` is niet het bedrag.

De geprijsde volumewezen tellen op tot `16.00 + 5.00 = 21.00`. Het totaal van de geprijsde snapshots is 0.50. Het eindtotaal is 21.50 USD per maand, getoond als `estimated_usd_month=21.50`. Elk rapport zegt dat het een schatting is uit lijstprijzen: de tekst heeft één regel `Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)` en de JSON heeft `"estimate": true` op het hoogste niveau (met dezelfde zin in `estimate_note`). De totalen heten in JSON `estimated_usd_month`, `volume_estimated_usd_month` en `snapshot_estimated_usd_month`.

```sh
ebs-orphan-bill --as-of 2026-11-09 --fail-usd 16 examples/volumes.json examples/snapshots.json examples/images.json
ebs-orphan-bill --as-of 2026-11-09 --fail-usd 15.99 --json examples/volumes.json examples/snapshots.json examples/images.json
```

`--fail-usd 16` raakt de gp3-regel, dus dat commando geeft exit 0. `--fail-usd 15.99` geeft exit 1. Een gekoppeld volume, een jonge snapshot, een AMI-snapshot, een archiefsnapshot en een ongeprijsd magnetisch volume halen de drempel niet. `--json` toont één JSON-object op standaarduitvoer. Waarschuwingen gaan in JSON-modus naar standaardfout en staan ook in de `warnings`-lijst van het object. De tekstmodus toont waarschuwingen op standaarduitvoer.

## Afsluitcodes

| Code | Betekenis |
| ---: | --- |
| 0 | Een geprijsd rapport is geschreven. |
| 1 | `--fail-usd` is gezet en de exacte maandprijs in USD van een geprijsde wees is strikt groter. |
| 2 | Slechte invoer, verkeerd gebruik, een standaarduitvoer die niet te schrijven is, of een `--output`-bestand dat niet te schrijven is. |
| 130 | SIGINT (Ctrl-C) op Unix. Er staat niets op standaarduitvoer en er blijft geen half bestand achter. |

`--help` en `--version` geven exit 0. Een gebruiksfout van clap geeft exit 2.

## Hoe het telt

Tarieven zijn de rijen US East (N. Virginia) op <https://aws.amazon.com/ebs/pricing/>, gelezen op 2026-11-09. Het prijsbestand dat die pagina laadt is gepubliceerd op 2026-11-07. De ingebouwde prijsopgave is `prices/ebs.json`.

- **Gigabyte.** GB = 1024^3 bytes. `Size` en `VolumeSize` zijn al die GB. `FullSnapshotSizeInBytes / 1024^3` is het GB-getal, als Decimal.
- **gp3.** Opslag 0.08 USD per GB-maand. IOPS 0.005 USD per IOPS-maand boven de basislijn van 3000. Doorvoer 40.96 USD per GiBps-maand, gedeeld door 1024: 0.04 USD per MB/s-maand boven 125 MB/s. Een ontbrekende `Iops` of `Throughput` gebruikt de basislijn; een waarde eronder heeft ook extra 0. Een gp3 van 2000 GB, 10000 IOPS en 500 MB/s kost een hele maand `160.00 + 35.00 + 15.00 = 210.00` USD. Het 0.06-voorbeeld van de pagina wordt niet gebruikt en de 12-uursvoorbeelden niet.
- **gp2.** 0.10 USD per GB-maand; I/O zit erin. 2000 GB is 200.00 USD.
- **io1.** Opslag 0.125, IOPS 0.065 per IOPS-maand. 2000 GB en 1000 IOPS is `250.00 + 65.00 = 315.00` USD. Een ontbrekende `Iops` prijst alleen de opslag en waarschuwt.
- **io2.** Opslag 0.125. IOPS zijn gelaagd per volume, niet per account: tot 32000 à 0.065, tot 64000 à 0.0455, daarboven 0.03185. 32000 IOPS is 2080.00; 32001 is 2080.0455 (getoond 2080.05); 60000 is 3354.00; 64000 is 3536.00; 64001 is 3536.03185 (getoond 3536.03). Tien volumes van 1000 IOPS zijn niet één volume van 10000 IOPS.
- **st1 en sc1.** 0.045 en 0.015 USD per GB-maand. 2000 GB is 90.00 en 30.00 USD.
- **Standaardsnapshots.** 0.05 USD per GB-maand. Een ontbrekende `StorageTier` telt als standaard. De factureerbare grootte zijn gewijzigde blokken, die `describe-snapshots` niet geeft. `FullSnapshotSizeInBytes` wordt gebruikt als die er is (basis `full-snapshot-bytes`), anders `VolumeSize` (basis `volume-size`). Beide zijn een bovengrens, behalve bij een eerste volledige snapshot.
- **Niet geprijsd.** Magnetische standaardvolumes (0.05), archiefopslag van snapshots (0.0125) en archief-ophalen (0.03), fast snapshot restore, directe API's, time-based copy, volume-initialisatie en volumeklonen. De tool toont daarover dezelfde regels als in de Engelse README.
- **Gratis laag.** De pagina rekent 30 GB opslag, 2 miljoen I/O's en 1 GB snapshotopslag voor het account. Deze export is niet het hele account, dus de korting wordt niet toegepast. Een volume van 30 GB gp2 is `30 * 0.10 = 3.00` USD, niet gratis.
- **Los volume.** `Attachments` ontbreekt of is leeg. Een niet-lege lijst (ook `detaching`) is gekoppeld en geen wees. `State` `in-use` met lege koppelingen is nog steeds los; het rapport waarschuwt.
- **Oude snapshot.** `StorageTier` `standard` of ontbrekend, leeftijd strikt groter dan `--older-than-days` (standaard 30) en door geen enkel image genoemd in `BlockDeviceMappings[].Ebs.SnapshotId`. Tekst met `ami-` in de beschrijving is geen koppeling. Zonder `describe-images`-document zijn snapshots geen wezen. `{"Images":[]}` betekent dat de images zijn gecontroleerd en geen snapshot noemen. `pending` en `error` worden met een waarschuwing overgeslagen. Archief en andere lagen worden getoond als ze anders wees zouden zijn, en niet geprijsd.
- **Leeftijd.** Met `--as-of JJJJ-MM-DD` is de leeftijd het aantal milliseconden van `StartTime` tot die datum om middernacht UTC. `StartTime` moet UTC zijn (`Z` of `+00:00`), met optionele fractie, jaren 1970 tot en met 2199, zonder schrikkelseconde. Gelijke leeftijd telt niet. Een starttijd na `--as-of` is een fout (exit 2). Zonder `--as-of` worden volumes geprijsd en zijn snapshots geen wezen.
- **Prijsleeftijd.** Ligt de `date` van het prijsbestand (ingebouwd of `--prices`) meer dan 180 dagen vóór `--as-of`, of vóór vandaag zonder `--as-of`, dan waarschuwt het rapport met die datum. De waarschuwing staat bij de andere waarschuwingen en gaat in elke modus ook naar standaardfout, behalve met `--quiet`. De exitcode blijft 0. Precies 180 dagen waarschuwt niet. Elke geldige datum die oud genoeg is waarschuwt, ook een datum voor 1970 zoals `1900-01-01`.
- **Weergave.** USD wordt één keer, half weg van nul, op 2 decimalen afgerond. Een regel en het totaal zijn de exacte som van de onafgeronde producten; die exacte som wordt afgerond. Past het exacte bedrag niet in de accumulator, dan geeft het commando exit 2 met `amount overflowed`. De drempel vergelijkt het exacte maandbedrag vóór deze afronding; een regel die 0.00 toont kan dus een drempel van 0 overschrijden.
- **Drempel.** `--fail-usd BEDRAG` geeft exit 1 als een geprijsde wees strikt meer kost dan `BEDRAG`. Gelijk faalt niet. `BEDRAG` is gewone cijfers met een optionele fractie, zonder teken of exponent, met hoogstens 28 decimalen en niet meer cijfers dan een decimal exact houdt (het grootste is 79228162514264337593543950335), anders exit 2. De tekst is hoogstens 40 tekens; decimalen tellen met hun nullen.
- **Uitvoerbestand.** `--output BESTAND` schrijft het rapport (tekst, `--json` of `--md`) naar een tijdelijk bestand in dezelfde map en hernoemt dat over `BESTAND`: het bestand is het oude of het hele nieuwe. Een bestaand gewoon bestand wordt vervangen. Een symlink wordt geweigerd en niet gevolgd, net als een map of een ander bestand dat niet gewoon is. Een ontbrekende bovenliggende map wordt niet aangemaakt. Een schrijffout geeft exit 2.
- **Markdown.** `--md` toont een kop, de schattingsregel, de notities als lijst, een lijst `## Warnings` (niet met `--quiet`), een tabel voor volumes en een voor snapshots, en de totalen als `estimated_usd_month=21.50`. De bedragen zijn dezelfde teksten als in tekst en JSON. Een `|` in een cel wordt `\|`; regeleinden en verborgen tekens worden `\u{..}`-escapes. `--md` met `--json` is een gebruiksfout (exit 2).
- **SIGINT.** Op Unix beëindigt Ctrl-C (SIGINT) het commando met exit 130 en de regel `ebs-orphan-bill: interrupted by SIGINT` op standaardfout. Vóór het schrijven van het rapport is er niets naar standaarduitvoer gegaan; een SIGINT tijdens dat ene schrijven laat het schrijven afmaken en geeft dan exit 130. Windows heeft zo'n handler niet.
- **JSON-schemaversie.** Elk `--json`-rapport begint met `"schema_version": 1`, een getal, als eerste sleutel. De tool heeft geen JSON-foutuitvoer: een mislukte run (exit 2) toont niets op standaarduitvoer.
- **Stil en uitgebreid.** `--quiet` verbergt de waarschuwingen en toont alleen het resultaat en fouten; de JSON-lijst `warnings` blijft en de exitcode verandert niet. `--verbose` voegt per invoer een regel `reading <bestand>` toe en één regel `context:` met aantallen op standaardfout; standaarduitvoer blijft gelijk. Samen zijn ze een gebruiksfout (exit 2).
- **Invoer.** Een bestand, één gzip-member (herkend aan de bytes `1f 8b`), een map (recursief, hoogstens 64 niveaus; slaat symlinks en andere niet-gewone bestanden over met een waarschuwing) of `-` voor standaardinvoer. Een bestand dat geen JSON is, is een fout, en één slecht bestand in een map stopt de hele run met exit 2. Eén bestand is één soort: volumes, snapshots of images. Dubbele id's zijn een fout. `nextToken` wordt niet gevolgd en zijn waarde wordt niet getoond. Eigenaar-id's, KMS-sleutel-id's, instance-id's en beschrijvingen worden niet getoond. Regeleinden, stuurtekens, onzichtbare tekens en richtingtekens in bestandsnamen en in argumenten die clap herhaalt worden als `\u{..}`-escapes op één regel getoond.
- **Invoertolerantie.** Een getal mag een JSON-string zijn: `"Size":"100"`. `VolumeType` wordt zonder hoofdlettergevoeligheid vergeleken: `"GP2"` is gp2. De `currency` van het prijsbestand wordt getoond en niet gecontroleerd. `--older-than-days` accepteert een voorloop-plus (`+5`).

## Beperkingen

- Offline en alleen-lezen. Geen AWS-SDK en geen netwerkaanroep. Niets in deze crate draait `aws`. Het rapport verwijdert geen volumes of snapshots.
- Prijzen zijn de us-east-1-tarieven van de pagina hierboven, voor een hele maand van de ingestelde grootte. Andere regio's verschillen: geef `--prices` met je eigen opgave in dezelfde vorm. De tool downloadt geen prijzen. Echte facturen zijn per seconde met een minimum van 60 seconden.
- Geen contractkorting en geen gratis-laagkorting. De korting van 30 GB, 2 miljoen I/O's en 1 GB snapshots geldt voor het account en één export is niet het hele account.
- Magnetische `standard`-volumes en archiefsnapshots zijn niet geprijsd. Fast snapshot restore, directe API's, time-based copy, volume-initialisatie en volumeklonen ook niet.
- De snapshotprijs is een bovengrens. Gewijzigde blokken staan niet in `describe-snapshots`.
- Een AMI-koppeling is alleen `describe-images` `BlockDeviceMappings[].Ebs.SnapshotId`. Zonder dat document zijn snapshots geen wezen.
- Een gedeeltelijke pagina heeft een `nextToken`. Het token wordt niet gevolgd; het rapport waarschuwt dat de pagina onvolledig kan zijn.
- USD per regel wordt apart afgerond. Een totaal kan minder dan één cent per regel afwijken van de som van de getoonde regels. `--fail-usd` gebruikt het exacte bedrag.
- Eén invoerbestand is hoogstens 268435456 bytes, ook na uitpakken. Een document heeft hoogstens 16777216 JSON-waarden plus objectsleutels, anders exit 2. Een bestand wordt geheel gelezen en in het geheugen geparsed, dus het geheugengebruik is een veelvoud van de bestandsgrootte (gemeten waarden en tijden staan in `README.md`). Splits een zeer grote export.
- Een gesloten standaarduitvoer (`>&-`) wordt alleen op Linux herkend (exit 2). Een volle of kapotte standaarduitvoer (`>/dev/full`, een gesloten pipe) geeft exit 2, gemeten op Linux.
- Het totaal aantal tests is een Linux-getal. Tests voor symlinks, FIFO's en SIGINT draaien alleen op Unix; tests voor `/dev/full`, een gesloten descriptor en bestandsnamen met stuurtekens of zonder UTF-8 alleen op Linux.

## Prijzen en verantwoordelijkheid

De ingebouwde prijzen zijn een momentopname: de us-east-1-lijstprijzen van <https://aws.amazon.com/ebs/pricing/>, met datum 2026-11-09 (de `date` in `prices/ebs.json`). De tool werkt ze niet bij. Controleer zelf de actuele prijzen voordat je op een bedrag vertrouwt en geef met `--prices <FILE>` je eigen prijsbestand mee als ze afwijken. Je bent zelf verantwoordelijk voor elke beslissing op basis van de uitvoer; het rapport is een schatting, geen factuur.

## Onderhoud

Meld bugs via GitHub Issues van deze repository. Na een release staat de tool in sluimermodus: een nieuwe versie komt alleen bij een gemelde bug, een aanvraag of een beveiligingsprobleem, en beveiliging gaat altijd voor.

## Ondersteuning

Vragen, bugmeldingen en ideeën: mail pep_ai@icloud.com, of open een issue in de repository. Lees bij een beveiligingsprobleem eerst `SECURITY.md`.

## Licentie

Beschikbaar onder een van beide licenties, naar keuze:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT-licentie ([LICENSE-MIT](LICENSE-MIT))

Copyright 2026 PEPAI. MIT OR Apache-2.0.

De licenties van de externe crates in het programma staan in [THIRD-PARTY-LICENSES.html](THIRD-PARTY-LICENSES.html).

## Contact

- E-mail: pep_ai@icloud.com
- Web: https://www.pepai.nl
- X: https://x.com/pepai_nl
- Ko-fi: https://ko-fi.com/pepai
