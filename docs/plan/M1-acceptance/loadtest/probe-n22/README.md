# Kurzproben M1-N22 (PERF-002 Konstanz im 1-Mio.-Crawl, K-80)

Messungen der Entwicklung (Welle 7c, Messregel Entwicklung: parallel wird gebaut, es zählt nur A gegen B unter gleicher Last; absolute Werte gelten nicht für A4). Arbeitsordner außerhalb des Repos, Release-Builds, `caffeinate -dimsu`, jede Folge unter einem `lockf` auf `bench.lock`, vor jedem Lauf `wait_quiet.sh 8 10` (in allen Läufen erfüllt: Load beim Start unter 8). Aufrufhilfe: `scripts/bench/ab_probe.sh`. Rohdaten (Berichte `*.json`, Logs, Lastproben je 30 s) unter `raw/`, Auswertung mit `summarize.py`, `ab_table.py`, `report_tables.py` in diesem Ordner. Maschine: Apple M2, 8 Kerne, 16 GB RAM, macOS 26.6.2. Basis A = `origin/main` `f8357b5` (Release-Build einmal, Binärdateien in `openspider-wt/n22-base/`, A-Target danach gelöscht), B = Branch `m1/m1-n22`.

## Ergebnis in Kürze

* **Ursache (belegt):** die fünf Sekundärindizes von `urls` über Textspalten mit zufälligen Schlüsseln (`urls_title1`, `urls_md1`, `urls_h1`, `urls_h2`, `urls_hash`). Mit ihnen schreibt jeder Batch rund 14 000 Seiten ins WAL (ohne sie 9 000), die Commit-Dauer je Batch wächst von 95 auf 196 ms, und die Rückführung des WAL im Commit wächst mit (39, 108, 121 ms je Batch). Ohne sie bleibt der Commit flach (74, 66, 78 ms), und die Rückführung auch (30, 29, 40 ms).
* **Nicht Ursache:** das Nachladen der Warteschlange (Wartezeit auf `writer.flush()` 0,3 bis 0,5 s je Nachladevorgang im Hintergrund, null Starvation in allen Läufen, ohne Warten bleibt der Verlauf gleich, das Mittel liegt mit +6 % im Rauschen) und die WAL-Rückführung als eigener Posten (Folgeeffekt der Zufallsindizes; größere Schwelle +5 %, Hintergrund-Checkpoint -3 %).
* **Abhilfe (Standardverhalten):** ab 250 000 URLs entfernt der Writer die fünf Indizes vor dem Batch, der die Schwelle erreicht, und baut sie beim regulären Ende in der Transaktion des letzten Batches sortiert auf (Vorwärmen des Dateicaches durch vier Leser, zwei Hilfsthreads des Sortierers bei allen Indexaufbauten). Kleinere Crawls ändern sich nicht.
* **A/B 500 000 Seiten (A, B, B, A, Abgabestand):** Mittel **+17,9 %** (B 4 247, A 3 604 URLs/s); Abschnitt 10 gegen schnellsten Abschnitt **0,95** (A 0,77); Commit je 100 Batches flach (letzter voller Eimer 68 ms gegen erster 72 ms); längster Stillstand 0,85 s (A 1,26 s); Endbatch 13,6 s (A 15,5 s), hochgerechnet auf 1 Mio. rund 27 s. Spitzen-RSS B 477 MB, A 469 MB (+8 MB; die Läufe streuen von 440 bis 507 MB, auch A).
* **Abschlussprobe 1 Mio. (Abgabestand):** `perf-001 --size large` zweimal mit **leerem `crawler_violations`** (G1: 3 732 URLs/s, Abschnitte 87 % bis 119 %; G2: 3 844 URLs/s, 86 % bis 119 %), Endbatch 31,5 und 28,6 s; `perf-005 --size large --kills 10` grün (Maximum 2264 = Grenze 2264); `writer_1m_uniform_segments` flach (99 % bis 101 %).
* **Streuung, ehrlich:** Eine frühere Fassung (ohne Sortierer-Threads) bestand `perf-001` in 3 von 4 Läufen, verfehlte ihn einmal (F1: Endbatch 82 s, weil die fünf Indexaufbauten 45 statt 5 s brauchten, der Dateicache war kalt; das Mittel sinkt dann, und die gleichmäßigen Abschnitte liegen über 130 %). Dasselbe kann dem Abgabestand bei kaltem Cache passieren (siehe Abschnitt 6); die Abnahme A4 braucht deshalb die ruhige Maschine.

## 1 Diagnose-Zähler

