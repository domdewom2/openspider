# Großläufe (M1-N26, A4, nach Welle 7c)

Abnahmemessung der Kriterien PERF-001, PERF-002 (K-72), PERF-003 (K-73) und PERF-005 mit 1 Mio. URLs auf der lokalen Referenzmaschine, Stand nach Welle 7c (M1-N22 PERF-002-Diagnose und Abhilfe, M1-N23, M1-N24, M1-N25 gemergt). Ersetzt den Bericht von M1-N21 (Stand nach Welle 7); dessen Berichte und Logs liegen unverändert als `n21-*` in diesem Ordner, die der Messung N14 als `n14-*`. `probe-n22/`, `probe-p32/` und `probe-p33/` sind die Kurzproben der Entwicklung. Fortsetzen/Pausieren (CRAWL-126), CRAWL-127, OUT-001 und alle Benchmark-Ignores: siehe [`../benchmarks/README.md`](../benchmarks/README.md) (M1-N21, Code seitdem nur durch Welle 7c verändert; die Wiederholung gehört nicht zu M1-N26).

## Ergebnis in Kürze

**A4 ist nicht erfüllt.** Alle RSS-, Stillstands- und Wiederaufnahme-Grenzen sind eingehalten, der Mittelwert liegt bei 2 725 bis 2 737 URLs/s (Grenze 500). Der in M1-N21 gemessene Verfall des Durchsatzes ist behoben: Die Abschnitte 2 bis 10 liegen im 1-Mio.-Crawl zwischen 3 615 und 4 143 URLs/s (96 % bis 112 % des Durchsatzes bis zur letzten abgerufenen URL). Verletzt bleiben zwei Punkte: **(1)** die Abschnittsgrenze 70 % bis 130 % des Mittelwerts (K-72) in beiden Läufen für neun von zehn Abschnitte, weil das Mittel durch den Endbatch von rund 100 s (Anteil 27 % der Crawl-Dauer) unter dem Durchsatz aller Abschnitte liegt, und **(2)** die Grenze des Endbatchs von 60 s bei 1 Mio. (K-80), gemessen 98,8 s und 96,1 s. Nachbesserungsauftrag und Rechnung unten.

