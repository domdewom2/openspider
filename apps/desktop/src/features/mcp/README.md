# M2-U25 – MCP-Oberfläche

Die fünf Reiter, das native Menü und das Statusleistenfeld verwenden ausschließlich die typisierten `ipc/mcp`-Commands. Das Menü ist auch ohne Crawl und bei ausgeschaltetem Server vorhanden. Das Statusfeld öffnet dieselben Aktionen wie das native Menü; Zustand und Sitzungszahl kommen über den vorhandenen Fenster-Ereignisbus. Die App erzeugt keinen zweiten Ereigniskanal und startet keinen zweiten Server beim Mounten.

Umgesetzt: UI-129, AGENT-043 sowie die UI-Teile von OUT-285, AGENT-003 und AGENT-002. Tests tragen die jeweiligen `// spec:`-Tags; nach M2 Anhang B sind OUT-285 als `[P1]` und AGENT-003 als `[M2]` getaggt.

- Server: Start/Stopp, Portvalidierung, gespeicherter Port, Auto-Start-Schalter, Live-Status, Meldung bei Startfehlern. Portnummern werden ohne Tausendertrennzeichen angezeigt. Verspätete Abfrageantworten überschreiben keine neueren Statusereignisse. Fehlerhinweise aus `notice` und `tick.notices` werden für Fehlercodes im Namensraum `mcp-`, `mcp.` oder `mcp_` angenommen; beim Mounten werden bereits eingetroffene Notices berücksichtigt.
- Werkzeuge: getrennte Klassen- und Einzelfreigaben, Auslieferungszustand nur `read`, Beschreibung und unverändertes JSON-Schema aus dem Kern, Markdown-Speichern über den nativen Dateidialog mit Vorgabename `openspider-mcp-tools.md`. Abbrechen schreibt nichts. Klassen und deaktivierte Werkzeuge bleiben beim erneuten Öffnen erhalten.
- Zugang: Klartext wird ausschließlich nach „Anzeigen“ in lokalem Komponentenstatus gehalten. Kopieren ruft den Token nur für die Zwischenablage ab. Reiterwechsel, Schließen und Sichtbarkeitswechsel entfernen die Anzeige; verspätete Antworten nach dem Unmount werden verworfen. Regenerieren fordert `reveal: false` an und verwirft eine vorherige Anzeige. Fehlermeldungen geben keine Backend-Details oder Argumente aus.
- Clients: Claude Desktop, Claude Code und Codex; Vorschau mit Diff und resultierender Konfiguration; explizite Bestätigung vor dem Schreibcommand; erneutes Lesen direkt vor dem Schreiben, bei geändertem Inhalt neue Bestätigung; Sicherungspfad nach Erfolg. Kopieren ist ohne Schreibzugriff möglich. Token und andere Zugangsdaten werden aus der angezeigten Vorschau entfernt.
- Protokoll: Zeit, Client, Werkzeug, redigierte Parameter, Dauer, Ergebnisgröße und Erfolg; Seiten à 25 Zeilen, explizites Aktualisieren, Vor/Zurück und Leer-/Fehlerzustände. Geheimnisse werden zusätzlich zur Redigierung des Kerns vor der Anzeige entfernt.

## Fremdergänzungen (K2-61)

Der genaue Diff steht in [K2-61.patch](K2-61.patch). Der Nutzerauftrag erlaubt diese Ausnahme ausdrücklich auch für das vorliegende Welle-2-Paket. Ausschließlich additive Änderungen:

| Datei | Ergänzung | Grund |
| --- | --- | --- |
| `apps/desktop/src/App.tsx` | Import von `McpHost`/`McpStatusField`, zusätzlicher Host-Aufruf und Statusfeld | Einhängepunkt für Menü/Dialog und UI-129-Statusleiste fehlte. |
| `crates/os-app/src/dto/app.rs` | `McpSettingsDto.classes: Option<Vec<String>>` und `port: Option<u16>`; beide `serde(default)`, ohne Wert nicht serialisiert | Gespeicherte Klassen müssen von effektiven Einzelfreigaben unterscheidbar sein (AGENT-003); UI muss gespeicherten Port lesen können (OUT-285). Fehlen Klassen, verwendet die UI nur `read`; fehlt Port, bleibt er serverseitig wählbar. |
| `crates/os-app/src/dto/mcp.rs` | `CallLogRow.result_bytes: Option<u32>`, `serde(default)`, ohne Wert nicht serialisiert | Ergebnisgröße gemäß AGENT-043 fehlte im Vertrag. Alte Einträge zeigen „—“ statt einer erfundenen Größe. |
| `apps/desktop/src/ipc/gen/McpSettingsDto.ts` | Neu erzeugte optionale Felder `classes`/`port` | Ableitung ausschließlich der eigenen additiven DTO-Ausgabe. |
| `apps/desktop/src/ipc/gen/CallLogRow.ts` | Neu erzeugtes optionales Feld `resultBytes` | Ableitung ausschließlich der eigenen additiven DTO-Ausgabe. |

Der Generator lief in ein temporäres Verzeichnis; nur die beiden betroffenen Typdateien wurden übernommen. Keine fremden Stub-Einträge oder Testdateien wurden verändert. Die einzigen regenerierten Goldens sind die beiden oben genannten TypeScript-Dateien. K2-60 und K2-63 waren nicht nötig. Keine neuen Abhängigkeiten oder Manifeständerungen.