`os-store` `sink::diag`: Aufbau- und Commit-Dauer je 100 Batches (16 Eimer, der letzte nimmt den Rest) als feste Zähler `*_h00` … `*_h15` (Summe und Maximum), Aufbau je Anweisungsgruppe (`g_urls_ins`, `g_urls_upd`, `g_links`, `g_counters`, `g_queue`, `g_rest`), Länge der `-wal`-Datei, Cache-Zähler (`cache_miss`/`cache_write` je Eimer, `cache_hit`/`cache_spill` gesamt, `sqlite3_db_status`), die WAL-Rückführung getrennt (`wal_cp_*`, nur mit dem Diagnoseschalter `OS_STORE_WAL_EXPLICIT=1`: `wal_autocheckpoint` steht dann auf 0 und der Writer führt das WAL nach dem Commit mit `PASSIVE` selbst zurück; ein eigener `wal_hook` hätte die Automatik von SQLite ersetzt und die Erkennung eines von außen gesetzten Wertes gestört), Indexaufbau (`url_index_*`, `link_index_*`) und Vorwärmen. `os-engine` `diag`: Dauer, Zeilen und Wartezeit auf `writer.flush()` je Nachladevorgang (Summe, Maximum, Verlauf je 10 Nachladevorgänge `q_refill_*_rNN`), Starvation (`q_starved_*`), Auslagern (`q_activate_*`). `os-loadtest` ist unverändert; alle Zähler stehen im `diag`-Block des Berichts.

## 2 Ursache trennen (500 000 Seiten, einzelne Läufe)

Eimer 1 bis 3 = Batches 0 bis 99, 100 bis 199, 200 bis Ende (der dritte Eimer ist unvollständig und enthält den Endbatch-Commit). Die Läufe stammen von Zwischenständen (`diag1`: nur Zähler; `exp1`: Zähler und Diagnoseschalter, Standard aus; im Abgabestand ist (a) das Standardverhalten) und aus zwei Blöcken mit unterschiedlicher Last der Maschine (letzte Spalte); sie ordnen die Größenordnungen ein, die Entscheidung trägt die A/B-Probe in Abschnitt 4.

| Lauf | Variante | Schalter | Mittel URLs/s | Dauer s | Abschnitte min..max % | Commit ms/Batch (Eimer 1/2/3) | WAL-Rückführung ms/Batch | Seiten ins WAL je Batch (Eimer 1/2/3) | Refill-Wartezeit ms | Load |
|---|---|---|---:|---:|---|---|---|---|---:|---|
| diag1/01-D0 | Diagnosezähler, Standard | `-` | 3101 | 161 | 98..139 | 124 / 190 / 258 | - | 11446 / 13779 / 17415 | 432 | 4.2..7.8 |
| diag1/02-D1 | WAL-Rückführung nach dem Commit gezählt | `OS_STORE_WAL_EXPLICIT=1` | 3215 | 156 | 96..141 | 57 / 64 / 70 | 39 / 108 / 121 | 11449 / 13775 / 16349 | 394 | 3.7..6.7 |
| exp1/01-Z0 | Standard (Wiederholung) | `-` | 3456 | 145 | 97..133 | 95 / 144 / 196 | - | 11058 / 14157 / 16364 | 383 | 3.3..6.9 |
| exp1/02-R1drop | (a) fünf Zufallsindizes von urls entfernt, am Ende gebaut | `OS_STORE_DROP_URL_INDEXES=1` | 4069 | 123 | 91..127 | 74 / 66 / 78 | - | 8927 / 8519 / 9913 | 278 | 3.1..5.2 |
| exp1/05-R5dropexp | (a) + Rückführung getrennt gezählt | `OS_STORE_DROP_URL_INDEXES=1, OS_STORE_WAL_EXPLICIT=1` | 4141 | 121 | 91..126 | 45 / 37 / 37 | 30 / 29 / 40 | 8838 / 8521 / 10057 | 268 | 4.3..4.8 |
| exp1/06-R2noflush | (b) Nachladen ohne Warten auf writer.flush() | `OS_ENGINE_REFILL_NOFLUSH=1` | 3666 | 136 | 91..130 | 100 / 121 / 216 | - | 11882 / 14636 / 17608 | 0 | 3.7..4.7 |
| exp1/03-R3wal | (c) WAL-Schwelle 512 MiB bis 1 GiB | `OS_STORE_WAL_BOUNDS=131072:262144` | 3648 | 137 | 91..129 | 95 / 99 / 199 | - | 11315 / 13893 / 17428 | 344 | 4.0..5.1 |
| exp1/04-R4bg | (c) Hintergrund-Checkpoint (K-76) | `OS_STORE_BACKGROUND_CHECKPOINT=1` | 3340 | 150 | 100..131 | 98 / 137 / 175 | - | 11072 / 13863 / 16840 | 389 | 4.5..5.3 |

Aufbauzeit je Anweisungsgruppe (ms je Batch, Eimer 1 bis 3), Standard gegen (a):

