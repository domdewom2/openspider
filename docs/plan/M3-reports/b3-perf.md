# B3-PERF – Leistung für beta.3 (Link Score, Skalierung 1 Mio.)

Stand: 04.11.2026, Branch `m3/perf` (Basis `origin/main` 4d12d776), Messungen auf dem Linux-Build-Server
(16 Kerne, 61 GB RAM), Werkzeuge aus `~/perf` (`run.sh`, `ts.py`, `phases.py`) plus `segments.py`
(Durchsatz je 10 % nach PERF-002) und `cmp.py` (Exportvergleich nach `Address`).

**Auftragsänderung (Nutzerentscheidung 04.11.2026):** Umgesetzt wird nur Teil 1 (Link Score im Speicher);
dazu die Diagnose der 1-Mio.-Skalierung. Writer/`write_ext`, `soft404_text` und Parser-Entschlackung
entfallen. Begonnene Arbeit daran liegt ungetestet auf dem lokalen Branch `m3/perf-parked`
(Commit 2fe42c7d: `write_ext` mit je Batch aufgelösten Schlüsselkennungen und `ext_texts`-Abfrage je URL
statt drei Anweisungen je Zeile; `content::PageTexts` mit einem Durchlauf je Inhaltsbereich/Body und
statischen Selektoren, samt Gleichheitstest). Nicht in `m3/perf`, nicht gepusht. Zu `soft404_text` gab es
vor dem Abbruch einen Befund (siehe Spec-Fragen 3).

## Commits (`m3/perf`, nicht gepusht)

| Commit | Inhalt |
|---|---|
| 3ce986ef | `analyze:` Link Score im Speicher statt zehn SQL-Durchläufen; direktes Schreiben in `url_ext_data`; Gleichheitstest gegen das bisherige SQL-Verfahren |
| d455a95f | `analyze:` gepackte Form bei verstreut gespeicherten Links (Speichergrenze), Messzähler `OS_SCALE_DIAG linkscore …` |
| 2948f501 | `analyze:` Formatierung, Typalias im Test (clippy) |
| a943ae9f | `analyze:` `OS_SCALE_DIAG linkscore` meldet auch die Graphform (`packed`) |
| 13390469 | `test:` Ausgabetext des Link-Score-Benchmarks (Graph im Speicher) |
| 664ec936 | `analyze:` `LIKE` auf `rel` nur bei gesetztem `rel`, Textvergleich ohne Zuordnung (5 Mio./100 Mio.: 71,5 → 47,8 s) |
| (dieser Bericht) | `docs:` Bericht |

## Teil 1: Link Score im Speicher (TECH-145, ONPAGE-017)

**Vorher** (`link_score.rs`, Stand beta.2): Routen in eine Temp-Tabelle, Kanten per `INSERT … SELECT … GROUP BY`
in eine zweite Temp-Tabelle, dann **zehn** `SELECT source, target … ORDER BY source, target` über alle
Kanten; Rückschreiben über die Sicht `url_ext` mit `INSTEAD OF`-Triggern (vier Anweisungen je Zeile) nach
drei Läufen über die ganze Sicht (`SELECT`/`DELETE`/`UPDATE … IN (SELECT …)` ohne Index auf `key_id`).

**Nachher:**
- Ein Lauf über `urls` (Knoten, Auflösungsdaten, `effective_follow`), Auflösung `res(u)` unverändert
  (höchstens 10 Schritte, Schleife → undefiniert), Zuordnung Kennung → Position über eine dichte Tabelle
  (Rückfall binäre Suche bei sehr lückenhaften Kennungen).
- Ein Lauf über `links` in Speicherreihenfolge. Die Folgbarkeit je Link (`follow`, `rel` mit
  nofollow/ugc/sponsored) bleibt derselbe SQL-Ausdruck wie bisher (also auch dieselbe `LIKE`-Semantik),
  das seitenweite nofollow kommt aus `urls`. Je Quellknoten ein doppelfreier Abschnitt (4 Byte je Kante:
  Zielknoten + Folgebit; „follow gewinnt“ wie `MAX(...)`). Taucht eine Quelle in einem späteren Lauf erneut
  auf, wechselt der Graph in eine gepackte Form (8 Byte je Kante), die sortiert und entdoppelt wird.
