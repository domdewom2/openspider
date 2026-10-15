# M1-P32: Schreibpfad großer Crawls – Bericht der Kurzproben und des Store-Benchmarks

Paket M1-P32 (Welle 7, Sonnet), Branch `m1/m1-p32`, Messtag 15.10.2026. Maschine: Apple M2, 16 GB RAM, macOS (Darwin 25.6).
Ein Entwicklungs-Lauf der Welle 7 (Messregel: Load je Block im Kopf der `log-*.txt`, `wait_quiet.sh 8 10`, Blöcke unter
`bench.lock` und `caffeinate`, A/B unter gleicher Last); **Absolutwerte kommen aus Welle 7b (M1-N21)**. Die Zahlen hier belegen
die Wirkung der Schritte, nicht die M1-Abnahme.

## Ergebnis in einem Blick

| Ziel auf dem Branch (Prompt) | main (Schritt 0) | Branch (Endstand, `step7a`/`step7b`/`step7c`) | erfüllt |
|---|---|---|---|
| Store-Benchmark, 1 Mio. Seiten: jeder Abschnitt ±30 % des Mittels | 64 % / 169 % (250 000 Seiten) | **97 % / 104 %** (1 Mio., 6 742 Seiten/s, größter Commit 0,79 s) | ja |
| Kurzprobe perf-001 (250 000 Seiten): jeder 10-%-Abschnitt ±30 % des Mittels | 55 % / 425 % | **81 % / 109 %** und **82 % / 115 %** (zwei Läufe) | ja |
| Kurzprobe perf-001: Mittel >= 1 000 URLs/s | 444 URLs/s | **3 863** und **4 180 URLs/s** | ja |
| Längster Stillstand <= 10 s in beiden Kurzproben | perf-003: 5,7 s | perf-003: **1,1 s** und **1,2 s**; perf-001: längster Schreibaufruf ohne Endbatch 1,6 s (Abschluss-Checkpoint); der Endbatch mit dem Indexaufbau dauert 8,0 s (siehe Befund 4) | ja |
| Spitzen-RSS nicht über Ausgangsstand + 10 MB | perf-001 450 MB, perf-003 472 MB | perf-001 **454 / 455 MB**, perf-003 **456 / 455 MB** (Store-Benchmark 359 MiB bei 1 Mio.) | ja |
| `perf_store.rs`, `lifecycle_pause.rs`, `bench_writer.rs` innerhalb ihrer Grenzen | | Tests und Benchmarks des Workspace grün (siehe unten) | ja |

PERF-003 (Spitzen-RSS unter 500 MB bei verlangsamtem Store): perf-003 bleibt mit 455 bis 456 MB darunter (main: 472 MB).

## Messbedingungen

* Release-Build, Profil „Ausgewogen“, `os-loadtest run --scenario perf-001|perf-003 --size large --pages 250000` (8 Abrufe parallel,
  `scripts/bench/segments.sh` liest alle 2 s `SELECT max(rowid) FROM links` der Crawl-Datei und rechnet daraus die gespeicherten
  Seiten und die zehn 10-%-Abschnitte), Store-Benchmark `writer_1m_uniform_segments` (30 gleichverteilte interne Linkziele je Seite,
  Batches wie die Engine mit 2 000 Seiten, alle URLs vorab als `NewUrl`). Crawl-Dateien in `../p32-work` außerhalb des Repos,
  danach gelöscht.
* Jeder Block lief unter `lockf bench.lock`; die Reihen **step4*, step6*, step7*** laufen jeweils innerhalb einer Sperre
  hintereinander (`P32_LOCKED=1`), damit die Varianten unter derselben Last stehen. Parallel liefen die Messläufe von M1-P33
  (nur nacheinander unter derselben Sperre) und Builds anderer Worktrees; die Last schwankte zwischen 2,2 und 7,7 (zu Beginn der Blöcke) (Load je Block in
  der Tabelle und in `log-*.txt`). Zwei gleichartige Läufe streuen um etwa 10 bis 15 %; Unterschiede darunter sind Rauschen.