| Lauf | Gruppe | Eimer 1 | Eimer 2 | Eimer 3 |
|---|---|---:|---:|---:|
| exp1/01-Z0 | Aufbau gesamt | 235.6 | 217.1 | 208.9 |
| exp1/01-Z0 | urls einfügen | 38.8 | 0.0 | 0.0 |
| exp1/01-Z0 | urls aktualisieren | 29.3 | 34.6 | 33.6 |
| exp1/01-Z0 | links | 58.2 | 65.4 | 64.2 |
| exp1/01-Z0 | url_counters | 95.0 | 106.4 | 101.0 |
| exp1/01-Z0 | queue | 6.2 | 1.2 | 1.0 |
| exp1/01-Z0 | Rest | 8.3 | 9.6 | 9.2 |
| exp1/01-Z0 | Commit | 95.4 | 143.5 | 196.3 |
| exp1/02-R1drop | Aufbau gesamt | 220.4 | 199.0 | 190.7 |
| exp1/02-R1drop | urls einfügen | 29.0 | 0.0 | 0.0 |
| exp1/02-R1drop | urls aktualisieren | 18.2 | 19.4 | 18.9 |
| exp1/02-R1drop | links | 60.5 | 64.7 | 63.2 |
| exp1/02-R1drop | url_counters | 97.9 | 104.3 | 98.5 |
| exp1/02-R1drop | queue | 6.2 | 1.1 | 1.0 |
| exp1/02-R1drop | Rest | 8.5 | 9.5 | 9.0 |
| exp1/02-R1drop | Commit | 74.2 | 66.0 | 77.5 |

Lesart: Im Standardlauf wächst nur der Commit (95, 144, 196 ms) und mit ihm die Zahl der Seiten, die der Cache ins WAL schreibt (11 000, 14 000, 16 000 je Batch); die Aufbauzeit je Gruppe ist flach. Mit dem getrennt gezählten Checkpoint sieht man, wer im Commit wächst: die Rückführung (39, 108, 121 ms je Batch), der Rest des Commits bleibt flach (57, 64, 70 ms). Ohne die fünf Indizes (a) fällt die Zahl der Seiten je Batch auf 9 000, `UPDATE urls` halbiert sich (29 auf 18 ms), der Commit bleibt flach, und die Rückführung wächst nicht mehr (30, 29, 40 ms). (b) und (c) ändern den Verlauf nicht.

## 3 Abhilfe

Siehe `docs/ARCHITECTURE.md`, Abschnitt Crawl-Datei und Speicher (Lebenszyklus, Inventur der Leser). Neu: `os_store::url_index` (Liste der Indizes, `ensure`, `drop_indexes`, `Prefetch`), `SqliteSink` (Schwelle, `keep_url_indexes`, `with_url_index_drop_at`), `link_index::ensure_if_idle` baut jetzt beide Arten nach. Tests: `crates/os-store/tests/url_index.rs`, `diag_series.rs`, `crates/os-engine/tests/perf_queue_window.rs` (Spec-Tag PERF-002).

## 4 A/B-Kurzprobe (A, B, B, A, 500 000 Seiten)

Abgabestand (`raw/ab3`); der Endbatch-Wert ist die Schreibzeit des längsten Batches (`engine_w_write_max_ns`), die Spalte „Commit“ gibt es nur für B (A hat die Zähler nicht).

| Lauf | Mittel URLs/s | Dauer s | Abschnitt 10 / schnellster | min..max % Mittel | RSS MB | Stillstand s | Commit erster Eimer ms | Commit letzter voller Eimer ms | Endbatch s | Load min..max |
|---|---|---|---|---|---|---|---|---|---|---|
| A1 | 3567.3 | 140.2 | 0.76 | 100..133 | 442.8 | 1.25 | - | - | 15.9 | 3.8..5.7 |
| B1 | 4238.9 | 118.0 | 0.94 | 90..122 | 447.1 | 0.80 | 72.8 | 68.0 | 13.8 | 3.3..4.1 |
| B2 | 4255.8 | 117.5 | 0.96 | 86..122 | 506.9 | 0.90 | 70.5 | 68.4 | 13.3 | 3.6..4.6 |
| A2 | 3640.6 | 137.3 | 0.78 | 96..130 | 494.9 | 1.27 | - | - | 15.2 | 2.8..4.1 |

Mittel A 3603.9, B 4247.4 URLs/s (+17.9 %)
RSS A 468.9 MB, B 477.0 MB (+8.1 MB)
Abschnitt 10 / schnellster: A 0.77, B 0.95
Stillstand: A 1.26 s, B 0.85 s
Endbatch: A 15.5 s, B 13.6 s (nur Schreibzeit des längsten Batches)

Frühere Fassungen derselben Probe (Indizes immer entfernt, ohne Sortierer-Threads; `raw/ab2`, `raw/ab1`):

| Lauf | Mittel URLs/s | Dauer s | Abschnitt 10 / schnellster | min..max % Mittel | RSS MB | Stillstand s | Commit erster Eimer ms | Commit letzter voller Eimer ms | Endbatch s | Load min..max |
|---|---|---|---|---|---|---|---|---|---|---|
| A1 | 3384.2 | 147.7 | 0.70 | 94..135 | 441.3 | 1.94 | - | - | 16.0 | 3.4..4.7 |
| B1 | 4119.7 | 121.4 | 0.99 | 92..127 | 449.7 | 0.90 | 69.2 | 71.1 | 19.1 | 3.2..5.2 |
| B2 | 4189.4 | 119.3 | 0.98 | 95..125 | 446.2 | 0.60 | 62.3 | 69.1 | 17.4 | 3.5..4.3 |
| A2 | 3629.3 | 137.8 | 0.76 | 97..132 | 446.8 | 1.29 | - | - | 15.5 | 3.7..5.2 |

