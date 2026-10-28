# fix-n02 – Nacharbeit zu M2-N02 / Fable approved 91

Basis: `origin/m2/n02` (`91d64ddc93d6c9d3f095c640f11bdec7f59d7bcb`), Branch `m2/fix-n02`, Worktree `/Users/dominikwojcik/Projects/wt-fix-n02`. Befundvorlage: `/Users/dominikwojcik/Projects/logs/fable-n02.md`, vollständig gelesen. Stand: 28.10.2026. Ursachen-Zeilennummern beziehen sich auf den Basisstand.

**Umsetzungsstand vor Fixrunde 2:** A1, MCP-1 bis MCP-4 und Befund 6 sind behoben. Die Orchestrator-Freigabe vom 28.10., 16:20 umfasst jetzt auch sämtliche mechanischen `ApiCtx`-Konstruktoranpassungen. A1 wurde in diesem Lauf vollständig umgesetzt; die bereits erledigten MCP- und README-Korrekturen wurden gemäß Auftrag erhalten. Es gab keinen Push, Merge oder PR.

Ausgangsstand dieses Fortsetzungslaufs: sauberer Arbeitsbaum auf `a45db86b`. `node_modules` war vorhanden; `npm ci` war deshalb nicht erforderlich. Nach A1 wurden die verlangten Prüfungen vollständig erneut ausgeführt. Die nachfolgenden MCP-Ursachen und Änderungen dokumentieren den bereits vorhandenen Paketstand; ihre E2E-Abdeckung ist Bestandteil von Schritt 6.

## A1 – Quota-Ledger / INTEG-006: behoben

**Ursache:** `crates/os-integrations/src/crawl_glue/mod.rs:36–40` verwendet `app_paths()`; `crates/os-integrations/src/crawl_glue/runtime.rs:84–105` biegt Crawl-Sidecars anhand des Dateinamens auf dessen Settings-Verzeichnis um. `AppConfig::new(home)` verwendet dagegen einen ausdrücklich übergebenen App-Pfad. Ohne Übergabe können App und Crawl verschiedene Ledger verwenden. Die beiden APIs-mode-Lifecycle-Tests konnten bisher das echte Benutzerverzeichnis beschreiben.

**Änderung:** `ApiCtx` hat additiv `quota_ledger: Option<PathBuf>`. `start_apis_mode_with` und `ApiRun::new` bevorzugen diesen vollständigen Ledger-Dateipfad unverändert, auch bei einem Dateinamen mit `.api-quotas.json`-Suffix. `session/control.rs` übergibt `app.config().paths.settings.join("api-quotas.json")`; die App-Konstruktoren in `commands/api.rs` und `commands/export_summary.rs` verwenden denselben Pfad. Alle übrigen Konstruktorstellen ergänzen ausschließlich `quota_ledger: None`. Ohne Kontextpfad bleibt die Auflösung über `app_paths()` erhalten; die bestehende explizite Ledger-Argumentübergabe für Einbetter bleibt kompatibel.

Die bereits vorhandene Testisolation bleibt erhalten: Die betroffenen Lifecycle-Tests laufen in getrennten Unterprozessen mit je einem von `tempfile` verwalteten `OPENSPIDER_HOME`. Der Elternprozess verändert keine globalen Umgebungsvariablen. Auch der neue Regressionstest verwendet diese Isolation und ein zweites temporäres Verzeichnis für den konfigurierten Ledger.

**Test/Ergebnis:** `cargo test -p os-integrations --test crawl_glue_lifecycle`: **9 grün**. `apis_mode_assigns_bulk_and_per_url_results_without_crawling` prüft mit Listen-Fixture und echten Mock-Inspektionen den Fallback über `start_apis_mode_with` → `ApiRun::new` → `app_paths()`. Der neue Test `configured_ledger_overrides_home_and_crawl_sidecars` (`// spec: INTEG-006`) prüft über dieselbe produktive Aufrufkette den abweichenden konfigurierten Pfad: vier echte Inspektionen, Erschöpfung des gemeinsamen Property-Kontingents, keine weiteren Inspektionen nach Neustart eines `ApiRun`, anschließend vier neue Inspektionen nach Mitternacht US-Pazifikzeit. Weder der temporäre Home-Fallback noch der übergebene Crawl-Sidecar werden dabei beschrieben. Der vollständige Paketlauf enthält diese neun Tests ebenfalls und ist grün.

