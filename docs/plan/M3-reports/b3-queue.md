# B3-QUEUE – Writer-Kanal so groß wie ein Batch (beta.3)

Stand: 05.11.2026, Branch `m3/queue` (Basis `main` 9eb82f5d), Messungen auf dem Linux-Build-Server
(16 Kerne, 61 GB RAM) mit den Werkzeugen aus `~/perf` (`run.sh`, `phases.py`, `segments.py`, `cmp.py`) plus
`~/perf/q/diag.py` (Writer-Leerlauf und Worker-Blockade aus `OS_SCALE_DIAG`) und einem Lastschreiber
(`~/perf/q/load.log`, Last alle 10 s). Auftrag: V1 aus B3-PERF umsetzen, V2 nur messen, Budgets PERF-001/002/003/005
und die K-49-Grenze neu belegen. Nutzerentscheidung vom 04.11.2026 (beta.3).

## Ergebnis in Kürze

- **Gewählt:** `CHANNEL_CAPACITY` **1 024** (aus der Batchgröße abgeleitet statt fester Zahl),
  `IN_FLIGHT_BYTES` **24 MiB** (= 3 × `BATCH_BYTES`; Startwert war 32 MiB), `BATCH_BYTES` bleibt 8 MiB.
- **Wirkung (Server, CLI-Methode wie B3-PERF):** 100k 78,0 s → 54,0 s gesamt (Median aus 3, −31 %), 1 Mio. 986 s →
  712 / 699 s (zwei Läufe, −28 %); Lasttest PERF-002 (8 Threads) 1 218 → 1 584 URLs/s. Writer-Leerlauf bei 1 Mio.
  246 s → 0,6 / 0,4 s, Worker-Blockade je Worker 396 s (44 % der Crawlzeit) → 163 / 149 s (26 / 24 %), davon am Kanal
  nur 32 / 25 s; der Rest ist Gegendruck des jetzt durchgehend schreibenden Writers.
- **Budgets:** PERF-001, PERF-002, PERF-003 und PERF-005 grün (Werte unten). PERF-003 (Slow-Sink) steigt auf Linux von
  416 auf 439 MB; Abstand zur Grenze 500 MB kleiner, aber vorhanden – auf dem Mac nicht gemessen (**Auflage vor dem
  beta.3-Tag**, siehe Entscheidungen und Auflagen).
- **K-49:** Formel unverändert (Threads + `CHANNEL_CAPACITY` + `BATCH_PAGES_BACKLOG`), die Zahl bei 8 Threads steigt
  von 4 264 auf **5 032** Seiten je Abbruch. Der Orchestrator nimmt 5 032 an (Fable-Empfehlung; Plan-Entscheidung,
  keine Spec-Änderung).
- **V2** (`BATCH_BYTES` 16 MiB) **nicht übernommen:** bei 100k kein Gewinn (Median 58,6 bzw. 54,0 s gegen 52,5–53,5 s),
  bei 1 Mio. −37 s gegenüber V1 in derselben Reihe; Bedingung „Gewinn bei 100k, 3 Läufe“ nicht erfüllt.

## Commits (`m3/queue`, nicht gepusht)

| Commit | Inhalt |
|---|---|
| 27bd4bf2 | `engine:` Kanal aus der Batchgröße abgeleitet (1 024), Zulassungsschwelle 3 × `BATCH_BYTES` (24 MiB), const-Prüfungen, Test gegen Fixture-Seiten, K-49-Kommentar mit neuer Zahl |
| 4059a939 | `docs:` Bericht B3-QUEUE |
| 3a07904c | `engine:` Fable-Nacharbeit: `MESSAGES_PER_PAGE` als Obergrenze beschrieben, const-Assert `>=` (Zweierpotenz-Grenzfall), Herleitungstest deckt ihn ab, Achtel-Regel des Rückstaus begründet |
| (diese Ergänzung) | `docs:` Bericht: Fable-Nacharbeit, Entscheidungen, Folgeauftrag Render-Nutzdaten |

## Gewählte Werte und Herleitung

**Befund (B3-PERF, hier bestätigt):** Je Seite schicken die Worker bis zu zwei Nachrichten in den Kanal
(Discovery-Nachricht mit neuen URLs/Warteschlange vor der Seite, `worker.rs`). Ein Batch der PERF-001-Fixture endet am
Nutzdatenbudget `BATCH_BYTES` = 8 MiB bei rund 464 Seiten (1 Mio.: 2 155 Batches; 100k: 459 Seiten je Batch), also
≈ 900 Nachrichten. Der alte Kanal mit 256 Nachrichten fasste nur ≈ 130 Seiten. Während der Writer schrieb, lief der
Kanal voll und die Worker blockierten; danach wartete der Writer bis zur Sammelzeit (500 ms) auf den Rest des Batches.