Mittel A 3506.8, B 4154.5 URLs/s (+18.5 %)
RSS A 444.0 MB, B 447.9 MB (+3.9 MB)
Abschnitt 10 / schnellster: A 0.73, B 0.98
Stillstand: A 1.62 s, B 0.75 s
Endbatch: A 15.8 s, B 18.3 s (nur Schreibzeit des längsten Batches)

| Lauf | Mittel URLs/s | Dauer s | Abschnitt 10 / schnellster | min..max % Mittel | RSS MB | Stillstand s | Commit erster Eimer ms | Commit letzter voller Eimer ms | Endbatch s | Load min..max |
|---|---|---|---|---|---|---|---|---|---|---|
| A1 | 3478.5 | 143.7 | 0.74 | 97..131 | 439.3 | 1.50 | - | - | 16.0 | 3.4..4.3 |
| B1 | 3978.4 | 125.7 | 0.97 | 94..125 | 438.3 | 0.80 | 62.3 | 64.6 | 18.7 | 3.4..4.7 |
| B2 | 3916.6 | 127.7 | 1.00 | 97..129 | 449.2 | 0.90 | 65.0 | 77.3 | 20.6 | 3.6..6.9 |
| A2 | 3706.1 | 134.9 | 0.74 | 98..133 | 467.2 | 1.25 | - | - | 15.2 | 4.6..4.9 |

Mittel A 3592.3, B 3947.5 URLs/s (+9.9 %)
RSS A 453.2 MB, B 443.8 MB (-9.5 MB)
Abschnitt 10 / schnellster: A 0.74, B 0.98
Stillstand: A 1.37 s, B 0.85 s
Endbatch: A 15.6 s, B 19.7 s (nur Schreibzeit des längsten Batches)

Einfluss der Sortierer-Threads (`raw/thr`: A = ohne, B = mit vier Threads) und des Standes mit vier Threads gegen `origin/main` (`raw/thr2`); in `thr2` fiel der Linkindex auch bei A in den letzten Läufen von 16 auf 8 s, der Dateicache war warm:

| Lauf | Mittel URLs/s | Dauer s | Abschnitt 10 / schnellster | min..max % Mittel | RSS MB | Stillstand s | Commit erster Eimer ms | Commit letzter voller Eimer ms | Endbatch s | Load min..max |
|---|---|---|---|---|---|---|---|---|---|---|
| A1 | 4115.2 | 121.5 | 0.98 | 94..126 | 546.0 | 0.80 | 68.5 | 69.2 | 18.2 | 4.5..6.3 |
| B1 | 4254.2 | 117.5 | 0.95 | 90..123 | 469.7 | 0.70 | 62.1 | 70.6 | 14.3 | 3.5..4.1 |
| B2 | 4257.2 | 117.4 | 0.97 | 90..122 | 459.9 | 0.80 | 68.2 | 70.5 | 13.4 | 4.0..6.1 |
| A2 | 4237.2 | 118.0 | 0.95 | 94..125 | 444.7 | 0.70 | 64.6 | 63.9 | 17.1 | 3.5..5.5 |

Mittel A 4176.2, B 4255.7 URLs/s (+1.9 %)
RSS A 495.3 MB, B 464.8 MB (-30.5 MB)
Abschnitt 10 / schnellster: A 0.96, B 0.96
Stillstand: A 0.75 s, B 0.75 s
Endbatch: A 17.7 s, B 13.9 s (nur Schreibzeit des längsten Batches)

| Lauf | Mittel URLs/s | Dauer s | Abschnitt 10 / schnellster | min..max % Mittel | RSS MB | Stillstand s | Commit erster Eimer ms | Commit letzter voller Eimer ms | Endbatch s | Load min..max |
|---|---|---|---|---|---|---|---|---|---|---|
| A1 | 3564.1 | 140.3 | 0.74 | 98..133 | 498.8 | 1.41 | - | - | 16.2 | 2.8..5.6 |
| B1 | 4310.0 | 116.0 | 0.97 | 89..121 | 456.7 | 0.70 | 68.3 | 68.5 | 13.1 | 2.8..4.3 |
| B2 | 4603.2 | 108.6 | 0.98 | 84..113 | 477.5 | 0.80 | 65.3 | 68.9 | 6.22 | 3.2..4.0 |
| A2 | 3843.0 | 130.1 | 0.78 | 92..123 | 440.8 | 1.16 | - | - | 8.18 | 4.0..6.2 |