* Die Ausgangsmessung `step0-main` (Kurzproben) lief bei einem Load von 7,7 bis 6,4, der Store-Lauf von `step0-main` bei 6,7 – die
  Werte von main sind eher zu gut als zu schlecht.
* Die Abschnitte des Store-Benchmarks bei 250 000 Seiten enden an Batch-Grenzen: neun Abschnitte haben 26 000, der letzte 16 000 Seiten.
  Die `store.json` der Läufe bis `step7a` rechnen „zum Mittel“ nach Zeit; die Tabellen unten sind nach Durchsatz umgerechnet
  (Seiten je Abschnitt durch Dauer, geteilt durch das Mittel). Der Benchmark rechnet seit dem Endstand selbst nach Durchsatz
  (`segment_pages` im JSON); bei 1 Mio. Seiten sind alle Abschnitte gleich groß.
* Verworfen: `step3-wal-background` (Kurzprobe lief parallel zu eigenen Tests, ersetzt durch `step3b`), `step5e` (falscher Aufruf).

## Schritte und Wirkung (Messung M nach jedem Schritt)

Stände der Ordner (alle Zahlen: 250 000 Seiten, außer wo „1 Mio.“ steht):

| Ordner | Stand |
|---|---|
| `step0-main` | origin/main (Schritt 0) |
| `step1-links-target` | K-74: `links_target` erst nach Crawlende |
| `step2-url-counters` | K-75: Schema v2, `url_counters` (mit Einfüge-Trigger) |
| `step3b-index-cache` | K-76: Hintergrund-Checkpoint, Indexaufbau mit 32 MiB Cache |
| `step4a-final`, `step4d-final-repeat` | Zählerzeilen vom Writer statt Trigger, Hintergrund-Checkpoint, 32 MiB (Basis der A/B-Reihe K-77) |
| `step4b-page16k`, `step4c-mmap1g` | K-77: `page_size` 16 KiB bzw. `mmap_size` 1 GiB auf der Basis von `step4a` |
| `step5a`…`5c` (1 Mio., Store-Benchmark) | Hintergrund-Checkpoint mit 32 MiB / 4 MiB Indexcache, Checkpoint im Commit (`OS_BENCH_AUTOCHECKPOINT`) |
| `step5d`, `step5f`, `step6a`…`6f` | A/B Hintergrund-Checkpoint gegen Checkpoint im Commit, Kurzproben |
| `step7a`, `step7b` (1 Mio.), `step7c` | **Endstand** des Branches |
| `step8-inlinks-without-index` | Abfragedauer ohne Index (K-74) |

### Store-Benchmark `writer_1m_uniform_segments`

| Schritt | Seiten | Seiten/s | Abschnitte min/max zum Mittel | größter Commit s | Endbatch s (Indexaufbau) | Spitzen-RSS MiB |
|---|---:|---:|---|---:|---:|---:|
| main (`step0`) | 250 000 | 2 254 | 64 % / 169 % | 2,69 | 0,0 | 349 |
| K-74 (`step1`) | 250 000 | 5 276 | 94 % / 105 % | 0,48 | 7,7 | 607 |
| K-75 (`step2`) | 250 000 | 9 259 | 84 % / 112 % | 0,57 | 6,2 | 609 |
| + Hintergrund-Checkpoint, 32 MiB (`step3b`) | 250 000 | 8 176 | 71 % / 113 % | 0,91 | 5,4 | 387 |
| Endstand (`step7a`) | 250 000 | 9 509 | 80 % / 111 % | 0,74 | 3,5 | 356 |
| Hintergrund-Checkpoint, 32 MiB (`step5a`) | 1 000 000 | 4 170 | 92 % / 122 % | 2,78 | 33,9 | 391 |
| Hintergrund-Checkpoint, 4 MiB (`step5b`) | 1 000 000 | 4 506 | 96 % / 106 % | 1,60 | 30,6 | 364 |
| Checkpoint im Commit, 32 MiB (`step5c`) | 1 000 000 | 5 740 | 92 % / 109 % | 0,98 | 35,8 | 387 |
| **Endstand (`step7b`)** | **1 000 000** | **6 742** | **97 % / 104 %** | **0,79** | **32,9** | **359** |
| Endstand, Wiederholung (`step7d`, Benchmark mit Durchsatz-Abschnitten) | 1 000 000 | 6 754 | 97 % / 102 % | 0,72 | 33,1 | 359 |

