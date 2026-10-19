# M2-F05b – Startcheckliste der Fachpakete

Infrastruktur nach `docs/plan/M2.md` 3.14.9/3.14.12. Keine Fachimplementierung; Commands antworten bis zur Übernahme mit `Unsupported` und dem angegebenen Besitzer. Signaturen sind IPC-Verträge (Argument → Ergebnis); `long` bezeichnet den späteren Operationsvertrag.

| Besitzer | Datei | Command | Signatur | long |
|---|---|---|---|---|
| M2-C01 | `crates/os-app/src/commands/compare.rs` | `compare.close` | `void → void` | false |
| M2-C01 | `crates/os-app/src/commands/compare.rs` | `compare.open` | `CompareOpenArgs → CompareSession` | false |
| M2-C01 | `crates/os-app/src/commands/compare.rs` | `compare.swap` | `void → CompareSession` | false |
| M2-C01 | `crates/os-app/src/commands/compare.rs` | `compare.testMapping` | `TestMappingArgs → RuleTrace` | false |
| M2-D19 | `crates/os-app/src/commands/detail_spelling.rs` | `detail.spellingContext` | `SpellingContextArgs → SpellingContext` | false |
| M2-D21 | `crates/os-app/src/commands/detail_a11y.rs` | `detail.a11yNode` | `A11yNodeArgs → A11yNodeDetail` | false |
| M2-E02 | `crates/os-app/src/commands/config_robots.rs` | `config.loadRobots` | `RobotsLoadArgs → string` | false |
| M2-E02 | `crates/os-app/src/commands/config_robots.rs` | `config.testRobots` | `RobotsTestArgs → RobotsTestResult` | false |
| M2-E02 | `crates/os-app/src/commands/crawl_add.rs` | `crawl.addUrls` | `AddUrlsArgs → AddUrlsResult` | true |
| M2-E03 | `crates/os-app/src/commands/settings_network.rs` | `settings.probeCertificates` | `CertificateProbeArgs → CertificateChainDto` | false |
| M2-E03 | `crates/os-app/src/commands/settings_network.rs` | `settings.setProxyPassword` | `ProxyPasswordArgs → void` | false |
| M2-I01 | `crates/os-app/src/commands/api.rs` | `api.connect` | `ApiConnectArgs → ApiAccount` | true |
| M2-I01 | `crates/os-app/src/commands/api.rs` | `api.disconnect` | `ApiKindArgs → void` | false |
| M2-I01 | `crates/os-app/src/commands/api.rs` | `api.errors` | `ApiErrorsArgs → ApiErrorRow[]` | false |
| M2-I01 | `crates/os-app/src/commands/api.rs` | `api.fetch` | `ApiFetchArgs → ApiFetchSummary` | true |
| M2-I01 | `crates/os-app/src/commands/api.rs` | `api.getSettings` | `ApiKindArgs → ApiSourceSettings` | false |
| M2-I01 | `crates/os-app/src/commands/api.rs` | `api.list` | `void → ApiSourceInfo[]` | false |
| M2-I01 | `crates/os-app/src/commands/api.rs` | `api.removeAccount` | `ApiAccountArgs → void` | false |
| M2-I01 | `crates/os-app/src/commands/api.rs` | `api.setSettings` | `ApiSetSettingsArgs → ApiSettingsResult` | false |
| M2-I01 | `crates/os-app/src/commands/api.rs` | `api.status` | `void → ApiRow[]` | false |
| M2-I01 | `crates/os-app/src/commands/api.rs` | `api.testConnection` | `ApiKindArgs → ApiTestResult` | false |
| M2-I01 | `crates/os-app/src/commands/api.rs` | `sidebar.apiStatus` | `void → ApiRow[]` | false |
| M2-K02 | `crates/os-app/src/commands/search.rs` | `search.parse` | `SearchParseArgs → RuleNode` | false |
| M2-K02 | `crates/os-app/src/commands/search.rs` | `search.validateRegex` | `ValidateRegexArgs → void` | false |
| M2-K03 | `crates/os-app/src/commands/sidebar_ext.rs` | `sidebar.responseTimes` | `ResponseTimesArgs → ResponseTimes` | false |
| M2-K03 | `crates/os-app/src/commands/sidebar_ext.rs` | `sidebar.siteStructure` | `SiteStructureArgs → SiteStructure` | false |
| M2-K04 | `crates/os-app/src/commands/analysis.rs` | `analysis.start` | `AnalysisStartArgs → AnalysisStatus` | true |
| M2-K04 | `crates/os-app/src/commands/analysis.rs` | `analysis.status` | `void → AnalysisStatus` | false |
| M2-K04 | `crates/os-app/src/commands/app_quit.rs` | `app.confirmQuit` | `BoolArgs → void` | false |
| M2-K04 | `crates/os-app/src/commands/app_quit.rs` | `app.requestQuit` | `void → QuitDecision` | false |
| M2-K04 | `crates/os-app/src/commands/crawl.rs` | `crawl.clear` | `void → void` | false |
| M2-K04 | `crates/os-app/src/commands/crawl.rs` | `crawl.open` | `OpenCrawlArgs → CrawlSummaryDto` | false |
| M2-K04 | `crates/os-app/src/commands/crawl.rs` | `crawl.parseList` | `ListSource → ListPreview` | false |
| M2-K04 | `crates/os-app/src/commands/crawl.rs` | `crawl.pause` | `void → void` | false |
| M2-K04 | `crates/os-app/src/commands/crawl.rs` | `crawl.recrawl` | `SelectionArgs → CountResult` | true |
| M2-K04 | `crates/os-app/src/commands/crawl.rs` | `crawl.remove` | `SelectionArgs → CountResult` | true |
| M2-K04 | `crates/os-app/src/commands/crawl.rs` | `crawl.resume` | `void → void` | false |
| M2-K04 | `crates/os-app/src/commands/crawl.rs` | `crawl.saveAs` | `SaveAsArgs → FileResult` | true |
| M2-K04 | `crates/os-app/src/commands/crawl.rs` | `crawl.start` | `StartRequest → CrawlRef` | false |
| M2-K04 | `crates/os-app/src/commands/crawl.rs` | `crawl.stop` | `void → void` | false |
| M2-K04 | `crates/os-app/src/commands/crawl.rs` | `crawl.validateStartUrl` | `ValidateStartUrlArgs → NormalizedUrl` | false |
| M2-K09 | `crates/os-app/src/commands/detail_ext.rs` | `crawl.path` | `UrlIdArgs → CrawlPathDto[]` | false |
| M2-K09 | `crates/os-app/src/commands/detail_ext.rs` | `detail.duplicates` | `DuplicatesArgs → DuplicatesResult` | false |
| M2-K09 | `crates/os-app/src/commands/detail_ext.rs` | `detail.imagePreview` | `UrlIdArgs → ImagePreview` | false |
| M2-K09 | `crates/os-app/src/commands/detail_ext.rs` | `detail.lighthouse` | `LighthouseArgs → LighthouseDto` | false |
| M2-K09 | `crates/os-app/src/commands/detail_ext.rs` | `detail.screenshot` | `UrlIdArgs → ScreenshotInfo` | false |
| M2-K09 | `crates/os-app/src/commands/detail_ext.rs` | `detail.serp` | `SerpArgs → SerpData` | false |
| M2-K09 | `crates/os-app/src/commands/detail_ext.rs` | `detail.serpMeasure` | `SerpMeasureArgs → SerpMeasure` | false |
| M2-K09 | `crates/os-app/src/commands/detail_ext.rs` | `detail.serpReset` | `SerpEditArgs → void` | false |
| M2-K09 | `crates/os-app/src/commands/detail_ext.rs` | `detail.serpSaveEdit` | `SerpEditArgs → void` | false |
| M2-K09 | `crates/os-app/src/commands/detail_ext.rs` | `detail.sourceDiff` | `SourceDiffArgs → DiffHunks` | true |
| M2-K09 | `crates/os-app/src/commands/detail_ext.rs` | `detail.sourceLines` | `SourceLinesArgs → string[]` | false |
| M2-K09 | `crates/os-app/src/commands/detail_ext.rs` | `detail.sourceOpen` | `SourceOpenArgs → SourceDoc` | false |
| M2-K09 | `crates/os-app/src/commands/detail_ext.rs` | `detail.structuredData` | `UrlIdArgs → SdTree` | false |
| M2-K14 | `crates/os-app/src/commands/segment.rs` | `segment.delete` | `IdArgs → void` | false |
| M2-K14 | `crates/os-app/src/commands/segment.rs` | `segment.list` | `void → SegmentDef[]` | false |
| M2-K14 | `crates/os-app/src/commands/segment.rs` | `segment.preview` | `SegmentPreviewArgs → SegmentPreview` | true |
| M2-K14 | `crates/os-app/src/commands/segment.rs` | `segment.reassign` | `void → CountResult` | true |
| M2-K14 | `crates/os-app/src/commands/segment.rs` | `segment.save` | `SegmentSaveArgs → SegmentSaveResult` | false |
| M2-K14 | `crates/os-app/src/commands/segment.rs` | `segment.validate` | `SegmentValidateArgs → ValidationIssue[]` | false |
| M2-K14 | `crates/os-app/src/commands/segment.rs` | `sidebar.segments` | `SegmentsViewArgs → SegmentRow[]` | false |
| M2-K16 | `crates/os-app/src/commands/viz.rs` | `viz.exportCsv` | `VizExportArgs → FileResult` | true |
| M2-K16 | `crates/os-app/src/commands/viz.rs` | `viz.focusInMain` | `UrlIdArgs → void` | false |
| M2-K16 | `crates/os-app/src/commands/viz.rs` | `viz.graph` | `VizGraphArgs → GraphDto` | false |
| M2-K16 | `crates/os-app/src/commands/viz.rs` | `viz.open` | `VizOpenArgs → VizOpenResult` | false |
| M2-K16 | `crates/os-app/src/commands/viz.rs` | `viz.words` | `VizWordsArgs → WordFreq[]` | false |
| M2-K17 | `crates/os-app/src/commands/mcp.rs` | `mcp.callLog` | `PageArgs → CallLogRow[]` | false |
| M2-K17 | `crates/os-app/src/commands/mcp.rs` | `mcp.clientConfigPreview` | `McpClientArgs → McpClientPreview` | false |
| M2-K17 | `crates/os-app/src/commands/mcp.rs` | `mcp.clientConfigWrite` | `McpClientArgs → McpClientWritten` | false |
| M2-K17 | `crates/os-app/src/commands/mcp.rs` | `mcp.regenerateToken` | `McpTokenArgs → McpToken` | false |
| M2-K17 | `crates/os-app/src/commands/mcp.rs` | `mcp.saveToolsMarkdown` | `PathArgs → void` | false |
| M2-K17 | `crates/os-app/src/commands/mcp.rs` | `mcp.setAutostart` | `BoolArgs → void` | false |
| M2-K17 | `crates/os-app/src/commands/mcp.rs` | `mcp.setClasses` | `SetClassesArgs → void` | false |
| M2-K17 | `crates/os-app/src/commands/mcp.rs` | `mcp.setToolEnabled` | `SetToolEnabledArgs → void` | false |
| M2-K17 | `crates/os-app/src/commands/mcp.rs` | `mcp.start` | `McpStartArgs → McpStatus` | false |
| M2-K17 | `crates/os-app/src/commands/mcp.rs` | `mcp.status` | `void → McpStatus` | false |
| M2-K17 | `crates/os-app/src/commands/mcp.rs` | `mcp.stop` | `void → McpStatus` | false |
| M2-K17 | `crates/os-app/src/commands/mcp.rs` | `mcp.token` | `McpTokenArgs → McpToken` | false |
| M2-K17 | `crates/os-app/src/commands/mcp.rs` | `mcp.tools` | `void → ToolInfo[]` | false |
| M2-O01 | `crates/os-app/src/commands/auth.rs` | `auth.deleteProfile` | `NameArgs → void` | false |
| M2-O01 | `crates/os-app/src/commands/auth.rs` | `auth.exportProfile` | `AuthExportArgs → FileResult` | false |
| M2-O01 | `crates/os-app/src/commands/auth.rs` | `auth.importProfile` | `AuthImportArgs → AuthProfileInfo` | false |
| M2-O01 | `crates/os-app/src/commands/auth.rs` | `auth.listProfiles` | `void → AuthProfileInfo[]` | false |
| M2-O01 | `crates/os-app/src/commands/auth.rs` | `auth.saveProfile` | `AuthSaveProfileArgs → AuthProfileInfo` | false |
| M2-O01 | `crates/os-app/src/commands/diag.rs` | `diag.bundle` | `BundleArgs → FileResult` | true |
| M2-O01 | `crates/os-app/src/commands/diag.rs` | `diag.debugInfo` | `void → DebugInfo` | false |
| M2-O01 | `crates/os-app/src/commands/diag.rs` | `diag.logFrontend` | `LogFrontendArgs → void` | false |
| M2-O01 | `crates/os-app/src/commands/diag.rs` | `diag.openLogs` | `void → void` | false |
| M2-O01 | `crates/os-app/src/commands/diag.rs` | `diag.previewBundle` | `void → BundlePreview` | false |
| M2-O01 | `crates/os-app/src/commands/diag.rs` | `diag.usageStats` | `UsageStatsArgs → UsageStats` | false |
| M2-O02 | `crates/os-app/src/commands/crawls.rs` | `crawls.checkLocation` | `PathArgs → string[]` | false |
| M2-O02 | `crates/os-app/src/commands/crawls.rs` | `crawls.copyIds` | `IdsArgs → void` | false |
| M2-O02 | `crates/os-app/src/commands/crawls.rs` | `crawls.delete` | `IdsArgs → CountResult` | true |
| M2-O02 | `crates/os-app/src/commands/crawls.rs` | `crawls.duplicate` | `IdsArgs → CountResult` | true |
| M2-O02 | `crates/os-app/src/commands/crawls.rs` | `crawls.export` | `CrawlsExportArgs → CountResult` | true |
| M2-O02 | `crates/os-app/src/commands/crawls.rs` | `crawls.import` | `CrawlsImportArgs → ImportResult` | true |
| M2-O02 | `crates/os-app/src/commands/crawls.rs` | `crawls.list` | `CrawlListQuery → CrawlListing[]` | false |
| M2-O02 | `crates/os-app/src/commands/crawls.rs` | `crawls.lock` | `IdsArgs → void` | false |
| M2-O02 | `crates/os-app/src/commands/crawls.rs` | `crawls.move` | `CrawlMoveArgs → CountResult` | true |
| M2-O02 | `crates/os-app/src/commands/crawls.rs` | `crawls.rename` | `CrawlRenameArgs → void` | false |
| M2-O02 | `crates/os-app/src/commands/crawls.rs` | `crawls.reveal` | `IdsArgs → void` | false |
| M2-O02 | `crates/os-app/src/commands/crawls.rs` | `crawls.unlock` | `IdsArgs → void` | false |
| M2-O02 | `crates/os-app/src/commands/projects.rs` | `projects.create` | `ProjectCreateArgs → ProjectInfo` | false |
| M2-O02 | `crates/os-app/src/commands/projects.rs` | `projects.list` | `void → ProjectInfo[]` | false |
| M2-O02 | `crates/os-app/src/commands/projects.rs` | `projects.lock` | `ProjectLockArgs → void` | false |
| M2-O02 | `crates/os-app/src/commands/projects.rs` | `projects.rename` | `ProjectRenameArgs → void` | false |
| M2-T01 | `crates/os-app/src/commands/tasks.rs` | `tasks.delete` | `IdArgs → void` | false |
| M2-T01 | `crates/os-app/src/commands/tasks.rs` | `tasks.export` | `TasksExportArgs → FileResult` | true |
| M2-T01 | `crates/os-app/src/commands/tasks.rs` | `tasks.get` | `IdArgs → TaskDef` | false |
| M2-T01 | `crates/os-app/src/commands/tasks.rs` | `tasks.history` | `TasksHistoryArgs → TaskRunRow[]` | false |
| M2-T01 | `crates/os-app/src/commands/tasks.rs` | `tasks.import` | `PathArgs → TaskInfo[]` | true |
| M2-T01 | `crates/os-app/src/commands/tasks.rs` | `tasks.list` | `void → TaskInfo[]` | false |
| M2-T01 | `crates/os-app/src/commands/tasks.rs` | `tasks.register` | `TaskRegisterArgs → PlannerPreview` | false |
| M2-T01 | `crates/os-app/src/commands/tasks.rs` | `tasks.run` | `IdArgs → TaskRunRow` | true |
| M2-T01 | `crates/os-app/src/commands/tasks.rs` | `tasks.save` | `TaskArgs → TaskInfo` | false |
| M2-T01 | `crates/os-app/src/commands/tasks.rs` | `tasks.unregister` | `IdArgs → void` | false |
| M2-T01 | `crates/os-app/src/commands/tasks.rs` | `tasks.validate` | `TaskArgs → ValidationIssue[]` | false |
| M2-T02 | `crates/os-app/src/commands/mail.rs` | `mail.deleteAccount` | `IdArgs → void` | false |
| M2-T02 | `crates/os-app/src/commands/mail.rs` | `mail.listAccounts` | `void → MailAccountInfo[]` | false |
| M2-T02 | `crates/os-app/src/commands/mail.rs` | `mail.saveAccount` | `MailSaveAccountArgs → MailAccountInfo` | false |
| M2-T02 | `crates/os-app/src/commands/mail.rs` | `mail.testAccount` | `MailTestArgs → MailTestResult` | true |
| M2-U17 | `crates/os-app/src/commands/auth_login.rs` | `auth.finishLogin` | `AuthFinishLoginArgs → AuthProfileInfo | null` | false |
| M2-U17 | `crates/os-app/src/commands/auth_login.rs` | `auth.openLogin` | `AuthOpenLoginArgs → LoginCapture` | true |
| M2-X01 | `crates/os-app/src/commands/export_multi.rs` | `export.multi` | `ExportMultiArgs → ExportBatchResult` | true |
| M2-X01 | `crates/os-app/src/commands/export_multi.rs` | `export.presetDelete` | `ExportPresetArgs → void` | false |
| M2-X01 | `crates/os-app/src/commands/export_multi.rs` | `export.presetList` | `void → ExportPreset[]` | false |
| M2-X01 | `crates/os-app/src/commands/export_multi.rs` | `export.presetSave` | `ExportPresetArgs → void` | false |
| M2-X03 | `crates/os-app/src/commands/export_summary.rs` | `export.customSummary` | `CustomSummaryArgs → FileResult` | true |
| M2-X03 | `crates/os-app/src/commands/export_summary.rs` | `export.customSummaryMetrics` | `void → CustomSummaryMetric[]` | false |