Mittel A 3703.6, B 4456.6 URLs/s (+20.3 %)
RSS A 469.8 MB, B 467.1 MB (-2.7 MB)
Abschnitt 10 / schnellster: A 0.76, B 0.97
Stillstand: A 1.29 s, B 0.75 s
Endbatch: A 12.2 s, B 9.6 s (nur Schreibzeit des längsten Batches)

Ziele auf dem Branch (Mittel beider B-Läufe gegen beide A-Läufe, Abgabestand): Commit-Dauer flach (68 ms gegen 72 ms, Faktor 0,94 bis 1,0; Grenze 1,3) erfüllt; Abschnitt 10 mindestens 90 % des schnellsten (0,94 und 0,96) erfüllt; Mittel nicht unter A - 5 % (+17,9 %) erfüllt; Spitzen-RSS nicht über A + 10 MB (+8,1 MB im Mittel von je zwei Läufen; einzelne Läufe beider Binärdateien springen um 40 bis 90 MB) erfüllt mit der genannten Streuung; längster Stillstand höchstens 10 s (0,9 s) erfüllt; Endbatch hochgerechnet auf 1 Mio. höchstens 60 s (13,6 s x 2 = 27 s, gemessen 29 bis 32 s) erfüllt.

## 5 Abschlussprobe (1 Mio., Abgabestand)

**G1: Abgabestand** (`final3/perf001/01-G1`): Mittel 3732.0 URLs/s, Dauer 268.0 s, Spitzen-RSS 474.4 MB, längster Stillstand 1.03 s, Datei 6276 MB, Load 3.9..5.2.

| Abschnitt | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| URLs/s | 3236 | 4372 | 4395 | 4246 | 4451 | 4457 | 4418 | 4338 | 4440 | 4408 |
| % des Mittels | 87 | 117 | 118 | 114 | 119 | 119 | 118 | 116 | 119 | 118 |

| Eimer (je 100 Batches) | 0 | 1 | 2 | 3 | 4 | 5 |
|---|---:|---:|---:|---:|---:|---:|
| Commit ms/Batch | 125 | 113 | 104 | 108 | 106 | 355 |
| Aufbau ms/Batch | 267 | 209 | 211 | 209 | 211 | 216 |
| davon urls aktualisieren | 19 | 20 | 20 | 20 | 20 | 21 |
| davon links | 60 | 64 | 66 | 66 | 66 | 68 |
| davon url_counters | 105 | 114 | 114 | 113 | 114 | 116 |
| Batches | 100 | 100 | 100 | 100 | 100 | 14 |

Endbatch (längster Schreibaufruf `engine_w_write_max_ns`): 31.5 s, davon Index `links_target` 22.3 s, fünf Indizes von `urls` 5.6 s (längster 3.7 s; Vorwärmen: 2.3 s längster Leser). Violations: []

**G2: Abgabestand** (`final3/perf001/02-G2`): Mittel 3844.4 URLs/s, Dauer 260.1 s, Spitzen-RSS 562.5 MB, längster Stillstand 1.00 s, Datei 6276 MB, Load 2.9..4.4.

| Abschnitt | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| URLs/s | 3317 | 4450 | 4467 | 4378 | 4543 | 4558 | 4577 | 4454 | 4474 | 4474 |
| % des Mittels | 86 | 116 | 116 | 114 | 118 | 119 | 119 | 116 | 116 | 116 |

| Eimer (je 100 Batches) | 0 | 1 | 2 | 3 | 4 | 5 |
|---|---:|---:|---:|---:|---:|---:|
| Commit ms/Batch | 116 | 108 | 107 | 94 | 112 | 218 |
| Aufbau ms/Batch | 259 | 208 | 207 | 203 | 209 | 216 |
| davon urls aktualisieren | 18 | 20 | 19 | 19 | 20 | 21 |
| davon links | 57 | 63 | 65 | 64 | 65 | 68 |
| davon url_counters | 101 | 114 | 112 | 110 | 113 | 116 |
| Batches | 100 | 100 | 100 | 100 | 100 | 21 |

Endbatch (längster Schreibaufruf `engine_w_write_max_ns`): 28.6 s, davon Index `links_target` 21.1 s, fünf Indizes von `urls` 4.6 s (längster 3.0 s; Vorwärmen: 2.3 s längster Leser). Violations: []

`perf-005 --size large --kills 10` (`raw/final3/perf005`): bestanden, wiederholte Requests je Abbruch [2264, 1801, 1988, 2264, 2264, 2264, 2264, 2264, 2264, 1108], Maximum 2264 bei Grenze 2264 (K-49), 0 doppelte URLs, `crawler_violations` leer. `writer_1m_uniform_segments` (Release, `--ignored`, `raw/final3/bench`): writer_1m_uniform_segments: 1000000 Seiten x 30 Links; URLs anlegen 6.1 s; Seiten schreiben 139.2 s (Wanduhr 141.7 s), 7182 Seiten/s; Abschnitt min/max zum Mittel 99 % / 101 %; groesster Commit 0.45 s; groesster Checkpoint 0.00 s; Endbatch (Indexaufbau) 11.7 s; finish 0.7 s; Datei 3335 MiB; Spitzen-RSS 373 MiB

