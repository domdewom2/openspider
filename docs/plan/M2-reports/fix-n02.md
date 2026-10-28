# fix-n02 – Nacharbeit zu M2-N02 / Fable approved 91

Basis: `origin/m2/n02` (`91d64ddc93d6c9d3f095c640f11bdec7f59d7bcb`), Branch `m2/fix-n02`, Worktree `/Users/dominikwojcik/Projects/wt-fix-n02`. Befundvorlage: `/Users/dominikwojcik/Projects/logs/fable-n02.md`, vollständig gelesen. Stand: 28.10.2026. Ursachen-Zeilennummern beziehen sich auf den Basisstand.

**Paket offen:** MCP-1 bis MCP-4 und Befund 6 sind behoben; A1 ist teilweise umgesetzt. Die verbleibende `ApiCtx`-Migration benötigt zusätzliche Pfadfreigaben. Es gab keinen Push, Merge oder PR.

**Erneute Prüfung des vorgefundenen Stands am 28.10.2026:** Der Arbeitsbaum war bei Beginn sauber, HEAD war bereits `00ab1bbe`; die unten beschriebenen Produktänderungen und Prüfergebnisse waren damit schon committed. Die erneute Lektüre bestätigt die offene A1-Voraussetzung: Die ausdrücklich erlaubte Dateiliste enthält nicht alle Konstruktorstellen. „Alle Freigaben sind erteilt“ erweitert diese konkrete Dateiliste nicht um weitere Pfade. Es wurden deshalb keine Strukturdefinition mit anschließend fehlenden Pflichtfeldern und keine Änderungen außerhalb der Freigabe eingebracht. `cargo fmt --all --check` und der Desktop-Typecheck wurden erneut erfolgreich ausgeführt; die übrigen nachstehenden Ergebnisse stammen aus dem bereits dokumentierten Prüflauf und wurden in dieser Nachprüfung nicht erneut ausgeführt. Der Diff gegen `91d64ddc` enthält keine Änderungen an `apps/desktop/e2e/mcp/**`. Tokio steht unverändert auf 1.53.3. Sowohl das lokale `target` als auch das unten genannte Paket-Target sind bereits entfernt. Nächster Schritt bleibt die ausdrückliche Erweiterung der Dateifreigabe auf die unten aufgelisteten mechanischen Konstruktoranpassungen; anschließend A1 vollständig implementieren und sämtliche Prüfungen erneut durchführen.

## A1 – Quota-Ledger / INTEG-006: teilweise umgesetzt, offen

**Ursache:** `crates/os-integrations/src/crawl_glue/mod.rs:36–40` verwendet `app_paths()`; `crates/os-integrations/src/crawl_glue/runtime.rs:84–105` biegt Crawl-Sidecars anhand des Dateinamens auf dessen Settings-Verzeichnis um. `AppConfig::new(home)` verwendet dagegen einen ausdrücklich übergebenen App-Pfad. Ohne Übergabe können App und Crawl verschiedene Ledger verwenden. Die beiden APIs-mode-Lifecycle-Tests konnten bisher das echte Benutzerverzeichnis beschreiben.

**Änderung:** Die betroffenen Lifecycle-Tests laufen in getrennten Unterprozessen mit je einem von `tempfile` verwalteten `OPENSPIDER_HOME`. Der Elternprozess verändert keine globalen Umgebungsvariablen; der temporäre Ordner bleibt bis zum Ende des Kindes bestehen. Die echten Mock-Abrufe und bisherigen Assertions laufen im Kind. Der INTEG-006-Test prüft zusätzlich nach `start_apis_mode_with` über `ApiRun::new`, dass `api-quotas.json` tatsächlich unter dem temporären Home entstanden ist.

**Test/Ergebnis:** `env -u OPENSPIDER_HOME cargo test -p os-integrations --test crawl_glue_lifecycle`: **8 grün**. Die vorhandene API-Listen-Fixture und GSC-Mock-Inspektion werden weiterhin verwendet. Die Testisolation und die echte Fallback-Auflösung sind damit geprüft.