| Kriterium (M1.md Abschnitt 6, A4) | Grenze | Lauf 1 (B1) | Lauf 2 (B2) | N21 (vorher) | bestanden |
|---|---|---|---|---|---|
| PERF-001: `crawler_violations` leer | leer | **9 Einträge** (Abschnitte 2 bis 10) | **9 Einträge** (Abschnitte 2 bis 10) | 6 Einträge | **nein** |
| PERF-001: Spitzen-RSS bei 1 Mio. URLs x 30 Links | < 1,5 GB | **468,5 MB** | **461,2 MB** | 471,1 MB | ja |
| PERF-001: RSS(1 Mio.) / RSS(100 k) | < 2 | **1,115** (468,5 / 420,3 MB) | **1,097** (461,2 / 420,4 MB) | 1,134 | ja |
| PERF-001: Cache-Grenze K-16 | Balanced 256 MiB; MaxSpeed <= min(25 % RAM, 4 GiB) | Balanced 268 435 456 B; MaxSpeed -4 194 304 KiB | gleich | gleich | ja |
| PERF-001: längster Stillstand (K-73) | <= 30 s | **1,985 s** | **2,348 s** | 6,085 s | ja |
| PERF-002 (K-72): Mittel abgerufene URLs ÷ Crawl-Dauer | >= 500 URLs/s | **2 736,6** (365,4 s) | **2 725,3** (366,9 s) | 2 098,8 (476,5 s) | ja |
| **PERF-002 (K-72): jeder der zehn 10-%-Abschnitte zwischen 70 % und 130 % des Mittelwerts** | 70 % bis 130 % (B1: 1 916 bis 3 558 URLs/s; B2: 1 908 bis 3 543) | Abschnitte 2 bis 10 über 130 % (132 bis 151 %), Abschnitt 1 mit 101 %; Spanne 101 % bis 151 % | Abschnitte 2 bis 10 über 130 % (136 bis 152 %), Abschnitt 1 mit 97 % | 68 % bis 186 % (Verfall) | **nein** |
| K-80: Endbatch (größter Schreibaufruf `engine_w_write_max_ns`, mit Indexaufbau) | <= 60 s bei 1 Mio. | **98,8 s** | **96,1 s** | 39,5 s | **nein** |
| PERF-003 (B3): Spitzen-RSS bei 1 Mio. URLs, SlowSink 100 ms | < 500 MB | **461,9 MB** | | 468,1 MB | ja |
| PERF-003: Crawl endet regulär | `end_reason = queue_empty` | `queue_empty`, 1 000 000 URLs | | gleich | ja |
| PERF-003: längster Stillstand (K-73) | <= 30 s | **2,646 s** | | 4,308 s | ja |
| PERF-005 (B4): 10 harte Abbrüche, danach `integrity_check = ok` | je Abbruch ok | 10 von 10 ok | | 10 von 10 | ja |
| PERF-005: URL-Menge und Status je URL = Referenz, keine Dublette | identisch, 0 Dubletten | identisch (100 001 URLs, 0 fehlend, 0 zusätzlich, 0 abweichender Status), 0 Dubletten | | gleich | ja |
| PERF-005: wiederholte Requests je Abbruch (K-49) | <= 2 264 (8 + 256 + 2 000) | `[2264, 1640, 2264, 2264, 2264, 2264, 2264, 2264, 1663, 2066]`, Maximum **2 264** | | `[2264 x 9, 2263]` | ja (an der Grenze) |
| PERF-005: Vorkommen von „abgeschnitten“ in Bericht und Log | 0 | 0 | | 0 | ja |

Nicht neu gemessen (nicht Teil von M1-N26, Stand M1-N21): CRAWL-124 (Profilvergleich, `n21-report-crawl-124-*-1m.json`: MinMemory -53,7 % RSS gegen Balanced) und die 18 Benchmark-Ignores. Der Code von `os-store` und `os-engine` hat sich seit M1-N21 durch M1-N22 verändert; CRAWL-124 und die Benchmarks sind deshalb auf dem neuen Stand nicht belegt. Eine Wiederholung ist eine Frage an den Planer (kein Kriterium der Aufgabe von M1-N26).

## Nachbesserungsauftrag (keine Codeänderung in diesem Paket)

**1. K-80: Endbatch über 60 s.**
- Spec-ID / Kriterium: PERF-002 (K-72) und K-80 (Endbatch <= 60 s bei 1 Mio.; `docs/plan/M1.md` M1-N22). Crates: `os-store` (`crates/os-store/src/url_index.rs`, `link_index.rs`, `writer.rs`), Aufruf im Endbatch durch `os-engine` (`writer_thread.rs`).
- Messwert: größter Schreibaufruf (`crawler_diag.engine_w_write_max_ns`) **98,8 s (B1), 96,1 s (B2), 92,3 s (B3)**; darin `store_link_index_build_ns` 33,7 s / 35,2 s / 33,1 s (ein Aufbau) und `store_url_index_build_ns` 57,6 s / 53,8 s / 51,9 s (fünf Aufbauten, größter 12,7 s / 11,0 s / 10,8 s; davon Vorwärmen der Leser `store_url_index_prefetch_ns` 14,8 s / 10,1 s / 10,0 s), Commit des letzten Eimers (`store_commit_max_ns_h05`) 7,4 s / 6,8 s / 7,0 s. M1-N22 gibt 29 bis 32 s für den Endbatch an (kalter Dateicache bis 82 s); die Messungen lagen bei 92 bis 99 s. Die Maschine trug beim Start jedes Blocks die Grundlast von 1,8 bis 2,0 (siehe unten); ob der Dateicache kalt war, ist nicht gemessen.
- Befehl zum Nachmessen (rund 7 Minuten, Release, ruhige Maschine): `cargo build --release --locked -p os-loadtest && cargo build --release --locked -p os-engine --example crawl_child && target/release/os-loadtest run --scenario perf-001 --size large --work-dir <ordner außerhalb des Repos> --out <datei>.json`; gelesen wird `metrics.crawler_diag.engine_w_write_max_ns`.