## 6 Endbatch und Streuung

Der Endbatch bei 1 Mio. Seiten besteht aus dem Index `links_target` (22 s mit zwei Sortierer-Threads, 33 s ohne; zu rund 60 % Lesen der 30 Mio. Zeilen von `links`, nach `sample`-Proben des Schreib-Threads) und den fünf Indizes von `urls` (4 bis 6 s bei warmem Dateicache). Weil `crawler_urls_per_s` den Endbatch enthält (K-80), senkt jede Sekunde hier das Mittel und hebt die gleichmäßigen Abschnitte relativ dazu: bei 4 400 URLs/s und Mittel 3 800 liegen sie bei 116 % bis 119 %, die Grenze 130 % entspricht einem Mittel von 3 400 URLs/s, also einem Endbatch von höchstens rund 55 s.

Frühere Fassungen und der gestörte Lauf, zur Einordnung der Streuung:

**M2: erste Fassung (Indizes immer entfernt, ohne Vorwärmen, ohne Sortierer-Threads)** (`m2-1mio-ohne-vorwaermen/01-B1`): Mittel 2746.5 URLs/s, Dauer 364.1 s, Spitzen-RSS 470.3 MB, längster Stillstand 1.72 s, Datei 6278 MB, Load 4.4..15.2.

| Abschnitt | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| URLs/s | 3253 | 3890 | 3995 | 2505 | 4027 | 3135 | 3236 | 4160 | 4136 | 4366 |
| % des Mittels | 118 | 142 | 145 | 91 | 147 | 114 | 118 | 151 | 151 | 159 |

| Eimer (je 100 Batches) | 0 | 1 | 2 | 3 | 4 | 5 |
|---|---:|---:|---:|---:|---:|---:|
| Commit ms/Batch | 134 | 164 | 113 | 140 | 109 | 223 |
| Aufbau ms/Batch | 255 | 292 | 233 | 283 | 215 | 216 |
| davon urls aktualisieren | 18 | 27 | 22 | 27 | 21 | 22 |
| davon links | 57 | 81 | 71 | 82 | 66 | 67 |
| davon url_counters | 101 | 169 | 128 | 159 | 118 | 116 |
| Batches | 100 | 100 | 100 | 100 | 100 | 29 |

Endbatch (längster Schreibaufruf `engine_w_write_max_ns`): 83.3 s, davon Index `links_target` 33.2 s, fünf Indizes von `urls` 46.4 s (längster 0.0 s; Vorwärmen: 0.0 s längster Leser). Violations: ['PERF-002: Abschnitt 2 (10 % bis 20 %) mit 3891 URLs/s = 142 % des Mittelwerts 2747 URLs/s, erlaubt 70 % bis 130 %', 'PERF-002: Abschnitt 3 (20 % bis 30 %) mit 3995 URLs/s = 145 % des Mittelwerts 2747 URLs/s, erlaubt 70 % bis 130 %', 'PERF-002: Abschnitt 5 (40 % bis 50 %) mit 4027 URLs/s = 147 % des Mittelwerts 2747 URLs/s, erlaubt 70 % bis 130 %', 'PERF-002: Abschnitt 8 (70 % bis 80 %) mit 4160 URLs/s = 151 % des Mittelwerts 2747 URLs/s, erlaubt 70 % bis 130 %', 'PERF-002: Abschnitt 9 (80 % bis 90 %) mit 4136 URLs/s = 151 % des Mittelwerts 2747 URLs/s, erlaubt 70 % bis 130 %', 'PERF-002: Abschnitt 10 (90 % bis 100 %) mit 4366 URLs/s = 159 % des Mittelwerts 2747 URLs/s, erlaubt 70 % bis 130 %']

**F1: Fassung mit Vorwärmen, ohne Sortierer-Threads (Ausreißer)** (`final/perf001/01-F1`): Mittel 3174.0 URLs/s, Dauer 315.1 s, Spitzen-RSS 467.4 MB, längster Stillstand 1.40 s, Datei 6277 MB, Load 2.8..6.4.

| Abschnitt | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| URLs/s | 3272 | 4387 | 4234 | 4442 | 4583 | 4536 | 4565 | 4441 | 4520 | 4544 |
| % des Mittels | 103 | 138 | 133 | 140 | 144 | 143 | 144 | 140 | 142 | 143 |

| Eimer (je 100 Batches) | 0 | 1 | 2 | 3 | 4 | 5 |
|---|---:|---:|---:|---:|---:|---:|
| Commit ms/Batch | 128 | 119 | 106 | 103 | 109 | 273 |
| Aufbau ms/Batch | 259 | 208 | 206 | 204 | 208 | 196 |
| davon urls aktualisieren | 18 | 20 | 19 | 19 | 20 | 19 |
| davon links | 58 | 64 | 65 | 64 | 65 | 62 |
| davon url_counters | 102 | 114 | 111 | 110 | 113 | 106 |
| Batches | 100 | 100 | 100 | 100 | 100 | 24 |