**Kanal aus der Batchgröße abgeleitet** (`crates/os-engine/src/writer_thread.rs`):

```rust
pub const MESSAGES_PER_PAGE: usize = 2;
pub const REFERENCE_PAGE_BYTES: u64 = 18 * 1024; // gemessen: 8 MiB / 464 Seiten ≈ 18 080 Byte
pub const CHANNEL_CAPACITY: usize =
    (MESSAGES_PER_PAGE * (BATCH_BYTES / REFERENCE_PAGE_BYTES) as usize).next_power_of_two(); // = 1 024
pub const IN_FLIGHT_BYTES: u64 = 3 * BATCH_BYTES; // = 24 MiB
```

Begründung: Der Kanal soll den nächsten vollen Batch aufnehmen, während der Writer den aktuellen schreibt; dann findet
der Writer nach jedem Commit einen vollen Batch vor. 8 MiB / 18 KiB = 455 Seiten × 2 Nachrichten = 910 → nächste
Zweierpotenz 1 024 (dasselbe Ergebnis für jede Referenzseite von 16 KiB bis unter 32 KiB; bei genau 16 KiB ist das
Produkt 2 × 512 = 1 024 selbst die Zweierpotenz, der Kanal fasst den Batch dann genau – deshalb prüft die
const-Assertion `>=`, Fable-Befund). Ändert jemand `BATCH_BYTES`, wächst der
Kanal mit (16 MiB → 2 048), statt still wieder kleiner als ein Batch zu sein. Feste Zahl 1 024 wäre gleichwertig, die
Herleitung macht die Abhängigkeit aber sichtbar und prüfbar.

**Zulassungsschwelle drei Batches statt 32 MiB:** Neben dem Batch in der Senke (8 MiB) muss der volle Kanal aus
Referenzseiten (512 Seiten × 18 KiB = 9 MiB) unter die Schwelle passen, dazu die Abrufe und Auswertungen der aktiven
Worker. 16 MiB (heute, zwei Batches) griffen, bevor der Kanal einen Batch hielt (100k: 56,6 s, Worker warten 4,9 s je
Worker am Byte-Gate). 24 und 32 MiB sind gleich schnell (100k 53,5 gegen 52,5 s, 1 Mio. 692 gegen 696 s), 24 MiB puffert
weniger (Slow-Sink-RSS 445 gegen 448 MB, bei schweren realen Seiten begrenzt allein die Byte-Schwelle den Puffer).

**Prüfungen:**
- Übersetzungszeit (`const _: () = { assert!(…) }`): Kanal ≥ Nachrichten eines vollen Batches, Kanal ≤ das Doppelte
  (Kanalanteil der K-49-Grenze bleibt begrenzt), schreibender Batch + voller Kanal aus Referenzseiten < Schwelle.
- Neuer Test `the_channel_holds_the_next_full_batch_of_reference_pages_while_the_writer_writes`
  (`writer_thread.rs`, `// spec: PERF-002, PERF-003`) gegen Fixture-Seiten: Die Senke hält einen vollen 8-MiB-Batch
  fest; derweil legt der Test 455 Referenzseiten im ungünstigsten Fall (je Discovery + Seite = 18 KiB) und eine
  überzählige Seite ohne Discovery mit `try_reserve` in den Kanal – keine Reservierung darf warten (Kanal und
  Byte-Gate). Nach der Freigabe schreibt der Writer genau 455 Seiten am Nutzdatenbudget, ohne auf die Sammelzeit zu
  warten (Batches `[1, 455, 1]`). Im Grenzfall „Kanal genau voll“ (Referenzseite 16 KiB) folgt die überzählige Seite
  nach der Freigabe; mit 16 KiB testweise übersetzt: grün (`[1, 512, 1]`). Mit den alten Konstanten schlägt der Test
  fehl (geprüft, siehe Tests: Kanal nach 128 Seiten × 2 Nachrichten voll).

## Vorher/Nachher

Methode wie B3-PERF: `openspider --headless --crawl <URL> --export-tabs "Internal:All" --save-crawl --output-folder <dir>
--json-progress` (Standard, 5 Threads), Site `os-testsite synthetic:n=100000,links=30` bzw. `n=1000000`, alle Läufe
mit `OS_SCALE_DIAG=1`. Phasen nach `phases.py`: Crawl = bis Beginn Analyse (einschließlich Endbatch), Analyse, Rest =
Export + Speichern. RSS = Spitze aus `/usr/bin/time -v` (über alle Phasen). Writer-Leerlauf = `w_idle_ns` +
`w_poll_sleep_ns`; Worker-Blockade = (`reserve_ns` + `send_ns` + `byte_wait_ns`) / 5 Worker. „vorher“ = `main` 9eb82f5d
(`openspider-q0`), „nachher“ = Commit 27bd4bf2 (`openspider-q1`). Varianten mit einer **nicht committeten**
Diagnosekopie (`openspider-qd`, Konstanten per `OSD_*` überschreibbar, Skript `~/perf/diagpatch.py` aus B3-PERF).
Gestört = fremde Last während des Laufs (Lastschreiber; eigene Last 100k ≤ 3, 1 Mio. ≤ 6,5).

