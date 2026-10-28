# fix-n02 – Nacharbeit zu M2-N02 / Fable approved 91

Basis: `origin/m2/n02` (`91d64ddc93d6c9d3f095c640f11bdec7f59d7bcb`), Branch `m2/fix-n02`, Worktree `/Users/dominikwojcik/Projects/wt-fix-n02`. Befundvorlage: `/Users/dominikwojcik/Projects/logs/fable-n02.md`, vollständig gelesen. Stand: 28.10.2026. Ursachen-Zeilennummern beziehen sich auf den Basisstand.

**Paket offen:** MCP-1 bis MCP-4 und Befund 6 sind behoben; A1 ist teilweise umgesetzt. Die verbleibende `ApiCtx`-Migration benötigt zusätzliche Pfadfreigaben. Es gab keinen Push, Merge oder PR.

**Erneuter vollständiger Prüflauf am 28.10.2026:** Anfangs sauberer Arbeitsbaum auf `4b741111`. Die MCP-Korrekturen, README-Bereinigung und A1-Testisolation waren bereits committed vorhanden. Schritte 1–5 wurden in der vorgegebenen Reihenfolge am Code nachvollzogen und jeweils separat im Bericht committed; zusätzliche Produktänderungen waren für MCP-1 bis MCP-4 nicht erforderlich. Sämtliche unten aufgeführten Prüfungen wurden in diesem Lauf erneut ausgeführt und sind grün. A1 bleibt wegen der ausdrücklich begrenzten Dateiliste offen; die notwendigen Konstruktorstellen sind unten vollständig aufgelistet. Kein Push, Merge oder PR.

## A1 – Quota-Ledger / INTEG-006: teilweise umgesetzt, offen

**Ursache:** `crates/os-integrations/src/crawl_glue/mod.rs:36–40` verwendet `app_paths()`; `crates/os-integrations/src/crawl_glue/runtime.rs:84–105` biegt Crawl-Sidecars anhand des Dateinamens auf dessen Settings-Verzeichnis um. `AppConfig::new(home)` verwendet dagegen einen ausdrücklich übergebenen App-Pfad. Ohne Übergabe können App und Crawl verschiedene Ledger verwenden. Die beiden APIs-mode-Lifecycle-Tests konnten bisher das echte Benutzerverzeichnis beschreiben.

**Änderung:** Die betroffenen Lifecycle-Tests laufen in getrennten Unterprozessen mit je einem von `tempfile` verwalteten `OPENSPIDER_HOME`. Der Elternprozess verändert keine globalen Umgebungsvariablen; der temporäre Ordner bleibt bis zum Ende des Kindes bestehen. Die echten Mock-Abrufe und bisherigen Assertions laufen im Kind. Der INTEG-006-Test prüft zusätzlich nach `start_apis_mode_with` über `ApiRun::new`, dass `api-quotas.json` tatsächlich unter dem temporären Home entstanden ist.

**Test/Ergebnis:** `cargo test -p os-integrations --test crawl_glue_lifecycle`: **8 grün**. Die vorhandene API-Listen-Fixture und GSC-Mock-Inspektion werden weiterhin verwendet. Die Testisolation und die echte Fallback-Auflösung sind damit geprüft.

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

Alle langen Befehle liefen im Vordergrund über einen synchronen Python-Wrapper mit `subprocess.run(timeout=3600)` (3600000 ms) je Befehl. Der Lauf verwendete ein von `tempfile` verwaltetes `OPENSPIDER_HOME`; die beiden betroffenen Lifecycle-Tests erzeugen zusätzlich jeweils ihr eigenes temporäres Home im Unterprozess. Alle 13 Prüf-/Buildbefehle endeten mit Exit 0. Die vollständigen Rohprotokolle dieses Laufs liegen unter `/var/folders/fz/9flwb7b55t788nq02b9q761h0000gp/T/fix-n02-verify-0c4dygse/` (`results.json`, `0.log` bis `12.log`), außerhalb des Repositorys.