**2. PERF-002, Abschnittsregel bei verfallsfreiem Durchsatz.**
- Spec-ID: PERF-002 (`docs/spec/07-performance.md`; K-72, K-80). Befund: Der Verfall ist weg, aber die Regel „jeder Abschnitt 70 % bis 130 % des Mittelwerts“ schlägt jetzt von der anderen Seite an. Der Mittelwert ist abgerufene URLs ÷ **gesamte** Crawl-Dauer (`crates/os-loadtest/src/engine.rs`, `mean_urls_per_s`); die zehn Abschnitte (`crates/os-loadtest/src/throughput.rs`, `segment_rates`) enden beim Erreichen der letzten abgerufenen URL. Der Endbatch nach der letzten abgerufenen URL (rund 100 s) drückt den Mittelwert unter jeden Abschnitt.
- Messwerte (URLs/s je Abschnitt 1 bis 10): B1 **2 762, 3 845, 4 040, 4 030, 4 073, 3 615, 4 043, 3 680, 3 985, 4 137** (Mittel 2 736,6; 101, 141, 148, 147, 149, 132, 148, 134, 146, 151 %); B2 **2 652, 3 814, 4 143, 3 946, 3 816, 3 790, 3 871, 3 709, 3 996, 3 946** (Mittel 2 725,3; 97, 140, 152, 145, 140, 139, 142, 136, 147, 145 %). Zeit bis zur letzten abgerufenen URL 265,2 s (B1) und 269,2 s (B2), danach Endbatch 100,2 s und 97,7 s von 365,4 s und 366,9 s Crawl-Dauer.
- Rechnung (aus den Messwerten, nicht gemessen): Bezogen auf den Durchsatz bis zur letzten abgerufenen URL (3 771 URLs/s in B1, 3 715 in B2) liegen alle zehn Abschnitte zwischen 71 % und 112 %. Mit der Regel unverändert müsste der Endbatch auf höchstens rund 54 s (B1) bzw. 45 s (B2) sinken, damit der größte Abschnitt (4 073 bzw. 4 143 URLs/s) 130 % des Mittelwerts nicht übersteigt; die Grenze von 60 s aus K-80 genügt dafür nicht.
- Frage an den Planer: entweder Endbatch auf rund 45 s drücken (Nachbesserung 1 verschärft) oder die Messregel präzisieren (Abschnittsgrenzen gegen den Durchsatz bis zur letzten abgerufenen URL, Mittelwert >= 500 weiter gegen die gesamte Dauer). Dieses Paket ändert die Regel nicht.
- Hinweis: Abschnitt 1 ist mit 2 652 bis 2 762 URLs/s der langsamste; der erste Eimer hat auch die längste Commit-Summe (`store_commit_ns_h00` 19,6 s und 20,7 s gegen 14,4 bis 17,3 s in den Eimern 1 bis 4). Eine Ursache ist nicht gemessen. Bezogen auf den Durchsatz bis zur letzten abgerufenen URL liegt Abschnitt 1 bei 73 % (B1) und 71 % (B2), knapp über der unteren Grenze von 70 %.

## Maschine und Stand