### 100k (je 3 Läufe, Median)

| Stand | Läufe gesamt (s) | Median gesamt | Crawl | Analyse | Export + Speichern | RSS (Median) | Last während Lauf |
|---|---|---|---|---|---|---|---|
| vorher `main` 9eb82f5d (q0) | 78,0 / 77,9 / 78,0 | **78,0 s** | 70,1 s | 2,7 s | 5,2 s | 672 MB | ruhig¹ |
| **nachher 27bd4bf2 (q1)** | 55,2² / 54,0 / 52,3 | **54,0 s** | 46,3 s | 2,8 s | 5,1 s | 657 MB | Mittel/Max 4,0/4,9², 1,4/2,0, 2,2/2,9 |
| Diagnose: nur Kanal 1 024, Schwelle 16 MiB | 56,6 / 56,5 / 58,0 | 56,6 s | 48,8 s | 2,8 s | 5,1 s | 657 MB | ruhig¹ |
| Diagnose: Kanal 1 024, 24 MiB (= Endstand) | 53,3 / 54,9 / 53,5 | 53,5 s | 45,7 s | 2,7 s | 5,1 s | 678 MB | ruhig¹ |
| Diagnose: Kanal 1 024, 32 MiB (Startwert) | 53,9 / 52,5 / 52,5 | 52,5 s | 44,8 s | 2,8 s | 5,1 s | 680 MB | ruhig¹ |

¹ Diagnosereihe 04.11. 21:32–22:05 vor Start des Lastschreibers: Last zu Beginn 1,0–2,0, am Ende 2,1–3,1 (eigene Last
des Crawls), keine fremden Prozesse beobachtet; Varianten abwechselnd gelaufen.
² Lauf 1 leicht gestört (Lastmittel 4,0 statt 1,4–2,2; Writer-Schreibzeit 44,8 statt 41,4–41,7 s). Ohne ihn: Median
53,2 s. Eine erste Nachher-Reihe um 00:00 lief in fremde Workspace-Builds (Last bis 10; 54,8 / 53,4 / 60,9 s) und ist
verworfen.

### 1 Mio.

| Stand | gesamt | Crawl | Analyse | Export + Speichern | RSS (Spitze, ganzer Lauf)³ | Last Mittel/Max |
|---|---|---|---|---|---|---|
| vorher q0 | 986,3 s | 918,3 s | 23,3 s | 44,7 s | 897 MB | ruhig (Endlast 1,9) |
| **nachher q1, Lauf 1** | **712,4 s** | 641,0 s | 25,7 s | 45,7 s | 1 111 MB | 3,5 / 4,3 |
| **nachher q1, Lauf 2** | **698,8 s** | 629,7 s | 23,7 s | 45,4 s | 875 MB | 3,6 / 4,6 |
| verworfen: q1, gestört | 721,8 s | 651,3 s | 23,9 s | 46,6 s | 999 MB | 6,4 / 15,2 |
| verworfen: q1, abgebrochen | – | – | – | – | – | Platte zeitweise voll durch fremde Läufe, Crawl bei 475 000 URLs pausiert (Plattenwächter) |
| Diagnose: Kanal 1 024, 24 MiB | 692,2 s | 623,6 s | 24,1 s | 44,5 s | 1 009 MB | ruhig |
| Diagnose: Kanal 1 024, 32 MiB | 695,9 s | 627,5 s | 23,5 s | 44,9 s | 893 MB | ruhig |
| Diagnose: V2 (16 MiB, Kanal 1 024, 48 MiB) | 655,7 s | 587,1 s | 24,0 s | 44,6 s | 922 MB | ruhig |

Durchsatz je 10 % (CLI, 5 Threads; PERF-002-Methode, nur Bericht – die Abnahme steht unter Budgets):
vorher 894 · 1 134 · 1 152 · 1 146 · 1 136 · 1 132 · 1 137 · 1 136 · 1 131 · 1 140 URLs/s;
nachher Lauf 1 1 284 · 1 673 · 1 680 · 1 643 · 1 663 · 1 633 · 1 620 · 1 644 · 1 643 · 1 612,
Lauf 2 1 372 · 1 702 · 1 706 · 1 681 · 1 664 · 1 660 · 1 670 · 1 596 · 1 614 · 1 684.

