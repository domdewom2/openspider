# Großläufe (M1-N21, A4, nach Welle 7)

Abnahmemessung der Kriterien PERF-001, PERF-002 (K-72), PERF-003 (K-73), PERF-005 und CRAWL-124 (Profilvergleich) mit 1 Mio. URLs auf der lokalen Referenzmaschine, Stand nach Welle 7 (P32, P33, N19, N20 gemergt). Fortsetzen/Pausieren (CRAWL-126), CRAWL-127, OUT-001 und alle Benchmark-Ignores: siehe [`../benchmarks/README.md`](../benchmarks/README.md). Die Berichte der Messung N14 (vor Welle 7) liegen unverändert als `n14-*` in diesem Ordner; `probe-p32/` und `probe-p33/` sind die Kurzproben der Entwicklung (Welle 7, nicht ruhige Maschine).

## Ergebnis in Kürze

| Kriterium (M1.md Abschnitt 6, A4) | Grenze | Messwert | N14 (vorher) | bestanden |
|---|---|---|---|---|
| PERF-001: Spitzen-RSS bei 1 Mio. URLs x 30 Links | < 1,5 GB | **471,1 MB** | 720,8 MB | ja |
| PERF-001: RSS(1 Mio.) / RSS(100 k) | < 2 | **1,134** (471,1 MB / 415,4 MB) | 1,69 | ja |
| PERF-001: Cache-Grenze K-16 über die PRAGMA-Werte | Balanced 256 MiB; MaxSpeed <= min(25 % RAM, 4 GiB) | Balanced 268 435 456 B; MaxSpeed -4 194 304 KiB (= 4 GiB = 25 % von 16 GiB) | gleich | ja |
| PERF-001: längster Stillstand (K-73) | <= 30 s | **6,1 s** | 31,95 s | ja |
| PERF-002 (K-72): Mittelwert abgerufene URLs ÷ Crawl-Dauer | >= 500 URLs/s | **2 098,8 URLs/s** (476,5 s) | 133,1 URLs/s | ja |
| **PERF-002 (K-72): jeder der zehn 10-%-Abschnitte zwischen 70 % und 130 % des Mittelwerts** | 70 % bis 130 % (1 469 bis 2 728 URLs/s) | Abschnitte 1 bis 5 über 130 % (149, 186, 173, 164, 139 %), Abschnitt 10 unter 70 % (68 %); Spanne 68 % bis 186 % | nicht gemessen (grob 290 → 75 URLs/s) | **nein** |
| PERF-003: Spitzen-RSS bei 1 Mio. URLs, SlowSink 100 ms | < 500 MB | **468,1 MB** | 707,6 MB | ja |
| PERF-003: Crawl endet regulär, 1 Mio. URLs | `end_reason = queue_empty` | `queue_empty`, 1 000 000 URLs | `queue_empty` | ja |
| PERF-003: längster Stillstand (K-73) | <= 30 s | **4,3 s** | 31,3 s | ja |
| PERF-005: 10 harte Abbrüche, danach `integrity_check = ok` | je Abbruch ok | 10 von 10 ok | 10 von 10 | ja |
| PERF-005: URL-Menge und Status je URL = Referenz, keine Dublette in `urls` | identisch, 0 Dubletten | identisch (100 001 URLs, 0 fehlend, 0 zusätzlich, 0 abweichender Status), 0 Dubletten | gleich | ja |
| PERF-005: wiederholte Requests je Abbruch (K-49) | <= 2 264 (8 + 256 + 2 000) | Liste `[2264 x 9, 2263]`, Maximum **2 264** | Mittel 2 171,1 | ja (genau an der Grenze) |
| PERF-005: Vorkommen von „abgeschnitten“ in Bericht und Log | 0 | 0 | 0 | ja |
| CRAWL-124: MinMemory-RSS mindestens 20 % unter Balanced (1 Mio. Seiten, alle drei Profile) | <= 80 % von Balanced | **-53,7 %** (217,4 MB gegen 469,9 MB) | -53,8 % bei 250 000 Seiten | ja |

