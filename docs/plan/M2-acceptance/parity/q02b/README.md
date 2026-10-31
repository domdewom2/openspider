# Q02b-Belege

- `lauf-final.json.gz`: vollständiger nativer K2-96-Endlauf, **218/218 grün**, drei begründete Skips. `lauf-final-vor-k296.json.gz` erhält den vorherigen 212/217-Stand; Zwischenläufe bleiben getrennt.
- `coverage-before-k296.json` / `coverage-after.json`: native SQ-01-Auswertung, jeweils **129/155**; `verified-je-tab.json` gruppiert ausschließlich die nativen verified-IDs.
- `original-23.json`: alle 23 Ausgangsexporte geschlossen. `restbefunde.json` ist leer; `restbefunde-vor-k296.json` erhält die zehn früheren Aufträge mit Spec-ID, Gruppe, Datei und Beispiel.
- `m1-k68.json` und `m1-mappings/`: **38/38** neue native A3-Diffs mit historischen M1-Mappings, null Word-Count-Fehler. Der vorherige Nachlauf bleibt separat erhalten.
- `pixel-k296.json`: 1.021 Titelgleichungen über alle zehn Sites, 297 verbessert, keiner verschlechtert; `/`=8 px, `.`=5 px bei 20 px Regular, Ziffern und andere Profile unverändert. 936 nichtleere Titel zusätzlich gegen echte CLI-Endwerte geprüft; 85 leere Titel haben dort keine numerische Breite. `pixel_measure.rs` dokumentiert die native Messquelle über den os-parity-Leser (temporär als Beispielprogramm ausgeführt, keine direkte CSV-Auswertung außerhalb os-parity).
- `internal-urlsets.json`: Internal-All-Mengen und native DEV-Belege aus dem K2-96-Endlauf. `endzahlen.json` enthält die vollständigen Zähler/Skips.
- `entscheidungen-q02b.md`: alle Entscheidungen und K2-63-Begründungen; `messfehler-crawl-path.md`: freigegebener Runner-/Layoutfix. Die erste reale deep8-Probe und der Endlauf zeigen neun Knoten → acht identische Verbindungen, 100 %.
- `links-detail.json`, `images-detail.json`, `other-populations.json`: eng begrenzte DEV-Populationsbelege. Historische diagnostische Pixel-/Hreflang-Proben sind keine bestandenen Produktvergleiche.
- `workspace-summary.json` und die Workspace-Logs: **Exit 0, 536 Targets / 3.988 bestanden / 0 Fehler / 43 bestehende Ignores**, vollständiger Gesamtbefehl mit C5. `workspace-k296-final.log.gz` ist der aktuelle Nachweis; erste/zweite C5-Läufe sind historische korrigierte Fehlversuche..
- `ohne-referenz.json`: K2-14-Liste mit nativen getaggten Fixture-Belegen, ohne verified-Gutschrift. Der Workspace-Prozessstatus bleibt separat ausgewiesen.
- `checks-final.json` / `cleanup.json`: Abschlussprüfungen und Entfernung ausschließlich eigener Build-/RAM-Profilverzeichnisse.

Logs sind gzip-komprimiert. Ältere Logs/Fehlversuche bleiben historische Belege und werden nicht als aktueller grüner Nachweis ausgegeben. Referenzdateien und Export-Schwellen sind unverändert; Test-Erwartungsänderungen sind ausschließlich nach K2-63/K2-96 begründet.

## Wiederholung

In der Repository-Wurzel, mit installiertem Chromium in `OPENSPIDER_CHROMIUM`:

```sh
timeout 3600s nice -n 10 env CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_PROFILE_TEST_DEBUG=line-tables-only cargo build -p os-cli -p os-parity -j 6 --locked
timeout 3600s nice -n 10 env OPENSPIDER_SECRET_STORE=memory OS_PARITY_MAX_EXAMPLES=10000 target/debug/os-parity run --all --openspider target/debug/openspider --out target/q02b-k296-final
target/debug/os-parity coverage --parity-report target/q02b-k296-final/report.json --write docs/plan/M2-acceptance/parity/q02b/coverage-after.json
python3 docs/plan/M2-acceptance/parity/q02b/auswertung.py target/q02b-k296-final/report.json
python3 docs/plan/M2-acceptance/parity/q02b/m1_nachlauf.py target/q02b-k296-final/report.json
```

Die Helfer erzeugen keinen eigenen Paritäts-/verified-Status; sie werten native Journale aus bzw. rufen `os-parity diff` auf. Nur os-parity liest Referenzdateien. B3f verwendet `os-parity coverage --milestone M2 --wave 4 --test-output …` und ergänzend den M1-Modus für dort nicht geführte M1-IDs.

`target_guard.py` ist die ursprüngliche Artefakthilfe; der K2-96-Nachlauf verwendet dieselbe Logik mit eigenem Logpfad und getrennten RAM-Verzeichnissen. Ausschließlich fertige eigene Programme werden vor dem ersten Test bytegleich ausgelagert; erst nach Ende eines Programms wird es entfernt. Binärverzeichnisstruktur und Verweise auf CLI-/Beispielprogramme bleiben erhalten; kein Strippen und keine Teständerung.

Die zusätzliche M2-Code-Coverage für die B3f-Zuordnung ist kein vollständiges B2-/UI-Gate: Exit 1 mit 29 offenen IDs, vier bestehenden ungültigen Ignore-Gründen und 62 offenen Anhang-B-Zuordnungen. `spec-coverage-k296.log.gz` erhält die vollständige Diagnose; alle 217 K2-14-IDs sind separat vollständig belegt. B3g-Filter-Coverage: Exit 0, 129/155.