Dienst-Stubs (synchron, Rückgabe `()`):

| Besitzer | Datei | Signatur |
|---|---|---|
| M2-O01 | `crates/os-app/src/logging.rs` | `pub fn init_for_app(_app: &crate::App)` |
| M2-O01 | `crates/os-app/src/logging.rs` | `pub fn apply_settings(_app: &crate::App)` |
| M2-I01 | `crates/os-app/src/commands/api.rs` | `pub fn on_startup(_app: &crate::App)` |
| M2-I01 | `crates/os-app/src/commands/api.rs` | `pub fn on_config_changed( _app: &crate::App, _key: &crate::pool::CrawlKey, _old: &std::sync::Arc<os_model::CrawlConfig>, _new: &std::sync::Arc<os_model::CrawlConfig>, )` |
| M2-O02 | `crates/os-app/src/commands/crawls.rs` | `pub fn start_retention(_app: &crate::App)` |
| M2-O02 | `crates/os-app/src/commands/crawls.rs` | `pub fn on_settings_changed(_app: &crate::App)` |
| M2-K03 | `crates/os-app/src/commands/sidebar_ext.rs` | `pub fn on_config_changed( _app: &crate::App, _key: &crate::pool::CrawlKey, _old: &std::sync::Arc<os_model::CrawlConfig>, _new: &std::sync::Arc<os_model::CrawlConfig>, )` |

