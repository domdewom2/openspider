# Großläufe (M1-N28, A4, nach Welle 7e)

Abnahmemessung der Kriterien PERF-001, PERF-002 (K-72, K-80), PERF-003 (K-73) und PERF-005 mit 1 Mio. URLs auf der lokalen Referenzmaschine, Stand nach Welle 7e (M1-N27 gemergt: Exact Duplicates als Gruppierung, fünf `urls`-Indizes ohne Aufbau im Endbatch, K-83/K-84). Ersetzt den Bericht von M1-N26 (Stand nach Welle 7c); dessen Berichte und Logs liegen unverändert als `n26-*` in diesem Ordner (`n26-README.md` ist sein Bericht), die der Messung M1-N21 als `n21-*`, die der Messung N14 als `n14-*`. `probe-n22/`, `probe-n27/`, `probe-p32/` und `probe-p33/` sind die Kurzproben der Entwicklung. Fortsetzen/Pausieren (CRAWL-126), CRAWL-127, OUT-001 und alle Benchmark-Ignores: siehe [`../benchmarks/README.md`](../benchmarks/README.md) (ebenfalls M1-N28, Block B6).

## Ergebnis in Kürze

**A4 ist erfüllt.** Beide Läufe von PERF-001 large sind `passed = true` mit leerem `crawler_violations` (einschließlich K-80), der Endbatch liegt bei **25,0 s und 24,2 s** (Grenze 60 s, Ziel 45 s), und alle zehn Abschnitte liegen zwischen 85 % und 121 % des Mittelwerts (Grenze 70 % bis 130 %). Der Mittelwert steigt gegenüber M1-N26 von 2 731 auf 3 612 URLs/s (+32 %). PERF-003 large (448,7 MB, `queue_empty`, Stillstand 3,2 s), PERF-005 large (10 von 10 Abbrüchen ok, Maximum 2 264 wiederholte Requests) und der Profilvergleich CRAWL-124 bei 1 Mio. Seiten (alle drei Profile `passed = true`, MinMemory -53,9 % RSS gegen Balanced) sind erfüllt. Die 21 Benchmark-Ignores bestehen (Block B6, Bericht in `../benchmarks/README.md`). Ein Nachbesserungsauftrag entsteht nicht.

