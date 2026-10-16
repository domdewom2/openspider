# Kurzproben M1-N27 (Endbatch bei 1 Mio. Seiten, K-83, K-84)

Messungen der Entwicklung (Welle 7e, Messregel Entwicklung: parallel wird gebaut und gearbeitet, es zählt nur A gegen B unter gleicher Last; absolute Werte gelten nicht für A4, das misst M1-N28). A ist `origin/main` zum Paketstart (a60403e, Welle 7d), B der Branch `m1/m1-n27`. Maschine: Apple M2, 8 Kerne, 16 GB RAM, macOS 26.6.2, Release-Builds (`CARGO_INCREMENTAL=0`, `RUSTC_WRAPPER=sccache`). Jede Folge lief unter einem `lockf` auf `bench.lock` und `caffeinate -dimsu` (`scripts/bench/ab_probe.sh`), vor jedem Lauf `wait_quiet.sh 8 10`; in allen Läufen war die Bedingung erfüllt (Load beim Start 2,7 bis 6,5, im Lauf bis 14, siehe Spalte „Load“; Ursache sind die parallel arbeitenden M2-Worktrees). Rohdaten (Berichte `*.json`, Logs, Lastproben je 30 s) unter `raw/`, Tabellen mit `ab_table.py`.

## Ergebnis in Kürze

* **K-83 trägt das Ziel allein.** „Exact Duplicates“ ist eine unkorrelierte Gruppierung (Treffermenge unverändert), kein Filter irgendeines Tabs benutzt einen der fünf Indizes `urls_title1`, `urls_md1`, `urls_h1`, `urls_h2`, `urls_hash`. Ab `DROP_FROM_URLS` baut der Writer sie im Endbatch nicht mehr auf, `ensure_if_idle` zieht sie nicht nach, das Vorwärmen entfällt. Der Endbatch fällt bei 1 Mio. Seiten von 53 bis 66 s (A) auf **26,8 und 27,0 s** (B1, B2); N26 maß 92 bis 99 s.
* **Ziel auf dem Branch (A, B, B, A bei 1 Mio. Seiten, Abschnitt 3):** Endbatch 27,0 / 26,8 s (Grenze 45 s) erfüllt; `crawler_violations` in beiden B-Läufen leer, einschließlich K-80, bei A 1 und 6 Verstöße; Mittel B 3 440,8 gegen A 3 137,8 URLs/s (+9,7 %); Spitzen-RSS B 468,4 MB gegen A 474,4 MB (-6,1 MB, Grenze +10 MB); längster Stillstand B 1,65 s (Grenze 10 s); Kaltprobe `links_target` 25,3 bis 27,2 s (Grenze 35 s), mit `COMMIT` höchstens 30,0 s.
* **Abschlussprobe:** `perf-003 --size large` 462, 466, 478 MB Spitzen-RSS (Grenze 490 MB), jeweils `queue_empty`; `perf-005 --size large --kills 10` grün, wiederholte Requests je Abbruch höchstens 2 264 (Grenze K-49: 2 264).
* **K-84 (3), kürzerer `links_target`-Aufbau:** nicht erreicht, und nicht nötig. Geprüft und verworfen: vier statt zwei Hilfsthreads des Sortierers (spart im Crawl 1,5 s, kostet aber bei `perf-003` in drei von vier Läufen 500 bis 590 MB Spitzen-RSS), größere Sortierpuffer (langsamer). `links_target` bleibt bei zwei Threads und 4 MiB Cache (Abschnitt 4). Es steht weiter im Endbatch, in der Transaktion des letzten Batches (CRAWL-125 und K-49 unverändert).
* **Nicht geändert:** K-72, K-80, Migrationen, Schemaspalten, `CHANNEL_CAPACITY`, `BATCH_PAGES_BACKLOG`, Thread-Zahl, Goldens, Paritätserwartungen; öffentliche Signaturen nur additiv (`url_index::is_large`, Zähler `end_data_ns`, `end_commit_ns`, `w_end_batch_ns`, `w_end_batches`).

## 1 K-83, `os-analyze`: Exact Duplicates als Gruppierung