| | |
|---|---|
| Rechner | Apple M2, 8 Kerne, 16 GB RAM (`sysctl hw.memsize` = 17 179 869 184 B) |
| macOS | 26.6.2 |
| Commit | Branch `m1/m1-n26` auf `origin/main` 5f65ffc (Welle 7c gemergt: f1867ad N22, 7e81cd8 N23, 8ef30d1 N24, ac41d7c N25); an Produktcode und Tests geändert: nichts (`git diff origin/main -- crates tests` ist leer) |
| Build | Release, `cargo build --release --locked -p os-loadtest` und `-p os-engine --example crawl_child`, `CARGO_INCREMENTAL=0`, `RUSTC_WRAPPER=sccache`; Target-Ordner `openspider-wt/target-m1-n26` |
| Konstanten | `BATCH_PAGES_BACKLOG` 2 000, `CHANNEL_CAPACITY` 256, 8 Threads, Profil Balanced |
| Wachhalten | `caffeinate -dimsu` um jeden Lauf (`scripts/bench/run_acceptance.sh`, Wartegrenze 120 min) |
| Arbeitsordner | `openspider-wt/n26-work/b1` bis `b4` außerhalb des Repos (der Lasttest löscht die Crawl-Dateien am Ende jedes Laufs; Spitze durch die 6,3-GB-Crawl-Datei); freier Platz vor jedem Block 20 GB (`wait_quiet.sh`; Vorgabe >= 20 GB) |
| Parallele Läufe | keine; vor dem Start waren die vier gemergten Worktrees (`m1-n22` bis `m1-n25`, sauber, alle Branches Vorfahren von `origin/main`) entfernt, `git worktree list` zeigte nur `main` und `m1-n26`; während der Blöcke wurde nichts gebaut oder getestet |
| Fremdlast | Der Nutzerprozess `bfs` (PID 71900, Dateisuche über `/`, rund 480 % CPU) lief vor dem Start und war bei der nächsten Prüfung um 10:20 beendet (nicht von M1-N26 beendet); danach blieb eine Systemlast von 3 bis 5 (`fileproviderd`, `WindowServer`, `mds`, `mediaanalysisd`, `spindump`) bis rund 11:40 |

### Last je Block

Jeder Block wurde erst gestartet, nachdem `scripts/bench/wait_quiet.sh 2 120` den 1-Minuten-Load unter 2 gesehen hatte. B1 wartete 80 Minuten (10:22 bis 11:42), danach lag die Wartezeit bei 8 Minuten (B2), 20 Minuten (B3) und 20 Minuten (B4). Load < 1 war wie in N14 und N21 nicht erreichbar; die Grundlast liegt bei 1,5 bis 2,5. Die Proben im Lauf (`log-*.txt.load`, jede Minute: Load 1/5/15 min) zeigen zusätzlich die Last des Lasttests selbst (Testserver, Crawler, Writer: rund 3 Kerne).

| Block | Lauf | Load (1 min) beim Start | Load-Proben im Lauf (1 min, Minimum bis Maximum) |
|---|---|---:|---|
| B1 | PERF-001 large, Lauf 1 | 1,91 | 1,91 bis 5,64 (7 Proben) |
| B2 | PERF-001 large, Lauf 2 | 1,97 | 1,97 bis 5,07 (7 Proben) |
| B3 | PERF-003 large | 1,85 | 1,85 bis 5,03 (8 Proben) |
| B4 | PERF-005 large, `--kills 10` | 1,82 | 1,82 und 3,57 (Lauf 71 s) |

Ein früherer Versuch von B3 und B4 (Start 12:03) wurde nach 20 Minuten Wartezeit abgebrochen, bevor der Block begann (Umbenennung der N21-Dateien in `n21-*`, damit die neuen Läufe sie nicht überschreiben); er hat keine Messung erzeugt. Alle vier Blöcke wurden gewertet; die Durchsätze sind mit dieser Grundlast zu lesen, RSS-Werte hängen davon nicht ab. Die Streuung der Mittel zwischen B1 und B2 liegt bei 0,4 % (2 736,6 gegen 2 725,3).

## Läufe, Befehle, Laufzeiten

Alle Befehle aus dem Worktree `m1-n26`; `<ordner>` = Arbeitsordner außerhalb des Repos, `os-loadtest` = `target-m1-n26/release/os-loadtest`. Aufruf jeweils über `scripts/bench/run_acceptance.sh <logdatei> <befehl>` (wartet auf Load < 2, schreibt Start/Ende, Load und die Lastproben).

