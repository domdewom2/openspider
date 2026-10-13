# 07 – Stabilität & Performance (nicht-funktionale Anforderungen)

Ziel: Große Crawls dürfen weder an Arbeitsspeicher scheitern noch die Oberfläche einfrieren. Jede Anforderung wird durch einen automatisierten Lasttest gegen `os-testsite` (synthetische Sites) geprüft. Referenzhardware: MacBook Apple Silicon, 16 GB RAM, SSD; Windows-x64-Runner in CI für die Smoke-Varianten.

### PERF-001 Speicherobergrenze im Textmodus
- **Priorität:** P0
- **Beschreibung:** Der Speicherverbrauch des Crawlers wächst nicht linear mit der Anzahl der URLs. Crawl-Daten liegen in der Crawl-Datei, im RAM nur begrenzte Caches und Warteschlangen.
- **Akzeptanzkriterien:**
  - [ ] Crawl einer synthetischen Site mit 1.000.000 URLs (je ~30 interne Links): Spitzen-RSS des Kernprozesses < 1,5 GB.
  - [ ] RSS bei 100.000 und 1.000.000 URLs unterscheidet sich um weniger als Faktor 2.
  - [ ] Konfigurierbares Speicherlimit für Caches (Standard: 25 % des RAM, max. 4 GB).
- **Testfixture:** `os-testsite` Generator `synthetic:n=1000000,links=30`

### PERF-002 Durchsatz
- **Priorität:** P1
- **Akzeptanzkriterien:**
  - [ ] Lokal (keine Netzwerklatenz, Textmodus): ≥ 500 URLs/s auf Referenzhardware.
  - [ ] Durchsatz bleibt über den gesamten 1-Mio-Crawl innerhalb ±30 % des Mittelwerts (kein Einbruch durch Speicherdruck).

### PERF-003 Gegendruck (Backpressure)
- **Priorität:** P0
- **Beschreibung:** Alle Warteschlangen zwischen Fetch, Parse, Render und Store sind begrenzt. Ist eine Stufe langsamer, drosseln die vorherigen.
- **Akzeptanzkriterien:**
  - [ ] Test mit künstlich verlangsamtem Store (100 ms je Batch): RSS bleibt unter 500 MB, Crawl läuft weiter.

### PERF-004 Reaktionsfähigkeit der Oberfläche
- **Priorität:** P0
- **Beschreibung:** Die UI lädt Tabellen seitenweise (nur sichtbarer Bereich + Puffer). Keine Datenbankabfrage oder Analyse läuft im UI-Thread.
- **Akzeptanzkriterien:**
  - [ ] Bei 1.000.000 Zeilen: Scrollen ohne Frame länger als 100 ms; erste Zeilen nach Tabwechsel < 300 ms.
  - [ ] Sortieren/Filtern einer Spalte bei 1.000.000 Zeilen < 3 s, UI bleibt währenddessen bedienbar (Spinner statt Freeze).
  - [ ] Während eines laufenden Crawls mit voller Last bleibt die UI bedienbar (Event-Throttling ≤ 10 Updates/s).

### PERF-005 Absturzsicherheit & Fortsetzen
- **Priorität:** P0
- **Akzeptanzkriterien:**
  - [ ] Hartes Beenden (SIGKILL / Task-Manager) mitten im Crawl: Crawl-Datei ist danach konsistent und lässt sich öffnen.
  - [ ] „Fortsetzen“ crawlt nur noch nicht abgeschlossene URLs; keine Duplikate, keine Verluste (Test: Zählung gegen vollständigen Referenz-Crawl).

### PERF-006 JavaScript-Rendering unter Kontrolle
- **Priorität:** P1
- **Akzeptanzkriterien:**
  - [ ] Anzahl paralleler Chromium-Tabs konfigurierbar (Standard abhängig vom RAM).
  - [ ] Browser-Prozess wird nach N Seiten bzw. bei Überschreiten eines Speicherlimits neu gestartet, ohne Crawl-Abbruch.
  - [ ] Hängende Seiten (Endlos-JS) brechen nach Timeout ab und werden als Fehler markiert; der Crawl läuft weiter.

### PERF-007 Skalierbare Nachlauf-Analysen
- **Priorität:** P1
- **Beschreibung:** Near-Duplicates per MinHash/LSH (nicht paarweise), Link Score iterativ auf der Crawl-Datei, alle Nachlauf-Analysen abbrechbar und mit Fortschrittsanzeige.
- **Akzeptanzkriterien:**
  - [ ] Near-Duplicate-Analyse über 1.000.000 Seiten < 10 min, RSS < 2 GB.
  - [ ] Link Score über 30.000.000 Kanten < 5 min.
  - [ ] Analyse jederzeit abbrechbar, UI bleibt bedienbar.

### PERF-008 Große Exporte
- **Priorität:** P1
- **Akzeptanzkriterien:**
  - [ ] CSV-Export von 1.000.000 Zeilen streamt (konstanter Speicher) und dauert < 60 s.
  - [ ] XLSX-Export warnt bei Überschreiten des Excel-Zeilenlimits (1.048.576) und teilt auf mehrere Blätter auf.

### PERF-009 Lasttest-Suite
- **Priorität:** P0
- **Beschreibung:** `cargo bench`/eigener Runner `os-loadtest` führt PERF-001 bis PERF-008 aus und schreibt einen Bericht (`target/loadtest/report.json`). Klein-Varianten (10.000 URLs) laufen in jeder CI; die 1-Mio-Varianten vor jeder Meilenstein-Abnahme durch Fable.