Abschnittswerte des Endstands bei 1 Mio.: 6 863, 6 861, 6 969, 6 644, 6 493, 6 752, 6 760, 6 624, 6 793, 6 681 Seiten/s
(Abschnitte 1 bis 10); der Durchsatz bleibt über den ganzen Lauf konstant (main fiel bei 250 000 Seiten von 3 810 auf 1 483 Seiten/s).
Die Datei hat bei 1 Mio. Seiten 3 341 MiB. Der Indexaufbau am Ende (33 s) entspricht der Vorprüfung des Planers (37 s).

### Kurzprobe perf-001 (250 000 Seiten, 30 Links je Seite)

| Schritt | URLs/s | Abschnitte (gespeichert) min/max zum Mittel | Spitzen-RSS MB | store_commit_max s | Load beim Start |
|---|---:|---|---:|---:|---|
| main (`step0`) | 444 | 55 % / 425 % | 450 | 10,27 | 7,7 |
| K-74 (`step1`) | 2 522 | 80 % / 123 % | 725 | 2,31 | 3,0 |
| K-75 (`step2`) | 3 176 | 37 % / 151 % | 725 | 0,92 | 4,5 |
| + Hintergrund-Checkpoint, Indexcache 32 MiB (`step3b`) | 2 816 | 37 % / 143 % | 492 | 1,66 | 3,7 |
| Zählerzeilen vom Writer (`step4a`, `step4d`) | 3 227 / 3 613 | 76 % / 120 %, 83 % / 119 % | 492 / 495 | 1,46 / 1,54 | 2,7 / 3,7 |
| **Endstand (`step7a`, `step7c`)** | **3 863 / 4 180** | **81 % / 109 %, 82 % / 115 %** | **454 / 455** | 0,90 / 0,84 | 4,6 / 3,4 |

Der langsame erste Abschnitt von `step2` und `step3b` (37 %) kam vom Einfüge-Trigger der Zählerzeilen (Befund 2).

### Kurzprobe perf-003 (verlangsamter Store, 250 000 Seiten)

| Schritt | URLs/s | Abschnitte min/max zum Mittel | längster Stillstand s | Spitzen-RSS MB | bestanden | Load beim Start |
|---|---:|---|---:|---:|---|---|
| main (`step0`) | 1 300 | 48 % / 182 % | 5,7 | 472 | ja | 6,4 |
| K-74 (`step1`) | 2 361 | 88 % / 112 % | 1,3 | 746 | **nein** (RSS >= 500 MB) | 4,4 |
| K-75 (`step2`) | 2 887 | 41 % / 131 % | 2,3 | 727 | **nein** (RSS >= 500 MB) | 3,5 |
| + Indexcache 32 MiB (`step3b`) | 2 637 | 41 % / 137 % | 2,6 | 492 | ja | 3,8 |
| Zählerzeilen vom Writer (`step4a`) | 3 122 | 86 % / 122 % | 0,8 | 494 | ja | 4,3 |
| **Endstand (`step7a`, `step7c`)** | **3 279 / 3 510** | **93 % / 108 %, 92 % / 107 %** | **1,1 / 1,2** | **456 / 455** | ja | 5,0 / 5,3 |

## Befunde

**1. Der Indexaufbau am Ende hob den Spitzen-RSS (K-74).** `CREATE INDEX` sortiert die 7,5 Mio. Schlüssel mit einem Sortierer, der bis zur
Größe des Seitencaches (256 MiB, „Ausgewogen“) im Speicher bleibt: perf-003 stieg von 472 auf 746 MB und verletzte die Grenze von 500 MB
(`step1`, `step2`). Der Aufbau läuft jetzt mit 4 MiB Seitencache (`link_index::ensure`, danach wird der Cache zurückgesetzt): perf-003 456 MB,
Store-Benchmark 359 MiB bei 1 Mio. Seiten; die Dauer ändert sich kaum (30,6 s gegen 33,9 s mit 32 MiB bei 1 Mio.).