**Freigabe:** Die Orchestrator-Entscheidung vom 28.10., 16:20 erlaubt sämtliche mechanischen `ApiCtx`-Konstruktoranpassungen in `crates/**/src` und `crates/**/tests`. Damit ist die zuvor dokumentierte Pfadgrenze aufgelöst. Umsetzung: Commit `22a7bedc`.

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

**Test/Ergebnis:** Zwei Runtime-Regressionstests prüfen den echten Startfehler bei besetztem Port vor dem Abonnement sowie eine erst nach dem Abonnement eingereihte Notice trotz anfänglich blockierender Coalescer-Frist. Fixrunde 2 injiziert die vorhandene `ManualClock` über `AppConfig::with_clock` und verwendet `#[tokio::test(start_paused = true)]`. App- und Tokio-Zeit werden gemeinsam kontrolliert vorgerückt. Assertions prüfen ausbleibende Zustellung bis 499 ms, die Hintergrundzustellung bei 500 ms ohne manuellen Flush sowie Einmaligkeit über spätere Ticks und erneutes Abonnieren. Der echte Startfehler wird beim Abonnieren sofort zugestellt und auch nach der Frist nicht wiederholt. Reale Sekunden-Timeouts und Polling-Schleifen sind entfernt. Die aktuellen Laufzahlen stehen im Abschnitt „Fixrunde 2“. `cmd_mcp_service` (6 Tests) und Autostart-E2Es gehören zu den nachfolgend dokumentierten früheren Prüfungen.

## MCP-4 – Token-Dialog / AGENT-002, UI-113: behoben

**Ursache:** `apps/desktop/src/features/mcp/AccessPanel.tsx:44–49`: Native Deaktivierung der fokussierten Token-Schaltfläche entzog ihr in Chromium den Fokus. Escape erreichte den Dialog danach nicht mehr zuverlässig.

**Änderung:** Der Bereich zeigt den laufenden Vorgang mit `aria-busy`, die Schaltflächen mit `aria-disabled`. Sie behalten ihren Fokus. Die bestehende Sperre verhindert parallele Aktionen; auch Anzeigen/Verbergen beachtet sie.

**Test/Ergebnis:** Die unveränderten Access-Dialog-E2Es prüfen Kopieren ohne Token im DOM, Anzeigen/Verbergen/Regenerieren, Escape und Fokus-Rückkehr. Grün in Chromium und WebKit.

## Befund 6 – Fixture-README: bereinigt

**Ursache:** `tests/fixtures/crawl-m2-list/README.md:12`: Die Fixture-Dokumentation nannte die entfernte Datei `export_tests.rs`.

**Änderung:** Der Hinweis nennt jetzt `crates/os-export/tests/list_m2_e06.rs`, wo die produktiven Export- und SERP-Rundreisetests tatsächlich liegen.

**Test/Ergebnis:** Zielpfad vorhanden; veralteter Eintrag entfernt. Die MCP-Prozessprüfung der Exporte gegen Kern/CLI besteht ebenfalls.

## Prüfungen

Die im ursprünglichen Paketlauf dokumentierten langen Befehle liefen im Vordergrund über einen synchronen Python-Wrapper mit `subprocess.run(timeout=3600)` (3600000 ms) je Befehl. Die Abschlussprüfungen verwendeten ein von `tempfile` verwaltetes `OPENSPIDER_HOME`; die betroffenen Lifecycle-Tests erzeugen zusätzlich jeweils ihr eigenes temporäres Home im Unterprozess. Alle neun Prüf-/Buildbefehle endeten mit Exit 0. Die damaligen Rohprotokolle sind nicht mehr verfügbar; ein Verweis darauf entfällt. Die folgende Tabelle hält die damaligen Ergebnisse fest und ist kein neuer Lauf der Fixrunde 2. Der Prüfabschluss wurde separat in `e18b49f7` committed.

