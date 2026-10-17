# Benchmarks (M1-N28, Block B6, nach Welle 7e)

Alle Tests mit `#[ignore = "Benchmark…"]` aus `crates/` (ohne `crates/os-parity/tests/data`), je Testdatei einzeln in Release ausgeführt:

```sh
cargo test --release --locked -p <crate> --test <datei> -- --ignored --nocapture --test-threads=1 [<testname>]
```

Aufrufhilfen: `scripts/bench/run_ignored_benchmarks.sh <ausgabeordner> <arbeitsordner>` (alle Zeilen der Liste nacheinander) über `scripts/bench/run_acceptance.sh` (wartet vor jedem Lauf auf `scripts/bench/wait_quiet.sh 2`, Load (1 min) < 2, schreibt Load, Zeiten und Rohausgabe, läuft unter `caffeinate -dimsu`). Die Testprogramme wurden vorher einmal gebaut (`cargo test --release --locked --no-run -p os-store -p os-export -p os-fetch -p os-engine -p os-parse -p os-testsite -p os-analyze`); `cargo test` hat beim Start der Läufe einzelne Crates trotzdem neu übersetzt (Zeilen `Compiling …` in den Rohdateien, wie schon in N21: die Funktionsauswahl je `-p` verändert die Feature-Vereinigung). Die Übersetzung läuft vor dem Test und fällt nicht in die Messwerte, die die Tests selbst messen (Spalte „Laufzeit“ = Zeile `test result … finished in`); der Load-Wert vor dem Start stammt aus `wait_quiet`.

Die Liste wurde vor B6 mit `grep -rn 'ignore = "Benchmark' crates --include='*.rs'` abgeglichen: **21 Tests in 14 Dateien**, alle ausgeführt, kein Benchmark-Ignore ohne Lauf. Gegenüber N21 (18 Tests in 12 Dateien) neu aus M1-N27: `crates/os-analyze/tests/bench_duplicates.rs` (1 Test) und `crates/os-store/tests/bench_links_build.rs` (2 Tests: `create_master`, `build_variants`). Die Liste im Skript wurde um diese beiden Dateien ergänzt; `bench_links_build` braucht zwei getrennte Läufe (zuerst `create_master`, dann `build_variants`, beide mit `OS_BENCH_MASTER`), `bench_duplicates` läuft mit `OS_BENCH_ROWS=1000000`. Die Ignores mit `M2:`-Präfix sind keine Benchmarks und bleiben unberührt (sie liegen in anderen Dateien).

## Maschine und Stand

| | |
|---|---|
| Rechner | Apple M2, 8 Kerne, 16 GB RAM (lokale Referenzmaschine) |
| macOS | 26.6.2 |
| Commit | `b6ba90a` (`origin/main` nach Merge M1-N27, Welle 7e), Branch `m1/m1-n28`; an Produktcode und Tests geändert: nichts |
| Build | Release, `--locked`, `CARGO_INCREMENTAL=0`, `RUSTC_WRAPPER=sccache`, Target `openspider-wt/target-m1-n28` |
| Datum | 17.10.2026, B6 von 05:37 bis 06:17 (lokal), jeder Lauf unter `caffeinate -dimsu` |
| Load (1 min) vor dem Start jedes Tests | 1,62 bis 1,98 (Grenze < 2), je Rohdatei im Kopf. Lastproben im Lauf (länger als eine Minute): `*.txt.load`, bei `bench_writer` 1,71 bis 4,32, bei `bench_duplicates` 1,96 bis 2,06, bei `create_master` 1,98 bis 2,08; kürzere Läufe haben keine Proben, ihr Load nach dem Ende steht im Fuß der Rohdatei (bis 9,4 nach dem Übersetzen und dem Test von `sitemap_gzip`) |
| Parallele Läufe | keine eigenen; die Fremdlast von B3 (Messung des Nutzers, siehe `../loadtest/README.md`) war beendet |
| Arbeitsordner | `openspider-wt/n28-work/b6` außerhalb des Repos (Vorlage `n28-links-master.ospider` 5,5 GB und Kopie danach gelöscht) |

## Ergebnis

**21 von 21 Benchmarks bestanden, keine Grenzverletzung.** Das Skript endete mit Exit 0, jede Rohdatei trägt `test result: ok` und `Exit 0`.