**Offen:** `ApiCtx.quota_ledger: Option<PathBuf>`, dessen Bevorzugung im Laufzeitpfad sowie die Übergabe des App-Settings-Pfads sind nicht umgesetzt. Auch ein `Option`-Feld muss in bestehenden Rust-Strukturliteralen angegeben werden. Die notwendigen mechanischen Anpassungen liegen teilweise außerhalb der ausdrücklich erlaubten Dateien:

- `crates/os-engine/src/crawl.rs`
- `crates/os-app/src/commands/api.rs`, `crates/os-app/src/commands/export_summary.rs`
- `crates/os-cli/src/exec.rs`, `crates/os-cli/src/google.rs`, `crates/os-cli/tests/m2_l01.rs`
- `crates/os-mcp/src/tools/link_metrics.rs`
- `crates/os-integrations/src/psi/mobile.rs`
- `crates/os-integrations/tests/{api_framework,drive_support,ga4_api,gsc_search_analytics,linkmetrics_providers,psi_local,psi_remote}.rs`

Die Freigabe dieser Konstruktoranpassungen ist der nächste notwendige Schritt. Es wurde weder eine nicht kompilierende Feldmigration committed noch die Pfadgrenze überschritten. Die Isolation ist eine Teilkorrektur und behebt noch nicht die unterschiedliche Ledger-Auflösung bei expliziter App-Konfiguration.

## MCP-1 – Audit-Log / AGENT-043: behoben

**Ursache:** `crates/os-mcp/src/audit.rs:39–76`: Alle String-Parameter wurden redigiert, einschließlich der zur Zuordnung nötigen Crawl-ID.

**Änderung:** `crawl_id`, `id`, `tab`, `filter`, `limit`, `offset`, `format` und `page` werden als Skalare sichtbar. Verschachtelte Werte werden nicht freigegeben. URL-, SQL-, Pfad-, Override-, Header-, Cookie- und übrige String-Werte bleiben redigiert oder werden bei unbekannten Schlüsseln vollständig ausgelassen. Array-Inhalte erscheinen weiterhin nur als Anzahl. Das vorhandene boolesche `confirm` bleibt sichtbar.

**Test/Ergebnis:** `audit_parameters.rs` prüft alle erlaubten Skalare, verschachtelte Geheimnisse, sensible Parameter, unbekannte Schlüssel und die vorhandenen HTTP-Header-Proben. Zusätzlich prüfen die unveränderten E2E-Fälle drei echte HTTP-Aufrufe gegen `out-basic-site`, sichtbare Crawl-ID und fehlenden Token im Log. Grün in beiden Browsern.

## MCP-2 – Verbindungsabbruch / AGENT-002, OUT-285: behoben

**Ursache:** `crates/os-mcp/src/http.rs:211–218`: rmcp behält die Sitzung nach dem Abbruch des alleinstehenden GET/SSE-Kanals; das Zählen der Manager-Einträge zeigte deshalb weiter einen Client.

**Änderung:** Der HTTP-Listener verknüpft erfolgreiche, authentifizierte GET/SSE-Anfragen mit ihrer HTTP/1-Verbindung. Beim Freigeben der Verbindung werden deren Sitzungen über `SessionManager::close_session` entfernt. Endliche POST-Antworten beenden keine Sitzung; fehlgeschlagene oder nicht autorisierte GETs werden nicht registriert. Es werden keine neuen Abhängigkeiten benötigt.

**Test/Ergebnis:** `m2_http.rs` öffnet zwei Sitzungen, bricht eine echte TCP-GET/SSE-Verbindung ab und prüft innerhalb einer Sekunde den Zähler 1, die Nichtverwendbarkeit der entfernten ID (404) und die weiter nutzbare zweite Sitzung. Alle fünf HTTP-Tests grün. Die unveränderten Browsertests prüfen zusätzlich Statuswechsel, Abbruch und echten Zehn-Minuten-Ablauf: beide grün.

