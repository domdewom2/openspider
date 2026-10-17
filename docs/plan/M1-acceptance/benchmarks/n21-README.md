# Benchmarks (M1-N21, Block B5 und B6, nach Welle 7)

Alle Tests mit `#[ignore = "Benchmark…"]` aus `crates/` (ohne `crates/os-parity/tests/data`), je Testdatei einzeln in Release ausgeführt:

```sh
cargo test --release --locked -p <crate> --test <datei> -- --ignored --nocapture --test-threads=1
```

Aufrufhilfen: `scripts/bench/run_ignored_benchmarks.sh <ausgabeordner>` (alle zwölf Dateien nacheinander) über `scripts/bench/run_acceptance.sh` (wartet vor jedem Lauf auf `scripts/bench/wait_quiet.sh 2`, Load (1 min) < 2, schreibt Load, Zeiten und Rohausgabe, läuft unter `caffeinate -dimsu`). Die Testprogramme wurden vorher einmal gebaut (`cargo test --release --locked --no-run -p os-store -p os-export -p os-fetch -p os-engine -p os-parse -p os-testsite`), damit der Bau nicht in die Messung fällt. Die Liste wurde vor B6 mit `grep -rn 'ignore = "Benchmark' crates --include='*.rs'` abgeglichen: **18 Tests in 12 Dateien**, alle ausgeführt, kein Benchmark-Ignore ohne Lauf. Gegenüber N14 (16 Tests in 12 Dateien) neu: `writer_1m_uniform_segments` und `inlinks_without_index_250k_pages` (beide `crates/os-store/tests/bench_writer.rs`, aus M1-P32); `bench_writer.rs` hat damit fünf statt drei Tests. Die Ignores mit `M2:`-Präfix sind keine Benchmarks und bleiben unberührt (sie liegen in anderen Dateien).

## Maschine und Stand

| | |
|---|---|
| Rechner | Apple M2, 8 Kerne, 16 GB RAM (lokale Referenzmaschine) |
| macOS | 26.6.2 |
| Commit | `88fd424` (origin/main nach Merge M1-N20, Welle 7 komplett), Branch `m1/m1-n21`; an Produktcode und Tests geändert: nichts |
| Build | Release, `--locked`, `CARGO_INCREMENTAL=0`, `RUSTC_WRAPPER=sccache` |
| Datum | 15.10.2026, B5 22:26, B6 22:27 bis 23:15 (lokal), jeder Lauf unter `caffeinate -dimsu` |
| Load (1 min) vor dem Start jedes Tests | 1,70 bis 1,99 (Grenze < 2), je Rohdatei im Kopf; die Last nach dem Lauf von `sitemap_gzip` (14,2) stammt vom Test selbst bzw. Hintergrundprozessen nach dem Messen; der nächste Test wartete wieder auf Load < 2. Lastproben im Lauf bei `perf_store` und `bench_writer` (länger als eine Minute): `*.txt.load`, 1,85 bis 4,95 |
| Parallele Läufe | keine |

## Ergebnis

**18 von 18 Benchmarks bestanden, keine Grenzverletzung.**