`tabs/content.rs`: `hash_md5 IN (SELECT hash_md5 FROM <Vergleichsmenge> GROUP BY hash_md5 HAVING COUNT(*) > 1)` statt `EXISTS (… d.hash_md5 = u.hash_md5 AND d.id <> u.id)`. Eine Seite `u` außerhalb der Vergleichsmenge (zweiter Zweig des Prädikats) braucht nur einen Treffer in ihr, eine Seite innerhalb zwei, weil sie sich selbst mitzählt. NULL und leere Hashes zählen nie; `only_indexable` und die Grundmenge wirken wie zuvor; der Filter bleibt live. Die vier Filter „Duplicate“ (Page Titles, Meta Description, H1, H2) gruppieren seit bf521c1 unkorreliert und bleiben unverändert.

Tests (`crates/os-analyze/tests/k83_duplicate_plans.rs`, Spec-Tags ONPAGE-143):

* Gleichheit alt gegen neu für **jede Zeile** einer Fixture mit 21 Zeilen: zwei 2xx-Seiten mit gleichem Hash, Nicht-2xx-HTML mit dem Hash einer 2xx-Seite (404 mit Treffer, zwei 404 ohne), Einzelseiten, nicht indexierbare Seiten, PDF, externe Seite, `in_tables = 0`, Status NULL, leere und fehlende Hashes; `only_indexable` an und aus; dazu die Sicht (`query_view`) mit denselben Mengen.
* `EXPLAIN QUERY PLAN` der fünf Duplicate-Filter ohne korrelierte Unterabfrage über `urls`, mit und ohne die fünf Indizes in der Datei (Gegenprobe: das alte `EXISTS` sucht über `urls_hash` und läuft ohne Index je Zeile über `urls`).
* `EXPLAIN QUERY PLAN` aller Filter aller Tabs (`tabs::all()`, über 100 Filter) auf einer Datei mit und ohne die fünf Indizes: kein Plan nennt einen der fünf Indizes, ohne sie läuft keine korrelierte Unterabfrage über `urls`. Die Inventur der Leser in `docs/ARCHITECTURE.md` ist berichtigt (Abschnitt Crawl-Datei); eine Suche über die Spalten gibt es in M1 nirgends (`grep` über `crates`: `title_1`, `meta_desc_1`, `h1_1`, `h2_1` und `hash_md5` kommen außerhalb von `os-store` (Writer, Migration) nur in den Spaltenausdrücken und Filtern von `os-analyze/src/tabs/` und im Schreibmodell vor, nie in `os-mcp`, `os-export`, `os-engine` als Suchbedingung).
* `crates/os-store/tests/url_index.rs`: nach `Completed` fehlen die fünf Indizes in einer großen Datei (Test-Hook `with_url_index_drop_at` und echte Schwelle über die größte Kennung), `links_target` steht, `ensure_if_idle` baut die fünf nicht (aber `links_target`), eine kleine Datei behält alle, `url_index::ensure` bleibt öffentlich.

Benchmark `crates/os-analyze/tests/bench_duplicates.rs` (`#[ignore]`, nur Bericht; 1 Mio. schmale Zeilen ohne Indizes, rund 30 % der Zeilen in Gruppen, Last 3,6 bis 3,9; Rohdaten `raw/bench-duplicates.{log,json}`; die Zeit enthält die Gesamtzahl und die erste Seite):

| Filter | Treffer | Zeit |
|---|---:|---:|
| Page Titles: Duplicate | 300 000 | 11,5 s |
| Meta Description: Duplicate | 300 000 | 11,7 s |
| H1: Duplicate | 300 000 | 12,0 s |
| H2: Duplicate | 300 000 | 13,4 s |
| Content: Exact Duplicates | 300 000 | 4,1 s |

Das korrelierte `EXISTS` ohne Index brauchte nach N22 bei 20 000 Zeilen 6,8 s (quadratisch, hochgerechnet auf 1 Mio. Zeilen Stunden). Die vier Titelfilter sind langsamer, weil sie mit der Kollation `OS_TEXT` gruppieren (K-58); das ist unverändert.

## 2 K-83, `os-store`

