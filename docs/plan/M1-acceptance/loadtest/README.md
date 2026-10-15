# Großläufe (M1-N14, A4)

Abnahmemessung der Kriterien PERF-001, PERF-003, PERF-005 und CRAWL-124 (Profilvergleich) auf der lokalen Referenzmaschine. Benchmark-Ignores und die Kriterien CRAWL-126, CRAWL-127 und OUT-001: siehe [`../benchmarks/README.md`](../benchmarks/README.md).

## Ergebnis in Kürze

| Kriterium (M1.md Abschnitt 6, A4) | Grenze | Messwert | bestanden |
|---|---|---|---|
| PERF-001: Spitzen-RSS bei 1 Mio. URLs x 30 Links | < 1,5 GB | **720,8 MB** | ja |
| PERF-001: RSS(1 Mio.) / RSS(100 k) | < 2 | **1,69** (720,8 MB / 425,9 MB) | ja |
| PERF-001: Cache-Grenze K-16 über die PRAGMA-Werte | Balanced 256 MiB; MaxSpeed <= min(25 % RAM, 4 GiB) | Balanced 268 435 456 B; MaxSpeed -4 194 304 KiB (= 4 GiB = 25 % von 16 GiB) | ja |
| PERF-003: Spitzen-RSS bei verlangsamtem Store (SlowSink 100 ms) | < 500 MB | **707,6 MB** | **nein** |
| PERF-003: Crawl läuft weiter und endet regulär | `end_reason = queue_empty`, 1 Mio. URLs | ja (`queue_empty`, 1 000 000 URLs) | ja |
| PERF-003: längster Stillstand (Lasttest-Konstante `MAX_STALL_SECS`) | <= 30 s | **31,3 s** | **nein** (knapp) |
| PERF-005: 10 harte Abbrüche, danach `integrity_check = ok` | je Abbruch ok | 10 von 10 ok | ja |
| PERF-005: URL-Menge und Status je URL = Referenz, keine Dublette in `urls` | identisch, 0 Dubletten | identisch (100 001 URLs, 0 fehlend, 0 zusätzlich, 0 abweichender Status), 0 Dubletten | ja |
| PERF-005: wiederholte Requests je Abbruch (K-49) | <= 2 264 (8 + 256 + 2 000) | Mittel 2 171,1 (10-Abbrüche-Lauf); Maximum 2 264 (Einzelabbrüche); nie überschritten | ja |
| PERF-005: Vorkommen von „abgeschnitten“ in Bericht und Log | 0 | 0 | ja |
| CRAWL-124: MinMemory-RSS mindestens 20 % unter Balanced (250 000 Seiten, Begründung unten) | <= 80 % | **-53,8 %** (216,4 MB gegen 468,5 MB) | ja |
| CRAWL-126: Pausieren mit 1 Mio. Warteschlangeneinträgen | < 3 s | 71 ms (Fortsetzen: Wiederaufbau 1,23 s) | ja |
| CRAWL-127: Öffnen von 1 Mio. URLs / Speichern unter | < 3 s / < 60 s | 4 ms / 5,7 s | ja |
| OUT-001: Export 1 Mio. x 60 Spalten | Speicherzuwachs <= 300 MB | 1 MiB | ja |

**A4 ist nicht erfüllt:** PERF-003 bei 1 Mio. URLs verletzt die RSS-Grenze (siehe Nachbesserungsauftrag). Alle übrigen Kriterien sind erfüllt.

## Nachbesserungsauftrag (keine Codeänderung in diesem Paket)