**A4 ist nicht erfüllt.** Verletzt ist allein das zweite Teilkriterium von PERF-002 (Abschnittsgrenzen 70 % bis 130 %, K-72): der Durchsatz fällt im 1-Mio.-Crawl von rund 3 900 auf rund 1 400 URLs/s. Der Mittelwert (>= 500 URLs/s), alle RSS-Grenzen, beide Stillstandsgrenzen, PERF-005, CRAWL-124, CRAWL-126/-127, OUT-001 und alle 18 Benchmark-Ignores sind erfüllt. Der Verstoß ist reproduziert (zweiter unabhängiger Lauf mit Balanced: Abschnitte 1 bis 4 bei 138 bis 172 %, siehe B4). Nachbesserungsauftrag unten.

## Nachbesserungsauftrag (keine Codeänderung in diesem Paket)

**1. PERF-002 (K-72), Abschnittsgrenzen 70 % bis 130 % des Mittelwerts im 1-Mio.-Crawl verletzt.**
- Spec-ID: PERF-002 (`docs/spec/07-performance.md`; K-72, A4 in `docs/plan/M1.md` Abschnitt 6). Crates: `os-engine` (Writer-Stufe, `crates/os-engine/src/writer_thread.rs`: `BATCH_PAGES_BACKLOG` 2 000, `CHANNEL_CAPACITY` 256; Warteschlange/Nachladen `engine_q_*`) und `os-store` (`crates/os-store/src/writer.rs`, Commit; `profile.rs`). Die Messung selbst ist in `os-loadtest` (`crates/os-loadtest/src/engine.rs`) korrekt und braucht keine Änderung.
- Messwert (`report-perf-001-large.json`, `crawler_segment_urls_per_s`, Mittel 2 098,8 URLs/s): **3 118, 3 904, 3 635, 3 440, 2 926, 2 429, 2 106, 1 577, 1 515, 1 421** URLs/s = 149, 186, 173, 164, 139, 116, 100, 75, 72, 68 % des Mittelwerts (erlaubt 70 % bis 130 %). `crawler_violations` nennt die Abschnitte 1 bis 5 und 10. Der Verlauf ist über den ganzen Crawl stetig fallend (Faktor 2,7 zwischen Abschnitt 2 und 10), kein Einzelereignis: längster Stillstand 6,1 s.
- Wiederholung (Block B4, Balanced, unabhängiger Lauf, `report-crawl-124-balanced-1m.json`): Mittel 2 244,3; Abschnitte 3 104, 3 863, 3 591, 3 534, 2 859, 2 449, 2 177, 1 934, 1 770, 1 603 URLs/s = 138, 172, 160, 157, 127, 109, 97, 86, 79, 71 % (Abschnitte 1 bis 4 über 130 %).
- Befunde zur Eingrenzung (alle gemessen, die Ursache selbst nicht): (a) Bis 500 000 Seiten gelten die Grenzen: `perf-003 --pages 500000` 96 % bis 126 %, 250 000 Seiten 85 % bis 114 %; der Verfall tritt erst in der zweiten Hälfte des 1-Mio.-Crawls auf. (b) Der Writer-Schritt ist der Engpass: `engine_w_write_ns` 414,9 s von 476,5 s (87 %), davon Commit `store_commit_ns` 248,5 s (498 von 518 Commits „slow“), Aufbau `store_build_ns` 131,9 s, Endbatch mit Indexaufbau `links_target` 32,9 s (`store_link_index_build_ns`); WAL-Checkpoints spielen keine Rolle (`store_checkpoint_ns` 42,6 ms, 2 Stück). (c) Die Cache-Größe ist nicht der Hebel: MinMemory (64 MiB) ist mit 2 324 URLs/s nicht langsamer als Balanced (256 MiB, 2 244) und zeigt denselben Verlauf (Abschnitte 125 % bis 75 %, siehe B4), MaxSpeed (4 GiB) ist mit 2 035 URLs/s langsamer und fällt noch stärker (60 % im Abschnitt 10). (d) Das reine Store-Benchmark `writer_1m_uniform_segments` (30 gleichverteilte Linkziele, 1 Mio. Seiten, Balanced) bleibt dagegen flach: Abschnitte 98 % bis 103 %, 7 044 Seiten/s, größter Commit 0,58 s (`../benchmarks/os-store-bench_writer.txt`); seine Datei hat 3 341 MiB, die Crawl-Datei im Lasttest 6 293 MB. Der Unterschied liegt also in dem, was der echte Crawl zusätzlich schreibt oder liest (`urls`-Zeilen mit allen Spalten, Warteschlangen-Nachladen mit 99 Refills, Seen-Abgleich), nicht im Links-Pfad.
- Vermutung (nicht gemessen, für die Nachbesserung zu prüfen): Zeit je Commit wächst mit der Größe von `urls` (Indizes auf `url`/Status bei zufälligen Schlüsseln; die Datei ist doppelt so groß wie im Store-Benchmark) oder das Nachladen der Warteschlange aus `urls` (`engine_q_refills` 99). Die Zeit je Abschnitt aus `store_commit_ns` und `store_build_ns` ist im `diag`-Block nur als Summe vorhanden; ein Verlauf (z. B. Commit-Dauer je 100 Batches) würde die Frage entscheiden.
- Befehl zum Nachmessen (rund 8 Minuten, Release): `cargo build --release --locked -p os-loadtest -p os-engine --example crawl_child && target/release/os-loadtest run --scenario perf-001 --size large --work-dir <ordner außerhalb des Repos> --out <datei>.json`. Erfüllt, wenn `crawler_violations` leer ist, also alle zehn Werte in `crawler_segment_urls_per_s` zwischen 70 % und 130 % von `crawler_urls_per_s` liegen und der Mittelwert >= 500 bleibt. Zwischenmessung des Fortschritts über die Dateigröße/gespeicherte Seiten: `scripts/bench/segments.sh`.
- Hinweis für den Planer: Der Mittelwert liegt weit über der Grenze (2 099 gegen 500 URLs/s), und auch der langsamste Abschnitt (1 421 URLs/s) liegt weit darüber. Verletzt ist die relative Regel (70 % bis 130 % des Mittelwerts je Abschnitt, K-72). Ein schneller Anfang allein verletzt die Obergrenze, solange das Ende langsamer ist; die Nachbesserung muss deshalb den Verfall bremsen, nicht den Anfang. Ob die relative Regel bei einem so hohen Mittelwert gewollt ist, ist eine Frage an den Planer; dieses Paket ändert die Regel nicht.