| Test | Datei | Spec-ID | Grenze | Messwert | bestanden | Laufzeit |
|---|---|---|---|---|---|---|
| `one_million_rows_by_sixty_columns_stay_within_300_mb_of_extra_memory` | `os-export/tests/pipeline_bench.rs` | OUT-001 | 1 Mio. x 60 Spalten, Speicherzuwachs <= 300 MB | Export in 6,9 s (Anlegen 4,1 s), Datei 336 MiB, RSS vorher 334 MiB, Spitze 334 MiB, Zuwachs 1 MiB | ja | 11,1 s |
| `one_million_links_are_written_as_csv_in_under_60_seconds` | `os-export/tests/links_bench.rs` | OUT-031 | 1 Mio. Links < 60 s | 2,27 s, 150 MiB; RSS vorher 258 MiB, Spitze 436 MiB | ja | 4,4 s |
| `default_byte_limit_of_50_mib_splits_files_with_very_long_urls` | `os-export/tests/sitemap_gen.rs` | OUT-129 | 26 000 URLs a 2 000 Zeichen (rund 52 MB) teilen sich in >= 2 Dateien, jede <= 52 428 800 Byte | Zusicherungen erfüllt (keine Zahl ausgegeben) | ja | 1,3 s |
| `default_limit_overflows_after_1048575_rows` | `os-export/tests/xlsx_cells.rs` | OUT-003 | 1,1 Mio. Zeilen: Blatt 1 mit 1 048 576 Zeilen (Kopf + 1 048 575), Blatt 2 mit dem Rest | Zusicherungen erfüllt (keine Zahl ausgegeben) | ja | 2,3 s |
| `opening_a_million_urls_takes_under_3_seconds_and_saving_under_60` | `os-engine/tests/perf_store.rs` | CRAWL-127 | Öffnen von 1 Mio. URLs < 3 s, Speichern unter < 60 s | Öffnen 1 ms (+ erste 100 Zeilen 3 ms, warmer Dateicache), Speichern unter 4,8 s (Kopie 612 MiB); Schreiben der Testdatei 25 s, 663 MiB; `COUNT(*)` über `urls` 929 ms (nur Bericht) | ja | Teil von 59,2 s |
| `group_commit_versus_small_batches_with_uniform_targets` | `os-engine/tests/perf_store.rs` | CRAWL-124 | nur Bericht (Gruppen-Commit, gleichverteilte Ziele) | 100 000 Seiten x 30 Links: 200 Seiten je Batch 10,4 s (9 643 Seiten/s), 1 000 Seiten 9,7 s (-6 %), 5 000 Seiten 8,1 s (-22 %); Datei 256 MiB | ja | Teil von 59,2 s |
| `pausing_with_a_million_queue_entries_takes_under_three_seconds` | `os-engine/tests/lifecycle_pause.rs` | CRAWL-126 | Pausieren mit 1 Mio. Warteschlangeneinträgen < 3 s | Fortsetzen (Aufbau aus der Datei) 648 ms, Pausieren 10,9 ms | ja | 6,0 s |
| `a_list_of_one_hundred_thousand_urls_is_read_in_under_ten_seconds` | `os-engine/tests/lifecycle_list.rs` | CRAWL-027 | 100 000 Zeilen < 10 s | Test-Gesamtlaufzeit 0,15 s (Einlesen und Planen), Zusicherungen erfüllt | ja | 0,15 s |
| `writer_100k_pages_with_30_links_under_60_seconds` | `os-store/tests/bench_writer.rs` | – (kein Tag; Writer-Rahmen PERF-001) | 100 000 Seiten x 30 Links < 60 s je Profil | Balanced 5,4 s (18 436 Seiten/s), MaxSpeed 5,6 s (17 884 Seiten/s), MinMemory 5,4 s (18 626 Seiten/s); Datei 250 MiB | ja | Teil von 264,3 s |
| `writer_100k_pages_uniform_targets_report` | `os-store/tests/bench_writer.rs` | – (nur Bericht) | keine | Balanced, gleichverteilte Ziele: 10,5 s (9 526 Seiten/s, 285 776 Links/s), Datei 256 MiB | ja | Teil von 264,3 s |
| `inlinks_paging_with_one_million_inlinks` | `os-store/tests/bench_writer.rs` | – (Inlink-Seitenabruf) | Seitenabruf bei 1 Mio. Inlinks (Gesamtzahl 1 000 000, Seite mit 50 Zeilen) | erste Seite 353 ms, Offset 500 000 342 ms, Offset 500 000 nach Anchor 1 534 ms | ja | Teil von 264,3 s |
| `writer_1m_uniform_segments` (neu, P32) | `os-store/tests/bench_writer.rs` | PERF-001, PERF-002, PERF-003 | 1 Mio. Seiten x 30 gleichverteilte Linkziele; zehn Abschnitte, Abschnittsdurchsatz zum Mittel (Bericht) | 7 044 Seiten/s (Seiten schreiben 142,0 s, URLs anlegen 7,2 s); Abschnitte 7 229, 7 010, 7 228, 7 194, 7 153, 7 040, 6 896, 6 920, 6 926, 6 873 Seiten/s = 98 % bis 103 % des Mittels; größter Commit 0,58 s, größter Checkpoint 0,00 s; Endbatch (Indexaufbau) 25,5 s; Datei 3 341 MiB; Spitzen-RSS 371 MiB | ja | Teil von 264,3 s (Wanduhr 144,4 s im Test) |
| `inlinks_without_index_250k_pages` (neu, P32) | `os-store/tests/bench_writer.rs` | PERF-002 (K-74) | Bericht: `links_page` In ohne Index, Hochrechnung 1 Mio. | 250 000 Seiten x 30 Links: ohne Index 0,24 s (Mittel aus vier Zielen 0,22 / 0,24 / 0,26 / 0,22), mit Index 0,0118 s; Indexaufbau 3,2 s; Hochrechnung 1 Mio. (x4,0): ohne Index 0,9 s, Aufbau 13 s | ja | Teil von 264,3 s |
| `over_50_mib_unpacked_is_read_completely_by_streaming` | `os-parse/tests/sitemap_gzip.rs` | CRAWL-060 | Sitemap > 50 MiB entpackt wird vollständig gelesen (2 `loc`, `size_bytes` > 52 428 800) | Zusicherungen erfüllt (keine Zahl ausgegeben) | ja | 0,06 s |
| `throughput_report` | `os-parse/tests/pixel_width.rs` | UI-038 | nur Bericht | 43,0 Mio. Titel/min | ja | 1,4 s |
| `benchmark_20k_requests_over_5000_per_second` | `os-testsite/tests/synthetic.rs` | PERF-002 (Server darf den Durchsatz nicht begrenzen) | > 5 000 Requests/s | 125 801 Requests/s (20 000 Requests in 0,16 s, 52,7 MB) | ja | 0,16 s |
| `slow_route_response_time_is_between_800_and_1000_ms` | `os-fetch/tests/http_status.rs` | CRAWL-022 | Antwortzeit der Route `/slow-800` zwischen 800 und 1 000 ms | Zusicherung erfüllt (keine Zahl ausgegeben) | ja | Teil von 3,9 s |
| `timeout_two_seconds_against_three_second_route` | `os-fetch/tests/http_status.rs` | CRAWL-095 | Timeout 2 s gegen 3-s-Route: Abbruch nach 2 000 bis 2 500 ms, `/slow-1000` liefert 200 | Zusicherungen erfüllt (keine Zahl ausgegeben) | ja | Teil von 3,9 s |