- Zehn Iterationen im RAM; Summation wie bisher in Quellreihenfolge aufsteigend (für jedes Ziel ist die
  Additionsreihenfolge dieselbe wie bei `ORDER BY source, target`), deshalb **bitgleiche Rohwerte**.
- Veröffentlichung direkt in `url_ext_data` mit einmal aufgelösten Schlüsselkennungen. Vorhandene Werte
  werden nur gelesen, wenn `linkscore.raw`/`linkscore.score` in `ext_keys` je angelegt wurden (erster Lauf:
  kein Lesen). Unveränderter Stand → kein neuer `change_seq` (wie bisher); entfernte Knoten bekommen den
  Stempel auf `urls`; verwaiste Werte (URL-Zeile gelöscht) werden wie bisher entfernt; Abbruch rollt zurück.
- `PRAGMA temp_store` wird nicht mehr angefasst (keine Temp-Tabellen).
- Speicher: 1 Mio./30 Mio. (zusammenhängend) ≈ 120 MB Kanten + 24 MB Abschnitte; 5 Mio./100 Mio.
  ≈ 0,5 GB (zusammenhängend) bzw. ≈ 0,9 GB gepackt; Grenze TECH-145: 2 GB. Messung unten.

**Tests** (`crates/os-analyze/tests/link_score_b3_perf.rs`, `// spec: TECH-145`, `ONPAGE-017`, `TECH-139`):
- `in_memory_graph_matches_previous_sql_iteration_bit_for_bit`: sieben Zufallsgraphen (60–1 500 Seiten;
  Weiterleitungen 301/302, Canonicals inkl. Selbst-Canonical, seitenweites nofollow und `NULL`,
  `rel` nofollow/`UGC`/`noopener Sponsored`/`nofollowx`, Mehrfachlinks, Selbstlinks, unbekannte und `NULL`-Ziele,
  Bildlinks, nicht crawlbare Links, lückenhafte Kennungen, mit und ohne verstreut gespeicherte Links)
  gegen das bisherige SQL-Verfahren als Orakel im Test: Rohwerte **bitgleich** (`f64::to_bits`), Scores gleich.
- `reanalysis_after_graph_change_replaces_rows_and_stamps_removed_nodes`: Neuanalyse nach Graphänderung,
  verwaister Wert, `change_seq` von `url_ext` und `urls`, unveränderte Wiederholung ohne Stempel.
- Bestehend und unverändert grün: `link_score_m2.rs` (Fixture `fx-ui-linkscore` mit den Spec-Goldens
  ± 1e-5 und Normierung log/linear, Python-Referenz-Zufallsgraphen < 1e-14, 10-Schritte-Grenze, Abbruch,
  `temp_store`-Wiederherstellung, Änderungsfeed), `graph_data.rs`, `graph_dirs.rs`. Der Abbruchtest lief
  30× hintereinander grün (die Iterationen sind jetzt schneller, der Abbruch greift in der Veröffentlichung).

## Messung (Server)

Befehl je Lauf (`~/perf/run.sh <bin> <label> <url> <rid>`):
`openspider --headless --crawl <URL> --export-tabs "Internal:All" --save-crawl --output-folder <dir> --json-progress`
(Standardwerte, 5 Threads), Site `os-testsite synthetic:n=100000,links=30` bzw. `n=1000000`.
1-Mio.-Läufe mit `OS_SCALE_DIAG=1`. Phasen nach `phases.py` (Crawl bis Beginn Analyse, Analyse, Rest =
Export + Speichern). „base“ = `origin/main` 4d12d776, „neu“ = `m3/perf`.

### 100k (je 3 Läufe, Median)

| Stand | Läufe gesamt (s) | Median gesamt | Crawl | Analyse | Export + Speichern | RSS (Median) | Crawl-Datei |
|---|---|---|---|---|---|---|---|
| base 4d12d776 | 85,6 / 85,0 / 85,6 | **85,6 s** | 69,4 s | 10,8 s | 5,1 s | 630 MB | 1,040 GB |
| neu 3ce986ef | 78,7 / 78,7 / 77,9 | **78,7 s** | 69,7 s | **3,5 s** | 5,1 s | 643 MB | 1,040 GB |
| neu d455a95f | 98,0¹ / 77,2 / 77,9 | 77,9 s | 69,2 s | 3,7 s | 5,0 s | 652 MB | 1,040 GB |
| **neu 664ec936 (Endstand)** | 78,5 / 77,5 / 77,2 | **77,5 s** | 69,6 s | **2,7 s** | 5,1 s | 674 MB | 1,040 GB |