| Prüfung | Ergebnis |
| --- | --- |
| `npm ci` bei fehlendem `node_modules` | nicht erforderlich; Verzeichnis anfangs vorhanden |
| `cargo test -p os-integrations --test crawl_glue_lifecycle` vor dem A1-Commit | **9/9 grün**, einschließlich explizitem Ledger-Pfad und echter Fallback-Auflösung |
| `cargo fmt --all --check` | grün, Exit 0 |
| `cargo clippy -p os-integrations -p os-mcp -p os-app --all-targets -- -D warnings` | grün, Exit 0; 70,6 Sekunden |
| `cargo build -p os-cli --bin openspider` | erfolgreich; Voraussetzung des vorhandenen MCP-Export-Prozesstests |
| `cargo test -p os-integrations -p os-mcp` | **319 bestanden**, 0 fehlgeschlagen, 1 vorhandener Lighthouse-Test ignoriert; 200,0 Sekunden |
| `cargo test -p os-app --test cmd_mcp_service` | **6/6 grün** |
| `cargo build -p os-app --features e2e-bridge --bin os-app-bridge -p os-testsite --bin os-testsite` | erfolgreich |
| `npm run -w apps/desktop e2e -- e2e/mcp --project=chromium --project=webkit --workers=1` | **24/24 grün**, 21,0 Minuten, einschließlich zweimal echtem Zehn-Minuten-Ablauf |
| `npm run -w apps/desktop typecheck` | grün, Exit 0 |
| `npm run -w apps/desktop test` | **771/771 grün**, 76 Testdateien |

Die CLI wurde vor dem Rust-Paketlauf gebaut, da der vorhandene MCP-Export-Prozesstest sie benötigt. Alle Abschlussprüfungen bestanden im ersten Lauf. Der vorhandene Test mit echter Lighthouse-Installation bleibt regulär ignoriert. Beim erfolgreichen Bridge-Link meldet der macOS-Linker die bekannte Größenwarnung für `__eh_frame`; Clippy mit `-D warnings` ist grün. Die MCP-E2E-Dateien und Assertions wurden nicht verändert oder abgeschwächt. Die bereits erledigten MCP-1–4- und README-Korrekturen wurden nicht erneut bearbeitet; die unveränderten E2Es bestätigen ihren Produktstand.

## Fremdänderungen / K2-72

Die folgenden Paketänderungen sind **Fremdänderungen im Sinne von K2-72**. Grundlage sind die ursprüngliche ausdrückliche Pfadfreigabe sowie die erweiterte Orchestrator-Entscheidung vom 28.10., 16:20. Die Erweiterung erlaubt an sämtlichen `ApiCtx`-Konstruktorstellen ausschließlich das neue Feld mit `None` beziehungsweise mit dem bekannten App-Settings-Pfad.