**2. Der Einfüge-Trigger der Zählerzeilen kostete 50 µs je neue URL (K-75).** `urls_counters_insert` (Migration 0002) zwingt SQLite je
eingefügter `urls`-Zeile zu einem Statement-Journal, das bei den vielen Indizes der Tabelle in eine temporäre Datei ausweicht:
100 000 URLs anlegen 5,6 s statt 0,7 s (`OS_BENCH_PAGES=100000`), bei 1 Mio. URLs rund 50 s, und die Entdeckung legt die meisten URLs früh an –
der erste Abschnitt der Kurzproben lag bei 37 % des Mittels. Der Writer entfernt den Trigger vor dem ersten Schreiben und legt die Zählerzeile
mit der URL selbst an; Pause, Ende und Schließen der Senke stellen ihn wieder her (direkte Einfügungen durch Werkzeuge und Tests behalten ihre
Zeile). Wirkung: erster Abschnitt 120 % statt 37 %. Eine abgebrochene Datei (Absturz) hat bis zum nächsten Schreiblauf keinen Einfüge-Trigger;
alle Zählerzeilen sind trotzdem vorhanden (Test `the_writer_creates_the_counter_rows_itself…`).

**3. Der Hintergrund-Checkpoint (K-76) ist langsamer als der Checkpoint im Commit und deshalb nicht der Standard.** Umgesetzt und getestet ist ein
Thread mit eigener Verbindung (`PASSIVE`, `wal_autocheckpoint = 0`, Auslöser nach WAL-Größe oder 5 s, `journal_size_limit`, Diag-Zähler
`bg_checkpoint_*`, `wait_checkpoint_*`). Die Messung (`step5*`, `step6*`) zeigt aber:

| 1 Mio. Seiten, Store-Benchmark | Seiten/s | größter Commit s | Abschnitte min/max | Zeit im COMMIT s je 100 000 Seiten |
|---|---:|---:|---|---:|
| Hintergrund-Checkpoint (`step5a`) | 4 170 | 2,78 | 92 % / 122 % | 11 bis 15 |
| Checkpoint im Commit, Schwelle 256 MiB (`step5c`) | 5 740 | 0,98 | 92 % / 109 % | 6 bis 9 |

| Kurzprobe 250 000 Seiten, gleiche Last, abwechselnd | Hintergrund-Checkpoint | Checkpoint im Commit |
|---|---|---|
| perf-001 URLs/s (`step6a`/`6b`, `6c`/`6d`) | 3 734, 3 343 | 4 164, 3 194 |
| perf-003 URLs/s, längster Stillstand (`step6e`/`6f`) | 3 248, 0,7 s | 3 349, 0,9 s |

Ursache: SQLite setzt das WAL nur zurück, wenn der **nächste Schreibvorgang nach dem Ende eines Durchlaufs beginnt**. Der Writer beginnt
den nächsten Batch unmittelbar nach dem Commit, der Durchlauf läuft dann noch; das WAL wächst, bis der Drosselwert (1 GiB) erreicht ist
(`bg_checkpoint_frames` je Durchlauf rund 262 000 Frames), jeder Commit hängt an eine immer längere Datei an, und die Durchläufe laufen
im Takt von 0,3 s weiter. Der Checkpoint im Commit kostet bei der Schwelle von 256 bis 512 MiB wenig: größter Commit 0,98 s bei 1 Mio.
Seiten (in N14 waren es 16 s, als der Index `links_target` und die breiten Zählerspalten zufällige Seiten dirty machten). Ein Stillstand durch
Checkpoints ist nach K-74/K-75 nicht mehr zu messen. **Entscheidung nach der Regel „Schritt bleibt nur bei besser oder neutral“:** Standard
bleibt der Checkpoint im Commit mit der wachsenden Schwelle; der Hintergrund-Thread bleibt als Option
(`SqliteSink::with_background_checkpoint`, Diagnoseschalter `OS_STORE_BACKGROUND_CHECKPOINT=1`), damit M1-N21 bei 1 Mio. echten URLs
beide Verfahren ohne Codeänderung vergleichen kann. **Befund an den Planer:** K-76 ist mit SQLite-WAL nicht als reiner Gewinn umsetzbar; wenn M1-N21 bei
1 Mio. URLs doch einen Checkpoint-Stillstand zeigt, ist der nächste Schritt eine kleinere feste Schwelle (256 MiB), kein Thread.