`url_index::is_large(conn)` (größte `urls.id` mindestens `drop_threshold()`) trifft dieselbe Entscheidung wie der Writer. `SqliteSink::write` baut die Indizes beim regulären Ende nur noch, wenn die Datei sie behalten hat (nicht „dropped“), und wärmt den Cache nur vor, wenn dann etwas fehlt. `link_index::ensure_if_idle` zieht die fünf Indizes nur für eine kleine Datei nach. Neue Zähler in `os_store::sink::diag`: `end_data_ns` (Datenanweisungen des letzten Batches) und `end_commit_ns` (`COMMIT`), in `os_engine::diag`: `w_end_batch_ns`, `w_end_batches` (Schreibaufruf mit `Completed`).

## 3 A/B-Kurzprobe (A, B, B, A, 1 Mio. Seiten, Abgabestand: zwei Sortierer-Threads)

`raw/ab1m`. Endbatch = Dauer des Schreibaufrufs mit `Completed` (bei A: größter Schreibaufruf `engine_w_write_max_ns`, das ist der Endbatch). Die Spalten „Daten“, „links_target“, „urls-Indizes“, „Vorwärmen“ und „Commit“ sind Bestandteile des Endbatchs in Sekunden (A hat „Daten“ nicht; „Commit“ ist bei A der größte Commit des letzten Eimers `store_commit_max_ns_h05`). „Nachlauf“ = Crawl-Dauer minus Zeit bis zur letzten abgerufenen URL (`crawler_tail_s`). „Verstöße“ = Zahl der Einträge in `crawler_violations`.

| Lauf | Mittel URLs/s | Dauer s | Abschnitte min..max % | RSS MB | Stillstand s | Endbatch s | Daten | links_target | urls-Indizes | Vorwärmen | Commit | Nachlauf s | Verstöße | Load min..max |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| A1 | 3 193,7 | 313,1 | 91..132 | 470,6 | 1,20 | 53,4 | - | 26,0 | 23,1 | 8,7 | 4,1 | - | 1 | 3,5..4,8 |
| B1 | 3 466,2 | 288,5 | 84..119 | 465,7 | 1,60 | **27,0** | 0,18 | 23,1 | - | - | 3,7 | 27,5 | **0** | 3,3..6,5 |
| B2 | 3 415,5 | 292,8 | 81..121 | 471,0 | 1,70 | **26,8** | 0,18 | 22,8 | - | - | 3,8 | 27,3 | **0** | 4,8..14,0 |
| A2 | 3 081,9 | 324,5 | 95..133 | 478,2 | 1,00 | 65,6 | - | 21,8 | 35,7 | 8,6 | 7,9 | - | 6 | 4,5..7,3 |

Mittel A 3 137,8, B 3 440,8 URLs/s (+9,7 %). Spitzen-RSS A 474,4 MB, B 468,4 MB (-6,1 MB). Längster Stillstand A 1,10 s, B 1,65 s. Endbatch A 59,5 s, B 26,9 s. Die Verstöße von A sind Abschnitte über 130 % des Mittels und bei A2 der Endbatch über 60 s (K-80), also dieselben Befunde wie in N26. Die Abschnitte von B liegen bei 81 % bis 121 % des Mittels. Der Endbatch von A schwankt (53 bis 66 s, N26: 92 bis 99 s, kalter Dateicache bis 82 s) vor allem durch die fünf Aufbauten (23 bis 36 s) und das Vorwärmen (9 s); `links_target` allein ist mit 22 bis 26 s (A) und 23 s (B) gleich. Der Endbatch von B besteht zu 85 % aus `links_target`.

Frühere Zwischenstände derselben Folge:

* `raw/k83` (nur K-83, A0 und B0): Endbatch A0 78,5 s (fünf Indexaufbauten 49,9 s, Vorwärmen 14,2 s, `links_target` 22,9 s), B0 28,1 s (`links_target` 24,7 s, Commit 3,2 s, Daten 0,17 s); Mittel 2 776,5 gegen 3 132,9 URLs/s; RSS 453,5 gegen 460,9 MB; Load 3,9 bis 11,2.
* `raw/ab1m-threads4-verworfen` (K-83 mit vier Sortierer-Threads): Endbatch B 22,0 und 21,9 s, `links_target` 18,6 und 18,4 s, Mittel +13,6 %; aber RSS B2 491,1 MB gegen A 470,9 und 474,4 MB. Abschnitt 4 zeigt, warum die Variante verworfen ist.

## 4 K-84, `links_target`: gemessene Hebel