## Beobachtungen (kein Kriterium)

- **Profile (CRAWL-124):** MinMemory ist bei 1 Mio. Seiten nicht langsamer als Balanced (+3,6 % URLs/s), MaxSpeed ist langsamer (-9,3 %) und belegt mit 5 437 MB RSS das 25-Fache von MinMemory (ein 4-GiB-Cache auf einer 16-GB-Maschine; K-16 erlaubt das). Ein Vorteil von MaxSpeed ist in diesem Szenario nicht messbar; bei 250 000 Seiten war er in N14 deutlich (2 451 gegen 1 262 URLs/s). Der Lasttest bewertet MaxSpeed seit N19 nicht mehr mit den PERF-001-RSS-Grenzen (`crawler_rss_limits`: „nicht bewertet (Profil max-speed, K-16)“); die PERF-002-Abschnittsprüfung wird dagegen für alle Profile ausgewertet und färbt `passed` rot (siehe B4).
- **PERF-002 in PERF-003:** Der Lasttest wertet die Abschnitte nur in `perf-001` als Verstoß (K-72 misst am Crawl von PERF-001). In `perf-003 large` stehen sie nur im Bericht: Mittel 2 101,1; 2 683, 3 389, 3 270, 3 078, 2 816, 2 387, 2 111, 1 981, 1 660, 1 476 URLs/s = 128, 161, 156, 146, 134, 114, 100, 94, 79, 70 %. `SlowSink` bremst nicht messbar (Mittel 2 101 gegen 2 099 in PERF-001): bei Gruppen-Commit trifft die Verzögerung nur rund 520 Schreibaufrufe (54,7 s Wartezeit insgesamt, die zum Teil hinter dem Rückstau des Writers verschwindet).
- **PERF-005 und die Puffergrenze (K-49):** In 9 von 10 Abbrüchen wird die Grenze 2 264 genau erreicht, einmal 2 263. Die Grenze ist scharf; jede künftige Vergrößerung von `CHANNEL_CAPACITY`, `BATCH_PAGES_BACKLOG` oder der Thread-Zahl muss `repeated_bound_per_kill` mitführen (der Lasttest leitet sie aus den Konstanten ab).