**1. PERF-003, Spitzen-RSS 707,6 MB >= 500 MB bei 1 Mio. URLs (SlowSink 100 ms je Schreibaufruf).**
- Crate und Dateien: `os-engine` (`crates/os-engine/src/writer_thread.rs`: `BATCH_PAGES_BACKLOG` = 2 000, `CHANNEL_CAPACITY` = 256) und `os-store` (`crates/os-store/src/profile.rs`: Balanced-Cache 256 MiB; `writer.rs`: WAL-Checkpoint).
- Messwert: `crawler_peak_rss_bytes` = 707 641 344 (`report-perf-003-large.json`).
- Befund: Der RSS wächst mit der URL-Zahl, nicht mit der SlowSink-Verzögerung. Gleiche Konfiguration, gleiche Binärdatei: 100 000 Seiten 426,5 MB, 250 000 Seiten 470,8 MB (beide bestanden), 1 000 000 Seiten 707,6 MB. PERF-001 ohne SlowSink erreicht bei 1 Mio. 720,8 MB. Die Grundlast liegt also bei rund 400 MB, und ab etwa 0,5 Mio. URLs wirken die mit der URL-Zahl wachsenden Strukturen im Prozess (Vermutung: Seen-Menge und Warteschlange im Arbeitsspeicher, nicht gemessen).
- Befehl zum Nachmessen: `cargo run --release --locked -p os-loadtest -- run --scenario perf-003 --size large --work-dir <ordner> --out <datei>.json` (Dauer rund 2 Stunden; Kurzprobe mit `--pages 250000` rund 3 Minuten, Grenzwert dort aber noch nicht erreicht).
- Offene Spezifikationsfrage für den Planer: `docs/spec/07-performance.md` PERF-003 nennt keine URL-Zahl (A4 ebenso); das Szenario `perf-003` wählt für `large` 1 Mio. Wenn die Grenze 500 MB für kleinere Crawls gemeint ist, ist sie mit 426 MB (100 000) und 470 MB (250 000) erfüllt.

**2. PERF-003, Stillstand 31,3 s > 30 s (`MAX_STALL_SECS` in `crates/os-loadtest/src/engine.rs`).**
- Das ist eine Konstante des Lasttests, die „Crawl läuft weiter“ übersetzt; die Spec nennt keine Sekundenzahl. Der Crawl lief weiter und endete regulär.
- Messwert: `crawler_max_stall_s` = 31,302. Ursache: ein einzelner Schreibaufruf des Writers dauerte 31,39 s (`engine_w_write_max_ns`), darin ein Commit von 20,40 s (`store_commit_max_ns`) und ein WAL-Checkpoint von 15,99 s (`store_checkpoint_max_ns`). In PERF-001 (ohne SlowSink) sind es 31,95 s bzw. 19,31 s und 16,80 s, es ist also keine Folge der SlowSink.
- Zusammenhang mit der Beobachtung unten: Commits und Checkpoints werden mit wachsender Datei sehr lang.

## Beobachtung Durchsatz bei 1 Mio. (PERF-002, nur berichtet, kein Kriterium)

Der Durchsatz fällt mit der Crawl-Größe überproportional ab (alle Läufe Balanced, 8 Threads, gleiche Maschine):

| Lauf | Seiten | Dauer | URLs/s |
|---|---:|---:|---:|
| CI-Bericht Small (macOS arm64, anderer Rechner) | 10 000 | 3,0 s | 3 321 |
| PERF-003, `--pages 100000` | 100 000 | 37,5 s | 2 664 |
| PERF-005, 10 Abbrüche (Dauer inkl. Neustarts) | 100 000 | 65,0 s | rund 1 540 |
| PERF-001 / CRAWL-124 Balanced | 250 000 | 198,1 s | 1 262 |
| PERF-003, `--pages 250000` | 250 000 | 196,6 s | 1 271 |
| PERF-003 | 1 000 000 | 6 948 s (116 min) | 143,9 |
| PERF-001 | 1 000 000 | 7 515 s (125 min) | 133,1 |

Verlauf von PERF-001 (grob, aus Stichproben `max(rowid)` der Tabelle `links` alle 10 bis 30 Minuten, 30 Links je Seite, Messung von Hand während des Laufs): in den ersten 31 min rund 290 Seiten/s, danach rund 130 Seiten/s (bis 23:07), rund 75 Seiten/s (23:07 bis 00:17).