³ **RSS-Verlauf** (je ein zusätzlicher 1-Mio.-Lauf mit VmRSS je Sekunde, `~/perf/q/rssrun.sh`): vorher Crawl 404 MB,
Analyse 831 MB, Export 640 MB; nachher Crawl 427 MB, Analyse 880 MB, Export 628 MB. Die Spitze des CLI-Laufs entsteht
in der Analyse (Link-Score-Graph, Links-Schritt), nicht im Crawl; die Writer-Änderung wirkt nur im Crawl (+23 MB, wie
im Lasttest). Die Analysespitze streut zwischen Läufen um bis zu ±120 MB (875–1 111 MB bei gleichem Code).

### Writer-Leerlauf und Worker-Blockade

| `OS_SCALE_DIAG` (5 Worker) | 100k vorher (Median) | 100k nachher (Median) | 1 Mio. vorher | 1 Mio. nachher (Lauf 1 / 2) |
|---|---|---|---|---|
| Crawlende (letzte URL) | ≈ 69 s | ≈ 45 s | 901,1 s | 624,1 / 613,1 s |
| Writer schreibt (`w_write_ns`) | 44,8 s | 41,7 s | 666,8 s | 635,6 / 624,6 s |
| **Writer-Leerlauf** (`w_idle_ns` + `w_poll_sleep_ns`) | 24,0 s | **1,9 s** | 245,7 s | **0,6 / 0,4 s** |
| Worker wartet auf Platz im Kanal (`reserve` + `send`) je Worker | 18,6 s | 1,0 s | 395,7 s | 31,6 / 24,9 s |
| Worker wartet an der Byte-Schwelle (`byte_wait`) je Worker | 0,0 s | 3,4 s | 0,1 s | 131,3 / 124,2 s |
| **Worker-Blockade gesamt** je Worker (Anteil am Crawl) | 18,6 s (27 %) | **4,3 s (9 %)** | 395,8 s (44 %) | **162,9 / 149,1 s (26 / 24 %)** |
| größte Kanalbelegung (Nachrichten) | 256 | 1 024 | 256 | 1 024 |
| größter Puffer (Byte-Bilanz) | 12,9 MB | 25,2 MB | 13,0 MB | 25,3 MB |
| Seiten je Batch | 459 | 459 | 464 | 464 |

Der Writer ist jetzt durchgehend beschäftigt (1 Mio.: Schreibzeit ≈ Crawldauer); der Crawl dauert so lange wie die
Schreibarbeit. Die verbleibende Wartezeit der Worker ist gewollter Gegendruck (PERF-003): Der Writer ist bei 1 Mio. der
Engpass, weil jeder Commit mit N teurer wird (Ursache 1 aus B3-PERF, Vorschlag V3). Sie liegt jetzt überwiegend an der
Byte-Schwelle statt am Kanal; mit 32 MiB lag sie am Kanal (121 s je Worker), bei gleicher Gesamtzeit.

Gleichheit: `internal_all.csv` (100k) vorher gegen jede Variante (`cmp.py`, ohne `Response Time`/`Crawl Timestamp`):
100 000 Zeilen, alle Werte gleich (q0 gegen c1k24, c1k32, v2 sowie q0 gegen den Endstand q1, Lauf 3; die übrigen Exporte wurden von den gleichnamigen 1-Mio.-Läufen überschrieben).

## Budgets

Alle Lasttests mit `os-loadtest run --size large --threads 8 --profile balanced` (Release, Server, `--work-dir` auf
der Platte), Ruhefenster 3 min (Last < 3) vor jedem Lauf, Lastschreiber während des Laufs. „vorher“: für PERF-001
`os-loadtest` aus `main` 9eb82f5d, für PERF-003 die Diagnosekopie ohne Überschreibung (Konstanten wie `main`).

### PERF-001 – Speicher (`perf-001 --size large`)

| | vorher | nachher | Grenze |
|---|---|---|---|
| Spitzen-RSS 1 Mio. | 412,7 MB | **435,3 MB** | < 1,5 GB ✓ |
| Spitzen-RSS Referenz 100k | 375,8 MB | 404,8 MB | – |
| Verhältnis 1 Mio. / 100k | 1,098 | **1,075** | < 2 ✓ |
| Crawl-Dauer | 821,2 s | 631,3 s | – |
| längster Stillstand | 1,06 s | 0,95 s | < 30 s ✓ (PERF-003, gilt auch hier) |
| Endbatch | 14,5 s | 17,7 s | K-80 ≤ 60 s ✓ |
| Ergebnis | `passed`, keine Verletzung | **`passed`, keine Verletzung** | |

`perf-001 --size small` (10 000 URLs) nachher: `passed`, 187,8 MB, Zuwachs gegenüber Referenz 135,9 MB bei Grenze
180,7 MB (Regel `cache-bounded-growth`). CLI-Läufe (5 Threads, mit Analyse und Export): 1 Mio. 875–1 111 MB < 1,5 GB.

### PERF-002 – Durchsatz (aus demselben 1-Mio.-Lauf, 8 Threads)

