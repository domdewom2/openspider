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
