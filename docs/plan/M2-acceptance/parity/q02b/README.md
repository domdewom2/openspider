# Q02b-Belege

- `lauf-final.json.gz`: vollständiger letzter nativer Paritätslauf, 212/217 grün, fünf rot, vier Skips. Frühere Läufe und unterbrochene Versuche bleiben getrennt erhalten.
- `coverage-before.json` / `coverage-after.json`: native SQ-01-Auswertung, jeweils 129/155; `verified-je-tab.json` gruppiert ausschließlich deren verified-IDs.
- `original-23.json`: alle 23 Ausgangsexporte, davon 18 geschlossen. `restbefunde.json`: zehn offene Einzelbefunde mit Spec-ID, Gruppe, Datei, Beispiel und Fix-Auftrag.
- `m1-k68.json` und `m1-mappings/`: 38 neue native A3-Diffs mit den historischen M1-Mappings; null Word-Count-Fehler.
- `workspace-summary.json`: 536 Targets, 3977 bestanden, ein Fehler, 43 bestehende Ignores. **Kein grüner Workspace-Gesamtbefehl:** Hauptlauf Timeout 124; die 36 noch nicht abgeschlossenen Testprogramme und 17 Doc-Test-Targets wurden separat grün nachgemessen. Der INTEG-009-Fehler bleibt rot. `workspace-tail.json` nennt die echten Programme, Rückgabewerte und Laufzeiten; die Nachläufe ersetzen keine abgeschlossenen Fehler.
- `ohne-referenz.json`: 217 K2-14-IDs mit mindestens einem positiven getaggten Fixture-Test; 216 native Statusbelege vollständig, INTEG-009 offen. Native Code-Coverage-M1/M2-Journale sind separat archiviert; keine vollständige B2/UI-Abnahme behauptet.
- `links-detail.json` / `images-detail.json`: eigene CLI-Filter-/Instanzexporte und native Vorbelege für DEV-054…056. `pixel-boundary.json` enthält eine ausdrücklich **diagnostische**, künstlich leer gesetzte OS-Messspalte zur nativen Extraktion der Referenzbreiten; deren 142 erwartete Probefehler sind keine Produktbefunde. Die 90 echten Grenzübertritte 202/199 px erklären den verbleibenden Overview-Zähler.
- `checks-final.json` / `cleanup.json`: Abschlussprüfungen und Entfernung ausschließlich eigener Build-/RAM-Profilverzeichnisse.

Logs sind gzip-komprimiert. Rust-/Frontend-Testquellen, Referenzdateien und Export-Schwellen wurden nicht verändert. Die zwei Pixel-Mappings folgen der vorhandenen UI-038/K-64-Regel ≤3 %, wie Internal/Page Titles; die Zählerprüfung bleibt 100 %.

## Wiederholung

In der Repository-Wurzel, mit installiertem Chromium in `OPENSPIDER_CHROMIUM` und ausreichend freiem Platz:

```sh
timeout 3600s nice -n 10 env CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo build -p os-cli -p os-parity -j 6 --locked
timeout 3600s nice -n 10 env OPENSPIDER_SECRET_STORE=memory OS_PARITY_MAX_EXAMPLES=10000 target/debug/os-parity run --all --openspider target/debug/openspider --out target/q02b-final4
target/debug/os-parity coverage --parity-report target/q02b-final4/report.json --write docs/plan/M2-acceptance/parity/q02b/coverage-after.json
python3 docs/plan/M2-acceptance/parity/q02b/auswertung.py target/q02b-final4/report.json
python3 docs/plan/M2-acceptance/parity/q02b/m1_nachlauf.py target/q02b-final4/report.json
```

Die Helfer schreiben **keinen** eigenen Paritäts- oder verified-Status. Sie werten native Journale aus bzw. rufen `os-parity diff` auf. Nur `os-parity` liest die Referenzdateien. B3f nutzt native Code-Coverage-Ergebnisse; dafür die archivierten Hauptlauf-Logs vorübergehend nach `target/` entpacken und Code-Coverage mit `--milestone M2 --wave 4` sowie ergänzend M1 erzeugen.

`target_guard.py` ist ausschließlich eine Server-Artefakthilfe. Vor Testbeginn kopiert sie fertige Testprogramme bytegleich in eigens angelegte ausführbare RAM-Profile, einschließlich Verweisen auf die realen Profil-Binaries und Beispiele. Sie strippt nichts und löscht nur nachweislich abgeschlossene Testprogramme. Die erzeugten RAM-Pfade stehen in den Cleanup-Belegen; fremde Prozesse/Targets werden nicht angefasst.