**4. Der Endbatch dauert bei 1 Mio. URLs rund 33 s (K-74).** Das Crawl-Ende verschiebt sich um den Indexaufbau (250 000 Seiten: 8,0 s,
`engine_w_write_max_ns` 8,3 s; 1 Mio.: 32,9 s). Er liegt hinter dem letzten abgerufenen URL; ein Fortschrittsmesser, der nur „steigende Zahl
abgerufener URLs“ zählt, sieht ihn nicht als Stillstand, ein Messer des Zeitraums bis zum Zustand `completed` schon (M1-N19/N21 beachten).

**5. Der Writer ist jetzt rechengebunden.** `store_build_ns` (Ausführung der Einfügungen im Writer) macht den größten Teil
der Schreibzeit aus (41 s von 61 s bei `step2`), `store_commit_ns` nur noch 10 s; weitere Beschleunigung liegt im Batch-Aufbau (Einfügen von 30 Links je Seite), nicht im Commit.

## A/B `page_size` und `mmap_size` (K-77): nicht übernommen

Basis `step4a`/`step4d` (zwei Läufe), je Variante Store-Benchmark (250 000 Seiten) und perf-001, unmittelbar hintereinander unter einer Sperre.

| Variante | Store Seiten/s | Store RSS MiB | perf-001 URLs/s | perf-001 RSS MB |
|---|---:|---:|---:|---:|
| Basis (`step4a` / `step4d`) | 8 168 / 8 066 | 387 / 386 | 3 227 / 3 613 | 492 / 495 |
| `page_size` 16 KiB (`step4b`) | 8 828 (+8 %) | 405 | 4 039 (+18 % zum Mittel der Basis) | **558 (+65)** |
| `mmap_size` 1 GiB (`step4c`) | 8 334 (+3 %) | 387 | 3 690 (+8 %) | **509 (+15)** |

Regel: übernehmen nur bei >= 10 % mehr Durchsatz **ohne** höheren Spitzen-RSS. 16 KiB bringt im Store-Benchmark 8 %, in der Kurzprobe 18 % bei
65 MB mehr Spitzen-RSS; `mmap` liegt im Rauschen (zwei Basisläufe streuen um 12 %) und kostet 15 MB. Beides wird nicht übernommen
(Seitengröße bleibt 4 KiB, kein `mmap`).

## Schritte der Umsetzung, Inventur, Migration, Leser

* **K-74** `links_target` erst nach Crawlende: `os_store::link_index::{present, drop_index, ensure, ensure_if_idle}`; der Writer entfernt den Index
  beim Öffnen und baut ihn im letzten Batch (`state = completed`), Ereignis `links_index_built`. Tabelle der Inventur aller Abfragen nach
  `links.target_id` mit dem Verhalten ohne Index: `docs/ARCHITECTURE.md`, Abschnitt „Crawl-Datei und Speicher“. Tests: Index fehlt während des
  Laufs und nach Pause, ist nach dem Ende da; jede Abfrage der Inventur liefert mit und ohne Index dieselben Zeilen
  (`crates/os-store/tests/link_index.rs`, `queries_links.rs`, os-export, os-engine).
