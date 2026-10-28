# Probelauf 10.000 URLs

- Stand: fcca401c; Release-Build `cargo build --release -p os-cli -p os-testsite -p os-loadtest` erfolgreich, Tokio 1.53.3.
- Mac mini, vor dem Lauf `wait_quiet.sh 2 120`: Load 1,85; `caffeinate -i`.
- Site: `os-testsite synthetic:n=10000,links=30` mit ephemerem Port. CLI: `openspider --crawl <URL> --headless --save-crawl --output-folder <temp> --json-progress`, isoliertes `OPENSPIDER_HOME`, Standardkonfiguration; 5 Threads laut Log.
- Ergebnis: 10.000 URLs, Exit 0, keine Warnungen; gemeldete Crawl-Phase 3 s (gerundet, ca. 3.333 URLs/s), vollständiger CLI-Aufruf 6,660 s (1.502 URLs/s).
- Crawl-Datei im Datenordner: 200.605.696 Bytes (20.060,6 Bytes/URL); zusätzliche gespeicherte Ausgabekopie 194.764.800 Bytes. Temporäre Dateien wurden gelöscht.
- `--save-crawl` erzeugt eine weitere Kopie; die folgenden Messungen verwenden die automatisch angelegte Datei im isolierten Datenordner, um das Platzbudget einzuhalten.
