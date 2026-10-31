# Nachher-Messung

Produktstand 958e5825, Messhilfe 14c018cc. Standardkonfiguration der CLI, 5 Threads, Balanced, synthetische Site links=30. Temporäre Crawl-Dateien nach jeder Messung gelöscht. Exporte der 100k-Stufe bytegleich nach derselben Normalisierung (exports.json).

Messruhe: Vorher Load < 2; finale Nachherläufe explizit Load < 3 plus CPU-Leerlauf mindestens 80 % (vorab mindestens 85 %). Grund: anhaltende Desktoplast bei weitgehend freier CPU. Tatsächliche Startwerte stehen je JSON unter quiet/uptime. Die geänderte Startbedingung begrenzt die Aussagekraft kleiner Zeitdifferenzen; Größen- und Ergebnisvergleich bleiben davon unabhängig.

| URLs | Bytes/URL | ungenutzt % | URLs/s erstes / letztes | WAL-Spitze Bytes | RSS-Spitze Bytes | CLI gesamt s |
|---:|---:|---:|---:|---:|---:|---:|
| 100000 | 10321.7 | 7.68 | 2941.2 / 2941.2 | 506170872 | 951681024 | 55.1 |
| 250000 | 10261.3 | 7.07 | 2643.9 / 2609.3 | 489315952 | 952926208 | 145.3 |
| 500000 | 10390.1 | 7.88 | 2439.0 / 2619.4 | 811471112 | 835108864 | 295.7 |

1 Mio. zunächst nicht ausgeführt: nach der 500k-Auswertung anhaltende Fremdlast durch macOS-Metadatenprozesse (Load zeitweise > 30). Keine Messung unter Last als Abnahmewert verwendet. Ausstehende 1-Mio.-Kriterien bleiben offen, bis eine ruhige Messung vorliegt.

WAL-Spitze ist das Maximum aus Fünf-Sekunden-Abtastung, Store-Zähler und Phasenendwerten. Sehr kurze Spitzen können trotzdem fehlen. CLI-Zeit hat bis zu fünf Sekunden Abtastfehler; das letzte 250k-Fenster umfasst 50k URLs.

## Nicht als endgültige Abnahme verwenden

Nach der numerisch erfolgreichen 1M-Stufe wurde die Analysebereitschaft separat mit derselben Release-CLI und 100k überprüft: `Crawl-Analyse fehlgeschlagen: SQLite-Fehler: database is locked`, `analysis_state=incomplete`; amp, canonicals, content, near_duplicates und sitemaps blieben pending. Die früheren Messhilfen erfassten diesen Zustand nicht. 0-ms-Phasen sind deshalb kein Geschwindigkeitsbeleg, und diese Stufen gelten nur als Zwischenstand.

Codebefunde: parallele Verbindungsinitialisierung konkurriert mit schon gestarteten Schreibtransaktionen; der Links-Graphdurchlauf hielt die MAIN-Schreibsperre auch für TEMP-Arbeit, während weitere Analysatoren und die Statuspersistenz nach fünf Sekunden aufgeben konnten. Korrektur: Initialisierung serialisieren und vor Arbeitsbeginn synchronisieren, angemessene begrenzte Schreibwartezeit mit Wiederherstellung des ursprünglichen Timeouts, Links-Graph in TEMP vor der atomaren Veröffentlichung berechnen. Schmale interne URL-Merkmale liegen dafür in einer TEMP-Tabelle, TEMP-Cache 32 MiB zusätzlich zu 64 MiB MAIN je Arbeiter. Unabhängige Rechenphasen bleiben parallel (CRAWL-141).

Neue Messungen speichern alle analysis_steps und verlangen completed ohne pending. Bisherige CSV-Vergleiche waren bytegleich, weil der CLI-Export die noch ausstehenden Schritte vor der Ausgabe nachholte. Endgültige Größen-/Zeit-/RSS-/WAL-Abnahme wird nur mit vollständig beendeter Analyse wiederholt.