¹ gestört: fremder Workspace-Test startete während des Laufs (Last 24 am Ende); alle anderen Läufe bei Last < 6.
Wirkung Teil 1 bei 100k: **−8 s gesamt (−9 %)**, Analyse 10,8 → 2,7 s; Crawl und Dateigröße unverändert
(Link Score schreibt dieselben Zeilen), RSS-Median +44 MB (Graph im Speicher; Streuung zwischen Läufen ±30 MB). Erwartet waren −9 s; die übrigen Analyseschritte laufen parallel
(Links-Schritt ~1,6 s) und bestimmen jetzt mit.

### 1 Mio. (je ein Lauf)

| Stand | gesamt | Crawl | Analyse | davon Link Score | Export + Speichern | RSS | Datei | Last während Lauf |
|---|---|---|---|---|---|---|---|---|
| beta.2 bf8d50e7 (Diagnose 04.11. früh) | 1 068,5 s | 910,0 s | 113,0 s | 112,8 s | 45,5 s | 789 MB | 10,6 GB | ruhig |
| base 4d12d776 | 1 081,8 s | 918,1 s | 113,3 s | ~113 s | 50,4 s | 863 MB | 10,6 GB | < 5 |
| neu 3ce986ef | 1 023,2 s | 931,3 s | **43,6 s** | 43,3 s | 48,3 s | 792 MB | 10,6 GB | ruhig |
| neu d455a95f | 1 159,7 s | 991,1 s² | 43,7 s | 43,4 s | 124,9 s² | 923 MB² | 10,6 GB | Mittel 10,8, Spitze 31 |
| **neu 664ec936 (Endstand)** | **975,3 s** | 906,8 s | **23,5 s** | **19,2 s** | 44,9 s | 935 MB | 10,6 GB | Mittel 2,8, Spitze 3,7 |

² d455a95f-Lauf durch parallele fremde Workspace-Tests gestört (Crawl-Abschnitt 3 nur 757 URLs/s, Export
dreimal so lang); für den Crawl ist der Code identisch mit 3ce986ef, die Analysezahl ist belastbar.
Link Score bei 1 Mio. (30 Mio. Kanten, `OS_SCALE_DIAG linkscore`, Endstand): `urls` lesen 2,0 s, `links`
lesen und Graph bauen 10,7 s (vor 664ec936: 18,4 s), zehn Iterationen **1,0 s**, Veröffentlichen 5,5 s
(2 Mio. Zeilen in `url_ext_data`, 924 MB WAL wie bisher; in den belasteten Läufen davor 22 s); Form
`packed=0`. **Wirkung Teil 1 bei 1 Mio.: Link Score 113 → 19 s, Analyse 113 → 23,5 s (−90 s), gesamt
1 082 → 975 s (−10 %)**; erwartet waren ~−100 s. Die Analyse begrenzt jetzt der parallel laufende Schritt
„Links“ (16–26 s). RSS 935 MB (base 863 MB; zwischen Läufen ±70 MB, der Graph belegt ~150 MB nach dem
Crawl; PERF-001 < 1,5 GB, Verhältnis 1 Mio./100k 1,4 < 2).


## Gleichheitsnachweis

- **100k-Synthetik:** `internal_all.csv` base gegen neu (3ce986ef und Endstand 664ec936), je Lauf paarweise (je 3 Paare): 100 000 Zeilen,
  65 Spalten, keine fehlenden/zusätzlichen Adressen, **alle Werte gleich** außer den laufabhängigen
  `Response Time` und `Crawl Timestamp` (ausgenommen; zwischen zwei base-Läufen gilt dasselbe). `Link Score`
  ist in allen 100 000 Zeilen befüllt und gleich (Verteilung identisch).
