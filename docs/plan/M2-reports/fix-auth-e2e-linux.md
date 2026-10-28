# fix-auth-e2e-linux

## Ursache und Einordnung

Der bisherige Test (`apps/desktop/e2e/auth/bridge.spec.ts`, ursprüngliche Zeilen 10–25 und 64) errät das Profil durch Auflisten von Node `os.tmpdir()`, filtert `openspider-chromium-*` und erwartet darin `DevToolsActivePort`. `crates/os-render/src/login.rs:134–137` verwirft jedoch jeden vorgegebenen Profilpfad; `crates/os-render/src/browser.rs:68–88,163–188` erzeugt das Profil unter Rust `std::env::temp_dir()` und übergibt es explizit als `user_data_dir`. Der Präfix ist auf allen Plattformen gleich. Node und Rust bestimmen ihr temporäres Verzeichnis unabhängig: auf Unix verwendet Rust `TMPDIR`, Node berücksichtigt auch `TMP` und `TEMP`; auf Windows gelten andere Systemregeln. Gleiche Verzeichnisse sind daher nicht garantiert. XDG-Profilvorgaben sind hier wegen des expliziten Pfads nicht maßgeblich.

Chromiumoxide 0.9.1 verwendet standardmäßig `--remote-debugging-port=0` und keine Debugging-Pipe; der tatsächliche WebSocket wird nach dem Start von Chromiumoxide ermittelt (`crates/os-render/src/browser.rs:356`). Der Test war zusätzlich vom Vorhandensein und Fundort der von Chromium geschriebenen Portdatei abhängig. Welcher dieser Umstände auf dem Linux-Runner konkret zutraf, lässt sich aus dem Timeout und dem Snapshot allein nicht belegen; insbesondere beweist die UI-Anweisung keinen erfolgreichen Browserstart, da sie während der laufenden Operation angezeigt wird. Die Linux-spezifische Ursache ist damit nicht abschließend nachgewiesen.

Headless ist nur bei aktivem Bridge-Feature und gesetztem `OPENSPIDER_LOGIN_HEADLESS` eingeschaltet (`crates/os-app/src/commands/auth_login.rs:29–35`); der Renderer nutzt Chromium `--headless=new` (`browser.rs:211–214`). Sandbox bleibt standardmäßig aktiv (`browser.rs:216–218`, `api.rs:95`), Chromiumoxide setzt standardmäßig `--disable-dev-shm-usage`; Unix-Profile werden mit 0700 erzeugt (`browser.rs:77–85`). Der CI-Kontext belegt weder einen Sandbox-, Anzeige-, /dev/shm- noch einen Rechtefehler. Keine Änderung dieser Einstellungen ist für die direkte Endpunktermittlung nötig.

## Änderung

Nach erfolgreichem Login-Start schreibt ausschließlich der mit `e2e-bridge` gebaute Prozess bei explizit gesetztem `OPENSPIDER_LOGIN_CDP_FILE` den tatsächlich verwendeten Browser-WebSocket in eine vom Test vorgegebene Datei. Der Test erstellt dafür ein privates temporäres Verzeichnis, entfernt die Datei vor jedem Login, prüft weiterhin das Ziel auf den eigenen Loopback-Origin und räumt anschließend auf. Damit entfallen Suche, Präfix- und Portdateiannahmen auf Linux, macOS und Windows; die normale Produktausführung bleibt unverändert. Eine Linux-Ausführung ist nicht verfügbar, der Nachweis folgt durch die CI des Orchestrators.

## Lokale Prüfung auf macOS

- `cargo build -p os-app --bin os-app-bridge --features e2e-bridge --locked`: erfolgreich.
- `npm run -w apps/desktop e2e -- e2e/auth --project=chromium --project=webkit --workers=1`: 4 Tests erfolgreich.
- `cargo fmt --all --check` und `cargo clippy -p os-render -p os-app --all-targets -- -D warnings`: erfolgreich.
- Ein Testziel `login` existiert nicht; `cargo test -p os-render --test login_capture`: 4 Tests erfolgreich.
- `cargo clippy --workspace --all-targets -- -D warnings`, `npm run -w apps/desktop typecheck` und `npm run -w apps/desktop test`: erfolgreich (771 UI-Tests).
- `cargo test --workspace`: nicht abgeschlossen; das Linken scheiterte zweimal wegen erschöpften Speicherplatzes (`errno=28`), auch mit `CARGO_PROFILE_TEST_DEBUG=0` und `CARGO_INCREMENTAL=0`. Die erzeugten Build-Artefakte wurden anschließend mit `cargo clean` entfernt. Der Workspace-Test muss auf einer Maschine mit ausreichend Platz wiederholt werden.