Mikrobenchmark `crates/os-store/tests/bench_links_build.rs` (`#[ignore]`): eine Store-Datei mit 1 Mio. Seiten x 30 gleichverteilten Links und breiten Linkzeilen (rund 130 Byte, wie in einer echten Crawl-Datei; 5,5 GB), ohne `links_target`; je Variante eine Kopie, Aufbau in einer Transaktion samt `COMMIT`. Rohdaten `raw/kaltprobe.log`.

**Kaltprobe.** `sudo -n /usr/sbin/purge` verlangt ein Passwort und ist hier nicht möglich (`sudo: a password is required`). Statt dessen kommt die Kopie der Vorlage (`fs::copy`, Klon auf APFS) ohne Seiten im Dateicache zustande, bei den Läufen „cold“ wird zusätzlich eine Füllerdatei von 26 GB (mehr als der RAM) geschrieben und gelöscht. Der Zähler `Pageins` von `vm_stat` (Seiten zu 16 KiB, systemweit) steigt in allen Läufen um 328 000 bis 341 000, das sind 5,0 bis 5,2 GiB und damit die Größe der Datei genau einmal: Der Aufbau liest die Datei von der Platte, und zwar sequenziell einmal von vorn bis hinten (nicht mehrfach, nicht je Zeile). Auch der Lauf ohne Füller ist deshalb kalt.

| Lauf | Aufbau s | mit `COMMIT` s | Benutzer-CPU s | System-CPU s | Pageins (16 KiB) | Spitzen-RSS |
|---|---:|---:|---:|---:|---:|---:|
| 2 Threads, 4 MiB, ohne Füller | 24,8 | 27,8 | 15,1 | 5,9 | 328 094 | 23 MiB |
| 2 Threads, 4 MiB, kalt | 27,2 | 30,0 | 15,4 | 5,7 | 340 752 | 24 MiB |
| 2 Threads, 4 MiB, kalt | 25,3 | 28,7 | 15,2 | 5,7 | 333 493 | 24 MiB |
| 4 Threads, 4 MiB, kalt | 22,5 | 25,5 | 16,0 | 6,3 | 331 836 | 32 MiB |

Die Grenze von 35 s (Kaltprobe `links_target`) ist erfüllt (25,3 bis 27,2 s, mit `COMMIT` 30,0 s). Ein früherer Durchgang mit geringerer Last (nicht protokolliert, Werte aus der Konsole): 2 Threads 20,1 bis 22,6 s, 4 Threads 18,1 bis 21,5 s. Im Crawl dauert der Aufbau 22 bis 26 s (A und B).

**Hebel auf der 500 000-Seiten-Vorlage** (Last 4 bis 7, einzelne Läufe, Streuung etwa 10 %): Threads 2: 10,3 und 10,8 s, Threads 4: 8,7 und 9,1 s, Threads 6 und 8: 8,7 bis 11,3 s, Threads 3: 8,8 s; Sortierpuffer (Cache) 2, 4, 8 MiB: kein Unterschied; sqlite3-Shell (15 Mio. Links, 4 Threads): 4 MiB 15,9 s, 64 MiB 18,6 s, 64 MiB ohne Threads 29,1 s (große Sortierlisten sind weniger cachefreundlich). Die Benutzerzeit der Hauptlinie (rund 15 s bei 1 Mio. Seiten: Schlussmischen, Füllen des Baums) bestimmt den Aufbau; mehr Hilfsthreads, größere Puffer oder ein anderer Ablageort der Sortierläufe ändern daran nichts Messbares. Das Verlegen des Aufbaus in eine eigene Transaktion vor `Completed` spart keine Zeit, weil die Datenanweisungen des letzten Batches nur 0,18 s dauern.

**Vier Threads (verworfen).** `perf-003 --size large`, Spitzen-RSS in MB: vier Threads 590, 462, 500, 577 (Läufe `final-perf003/01-B3`, `02-B4`, `final-perf003b/01-B5`, `03-B6`), A 465 und 462; zwei Threads 462, 466, 478 (`final-perf003c`). Die Grenze ist 500 MB mit mindestens 10 MB Reserve. Der Gewinn von vier Threads im Crawl beträgt 1,5 s (`links_target` 18,5 gegen 20 s im Zwischenlauf). Deshalb bleibt es bei zwei Threads und 4 MiB Cache (`BUILD_THREADS`, `BUILD_CACHE_KIB` unverändert); die Dokumentation der Konstanten nennt den Befund.

