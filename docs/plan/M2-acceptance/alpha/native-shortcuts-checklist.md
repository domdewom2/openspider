# Alpha-Abnahme: native Tastenkürzel ⌘/Strg+C, +A, +V in der echten App (K2-65)

Manuelle Prüfung durch den Orchestrator, je einmal auf **macOS** (⌘) und **Windows** (Strg) mit der gebauten Tauri-App (nicht Playwright,
nicht E2E-A). Hintergrund: Die Kürzel-Logik ist in E2E-A auf allen Plattformen grün; der native Nachweis scheiterte bisher daran, dass der
Prüfaufbau die frisch gestartete App nicht in den Vordergrund bringt (K2-65, `docs/plan/M2-spikes/S-3.md`). Spec: UI-012 (Kopieren,
Auswahl), UI-002 (Menü, Kürzel), UI-003 (URL-Feld).

## Vorbereitung

1. Fixture-Server starten: `cargo run -p os-testsite -- ui/fx-ui-basic` (gibt `http://127.0.0.1:<Port>` aus), App starten
   (`npm run -w apps/desktop tauri dev` oder das gebaute Programm) und das Fenster anklicken, damit es aktiv ist.
2. Crawl der Fixture-URL über die Kopfleiste (Enter im Feld „Start URL“); Tab **Response Codes** öffnen (27 Zeilen).
3. Zum Gegenprüfen der Zwischenablage ein Textfeld außerhalb der App bereithalten (Editor oder Notizen).

## Prüfschritte

| # | Schritt | Erwartung | macOS | Windows |
|---|---|---|---|---|
| 1 | Feld „Start URL“: Text eintippen, ⌘/Strg+A, dann ⌘/Strg+C | gesamter Text markiert; im Fremdeditor eingefügt erscheint genau dieser Text | ☐ | ☐ |
| 2 | Feld „Start URL“: ⌘/Strg+V mit einer URL aus dem Fremdeditor, die den markierten Inhalt ersetzt | URL steht einmal (nicht doppelt) im Feld; „Start“ wird aktiv | ☐ | ☐ |
| 3 | Modus „List“, Feld „Paste URLs“: ⌘/Strg+V mit drei Zeilen | alle drei Zeilen erscheinen unverändert, ein Einfügevorgang | ☐ | ☐ |
| 4 | Suchfeld der Tabelle: ⌘/Strg+V, Enter | Suche wird ausgelöst, Zähler „Showing: N of 27“ ändert sich; ⌘/Strg+A markiert den Feldtext | ☐ | ☐ |
| 5 | Tabelle: eine Zeile anklicken, ⌘/Strg+A | alle Zeilen der Ansicht markiert (Zähler „27 selected“), auch die nicht sichtbaren | ☐ | ☐ |
| 6 | Tabelle: zwei Zeilen markieren (Umschalt+Klick), ⌘/Strg+C, im Fremdeditor einfügen | Tabulator-getrennter Text, erste Zeile = Spaltenköpfe der sichtbaren Spalten, dann die beiden Zeilen; Zeilenende `\n` (macOS) bzw. `\r\n` (Windows) | ☐ | ☐ |
| 7 | Tabelle: ⌘/Strg+C ohne Markierung | Zwischenablage unverändert, keine Fehlermeldung | ☐ | ☐ |
| 8 | Detailfenster, Reiter „HTTP Headers“ oder „URL Details“: Wert markieren, ⌘/Strg+C | Wert im Fremdeditor | ☐ | ☐ |
| 9 | Menü „Bearbeiten“ (macOS) bzw. Kontextmenü in einem Textfeld (Windows) | Einträge Kopieren, Einfügen, Alles auswählen tragen die Kürzel und wirken wie die Tasten | ☐ | ☐ |
| 10 | Einstellungsfenster (Datei → Einstellungen), ein Textfeld: ⌘/Strg+A, C, V | wie Schritt 1 und 2, das Hauptfenster reagiert nicht mit | ☐ | ☐ |

Immer mitprüfen: Kürzel wirken **genau einmal** (kein doppeltes Einfügen durch Menü und Seite gleichzeitig), nur im aktiven Fenster und
nur im fokussierten Element; nach dem Wechsel in ein anderes Programm und zurück funktionieren sie weiter.

## Ergebnis eintragen

| Feld | macOS | Windows |
|---|---|---|
| Datum, Prüfer | | |
| System und Version | | |
| App-Stand (Commit, Build) | | |
| Ergebnis (alle Schritte grün / Abweichungen) | | |
| Abweichungen (Schritt, Beobachtung, Screenshot-Pfad) | | |

Abweichungen kommen als Befund nach `befunde.md` (Datei, Reproduktion, Spec-ID, Plattform, Schwere).