| Test | Datei | Spec-ID | Grenze | Messwert (N28) | N21 (Vergleich) | bestanden | Laufzeit (N28 / N21) |
|---|---|---|---|---|---|---|---|
| `one_million_rows_by_sixty_columns_stay_within_300_mb_of_extra_memory` | `os-export/tests/pipeline_bench.rs` | OUT-001 | 1 Mio. x 60 Spalten, Speicherzuwachs <= 300 MB | Export in 6,9 s (Anlegen 4,0 s), Datei 336 MiB, RSS vorher 334 MiB, Spitze 334 MiB, Zuwachs 1 MiB | 6,9 s (Anlegen 4,1 s), 336 MiB, Zuwachs 1 MiB | ja | 11,0 s / 11,1 s |
| `one_million_links_are_written_as_csv_in_under_60_seconds` | `os-export/tests/links_bench.rs` | OUT-031 | 1 Mio. Links < 60 s | 2,20 s, 150 MiB; RSS vorher 259 MiB, Spitze 436 MiB | 2,27 s, 150 MiB; 258 / 436 MiB | ja | 4,1 s / 4,4 s |
| `default_byte_limit_of_50_mib_splits_files_with_very_long_urls` | `os-export/tests/sitemap_gen.rs` | OUT-129 | 26 000 URLs à 2 000 Zeichen (rund 52 MB) teilen sich in >= 2 Dateien, jede <= 52 428 800 Byte | Zusicherungen erfüllt (keine Zahl ausgegeben) | gleich | ja | 1,1 s / 1,3 s |
| `default_limit_overflows_after_1048575_rows` | `os-export/tests/xlsx_cells.rs` | OUT-003 | 1,1 Mio. Zeilen: Blatt 1 mit 1 048 576 Zeilen (Kopf + 1 048 575), Blatt 2 mit dem Rest | Zusicherungen erfüllt (keine Zahl ausgegeben) | gleich | ja | 2,3 s / 2,3 s |
| `opening_a_million_urls_takes_under_3_seconds_and_saving_under_60` | `os-engine/tests/perf_store.rs` | CRAWL-127 | Öffnen von 1 Mio. URLs < 3 s, Speichern unter < 60 s | Öffnen 1 ms (+ erste 100 Zeilen 1 ms, warmer Dateicache), Speichern unter **1,4 s** (Kopie 569 MiB); Schreiben der Testdatei 22 s, 614 MiB; `COUNT(*)` über `urls` 67 ms (nur Bericht) | Öffnen 1 ms (+ 3 ms), Speichern unter 4,8 s (612 MiB); Schreiben 25 s, 663 MiB; `COUNT(*)` 929 ms | ja | Teil von 51,8 s / 59,2 s |
| `group_commit_versus_small_batches_with_uniform_targets` | `os-engine/tests/perf_store.rs` | CRAWL-124 | nur Bericht (Gruppen-Commit, gleichverteilte Ziele) | 100 000 Seiten x 30 Links: 200 Seiten je Batch 10,3 s (9 672 Seiten/s), 1 000 Seiten 9,7 s (-6 %), 5 000 Seiten 8,1 s (-21 %); Datei 256 MiB | 10,4 s (9 643 Seiten/s), 9,7 s (-6 %), 8,1 s (-22 %); 256 MiB | ja | Teil von 51,8 s / 59,2 s |
| `pausing_with_a_million_queue_entries_takes_under_three_seconds` | `os-engine/tests/lifecycle_pause.rs` | CRAWL-126 | Pausieren mit 1 Mio. Warteschlangeneinträgen < 3 s | Fortsetzen (Aufbau aus der Datei) 750 ms, Pausieren 11,0 ms | 648 ms, 10,9 ms | ja | 5,7 s / 6,0 s |
| `a_list_of_one_hundred_thousand_urls_is_read_in_under_ten_seconds` | `os-engine/tests/lifecycle_list.rs` | CRAWL-027 | 100 000 Zeilen < 10 s | Test-Gesamtlaufzeit 0,15 s (Einlesen und Planen), Zusicherungen erfüllt | 0,15 s | ja | 0,15 s / 0,15 s |
| `writer_100k_pages_with_30_links_under_60_seconds` | `os-store/tests/bench_writer.rs` | – (kein Tag; Writer-Rahmen PERF-001) | 100 000 Seiten x 30 Links < 60 s je Profil | Balanced 5,5 s (18 347 Seiten/s), MaxSpeed 5,5 s (18 213), MinMemory 5,4 s (18 579); Datei 250 MiB | 5,4 s (18 436), 5,6 s (17 884), 5,4 s (18 626); 250 MiB | ja | Teil von 240,3 s / 264,3 s |
| `writer_100k_pages_uniform_targets_report` | `os-store/tests/bench_writer.rs` | – (nur Bericht) | keine | Balanced, gleichverteilte Ziele: 10,6 s (9 478 Seiten/s, 284 333 Links/s), Datei 256 MiB | 10,5 s (9 526 Seiten/s), 256 MiB | ja | Teil von 240,3 s / 264,3 s |
| `inlinks_paging_with_one_million_inlinks` | `os-store/tests/bench_writer.rs` | – (Inlink-Seitenabruf) | Seitenabruf bei 1 Mio. Inlinks (Gesamtzahl 1 000 000, Seite mit 50 Zeilen) | erste Seite 355 ms, Offset 500 000 342 ms, Offset 500 000 nach Anchor 1 532 ms | 353 ms, 342 ms, 1 534 ms | ja | Teil von 240,3 s / 264,3 s |
| `writer_1m_uniform_segments` (P32) | `os-store/tests/bench_writer.rs` | PERF-001, PERF-002, PERF-003 | 1 Mio. Seiten x 30 gleichverteilte Linkziele; zehn Abschnitte, Abschnittsdurchsatz zum Mittel (Bericht) | 7 498 Seiten/s (Seiten schreiben 133,4 s, URLs anlegen 5,9 s); Abschnitte 7 710, 7 671, 7 638, 7 576, 7 508, 7 482, 7 467, 7 208, 7 407, 7 339 Seiten/s = 96 % bis 103 % des Mittels; größter Commit 0,52 s, größter Checkpoint 0,00 s; Endbatch (Indexaufbau) **14,5 s**; Datei 3 292 MiB; Spitzen-RSS 393 MiB | 7 044 Seiten/s (142,0 s, URLs 7,2 s), 98 % bis 103 %, größter Commit 0,58 s, Endbatch 25,5 s, 3 341 MiB, 371 MiB | ja | Teil von 240,3 s / 264,3 s (Wanduhr 135,8 s im Test) |
| `inlinks_without_index_250k_pages` (P32) | `os-store/tests/bench_writer.rs` | PERF-002 (K-74) | Bericht: `links_page` In ohne Index, Hochrechnung 1 Mio. | 250 000 Seiten x 30 Links: ohne Index 0,23 s (Mittel aus vier Zielen, einzeln 0,22 / 0,24 / 0,26 / 0,22), mit Index 0,0003 s; Indexaufbau 1,9 s; Hochrechnung 1 Mio. (x4,0): ohne Index 0,9 s, Aufbau 8 s | 0,24 s, mit Index 0,0118 s, Aufbau 3,2 s, Hochrechnung 0,9 s / 13 s | ja | Teil von 240,3 s / 264,3 s |
| `over_50_mib_unpacked_is_read_completely_by_streaming` | `os-parse/tests/sitemap_gzip.rs` | CRAWL-060 | Sitemap > 50 MiB entpackt wird vollständig gelesen (2 `loc`, `size_bytes` > 52 428 800) | Zusicherungen erfüllt (keine Zahl ausgegeben) | gleich | ja | 0,06 s / 0,06 s |
| `throughput_report` | `os-parse/tests/pixel_width.rs` | UI-038 | nur Bericht | 43,4 Mio. Titel/min | 43,0 Mio. Titel/min | ja | 1,4 s / 1,4 s |
| `benchmark_20k_requests_over_5000_per_second` | `os-testsite/tests/synthetic.rs` | PERF-002 (Server darf den Durchsatz nicht begrenzen) | > 5 000 Requests/s | 118 873 Requests/s (20 000 Requests in 0,17 s, 52,7 MB) | 125 801 Requests/s (0,16 s) | ja | 0,17 s / 0,16 s |
| `slow_route_response_time_is_between_800_and_1000_ms` | `os-fetch/tests/http_status.rs` | CRAWL-022 | Antwortzeit der Route `/slow-800` zwischen 800 und 1 000 ms | Zusicherung erfüllt (keine Zahl ausgegeben) | gleich | ja | Teil von 3,9 s / 3,9 s |
| `timeout_two_seconds_against_three_second_route` | `os-fetch/tests/http_status.rs` | CRAWL-095 | Timeout 2 s gegen 3-s-Route: Abbruch nach 2 000 bis 2 500 ms, `/slow-1000` liefert 200 | Zusicherungen erfüllt (keine Zahl ausgegeben) | gleich | ja | Teil von 3,9 s / 3,9 s |
| `five_duplicate_filters_on_one_million_rows_without_the_indexes` (neu, N27) | `os-analyze/tests/bench_duplicates.rs` | ONPAGE-143 (K-83) | nur Bericht, Zeit je Filter bei 1 Mio. Zeilen **ohne** die fünf Indizes | Page Titles: Duplicate 9,04 s, Meta Description: Duplicate 10,14 s, H1: Duplicate 10,63 s, H2: Duplicate 11,20 s, Content: Exact Duplicates 3,33 s; je 300 000 Treffer; Füllen 41,1 s, Datei 469 MiB | – (neu; N27-Probe unter Last: 11,5 / 11,7 / 12,0 / 13,4 / 4,1 s) | ja | 85,6 s |
| `create_master` (neu, N27) | `os-store/tests/bench_links_build.rs` | PERF-001 (K-84) | Vorlage für `build_variants`: 1 Mio. Seiten x 30 gleichverteilte Links, breite Zeilen, ohne `links_target` | 168 s, 5 476 MiB | – (neu) | ja | 167,9 s |
| `build_variants` (neu, N27) | `os-store/tests/bench_links_build.rs` | PERF-001 (K-84) | nur Bericht: Aufbau von `links_target` je Einstellung (Kaltprobe in N27: <= 35 s) | 2 Hilfsthreads, 4 MiB Cache (Stand im Crawl): Aufbau **19,3 s**, mit `COMMIT` 21,1 s, CPU 14,4 s Benutzer / 4,8 s System, Spitzen-RSS 23 MiB; 4 Hilfsthreads: 17,7 s, mit `COMMIT` 19,2 s, CPU 14,8 / 5,0 s, RSS 33 MiB. Warmer Dateicache (Kopie der Vorlage, kein Füller, `inblock` 0), `Pageins` 326 125 bzw. 332 143 | – (neu; N27 maß 24,8 s bis 27,2 s mit kaltem Cache unter Last) | ja | 40,7 s |