## Maschine und Stand

| | |
|---|---|
| Rechner | Apple M2, 8 Kerne, 16 GB RAM (`sysctl hw.memsize` = 17 179 869 184 B) |
| macOS | 26.6.2 |
| Commit | `88fd424` (origin/main nach Merge M1-N20, Welle 7 komplett: P32, P33, N19, N20), Branch `m1/m1-n21`; an Produktcode und Tests geändert: nichts |
| Build | Release, `cargo build --release --locked -p os-loadtest -p os-engine --example crawl_child` (`os-loadtest` ein zweites Mal einzeln), `CARGO_INCREMENTAL=0`, `RUSTC_WRAPPER=sccache`; Target-Ordner `openspider-wt/target-m1-n21` |
| Konstanten | `BATCH_PAGES_BACKLOG` 2 000, `CHANNEL_CAPACITY` 256, 8 Threads, Profil Balanced (außer B4) |
| Wachhalten | `caffeinate -dimsu` um jeden Lauf (`scripts/bench/run_acceptance.sh`) |
| Arbeitsordner | `openspider-wt/n21-work/` außerhalb des Repos (der Lasttest löscht die Crawl-Dateien am Ende jedes Laufs; Spitze durch die 6,3-GB-Crawl-Datei); freier Platz vor jedem Block 77 bis 79 GB (Vorgabe >= 20 GB) |
| Parallele Läufe | keine; ein einziger Worktree (`git worktree list` zeigte vor dem Start nur `main`), nicht gebaut oder getestet, solange ein Messblock lief |

### Last je Block

Jeder Block wurde erst gestartet, nachdem `scripts/bench/wait_quiet.sh 2` den 1-Minuten-Load unter 2 gesehen hatte (Wartezeit bis zu 15 Minuten, danach klang die Last der macOS-Dienste `fileproviderd` und `syspolicyd` nach dem Bau ab). Load < 1 war wie in N14 nicht erreichbar; die Grundlast liegt bei 1,5 bis 2,5. Die Proben im Lauf (`log-*.txt.load`, jede Minute: Load 1/5/15 min) zeigen zusätzlich die Last des Lasttests selbst (Testserver, Crawler, Writer: rund 3 Kerne).