Endbatch (längster Schreibaufruf `engine_w_write_max_ns`): 82.4 s, davon Index `links_target` 33.4 s, fünf Indizes von `urls` 44.6 s (längster 9.2 s; Vorwärmen: 2.3 s längster Leser). Violations: ['PERF-002: Abschnitt 2 (10 % bis 20 %) mit 4387 URLs/s = 138 % des Mittelwerts 3174 URLs/s, erlaubt 70 % bis 130 %', 'PERF-002: Abschnitt 3 (20 % bis 30 %) mit 4234 URLs/s = 133 % des Mittelwerts 3174 URLs/s, erlaubt 70 % bis 130 %', 'PERF-002: Abschnitt 4 (30 % bis 40 %) mit 4442 URLs/s = 140 % des Mittelwerts 3174 URLs/s, erlaubt 70 % bis 130 %', 'PERF-002: Abschnitt 5 (40 % bis 50 %) mit 4583 URLs/s = 144 % des Mittelwerts 3174 URLs/s, erlaubt 70 % bis 130 %', 'PERF-002: Abschnitt 6 (50 % bis 60 %) mit 4536 URLs/s = 143 % des Mittelwerts 3174 URLs/s, erlaubt 70 % bis 130 %', 'PERF-002: Abschnitt 7 (60 % bis 70 %) mit 4565 URLs/s = 144 % des Mittelwerts 3174 URLs/s, erlaubt 70 % bis 130 %', 'PERF-002: Abschnitt 8 (70 % bis 80 %) mit 4441 URLs/s = 140 % des Mittelwerts 3174 URLs/s, erlaubt 70 % bis 130 %', 'PERF-002: Abschnitt 9 (80 % bis 90 %) mit 4520 URLs/s = 142 % des Mittelwerts 3174 URLs/s, erlaubt 70 % bis 130 %', 'PERF-002: Abschnitt 10 (90 % bis 100 %) mit 4544 URLs/s = 143 % des Mittelwerts 3174 URLs/s, erlaubt 70 % bis 130 %']

**F3: dieselbe Fassung** (`final2/01-F3`): Mittel 3701.0 URLs/s, Dauer 270.2 s, Spitzen-RSS 469.4 MB, längster Stillstand 0.80 s, Datei 6277 MB, Load 2.3..5.1.

| Abschnitt | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| URLs/s | 3370 | 4584 | 4549 | 4513 | 4592 | 4634 | 4535 | 4510 | 4542 | 4590 |
| % des Mittels | 91 | 124 | 123 | 122 | 124 | 125 | 123 | 122 | 123 | 124 |

| Eimer (je 100 Batches) | 0 | 1 | 2 | 3 | 4 | 5 |
|---|---:|---:|---:|---:|---:|---:|
| Commit ms/Batch | 109 | 104 | 105 | 100 | 104 | 253 |
| Aufbau ms/Batch | 258 | 205 | 210 | 204 | 206 | 207 |
| davon urls aktualisieren | 18 | 19 | 20 | 19 | 20 | 20 |
| davon links | 58 | 63 | 66 | 64 | 65 | 66 |
| davon url_counters | 102 | 113 | 113 | 110 | 111 | 111 |
| Batches | 100 | 100 | 100 | 100 | 100 | 23 |

Endbatch (längster Schreibaufruf `engine_w_write_max_ns`): 42.4 s, davon Index `links_target` 33.4 s, fünf Indizes von `urls` 5.2 s (längster 2.9 s; Vorwärmen: 2.5 s längster Leser). Violations: []

**F4: dieselbe Fassung** (`final2/02-F4`): Mittel 3757.7 URLs/s, Dauer 266.1 s, Spitzen-RSS 475.2 MB, längster Stillstand 0.80 s, Datei 6277 MB, Load 3.1..4.4.

| Abschnitt | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| URLs/s | 3420 | 4531 | 4565 | 4520 | 4615 | 4639 | 4622 | 4537 | 4550 | 4462 |
| % des Mittels | 91 | 121 | 121 | 120 | 123 | 123 | 123 | 121 | 121 | 119 |

| Eimer (je 100 Batches) | 0 | 1 | 2 | 3 | 4 | 5 |
|---|---:|---:|---:|---:|---:|---:|
| Commit ms/Batch | 111 | 108 | 99 | 99 | 103 | 257 |
| Aufbau ms/Batch | 258 | 208 | 205 | 205 | 203 | 218 |
| davon urls aktualisieren | 18 | 19 | 19 | 19 | 19 | 21 |
| davon links | 58 | 64 | 64 | 64 | 64 | 69 |
| davon url_counters | 102 | 114 | 111 | 111 | 110 | 116 |
| Batches | 100 | 100 | 100 | 100 | 100 | 22 |

Endbatch (längster Schreibaufruf `engine_w_write_max_ns`): 38.7 s, davon Index `links_target` 31.0 s, fünf Indizes von `urls` 4.4 s (längster 2.1 s; Vorwärmen: 1.8 s längster Leser). Violations: []