| Block | Befehl (`os-loadtest run …`) | Berichte und Logs | Laufzeit (Start–Ende, lokal, 16.10.2026) |
|---|---|---|---|
| B1 PERF-001 Lauf 1 | `--scenario perf-001 --size large --work-dir <ordner> --out docs/plan/M1-acceptance/loadtest/report-perf-001-large-run1.json` | `report-perf-001-large-run1.json`, `log-perf-001-large-run1.txt` | 11:42–11:48 (Crawl 365,4 s), Exit 1 |
| B2 PERF-001 Lauf 2 | wie B1, `…-run2.json` | `report-perf-001-large-run2.json`, `log-perf-001-large-run2.txt` | 11:56–12:03 (Crawl 366,9 s), Exit 1 |
| B3 PERF-003 | `--scenario perf-003 --size large …` | `report-perf-003-large.json`, `log-perf-003-large.txt` | 12:43–12:50 (Crawl 414,2 s), Exit 0 |
| B4 PERF-005 | `--scenario perf-005 --size large --kills 10 …` | `report-perf-005-large.json`, `log-perf-005-large.txt` | 13:10–13:11 (Crawl 39,4 s), Exit 0 |

Der Exit-Code 1 von B1 und B2 ist das Urteil des Lasttests (`passed = false` wegen `crawler_violations`), kein Absturz. Die lokale Gegenprobe (Workspace-Tests, Parität, CLI-E2E, MCP-E2E) steht in `../checklist.md`, Abschnitt „Lokale Gegenprobe“.

## Einzelergebnisse

### B1 und B2: PERF-001 large (1 000 000 URLs x 30 Links), zwei Läufe

| Messwert | Lauf 1 | Lauf 2 |
|---|---|---|
| Spitzen-RSS (`crawler_peak_rss_bytes`) | 468 549 632 B = 468,5 MB (Grenze 1 500 000 000) | 461 160 448 B = 461,2 MB |
| Referenz 100 000 Seiten | 420 331 520 B = 420,3 MB; Verhältnis 1,115 (Regel `ratio`, Grenze 2,0) | 420 380 672 B = 420,4 MB; Verhältnis 1,097 |
| URLs abgerufen | 1 000 000 in 365,4 s = 2 736,6 URLs/s (N21: 476,5 s, 2 098,8) | 1 000 000 in 366,9 s = 2 725,3 URLs/s |
| Abschnitte (URLs/s, 1 bis 10) | 2 762, 3 845, 4 040, 4 030, 4 073, 3 615, 4 043, 3 680, 3 985, 4 137 | 2 652, 3 814, 4 143, 3 946, 3 816, 3 790, 3 871, 3 709, 3 996, 3 946 |
| Abschnitte in % des Mittelwerts | 101, 141, 148, 147, 149, 132, 148, 134, 146, 151: **Verstoß** (Abschnitte 2 bis 10) | 97, 140, 152, 145, 140, 139, 142, 136, 147, 145: **Verstoß** (Abschnitte 2 bis 10) |
| Abschnitte in % des Durchsatzes bis zur letzten abgerufenen URL (3 771 / 3 715 URLs/s; Rechnung, nicht Kriterium) | 73, 102, 107, 107, 108, 96, 107, 98, 106, 110 | 71, 103, 112, 106, 103, 102, 104, 100, 108, 106 |
| Längster Stillstand (`crawler_max_stall_s`) | 1,985 s (Grenze 30 s) | 2,348 s |
| Dateigröße | 6 276 964 352 B (Referenz 100 000 Seiten: 603 410 432 B) | 6 277 152 768 B (Referenz: 603 389 952 B) |
| Profil / Cache | Balanced, 268 435 456 B (256 MiB); MaxSpeed-Cache -4 194 304 KiB = 4 GiB (K-16: 25 % von 16 GiB, höchstens 4 GiB) | gleich |
| Testserver (100 000 Probe-Requests) | 130 579 Requests/s, p50 0,233 ms, p99 0,481 ms, 0 Fehler, deterministisch | 131 873 Requests/s, p50 0,234 ms, p99 0,452 ms, 0 Fehler, deterministisch |
| Verstöße / Warnungen | 9 Verstöße gegen die PERF-002-Abschnittsgrenzen; keine Warnungen | 9 Verstöße; keine Warnungen |