| | Mittel | zehn Abschnitte (URLs/s) | Abweichung vom Mittel |
|---|---|---|---|
| vorher | 1 217,8 URLs/s | 1 003,8 · 1 278,0 · 1 289,3 · 1 272,4 · 1 284,1 · 1 276,3 · 1 271,5 · 1 262,8 · 1 267,2 · 1 272,0 | −17,6 … +5,9 % |
| **nachher** | **1 584,1 URLs/s** (+30 %) | **1 365,5 · 1 701,9 · 1 701,5 · 1 702,1 · 1 659,9 · 1 683,6 · 1 642,3 · 1 625,5 · 1 661,1 · 1 659,4** | **−13,8 … +7,4 %** |

Kriterien: Mittel ≥ 500 URLs/s ✓; jeder Abschnitt zwischen 70 und 130 % des Mittels (1 108,9 … 2 059,3 URLs/s) ✓.
Der erste Abschnitt bleibt der langsamste (Einfügen neuer URLs, Auslagern der Warteschlange; V6 in B3-PERF).

### PERF-003 – Gegendruck (`perf-003 --size large`, SlowSink 100 ms je Batch)

| | vorher | Diagnose 1 024 / 24 MiB | Diagnose 1 024 / 32 MiB | **nachher q1** | Grenze |
|---|---|---|---|---|---|
| Spitzen-RSS | 415,7 MB | 445,0 MB | 448,0 MB | **438,9 MB** | < 500 MB ✓ |
| längster Stillstand | 1,19 s | 1,06 s | 1,10 s | **1,10 s** | ≤ 30 s ✓ |
| Ende | `queue_empty` | `queue_empty` | `queue_empty` | **`queue_empty`**, 1 000 000 URLs | regulär ✓ |
| Durchsatz | 973,0 URLs/s | 1 165,3 | 1 153,2 | 1 156,8 URLs/s | – |
| größter Puffer (Byte-Bilanz) | 13,0 MB | 25,3 MB | 26,5 MB | 25,3 MB | – |

Abschnitte nachher (nur Bericht): 982,1 · 1 213,0 · 1 212,1 · 1 224,8 · 1 213,6 · 1 205,8 · 1 196,4 · 1 189,3 ·
1 192,6 · 1 199,6 URLs/s. Der Puffer wächst um 12 MB, die Spitze um 23–32 MB (Vektorwachstum, Allocator). Abstand zur
Grenze auf Linux 61 MB (vorher 84 MB). Mac: Auflage, siehe Entscheidungen und Auflagen.

### PERF-005 – harter Abbruch und Fortsetzen (`perf-005 --size large --kills 10`, Seed 20261014)

`passed`: 100 000 Seiten, 10 von 10 Abbrüchen (bei 14 177 … 84 155 URLs), nach jedem `integrity_check = ok`, 0 doppelte
URLs, 0 fehlende/zusätzliche URLs, Status je URL wie der Referenzcrawl. Wiederholte Requests je Abbruch
**[1 490, 403, 1 073, 1 327, 1 210, 1 012, 1 021, 1 302, 1 492, 1 083]**, Maximum 1 492, Mittel 1 141,3, Summe 11 413;
Grenze je Abbruch (K-49, aus den Konstanten) **5 032**, gesamt 50 320; 0 Wiederholungen vor dem ersten Abbruch.
Vergleich vorher (M2-Q03, 30.10., 5 Abbrüche): Maximum 2 971 bei Grenze 4 264. Der neue Höchstwert liegt auch unter der
alten Zahl: Bei Fixture-Seiten endet der Batch am Nutzdatenbudget (≈ 464 Seiten), real begrenzen also
Threads + Kanal + ≈ 464.

## K-49: Grenze ungeschriebener Seiten beim harten Abbruch (PERF-005)

**Herleitung.** Was beim harten Abbruch abgerufen, aber noch nicht in der Datei ist, liegt an genau drei Stellen:
1. in einem Worker zwischen Abruf und Senden (höchstens eine Seite je Thread),
2. im Kanal: eine Worker-Nachricht trägt höchstens eine Seite, also höchstens `CHANNEL_CAPACITY` Seiten,
3. im Writer: entweder der sich füllende oder der gerade geschriebene Batch (nie beide gleichzeitig offen), höchstens
   `BATCH_PAGES_BACKLOG` Seiten (der Writer schneidet, sobald das Seitenlimit erreicht ist; bei Seiten der
   PERF-001/005-Fixture endet der Batch schon vorher am Nutzdatenbudget, bei ≈ 464 Seiten).

Grenze je Abbruch = Threads + `CHANNEL_CAPACITY` + `BATCH_PAGES_BACKLOG` – die K-49-Formel ist unverändert. Der
Lasttest (`os-loadtest`, `REPEAT_BUFFER_ALLOWANCE`) und der Adaptertest (`engine_adapter.rs`, `// spec: PERF-005`)
leiten sie aus den Konstanten ab; dort war nichts zu ändern. Neu ist nur die Zahl:

| | Threads | Kanal | größter Batch | Grenze je Abbruch |
|---|---|---|---|---|
| M1 (P28) | 8 | 256 | 1 000 | 1 264 |
| M1-Abnahme | 8 | 256 | 2 000 | 2 264 |
| M2-E07 bis beta.2 | 8 | 256 | 4 000 | 4 264 |
| **B3-QUEUE** | 8 | **1 024** | 4 000 | **5 032** |

Die Garantie selbst ist nicht schwächer: „keine Duplikate, keine Verluste“ (PERF-005) bleibt, und wiederholt werden nur
nicht persistierte Abrufe. Größer wird nur die Zahl der Seiten, die nach einem harten Abbruch erneut abgerufen werden
dürfen (+768). Kommentare in `writer_thread.rs` nennen jetzt die Formel und beide Zahlen. Messung siehe Budgets
(PERF-005). Die neue Zahl hat der Orchestrator nach Fable-Empfehlung angenommen (Plan-Entscheidung, keine
Spec-Änderung); die Alternative `BATCH_PAGES_BACKLOG` 3 232 entfällt.

## V2 (`BATCH_BYTES` 16 MiB) – nur gemessen

| Variante | Kanal | Schwelle | 100k Median (3 Läufe) | 1 Mio. | Writer schreibt / wartet (1 Mio.) |
|---|---|---|---|---|---|
| V1 c1k24 (gewählt) | 1 024 | 24 MiB | 53,5 s (53,3 / 54,9 / 53,5) | 692,2 s | 618,8 / 0,4 s |
| V1 c1k32 (Startwert) | 1 024 | 32 MiB | 52,5 s (53,9 / 52,5 / 52,5) | 695,9 s | 622,2 / 0,5 s |
| V2 wie B3-PERF | 1 024 | 48 MiB | 58,6 s (58,6 / 59,3 / 57,4) | 655,7 s | 535,9 / 42,9 s |
| V2 abgeleitet | 2 048 | 64 MiB | 54,0 s (55,6 / 54,0 / 52,5) | – | – |

Bei 100k wird die Schreibzeit durch doppelte Batches nicht kürzer (41,0–41,9 s in allen Varianten, die Datei liegt im
Cache). Bei 1 Mio. sinkt sie um 83 s (die Commit-Kosten wachsen mit N, Ursache 1 in B3-PERF), aber mit 1 024 Nachrichten
ist der Kanal wieder kleiner als ein 16-MiB-Batch (≈ 880 Seiten ≈ 1 760 Nachrichten) und der Writer wartet 43 s. Mit der
abgeleiteten Kanalgröße (2 048) wäre das behoben, kostet aber +1 024 Seiten in K-49 und mehr Puffer. Ergebnis: **V2 nicht
übernommen** (Bedingung „messbarer Gewinn bei 100k, Median aus 3“ nicht erfüllt). Für große Crawls bleibt V2 zusammen mit
V3 (Änderungsfeed ohne zufälligen Index) ein Kandidat; dann mit abgeleitetem Kanal und PERF-003/K-49 neu messen.

## Tests

- **Voller Lauf auf dem Server** (Stand = Commit 27bd4bf2, per `srv-sync` gespiegelt, mit `~/ws-slot.sh`):
  `cargo fmt --all --check` → 0, `cargo clippy --workspace --all-targets -- -D warnings` → 0,
  `cargo test --workspace` → 0: **605 Testläufe, 4 353 bestanden, 0 fehlgeschlagen, 51 ignoriert**
  (B3-PERF-Endstand: 4 322 / 0 / 51). Debug-Target danach 118 GB, gelöscht.
- Gezielt vorab: `cargo test -p os-engine --lib writer_thread` (16 bestanden, darunter der neue Test),
  `-p os-engine --test core_writer --test perf_backpressure` (3 + 1), `-p os-loadtest --test engine_adapter` (9; darin
  PERF-005 klein mit der aus den Konstanten abgeleiteten K-49-Grenze und PERF-003 klein).
- **Gegenprobe:** Der neue Test mit den alten Konstanten (Kanal 256, Schwelle 16 MiB, ohne const-Prüfung) schlägt fehl:
  „Worker wartet vor Discovery 128 von 455“ (Kanal nach 128 Seiten × 2 Nachrichten voll). Datei danach zurückgesetzt.