| Freigegebener Fremdpfad | Umfang |
| --- | --- |
| `crates/os-integrations/src/core.rs` | Additives Feld `ApiCtx.quota_ledger: Option<PathBuf>` |
| `crates/os-integrations/src/crawl_glue/mod.rs`, `crates/os-integrations/src/crawl_glue/runtime.rs` | Vorrang des konfigurierten Ledgers vor Fallback und Sidecar-Auflösung |
| `crates/os-integrations/tests/crawl_glue_lifecycle.rs` | Prozessisolation, echte Fallback-Auflösung und Regressionstest für explizites Ledger, Neustart und Tagesreset |
| `crates/os-app/src/session/control.rs` | Ausschließlich Übergabe des Ledger-Dateipfads aus den App-Settings |
| `crates/os-app/src/commands/api.rs`, `crates/os-app/src/commands/export_summary.rs` | An drei Konstruktorstellen ausschließlich neues Feld mit bekanntem App-Settings-Pfad |
| `crates/os-engine/src/crawl.rs` | Ausschließlich `quota_ledger: None` im `ApiCtx`-Literal |
| `crates/os-cli/src/exec.rs`, `crates/os-cli/src/google.rs`, `crates/os-cli/tests/m2_l01.rs` | Ausschließlich `quota_ledger: None` in den `ApiCtx`-Literalen |
| `crates/os-mcp/src/tools/link_metrics.rs` | Ausschließlich `quota_ledger: None` im `ApiCtx`-Literal |
| `crates/os-integrations/src/psi/mobile.rs` | Ausschließlich `quota_ledger: None` in zwei `ApiCtx`-Literalen |
| `crates/os-integrations/tests/api_framework.rs`, `crates/os-integrations/tests/drive_support.rs`, `crates/os-integrations/tests/ga4_api.rs`, `crates/os-integrations/tests/gsc_search_analytics.rs`, `crates/os-integrations/tests/linkmetrics_providers.rs`, `crates/os-integrations/tests/psi_local.rs`, `crates/os-integrations/tests/psi_remote.rs` | Ausschließlich `quota_ledger: None` in den vorhandenen `ApiCtx`-Literalen |
| `crates/os-mcp/src/audit.rs`, `crates/os-mcp/tests/audit_parameters.rs` | Bereits erledigt: Audit-Allowlist und Regressionstest (K04) |
| `crates/os-mcp/src/http.rs`, `crates/os-mcp/tests/m2_http.rs` | Bereits erledigt: GET/SSE-Verbindungslebensdauer und HTTP-Test (K04/K17) |
| `crates/os-app/src/runtime/mod.rs` | Bereits erledigt: Nachlieferung beim Abonnieren und Notice-Tests |
| `crates/os-app/src/mcp/mod.rs` | Bereits erledigt: wiederholter Flush vorgemerkter Meldungen (K17) |
| `apps/desktop/src/features/mcp/AccessPanel.tsx` | Bereits erledigt: Fokus bei laufenden Token-Aktionen (U25) |
| `tests/fixtures/crawl-m2-list/README.md` | Bereits erledigt: veralteter Dateihinweis (E06/Integrator) |

Der A1-Commit `22a7bedc` umfasst genau 20 freigegebene Dateien. Die übrigen Paketkorrekturen blieben unverändert. Manifeste und Lockfile sind unverändert, Tokio bleibt **1.53.3**; kein `cargo update`. Commits verwenden ausschließlich explizite Dateipfade und den verlangten Co-Author-Trailer. Die MCP-E2E-Dateien wurden nicht geändert. Generierte Playwright-Artefakte und das paketbezogene `CARGO_TARGET_DIR=/Users/dominikwojcik/Projects/build.noindex/target-fix-n02` wurden nach den Prüfungen entfernt.

## Abschluss des ursprünglichen Schritts 7

Bericht mit Ursache, Änderung, Test, Ergebnis und vollständiger Dateiliste der freigegebenen Fremdänderungen aktualisiert. A1 ist vollständig umgesetzt und geprüft. Alle verlangten Prüfungen sind grün; Arbeitsbaum sauber und paketbezogenes Target-Verzeichnis gelöscht. Kein Push, Merge oder PR.


## Fixrunde 2 – Astra-Befunde 1–3

**Befund 1 / Paketautorität:** Fixpakete haben keinen eigenen Eintrag in `docs/plan/M2-packages.json`. Maßgeblich ist der Auftrag `/Users/dominikwojcik/Projects/fix-n02.txt` einschließlich der Orchestrator-Entscheidungen und Konstruktorfreigabe vom 28.10., 16:20; die oben aufgeführten Fremdänderungen beziehen sich darauf. Das Nachführen einer Paketdefinition ist Sache des Orchestrators und kein offener Implementierungsschritt dieses Fixpakets.

**Befund 2 / deterministische Tests:** Commit `c6a4a76f` ändert ausschließlich `crates/os-app/src/runtime/mod.rs` (freigegebener Fremdpfad). Beide Notice-Tests verwenden die bestehende App-Uhr-Abstraktion und pausierte Tokio-Zeit; die Fristgrenzen und einmalige Zustellung werden ausdrücklich geprüft. Keine Produktionslogik wurde geändert.