**Commit-Verlauf je 100 Batches (`diag`, M1-N22; Summe in Sekunden, Eimer 0 bis 5; der letzte Eimer enthält die Restbatches einschließlich Endbatch):**

| | Eimer 0 | 1 | 2 | 3 | 4 | 5 (Rest, 23 / 25 Batches) |
|---|---:|---:|---:|---:|---:|---:|
| Lauf 1: Commit (`store_commit_ns_h0n`) | 19,6 | 14,5 | 15,7 | 15,3 | 14,8 | 10,6 |
| Lauf 1: Aufbau (`store_build_ns_h0n`) | 27,5 | 21,6 | 21,7 | 22,0 | 22,1 | 5,1 |
| Lauf 1: größter Commit (`store_commit_max_ns_h0n`) | 1,68 | 0,96 | 1,54 | 1,46 | 1,06 | 7,38 |
| Lauf 2: Commit | 20,7 | 14,4 | 14,7 | 17,3 | 14,9 | 10,5 |
| Lauf 2: Aufbau | 27,2 | 22,1 | 22,4 | 22,1 | 22,1 | 5,5 |
| Lauf 2: größter Commit | 2,07 | 0,98 | 1,21 | 1,50 | 1,03 | 6,81 |

Der Commit je 100 Batches ist flach: der letzte volle Eimer (Eimer 4) liegt bei 14,8 s und 14,9 s, 0,72- bis 0,76-fach des ersten Eimers (19,6 s, 20,7 s) und 0,86- bis 1,03-fach der Eimer 1 bis 3; Commit-Summe 90,5 s und 92,4 s (N21: 248,5 s), Aufbau 120,0 s und 121,5 s (N21: 131,9 s), Schreibzeit 303,4 s und 304,6 s (N21: 414,9 s). 501 von 523 Commits gelten als „slow“ (N21: 498 von 518). Die `urls`-Einfügungen im Aufbau (`store_g_urls_ins_ns`) kosten nur noch im Eimer 0 (6,7 s) und danach höchstens 55 µs je Eimer: die Sekundärindizes sind ab der Schwelle entfernt und werden im Endbatch gebaut (K-80 a).

**Endbatch (Dauer des größten Schreibaufrufs, `engine_w_write_max_ns`):** 98,8 s (Lauf 1) und 96,1 s (Lauf 2); darin Aufbau von `links_target` 33,7 s und 35,2 s, fünf Aufbauten der `urls`-Indizes 57,6 s und 53,8 s (größter 12,7 s und 11,0 s, Vorwärmen 14,8 s und 10,1 s), Commit des letzten Eimers (größter) 7,4 s und 6,8 s. WAL-Rückführung: 2 Checkpoints (1,40 s und 1,64 s zusammen), kein Hintergrund-Checkpoint; größte WAL-Größe 1,18 GB im Endbatch.

**Diagnose Speicher und Warteschlange der Engine (Ende):** Seen-Menge 24,6 MB (1 000 001 Einträge, 0 Überläufe, 0 Kollisionen), Warteschlange im RAM 4,9 MB und 5,0 MB (23 067 und 27 447 Einträge; 881 554 und 881 507 auf Platte), 99 Nachladevorgänge (988 478 und 988 480 Zeilen; Wartezeit auf `writer.flush()` 41,4 s und 43,2 s, N21: 99 Vorgänge), 1 Trim, Sortierstruktur 0,8 MB, Kern 29,2 MB und 28,8 MB, größter Writer-Batch 31,2 MB und 32,5 MB, 0 Fetch-Warnungen (`engine_fetch_slow` 0).

