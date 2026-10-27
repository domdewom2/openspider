# CI-Browsertests

## Analyse und Änderung

- `timeout_keeps_parallel_session_available`: Das Seitenbudget umfasst bereits Kontext- und Tab-Erzeugung sowie den Aufbau des Steuerkanals. Die bisherigen 1,5 Sekunden können vor Erreichen des Test-Hooks verstreichen; dann liefert der Auftrag `Timeout` und kein zweites Ziel. Der gehaltene Nachbartab belegt einen von drei Semaphore-Plätzen und einen Lesezugriff auf die Sitzung; bei deaktiviertem Recycling wird kein Schreibzugriff benötigt. Das erklärt das beobachtete Warten auf dem Kanal, beweist aber nicht allein den genauen Ablauf auf dem CI-Runner. Das Budget beträgt nun 15 Sekunden, das des gehaltenen Tabs 120 Sekunden. Der Test verlangt weiterhin den Timeout des hängenden Tabs, einen erfolgreichen parallelen Folgerender und dieselbe Browsergeneration.
- `denies_downloads_in_each_context`: Der Listener wird vor der Navigation registriert, aber der Test wartete erst nach `render()` auf den Abbruch. `render()` entsorgt vorher den BrowserContext; der durch das Load-Ereignis ausgelöste Download kann noch ausstehen. Das 3-Sekunden-Fenster und das Standard-Seitenbudget sind auf belasteten Runnern zusätzlich knapp. Ein ausschließlich im Test vorhandener Abschluss-Hook hält den Kontext nach der Renderarbeit bis zur Ereignisbeobachtung offen. Zwei aufeinanderfolgende Renderings prüfen zwei getrennte Kontexte und zwei verschiedene abgebrochene Downloads; abgeschlossene Downloads bleiben ein Fehler. Das Wartefenster ist endlich.

Ein Produktfehler wurde aus den vorliegenden Logs nicht nachgewiesen. Die Konfiguration `Deny` mit aktivierten Ereignissen erfolgt bereits vor der Navigation für jeden Kontext. Die Plattformursachen sind anhand der Logs nicht abschließend verifiziert; ein CI-Lauf auf Windows und macOS bleibt erforderlich.

## Lokaler Nachweis

macOS arm64, Google Chrome unter `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome` via `OPENSPIDER_CHROMIUM`; CI verwendet die System-Browsersuche des Renderers. `cargo fmt --all --check`, Workspace-Clippy und `cargo test -p os-render` erfolgreich. Beide betroffenen Tests liefen zusätzlich je fünfmal erfolgreich mit zwei CPU-Lastprozessen. Ein vorheriger reproduzierbarer Fehlschlag wurde lokal nicht nachgewiesen; die Ursachenbewertung stützt sich auf Code und CI-Logs. Der vorhandene Screenshot-Benchmark bleibt unverändert ignoriert.

Der unveränderte Ausgangsstand wurde zusätzlich mit `cargo test -p os-render --lib requires_chromium -- --test-threads=4` und zwei CPU-Lastprozessen ausgeführt: 9 Tests grün, keine lokale Reproduktion. Nach der letzten Änderung: Format, Workspace-Clippy, `cargo test -p os-render` und `cargo test -p os-cli` grün; der Download-Test zusätzlich fünfmal grün. Desktop-Typecheck und -Tests: grün (75 Dateien, 768 Tests).

`cargo test -p os-app -p os-cli` scheiterte beim Linken von `os-app` mit `No space left on device`; vor dem Lauf waren etwa 12 GiB frei. Die von diesem Lauf erzeugten App-Testprogramme wurden entfernt. `cargo test --workspace` konnte wegen derselben Platzgrenze nicht sinnvoll durchgeführt werden. Die Plattformläufe in CI stehen aus.