| Block | Lauf | Load (1 min) beim Start | Load-Proben im Lauf (1 min, Minimum bis Maximum) |
|---|---|---:|---|
| B1 | PERF-001 large | 1,96 | 1,96 bis 5,99 (9 Proben) |
| B2 | PERF-003 large | 1,64 | 1,64 bis 4,10 (8 Proben) |
| B2 ergänzend | PERF-003 `--pages 100000` / `250000` / `500000` | 1,81 / 1,81 / 1,74 | Lauf 1 bis 3 Minuten; 1,81 / 1,81 bis 3,15 / 1,74 bis 3,31 |
| B3 | PERF-005 large, `--kills 10` | 1,90 | 1,90 und 3,53 (Lauf 60 s) |
| B4 | CRAWL-124 Balanced 1 Mio. | 1,93 | 1,93 bis 3,90 (8 Proben) |
| B4 | CRAWL-124 MinMemory 1 Mio. | 1,80 | 1,80 bis 5,19 (8 Proben) |
| B4 | CRAWL-124 MaxSpeed 1 Mio. | 1,69 | 1,69 bis 4,34 (9 Proben) |
| B5/B6 | Fortsetzen/Pausieren, Benchmarks | 1,70 bis 1,99 | siehe `../benchmarks/README.md` |

Die Regel „Load < 1 anstreben“ wurde nicht erreicht, die Regel „Load < 2 vor jedem Block“ in allen Blöcken eingehalten. Durchsätze und Laufzeiten sind mit dieser Grundlast zu lesen; RSS-Werte hängen davon nicht ab. Der Durchsatz-Verlauf in PERF-002 ist in zwei unabhängigen Läufen (B1, B4 Balanced) derselbe, die Streuung der Mittel liegt bei 7 % (2 099 gegen 2 244).

## Läufe, Befehle, Laufzeiten

Alle Befehle aus dem Worktree; `<ordner>` = Arbeitsordner außerhalb des Repos, `os-loadtest` = `target-m1-n21/release/os-loadtest`. Aufruf jeweils über `scripts/bench/run_acceptance.sh <logdatei> <befehl>` (wartet auf Load < 2, schreibt Start/Ende, Load und die Lastproben).

| Block | Befehl (`os-loadtest run …`) | Berichte und Logs | Laufzeit (Start–Ende, lokal, 15.10.2026) |
|---|---|---|---|
| B1 PERF-001 | `--scenario perf-001 --size large --work-dir <ordner> --out docs/plan/M1-acceptance/loadtest/report-perf-001-large.json` | `report-perf-001-large.json`, `log-perf-001-large.txt` | 21:12–21:20 (8 min; Crawl 476,5 s) |
| B2 PERF-003 | `--scenario perf-003 --size large …` | `report-perf-003-large.json`, `log-perf-003-large.txt` | 21:21–21:29 (Crawl 475,9 s) |
| B2 ergänzend | `--scenario perf-003 --size large --pages 100000` / `250000` / `500000` | `report-perf-003-large-<n>-pages.json`, `log-perf-003-large-<n>-pages.txt` | 21:30–21:31, 21:31–21:33, 21:34–21:36 |
| B3 PERF-005 | `--scenario perf-005 --size large --kills 10 …` | `report-perf-005-large.json`, `log-perf-005-large.txt` | 21:38–21:39 (Crawl 35,7 s) |
| B4 CRAWL-124 | `--scenario perf-001 --size large --profile balanced` / `min-memory` / `max-speed` | `report-crawl-124-<profil>-1m.json`, `log-crawl-124-<profil>-1m.txt` | 21:41–21:48, 21:50–21:58, 21:59–22:08 (alle drei unter 90 min: kein Ersatz mit 250 000 Seiten nötig) |
| B5 Fortsetzen/Pausieren | `cargo test --release --locked -p os-engine --test lifecycle_pause -- --ignored --nocapture --test-threads=1` | `../benchmarks/os-engine-lifecycle_pause.txt` | 22:26, 6 s |
| B6 Benchmarks | siehe `../benchmarks/README.md` (`scripts/bench/run_ignored_benchmarks.sh`) | `../benchmarks/*.txt` | 22:27 bis 23:15 (inkl. Wartezeiten auf Load < 2) |

Nach den Messungen einmal `cargo test --workspace --exclude openspider-desktop --locked --no-fail-fast`: 2 037 bestanden, 0 fehlgeschlagen, 36 ignoriert (davon 18 Benchmark-Ignores); `cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings` und `cargo fmt --all --check` ohne Befund (Debug-Build, keine Codeänderung).