### B3: PERF-003 large (SlowSink 100 ms je Schreibaufruf, K-52; 1 Mio. URLs, K-73)

| Messwert | Wert |
|---|---|
| Spitzen-RSS | 461 946 880 B = 461,9 MB (Grenze 500 000 000), bestanden |
| Crawl | endet regulär (`end_reason = queue_empty`), 1 000 000 URLs, 414,2 s = 2 414,1 URLs/s (N21: 475,9 s, 2 101,1) |
| Längster Stillstand | 2,646 s (Grenze 30 s) |
| `diag`: Zahl der Batches | 525 (`engine_w_batches`, `store_batches`; 501 mit mehr als 1 000 Seiten, 1 mit bis zu 200, 23 mit bis zu 50) |
| `diag`: `engine_w_pages_max` | 2 000 (= `BATCH_PAGES_BACKLOG`) |
| `diag`: `engine_w_queue_max` | 256 (= `CHANNEL_CAPACITY`; der Kanal war voll) |
| `diag`: `loadtest_slow_wait_ns` | 54 433 416 186 ns = 54,4 s bei 525 Schreibaufrufen (`loadtest_slow_calls`) |
| `diag`: Hintergrund-Checkpoints | 0 (Zahl), Maximum 0 ns, Wartezeit des Writers auf einen Checkpoint 0 ns. Checkpoints im Commit: 2, zusammen 1,20 s |
| `diag`: Schreibzeit | gesamt 354,4 s, größte 92,3 s (Endbatch mit Indexaufbau), Commit gesamt 98,2 s (größter 7,0 s; 501 von 525 „slow“) |
| `diag`: Commit je 100 Batches (Eimer 0 bis 5) | 23,3 / 16,6 / 15,5 / 15,5 / 16,0 / 11,3 s; Aufbau 26,8 / 21,0 / 20,1 / 21,1 / 21,1 / 4,9 s |
| `diag`: Speicherzähler der Engine | Seen-Menge 24,6 MB (1 000 001 Einträge), Warteschlange im RAM 4,8 MB (23 007 Einträge, 881 240 auf Platte, 99 Nachladevorgänge, 1 Trim), Sortierstruktur 0,8 MB, Weiterleitungen 0,5 MB (19 975 Einträge), Kern 29,0 MB, größter Writer-Batch 30,4 MB |
| Abschnitte (nur Bericht, PERF-002 gilt in PERF-001) | 2 344, 2 899, 3 090, 3 287, 3 377, 3 322, 3 362, 3 280, 3 340, 3 241 URLs/s = 97, 120, 128, 136, 140, 138, 139, 136, 138, 134 % des Mittelwerts 2 414,1 (75 bis 108 % des Durchsatzes bis zur letzten URL, 3 118 URLs/s) |

Das Drosseln selbst belegt der Test `full_channel_blocks_the_sender_until_the_writer_catches_up` (`writer_thread.rs`, Tag PERF-003); hier nur zur Einordnung (K-52). Der RSS liegt mit 461,9 MB 38,1 MB unter der Grenze (N21: 468,1 MB).

### B4: PERF-005 large (100 000 Seiten, `--kills 10`, Seed 20261014)