- **Fable-Nacharbeit (3a07904c), Server:** `cargo fmt --all --check` 0, `cargo clippy --workspace --all-targets -- -D
  warnings` 0, `cargo test -p os-engine --lib writer_thread` 16 bestanden, `--test core_writer --test perf_backpressure`
  3 + 1 bestanden; zusätzlich `--lib writer_thread` mit `REFERENCE_PAGE_BYTES` = 16 KiB (Zweierpotenz-Grenzfall,
  danach zurückgesetzt): übersetzt und 16 bestanden. Kein Workspace-Volltest (nur `writer_thread.rs` und Bericht
  geändert, Vorgabe Orchestrator).
- Bestehende Prüfungen, die die Konstanten verwenden, unverändert und grün: `core_writer.rs` (Rückstau-Grenze
  `CHANNEL_CAPACITY + 2 × BATCH_PAGES_BACKLOG + 100`), `writer_thread.rs` (Byte-Gate-Test mit `IN_FLIGHT_BYTES`,
  Batchschnitt mit `BATCH_BYTES`, Kanal-4-Test), `os-loadtest` `REPEAT_BUFFER_ALLOWANCE`/`engine_adapter.rs` (K-49).
  Keine Schwelle, kein Test und kein Budget gelockert (K2-63). Die Store-Benchmarks (`bench_writer.rs`,
  `bench_links_build.rs`, `perf_store.rs`) nennen nur `BATCH_PAGES_BACKLOG` (unverändert) und hängen nicht vom Kanal ab;
  nicht erneut gelaufen.
- Lasttests: siehe Budgets (`perf-001` large und small, `perf-003` large, `perf-005` large mit 10 Abbrüchen: alle `passed`).
- App nicht geändert, daher kein `npm`-Lauf.

## Rückstau-Heuristik (Fable-Befund, entschieden: relativ lassen)

Der Writer sammelt im Rückstau-Modus bis `BATCH_PAGES_BACKLOG`, wenn `behind` gilt (nach dem letzten Schreiben stand
Arbeit im Kanal) oder wenn mehr als ein Achtel des Kanals wartet (`rx.len() * 8 > capacity`). Mit 1 024 Nachrichten
greift die Achtel-Regel ab 129 statt ab 33 wartenden Nachrichten. **Entscheidung: so gelassen**, begründet im Kommentar
an der Stelle. Hauptauslöser ist `behind`, das nach jedem Schreiben mit nicht leerem Kanal gesetzt wird; die
Achtel-Regel entscheidet nur über den ersten Batch nach einer Ruhephase. 129 Nachrichten sind höchstens 129 Seiten,
also weniger als `BATCH_PAGES` (200): Der Batch endet dann regulär bei 200 Seiten oder 100 ms, und der nächste sammelt
mit `behind` im Rückstau-Modus. Eine absolute Schwelle von 32 Nachrichten würde höchstens einen 200-Seiten-Batch je
Ruhephase vermeiden und wäre ein ungemessener Eingriff; alle Messungen dieses Berichts liefen mit der relativen Regel.

## Entscheidungen und Auflagen (Orchestrator, nach Fable-Abnahme „approved 86“)

1. **K-49:** Der Orchestrator nimmt 5 032 wiederholte Requests je Abbruch (8 Threads) an, nach Fable-Empfehlung. Das
   ist eine Plan-Entscheidung, keine Spec-Änderung (PERF-005 verlangt „keine Duplikate, keine Verluste“, die Formel aus
   K-49 bleibt). Die Alternative `BATCH_PAGES_BACKLOG` 3 232 entfällt.
2. **Auflage vor dem beta.3-Tag – PERF-003 auf dem Mac mini:**
   `N20_MAX_LOAD=2.5 bash crates/os-loadtest/scripts/n20-mac.sh <out>` (PERF-003 large zweimal, PERF-001 large einmal,
   PERF-001 small dreimal). Linux stieg von 416 auf 439 MB (+23 MB); der letzte Mac-Wert (M2-N20, 31.10.) lag bei
   451 MB, erwartet also ≈ 475 MB bei Grenze 500 MB. **Rückfall, falls die Grenze reißt:**
   `IN_FLIGHT_BYTES = 5 * BATCH_BYTES / 2` (**20 MiB**). Die const-Prüfung bleibt damit erfüllt (schreibender Batch
   8 MiB + voller Kanal aus Referenzseiten 9 MiB = 17 MiB < 20 MiB); der Kanal allein brachte bei 16 MiB schon
   78 → 56,6 s (100k).
3. **Referenzseite als Konstante** (nur zur Kenntnis): `REFERENCE_PAGE_BYTES` (18 KiB) ist ein Messwert der
   PERF-001-Fixture, keine Spec-Zahl. `MESSAGES_PER_PAGE` = 2 ist die Obergrenze zu Crawlbeginn; im eingeschwungenen
   Crawl kommt etwa eine Nachricht je Seite an, der Kanal fasst dann Seiten für gut zwei Batches, und die Byte-Schwelle
   begrenzt den Puffer (PERF-005: höchstens 1 492 wiederholte Abrufe je Abbruch). „Kanal = genau ein Batch“ gilt nur zu
   Crawlbeginn; Kommentar in `writer_thread.rs` entsprechend präzisiert (3a07904c).