Aus dem `diag`-Block von PERF-001 (1 Mio.): 501 Batches, `store_commit_ns` 3 980 s (53 % der Laufzeit; im Mittel 7,96 s je Batch, Maximum 19,3 s, alle 500 Gruppen-Commits „slow“), `store_build_ns` 3 455 s, `engine_send_ns` 3 937 s (Worker warten auf den Writer). `bench_writer` (Block B6) zeigt denselben Effekt im Kleinen: 100 000 Seiten x 30 Links mit gleichverteilten Linkzielen 28,5 s gegen 6,9 s bei Seiten-ähnlichen Zielen. Die Vermutung ist, dass die Indexpflege der Tabelle `links` bei gleichverteilten Zielen in einer 6 GB großen Datei mit 256-MiB-Cache auf zufällige Seiten zugreift. Das ist nicht belegt; die Läufe verändern keinen Code.

Folgen: Ein 1-Mio.-Crawl braucht auf der Referenzmaschine über zwei Stunden. Der Profilvergleich CRAWL-124 wurde deshalb nach der Vorgabe (mehr als 90 min je Lauf) bei 250 000 Seiten ausgeführt.

## Maschine und Stand

| | |
|---|---|
| Rechner | Apple M2, 8 Kerne, 16 GB RAM (physischer RAM 17 179 869 184 B laut Bericht) |
| macOS | 26.6.2 |
| Commit | `43caf19` (origin/main, Welle 6 gemergt), Branch `m1/m1-n14` |
| Build | Release (`cargo build --release --locked -p os-loadtest`, `-p os-engine --example crawl_child`), `CARGO_INCREMENTAL=0`, `RUSTC_WRAPPER=sccache`; nichts am Produktcode geändert |
| Konstanten | `BATCH_PAGES_BACKLOG` 2 000, `CHANNEL_CAPACITY` 256, 8 Threads |
| Wachhalten | `caffeinate -dimsu` um jeden Lauf |
| Arbeitsordner | `<repo-parent>/n14-work/` außerhalb des Repos, nach jedem Lauf gelöscht (freier Platz vor jedem Block 75 GB, Spitze durch die 6,3-GB-Crawl-Datei; Target-Ordner des Pakets zuletzt 10 GB, danach 67 GB frei) |
| Parallele Läufe | keine; kein anderer Paket-Worktree, keine parallelen Builds oder Läufe während der Messungen |

### Last je Block

Der Startcheck des Orchestrators lag bei Load 1,59 / 2,25 / 2,39. Die Grundlast der Maschine lag während der ganzen Messung bei etwa 1,5 bis 2,5, hauptsächlich durch macOS-Hintergrunddienste (`FileProvider`, `spindump`, `runningboardd`), die ich nicht beende. Load < 1 war deshalb nicht erreichbar; jeder Block wurde nach der Regel „Load < 2“ gestartet (`scripts/bench/run_block.sh` wartet darauf). Einzelwerte stehen im Kopf der Logdateien.

| Block | Lauf | Load (1 min) beim Start | Load-Stichproben während des Laufs (1 min) |
|---|---|---|---|
| B1 | PERF-001 large | 1,88 | 1,13 bis 2,58 |
| B2 | PERF-003 large | 1,68 | 1,42 bis 2,59 |
| B2 ergänzend | PERF-003 `--pages 100000` / `250000` | 1,76 / 1,88 | – (Läufe 38 s bzw. 3 min) |
| B3 | PERF-005 large, `--kills 10` | 1,98 | 3,64 (Mitte), 3,09 (Ende); Lauf nur 102 s, die Last stammt von Testserver und Crawl-Kindprozessen selbst |
| B3 ergänzend | 10 Einzelabbrüche | 1,68 bis 1,98 (1,84 / 1,98 / 1,79 / 1,72 / 1,68 / 1,82 / 1,72 / 1,89 / 1,98 / 1,91) | – |
| B4 | CRAWL-124 Balanced / MinMemory / MaxSpeed | 1,80 / 1,96 / 1,77 | – (Läufe 3 bis 4,5 min) |
| B5/B6 | Benchmarks | 1,61 bis 1,98 | siehe `../benchmarks/README.md` |