* **K-75** `0002_url_counters.sql` (`SCHEMA_VERSION` 2). Migration einer v1-Datei übernimmt die zehn Zähler aus `urls` und berechnet `html_2xx_sources`
  und `image_link_inlinks` aus `links`/`urls`; Sicherungskopie `<name>.v1.bak.ospider` wie bisher; ReadOnly-Öffnen einer v1-Datei liefert
  `MigrationRequired`. Fixtures `tests/fixtures/crawl-files/{corrupt,schema-v2,schema-v99}.ospider` mit `UPDATE_FIXTURES=1` neu erzeugt, `schema-v1` bleibt.
  Geänderte Leser: os-analyze (Tabs internal, links, images, external, response_codes, views), os-export (Redirect-Report, `links_export`),
  os-mcp (`url_details`), os-engine (`rescan.rs`, Tests `lifecycle_rescan`, `lifecycle_sitemaps`, `core_limits`), os-store (`counters`, `queries`,
  Tests). `os-engine/src/resume.rs` (M1-P33) liest keine Zähler aus `urls`; die Zählerspalten in `urls` werden nicht mehr gepflegt.
* **K-76** siehe Befund 3. **K-77** siehe oben.
* Keine Golden-Änderung, keine Paritätserwartung geändert; Gegenprobe `os-parity run --all` (5 von 5 Sites) endet mit Exit 0.
* `os-model`, `Cargo.toml`, `Cargo.lock` unverändert; öffentliche Signaturen nur additiv (`link_index`, `with_background_checkpoint`,
  `diag`-Zähler).
* Debug-Test `crates/os-export/tests/zz_debug.rs` einer früheren Instanz entfernt.

## Abfragedauer ohne Index (K-74)

Benchmark `inlinks_without_index_250k_pages` (`step8-inlinks-without-index/`, 250 000 Seiten x 30 Links, `queries::inlinks` = `links_page` In, 50 Zeilen, Load 3,4):

| | Dauer |
|---|---:|
| ohne Index `links_target` (ein Lauf über `links`, Mittel aus 4 Zielen: 0,22 / 0,24 / 0,26 / 0,22 s) | **0,24 s** |
| mit Index | 0,0003 s |
| Hochrechnung 1 Mio. Seiten (x4, Seitencache warm) | ohne Index rund 0,9 s |
| Indexaufbau (`link_index::ensure`) | 3,2 s bei 250 000, hochgerechnet 13 s; gemessen im Endbatch des Store-Benchmarks bei 1 Mio.: 33 s |

Die Inlinks einer URL während des Crawls kosten damit einen Lauf über 7,5 Mio. Links (unter einer Sekunde bei warmem Cache); kein Pfad liest `links` je Ergebniszeile.

## Prüfungen des Branches

`cargo fmt --all --check`, `cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings` und
`cargo test --workspace --exclude openspider-desktop --locked --no-fail-fast` (210 Testprogramme) sind grün. `cargo deny check`: bans, licenses, sources ok;
**advisories meldet RUSTSEC-2026-0346 für `rustls 0.23.45`** (Cargo.lock, nicht im Besitz dieses Pakets) – Bedarf an den Orchestrator.

CI-Lauf per `workflow_dispatch` auf `m1/m1-p32` (Commit vor diesem Bericht-Nachtrag): [38770592004](https://github.com/domdewom2/openspider/actions/runs/38770592004),
alle 16 Jobs grün (build auf macos-arm64, macos-x64, windows-x64; parity; spec-coverage; loadtest-small; crawlfile create/load; cargo-deny; rustfmt). Zwei frühere
Läufe scheiterten nur auf windows-x64: ein Test des Hintergrund-Checkpoints erwartete eine WAL-Größenschranke (194 MiB, siehe Befund 3), und ein leerer Batch dauerte
durch das Entfernen des Triggers 40 ms (der Trigger wird jetzt nur vor Batches mit neuen URLs entfernt).

## Dateien

`step*/log-*.txt` Rohprotokolle mit Load je Block, `store.json`, `perf-00?.json` Berichte des Lasttests, `perf-00?-segments.{txt,json}` Abschnitte
der gespeicherten Seiten, `summary.json` Kurzfassung je Schritt (`scripts/bench/p32_summary.py`); Tabellen erzeugt mit `scripts/bench/p32_report.py`.
Target-Größe nach dem Bericht: debug 15 GB, release 0,9 GB (release nach dem Bericht gelöscht).
