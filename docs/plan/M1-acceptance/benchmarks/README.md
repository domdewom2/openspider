# Benchmarks (M1-N14, Block B5 und B6)

Alle Tests mit `#[ignore = "Benchmark…"]` aus `crates/` (ohne `crates/os-parity/tests/data`), einzeln in Release ausgeführt:

```sh
cargo test --release --locked -p <crate> --test <datei> -- --ignored --nocapture --test-threads=1
```

Aufrufhilfe: `scripts/bench/run_bench.sh` (wartet vor jedem Lauf auf Load < 2, schreibt Load, Zeiten und Rohausgabe). Die Liste wurde vor B6 mit `grep -rn 'ignore = "Benchmark' crates --include='*.rs'` gegen den Stand `43caf19` abgeglichen: 16 Tests in 12 Dateien, alle ausgeführt, kein Benchmark-Ignore ohne Lauf. Die Ignores mit `M2:`-Präfix sind keine Benchmarks und bleiben unberührt.

## Maschine und Stand

| | |
|---|---|
| Rechner | Apple M2, 8 Kerne, 16 GB RAM (lokale Referenzmaschine) |
| macOS | 26.6.2 |
| Commit | `43caf19` (origin/main, Welle 6 gemergt), Branch `m1/m1-n14` |
| Build | Release, `--locked`, `CARGO_INCREMENTAL=0` |
| Datum | 15.10.2026, 03:31 bis 03:47 (lokal), unter `caffeinate -dimsu` |
| Load (1 min) vor dem Start jedes Tests | 1,61 bis 1,98 (Grenze < 2), je Rohdatei im Kopf; die Lasterhöhung am Ende eines Laufs (bis 9,98 nach `sitemap_gzip`) stammt vom Test selbst bzw. Hintergrundprozessen nach dem Messen |

## Ergebnis

16 von 16 Benchmarks bestanden, keine Grenzverletzung.

| Test | Datei | Spec-ID | Grenze | Messwert | bestanden | Laufzeit |
|---|---|---|---|---|---|---|
| `one_million_rows_by_sixty_columns_stay_within_300_mb_of_extra_memory` | `os-export/tests/pipeline_bench.rs` | OUT-001 | 1 Mio. x 60 Spalten, Speicherzuwachs <= 300 MB | Export in 7,1 s (Anlegen 3,4 s), Datei 336 MiB, RSS vorher 329 MiB, Spitze 329 MiB, Zuwachs 1 MiB | ja | 10,6 s |
| `one_million_links_are_written_as_csv_in_under_60_seconds` | `os-export/tests/links_bench.rs` | OUT-031 | 1 Mio. Links < 60 s | 2,26 s, 150 MiB; RSS vorher 254 MiB, Spitze 432 MiB | ja | 4,2 s |
| `default_byte_limit_of_50_mib_splits_files_with_very_long_urls` | `os-export/tests/sitemap_gen.rs` | OUT-129 | 26 000 URLs a 2 000 Zeichen (rund 52 MB) teilen sich in >= 2 Dateien, jede <= 52 428 800 Byte | Zusicherungen erfüllt (keine Zahl ausgegeben) | ja | 1,05 s |
| `default_limit_overflows_after_1048575_rows` | `os-export/tests/xlsx_cells.rs` | OUT-003 | 1,1 Mio. Zeilen: Blatt 1 mit 1 048 576 Zeilen (Kopf + 1 048 575), Blatt 2 mit dem Rest | Zusicherungen erfüllt (keine Zahl ausgegeben) | ja | 2,3 s |
| `opening_a_million_urls_takes_under_3_seconds_and_saving_under_60` | `os-engine/tests/perf_store.rs` | CRAWL-127 | Öffnen von 1 Mio. URLs < 3 s, Speichern unter < 60 s | Öffnen 4 ms (+ erste 100 Zeilen 8 ms, warmer Dateicache), Speichern unter 5,7 s (Kopie 635 MiB); Schreiben der Testdatei 27 s, 687 MiB; `COUNT(*)` über `urls` 1,12 s (nur Bericht) | ja | Teil von 97,4 s |
| `group_commit_versus_small_batches_with_uniform_targets` | `os-engine/tests/perf_store.rs` | CRAWL-124 | nur Bericht (Gruppen-Commit, gleichverteilte Ziele) | 100 000 Seiten x 30 Links: 200 Seiten je Batch 27,9 s, 1 000 Seiten 20,6 s (-26 %), 5 000 Seiten 14,6 s (-48 %) | ja | Teil von 97,4 s |
| `pausing_with_a_million_queue_entries_takes_under_three_seconds` | `os-engine/tests/lifecycle_pause.rs` | CRAWL-126 | Pausieren mit 1 Mio. Warteschlangeneinträgen < 3 s | Fortsetzen (Wiederaufbau aus der Datei) 1,23 s, Pausieren 71 ms | ja | 5,7 s |
| `a_list_of_one_hundred_thousand_urls_is_read_in_under_ten_seconds` | `os-engine/tests/lifecycle_list.rs` | CRAWL-027 | 100 000 Zeilen < 10 s | Test-Gesamtlaufzeit 0,15 s (Einlesen und Planen), Zusicherungen erfüllt | ja | 0,15 s |
| `writer_100k_pages_with_30_links_under_60_seconds` | `os-store/tests/bench_writer.rs` | – (kein Tag; Writer-Rahmen PERF-001) | 100 000 Seiten x 30 Links < 60 s je Profil | Balanced 6,9 s (14 562 Seiten/s), MaxSpeed 6,9 s (14 583 Seiten/s), MinMemory 6,7 s (15 024 Seiten/s); Datei 315 MiB | ja | Teil von 55,1 s |
| `writer_100k_pages_uniform_targets_report` | `os-store/tests/bench_writer.rs` | – (nur Bericht) | keine | Balanced, gleichverteilte Ziele: 28,5 s (3 513 Seiten/s, 105 396 Links/s), Datei 321 MiB | ja | Teil von 55,1 s |
| `inlinks_paging_with_one_million_inlinks` | `os-store/tests/bench_writer.rs` | – (Inlink-Seitenabruf) | Seitenabruf bei 1 Mio. Inlinks (Gesamtzahl 1 000 000, Seite mit 50 Zeilen) | erste Seite 134 ms, Offset 500 000 377 ms, Offset 500 000 nach Anchor 1 270 ms | ja | Teil von 55,1 s |
| `over_50_mib_unpacked_is_read_completely_by_streaming` | `os-parse/tests/sitemap_gzip.rs` | CRAWL-060 | Sitemap > 50 MiB entpackt wird vollständig gelesen (2 `loc`, `size_bytes` > 52 428 800) | Zusicherungen erfüllt (keine Zahl ausgegeben) | ja | 0,06 s |
| `throughput_report` | `os-parse/tests/pixel_width.rs` | UI-038 | nur Bericht | 42,8 Mio. Titel/min | ja | 1,4 s |
| `benchmark_20k_requests_over_5000_per_second` | `os-testsite/tests/synthetic.rs` | PERF-002 (Server darf den Durchsatz nicht begrenzen) | > 5 000 Requests/s | 125 978 Requests/s (20 000 Requests in 0,16 s, 52,7 MB) | ja | 0,16 s |
| `slow_route_response_time_is_between_800_and_1000_ms` | `os-fetch/tests/http_status.rs` | CRAWL-022 | Antwortzeit der Route `/slow-800` zwischen 800 und 1 000 ms | Zusicherung erfüllt (keine Zahl ausgegeben) | ja | Teil von 3,9 s |
| `timeout_two_seconds_against_three_second_route` | `os-fetch/tests/http_status.rs` | CRAWL-095 | Timeout 2 s gegen 3-s-Route: Abbruch nach 2 000 bis 2 500 ms, `/slow-1000` liefert 200 | Zusicherungen erfüllt (keine Zahl ausgegeben) | ja | Teil von 3,9 s |