| Prüfung | Ergebnis |
| --- | --- |
| `npm ci` bei fehlendem `node_modules` | nicht erforderlich; Verzeichnis anfangs vorhanden |
| `cargo fmt --all --check` | grün, Exit 0 |
| `cargo clippy -p os-integrations -p os-mcp -p os-app --all-targets -- -D warnings` | grün, Exit 0; 56 Sekunden |
| `cargo test -p os-integrations -p os-mcp` | 318 bestanden, 0 fehlgeschlagen, 1 vorhandener Lighthouse-Test ignoriert |
| `cargo test -p os-integrations --test crawl_glue_lifecycle` | 8 bestanden; echte Fallback-Auflösung mit dauerhafter Prozessisolation |
| `cargo test -p os-app --lib subscription_notice_tests` | 2 bestanden |
| `cargo test -p os-app --test cmd_mcp_service` | 6 bestanden |
| `cargo build -p os-cli --bin openspider` | erfolgreich; Voraussetzung des vorhandenen Export-Prozesstests |
| `cargo build -p os-app --features e2e-bridge --bin os-app-bridge -p os-testsite --bin os-testsite` | erfolgreich |
| `npm run -w apps/desktop e2e -- e2e/mcp --project=chromium --project=webkit --workers=1` | **24/24 grün**, 21,0 Minuten, einschließlich zweimal echtem Zehn-Minuten-Ablauf |
| `npm run -w apps/desktop typecheck` | grün, Exit 0 |
| `npm run -w apps/desktop test` | **771/771 grün**, 76 Testdateien |

Die CLI wurde vor dem vollständigen Rust-Paketlauf gebaut, da der vorhandene MCP-Export-Prozesstest sie benötigt. Der Paketlauf bestand ohne Wiederholung in 189,5 Sekunden. Ein vorhandener Test mit echter Lighthouse-Installation bleibt regulär ignoriert. Beim erfolgreichen Bridge-Link meldet der macOS-Linker eine Warnung zur Größe von `__eh_frame`; die Clippy-Prüfung mit `-D warnings` ist grün. Die MCP-E2E-Dateien und Assertions wurden nicht verändert oder abgeschwächt.

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

## Abschluss von Schritt 7

Bericht mit Ursachen, Änderungen, Tests, Ergebnissen und Fremdänderungen aktualisiert. In diesem erneuten Lauf wurde ausschließlich dieser freigegebene Bericht geändert; sämtliche vorhandenen Produktkorrekturen wurden erhalten und erneut geprüft. A1 ist ausdrücklich weiterhin unvollständig: Die App übergibt noch keinen konfigurierten Ledger-Pfad. Grüne Tests ersetzen diese ausstehende Implementierung nicht.

**Nächster Schritt:** Die oben genannten mechanischen `ApiCtx`-Konstruktoranpassungen zusätzlich zur bisherigen Dateiliste ausdrücklich freigeben; dann `quota_ledger: Option<PathBuf>` additiv einführen, in `ApiRun::new`/`start_apis_mode_with` bevorzugen, in `session/control.rs` den Ledger-Pfad aus `app.config().paths.settings` setzen und die reale Auflösung bei abweichender App-Konfiguration testen. Anschließend vollständige Prüfungen erneut ausführen. Keine Rückfrage oder zusätzliche Genehmigungsaktion ausgelöst.

## Schritt 6 – Abschlussprüfung nach A1-Commit 22a7bedc

Die zusätzlich freigegebene A1-Migration ist umgesetzt; der obige offene Status stammt aus dem vorherigen Lauf und wird in Schritt 7 konsolidiert. Alle neun Befehle des abschließenden Prüflaufs sind mit Exit 0 beendet: Format, Clippy (`-D warnings`), CLI-Build, Rust-Pakettests (**319 bestanden**, ein vorhandener Test ignoriert), `cmd_mcp_service` (**6 bestanden**), Bridge/Testsite-Build, unveränderte MCP-E2Es (**24/24**, Chromium und WebKit, ein Worker, 21,0 Minuten), Typecheck und Frontend-Tests (**771/771**). Der zusätzliche fokussierte Lifecycle-Lauf vor dem A1-Commit bestand mit **9/9** Tests.

Alle langen Befehle liefen im Vordergrund mit `subprocess.run(timeout=3600)` (3600000 ms), die Abschlussprüfungen mit temporärem `OPENSPIDER_HOME`. Rohprotokolle: `/var/folders/fz/9flwb7b55t788nq02b9q761h0000gp/T/fix-n02-final-whbgohrx/` (`results.json`, `0.log` bis `8.log`). Die bekannte macOS-Linkerwarnung zu `__eh_frame` trat beim erfolgreichen Bridge-Build auf; Clippy ist grün. Kein Push, Merge oder PR.