## Prüfungen

Die Vitest-Fixture `fixtures/agent-m2-ui.ts` stellt die bereits vorhandenen IPC-Verträge bereit; sie ist kein Bestandteil des Produktionspfads. Die Browser-Fixture unter `e2e/mcp` nutzt dieselbe Oberfläche und für Client-Konfigurationen ein temporäres Benutzerverzeichnis. Sie prüft tatsächliche Sicherungsdateien und unveränderte fremde Server-Einträge, ohne das echte Benutzerverzeichnis oder den OS-Schlüsselbund zu öffnen.

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --exclude openspider-desktop --all-targets -j 2 -- -D warnings
RUST_TEST_THREADS=1 OPENSPIDER_SECRET_STORE=memory cargo test --locked --workspace --exclude openspider-desktop --no-fail-fast -j 2
cargo deny check licenses bans sources
npm ci
npm run -w apps/desktop typecheck
npm run -w apps/desktop test -- --maxWorkers=2
npm run -w apps/desktop check:static
# aus apps/desktop:
npx playwright test --config e2e/mcp/playwright.config.ts --project=chromium --project=webkit --workers=2
```

Lokale Umgebung: macOS arm64, Rust 1.99.1, Node 22.20.0. Frontend: 209 Tests in 13,50 s, darunter 18 neue U25-Komponententests; Browser: 10 Tests in 8,8 s; statische Prüfungen: 28 Tests in 1,69 s plus aktuelle IPC-Wrapper und npm-Lizenzprüfung. `npm ci`, TypeScript, Formatierung, Clippy (2 min 52 s), `cargo deny` und die Neutralitätsprüfung sind grün.

Die ersten beiden Rust-Test-Builds scheiterten beim Linken an einem vollen gemeinsamen Datenträger, noch vor der Testausführung. Nur der eigene Cargo-Buildordner wurde bereinigt. Der erneute Workspace-Testlauf verwendet deshalb zusätzlich `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_PROFILE_DEV_OPT_LEVEL=1`, `CARGO_PROFILE_TEST_OPT_LEVEL=1`, `CARGO_PROFILE_DEV_STRIP=symbols` und `CARGO_PROFILE_TEST_STRIP=symbols`. Das verändert weder Quellcode noch Features; Debug-Assertions bleiben aktiv. Keine fremden Buildordner wurden gelöscht.

Im anschließenden parallelen Workspace-Lauf schlug der bestehende Engine-Test `core_robots::blocked_urls_get_status_zero_are_never_requested_and_do_not_use_up_the_limit` fehl (`crates/os-engine/tests/core_robots.rs:100`: drei statt vier Requests). Derselbe unveränderte Workspace-Test war isoliert grün. Der vollständige Wiederholungslauf verwendet deshalb zusätzlich `RUST_TEST_THREADS=1` und `--no-fail-fast`, ohne einen Test zu überspringen. Die Ursache des sporadischen Engine-Fehlers ist außerhalb von U25 weiterzuverfolgen; weder Engine-Verhalten noch Test-Erwartung wurden angepasst.

Endergebnis des vollständigen Workspace-Laufs einschließlich Doc-Tests: **3.028 bestanden, 0 fehlgeschlagen, 89 bestehende Ignores, 411 abgeschlossene Testsuites, Exit-Code 0**. Der serielle Wiederholungslauf dauerte etwa 30 min 20 s bei vorhandenem Build-Cache; der kompakte Erstaufbau zuvor 22 min 33 s. Alle drei `core_robots`-Tests sind im vollständigen Wiederholungslauf grün. U25 fügt keine Ignores hinzu. Keine CI-Lauf-ID: gemäß Auftrag kein Push, Merge oder PR.

## Abweichungen und offene Integration

Der vorhandene `crates/os-app/src/commands/mcp.rs` ist ausdrücklich `STUB(M2-K17)`. Das Paket ersetzt ihn nicht: Server, Schlüsselbund, wirksame Token-Widerrufung, Dateischreiber/Sicherungen, Werkzeugfreigaben, Markdown-Erzeugung und Erfassung der Aufrufe sind Zuständigkeit K17. Die Oberfläche behandelt `Unsupported` sichtbar und wiederholbar. Die Browser- und Komponententests prüfen deshalb die U25-Verträge mit lokalen Fixtures, keinen echten HTTP-Server und keinen echten MCP-Crawl.

Nach Integration von K17 bleiben die echten HTTP-Prüfungen (401/403, zwei Sitzungen, zehnminütiger Ablauf, Auto-Start ≤ 2 s, Portkonflikt) und die echte Datei-/Markdown-Integration auszuführen. K17 muss die ergänzten optionalen Felder füllen und Startfehler über den MCP-Notice-Namensraum melden. Das erneute Lesen der Client-Konfiguration verkleinert das Zeitfenster zwischen Vorschau und Schreiben; eine atomare Prüfung samt Sicherung muss der Dateischreiber selbst garantieren.

Keine fachliche Spec-Frage offen. Kein Referenzprodukt eingesehen. Kein Push, Merge oder PR. CI-Matrix wird nicht lokal ersetzt und wurde hier nicht ausgelöst.
