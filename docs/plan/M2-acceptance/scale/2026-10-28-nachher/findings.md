# Finale Nachher-Auswertung

| URLs | Bytes/URL vorher → nachher | ungenutzt % vorher ≥ → nachher | URLs/s erstes/letztes vorher → nachher | WAL-Spitze MB vorher → nachher | Abschluss s vorher → nachher |
|---:|---:|---:|---:|---:|---:|
| 100,000 | 20269.5 → 10322.4 | ≥37.42 → 7.78 | 2632/2632 → 2941/2941 | 522.2 → 506.7 | 22.2 → 16.1 |
| 250,000 | 20228.9 → 10261.3 | ≥37.17 → 7.17 | 2357/2434 → 2691/2718 | 849.5 → 494.1 | 71.5 → 39.3 |
| 500,000 | 20344.4 → 10419.5 | ≥37.70 → 7.59 | 2120/2156 → 2389/2691 | 609.2 → 809.6 | 90.8 → 92.7 |
| 1,000,000 | 20548.1 → 10528.8 | ≥37.20 → 7.78 | 1829/1985 → 2045/2486 | 3414.9 → 890.4 | 300.2 → 252.7 |

Vorher 100k: Exportbasis 219ef006; größere Stufen: Diagnose 5067b517. Finale Produktkorrekturen 506f3e1c, Messhilfe einschließlich RSS-Erfassung c9ae4e72, spätere Commits ergänzen Tests/Auswertung. Alle Crawl-Dateien wurden nach jeder Messung gelöscht. Bytes/URL beziehen sich auf gecrawlte URLs N; url_rows (einschließlich Robots-Zeile) stehen zusätzlich im JSON.

Abschlusszeit = abgetastete CLI-Gesamtdauer minus Crawl-Fortschrittszeit, einschließlich Endbatch/Index/Analyse/Checkpoint, ohne die separat angeforderten Vergleichsexporte. Die Abtastung kann die CLI-Zeit um bis zu fünf Sekunden überschätzen; Fenstergrenzen sind aus ganzzahligen Fortschrittssekunden interpoliert. Das letzte 250k-Fenster umfasst 50k URLs.

WAL-Spitze = Maximum aus 5-s-Abtastung, Store-Zähler und beiden Phasengrenzen. Deshalb ist der alte 500k-Wert hier höher als die frühere reine 5-s-Abtastung. Sehr kurze WAL-Spitzen können weiterhin fehlen. Vorher-RSS ist abgetastet; nachher zusätzlich Betriebssystem-RSS-Maximum der beendeten Kinder vor dem Export (CLI dominiert ps/cargo metadata). Ungenutzter Raum nachher umfasst alle dbstat-Bäume einschließlich SQLite-Autoindizes plus Freiliste. Das alte Inventar ließ Autoindizes aus: seine Werte sind ausdrücklich Untergrenzen, kein vollständiger Raumvergleich.

Messruhe: vorher Load < 2; final nachher wait_quiet < 3 und zusätzlich mindestens 80 % CPU-Leerlauf, caffeinate -i. Die Desktoplast blieb bei freier CPU teilweise knapp über 2; die Bedingung wurde offen geändert und in quiet/uptime je JSON gespeichert. Kleine Zeitdifferenzen sind damit keine isolierte A/B-Kausalitätsmessung. Größenverhältnis, Wertegleichheit und die absoluten Abnahmegrenzen sind davon unabhängig. Kein anderer eigener Build/Test lief parallel zu diesen Messungen.

Alle Analysen vollständig; alle sechs Abnahmebedingungen erfüllt. Details: acceptance.json; Gesamtprüfung: checks.json; Exporte: exports.json.