- **Fixture-Sites** (`os-testsite <szenario>`, Crawl mit base und neu – 3ce986ef und Endstand 664ec936 –,
  Exporte `Internal:All` und `Links:All`; Skript `~/perf/fixtures.sh`):
  tech-m2-links (2 Zeilen), parity-m2-links (142), parity-m2-onpage (59), parity-m2-technical (67),
  parity-m2-security (17), crawl-site-100 (116), crawl-redirect-chains (25), crawl-canonicals (15),
  n10-links (6): **beide Exporte je Site zellgleich** (ohne `Response Time`/`Crawl Timestamp`); Link Score
  befüllt in 2/142/51/51/15/100/5/9/5 Zeilen (nicht teilnehmende URLs leer wie spezifiziert).
- **Rohwerte:** bitgleich gegen das alte Verfahren (Test oben); Spec-Goldens ± 1e-5 und Python-Referenz < 1e-14
  unverändert grün.

## Diagnose: Warum skaliert 1 Mio. schlechter als 100k?

Grundlage: 1-Mio.-Läufe mit `OS_SCALE_DIAG=1` (Writer-Zähler `os_store::sink::diag` je 100 Batches,
`os_engine::diag`), Fortschrittszeilen je Sekunde, 100k-Diagnoselauf `b2diag` vom selben Tag.
Diagnosevarianten mit einer **nicht committeten** Kopie (`~/Projects/openspider-wt/m3-perf-diag`), in der
`CHANNEL_CAPACITY`, `IN_FLIGHT_BYTES`, `BATCH_BYTES` per Umgebungsvariable überschreibbar sind und der Index
`url_counters_change_seq` testweise abgeworfen werden kann.

### Wo die Zeit hingeht

Hochgerechnet aus 100k (85,6 s × 10 = 856 s) fehlen bei 1 Mio. (base 1 082 s) **226 s, praktisch alle im
Crawl** (918 statt 694 s). Analyse (113 statt 108 s) und Export + Speichern (50 statt 51 s) skalieren linear.

| Phase (1 Mio., base) | Dauer | Anmerkung |
|---|---|---|
| Crawl | 918 s | davon Endbatch 13–20 s (Aufbau `links_target` 11,5–18 s, K-74) |
| Analyse | 113 s | parallel; Link Score 113 s, Links 17–26 s, Canonicals 5–6 s, Hreflang 4–5 s, Rest < 1 s; Endstand: 23,5 s (Link Score 19 s) |
| Export `Internal:All` | 18,8 s | 1 Mio. Zeilen (PERF-008 < 60 s erfüllt) |
| Speichern `--save-crawl` | 26,7 s | `VACUUM INTO` der 10,6-GB-Datei |

**Durchsatz je 10 % (PERF-002-Methode):** 1 Mio.: 840 · 1 139 · 1 162 · 1 171 · 1 153 · 1 131 · 1 147 · 1 143 ·
1 143 · 1 157 URLs/s; 100k: 1 116 · 1 503 · 1 573 · 1 626 · 1 478 · 1 626 · 1 427 · 1 444 · 1 648 · 1 552.
**Der Crawl wird gegen Ende nicht langsamer**; er läuft bei 1 Mio. vom zweiten Abschnitt an gleichmäßig
~26 % langsamer (≈ 1 150 statt ≈ 1 550 URLs/s). Der erste Abschnitt ist in beiden Größen langsamer (Einfügen
der neu entdeckten URL-Zeilen und Auslagern der Warteschlange: `g_urls_ins` 7,2 + 5,3 + 2,1 s in den ersten
drei Eimern bei 1 Mio.).

**Ausgeschlossen** (Zähler flach bzw. klein): WAL/Checkpoints (kein Checkpoint während des Crawls außer im
Commit, WAL-Spitze konstant 582 MB, Abschluss-Checkpoint 0,1–0,5 s), Warteschlange/Nachladen
(`q_starved` 0, `q_refill_wait` 29 s im Hintergrund), Dedupe (`seen` 24 MB, 0 Kollisionen), Cache-Fehlgriffe
(~4 000 je 100 Batches), Abruf und Parsen je Seite (je Worker 0,97–1,01 ms Abruf, 0,95–0,99 ms Parsen pro
Seite in beiden Größen).

### Ursache 1: zufällige Zähler-Updates machen jeden Commit teurer (wächst mit N)

| je Batch (eingeschwungen) | 100k | 1 Mio. | 1 Mio. ohne Index `url_counters_change_seq` |
|---|---|---|---|
| geschriebene Seiten (`cache_write`) | 3 900 | 12 000 | 7 600 |
| Commit | 32–38 ms | 114 ms | 72 ms |
| Aufbau `url_counters` (`g_counters`) | 39 ms | 62 ms | 35 ms |
| Aufbau gesamt | 159 ms | 190 ms | 160 ms |