Die Regel „Load < 1“ aus den Startbedingungen wurde damit nicht eingehalten (Ausnahmeregel „< 2“ der Vorgabe angewendet). Die Messwerte, die an der Maschine hängen (Durchsatz, Laufzeiten), sind mit dieser Grundlast zu lesen; RSS-Werte hängen davon nicht ab.

## Läufe, Befehle, Laufzeiten

Alle Befehle aus dem Arbeitsverzeichnis des Worktrees; `<ordner>` ist der Arbeitsordner außerhalb des Repos. Aufrufhilfe `scripts/bench/run_block.sh` (wartet auf Load < 2, schreibt Load und Zeiten in die Logdatei).

| Block | Befehl | Berichte und Logs | Laufzeit (Start–Ende, lokal) |
|---|---|---|---|
| B1 PERF-001 | `os-loadtest run --scenario perf-001 --size large --work-dir <ordner> --out docs/plan/M1-acceptance/loadtest/report-perf-001-large.json` | `report-perf-001-large.json`, `log-perf-001-large.txt` | 22:16–00:22 (126 min; Crawl 7 515 s) |
| B2 PERF-003 | `os-loadtest run --scenario perf-003 --size large --work-dir <ordner> --out …/report-perf-003-large.json` | `report-perf-003-large.json`, `log-perf-003-large.txt` | 00:25–02:20 (116 min; Crawl 6 948 s) |
| B2 ergänzend | `… --scenario perf-003 --size large --pages 100000` und `--pages 250000` | `report-perf-003-large-100000-pages.json`, `report-perf-003-large-250000-pages.json`, `log-perf-003-smaller-sites.txt` | 03:56–03:57 und 03:58–04:01 |
| B3 PERF-005 | `… --scenario perf-005 --size large --kills 10 --work-dir <ordner> --out …/report-perf-005-large.json` | `report-perf-005-large.json`, `log-perf-005-large.txt` | 02:31–02:33 (Crawl 65 s) |
| B3 ergänzend | 10 Läufe `… --scenario perf-005 --size large --kills 1 --seed <1..10>` | `perf-005-per-kill.json` | 02:36–03:15, je rund 35 s Crawl |
| B4 CRAWL-124 | `… --scenario perf-001 --size large --pages 250000 --profile <balanced\|min-memory\|max-speed>` | `report-crawl-124-<profil>-250k.json`, `log-crawl-124-profiles-250k.txt` | 03:16–03:19, 03:20–03:25, 03:25–03:26 |
| B5 Fortsetzen/Pausieren | `cargo test --release --locked -p os-engine --test lifecycle_pause -- --ignored --nocapture --test-threads=1` | `../benchmarks/os-engine-lifecycle_pause.txt` | 03:31, 6 s |
| B6 Benchmarks | siehe `../benchmarks/README.md` | `../benchmarks/*.txt` | 03:34–03:47 |

Zusätzlich vor dem Bericht (Vorgabe aus `prompt_notes`) einmal `cargo test --workspace --exclude openspider-desktop --locked --no-fail-fast`: 1 981 bestanden, 0 fehlgeschlagen, 34 ignoriert; `cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings` und `cargo fmt --all --check` ohne Befund (Debug-Build, keine Codeänderung).

## Einzelergebnisse

### B1: PERF-001 large (1 000 000 URLs x 30 Links)