## MCP-3 – Notice-Zustellung / OUT-285: behoben

**Ursache:** `crates/os-app/src/mcp/mod.rs:128–139`, `crates/os-app/src/runtime/mod.rs:580–591`: Ohne Fenster bleibt die Startfehler-Notice gespeichert. Beim Abonnieren gab es keinen Flush; der MCP-Loop sendete bei unverändertem Status `Off` ebenfalls nichts. Ein bereits abonniertes Fenster konnte eine Notice zudem wegen einer noch nicht fälligen Coalescer-Frist dauerhaft verpassen.

**Änderung:** `App::subscribe` liefert ausstehende Ticks unmittelbar über den bestehenden Zusammenbau- und Pufferpfad nach. Der MCP-Loop versucht anschließend alle 100 ms den normalen, weiterhin vom Coalescer begrenzten Flush, auch bei unverändertem Status.

**Test/Ergebnis:** Zwei Runtime-Regressionstests prüfen den echten Startfehler bei besetztem Port vor dem Abonnement, einmalige Zustellung sowie eine erst nach dem Abonnement eingereihte Notice trotz anfänglich blockierender Coalescer-Frist. Beide grün; auch `cmd_mcp_service` (6 Tests) und die unveränderten Autostart-E2Es sind grün.

## MCP-4 – Token-Dialog / AGENT-002, UI-113: behoben

**Ursache:** `apps/desktop/src/features/mcp/AccessPanel.tsx:44–49`: Native Deaktivierung der fokussierten Token-Schaltfläche entzog ihr in Chromium den Fokus. Escape erreichte den Dialog danach nicht mehr zuverlässig.

**Änderung:** Der Bereich zeigt den laufenden Vorgang mit `aria-busy`, die Schaltflächen mit `aria-disabled`. Sie behalten ihren Fokus. Die bestehende Sperre verhindert parallele Aktionen; auch Anzeigen/Verbergen beachtet sie.

**Test/Ergebnis:** Die unveränderten Access-Dialog-E2Es prüfen Kopieren ohne Token im DOM, Anzeigen/Verbergen/Regenerieren, Escape und Fokus-Rückkehr. Grün in Chromium und WebKit.

## Befund 6 – Fixture-README: bereinigt

**Ursache:** `tests/fixtures/crawl-m2-list/README.md:12`: Die Fixture-Dokumentation nannte die entfernte Datei `export_tests.rs`.

**Änderung:** Der Hinweis nennt jetzt `crates/os-export/tests/list_m2_e06.rs`, wo die produktiven Export- und SERP-Rundreisetests tatsächlich liegen.

**Test/Ergebnis:** Zielpfad vorhanden; veralteter Eintrag entfernt. Die MCP-Prozessprüfung der Exporte gegen Kern/CLI besteht ebenfalls.

## Prüfungen

Alle langen Befehle liefen im Vordergrund über einen externen Prozess-Wrapper mit 3600 Sekunden Timeout je Befehl. Für die Rust-Pakettests wurde zusätzlich ein temporäres `OPENSPIDER_HOME` verwendet; die gezielte Lifecycle-Gegenprobe lief ausdrücklich ohne einen solchen Wert im Elternprozess.