## Prüfnachweis (17.10.2026, macOS arm64)

- `cargo fmt --all --check`: grün.
- `cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings`: grün.
- `cargo clippy -p openspider-desktop --locked -- -D warnings`: grün.
- `cargo test --workspace --exclude openspider-desktop --locked`: grün, 2.266 bestanden, 39 bestehende Ignores, keine neuen Ignores.
- `npm ci`, Desktop-Typecheck und Vitest: grün (133 Tests); `check:static` inklusive IPC-Generator und npm-Lizenzen: grün.
- `cargo deny check licenses bans sources`: grün (temporär installiertes cargo-deny 0.20.2).
- `python3 -I docs/plan/M2-check.py` und `scripts/check-neutral.sh`: grün.

Umgebungshinweis: Der unveränderte CLI-Test `a_python_reader_reads_a_finished_file_and_rejects_an_unknown_read_api_version` (OUT-234, `crates/os-cli/tests/read_api_export.rs:457`) scheitert mit dem System-Python / SQLite 3.51.0 beim Lesen von `q_manifest` mit „unable to open database file“. Derselbe Test besteht mit dem vorhandenen Homebrew-Python 3.10 / SQLite 3.43.2. Der vollständige grüne Workspace-Lauf verwendet diesen Interpreter als `python3` über einen temporären PATH-Eintrag. Keine Änderung an CLI, Python-Leser oder Tests außerhalb des Paketbesitzes.