Das sind 18 Zeilen (zwei Tests in `perf_store.rs`, fünf in `bench_writer.rs`, zwei in `http_status.rs`, je einer in den übrigen neun Dateien). Hinweis zu den Zeilen ohne Zahl: Die Tests drucken keinen Messwert, sondern prüfen die Grenze per Zusicherung; „bestanden“ heißt, dass alle Zusicherungen gehalten haben. Die Laufzeit steht in der Spalte „Laufzeit“ (Zeile `test result … finished in`).

## Vergleich mit N14 (vor Welle 7, Commit `43caf19`)

| Messwert | N14 | N21 |
|---|---:|---:|
| `bench_writer`: 100 000 Seiten x 30 Links, Seiten-ähnliche Ziele (Balanced) | 6,9 s (14 562 Seiten/s) | 5,4 s (18 436 Seiten/s) |
| `bench_writer`: 100 000 Seiten, gleichverteilte Ziele (Balanced) | 28,5 s (3 513 Seiten/s) | 10,5 s (9 526 Seiten/s) |
| `perf_store`: gleichverteilte Ziele, 200 Seiten je Batch | 27,9 s | 10,4 s |
| `perf_store`: gleichverteilte Ziele, 5 000 Seiten je Batch | 14,6 s (-48 %) | 8,1 s (-22 %) |
| CRAWL-127 Speichern unter (1 Mio. URLs) | 5,7 s | 4,8 s |
| CRAWL-126 Pausieren / Fortsetzen (1 Mio. Warteschlange) | 71 ms / 1,23 s | 10,9 ms / 0,65 s |
| Inlink-Seitenabruf bei 1 Mio. Inlinks, erste Seite | 134 ms | 353 ms |
| OUT-001 Speicherzuwachs | 1 MiB | 1 MiB |
| OUT-031 1 Mio. Links als CSV | 2,26 s | 2,27 s |

Der Abstand zwischen gleichverteilten und Seiten-ähnlichen Zielen (N14: Faktor 4,1) ist nach K-74/K-75 auf den Faktor 1,9 geschrumpft. Der Inlink-Seitenabruf (`inlinks_paging_with_one_million_inlinks`) hat sich bei der ersten Seite von 134 auf 353 ms verlängert, bei Offset 500 000 von 377 auf 342 ms verkürzt. Der Test hat keine Zeitgrenze und es gibt kein Spec-Kriterium dazu; die Ursache (Streuung oder K-74/K-75) ist nicht untersucht.

## Einordnung

- **CRAWL-126 Fortsetzen mit 1 Mio. Warteschlangeneinträgen:** Wiederaufbau aus der Datei 0,65 s (P19 hatte rund 5,7 s geschätzt, N14 1,23 s), Pausieren 10,9 ms (Grenze < 3 s).
- **CRAWL-127:** Öffnen 1 ms gegen < 3 s, Speichern unter 4,8 s gegen < 60 s. `COUNT(*)` über 1 Mio. URLs dauert 0,93 s (warmer Cache), nicht Teil des Kriteriums.
- **Writer:** Im Store-Benchmark `writer_1m_uniform_segments` ist der Durchsatz über 1 Mio. Seiten flach (98 % bis 103 % des Mittels). Im echten Crawl (PERF-001 large, `../loadtest/README.md`) fällt er dagegen von rund 3 900 auf rund 1 400 URLs/s: das Auseinanderklaffen von Store-Benchmark und Crawl steht im Nachbesserungsauftrag zu PERF-002.

## Rohausgabe

Gekürzt auf Kopf (Start, `wait_quiet`, Load, Befehl), Messzeilen und Ende:

- `os-export-pipeline_bench.txt`, `os-export-links_bench.txt`, `os-export-sitemap_gen.txt`, `os-export-xlsx_cells.txt`
- `os-engine-perf_store.txt`, `os-engine-lifecycle_pause.txt`, `os-engine-lifecycle_list.txt`
- `os-store-bench_writer.txt`
- `os-parse-sitemap_gzip.txt`, `os-parse-pixel_width.txt`
- `os-testsite-synthetic.txt`, `os-fetch-http_status.txt`
- `*.txt.load`: Lastproben im Lauf, jede Minute (nur bei Läufen über einer Minute).