Hinweis zu den Zeilen ohne Zahl: Die Tests drucken keinen Messwert, sondern prüfen die Grenze per Zusicherung; „bestanden“ heißt, dass alle Zusicherungen gehalten haben. Die Laufzeit steht in der Spalte „Laufzeit“ (Zeile `test result … finished in`).

## Rohausgabe

Gekürzt auf Kopf (Start, Load, Befehl), Messzeilen und Ende:

- `os-export-pipeline_bench.txt`, `os-export-links_bench.txt`, `os-export-sitemap_gen.txt`, `os-export-xlsx_cells.txt`
- `os-engine-perf_store.txt`, `os-engine-lifecycle_pause.txt`, `os-engine-lifecycle_list.txt`
- `os-store-bench_writer.txt`
- `os-parse-sitemap_gzip.txt`, `os-parse-pixel_width.txt`
- `os-testsite-synthetic.txt`, `os-fetch-http_status.txt`

## Einordnung

- **CRAWL-126 Fortsetzen mit 1 Mio. Warteschlangeneinträgen:** Wiederaufbau aus der Datei 1,23 s (P19 hatte rund 5,7 s geschätzt), Pausieren 71 ms (Grenze < 3 s).
- **CRAWL-127:** Öffnen 4 ms gegen < 3 s, Speichern unter 5,7 s gegen < 60 s. `COUNT(*)` über 1 Mio. URLs dauert 1,12 s (warmer Cache; P28 hatte 5 bis 7 s gemessen), nicht Teil des Kriteriums.
- **Writer mit gleichverteilten Zielen:** `bench_writer` zeigt bei 100 000 Seiten 28,5 s für gleichverteilte Linkziele gegen 6,9 s für Seiten-ähnliche Ziele. Das passt zum Befund im Großlauf (siehe `../loadtest/README.md`, Abschnitt „Beobachtung Durchsatz bei 1 Mio.“).