Keine eigenen `spec_ids` im Paketmanifest: Die Tests prüfen Infrastruktur für UI-140, UI-143, CRAWL-141, OUT-184, OUT-225, OUT-241, OUT-242 und OUT-267; Fachabnahme bleibt bei den jeweiligen Besitzerpaketen. Ohne F05a ist Schema v3 aktuell; der vorbereitete Migrationstest prüft diesen Fall wie in 3.14.12 vorgesehen.

Kein Push, Merge oder PR; deshalb keine CI-Lauf-ID und kein Windows-Nachweis. Offene fachliche Fragen: keine.

## Review-Runde 1 (Fable)

- `window_closed` ruft zuerst die registrierten Hörer auf und führt danach `sessions.clear` aus. Hörer können damit noch auf die Sitzung zugreifen; `sessions_extensions_and_window_cleanup` prüft diese Reihenfolge.
- Linux-Dateimanager-Reveal liefert bewusst `Unsupported` (`err-desktop-platform`). Eine native Linux-Anbindung folgt erst mit einer gesonderten Plattformentscheidung.
- Die ursprüngliche Paketänderung hat 26 vorhandene JSON-Fixtures unter `apps/desktop/src/ipc/__fixtures__/` umformatiert. Die Umformatierung ändert keine Vertragswerte.
- F-11: `QuitDecision` ist mit `kind` getaggt und unterstützt `quit` sowie `confirm` mit Gründen für Crawl, Analyse, Export und HTTP-MCP-Sitzungen (UI-001). K04 übernimmt weiterhin die Entscheidungslogik; `WindowHost::exit` markiert kontrolliertes Beenden und beendet die Desktop-Hülle.
- Tick-Änderungen werden je abonniertem Fenster gepuffert. Beim nächsten fälligen Versand werden Zustände je Operation/View zusammengeführt und Meldungen in Reihenfolge zugestellt.
- Windows-Reveal initialisiert ein STA-COM-Apartment und räumt es auch bei Fehlern auf. Auf dem in Tauri-Setup erfassten Hauptthread wird Reveal direkt ausgeführt, damit kein synchrones Warten auf denselben Thread entsteht.
- Befund 4 (Draft-PR/CI-Matrix) bleibt laut Review beim Orchestrator nach dessen Push; diese Fix-Runde erzeugt weder Push noch PR.