## Einzelergebnisse

### B1: PERF-001 large (1 000 000 URLs x 30 Links)

| Messwert | Wert |
|---|---|
| Spitzen-RSS (`crawler_peak_rss_bytes`) | 471 121 920 B = 471,1 MB (Grenze 1 500 000 000) |
| Referenz 100 000 Seiten | 415 399 936 B = 415,4 MB; Verhältnis 1,134 (Regel `ratio`, Grenze 2,0) |
| URLs abgerufen | 1 000 000 in 476,5 s = 2 098,8 URLs/s (N14: 7 515 s, 133,1 URLs/s) |
| Abschnitte (URLs/s, 1 bis 10) | 3 118, 3 904, 3 635, 3 440, 2 926, 2 429, 2 106, 1 577, 1 515, 1 421 = 149, 186, 173, 164, 139, 116, 100, 75, 72, 68 % des Mittelwerts: **Verstoß** (Abschnitte 1 bis 5 und 10) |
| Längster Stillstand (`crawler_max_stall_s`) | 6,085 s (Grenze 30 s) |
| Dateigröße | 6 293 188 608 B (Referenz 100 000 Seiten: 603 389 952 B) |
| Profil / Cache | Balanced, 268 435 456 B (256 MiB); MaxSpeed-Cache -4 194 304 KiB = 4 GiB (K-16: 25 % von 16 GiB, höchstens 4 GiB) |
| Testserver (100 000 Probe-Requests) | 112 464 Requests/s, p50 0,248 ms, p99 0,819 ms, 0 Fehler, deterministisch |
| `diag` Writer | 518 Batches (500 mit mehr als 1 000 Seiten), Seiten je Batch höchstens 2 000, Kanalfüllstand höchstens 256; Schreibzeit 414,9 s (größte 39,5 s = Endbatch mit Indexaufbau 32,9 s), Commit 248,5 s (größter 6,3 s), Aufbau 131,9 s, 2 Checkpoints (zusammen 42,6 ms), kein Hintergrund-Checkpoint |
| `diag` Speicher der Engine (Spitze/Ende) | Seen-Menge 24,6 MB (1 000 001 Einträge, 0 Überläufe, 0 Kollisionen), Warteschlange im RAM 4,8 MB (23 027 Einträge; 881 805 auf Platte, 99 Nachladevorgänge), Sortierstruktur 0,8 MB, Kern 29,4 MB, größter Writer-Batch 30,2 MB |
| Verstöße / Warnungen | 6 Verstöße gegen die PERF-002-Abschnittsgrenzen; sonst keine Verstöße, keine Warnungen |

### B2: PERF-003 large (SlowSink 100 ms je Schreibaufruf, K-52; 1 Mio. URLs, K-73)