`m1-1mio-gestoert` ist ein Lauf, während dessen ich `cargo test` ausführte (Load bis 37): Der Verlauf in der Mitte ist gestört und nicht auswertbar, der Endbatch (93 s, davon 54 s Indizes von `urls`) zeigt dieselbe Abhängigkeit vom Dateicache. Ein einzelner Indexaufbau braucht bei warmem Cache 1 bis 4 s und kalt rund 9 s (synchrone 4-KiB-Lesezugriffe auf die verstreuten Blätter von `urls`); die Läufe F1 und M2 liefen kurz nach einem Bau bzw. mit Fremdlast, vermutlich hat das den Dateicache verdrängt (nicht belegt). Das Vorwärmen ändert das Muster nicht: Auch in F1 las es alle 1 000 001 Zeilen in 2,3 s, die Aufbauten brauchten danach trotzdem 9 s je Index. Ursache offen; Vorschlag für den Planer: bei der Abnahme A4 den Endbatch (`engine_w_write_max_ns`, `store_url_index_build_ns`) im Bericht prüfen und bei Abweichung über 60 s wiederholen.

## 7 Befunde und Vorschläge an den Planer

1. **Filter „Duplicate“ auf laufender großer Datei.** Die fünf Filter der Tabs Page Titles, Meta Description, H1, H2 und Content/Exact Duplicates (`os-analyze`, korreliertes `EXISTS` auf der Spalte) brauchen die Indizes; ohne sie sind sie korrekt, aber quadratisch (20 000 Zeilen 6,8 s, `SCAN d` im Plan, mit Index 6 ms). In einer laufenden oder pausierten Datei über 250 000 URLs sind diese fünf Filter deshalb nicht benutzbar, nach dem Crawl-Ende (und in Analyse und Exporten über `ensure_if_idle`) schon. Vorschlag K-83 (nicht in `os-store`/`os-engine`, M2-Oberfläche): die Filter mit einem `GROUP BY ... HAVING COUNT(*) > 1` in einem Lauf statt mit korreliertem `EXISTS` formulieren.
2. **Fixtures `tests/fixtures/crawl-files`** sind unverändert und müssen es nicht werden: Alle Fixtures haben weniger als 250 000 URLs und behalten ihre Indizes; die Generator-Prüfung `committed_fixtures_match_the_generator` ist grün. Sie bilden nur keinen großen, pausierten Crawl ab (ohne die fünf Indizes und ohne `links_target`).
3. **`os-loadtest`** ist unverändert (die Zähler erscheinen im `diag`-Block). Die Berichte der Abnahme A4 sollten den Endbatch ausdrücklich nennen (`engine_w_write_max_ns`).
4. K-79 (Schemaänderung, z. B. schmale Tabelle für die Zählerzeile oder Verzicht auf `urls_hash` als Index) wird nicht gebraucht: Die Ursache war mit den Mitteln aus K-80 zu beheben.

## 8 Stand und Aufwand

Branch `m1/m1-n22` (Basis `origin/main` `f8357b5`); Builds mit `--locked`, `CARGO_INCREMENTAL=0`, `RUSTC_WRAPPER=sccache`, Target `openspider-wt/target-m1-n22`; Konstanten unverändert (`CHANNEL_CAPACITY` 256, `BATCH_PAGES_BACKLOG` 2 000, 8 Threads); `Cargo.toml` und `Cargo.lock` unverändert. Prüfung des Abgabestands: `cargo fmt --all --check`, `cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings` ohne Befund, `cargo test --workspace --exclude openspider-desktop --locked --no-fail-fast`: 2 046 bestanden, 0 fehlgeschlagen, 36 ignoriert (N21: 2 037 bestanden; neu 9 Tests mit Spec-Tag PERF-002). Diagnoseschalter (Standard aus): `OS_STORE_WAL_EXPLICIT`, `OS_STORE_WAL_BOUNDS=min:max`, `OS_STORE_BACKGROUND_CHECKPOINT`, `OS_ENGINE_REFILL_NOFLUSH`, `OS_STORE_DROP_URL_INDEXES` (`1` immer, `0` nie).

## Dateien in diesem Ordner

* `raw/diag1`, `raw/exp1`: Ursachentrennung (Abschnitt 2); `raw/ab1`, `raw/ab2`, `raw/ab3`, `raw/thr`, `raw/thr2`: A/B-Proben (Abschnitt 4); `raw/final`, `raw/final2`, `raw/final3`, `raw/m2-1mio-ohne-vorwaermen`, `raw/m1-1mio-gestoert`, `raw/prof-p1`: 1-Mio.-Läufe und Abschlussprobe (Abschnitte 5 und 6). Jeder Lauf: `NN-<name>.json` (Bericht), `.log`, `.load`.
* `summarize.py` (Verlauf je 100 Batches aus einem Bericht), `ab_table.py` (Vergleichstabelle eines A/B-Ordners), `report_tables.py` (Tabellen dieses Berichts).