| Prüfung | Ergebnis |
| --- | --- |
| `npm ci` bei fehlendem `node_modules` | erfolgreich |
| `cargo fmt --all --check` | grün, Exit 0 |
| `cargo clippy -p os-integrations -p os-mcp -p os-app --all-targets -- -D warnings` | grün, Exit 0; nach der letzten Teständerung erneut geprüft |
| `cargo test -p os-integrations -p os-mcp` | 318 bestanden, 0 fehlgeschlagen, 1 vorhandener Lighthouse-Test ignoriert |
| `env -u OPENSPIDER_HOME cargo test -p os-integrations --test crawl_glue_lifecycle` | 8 bestanden; Gegenprobe nach Ergänzung der dauerhaften Isolation |
| `cargo test -p os-app --lib subscription_notice_tests` | 2 bestanden |
| `cargo test -p os-app --test cmd_mcp_service` | 6 bestanden |
| `cargo build -p os-cli --bin openspider` | erfolgreich; Voraussetzung des vorhandenen Export-Prozesstests |
| `cargo build -p os-app --features e2e-bridge --bin os-app-bridge -p os-testsite --bin os-testsite` | erfolgreich |
| `npm run -w apps/desktop e2e -- e2e/mcp --project=chromium --project=webkit --workers=1` | **24/24 grün**, 21,0 Minuten, einschließlich zweimal echtem Zehn-Minuten-Ablauf |
| `npm run -w apps/desktop typecheck` | grün, Exit 0 |
| `npm run -w apps/desktop test` | **771/771 grün**, 76 Testdateien |

Der erste Rust-Paketlauf scheiterte nur an der fehlenden CLI-Binärdatei, die der vorhandene MCP-Export-Prozesstest voraussetzt. Nach `cargo build -p os-cli --bin openspider` wurde der vollständige Paketlauf erfolgreich wiederholt. Ein vorhandener Test mit echter Lighthouse-Installation bleibt regulär ignoriert. Die MCP-E2E-Dateien und Assertions wurden nicht verändert oder abgeschwächt.

## Fremdänderungen / K2-72

Die folgenden freigegebenen Paketänderungen werden gemäß Arbeitsauftrag als **Fremdänderungen im Sinne von K2-72** deklariert. Grundlage ist die explizite Orchestrator-Freigabe der Dateiliste:

| Freigegebener Fremdpfad | Umfang |
| --- | --- |
| `crates/os-integrations/tests/crawl_glue_lifecycle.rs` | A1-Testisolation und echte Fallback-Auflösung |
| `crates/os-mcp/src/audit.rs`, `crates/os-mcp/tests/audit_parameters.rs` | Audit-Allowlist und Regressionstest (K04-Linie) |
| `crates/os-mcp/src/http.rs`, `crates/os-mcp/tests/m2_http.rs` | GET/SSE-Verbindungslebensdauer und HTTP-Test (K04/K17) |
| `crates/os-app/src/runtime/mod.rs` | Nachlieferung beim Abonnieren und zwei Notice-Tests (Runtime-Besitzer) |
| `crates/os-app/src/mcp/mod.rs` | Wiederholter Flush vorgemerkter Meldungen (K17) |
| `apps/desktop/src/features/mcp/AccessPanel.tsx` | Fokus bei laufenden Token-Aktionen (U25) |
| `tests/fixtures/crawl-m2-list/README.md` | Veralteter Dateihinweis (E06/Integrator) |

`crates/os-integrations/tests/crawl_glue_lifecycle.rs` enthält die Teilkorrektur der A1-Testisolation. Keine anderen Produktpfade, Manifeste oder Lockfiles wurden geändert. Tokio bleibt **1.53.3**; kein `cargo update`. Alle Commits verwenden explizite Dateipfade und den verlangten Co-Author-Trailer. Der Arbeitsbaum wurde abschließend bereinigt; das paketbezogene `CARGO_TARGET_DIR=/Users/dominikwojcik/Projects/build.noindex/target-fix-n02` wurde nach den Prüfungen gelöscht.

## Erneuter Paketlauf ab `4b741111` (28.10.2026)

Schritt 1 abgeschlossen: Arbeitsbaum anfangs sauber; `CLAUDE.md`, die vollständige Fable-Vorlage, Ursachen und vorhandene Korrekturen gelesen. `node_modules` vorhanden, daher kein erneutes `npm ci`. Vorgefundener Branch `m2/fix-n02` enthält bereits die MCP-Korrekturen und A1-Testisolation; diese werden erhalten.