| Messwert | Wert |
|---|---|
| Spitzen-RSS | 468 090 880 B = 468,1 MB (Grenze 500 000 000), bestanden |
| Crawl | endet regulär (`end_reason = queue_empty`), 1 000 000 URLs, 475,9 s = 2 101,1 URLs/s |
| Längster Stillstand | 4,308 s (Grenze 30 s) |
| `diag`: Zahl der Batches | 520 (`engine_w_batches`, `store_batches`; 500 mit mehr als 1 000 Seiten, 1 mit bis zu 1 000, 1 mit bis zu 200, 18 mit bis zu 50) |
| `diag`: `engine_w_pages_max` | 2 000 (= `BATCH_PAGES_BACKLOG`) |
| `diag`: `engine_w_queue_max` | 256 (= `CHANNEL_CAPACITY`; der Kanal war voll) |
| `diag`: `loadtest_slow_wait_ns` | 54 742 353 668 ns = 54,7 s bei 520 Schreibaufrufen (`loadtest_slow_calls`) |
| `diag`: Hintergrund-Checkpoints | 0 (Zahl), Maximum 0 ns, Fehler 0; Wartezeit des Writers auf einen Checkpoint (`store_wait_checkpoint_ns`) 0 ns, 0 Wartevorgänge. Checkpoints im Commit: 2, zusammen 74,2 ms (größter 74,1 ms) |
| `diag`: Schreibzeit | gesamt 418,2 s, größte 42,7 s (Endbatch mit Indexaufbau), Commit gesamt 205,1 s (größter 11,6 s, 499 von 520 „slow“) |
| `diag`: Speicherzähler der Engine (P33) | Seen-Menge 24,6 MB (1 000 001 Einträge), Warteschlange im RAM 4,6 MB (22 951 Einträge, 882 091 auf Platte, 99 Nachladevorgänge, 1 Trim), Sortierstruktur 0,8 MB, Weiterleitungen 0,5 MB (19 975 Einträge), Kern 29,5 MB, größter Writer-Batch 30,7 MB |
| Abschnitte (nur Bericht, PERF-002 gilt in PERF-001) | 2 683, 3 389, 3 270, 3 078, 2 816, 2 387, 2 111, 1 981, 1 660, 1 476 URLs/s |

Das Drosseln selbst belegt der Test `full_channel_blocks_the_sender_until_the_writer_catches_up` (`writer_thread.rs`, Tag PERF-003); hier nur zur Einordnung (K-52).

**RSS-Trend (ergänzend, derselbe Befehl mit `--pages`):**

| Seiten | Spitzen-RSS | Dauer | URLs/s | längster Stillstand | Abschnitte min bis max zum Mittel | N14 (RSS) |
|---:|---:|---:|---:|---:|---|---:|
| 100 000 | 413,5 MB | 24,4 s | 4 103 | 0,76 s | 86 % bis 120 % | 426,5 MB |
| 250 000 | 426,7 MB | 66,7 s | 3 748 | 0,70 s | 85 % bis 114 % | 470,8 MB |
| 500 000 | 436,8 MB | 158,3 s | 3 159 | 1,12 s | 96 % bis 126 % | – |
| 1 000 000 | 468,1 MB | 475,9 s | 2 101 | 4,31 s | 70 % bis 161 % | 707,6 MB |

Der RSS wächst nahezu linear mit rund 6 MB je 100 000 Seiten (Ausgangsstand laut P33: 26,8 MB je 100 000); die Hochrechnung von P33 (459 MB bei 1 Mio.) trifft mit 468 MB innerhalb ihrer Unsicherheit.

### B3: PERF-005 large (100 000 Seiten, `--kills 10`, Seed 20261014)

- Kill-Punkte (abgerufene Seiten): 14 177, 15 814, 27 500, 34 721, 38 982, 46 925, 61 498, 64 126, 71 705, 84 155.
- Je Abbruch: `integrity_check = ok` (10 von 10), `duplicate_urls` = 0 (10 von 10).
- Nach dem Fortsetzen: 100 001 URLs, 100 000 abgerufen, `urls_missing` 0, `urls_extra` 0, `urls_with_other_status` 0, Status identisch mit dem Referenzcrawl (`crawler_status_identical` = true; 200: 96 582, 301: 1 973, 404: 973, 500: 236, 503: 237).
- Wiederholte Requests je Abbruch (`crawler_repeated_per_kill`): **[2264, 2264, 2264, 2264, 2264, 2264, 2264, 2264, 2264, 2263]**, Maximum 2 264, Mittel 2 263,9, Summe 22 639 (Gesamtgrenze 22 640). Grenze je Abbruch (`crawler_repeated_bound_per_kill`, K-49) = 8 Threads + 256 Kanal + 2 000 Batch = **2 264**: nie überschritten, in 9 von 10 Abbrüchen genau erreicht. Vor dem ersten Abbruch keine wiederholten Requests (`crawler_repeated_before_first_kill` 0); robots.txt 1 Abruf; Requests insgesamt 122 640.
- Je Abbruch in der Pipeline verloren (`lost_in_pipeline`: abgerufen, aber nicht in der Datei): 1 902, 2 253, 2 255, 2 255, 2 255, 2 255, 2 255, 2 255, 1 883, 2 054.
- „abgeschnitten“ kommt in Bericht und Log 0-mal vor.
- Laufzeit des Crawls 35,7 s (N14: 65 s).