Jeder Batch (~464 Seiten × 30 Links) ändert ~14 000 zufällige Zeilen in `url_counters` und versetzt für jede
den Eintrag im Index `url_counters_change_seq` (alter Wert an zufälliger Stelle löschen, neuer am Ende). Bei
100k passen beide in ~1 300 Seiten, die jeder Batch ohnehin fast alle berührt; bei 1 Mio. sind es ~9 000 bzw.
~3 000 Seiten, von denen ein Batch ~80 % schmutzig macht. Der Commit schreibt diese Seiten ins WAL, daher wächst
er mit N bis zur Sättigung. Ohne den Index: Writer-Zeit 677 → 519 s, **Crawl 918 → 772 s (−146 s)**.

### Ursache 2: Kanal kleiner als ein Batch → Writer und Worker warten abwechselnd

Der Writer-Kanal fasst 256 Nachrichten (≈ 256 Seiten), ein Rückstau-Batch endet am Byte-Budget bei ~464
Seiten (8 MiB). Während der Writer schreibt (1 Mio.: ~300 ms je Batch, 100k: ~200 ms), läuft der Kanal voll und
die Worker blockieren (`reserve`: 375 s je Worker = 41 % ihrer Zeit; 100k: 16 s = 23 %); danach wartet der
Writer, bis der Batch voll ist (`w_poll_sleep` 235 s, Writer nur 74 % belegt). Bei 1 Mio. ist der Effekt
größer, weil jeder Batch wegen Ursache 1 länger schreibt.

| Variante (1 Mio., sonst Standard) | Crawl | gesamt | Writer belegt | Writer wartet | Worker blockiert | URLs/s eingeschwungen |
|---|---|---|---|---|---|---|
| Standard (base) | 918 s | 1 082 s | 677 s | 235 s | 375 s | ~1 150 |
| `BATCH_BYTES` 16 MiB, `IN_FLIGHT_BYTES` 32 MiB | 911 s | 987 s³ | 536 s | 365 s | 354 s | ~1 140 |
| `CHANNEL_CAPACITY` 1 024, `IN_FLIGHT_BYTES` 32 MiB | **628 s** | 703 s³ | 623 s | 0 s | 121 s | ~1 670 |
| beides, `IN_FLIGHT_BYTES` 48 MiB | **593 s** | 670 s³ | – | – | – | ~1 830 |
| Index `url_counters_change_seq` abgeworfen | 772 s | 847 s³ | 519 s | 247 s | 219 s | ~1 370 |

³ Die Diagnosekopie enthält den neuen Link Score (Stand 3ce986ef), daher Analyse ~30 s statt 113 s.
Größere Batches allein halbieren die Commit-Kosten je Seite (Writer −141 s), helfen aber nicht, solange der
Kanal kleiner ist als der Batch. Mit größerem Kanal ist der Writer durchgehend belegt; der Crawl ist dann
genau so lang wie die Writer-Zeit. RSS der Varianten 804–950 MB (base 863 MB, Schwankung zwischen Läufen
~±70 MB; PERF-001-Grenze 1,5 GB).

**Gegenprobe 100k** (Diagnosekopie, je 2 Läufe, Last < 3): Standard 77,7 / 77,9 s (Crawl 69,5 / 69,3 s),
`CHANNEL_CAPACITY` 1 024 + `IN_FLIGHT_BYTES` 32 MiB **53,1 / 53,5 s (Crawl 44,7 / 44,8 s)**, RSS 668–679 statt
635–637 MB. Der Kanal-Engpass besteht also schon bei 100k (Writer wartet dort 23 von 69 s); bei 1 Mio. kommt
Ursache 1 hinzu und verlängert jede Schreibrunde.

### Mac mini (Faktor 16 statt 10)