| Kriterium (M1.md Abschnitt 6, A4) | Grenze | Lauf 1 (B1) | Lauf 2 (B2) | N26 (vorher, Lauf 1 / 2) | bestanden |
|---|---|---|---|---|---|
| PERF-001: `passed` | `true` | `true` | `true` | `false` / `false` | ja |
| PERF-001: `crawler_violations` leer (inkl. K-80) | leer | **leer** | **leer** | 9 Einträge / 9 Einträge | ja |
| PERF-001: Spitzen-RSS bei 1 Mio. URLs x 30 Links | < 1,5 GB | **466,7 MB** | **468,2 MB** | 468,5 / 461,2 MB | ja |
| PERF-001: RSS(1 Mio.) / RSS(100 k) | < 2 | **1,126** (466,7 / 414,3 MB) | **1,103** (468,2 / 424,6 MB) | 1,115 / 1,097 | ja |
| PERF-001: Cache-Grenze K-16 | Balanced 256 MiB; MaxSpeed <= min(25 % RAM, 4 GiB) | Balanced 268 435 456 B; MaxSpeed 4 GiB | gleich | gleich | ja |
| PERF-001: längster Stillstand (K-73) | <= 30 s | **1,994 s** | **1,099 s** | 1,985 / 2,348 s | ja |
| PERF-002 (K-72): Mittel abgerufene URLs ÷ Crawl-Dauer | >= 500 URLs/s | **3 644,9** (274,4 s) | **3 579,9** (279,3 s) | 2 736,6 / 2 725,3 | ja |
| **PERF-002 (K-72): jeder der zehn 10-%-Abschnitte zwischen 70 % und 130 % des Mittelwerts** | 70 % bis 130 % (B1: 2 551 bis 4 738 URLs/s; B2: 2 506 bis 4 654) | **85 bis 118 %** (Abschnitt 1: 85 %, größter: 118 %) | **91 bis 121 %** (Abschnitt 1: 91 %, größter: 121 %) | Abschnitte 2 bis 10 über 130 % | ja |
| **K-80: Endbatch (`crawler_end_batch_s`)** | <= 60 s bei 1 Mio. (Ziel 45 s) | **25,0 s** | **24,2 s** | 98,8 / 96,1 s | ja |
| PERF-003 (B3): Spitzen-RSS bei 1 Mio. URLs, SlowSink 100 ms | < 500 MB | **448,7 MB** | | 461,9 MB | ja |
| PERF-003: Crawl endet regulär | `end_reason = queue_empty` | `queue_empty`, 1 000 000 URLs | | gleich | ja |
| PERF-003: längster Stillstand (K-73) | <= 30 s | **3,224 s** | | 2,646 s | ja |
| PERF-005 (B4): 10 harte Abbrüche, danach `integrity_check = ok` | je Abbruch ok | 10 von 10 ok | | 10 von 10 | ja |
| PERF-005: URL-Menge und Status je URL = Referenz, keine Dublette | identisch, 0 Dubletten | identisch (100 001 URLs, 0 fehlend, 0 zusätzlich, 0 abweichender Status), 0 Dubletten | | gleich | ja |
| PERF-005: wiederholte Requests je Abbruch (K-49) | <= 2 264 (8 + 256 + 2 000) | `[2264, 2251, 2264, 2264, 2264, 2264, 2264, 2264, 2264, 2010]`, Maximum **2 264** | | `[2264, 1640, 2264, 2264, 2264, 2264, 2264, 2264, 1663, 2066]`, Maximum 2 264 | ja (an der Grenze) |
| PERF-005: Vorkommen von „abgeschnitten“ in Bericht und Log | 0 | 0 | | 0 | ja |
| CRAWL-124 (B5): Profilvergleich bei 1 Mio. Seiten | alle drei Profile `passed` | Balanced 482,0 MB, MaxSpeed 5 412,7 MB (nicht bewertet, K-16), MinMemory 222,1 MB; alle `passed = true`, leeres `crawler_violations` | | N21: alle drei `passed = false` (Abschnittsregel) | ja |

Die Grenzen 70 % bis 130 % rechnen mit dem Mittelwert 3 644,9 (B1) bzw. 3 579,9 URLs/s (B2): 2 551 bis 4 738 und 2 506 bis 4 654 URLs/s.

## Maschine und Stand