### B4: CRAWL-124 Profilvergleich (1 000 000 Seiten, Szenario perf-001, alle drei Profile)

Alle drei Läufe unter der 90-Minuten-Grenze der Vorgabe (7 bis 8 Minuten): kein Ersatz mit 250 000 Seiten.

| Profil | Cache | Spitzen-RSS | Veränderung RSS gegen Balanced | Dauer | URLs/s | Veränderung Durchsatz | längster Stillstand |
|---|---|---:|---:|---:|---:|---:|---:|
| Balanced | 256 MiB | 469,9 MB | – | 445,6 s | 2 244,3 | – | 4,7 s |
| MinMemory | 64 MiB | **217,4 MB** | **-53,7 %** | 430,3 s | 2 324,0 | +3,6 % | 3,8 s |
| MaxSpeed | 4 GiB | 5 437,2 MB | +1 057 % | 491,5 s | 2 034,8 | -9,3 % | 6,8 s |

- Kriterium „MinMemory mindestens 20 % unter Balanced“: erfüllt (-53,7 %; Grenze <= 375,9 MB).
- RSS-Bewertung der Berichte: Balanced und MinMemory werden mit den PERF-001-Grenzen bewertet (RSS-Verhältnis 1,130 bzw. 1,316 gegen Grenze 2), MaxSpeed ist „nicht bewertet (Profil max-speed, K-16)“ (nach N19 wie vorgegeben, Verhältnis 7,13 steht nur im Bericht).
- PERF-002-Abschnitte (Mittel; URLs/s je Abschnitt; Anteil am Mittelwert) – gelten nur für das Standardprofil Balanced, der Lasttest wertet sie für alle Profile und meldet `passed = false`, Exit-Code 1 für alle drei Läufe:
  - Balanced: 2 244; 3 104, 3 863, 3 591, 3 534, 2 859, 2 449, 2 177, 1 934, 1 770, 1 603; 138, 172, 160, 157, 127, 109, 97, 86, 79, 71 %.
  - MinMemory: 2 324; 2 908, 3 928, 3 616, 3 572, 2 895, 2 542, 2 261, 2 052, 1 846, 1 735; 125, 169, 156, 154, 125, 109, 97, 88, 79, 75 %.
  - MaxSpeed: 2 035; 3 293, 4 155, 4 139, 3 930, 3 141, 2 385, 1 901, 1 508, 1 339, 1 227; 162, 204, 203, 193, 154, 117, 93, 74, 66, 60 %.

## Dateien in diesem Ordner

- `report-perf-001-large.json`, `report-perf-003-large.json`, `report-perf-005-large.json`: Berichte von `os-loadtest` (Schema `{scenario, size, started_at, metrics, passed}`).
- `report-perf-003-large-{100000,250000,500000}-pages.json`: RSS-Trend.
- `report-crawl-124-{balanced,min-memory,max-speed}-1m.json`: Profilvergleich bei 1 Mio. Seiten.
- `log-*.txt`: Kopf (Start, `wait_quiet`-Ergebnis, Load, Befehl), Konsolenausgabe und Ende je Block; `log-*.txt.load`: Lastproben jede Minute (Zeit, Load 1/5/15 min).
- `n14-*`: Berichte und Logs der Messung N14 (vor Welle 7, Commit `43caf19`), nur zum Vergleich. Dort fehlt PERF-002.
- `probe-p32/`, `probe-p33/`: Kurzproben der Pakete M1-P32 und M1-P33 (Entwicklung, Welle 7).