## Folgeauftrag: Render-Nutzdaten im Byte-Gate (Fable-Befund, nicht in diesem Paket)

**Befund:** `estimate_bytes` (`writer_thread.rs`) zählt für die M2-Erweiterungen nur die Strukturgröße der Vektoren
(`capacity × size_of_val`), nicht deren Heap-Inhalt. Ungezählt sind vor allem `RenderWrite.html` (gerendertes DOM),
`RenderWrite.screenshots[].data` (Bilddaten), `RenderWrite.console`/`network`/`blocked`, `structured_data`, `a11y` und
`custom_js`. Diese Daten passieren die Byte-Schwelle (`IN_FLIGHT_BYTES`) und das Nutzdatenbudget (`BATCH_BYTES`) also
fast ungezählt. Der Kanal ist seit B3-QUEUE viermal so groß (1 024 statt 256 Nachrichten); im Textmodus begrenzt die
Byte-Schwelle den Puffer, im Rendermodus nur die Nachrichtenzahl.

**Szenario für den Folgeauftrag:** Crawl mit JavaScript-Rendering, mit Screenshots und gespeichertem
gerendertem HTML, langsamer Senke (SlowSink 100 ms je Batch wie PERF-003) und Seiten mit großem DOM (z. B. 500 KB
gerendertes HTML, 1–2 MB Screenshot-Kacheln, einige hundert Netzwerk-/Konsoleneinträge). Erwartet: Bis zu 1 024
Nachrichten im Kanal plus ein Sammelbatch bis `BATCH_PAGES_BACKLOG` Seiten, bei 1–2 MB je Seite also Hunderte MB bis
über 1 GB Puffer, ohne dass das Byte-Gate greift (der Kanalanteil war mit 256 Nachrichten ein Viertel davon). Prüfung:
`estimate_bytes` zählt die genannten Heap-Daten (Test gegen eine Fixture mit Render-Daten: Schätzung ≥ Summe der
Puffer), der Rückstau im Rendermodus bleibt durch `IN_FLIGHT_BYTES` begrenzt (Fixture-Crawl mit gerenderten Seiten und
SlowSink: `mem_writer_backlog_bytes_max` ≤ Schwelle + eine Seite, Spitzen-RSS des Kernprozesses ohne Browser
gemessen). Spec-Bezug: PERF-003 (alle Warteschlangen begrenzt), PERF-006 (Rendering unter Kontrolle).

## Fremde Dateien (K2-92)

| Ort | Datei/Verzeichnis | Herkunft | Umgang |
|---|---|---|---|
| `~/Projects/openspider` (Haupt-Checkout, Mac) | `docs/plan/beta3.md` (geändert), `docs/release/release-rhythmus.md` (neu) | Orchestrator/andere Pakete | gesehen, nicht angefasst |
| `~/Projects/openspider-wt/c-queue` (Mac) | – | nur eigene Änderungen | – |
| `~/Projects/openspider-wt/m3-queue` (Server) | `.claude-active`, `target-rel/` | `srv-run.sh` bzw. eigene Release-Builds | nicht committet, `target-rel` am Ende gelöscht |
| Server `~/perf` | B3-PERF-Werkzeuge und -Logs, `bin/*-base`, `bin/*-final3` u. a. | Vorgängerpaket | nur gelesen bzw. aufgerufen; eigene Dateien unter `~/perf/q/`, `bin/*-q0`, `*-qd`, `*-q1`, `runs/q-*` |
| Server allgemein | fremde `cargo check`/`cargo test --workspace`, Playwright (u. a. `m3-rel3`, `m3-ui3`, `m3-cursor`) | andere Pakete | nicht angefasst; meine Läufe warten auf 3 min Ruhe, gestörte Läufe sind markiert und wiederholt |

## Server-Aufräumstand

Freier Platz am Ende: **253 GB** (`/` 72 GB belegt). Gelöscht: Debug-Target (118 GB) und Release-Target
`target-rel` des Server-Worktrees, alle Crawl-Dateien und Exporte (`~/perf/out/q-*`, `~/perf/home/q-*`; jeder
1-Mio.-Lauf löschte seine Dateien direkt danach), Arbeitsordner der Lasttests, Diagnose-Binärdateien `*-qd`. Testsites
und Lastschreiber beendet. Übrig: Logs und Skripte unter `~/perf/q/` und `~/perf/runs/q-*`, Lasttest-Berichte
`~/perf/q/lt/*.json`, Binärdateien `~/perf/bin/{openspider,os-loadtest,os-testsite}-{q0,q1}` (vorher/nachher, ~330 MB,
für Nachmessungen; können weg). Server-Worktree `m3-queue` 127 MB.