| Messwert | Wert |
|---|---|
| Spitzen-RSS (`crawler_peak_rss_bytes`) | 720 846 848 B = 720,8 MB (Grenze 1 500 000 000) |
| Referenz 100 000 Seiten | 425 869 312 B = 425,9 MB; Verhältnis 1,693 (Regel `ratio`, Grenze 2,0) |
| URLs abgerufen | 1 000 000, 133,1 URLs/s, 7 514,9 s |
| Dateigröße | 6 334 238 720 B (Referenz 100 000 Seiten: 613 355 520 B) |
| Profil / Cache | Balanced, 268 435 456 B (256 MiB); MaxSpeed-Cache -4 194 304 KiB = 4 GiB (K-16: 25 % von 16 GiB, höchstens 4 GiB) |
| Testserver (100 000 Probe-Requests) | 137 140 Requests/s, p50 0,225 ms, p99 0,458 ms, 0 Fehler, deterministisch |
| Verstöße / Warnungen | keine |

### B2: PERF-003 large (SlowSink 100 ms je Schreibaufruf, K-52)

| Messwert | Wert |
|---|---|
| Spitzen-RSS | 707 641 344 B = 707,6 MB (Grenze 500 000 000): **Verstoß** |
| Crawl | endet regulär (`queue_empty`), 1 000 000 URLs, 143,9 URLs/s, 6 948 s |
| Längster Stillstand | 31,302 s (Lasttest-Grenze 30 s): **Verstoß** (siehe Nachbesserungsauftrag 2) |
| `diag`: Zahl der Batches | 501 (500 Batches mit mehr als 1 000 Seiten, 1 Batch bis 200 Seiten) |
| `diag`: Seiten je Batch | Mittel 1 996,0 (1 000 001 Seiten / 501); Maximum nicht im `diag`-Block, die Obergrenze ist `BATCH_PAGES_BACKLOG` = 2 000 |
| `diag`: höchster Kanalfüllstand | 256 (`engine_w_queue_max` = `CHANNEL_CAPACITY`, Kanal war voll) |
| `diag`: Summe der SlowSink-Wartezeit | 50,1 s (errechnet: 501 Schreibaufrufe x 100 ms; der `diag`-Block hat dafür keinen eigenen Zähler) |
| `diag`: Schreibzeit gesamt / größte | 6 872 s / 31,39 s (`engine_w_write_ns`, `engine_w_write_max_ns`); davon Store-Commit 3 569 s, Aufbau 3 248 s, WAL-Checkpoints 16,0 s (2 Stück) |
| Durchsatz im Vergleich zu PERF-001 large | 143,9 gegen 133,1 URLs/s (+8 %; die Szenarien unterscheiden sich im Fehleranteil der Site, nur zur Einordnung, K-52) |

Das Drosseln selbst belegt der Test `full_channel_blocks_the_sender_until_the_writer_catches_up` (`writer_thread.rs`, Tag PERF-003); hier nur zur Einordnung.

Kleinere Crawls mit derselben Konfiguration (ergänzend): 100 000 Seiten 426,5 MB RSS, 37,5 s, längster Stillstand 1,5 s; 250 000 Seiten 470,8 MB, 196,6 s, Stillstand 5,4 s; beide bestanden.

### B3: PERF-005 large (100 000 Seiten, `--kills 10`, Seed 20261014)

