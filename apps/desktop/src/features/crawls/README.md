# M2-U15 – Crawl-Verwaltung

Umgesetzt: UI-124 sowie die UI-Anteile von OUT-228, CRAWL-127,
CRAWL-143, OUT-191 [M2] und OUT-230 [M2]. Clean-Room ausschließlich
auf Grundlage der Projekt-Spec. Keine neuen Abhängigkeiten.

`CrawlsHost` ist im Hauptfenster eingebunden. Datei → Crawls verwalten…,
Öffnen, Speichern unter, Importieren, Exportieren und Zuletzt geöffnet
verwenden dieselben Aktionen wie die Oberfläche. Die Verwaltung zeigt
Projektbaum und sämtliche elf Metadatenspalten. Suche über Name, Projekt
und Start-URL, Projektfilter und Crawl-/Vergleichsfilter wirken kombiniert;
jede Spalte ist sortierbar. Die Metadaten kommen aus `crawls.list`, ohne
Crawl-Dateien im Frontend zu öffnen.

Bis zu 1.000 Einträge können über Seiten hinweg ausgewählt werden.
100 Zeilen je Seite begrenzen den DOM-Umfang. Checkboxen, Umschalt-Auswahl,
Mod+A, Mod+F, Umschalt+F10, Kontextmenütaste und Entf sind erreichbar.
Kopieren übergibt IDs in sichtbarer Tabellenreihenfolge. Gesperrte Projekte
schützen auch ihre enthaltenen Crawls. Laufende Crawls lassen sich weder
löschen, umbenennen, verschieben noch exportieren. Fortsetzen öffnet den
gewählten pausierten/unterbrochenen Crawl vor `crawl.resume`.

Löschen bestätigt Anzahl, Speicherplatz und Unwiderruflichkeit. Benennen,
Verschieben, Projekte anlegen/umbenennen/sperren, Entsperren, Duplizieren,
Dateimanager, Import/Export und Auswahl zweier Vergleichspartner verwenden
die vorhandenen IPC-Verträge. Ein laufender aktiver Crawl wird nur nach
Bestätigung pausiert, bevor ein anderer geöffnet wird. Native Dateidialoge
können abgebrochen werden, ohne einen Datei-Auftrag zu erzeugen.

Lang laufende Aktionen verwenden `OpHandle` und den bestehenden Tick-Store.
Fortschritt, Abbruch, Erfolg und Fehler bleiben nach Schließen der Verwaltung
sichtbar. Reparaturen laufen ohne Abbruchmöglichkeit in der Schreibwarteschlange,
da die vorhandene SQLite-Reparatur nicht sicher unterbrechbar ist. Sie
schreiben ausschließlich eine neue `.repaired.ospider` und zeigen
Integrität, Verwendbarkeit und übersprungene Zeilen im Ergebnisdialog.
Dateiformat- und Versionsfehler nennen Dateiname und Ursache; neuere Schemata
zeigen einen Aktualisierungshinweis. Beschädigte Dateien bieten die Reparatur an.
Die Konfiguration wird über einen kurzlebigen Lesepool-Slot ausgelesen und
nur auf ausdrückliche Aktion an `config.set` übergeben.

Zuletzt geöffnet: zehn eindeutige Pfade, neueste zuerst, persistent im
Frontend-Speicher. Nur tatsächlich fehlende Dateien werden nach Anklicken
mit Hinweis entfernt. Ein `Io`-Fehler der Pfadkanonisierung wird dafür über
den schreibgeschützten Dateiadapter nachgeprüft; Berechtigungs-, Format-
und Versionsfehler löschen keinen Verlaufseintrag.

## Fremdergänzungen (K2-61)

Der vollständige Diff liegt in [K2-61.patch](K2-61.patch). Alle Ergänzungen
lassen bestehende Signaturen und das bisherige Verhalten bei unveränderten
Aufrufern bestehen. Keine fremden Tests oder Goldens wurden geändert.

| Datei | Ergänzung |
| --- | --- |
| `apps/desktop/src/App.tsx` | Import und zusätzlicher Aufruf von `CrawlsHost`. |
| `apps/desktop/src/ui/menu/types.ts` | Optionales Feld `MenuItemSpec.literalLabel`; Default ist weiterhin `labelKey`. |
| `apps/desktop/src/ui/menu/build.ts` | Zusätzliche bedingte Zuweisung zur Übernahme einer wörtlichen Beschriftung, nur wenn das neue Feld gesetzt ist; benötigt für Dateipfade im Verlauf. |
| `crates/os-app/src/commands/mod.rs` | Zusätzliche Moduldeklaration `crawl_files`. |
| `crates/os-app/src/commands/crawl_files.rs` (neu) | Adapterfunktionen für bestehende Store-Integritätsprüfung, eingebettete Konfiguration und Reparatur als Operation; zwei eigene Fixturetests. |
| `apps/desktop/src-tauri/src/crawl_files.rs` (neu) | Dünne native Adapter `crawl_file_check`, `crawl_file_config`, `crawl_file_repair`. |
| `apps/desktop/src-tauri/src/lib.rs` | Zusätzliche Moduldeklaration und drei zusätzliche Einträge in der bestehenden Invoke-Registrierung. |

Die fehlenden Dateifunktionen werden als zusätzliche native Commands
angeboten. Der eingefrorene Service-Katalog und seine bestehenden Commands
bleiben unverändert. Die native Hülle enthält keine Datenbanklogik;
SQL-Arbeit läuft über Lesepool, Schreibwarteschlange bzw. die bestehende
Store-Prüffunktion auf einem Blocking-Thread.