Lokale Prüfumgebung der Fix-Runde: macOS, vollständiger Workspace einschließlich `openspider-desktop`. Für den Python-Beispielleser wird das vorhandene Homebrew-Python 3.10 verwendet (über einen temporären PATH-Eintrag, ohne Repository-Änderung). Apples Python 3.9 aus den Command Line Tools scheitert reproduzierbar in `os-cli::read_api_export::a_python_reader_reads_a_finished_file_and_rejects_an_unknown_read_api_version` beim Öffnen der Datenbank mit `unable to open database file`; dieselbe Test-Binärdatei besteht mit Homebrew-Python. Die Windows-/Linux-spezifischen Reveal-Tests sind hier nicht ausführbar und bleiben der Plattformmatrix des Orchestrators vorbehalten.

Abschlussprüfung der Fix-Runde: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (Homebrew-Python 3.10 wie oben), `npm ci`, `npm run -w apps/desktop typecheck` und `npm run -w apps/desktop test` sind grün (133 Desktop-Tests). Zusätzlich grün: `os-app-manifest --check` und `git diff --check`. Die neuen Regressionstests für Quit-Hook/Exit-Vertrag, beide Quit-Fixtures, TS-Export, verzögertes zweites Fenster mit Meldungen und finalem Operationszustand, Command-Reihenfolge und Upload-Reihenfolge-Default bestehen. Keine Änderungen außerhalb des Paketbesitzes.