- Kill-Punkte (abgerufene Seiten): 14 177, 15 814, 27 500, 34 721, 38 982, 46 925, 61 498, 64 126, 71 705, 84 155.
- Je Abbruch: `integrity_check = ok` (10 von 10), `duplicate_urls` = 0 (10 von 10).
- Nach dem Fortsetzen: 100 001 URLs, 100 000 abgerufen, `urls_missing` 0, `urls_extra` 0, `urls_with_other_status` 0, Status identisch mit dem Referenzcrawl (200: 96 582, 301: 1 973, 404: 973, 500: 236, 503: 237).
- Wiederholte Requests (`repeated_requests`) gesamt 21 711 bei 10 Abbrüchen: `repeated_per_kill` (Mittel) **2 171,1**, `repeated_bound_per_kill` **2 264** (8 Threads + 256 Kanal + 2 000 Batch), Gesamtgrenze 22 640. robots.txt: 1 Abruf (nicht mitgezählt).
- Das Maximum je Abbruch gibt der 10-Abbrüche-Bericht nicht aus (nur Summe und Mittel). Deshalb ergänzend 10 Einzelabbrüche (`--kills 1`, Seeds 1 bis 10, `perf-005-per-kill.json`): wiederholte Requests 2 264, 2 264, 2 264, 2 264, 1 291, 2 264, 2 043, 1 576, 2 264, 2 264; **Maximum 2 264**, Mittel 2 075,8. Die Grenze wird in 6 von 10 Fällen genau erreicht, aber nie überschritten; sie ist die Obergrenze des Inhalts aller Puffer (K-49) und damit scharf. Alle 10 Einzelläufe: `integrity_check` ok, 0 Dubletten, Status identisch.
- Näherung aus dem 10-Abbrüche-Lauf: `lost_in_pipeline` je Abbruch 2 255, 1 868, 1 686, 2 255, 2 118, 2 255, 2 255, 1 273, 2 255, 2 255 (Maximum 2 255 <= 2 264); das sind die abgerufenen, aber nicht persistierten Seiten, ohne die gerade laufenden Abrufe.
- „abgeschnitten“ kommt in Bericht, Log und den 10 Einzelläufen 0-mal vor.

### B4: CRAWL-124 Profilvergleich (250 000 Seiten, perf-001-Szenario)

Begründung der Ersatzgröße: Der Balanced-Lauf bei 1 Mio. dauerte 125 min und damit mehr als die in der Vorgabe genannten 90 min; zwei weitere 1-Mio.-Läufe (MinMemory ist langsamer) hätten über 4 Stunden gekostet. Deshalb `--pages 250000` für alle drei Profile.

| Profil | Cache | Spitzen-RSS | Veränderung RSS gegen Balanced | Dauer | URLs/s | Veränderung Durchsatz |
|---|---|---:|---:|---:|---:|---:|
| Balanced | 256 MiB | 468,5 MB | – | 198,1 s | 1 261,7 | – |
| MinMemory | 64 MiB | **216,4 MB** | **-53,8 %** | 269,2 s | 928,7 | -26,4 % |
| MaxSpeed | 4 GiB | 2 108,3 MB | +350 % | 102,0 s | 2 451,3 | +94,3 % |

- Kriterium „MinMemory mindestens 20 % unter Balanced“: erfüllt (-53,8 %).
- Der Lasttest bewertet auch den MaxSpeed-Lauf mit den PERF-001-Grenzen und meldet daher „VERLETZT“ (RSS 2 108 MB >= 1,5 GB; Wachstumsregel); Exit-Code 1. Das ist kein Befund: der Cache des Profils beträgt laut K-16/CRAWL-124 bis zu 4 GiB (hier 25 % von 16 GiB), die 1,5 GB gelten für das Standardprofil. Hinweis an den Planer: `--profile max-speed` kann PERF-001 prinzipbedingt nicht bestehen; der Lasttest könnte die RSS-Grenzen für dieses Profil auslassen.
- Balanced und MinMemory bestehen die PERF-001-Prüfung bei 250 000 Seiten.

## Dateien in diesem Ordner

- `report-perf-001-large.json`, `report-perf-003-large.json`, `report-perf-005-large.json`: Berichte von `os-loadtest` (Schema `{scenario, size, started_at, metrics, passed}`).
- `report-perf-003-large-100000-pages.json`, `report-perf-003-large-250000-pages.json`: Einordnung des RSS-Verstoßes.
- `report-crawl-124-{balanced,min-memory,max-speed}-250k.json`: Profilvergleich.
- `perf-005-per-kill.json`: wiederholte Requests je einzelnem Abbruch (aus 10 Läufen mit `--kills 1`).
- `log-*.txt`: Kopf (Start, Load) und Konsolenausgabe je Block.
