# M2-U15 – Crawl-Verwaltung

Der verbindliche Paketbericht mit Fables Befunden, Prüfständen,
Fremdergänzungen und offenen Integrationspunkten steht unter
[docs/plan/M2-reports/u15.md](../../../../../docs/plan/M2-reports/u15.md).

Umgesetzt sind UI-124 sowie die UI-Anteile von OUT-228, CRAWL-127,
CRAWL-143, OUT-191 [M2] und OUT-230 [M2]. Grundlage ausschließlich
`docs/spec/*` und die verbindlichen Planentscheidungen; keine neuen Abhängigkeiten.

`CrawlsHost` bindet Datei → Crawls verwalten…, Öffnen, Speichern unter,
Import/Export und die zehn zuletzt geöffneten Dateien ein. Projektbaum
und elf Metadatenspalten verwenden `crawls.list` und `projects.list`.
Suche, Projekt-/Typfilter und Sortierung berücksichtigen den gesamten
Metadatenbestand. 100 Zeilen je Seite begrenzen die Darstellung;
Checkbox, Umschalt-Auswahl und Mod+A wählen höchstens 1.000 Crawls aus.

Fortsetzen verwendet ausschließlich `canResume` aus der gemeinsamen
Kernprüfung nach K2-34. Damit sind auch abgeschlossene Crawls mit
Restwarteschlange fortsetzbar. Der gewählte Crawl wird zuerst geöffnet.
Laufende und gesperrte Crawls haben die vorgesehenen Aktionseinschränkungen.
Formularfehler erscheinen im aktiven Formular und erhalten die Eingaben.

**Offene Vergleichsintegration:** `compare.open` liefert im tatsächlichen
Kern weiterhin `Unsupported` (M2-C01). „Zum Vergleich wählen“ ist deshalb
in Toolbar und Kontextmenü mit übersetzter Begründung deaktiviert.
Der erfolgreiche Mock ist entfernt; ein Test über die echte Bridge
prüft diesen Zustand mit zwei realen Crawl-Dateien. Die vollständige
OUT-228-Vergleichsaktion bleibt bis zur Integration von C01/U27 offen.
Der vollständige [C9-Test](../../../e2e/crawls/compare.pending.spec.ts)
prüft danach Aktivierung, UI-Aufruf und echte Kernantwort; N02 entfernt das
Fixme nach der Fachintegration.

Dateiaktionen laufen über die bestehenden IPC-Verträge und die ergänzten
nativen Dateiadapter. Operationen zeigen Fortschritt, Abbruch und Ergebnis
auch nach Schließen der Verwaltung. Reparaturen schreiben eine neue Datei
und melden Integrität, Verwendbarkeit und übersprungene Zeilen. Die
Konfiguration wird nur auf ausdrückliche Aktion übernommen. Tatsächlich
fehlende Dateien werden beim Anklicken aus dem Verlauf entfernt;
Berechtigungs-, Format- und Versionsfehler erhalten den Eintrag.

Der vollständige Fremddiff im
[Paketbericht](../../../../../docs/plan/M2-reports/u15.md#fremdergänzungen-k2-61)
belegt die sieben additiven Fremdergänzungen der ursprünglichen Umsetzung.
Diese Fix-Runde verändert keine fremde Implementierung, Tests, Goldens,
Manifeste oder generierten IPC-Dateien.
Der angeforderte Bericht ist die einzige bearbeitete Datei außerhalb von `owns`.
Er benennt außerdem die drei regulären Crawl-Datei-Commands für N02 und
den noch offenen Windows-Kopierfehler aus dem R5-CI-Nachweis.

Fixtures: `fixtures/management.json`, `fixtures/fake.ospider` und daraus
abgeleitete 1.005 Metadaten. Vitest prüft Verträge und Grenzfälle;
Playwright prüft Tastatur/Fokus in Chromium und WebKit sowie reale
Kernantworten über `os-app-bridge`. Prüfkommandos und Ergebnisse stehen
im Paketbericht. Native OS-Dateidialoge und Referenzhardware-Großläufe
werden hier nicht als ausgeführt behauptet.