Auf dem Mac wurde nach Arbeitsregel nicht gemessen. Wahrscheinlich: dieselben zwei Ursachen (beide hängen nur
von N ab) **plus** Speicherdruck: Die 1-Mio.-Datei (10,6 GB) passt bei 16 GB RAM und laufender App nicht in den
Dateicache. Betroffen sind die Schritte, die große Teile der Datei lesen: Endbatch (`links_target`, Aufbau
liest `links`), der alte Link Score (zehn Läufe über eine Temp-Tabelle auf der Platte plus drei Läufe über
`url_ext` ≈ 5 GB; jetzt ein Lauf über `links`), Export und `VACUUM INTO` (10,6 GB lesen und schreiben). Auf
dem Server (61 GB RAM) bleibt alles im Cache. Zur Bestätigung ein Lauf mit `OS_SCALE_DIAG=1` auf dem Mac,
wenn er frei ist (Phasen wie oben, dazu `vm_stat`).

### Vorschläge (nicht umgesetzt) mit Größenordnung (1 Mio., Server)

| # | Maßnahme | Wirkung | Aufwand/Risiko |
|---|---|---|---|
| V1 | `CHANNEL_CAPACITY` 256 → 1 024 und `IN_FLIGHT_BYTES` 16 → 32 MiB (`os-engine/src/writer_thread.rs`) | Crawl −290 s (−32 %), gesamt −35 %; bei 100k −24 s (−31 %) | zwei Konstanten; verändert aber die K-49-Grenze ungeschriebener Seiten (+768) und die Speicherbilanz: PERF-003 (Slow-Sink, RSS < 500 MB) und PERF-005 (K-49) neu messen, daher hier nicht umgesetzt |
| V2 | zusätzlich `BATCH_BYTES` 8 → 16 MiB, `IN_FLIGHT_BYTES` 48 MiB | weitere −35 s | wie V1 |
| V3 | Änderungsfeed von `url_counters` ohne zufälligen Index (z. B. Stempel nur in einer anhängenden Tabelle je Batch oder Feed über `urls.change_seq`) | −146 s bei Standardkanal; mit V1 kleiner, aber Writer-Zeit −158 s | Schema + `queries::changed_since`, eigenes Paket |
| V4 | Analyse-Schritt „Links“ (16–26 s, begrenzt jetzt die Analyse) und Link Score weiter entschlacken (Lesen von `links` 10,7 s, Schreiben in die Cluster von `url_ext_data` 5,5 s/924 MB WAL) | −5…−15 s Analyse | „Links“ nicht untersucht; Link Score in eigener Tabelle wäre eine Schemaänderung |
| V5 | `--save-crawl` einer geschlossenen, zurückgeführten Datei per Dateikopie (APFS-Klon / `copy_file_range`) statt `VACUUM INTO` | −20…−27 s | Ergebnis nicht verdichtet → Spec-Entscheidung (OUT-230/CRAWL-127) |
| V6 | erster 10-%-Abschnitt (Einfügen neuer URLs, Auslagern der Warteschlange) | ≤ 20 s | nicht untersucht |

## Testergebnisse

- `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
  auf dem Server (mit `~/ws-slot.sh`), Endstand 664ec936: fmt 0, clippy 0, Tests 0 – 602 Testläufe,
  4 322 bestanden, 0 fehlgeschlagen, 51 ignoriert (davor auf 2948f501 ebenfalls grün: 4 310/0/51). Das
  Debug-Target (~115 GB) danach gelöscht.
- Gezielt: `cargo test -p os-analyze --test link_score_b3_perf --test link_score_m2 --test graph_data --test graph_dirs`
  grün; Abbruchtest 30× grün.
- App nicht geändert, daher kein `npm`-Lauf.
- Leistungstests (TECH-145/PERF-007, vorhandene `#[ignore]`-Benchmarks, Release, Server, `TMPDIR` auf der
  Platte, da `/tmp` ein tmpfs ist): `cargo test --release -p os-analyze --test link_score_benchmarks -- --ignored --nocapture --exact <name>`
  - `thirty_million_edges_under_five_minutes` (1 Mio./30 Mio., PERF-007 < 5 min): **17,1 s**, Spitzen-RSS des
    Testprozesses 597 MB – grün.
  - `hundred_million_edges_under_sixty_seconds` (5 Mio./100 Mio., TECH-145 ≤ 60 s, ≤ 2 GB): mit d455a95f
    **71,5 s – rot**; Aufschlüsselung zeigte das Lesen von `links` als größten Teil (drei `LIKE` je Link).
    Nach 664ec936 (`LIKE` nur bei gesetztem `rel`, gleiche Semantik) **47,8 s, Spitzen-RSS 1,34 GB – grün**
    (`urls` 2,7 s, `links` + Sortieren 24,2 s, Iterationen 1,8 s, Veröffentlichen 19,0 s; Form `packed=1`, weil
    der Benchmark die Links in 20 Durchläufen über alle Quellen anlegt). Zum Vergleich der alte Stand: bei
    1 Mio./30 Mio. im Crawl 113 s. Der Benchmark läuft auf dem Build-Server, nicht auf der in M1 genannten
    Referenzmaschine (Mac mini); dort nicht gemessen.