**Befund 3 / Prüfnachweis:** Die Ergebnisse der folgenden lokalen Prüfungen werden dauerhaft hier im Repository dokumentiert. Verweise auf nicht vorhandene Rohprotokolle wurden entfernt. Alle vier Befehle liefen im Vordergrund mit `subprocess.run(timeout=3600)` (3600000 ms) je Befehl, ohne `cargo update`; Tokio bleibt 1.53.3, Manifeste und Lockfile unverändert.

| Lokale Prüfung der Fixrunde 2 (28.10.2026) | Ergebnis |
| --- | --- |
| `cargo test -p os-app --lib subscription_notice_tests` | **2 bestanden**, 0 fehlgeschlagen, 0 ignoriert, 25 herausgefiltert; Testzeit 0,03 s; Exit 0 |
| `cargo test -p os-app --lib` | **27 bestanden**, 0 fehlgeschlagen, 0 ignoriert, 0 herausgefiltert; Testzeit 4,07 s; Exit 0 |
| `cargo fmt --all --check` | grün, Exit 0 |
| `cargo clippy -p os-app --all-targets -- -D warnings` | grün, Exit 0; 54,10 s |

Beide Testläufe melden ausschließlich die bereits bekannte macOS-Linkerwarnung zur Größe von `__eh_frame`; die Tests bestehen.

**CI-/Plattform-/DoD-Nachweis:** folgt über den Integrationsbranch des Orchestrators. Es liegt hier keine CI-Lauf-ID vor. Die workspaceweiten Rust-Prüfungen, weitere Plattformläufe und erneute Desktop-Prüfungen wurden in dieser Fixrunde nicht ausgeführt; ihr aktueller Integrationsnachweis bleibt ausdrücklich offen. Die obige Tabelle dokumentiert den früheren Paketlauf.

**Abschluss Fixrunde 2:** Beide beauftragten Schritte sind umgesetzt; alle vier lokalen Prüfungen sind grün. Zwei getrennte Commits mit dem verlangten Co-Author-Trailer. Arbeitsbaum sauber, paketbezogenes `CARGO_TARGET_DIR=/Users/dominikwojcik/Projects/build.noindex/target-fix-n02` entfernt. Kein Push, Merge oder PR. Der Integrationsnachweis bleibt wie oben ausgewiesen beim Orchestrator.

## Integrationskorrektur int-15

`fix-int15` (28.10.2026), Commit `77636767`: `App::subscribe` ruft `flush_tick_to` nur bei wartenden Notices auf. Ohne Notices bleiben Beiträge und Wasserstände bis zum regulären Flush unberührt; die Nachlieferung aus MCP-3 bleibt erhalten. `ops_cancel.rs` bleibt unverändert. Der Notice-Test in `runtime_events.rs` prüft gemäß K2-63 jetzt die sofortige einmalige Zustellung beim Abonnieren und keine Wiederholung beim nächsten Flush; die Begründung steht im Testkommentar.

Lokale Prüfungen: `cargo test -p os-app` vollständig einschließlich aller Integrationstests und Doc-Tests grün (426 bestanden, 0 fehlgeschlagen, 0 ignoriert; 775,93 s). Beide `subscription_notice_tests` und beide ursprünglich fehlgeschlagenen Tests bestehen. `cargo fmt --all --check` (10,65 s) und `cargo clippy -p os-app --all-targets -- -D warnings` (111,81 s) jeweils Exit 0. Jeder Befehl lief mit `subprocess.run(timeout=3600)` (3600000 ms). Der erste Testversuch brach beim Linken wegen Platzmangels ab; nach Löschen des Paket-Targets liefen Tests und Clippy mit `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0` und `CARGO_INCREMENTAL=0` erfolgreich. Beim Test-Build erschien die bekannte macOS-Linkerwarnung zu `__eh_frame`. Dies ist ein lokaler macOS-Nachweis, kein neuer Linux-CI-Lauf.

Manifeste und Lockfile unverändert, Tokio weiterhin 1.53.3; kein `cargo update`. Zwei getrennte Commits mit Co-Author-Trailer, kein Push, Merge oder PR. Das Paket-Target `/Users/dominikwojcik/Projects/build.noindex/target-fix-int15` ist gelöscht; der Arbeitsbaum wird sauber übergeben.