| | |
|---|---|
| Rechner | Apple M2, 8 Kerne, 16 GB RAM (`sysctl hw.memsize` = 17 179 869 184 B) |
| macOS | 26.6.2 |
| Commit | Branch `m1/m1-n28` auf `origin/main` b6ba90a (`m1: Merge m1-n27 (Welle 7e)`, Abnahme-Commit; `origin/main` war bei Start und Ende von M1-N28 derselbe Commit); an Produktcode und Tests geändert: nichts (`git diff origin/main -- crates tests Cargo.toml Cargo.lock .github` ist leer). Grüner main-Lauf auf b6ba90a: [38977333721](https://github.com/domdewom2/openspider/actions/runs/38977333721) (CI, 16 von 16 Jobs), dazu `linux-core` und `cloud-setup` |
| Build | Release, `cargo build --release --locked -p os-loadtest` und `-p os-engine --example crawl_child`, `CARGO_INCREMENTAL=0`, `RUSTC_WRAPPER=sccache`; Target-Ordner `openspider-wt/target-m1-n28`; einmal vor den Blöcken gebaut; danach wurde bis zum Ende von B5 nichts mehr gebaut. In B6 hat `cargo test` vor einzelnen Benchmarks Crates erneut übersetzt (siehe `../benchmarks/README.md`), die lokale Gegenprobe lief erst nach B6 |
| Konstanten | `BATCH_PAGES_BACKLOG` 2 000, `CHANNEL_CAPACITY` 256, 8 Threads, Profil Balanced (K-49 unverändert) |
| Wachhalten | `caffeinate -dimsu` um jeden Lauf (`scripts/bench/run_acceptance.sh`, Wartegrenze 120 min) |
| Arbeitsordner | `openspider-wt/n28-work/<block>` außerhalb des Repos; der Lasttest löscht die Crawl-Dateien am Ende jedes Laufs, der Ordner wurde danach entfernt (Spitze durch die 6,1-GB-Crawl-Datei); freier Platz vor jedem Block 69 bis 80 GB (Vorgabe >= 20 GB) |
| Parallele Läufe | keine eigenen: es gab nur den Hauptcheckout und `m1-n28` (`git worktree list`), keinen M2-Worktree; während der Blöcke B1 bis B5 wurde nichts gebaut oder getestet |
| Fremdlast | Auf der Maschine lief außerhalb dieses Pakets ein Prozess des Nutzers (`python -m pytest tests/messung_50k.py --e2e`, PID 50029, ab 03:00 mit rund 100 % CPU, dazu eine Virtualization-VM mit bis zu 6,6 GB Speicher). Sie hielt den 1-Minuten-Load zwischen 03:05 und 04:30 bei 2,2 bis 4,0 und verlängerte die Wartezeit von B3 auf 87 Minuten; um 05:36 liefen beide Prozesse nicht mehr (Swap zu diesem Zeitpunkt 4,4 von 5,1 GB belegt). Sie wurde nicht angetastet |

### Last je Block

Jeder Block wurde erst gestartet, nachdem `scripts/bench/wait_quiet.sh 2 120` den 1-Minuten-Load unter 2 gesehen hatte; die Wartegrenze von 120 min wurde nie erreicht (längste Wartezeit B3: 87 min), kein Block wurde verworfen. Load < 1 war wie in N14, N21 und N26 nicht erreichbar; die Grundlast liegt bei 1,5 bis 2,5 (Dienste `fileproviderd`, `mds`, `syspolicyd`, `spindump`). Die Proben im Lauf (`log-*.txt.load`, jede Minute: Load 1/5/15 min) zeigen zusätzlich die Last des Lasttests selbst (Testserver, Crawler, Writer: rund 3 Kerne).

| Block | Lauf | Wartezeit (rund) | Load (1 min) beim Start | Load-Proben im Lauf (1 min, Minimum bis Maximum) |
|---|---|---:|---:|---|
| B1 | PERF-001 large, Lauf 1 | 14 min (02:31 bis 02:45; Restlast des Release-Builds) | 1,99 | 1,99 bis 6,25 (5 Proben) |
| B2 | PERF-001 large, Lauf 2 | 8 min | 1,77 | 1,77 bis 6,53 (6 Proben) |
| B3 | PERF-003 large | 87 min (Fremdlast, siehe oben) | 1,83 | 2,00 bis 5,13 (6 Proben) |
| B4 | PERF-005 large, `--kills 10` | 18 min | 1,95 | 1,95 und 3,16 (Lauf 61 s) |
| B5 | CRAWL-124, Balanced | 11 min | 1,87 | 1,87 bis 5,16 (5 Proben) |
| B5 | CRAWL-124, MaxSpeed | 12 min | 1,94 | 1,94 bis 4,99 (5 Proben) |
| B5 | CRAWL-124, MinMemory | 2 min | 1,75 | 1,75 bis 4,09 (5 Proben) |

B3 lief im Durchsatz auffällig langsamer als die übrigen Blöcke (Mittel 2 829,5 URLs/s gegen 3 580 bis 4 104; Testserver-Probe 91 780 Requests/s gegen 120 979 bis 124 157): Die Ursache ist nicht gemessen; wahrscheinlich war die Maschine noch durch die Fremdlast belegt (die VM lief um 04:19 noch), obwohl der Load beim Start unter 2 lag. Die Kriterien von PERF-003 (RSS, regulärer Abschluss, Stillstand) hängen davon nicht ab und sind mit großer Reserve erfüllt; der Block wird gewertet, weil seine Grenzen ausdrücklich nur RSS, `queue_empty` und Stillstand sind.

## Läufe, Befehle, Laufzeiten

Alle Befehle aus dem Worktree `m1-n28`; `<ordner>` = Arbeitsordner außerhalb des Repos, `os-loadtest` = `target-m1-n28/release/os-loadtest`. Aufruf jeweils über `scripts/bench/run_acceptance.sh <logdatei> <befehl>` (wartet auf Load < 2, schreibt Start/Ende, Load und die Lastproben).

| Block | Befehl (`os-loadtest run …`) | Berichte und Logs | Laufzeit (Start–Ende, lokal, 17.10.2026) |
|---|---|---|---|
| B1 PERF-001 Lauf 1 | `--scenario perf-001 --size large --work-dir <ordner> --out docs/plan/M1-acceptance/loadtest/report-perf-001-large-run1.json` | `report-perf-001-large-run1.json`, `log-perf-001-large-run1.txt` | 02:45–02:50 (Crawl 274,4 s), Exit 0 |
| B2 PERF-001 Lauf 2 | wie B1, `…-run2.json` | `report-perf-001-large-run2.json`, `log-perf-001-large-run2.txt` | 02:59–03:04 (Crawl 279,3 s), Exit 0 |
| B3 PERF-003 | `--scenario perf-003 --size large …` | `report-perf-003-large.json`, `log-perf-003-large.txt` | 04:32–04:38 (Crawl 353,4 s), Exit 0 |
| B4 PERF-005 | `--scenario perf-005 --size large --kills 10 …` | `report-perf-005-large.json`, `log-perf-005-large.txt` | 04:56–04:57 (Crawl 36,4 s), Exit 0 |
| B5 CRAWL-124 Balanced | `--scenario perf-001 --size large --profile balanced …` | `report-crawl-124-balanced-1m.json`, `log-crawl-124-balanced-1m.txt` | 05:08–05:12 (Crawl 251,0 s), Exit 0 |
| B5 CRAWL-124 MaxSpeed | `… --profile max-speed …` | `report-crawl-124-max-speed-1m.json`, `log-crawl-124-max-speed-1m.txt` | 05:24–05:29 (Crawl 243,7 s), Exit 0 |
| B5 CRAWL-124 MinMemory | `… --profile min-memory …` | `report-crawl-124-min-memory-1m.json`, `log-crawl-124-min-memory-1m.txt` | 05:31–05:36 (Crawl 244,5 s), Exit 0 |
| B6 Benchmark-Ignores | `scripts/bench/run_ignored_benchmarks.sh <ausgabeordner> <arbeitsordner>` | `../benchmarks/` | siehe `../benchmarks/README.md` |

Die lokale Gegenprobe (Workspace-Tests, Parität, CLI-E2E, MCP-E2E) steht in `../checklist.md`, Abschnitt „Lokale Gegenprobe“.

## Einzelergebnisse

### B1 und B2: PERF-001 large (1 000 000 URLs x 30 Links), zwei Läufe

| Messwert | Lauf 1 | Lauf 2 |
|---|---|---|
| Spitzen-RSS (`crawler_peak_rss_bytes`) | 466 665 472 B = 466,7 MB (Grenze 1 500 000 000) | 468 221 952 B = 468,2 MB |
| Referenz 100 000 Seiten | 414 318 592 B = 414,3 MB; Verhältnis 1,126 (Regel `ratio`, Grenze 2,0) | 424 591 360 B = 424,6 MB; Verhältnis 1,103 |
| URLs abgerufen | 1 000 000 in 274,4 s = 3 644,9 URLs/s (N26: 365,4 s, 2 736,6) | 1 000 000 in 279,3 s = 3 579,9 URLs/s (N26: 366,9 s, 2 725,3) |
| Zeit bis zur letzten abgerufenen URL (`crawler_seconds` - `crawler_tail_s`) | 248,95 s = 4 016,8 URLs/s | 254,73 s = 3 925,8 URLs/s |
| Abschnitte (URLs/s, 1 bis 10) | 3 096, 3 974, 4 255, 4 148, 4 194, 4 240, 4 248, 4 189, 4 312, 3 869 | 3 245, 4 318, 4 200, 3 723, 4 005, 4 011, 3 995, 3 961, 3 975, 4 042 |
| Abschnitte in % des Mittelwerts | 85, 109, 117, 114, 115, 116, 117, 115, 118, 106: **innerhalb 70 bis 130 %** | 91, 121, 117, 104, 112, 112, 112, 111, 111, 113: **innerhalb 70 bis 130 %** |
| Abschnitte in % des Durchsatzes bis zur letzten abgerufenen URL (Rechnung, nicht Kriterium) | 77, 99, 106, 103, 104, 106, 106, 104, 107, 96 | 83, 110, 107, 95, 102, 102, 102, 101, 101, 103 |
| Endbatch (`crawler_end_batch_s`) | **25,043 s** | **24,179 s** |
| Nachlauf (`crawler_tail_s`: Crawl-Dauer minus Zeit bis zur letzten URL) | 25,403 s | 24,610 s |
| Längster Stillstand (`crawler_max_stall_s`) | 1,994 s (Grenze 30 s) | 1,099 s |
| Dateigröße | 6 127 230 976 B (Referenz 100 000 Seiten: 603 467 776 B; N26: 6 276 964 352 B, ohne die fünf `urls`-Indizes 150 MB kleiner) | 6 127 292 416 B (Referenz: 603 488 256 B) |
| Profil / Cache | Balanced, 268 435 456 B (256 MiB); MaxSpeed-Cache 4 GiB (K-16: 25 % von 16 GiB, höchstens 4 GiB) | gleich |
| Testserver (100 000 Probe-Requests) | 123 541 Requests/s, p50 0,244 ms, p99 0,599 ms, 0 Fehler, deterministisch | 120 979 Requests/s, p50 0,243 ms, p99 0,685 ms, 0 Fehler, deterministisch |
| Verstöße / Warnungen | keine / keine | keine / keine |

**Endbatch-Bestandteile** (Dauer des Schreibaufrufs mit `Completed`, `engine_w_end_batch_ns`, ein Aufruf; Bestandteile aus den Zählern von `os-store`):

| Bestandteil | Lauf 1 | Lauf 2 | N26 (Lauf 1 / 2) |
|---|---:|---:|---:|
| Endbatch gesamt | **25,04 s** | **24,18 s** | 98,8 / 96,1 s |
| Daten des letzten Batches (`store_end_data_ns`) | 0,17 s | 0,17 s | n. e. |
| Aufbau `links_target` (`store_link_index_build_ns`, ein Aufbau) | 21,52 s | 20,72 s | 33,7 / 35,2 s |
| Aufbau der fünf `urls`-Indizes (`store_url_index_build_ns`) | **0 s (kein Aufbau, 0 Aufrufe)** | **0 s** | 57,6 / 53,8 s |
| Vorwärmen der Leser (`store_url_index_prefetch_ns`) | **0 s** | **0 s** | 14,8 / 10,1 s |
| Commit des letzten Eimers (`store_end_commit_ns`) | 3,35 s | 3,28 s | 7,4 / 6,8 s |

Die Summe der drei Bestandteile (25,04 s und 24,17 s) ist der Endbatch. Den Rückgang gegenüber N26 (rund 74 s) trägt zu rund 78 % der Wegfall der fünf Indexaufbauten samt Vorwärmen (K-83, 57,6 s in N26 Lauf 1), zu rund 17 % der kürzere `links_target`-Aufbau (21,5 gegen 33,7 s; das Verfahren ist unverändert: zwei Hilfsthreads, 4 MiB Cache, K-84 (3) in N27 nicht verändert; N27 maß im Crawl 22 bis 26 s; die Ursache der Differenz zu N26 ist nicht untersucht) und zu rund 5 % der kleinere Commit des letzten Eimers (3,3 gegen 7,4 s). WAL-Rückführung: 2 Checkpoints (0,33 s und 0,40 s zusammen), kein Hintergrund-Checkpoint; größte WAL-Größe 1,03 GB im Endbatch.

**Commit-Verlauf je 100 Batches (`diag`, M1-N22; Summe in Sekunden, Eimer 0 bis 5; der letzte Eimer enthält die Restbatches einschließlich Endbatch):**

| | Eimer 0 | 1 | 2 | 3 | 4 | 5 (Rest, 22 Batches) |
|---|---:|---:|---:|---:|---:|---:|
| Lauf 1: Commit (`store_commit_ns_h0n`) | 13,3 | 12,8 | 11,6 | 11,8 | 10,7 | 7,5 |
| Lauf 1: Aufbau (`store_build_ns_h0n`) | 26,7 | 21,3 | 21,7 | 22,0 | 21,6 | 5,1 |
| Lauf 1: größter Commit (`store_commit_max_ns_h0n`) | 0,83 | 1,87 | 0,78 | 0,68 | 0,69 | 3,35 |
| Lauf 2: Commit | 12,1 | 14,3 | 12,3 | 11,8 | 12,3 | 5,7 |
| Lauf 2: Aufbau | 26,2 | 21,8 | 22,3 | 22,2 | 22,9 | 5,1 |
| Lauf 2: größter Commit | 0,67 | 0,80 | 0,86 | 0,84 | 0,88 | 3,28 |

Der Commit je 100 Batches ist flach (letzter voller Eimer 10,7 s und 12,3 s, 0,80- bzw. 1,02-fach des ersten, 13,3 s und 12,1 s); Commit-Summe 67,6 s und 68,6 s (N26: 90,5 s und 92,4 s; N21: 248,5 s), Aufbau 118,4 s und 120,5 s (N26: 120,0 s und 121,5 s), Schreibzeit 209,1 s und 211,4 s (N26: 303,4 s und 304,6 s; die Differenz ist der Endbatch von 98 s auf 25 s und der kleinere Commit). 500 und 501 von 522 Commits gelten als „slow“ (N26: 501 von 523). Der größte Commit im Crawl (ohne Endbatch) liegt bei 1,87 s und 0,88 s.

**Diagnose Speicher und Warteschlange der Engine (Ende):** Seen-Menge 24,6 MB (1 000 001 Einträge, 0 Überläufe, 0 Kollisionen), Warteschlange im RAM 4,7 MB und 4,6 MB (23 040 und 23 018 Einträge; 880 402 und 881 767 auf Platte), 99 Nachladevorgänge, 1 Trim, Kern 29,5 MB und 29,4 MB, größter Writer-Batch 34,8 MB und 33,7 MB, 0 Fetch-Warnungen (`engine_fetch_slow` 0).

### B3: PERF-003 large (SlowSink 100 ms je Schreibaufruf, K-52; 1 Mio. URLs, K-73)

| Messwert | Wert |
|---|---|
| Spitzen-RSS | 448 708 608 B = 448,7 MB (Grenze 500 000 000; Grenze der Abschlussprobe von N27 war 490 MB), bestanden |
| Crawl | endet regulär (`end_reason = queue_empty`), 1 000 000 URLs, 353,4 s = 2 829,5 URLs/s (N26: 414,2 s, 2 414,1) |
| Längster Stillstand | 3,224 s (Grenze 30 s) |
| Endbatch / Nachlauf | 23,532 s / 23,983 s; Bestandteile: `links_target` 19,92 s, Commit 3,32 s, Daten 0,17 s, kein Aufbau der `urls`-Indizes |
| `diag`: Zahl der Batches | 526 (`store_batches`; 500 mit mehr als 1 000 Seiten, 1 mit bis zu 1 000, 1 mit bis zu 200, 24 mit bis zu 50) |
| `diag`: `engine_w_pages_max` | 2 000 (= `BATCH_PAGES_BACKLOG`) |
| `diag`: `engine_w_queue_max` | 256 (= `CHANNEL_CAPACITY`; der Kanal war voll) |
| `diag`: `loadtest_slow_wait_ns` | 54 813 582 112 ns = 54,8 s bei 526 Schreibaufrufen (`loadtest_slow_calls`) |
| `diag`: Hintergrund-Checkpoints | 0, Checkpoints im Commit: 2, zusammen 0,42 s |
| `diag`: Schreibzeit | gesamt 287,4 s, Commit gesamt 93,2 s (größter 3,32 s im Endbatch; 501 von 526 „slow“), Aufbau 117,9 s |
| `diag`: Speicherzähler der Engine | Seen-Menge 24,6 MB (1 000 001 Einträge), Warteschlange im RAM 4,8 MB (23 024 Einträge, 881 701 auf Platte, 99 Nachladevorgänge, 1 Trim), Weiterleitungen 0,5 MB (19 975 Einträge), Kern 29,1 MB, größter Writer-Batch 31,3 MB |
| Abschnitte (nur Bericht, PERF-002 gilt in PERF-001) | 2 084, 3 139, 3 234, 3 287, 3 314, 3 256, 3 010, 2 577, 3 631, 3 598 URLs/s = 74, 111, 114, 116, 117, 115, 106, 91, 128, 127 % des Mittelwerts 2 829,5 (vermutlich Fremdlast, siehe oben) |

Das Drosseln selbst belegt der Test `full_channel_blocks_the_sender_until_the_writer_catches_up` (`writer_thread.rs`, Tag PERF-003); hier nur zur Einordnung (K-52). Der RSS liegt mit 448,7 MB 51,3 MB unter der Grenze (N26: 461,9 MB, N21: 468,1 MB).

### B4: PERF-005 large (100 000 Seiten, `--kills 10`, Seed 20261014)

- Kill-Punkte (abgerufene Seiten): 14 177, 15 814, 27 500, 34 721, 38 982, 46 925, 61 498, 64 126, 71 705, 84 155 (wie N21 und N26).
- Je Abbruch: `integrity_check = ok` (10 von 10), `duplicate_urls` = 0 (10 von 10).
- Nach dem Fortsetzen: 100 001 URLs, 100 000 abgerufen, `urls_missing` 0, `urls_extra` 0, `urls_with_other_status` 0, Status identisch mit dem Referenzcrawl (`crawler_status_identical` = true; 200: 96 582, 301: 1 973, 404: 973, 500: 236, 503: 237).
- Wiederholte Requests je Abbruch (`crawler_repeated_per_kill`): **[2264, 2251, 2264, 2264, 2264, 2264, 2264, 2264, 2264, 2010]**, Maximum 2 264, Mittel 2 237,3, Summe 22 373 (Gesamtgrenze 22 640). Grenze je Abbruch (`crawler_repeated_bound_per_kill`, K-49) = 8 Threads + 256 Kanal + 2 000 Batch = **2 264**: nie überschritten, in 8 von 10 Abbrüchen genau erreicht (N26: 7 von 10, N21: 9 von 10). Vor dem ersten Abbruch keine wiederholten Requests (`crawler_repeated_before_first_kill` 0); robots.txt 1 Abruf; Requests insgesamt 122 374.
- Je Abbruch in der Pipeline verloren (`lost_in_pipeline`: abgerufen, aber nicht in der Datei): 2 255, 1 986, 2 255, 2 255, 2 162, 2 255, 2 255, 2 255, 1 957, 1 476.
- „abgeschnitten“ kommt in Bericht und Log 0-mal vor.
- Laufzeit des Crawls 36,4 s (N26: 39,4 s). Testserver 120 180 Requests/s, p99 0,663 ms, 0 Fehler.

### B5: CRAWL-124, Profilvergleich bei 1 Mio. Seiten (`perf-001 --size large --profile <profil>`)

Wie N21 (`n21-report-crawl-124-*-1m.json`): derselbe Lauf wie B1 mit je einem Profil. MaxSpeed wird seit N19 nicht mit den RSS-Grenzen von PERF-001 bewertet (K-16).

| Profil | `passed` | Verstöße | Spitzen-RSS | RSS gegen Balanced | Mittel URLs/s | Crawl-Dauer | Abschnitte in % des Mittelwerts | Stillstand | Endbatch | Cache |
|---|---|---:|---:|---:|---:|---:|---|---:|---:|---|
| Balanced | ja | 0 | 482,0 MB | | 3 984,4 | 251,0 s | 86 bis 114 | 0,80 s | 22,0 s | 256 MiB |
| MaxSpeed | ja | 0 | 5 412,7 MB | +1 023 % (nicht bewertet, K-16) | 4 103,6 | 243,7 s | 84 bis 117 | 0,85 s | 24,1 s | 4 GiB |
| MinMemory | ja | 0 | 222,1 MB | **-53,9 %** | 4 089,7 | 244,5 s | 78 bis 116 | 0,97 s | 20,5 s | 64 MiB |

Zum Vergleich N21 (Stand nach Welle 7): Balanced 469,9 MB, MaxSpeed 5 437,2 MB, MinMemory 217,4 MB (-53,7 % gegen Balanced); Mittel 2 244,3 / 2 034,8 / 2 324,0 URLs/s; alle drei Läufe `passed = false` (Abschnittsregel). Die Reihenfolge der RSS-Werte ist unverändert (MinMemory < Balanced < MaxSpeed), die Differenz von MinMemory gegen Balanced beträgt weiter rund 54 %. MaxSpeed hat mit 4 104 gegen 3 984 URLs/s (+3,0 %) weiter keinen nennenswerten Tempovorteil bei 5,4 GB RSS (Beobachtung, M2-Backlog; kein Kriterium).

### B6: Benchmark-Ignores

Siehe [`../benchmarks/README.md`](../benchmarks/README.md).

## Vergleich mit N26 (Stand nach Welle 7c) und N21 (Stand nach Welle 7)

| | N21 (88fd424) | N26 (5f65ffc, Welle 7c) | N28 (b6ba90a, Welle 7e) | Veränderung gegen N26 |
|---|---|---|---|---|
| PERF-001 Mittel (URLs/s) | 2 098,8 | 2 736,6 / 2 725,3 | 3 644,9 / 3 579,9 | +33 % / +31 % |
| PERF-001 Crawl-Dauer | 476,5 s | 365,4 s / 366,9 s | 274,4 s / 279,3 s | -25 % / -24 % |
| PERF-001 Abschnitte relativ zum Mittelwert | 68 % bis 186 %, stetig fallend | 97 % bis 152 %, Abschnitte 2 bis 10 über 130 % | 85 % bis 118 % / 91 % bis 121 % | Regel erfüllt |
| PERF-001 `crawler_violations` | 6 Einträge | 9 / 9 Einträge | leer / leer | behoben |
| Endbatch | 39,5 s | 98,8 s / 96,1 s | 25,0 s / 24,2 s | -74 s / -72 s |
| Commit-Summe | 248,5 s | 90,5 s / 92,4 s | 67,6 s / 68,6 s | -25 % / -26 % |
| PERF-001 Spitzen-RSS | 471,1 MB | 468,5 / 461,2 MB | 466,7 / 468,2 MB | +-0,4 % / +1,5 % |
| PERF-001 längster Stillstand | 6,085 s | 1,985 / 2,348 s | 1,994 / 1,099 s | gleich / besser |
| PERF-003 Spitzen-RSS | 468,1 MB | 461,9 MB | 448,7 MB | -2,9 % |
| PERF-003 Stillstand | 4,308 s | 2,646 s | 3,224 s | +0,6 s (Fremdlast) |
| PERF-005 Maximum wiederholte Requests je Abbruch | 2 264 | 2 264 | 2 264 | gleich (Grenze scharf) |

## Dateien in diesem Ordner

- `report-perf-001-large-run1.json`, `report-perf-001-large-run2.json`, `report-perf-003-large.json`, `report-perf-005-large.json`, `report-crawl-124-{balanced,max-speed,min-memory}-1m.json`: Berichte von `os-loadtest` (Schema `{scenario, size, started_at, metrics, passed}`) dieser Messung (M1-N28).
- `log-*.txt`: Kopf (Start, `wait_quiet`-Ergebnis, Load, Befehl), Konsolenausgabe und Ende je Block; `log-*.txt.load`: Lastproben jede Minute (Zeit, Load 1/5/15 min).
- `n26-*`: Berichte, Logs und Bericht (`n26-README.md`) der Messung M1-N26 (Stand nach Welle 7c, Commit 5f65ffc), unverändert.
- `n21-*`: Berichte und Logs der Messung M1-N21 (Stand nach Welle 7, Commit 88fd424), unverändert; enthalten auch den Profilvergleich CRAWL-124 und den RSS-Trend von PERF-003.
- `n14-*`: Berichte und Logs der Messung N14 (vor Welle 7, Commit `43caf19`), nur zum Vergleich. Dort fehlt PERF-002.
- `probe-n22/`, `probe-n27/`, `probe-p32/`, `probe-p33/`: Kurzproben der Pakete M1-N22, M1-N27, M1-P32 und M1-P33 (Entwicklung).