## 5 K-84, `os-loadtest`

`crawler_end_batch_s` (aus dem Zähler `engine_w_end_batch_ns` des Kindprozesses) und `crawler_tail_s` (Crawl-Dauer minus Summe der zehn Abschnittsdauern, also minus die Zeit bis zur letzten abgerufenen URL) stehen in jedem Szenario im Bericht, `null`, wenn der Crawler sie nicht liefert (z. B. `perf-005`). In `perf-001` ab 1 000 000 Seiten steht „K-80: Endbatch <x> s > 60 s“ in `crawler_violations`, sobald der Endbatch 60 s übersteigt; ein nicht messbarer Endbatch ist dort selbst eine Verletzung („K-80: Endbatch nicht messbar“). Die Auswertung sitzt in `throughput.rs` (`end_batch_violation`, `tail_seconds`, `end_batch_seconds`) und wird in `runner.rs` angewendet (Einheitstests dort und in `tests/run.rs`, Spec PERF-002).

## 6 Abschlussprobe (keine Abnahme)

Binär: Abgabestand (zwei Sortierer-Threads).

| Lauf | Ergebnis | Rohdaten |
|---|---|---|
| `perf-003 --size large`, T1/T2/T3 | Spitzen-RSS 462,3 / 466,3 / 478,9 MB (< 490 MB), `end_reason = queue_empty`, Endbatch 27,6 / 24,8 / 26,0 s, Stillstand 1,4 / 1,2 / 2,3 s, `crawler_violations` leer | `raw/final-perf003c` |
| `perf-005 --size large --kills 10` | `passed = true`, 10 Abbrüche, jeder mit `integrity_check = ok`, 100 001 URLs, 0 Doppelte, wiederholte Requests je Abbruch `[2264, 2172, 2264, 2264, 2037, 2264, 2264, 2264, 2177, 928]` (Maximum 2 264 = Grenze K-49) | `raw/final-perf005` |

## 7 Hinweise

* **Speicherplatz.** Während der Arbeit war die Platte der Maschine zeitweise voll (andere Worktrees und alte `target-m1-n2*`-Ordner, Verfügbar zwischen 0,1 und 100 GB); ein erster A-Lauf wurde dafür abgebrochen und alle Arbeitsdateien dieses Pakets danach gelöscht. Messblöcke brauchen rund 7,5 GB frei (Crawl-Datei 6,3 GB, WAL 1,2 GB).
* **Streuung.** Spitzen-RSS und Durchsatz streuen auch bei gleicher Binärdatei (A: 453 bis 478 MB, B: 460 bis 471 MB im Abgabestand); `perf-003` zeigte bei den verworfenen vier Threads Ausreißer bis 590 MB. Die Last der parallelen M2-Worktrees (Load bis 14) beeinflusst den Durchsatz, nicht die Aussage Endbatch gegen 45 s.
* **Abbruch im Endbatch** (CRAWL-125): unverändert. `links_target` entsteht in der Transaktion des letzten Batches; ein Abbruch davor lässt die Datei ohne Index und ohne `Completed`, `ensure_if_idle` baut ihn beim nächsten Lesen mit Schreibzugriff nach.
* **Reproduktion:** `scripts/bench/ab_probe.sh docs/plan/M1-acceptance/loadtest/probe-n27/raw/ab1m <arbeitsordner> 1000000 A1=<ordner-mit-release/os-loadtest-von-origin/main> B1=<ordner-B> B2=<ordner-B> A2=<ordner-A>`; Tabellen: `python3 ab_table.py raw/ab1m`; Mikrobenchmark: `OS_BENCH_MASTER=<datei> OS_BENCH_PAGES=1000000 cargo test -p os-store --release --test bench_links_build -- --ignored --nocapture create_master`, danach `OS_BENCH_VARIANTS=2:4096 [OS_BENCH_FILLER_MB=26000] … build_variants`; Duplicate-Filter: `OS_BENCH_ROWS=1000000 OS_BENCH_DIR=<ordner> OS_BENCH_OUT=<json> cargo test -p os-analyze --release --test bench_duplicates -- --ignored --nocapture`.
