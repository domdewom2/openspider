# Zwischenmessung nach Speicher- und Batchkorrektur

Stand aacb0bd0. 100k: 1.032.146.944 Bytes (10.321,5 Bytes/gecrawlter URL), 7,68 % ungenutzter Raum, 2.941 URLs/s; normalisierte Exporte beide bytegleich (exports.json). 250k: 2.565.242.880 Bytes (10.261 Bytes/URL), 7,07 % ungenutzter Raum, Fenster 2.644 / 2.805 / 2.700 URLs/s. LinkScore 42,2 s statt 61,8 s der Diagnose, WAL-Zuwachs 339 MB statt 592 MB.

Die RSS-Spitze 1.590.378.496 Bytes liegt nach Crawlende (Sample 135,35 s, Crawlzeit 92 s), nicht im Writer. Jeder parallele Analysearbeiter öffnet bislang eine Verbindung mit 256-MiB-Cache. Folgekorrektur: 64 MiB je Analysearbeiter; Writer-Profil unverändert. Der Worker-Vertragstest prüft den gesetzten Cache.

Der Link-Score-Publisher stempelte zuvor für jeden geschriebenen Score zusätzlich die breite urls-Zeile. Der öffentliche Änderungsfeed vereinigt bereits urls, url_counters und url_ext; neue Scores besitzen ihren eigenen change_seq. Nur beim Entfernen eines alten Scores ohne Ersatz ist der zusätzliche urls-Stempel nötig. Folgekorrektur: nur solche entfernten Knoten stempeln. Werte, q_-Sichten, Read-API-Version und atomare Transaktion bleiben gleich. Das vermeidet das erneute Schreiben der vollständigen urls-Blätter bei der Score-Veröffentlichung. Gesamte Nachherreihe wird mit diesen Korrekturen erneut ausgeführt; dieser Ordner ist keine abschließende Abnahme.