K2-60: keine Stub-Ablösung durch U15. K2-63: keine Erwartungsänderungen.
Keine fremden Fixtures oder Goldens neu erzeugt.

## Prüfungen

- Fixture `fixtures/management.json`: drei Projekte und sieben Crawls mit
  unterschiedlichen Zuständen; daraus auch 1.000 Metadaten für die Auswahlprüfung.
- Fixture `fixtures/fake.ospider`: absichtlich ungültige Datei.
- Vitest: Menü, alle Spalten, Filter, ID-Reihenfolge, Rechte, Löschen,
  Umbenennen/Projekte, Verschieben/Duplizieren/Export, Dateidialog-Abbruch,
  Verlauf, Reparatur, Integrität/Konfiguration, Sperren, 1.000er-Auswahl,
  Speichern unter und Pause vor Öffnen.
- Playwright: dieselben Vertragsabläufe einschließlich tatsächlicher
  Tastatur-/Fokusführung in Chromium und WebKit.
- Echte Bridge: Crawl gegen lokalen Fixture-HTTP-Server, Auflisten,
  Umbenennen, unveränderte Kennung, Export und Prüfung des Dateiformats.
- Rust: Fremddatei unverändert ablehnen; reale Crawl-Datei über die
  Schreibwarteschlange erzeugen, Konfiguration lesen, Reparatur-Operation
  und erneute Integritätsprüfung, Quelle und vorhandene Reparaturkopie schützen.

| Prüfung | Ergebnis |
| --- | --- |
| `npm ci` | Erfolgreich; 0 Audit-Funde. Bestehende Engine-Warnungen von vier Abhängigkeiten bei Node 22.20.0. |
| `npm run -w apps/desktop typecheck` | Grün. |
| `npm run -w apps/desktop test -- --maxWorkers=2` | 212 Tests grün, davon 21 eigene; 18,14 s. |
| `npm run -w apps/desktop check:static` | Grün: generierte Wrapper aktuell, 263 erlaubte npm-Lizenzen, 28 statische/Token-Tests. |
| `npm run -w apps/desktop e2e -- --config e2e/crawls/playwright.config.ts` | 14 Tests grün, Chromium und WebKit; 10,3 s; davon zwei echte Bridge-Läufe. |
| `cargo fmt --all --check` | Grün. |
| `cargo clippy --workspace --exclude openspider-desktop --all-targets -- -D warnings` | Grün. |
| `cargo clippy -p openspider-desktop --all-targets -j 2 -- -D warnings` | Grün, einschließlich der neuen nativen Adapter; 2 min 33 s. |
| `cargo test --workspace --exclude openspider-desktop -j 2` | 3030 Tests grün, 89 vorhandene Ignores; einschließlich beider neuen Rust-Fixturetests und aller Doctests. |
| `bash scripts/check-neutral.sh` | Keine Fundstellen; vier bestehende verwaiste Allowlist-Einträge werden angemahnt. |

Der erste breite Frontend-Lauf überschritt bei gleichzeitiger Build-Last
mehrere Test-Zeitgrenzen (auch im bestehenden U10-Test). Begrenzung auf zwei
Worker ließ die gesamte Suite ohne Änderung fremder Tests durchlaufen.
Der erste Rust-Linklauf scheiterte an vollem Datenträger; ausschließlich
die erzeugten ausführbaren Dateien im eigenen `target-u15` wurden verkleinert
bzw. fehlgeschlagene Artefakte neu gebaut. Der Wiederholungslauf verwendet
zwei Build-Jobs. Ein unvollständiges, nicht ausführbares Testartefakt aus
dem fehlgeschlagenen Linklauf wurde ebenfalls entfernt und neu gebaut;
anschließend wurde der gesamte Workspace-Lauf nochmals gestartet. Keine Testlogik, Abhängigkeit oder Repository-Konfiguration
wurde zur Umgehung dieser Ressourcenprobleme geändert.

## Abweichungen und Grenzen

Keine fachliche Abweichung im M2-UI-Umfang. Projekt-ZIP-Export und benannte
gespeicherte Vergleichsreferenzen einschließlich Löschwarnung bleiben
P2 gemäß M2.md/K2-32 und OUT-228. Der Vergleichsfilter ist vorhanden;
die aktuelle Service-Schicht liefert noch keine gespeicherten
Vergleichsreferenzen. Aufbewahrung, Absturzsicherheit, ID-Vergabe,
Dateimigration und portable Dateiinhalte liefert O02 bzw. die vorhandene
Crawl-/Store-Schicht. Diese Fachlogik wird im Frontend nicht nachgebildet.

Der zusätzliche Besitzcheck umfasst 27 geänderte Dateien: eigene Pfade sowie
die sieben oben dokumentierten Fremdergänzungen. Der vollständige Fremddiff
ist mit `git apply --reverse --check --unidiff-zero` gegen den Endstand geprüft.

Native OS-Dateifenster werden über den vorhandenen IPC-Transport geöffnet;
die Browsertests geben deterministische Pfade vor. Die Grenzwerte für
1-Mio.-URL-Dateien und Performance auf der Referenzhardware wurden hier
nicht neu gemessen. Keine offenen fachlichen Fragen.

Kein Push, Merge oder PR. CI-Lauf-ID: keiner (lokale Prüfungen).

Adapter-Commit: `ebb8c2d`. Oberflächen-Commit: `9362ccb`. Der abschließende
Commit enthält Dateifehler-Hinweise, Browserprüfungen und diesen Bericht.