## Spec-Fragen / Unklarheiten

1. **PERF-007** nennt „Link Score iterativ auf der Crawl-Datei“. Umgesetzt ist: Eingabe aus der Crawl-Datei,
   Iteration über einen kompakten Graphen im RAM innerhalb der TECH-145-Grenze (≤ 2 GB zusätzlicher
   Arbeitsspeicher bei 5 Mio./100 Mio.). Falls „auf der Crawl-Datei“ eine plattenbasierte Iteration verlangt,
   bitte klarstellen; die Zahlen von TECH-145/PERF-007 erfüllt die Lösung.
2. **PERF-002 „Durchsatzbudget für neue Analysen“** (Auftrag): PERF-002 legt nur den Crawl-Durchsatz fest
   (≥ 500 URLs/s mit 8 Threads, ±30 % je 10 %-Abschnitt), kein Budget für Analysen. Für Link Score gelten
   TECH-145 (5 Mio./100 Mio. ≤ 60 s, ≤ 2 GB) und PERF-007 (30 Mio. Kanten < 5 min); beide als
   `#[ignore]`-Benchmarks vorhanden, hier gemessen (siehe Testergebnisse). Ein Gesamtbudget für die
   Crawl-Analyse (CRAWL-141) fehlt in der Spec.
3. (Nur Befund, Teil entfallen) `content.soft404_text` lässt sich nicht ohne Verhaltensänderung aus
   gespeicherten Daten bilden: Der Filter ONPAGE-147 läuft als Live-SQL mit den aktuellen Phrasen,
   der Text ist Unicode-kleingeschrieben (SQLite `lower()` nur ASCII) und enthält alle `<title>`/`<h1>`
   (auch in SVG), die nicht gespeichert sind. Eine Lösung bräuchte eine registrierte SQL-Funktion auf jeder
   lesenden Verbindung oder einen Analyse-Schritt (Filter wäre dann bis zur Analyse leer).

## Fremde Dateien im Arbeitsverzeichnis (K2-92)

| Ort | Datei/Verzeichnis | Herkunft | Umgang |
|---|---|---|---|
| `~/Projects/openspider` (Haupt-Checkout, Mac) | – | `git status` beim Start sauber | nicht angefasst |
| `~/Projects/openspider-wt/c-perf` (Mac) | – | nur eigene Änderungen | – |
| `~/Projects/openspider-wt/m3-perf` (Server) | `.claude-active`, `target-rel/` | `srv-run.sh` bzw. eigene Release-Builds | nicht committet |
| `~/Projects/openspider-wt/m3-perf-diag` (Server, kein git) | Kopie mit Diagnose-Patch | eigene Messvarianten (Konstanten per Umgebungsvariable, Index-Abwurf) | nur Messung, nicht im Branch |
| Server allgemein | fremde `cargo test --workspace` (u. a. `m3-diff`) | andere Pakete | nicht angefasst; ein 1-Mio.-Lauf pausierte automatisch wegen Plattenplatz, Messungen danach nur bei Last < 5 und ≥ 40 GB frei |

**Plattenplatz:** Das Debug-Target meines Server-Worktrees war nach zwei Workspace-Testläufen auf ~115 GB
gewachsen und trug dazu bei, dass die Serverplatte voll lief. Es ist gelöscht (nach jedem weiteren Workspace-Lauf
ebenfalls), ebenso die Diagnosekopie `m3-perf-diag`, `target-rel` und alle Crawl-Dateien/1-Mio.-Exporte; jeder
Messlauf löscht seine Crawl-Datei direkt danach. Übrig in `~/perf`: Logs, 100k-Exporte, Binärdateien (`bin/`).