- Kill-Punkte (abgerufene Seiten): 14 177, 15 814, 27 500, 34 721, 38 982, 46 925, 61 498, 64 126, 71 705, 84 155 (wie N21).
- Je Abbruch: `integrity_check = ok` (10 von 10), `duplicate_urls` = 0 (10 von 10).
- Nach dem Fortsetzen: 100 001 URLs, 100 000 abgerufen, `urls_missing` 0, `urls_extra` 0, `urls_with_other_status` 0, Status identisch mit dem Referenzcrawl (`crawler_status_identical` = true; 200: 96 582, 301: 1 973, 404: 973, 500: 236, 503: 237).
- Wiederholte Requests je Abbruch (`crawler_repeated_per_kill`): **[2264, 1640, 2264, 2264, 2264, 2264, 2264, 2264, 1663, 2066]**, Maximum 2 264, Mittel 2 121,7, Summe 21 217 (Gesamtgrenze 22 640). Grenze je Abbruch (`crawler_repeated_bound_per_kill`, K-49) = 8 Threads + 256 Kanal + 2 000 Batch = **2 264**: nie überschritten, in 7 von 10 Abbrüchen genau erreicht (N21: 9 von 10). Vor dem ersten Abbruch keine wiederholten Requests (`crawler_repeated_before_first_kill` 0); robots.txt 1 Abruf; Requests insgesamt 121 218.
- Je Abbruch in der Pipeline verloren (`lost_in_pipeline`: abgerufen, aber nicht in der Datei): 1 991, 1 426, 2 255, 2 255, 2 255, 2 255, 2 255, 2 255, 1 097, 1 570.
- „abgeschnitten“ kommt in Bericht und Log 0-mal vor.
- Laufzeit des Crawls 39,4 s (N21: 35,7 s). Testserver 134 989 Requests/s, p99 0,475 ms, 0 Fehler.

## Vergleich mit N21 (Stand nach Welle 7)

| | N21 (88fd424) | N26 (5f65ffc, Welle 7c) | Veränderung |
|---|---|---|---|
| PERF-001 Mittel (URLs/s) | 2 098,8 | 2 736,6 / 2 725,3 | +30 % |
| PERF-001 Crawl-Dauer | 476,5 s | 365,4 s / 366,9 s | -23 % |
| PERF-001 Abschnitte relativ zum Mittelwert | 68 % bis 186 %, stetig fallend (3 118 → 1 421 URLs/s) | 97 % bis 152 %, flach ab Abschnitt 2 (3 709 bis 4 143 URLs/s) | Verfall behoben, Regel dennoch verletzt (Endbatch im Mittel) |
| Commit-Summe | 248,5 s | 90,5 s / 92,4 s | -63 % bis -64 % |
| Endbatch | 39,5 s (Schreibzeit des Endbatchs) | 98,8 s / 96,1 s | +59 s; Indexaufbau wanderte in den Endbatch (K-80 a) |
| PERF-001 Spitzen-RSS | 471,1 MB | 468,5 MB / 461,2 MB | -0,6 % / -2,1 % |
| PERF-001 längster Stillstand | 6,085 s | 1,985 s / 2,348 s | besser |
| PERF-003 Spitzen-RSS | 468,1 MB | 461,9 MB | -1,3 % |
| PERF-003 Stillstand | 4,308 s | 2,646 s | besser |
| PERF-005 Maximum wiederholte Requests je Abbruch | 2 264 | 2 264 | gleich (Grenze scharf) |

## Dateien in diesem Ordner

- `report-perf-001-large-run1.json`, `report-perf-001-large-run2.json`, `report-perf-003-large.json`, `report-perf-005-large.json`: Berichte von `os-loadtest` (Schema `{scenario, size, started_at, metrics, passed}`) dieser Messung (M1-N26).
- `log-*.txt`: Kopf (Start, `wait_quiet`-Ergebnis, Load, Befehl), Konsolenausgabe und Ende je Block; `log-*.txt.load`: Lastproben jede Minute (Zeit, Load 1/5/15 min).
- `n21-*`: Berichte und Logs der Messung M1-N21 (Stand nach Welle 7, Commit 88fd424), unverändert; enthalten auch den Profilvergleich CRAWL-124 bei 1 Mio. Seiten und den RSS-Trend von PERF-003.
- `n14-*`: Berichte und Logs der Messung N14 (vor Welle 7, Commit `43caf19`), nur zum Vergleich. Dort fehlt PERF-002.
- `probe-n22/`, `probe-p32/`, `probe-p33/`: Kurzproben der Pakete M1-N22, M1-P32 und M1-P33 (Entwicklung).