Das sind 21 Zeilen (zwei Tests in `perf_store.rs`, fünf in `bench_writer.rs`, zwei in `http_status.rs`, zwei in `bench_links_build.rs`, je einer in den übrigen neun Dateien). Hinweis zu den Zeilen ohne Zahl: Die Tests drucken keinen Messwert, sondern prüfen die Grenze per Zusicherung; „bestanden“ heißt, dass alle Zusicherungen gehalten haben. `sudo -n /usr/sbin/purge` verlangt ein Passwort; die Aufbauzeiten von `build_variants` gelten deshalb für einen warmen Cache (die Kaltprobe mit Füllerdatei steht in N27, `../loadtest/probe-n27/README.md`, Abschnitt 4).

## Vergleich mit N21 (Stand nach Welle 7) und N14 (vor Welle 7, Commit `43caf19`)

| Messwert | N14 | N21 | N28 |
|---|---:|---:|---:|
| `bench_writer`: 100 000 Seiten x 30 Links, Seiten-ähnliche Ziele (Balanced) | 6,9 s (14 562 Seiten/s) | 5,4 s (18 436 Seiten/s) | 5,5 s (18 347 Seiten/s) |
| `bench_writer`: 100 000 Seiten, gleichverteilte Ziele (Balanced) | 28,5 s (3 513 Seiten/s) | 10,5 s (9 526 Seiten/s) | 10,6 s (9 478 Seiten/s) |
| `bench_writer`: 1 Mio. Seiten, gleichverteilte Ziele | – | 7 044 Seiten/s, Endbatch 25,5 s | 7 498 Seiten/s, Endbatch 14,5 s |
| `perf_store`: gleichverteilte Ziele, 200 Seiten je Batch | 27,9 s | 10,4 s | 10,3 s |
| `perf_store`: gleichverteilte Ziele, 5 000 Seiten je Batch | 14,6 s (-48 %) | 8,1 s (-22 %) | 8,1 s (-21 %) |
| CRAWL-127 Speichern unter (1 Mio. URLs) | 5,7 s | 4,8 s | 1,4 s |
| CRAWL-126 Pausieren / Fortsetzen (1 Mio. Warteschlange) | 71 ms / 1,23 s | 10,9 ms / 0,65 s | 11,0 ms / 0,75 s |
| Inlink-Seitenabruf bei 1 Mio. Inlinks, erste Seite | 134 ms | 353 ms | 355 ms |
| OUT-001 Speicherzuwachs | 1 MiB | 1 MiB | 1 MiB |
| OUT-031 1 Mio. Links als CSV | 2,26 s | 2,27 s | 2,20 s |

Zwei Messwerte bewegen sich gegenüber N21 deutlich: **Speichern unter** (CRAWL-127) von 4,8 s auf 1,4 s und der **Endbatch im Store-Benchmark** von 25,5 s auf 14,5 s. Beide hängen mit M1-N27 zusammen: Die Testdatei kommt ohne die fünf `urls`-Indizes aus (614 gegen 663 MiB beim Schreiben, Kopie 569 gegen 612 MiB), und der Endbatch baut sie nicht mehr auf (K-83). `COUNT(*)` über `urls` fällt von 929 auf 67 ms; die Zeit hängt am Dateicache und ist nicht Teil des Kriteriums. Der Abstand zwischen gleichverteilten und Seiten-ähnlichen Zielen bleibt beim Faktor 1,9. Der Inlink-Seitenabruf der ersten Seite liegt wie in N21 bei rund 355 ms (N14: 134 ms); der Test hat keine Zeitgrenze und es gibt kein Spec-Kriterium dazu, die Ursache (Streuung oder K-74/K-75) ist weiter nicht untersucht.

## Einordnung

- **CRAWL-126 Fortsetzen mit 1 Mio. Warteschlangeneinträgen:** Wiederaufbau aus der Datei 0,75 s (Grenze für Pausieren < 3 s), Pausieren 11,0 ms.
- **CRAWL-127:** Öffnen 1 ms gegen < 3 s, Speichern unter 1,4 s gegen < 60 s.
- **OUT-001:** Speicherzuwachs 1 MiB gegen 300 MB.
- **Writer:** Im Store-Benchmark `writer_1m_uniform_segments` ist der Durchsatz über 1 Mio. Seiten flach (96 % bis 103 % des Mittels). Im echten Crawl (PERF-001 large, `../loadtest/README.md`) liegen die Abschnitte seit M1-N27 bei 85 % bis 121 % des Mittelwerts.
- **K-83 (N27):** Die fünf Duplicate-Filter brauchen auf 1 Mio. Zeilen ohne Indizes 3,3 s („Exact Duplicates“) bis 11,2 s (H2), das korrelierte `EXISTS` hätte dort Stunden gebraucht.

## Rohausgabe

Gekürzt auf Kopf (Start, `wait_quiet`, Load, Befehl), Übersetzungszeilen, Messzeilen und Ende:

- `os-export-pipeline_bench.txt`, `os-export-links_bench.txt`, `os-export-sitemap_gen.txt`, `os-export-xlsx_cells.txt`
- `os-engine-perf_store.txt`, `os-engine-lifecycle_pause.txt`, `os-engine-lifecycle_list.txt`
- `os-store-bench_writer.txt`, `os-store-bench_links_build-create_master.txt`, `os-store-bench_links_build-build_variants.txt`
- `os-analyze-bench_duplicates.txt`
- `os-parse-sitemap_gzip.txt`, `os-parse-pixel_width.txt`
- `os-testsite-synthetic.txt`, `os-fetch-http_status.txt`
- `*.txt.load`: Lastproben im Lauf, jede Minute (nur bei Läufen über einer Minute).
- `n21-*`: Rohausgaben und Bericht (`n21-README.md`) der Messung M1-N21, unverändert.
