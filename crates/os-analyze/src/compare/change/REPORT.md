# M2-C02 – Umsetzungsbericht

## Umfang

TECH-300, TECH-301, TECH-302, TECH-303, TECH-304, TECH-305, TECH-307, TECH-308,
OUT-144, OUT-145, OUT-152, CRAWL-036.

- 14 kanonische Elemente; konfigurierte Auswahl, Schwelle und leere Auswahl; dynamische Spalten und Filter.
- Seitenpaare nach bestehendem URL-Mapping, ausschließlich beidseitig beantwortete interne HTML-Seiten.
- Normalisierter erster Text; numerische Änderungen aus den Writer-Zählern; Typmengen und Indexability-Grundmengen.
- MinHash aus M2-D02, 128 feste Permutationen, gespeicherte Signaturen oder gespeicherter Inhaltsbereich; gerundete strikte Schwelle und plattformunabhängiges CSV-Golden.
- SQLite-Temptabellen statt einer Sammlung aller Seitenwerte im RAM; Abbruch rollt die neue Berechnung zurück.
- Changed/Not Changed, SQL-Sortierung mit leerem Prozentwert als +∞, Rollenwechsel, Suche und Export.
- Auto Compare nach CLI-/Aufgaben-Crawl; unmittelbarer erfolgreicher Vorlauf desselben Projekts; automatische Analyse; unveränderliche `.oscompare`-SQLite-Artefakte mit Konfiguration, Klassifikationen, Paaren, Zellen und Mail-Zählern. Wiederholung erzeugt keinen zweiten Eintrag für dasselbe Paar/dieselbe Konfiguration.
- Optionale benannte Overview-Zähler und Content-Voraussetzungshinweis; alle App-Datenbankaufrufe des Vergleichs laufen in `spawn_blocking`.

## Validierung

Verbindliche Abschlussresultate und letzter Commit stehen in der Abschlussnachricht.
Rust-Gates: `cargo fmt --all --check`, `cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings`, `cargo test --workspace --exclude openspider-desktop --locked`.
Frontend wegen des additiv generierten DTO-Felds: `npm ci`, `npm run -w apps/desktop typecheck`, `npm run -w apps/desktop test` (72 Dateien, 700 Tests).
Zusätzlich `cargo deny check licenses bans sources`, npm-Lizenzprüfung (474 Pakete), `check:static` (28 Tests), Neutralitätsprüfung und lokaler CLI-Smoke: drei echte Loopback-Crawls, erster Lauf ohne Vorlauf mit Exit 0, beide Flag-Schreibweisen, zwei gespeicherte Vergleiche, Changed-CSV und Comparison-Status im normalen Tab-Export; ungültiger boolescher Wert und fehlendes Projekt mit Exit 2. Zwei zusätzliche Aufgabenläufe prüfen Vergleichs-Arbeitsmappen und das Überspringen leerer Change-Detection-Blätter; eine Aufgabe ohne explizites Projekt wird abgelehnt.

Fixtures liegen als deterministische SQLite-Site-Versionen in `tests/change_detection_fixture.rs`; das Golden liegt unter `change/content-change.golden.csv`. Diese Ablage nutzt die erlaubten eigenen Pfade, statt fremde Fixture-Verzeichnisse zu verändern.

## Abweichungen und Integration

- Keine Abhängigkeit geändert, kein Push/Merge/PR. Ausgangsworktree war sauber.
- Der SMTP-Versand und die Mail-Kontenverwaltung bleiben beim vorhandenen M2-T02-Stub. Dieses Paket liefert dafür die persistierte, getestete Tabelle über `change::auto::notification_counts`/`saved_counts`; `None` kennzeichnet den ersten Lauf ohne Vorlauf.
- Die Verwaltung/GUI für gespeicherte Vergleiche nach OUT-150 (P2) ist kein Teil von C02. Gespeicherte Auto-Vergleiche sind über `change::auto::load` lesbar; das Crawl-Metadatum `compare.latest` verweist darauf.
- Erwartungsänderung nach K2-63: `cmd_compare_basic.rs` erwartet bei explizit `elements: []` jetzt eine leere Auswahl (TECH-300); ohne Feld gelten die Crawl-Konfigurationsdefaults. Die Testlogik bleibt unverändert.
- macOS lokal; plattformübergreifende CI-Lauf-ID entfällt wegen des ausdrücklichen Push-Verbots. Das deterministische Golden ist für dieselben Tests auf macOS/Windows ausgelegt.

## Änderungen außerhalb von owns

Die folgenden Diffs dokumentieren vollständig die Ausnahmen. Eigene Implementierungen unter `compare/change`, der eigene Tab, eigene Tests und `commands/compare.rs` sind hiervon ausgenommen.

## Fremdergänzungen (K2-61)

### `crates/os-analyze/src/compare/mod.rs`

Export des neuen Moduls `compare::change`.

```diff
diff --git a/crates/os-analyze/src/compare/mod.rs b/crates/os-analyze/src/compare/mod.rs
index 7ddf54a5..bfe919e8 100644
--- a/crates/os-analyze/src/compare/mod.rs
+++ b/crates/os-analyze/src/compare/mod.rs
@@ -12,6 +12,7 @@
 //! Lange Schritte prüfen das [`CancelToken`] an Checkpoints: eine Pause des Ressourcenwächters (CRAWL-129, K2-56)
 //! hält die Vergleichsberechnung an, statt sie abzubrechen; ein Abbruch liefert [`CompareError::Cancelled`].
 
+pub mod change;
 pub mod classify;
 pub mod links;
 pub mod mapping;
```

### `crates/os-app/src/dto/catalog.rs`

Optionales `FilterInfo.comparison` mit serde-Default; benannte Current/Previous/Change-Zähler. Bestehende Filter serialisieren weiterhin ohne dieses Feld.

```diff
diff --git a/crates/os-app/src/dto/catalog.rs b/crates/os-app/src/dto/catalog.rs
index 285f342b..b9ed87cf 100644
--- a/crates/os-app/src/dto/catalog.rs
+++ b/crates/os-app/src/dto/catalog.rs
@@ -70,6 +70,13 @@ pub struct IssueBadge {
 #[serde(rename_all = "camelCase")]
 #[ts(optional_fields = nullable)]
 pub struct FilterInfo {
+    /// Vergleichszähler (TECH-301); bei gewöhnlichen Filtern nicht gesetzt.
+    #[serde(default, skip_serializing_if = "Option::is_none")]
+    #[ts(
+        type = "{ current: number; previous: number; change: number } | null",
+        optional
+    )]
+    pub comparison: Option<ChangeCountDto>,
     /// Kanonische Kennung (mit aufgelösten Schwellen, z. B. `Over 60 Characters`).
     pub id: String,
     /// Vorlage der Kennung mit Platzhaltern (für Übersetzung).
@@ -88,6 +95,14 @@ pub struct FilterInfo {
     pub analysis_step: Option<String>,
 }
 
+/// Anzahlen geänderter URLs in einem Change-Detection-Filter (TECH-301).
+#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
+pub struct ChangeCountDto {
+    pub current: u64,
+    pub previous: u64,
+    pub change: u64,
+}
+
 /// Ein Tab der oberen Tabelle (`catalog.tabs`, ohne Zähler).
 #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
 #[serde(rename_all = "camelCase")]
```

### `apps/desktop/src/ipc/gen/FilterInfo.ts`

Nur die zusätzliche optionale Eigenschaft aus `os-app-manifest` übernommen; übrige generierte Dateien sind unverändert.

```diff
diff --git a/apps/desktop/src/ipc/gen/FilterInfo.ts b/apps/desktop/src/ipc/gen/FilterInfo.ts
index ad830c31..f08cdea5 100644
--- a/apps/desktop/src/ipc/gen/FilterInfo.ts
+++ b/apps/desktop/src/ipc/gen/FilterInfo.ts
@@ -7,6 +7,10 @@ import type { JsonValue } from "./serde_json/JsonValue";
  * Ein Filter mit Zähler (`catalog.filterCounts`, `sidebar.overview`).
  */
 export type FilterInfo = { 
+/**
+ * Vergleichszähler (TECH-301); bei gewöhnlichen Filtern nicht gesetzt.
+ */
+comparison?: { current: number; previous: number; change: number } | null, 
 /**
  * Kanonische Kennung (mit aufgelösten Schwellen, z. B. `Over 60 Characters`).
  */
```

### `crates/os-cli/src/plan.rs`

Neues boolesches Planfeld (Standard ohne Flag: false), Initialisierung und Validierung der aktivierten Option; die Flag-Ablösung gehört außerdem zu K2-71.

```diff
diff --git a/crates/os-cli/src/plan.rs b/crates/os-cli/src/plan.rs
index 6b5eb1f5..687125a5 100644
--- a/crates/os-cli/src/plan.rs
+++ b/crates/os-cli/src/plan.rs
@@ -54,6 +54,8 @@ pub struct BatchSettings {
 /// Ablaufplan nach der Validierung.
 #[derive(Debug)]
 pub struct Plan {
+    /// Auto-Vergleich nach erfolgreichem Abschluss (OUT-152).
+    pub compare_with_previous: bool,
     /// Stapelmodus der Exporte (geplante Aufgaben); `None` = einzeln wie auf der Kommandozeile.
     pub batch: Option<BatchSettings>,
     /// Optionaler Segmentkontext; ohne Flag bleibt der Export ungefiltert.
@@ -346,6 +348,26 @@ fn build_impl(
         )));
     }
 
+    let compare_with_previous =
+        match args.project_crawl_comparison.as_ref() {
+            None => args.compare_with_previous,
+            Some(Some(value)) if value == "true" => true,
+            Some(Some(value)) if value == "false" && !args.compare_with_previous => false,
+            _ => return Err(CliError::usage(
+                "--project-crawl-comparison requires true or false; conflicting comparison options",
+            )),
+        };
+    if compare_with_previous
+        && (args
+            .project_name
+            .as_ref()
+            .is_none_or(|p| p.trim().is_empty())
+            || args.no_store)
+    {
+        return Err(CliError::usage(
+            "Auto comparison requires --project-name and a stored crawl",
+        ));
+    }
     let format = parse_format(args)?;
 
     // Quelle
@@ -493,6 +515,7 @@ fn build_impl(
         }
     }
     Ok(Plan {
+        compare_with_previous,
         batch,
         segment: args.segment.clone(),
         source,
```

### `crates/os-cli/src/exec.rs`

Zusätzlicher Auto-Compare-Aufruf und zusätzliche Exportfunktionen für den explizit angeforderten Vergleichskontext, einschließlich Aufgaben-Arbeitsmappen. Der normale Ablauf ohne Option bleibt erhalten.

```diff
diff --git a/crates/os-cli/src/exec.rs b/crates/os-cli/src/exec.rs
index 4c0bc3f1..e378aa59 100644
--- a/crates/os-cli/src/exec.rs
+++ b/crates/os-cli/src/exec.rs
@@ -134,6 +134,23 @@ async fn run(plan: &Plan, rep: &Arc<Reporter>, signals: &Signals) -> Result<Outc
         }
     }
 
+    if plan.compare_with_previous {
+        match os_analyze::compare::change::auto::run(
+            &crawled.path,
+            &plan.paths.crawls,
+            &plan.paths.crawls.join("comparisons"),
+            Some(&plan.config.compare),
+            &signals.cancel_token(),
+        ) {
+            Ok(os_analyze::compare::change::auto::AutoResult::NoPrevious) => {
+                rep.info("kein vorheriger Crawl")
+            }
+            Ok(os_analyze::compare::change::auto::AutoResult::Saved(path)) => {
+                rep.info(format!("Vergleich gespeichert: {}", path.display()))
+            }
+            Err(e) => rep.warn(format!("Auto-Vergleich: {e}")),
+        }
+    }
     let mut failures = 0u32;
     let mut files = 0usize;
     if plan.output.is_some() {
@@ -613,6 +630,23 @@ fn export_phase(plan: &Plan, rep: &Reporter, signals: &Signals, crawled: &Crawle
         if signals.is_interrupted() {
             return ExportEnd::Interrupted;
         }
+        if plan.compare_with_previous {
+            if let Some(result) =
+                comparison_export(&file, &cfg, item, out, plan.format, &signals.cancel_token())
+            {
+                match result {
+                    Ok(info) => {
+                        report_file(rep, &info);
+                        files += 1;
+                    }
+                    Err(message) => {
+                        failures += 1;
+                        rep.error(message);
+                    }
+                }
+                continue;
+            }
+        }
         if let Some(segment) = plan.segment.as_deref() {
             match run_segment_export(&file, &cfg, item, out, plan.format, segment, rep) {
                 Ok(n) => files += n,
@@ -711,6 +745,9 @@ fn run_batch(
     signals: &Signals,
     rep: &Reporter,
 ) -> Result<usize, BatchEnd> {
+    if plan.compare_with_previous && file.get_meta("compare.latest").ok().flatten().is_some() {
+        return comparison_batch(file, cfg, plan, batch, out, signals, rep);
+    }
     let selections: Vec<os_export::types::ExportSelection> = plan
         .exports
         .iter()
@@ -807,3 +844,227 @@ fn run_segment_export(
     files.iter().for_each(|info| report_file(rep, info));
     Ok(files.len())
 }
+
+/// Vergleichs-Tabellen verwenden die gespeicherte Zuordnung und Ergebnisse (OUT-152).
+fn comparison_export(
+    file: &CrawlFile,
+    cfg: &CrawlConfig,
+    item: &Resolved,
+    out: &Path,
+    format: Format,
+    cancel: &os_model::CancelToken,
+) -> Option<Result<ExportFileInfo, String>> {
+    let Resolved::Tab { tab, filter } = item else {
+        return None;
+    };
+    let path = match file.get_meta("compare.latest") {
+        Ok(Some(path)) => path,
+        Ok(None) => return None,
+        Err(e) => return Some(Err(e.to_string())),
+    };
+    Some((|| {
+        let (mut c, config) = os_analyze::compare::change::auto::load(Path::new(&path), cancel)
+            .map_err(|e| e.to_string())?;
+        let mut q = ViewQuery::new(tab, filter);
+        if tab == "Change Detection" {
+            let def =
+                os_analyze::compare::change::tab_for_query(&config.change_detection, true, &mut q);
+            return os_export::tab_export::export_tab_in(
+                &[def],
+                &c.current().file,
+                cfg,
+                &q,
+                out,
+                format,
+            )
+            .map_err(|e| e.to_string());
+        }
+        let name = format!("{tab}:{filter}");
+        let target = out.join(os_export::naming::file_name(
+            &name,
+            None,
+            format.extension(),
+        ));
+        let mut writer = os_export::open_writer(
+            &target,
+            format,
+            &name,
+            &os_export::pipeline::writer_options(cfg),
+        )
+        .map_err(|e| e.to_string())?;
+        let mut ctx = os_export::types::ExportCtx {
+            cancel,
+            progress: &|_| {},
+            skip_empty: false,
+            workbook: None,
+            skipped: vec![],
+        };
+        let spec = os_export::compare::TabSelection {
+            tab: tab.clone(),
+            filter: filter.clone(),
+            class: None,
+            columns: q.columns,
+        };
+        os_export::compare::write_tab(&mut c, &spec, writer.as_mut(), &mut ctx)
+            .map_err(|e| e.to_string())?;
+        writer.finish().map_err(|e| e.to_string())
+    })())
+}
+
+/// Scheduling verwendet dieselben Vergleichs-Tabellen auch in Arbeitsmappen (OUT-152).
+fn comparison_batch(
+    file: &CrawlFile,
+    cfg: &CrawlConfig,
+    plan: &Plan,
+    batch: crate::plan::BatchSettings,
+    out: &Path,
+    signals: &Signals,
+    rep: &Reporter,
+) -> Result<usize, BatchEnd> {
+    use os_export::types::TableWriter;
+    let run = || -> Result<usize, String> {
+        let token = signals.cancel_token();
+        let path = file
+            .get_meta("compare.latest")
+            .map_err(|e| e.to_string())?
+            .ok_or("comparison missing")?;
+        let (mut comparison, config) =
+            os_analyze::compare::change::auto::load(Path::new(&path), &token)
+                .map_err(|e| e.to_string())?;
+        let name = file.meta().map_err(|e| e.to_string())?.name;
+        let destination = out.join(os_export::naming::file_name(
+            &format!("{name} export"),
+            None,
+            "xlsx",
+        ));
+        let mut workbook = if batch.single_workbook && plan.format == Format::Xlsx {
+            let mut w = os_export::xlsx::XlsxWriter::create(
+                &destination,
+                "Export",
+                &os_export::pipeline::writer_options(cfg),
+            )
+            .map_err(|e| e.to_string())?;
+            w.set_cancel(token.clone());
+            Some(w)
+        } else {
+            None
+        };
+        let mut files = 0;
+        for item in &plan.exports {
+            if token.is_cancelled() {
+                return Err("Export cancelled".into());
+            }
+            let mut ctx = os_export::types::ExportCtx {
+                cancel: &token,
+                progress: &|_| {},
+                skip_empty: batch.skip_empty,
+                workbook: workbook
+                    .as_mut()
+                    .map(|w| w as &mut dyn os_export::types::WorkbookTarget),
+                skipped: vec![],
+            };
+            if let Resolved::Tab { tab, filter } = item {
+                let label = format!("{tab}:{filter}");
+                let target = out.join(os_export::naming::file_name(
+                    &label,
+                    None,
+                    plan.format.extension(),
+                ));
+                let result = if tab == "Change Detection" {
+                    let mut q = ViewQuery::new(tab, filter);
+                    let def = os_analyze::compare::change::tab_for_query(
+                        &config.change_detection,
+                        true,
+                        &mut q,
+                    );
+                    os_export::pipeline::export_view_with(
+                        &[def],
+                        &comparison.current().file,
+                        cfg,
+                        &q,
+                        &target,
+                        plan.format,
+                        &label,
+                        false,
+                        &mut ctx,
+                    )
+                } else {
+                    os_export::output::write_table(
+                        &target,
+                        plan.format,
+                        &label,
+                        &os_export::pipeline::writer_options(cfg),
+                        &mut ctx,
+                        |writer| {
+                            let selection = os_export::compare::TabSelection {
+                                tab: tab.clone(),
+                                filter: filter.clone(),
+                                class: None,
+                                columns: os_analyze::views::ColumnSelection::Default,
+                            };
+                            let mut inner = os_export::types::ExportCtx {
+                                cancel: &token,
+                                progress: &|_| {},
+                                skip_empty: false,
+                                workbook: None,
+                                skipped: vec![],
+                            };
+                            os_export::compare::write_tab(
+                                &mut comparison,
+                                &selection,
+                                writer,
+                                &mut inner,
+                            )
+                            .map(|_| ())
+                        },
+                    )
+                }
+                .map_err(|e| e.to_string())?;
+                if let Some(info) = result {
+                    report_file(rep, &info);
+                    files += 1;
+                }
+            } else {
+                let (kind, name) = match item {
+                    Resolved::Bulk(n) => ("bulk", n.clone()),
+                    Resolved::Issues => ("bulk", os_export::catalog::bulk_issues::BULK_NAME.into()),
+                    Resolved::Report(n) => ("report", n.clone()),
+                    _ => unreachable!(),
+                };
+                let selection = os_export::types::ExportSelection {
+                    kind: kind.into(),
+                    name: name.clone(),
+                };
+                let options = os_export::pipeline::BatchOptions {
+                    format: plan.format,
+                    directory: out,
+                    crawl_name: &name,
+                    single_workbook: false,
+                };
+                let result =
+                    os_export::pipeline::export_batch(file, cfg, &[selection], &options, &mut ctx)
+                        .map_err(|e| e.to_string())?;
+                for info in &result.files {
+                    report_file(rep, info);
+                }
+                files += result.files.len();
+                for warning in result.warnings {
+                    rep.warn(warning);
+                }
+            }
+        }
+        if let Some(workbook) = workbook {
+            if workbook.has_tables() {
+                let info = Box::new(workbook).finish().map_err(|e| e.to_string())?;
+                report_file(rep, &info);
+                files += 1;
+            }
+        }
+        Ok(files)
+    };
+    match run() {
+        Ok(n) => Ok(n),
+        Err(_) if signals.is_interrupted() => Err(BatchEnd::Interrupted),
+        Err(e) => Err(BatchEnd::Failed(e)),
+    }
+}
```

## Platzhalter-Ablösung (K2-71)

### `crates/os-app/src/commands/catalog.rs`

TECH-300/301: bislang bedingungslos sichtbarer leerer Tab; Routing von Metadaten, Spalten und Zählern an die fenstergebundene Vergleichssitzung. `comparison: None` ist die zu K2-61 gehörende Feldinitialisierung.

```diff
diff --git a/crates/os-app/src/commands/catalog.rs b/crates/os-app/src/commands/catalog.rs
index 79c27fa3..74ed4b0f 100644
--- a/crates/os-app/src/commands/catalog.rs
+++ b/crates/os-app/src/commands/catalog.rs
@@ -82,6 +82,7 @@ pub(crate) fn info(tab: &str, filter: &defs::FilterDef, cfg: &CrawlConfig) -> Fi
         priority: priority(e.priority),
     });
     FilterInfo {
+        comparison: None,
         id: names::label(filter.id, cfg),
         label_template: filter.id.into(),
         params: {
@@ -293,30 +294,14 @@ pub(crate) fn expanded_columns(
         .collect()
 }
 fn get_tabs<'a>(app: &'a App, ctx: CallCtx, _: serde_json::Value) -> BoxFuture<'a, CmdResult> {
-    Box::pin(async move {
-        let cfg = app
-            .sessions()
-            .current(&ctx.window)
-            .map(|s| s.config)
-            .unwrap_or_default();
-        let tabs = tabs::configured(&cfg)
-            .iter()
-            .enumerate()
-            .map(|(i, t)| TabInfo {
-                id: t.id.into(),
-                default_order: i as u16,
-                filters: t.filters.iter().map(|f| info(t.id, f, &cfg)).collect(),
-                // Every registered tab has URL-backed rows (TabDef.population over urls u),
-                // so URL Details and link detail views are available (ONPAGE-036/UI-031).
-                detail_capable: true,
-            })
-            .collect::<Vec<_>>();
-        to_json(&tabs)
-    })
+    Box::pin(async move { to_json(&super::compare::change_tabs(app, &ctx).await?) })
 }
 fn columns<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
     Box::pin(async move {
         let args: ColumnsArgs = parse_args(args)?;
+        if let Some(result) = super::compare::change_columns(app, &ctx, &args).await {
+            return result;
+        }
         let current = app.sessions().current(&ctx.window);
         let cfg = current
             .as_ref()
@@ -353,7 +338,19 @@ fn columns<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture
 fn counts<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
     Box::pin(async move {
         let args: ScopeArgs = parse_args(args)?;
-        check_scope(&args.segment, &args.compare)?;
+        if let Some(result) = super::compare::change_overview(app, &ctx, &args, false).await {
+            return result;
+        }
+        let change_rows = super::compare::change_counts(app, &ctx, args.compare).await?;
+        let compare_scope = if matches!(
+            args.compare,
+            Some(CompareClass::Changed | CompareClass::NotChanged)
+        ) {
+            None
+        } else {
+            args.compare
+        };
+        check_scope(&args.segment, &compare_scope)?;
         let s = session(app, &ctx)?;
         app.read_pool()
             .run_snapshot(s.key, move |file, _| {
@@ -362,10 +359,10 @@ fn counts<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<
                 Ok(views::filter_counts_in(&registry, file, &s.config)
                     .map_err(view_error)
                     .and_then(|counts| {
-                        to_json(&FilterCounts {
-                            version,
-                            filters: count_rows(&registry, &s.config, &counts),
-                        })
+                        let mut filters = count_rows(&registry, &s.config, &counts);
+                        filters.retain(|r| r.tab != "Change Detection");
+                        filters.extend(change_rows);
+                        to_json(&FilterCounts { version, filters })
                     }))
             })
             .await?
```

### `crates/os-app/src/commands/sidebar.rs`

TECH-300/301/307: Change-Detection-Overview und Changed/Not Changed statt des bisherigen Unsupported-/leeren Pfades.

```diff
diff --git a/crates/os-app/src/commands/sidebar.rs b/crates/os-app/src/commands/sidebar.rs
index 2b39a514..7fd84e54 100644
--- a/crates/os-app/src/commands/sidebar.rs
+++ b/crates/os-app/src/commands/sidebar.rs
@@ -25,7 +25,19 @@ pub(crate) fn summary_rows(summary: Vec<(String, u64)>) -> Vec<SummaryRow> {
 fn overview<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
     Box::pin(async move {
         let args: ScopeArgs = parse_args(args)?;
-        catalog::check_scope(&args.segment, &args.compare)?;
+        if let Some(result) = super::compare::change_overview(app, &ctx, &args, true).await {
+            return result;
+        }
+        let change_rows = super::compare::change_counts(app, &ctx, args.compare).await?;
+        let compare_scope = if matches!(
+            args.compare,
+            Some(CompareClass::Changed | CompareClass::NotChanged)
+        ) {
+            None
+        } else {
+            args.compare
+        };
+        catalog::check_scope(&args.segment, &compare_scope)?;
         let s = session(app, &ctx)?;
         app.read_pool()
             .run_snapshot(s.key, move |file, _| {
@@ -36,7 +48,9 @@ fn overview<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFutur
                     os_analyze::views::filter_counts_in(&registry, file, &s.config)
                         .map_err(view_error)
                         .and_then(|counts| {
-                            let rows = catalog::count_rows(&registry, &s.config, &counts);
+                            let mut rows = catalog::count_rows(&registry, &s.config, &counts);
+                            rows.retain(|r| r.tab != "Change Detection");
+                            rows.extend(change_rows);
                             let tabs = registry
                                 .iter()
                                 .filter_map(|t| {
```

### `crates/os-app/src/commands/view.rs`

TECH-301/307: frühes Routing ausschließlich für Change-Detection-Views; ersetzt den bisherigen leeren bzw. Unsupported-Pfad für diese Views.

```diff
diff --git a/crates/os-app/src/commands/view.rs b/crates/os-app/src/commands/view.rs
index b793f112..c9289701 100644
--- a/crates/os-app/src/commands/view.rs
+++ b/crates/os-app/src/commands/view.rs
@@ -829,6 +829,9 @@ async fn page_query(
 }
 fn open<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
     Box::pin(async move {
+        if let Some(result) = super::compare::change_view(app, &ctx, "view.open", &args).await {
+            return result;
+        }
         let args: ViewOpenArgs = parse_args(args)?;
         let s = session(app, &ctx)?;
         let id = app
@@ -978,6 +981,9 @@ fn handle(view_id: String, p: DataPage) -> ViewHandle {
 }
 fn update<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
     Box::pin(async move {
+        if let Some(result) = super::compare::change_view(app, &ctx, "view.update", &args).await {
+            return result;
+        }
         let args: ViewUpdateArgs = parse_args(args)?;
         let mut v = get_owned(app, &ctx, &args.view_id)?;
         v.session = session(app, &ctx)?;
@@ -1027,6 +1033,9 @@ fn update<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<
 }
 fn rows<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
     Box::pin(async move {
+        if let Some(result) = super::compare::change_view(app, &ctx, "view.rows", &args).await {
+            return result;
+        }
         let args: RowsArgs = parse_args(args)?;
         let v = get(app, &ctx, &args.view_id)?;
         if !(1..=500).contains(&args.limit) {
@@ -1149,6 +1158,9 @@ fn copy<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a
 }
 fn close<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
     Box::pin(async move {
+        if let Some(result) = super::compare::change_view(app, &ctx, "view.close", &args).await {
+            return result;
+        }
         let args: ViewCloseArgs = parse_args(args)?;
         let views = registry(app);
         let mut entries = views.entries.lock().unwrap_or_else(|p| p.into_inner());
```

### `crates/os-app/src/commands/export.rs`

TECH-307: frühes Routing ausschließlich für den Export solcher Views, mit aktivem Umschalter.

```diff
diff --git a/crates/os-app/src/commands/export.rs b/crates/os-app/src/commands/export.rs
index aa49331d..ed5b90ff 100644
--- a/crates/os-app/src/commands/export.rs
+++ b/crates/os-app/src/commands/export.rs
@@ -349,6 +349,9 @@ fn export_view<'a>(
     args: serde_json::Value,
 ) -> BoxFuture<'a, CmdResult> {
     Box::pin(async move {
+        if let Some(result) = super::compare::change_export(app, &ctx, &args).await {
+            return result;
+        }
         let args: ExportViewArgs = parse_args(args)?;
         let fmt = format(args.format)?;
         let v = view::get(app, &ctx, &args.view_id)?;
```

### `crates/os-cli/src/args.rs`

OUT-152/TECH-308/CRAWL-036: nur `--compare-with-previous` und `--project-crawl-comparison` aus der Unsupported-Liste entfernt, Hilfe aktualisiert. `--crawl-comparison` bleibt bei M2-L02.

```diff
diff --git a/crates/os-cli/src/args.rs b/crates/os-cli/src/args.rs
index 6417ef62..1b1e697d 100644
--- a/crates/os-cli/src/args.rs
+++ b/crates/os-cli/src/args.rs
@@ -125,11 +125,11 @@ pub struct RunArgs {
     #[arg(long, num_args = 0.., value_name = "ID1 ID2", help_heading = "Vergleich")]
     pub crawl_comparison: Option<Vec<String>>,
 
-    /// [noch nicht unterstützt] Vergleicht mit dem vorherigen Crawl des Projekts.
+    /// Vergleicht mit dem vorherigen erfolgreichen Crawl des Projekts.
     #[arg(long, num_args = 0..=1, value_name = "true|false", help_heading = "Vergleich")]
     pub project_crawl_comparison: Option<Option<String>>,
 
-    /// [noch nicht unterstützt] Alias für `--project-crawl-comparison true`.
+    /// Alias für `--project-crawl-comparison true`.
     #[arg(long, help_heading = "Vergleich")]
     pub compare_with_previous: bool,
 
@@ -284,7 +284,7 @@ pub fn long_option_names() -> Vec<String> {
 impl RunArgs {
     /// Die noch nicht unterstützten Optionen, die der Aufruf enthält (Namen mit `--`).
     pub fn unsupported(&self) -> Vec<&'static str> {
-        let checks: [(bool, &'static str); 17] = [
+        let checks: [(bool, &'static str); 15] = [
             (self.crawl_google_sheet.is_some(), "--crawl-google-sheet"),
             (
                 self.google_drive_account.is_some(),
@@ -292,11 +292,6 @@ impl RunArgs {
             ),
             (self.auth_config.is_some(), "--auth-config"),
             (self.crawl_comparison.is_some(), "--crawl-comparison"),
-            (
-                self.project_crawl_comparison.is_some(),
-                "--project-crawl-comparison",
-            ),
-            (self.compare_with_previous, "--compare-with-previous"),
             (
                 self.use_google_analytics_4.is_some(),
                 "--use-google-analytics-4",
```

### `crates/os-cli/src/run_task.rs`

OUT-152: Unsupported-Rumpf des Vergleichshooks durch Auto Compare ersetzt; Aufgabenoption zusätzlich vor dem Export an den Plan weitergegeben.

```diff
diff --git a/crates/os-cli/src/run_task.rs b/crates/os-cli/src/run_task.rs
index b3c7d227..6d55f181 100644
--- a/crates/os-cli/src/run_task.rs
+++ b/crates/os-cli/src/run_task.rs
@@ -59,11 +59,20 @@ pub trait TaskHooks: Send + Sync {
     }
 
     /// Vergleich mit dem vorherigen Crawl des Projekts (OUT-152).
-    fn compare_with_previous(&self, _task: &Task, _crawl: Option<&Path>) -> Result<(), String> {
-        Err(
-            "comparison with the previous crawl is not available in headless runs of this version"
-                .into(),
+    fn compare_with_previous(&self, _task: &Task, crawl: Option<&Path>) -> Result<(), String> {
+        let crawl = crawl.ok_or("Auto comparison: no crawl file available")?;
+        let dir = crawl
+            .parent()
+            .ok_or("Auto comparison: crawl directory missing")?;
+        os_analyze::compare::change::auto::run(
+            crawl,
+            dir,
+            &dir.join("comparisons"),
+            None,
+            &os_model::CancelToken::new(),
         )
+        .map_err(|e| e.to_string())?;
+        Ok(())
     }
 
     /// Zeitreihen-Zeile für Looker Studio (OUT-210); Rückgabe = Hinweise (Warnungen) des Laufs.
@@ -318,6 +327,7 @@ fn to_run_args(task: &Task, paths: &AppPaths, exports: &ExportPlan, export_dir:
         Source::Spider { url } => args.crawl = Some(url.trim().to_string()),
         Source::List { .. } => args.crawl_list = task.crawl.source.list_sources(),
     }
+    args.compare_with_previous = task.execution.auto_compare;
     args.config = task
         .crawl
         .config
```

### `crates/os-sched/src/tasks/validate.rs`

OUT-152: fehlende Prüfung ergänzt, dass die aktivierte Aufgabenoption einen expliziten Projektnamen braucht; der Aufgabenname als Fallback genügt nicht.

```diff
diff --git a/crates/os-sched/src/tasks/validate.rs b/crates/os-sched/src/tasks/validate.rs
index 075342cb..6ee50099 100644
--- a/crates/os-sched/src/tasks/validate.rs
+++ b/crates/os-sched/src/tasks/validate.rs
@@ -147,6 +147,14 @@ pub fn start_url_valid(text: &str) -> bool {
 pub fn validate(task: &Task, env: &dyn Env) -> Vec<Issue> {
     let mut out = Vec::new();
 
+    // OUT-152: Auto Compare braucht einen expliziten Projektnamen; der Aufgabenname ist kein Projekt.
+    if task.execution.auto_compare && task.project.trim().is_empty() {
+        out.push(Issue::error(
+            "project",
+            "tasks-issue-auto-compare-project-required",
+        ));
+    }
+
     // Allgemein
     if task.name.trim().is_empty() {
         out.push(Issue::error("name", "tasks-issue-name-required"));
```

## Stub-Ablösung (K2-60)

### `crates/os-engine/f05a-stubs.toml`

Genau die beiden Stub-Verweise auf den eigenen Change-Detection-Tab entfernt (F05a/F02).

```diff
diff --git a/crates/os-engine/f05a-stubs.toml b/crates/os-engine/f05a-stubs.toml
index 5395cea6..a6bc3f71 100644
--- a/crates/os-engine/f05a-stubs.toml
+++ b/crates/os-engine/f05a-stubs.toml
@@ -2,12 +2,6 @@
 # Signaturen der mit STUB markierten Erweiterungspunkte; generated from the owned source files.
 package = "M2-F05a"
 
-[[stub]]
-owner = "M2-C02"
-file = "crates/os-analyze/src/tabs/change_detection.rs"
-function = "tab"
-signature = "pub fn tab() -> TabDef"
-
 [[stub]]
 owner = "M2-D02"
 file = "crates/os-analyze/src/tabs/duplicates.rs"
@@ -62,12 +56,6 @@ file = "crates/os-analyze/src/tabs/analytics.rs"
 function = "tab"
 signature = "pub fn tab() -> TabDef"
 
-[[stub]]
-owner = "M2-F02"
-file = "crates/os-analyze/src/tabs/change_detection.rs"
-function = "tab"
-signature = "pub fn tab() -> TabDef"
-
 [[stub]]
 owner = "M2-F02"
 file = "crates/os-analyze/src/tabs/custom_js.rs"
```

### `crates/os-analyze/tests/filters_registry.rs`

Erwartete Stub-Tab-Anzahl 5 → 4, weil TECH-301 jetzt implementiert ist; keine Testlogik verändert.

```diff
diff --git a/crates/os-analyze/tests/filters_registry.rs b/crates/os-analyze/tests/filters_registry.rs
index 37be6618..dba816fa 100644
--- a/crates/os-analyze/tests/filters_registry.rs
+++ b/crates/os-analyze/tests/filters_registry.rs
@@ -288,7 +288,7 @@ fn every_tab_is_a_real_definition_or_a_marked_stub() {
     // M2-D01 implements Meta Keywords; M2-D05 Pagination, M2-D14 Custom Search, M2-I03 Search Console, M2-I04 PageSpeed, M2-I02 Analytics.
     // M2-I05 ersetzt den Link-Metrics-Stub (K2-60, INTEG-151).
     assert_eq!(
-        stubs, 5,
+        stubs, 4,
         "Stub-Tabs (Structured Data: M2-D09, Validation: M2-D10, Custom Extraction: M2-D15)"
     );
 }
```

## Fremdergänzungen (K2-61): zusätzliche Defaults

`crates/os-app/src/dto/compare.rs`: serde-Default für ein fehlendes `elements`-Feld; der Rust-Feldtyp bleibt unverändert. `compare.open` unterscheidet dadurch fehlende Auswahl (Konfigurationsdefaults) von einer explizit leeren Auswahl (Tab aus, TECH-300).

```diff
diff --git a/crates/os-app/src/dto/compare.rs b/crates/os-app/src/dto/compare.rs
index 43c446b4..adebeb4d 100644
--- a/crates/os-app/src/dto/compare.rs
+++ b/crates/os-app/src/dto/compare.rs
@@ -27,6 +27,7 @@ pub struct CompareOpenArgs {
     /// URL-Mapping.
     pub mapping: Vec<MappingRuleDto>,
     /// Elemente der Change Detection (Kennungen laut TECH-300).
+    #[serde(default)]
     pub elements: Vec<String>,
     /// Schwelle für Inhaltsänderungen in Prozent.
     pub content_threshold_pct: Option<u8>,
```

## Erwartungsänderungen (K2-63): Vergleichsoptionen

`crates/os-app/tests/cmd_compare_basic.rs`, TECH-300: Die Erwartung für explizit `elements: []` wechselt von nicht leer zu leer; keine Testlogik geändert.

```diff
diff --git a/crates/os-app/tests/cmd_compare_basic.rs b/crates/os-app/tests/cmd_compare_basic.rs
index 0d0b3f92..51889da3 100644
--- a/crates/os-app/tests/cmd_compare_basic.rs
+++ b/crates/os-app/tests/cmd_compare_basic.rs
@@ -62,9 +62,9 @@ async fn open_orders_the_crawls_by_start_time() {
     );
     assert!(s.current.started_at > s.previous.started_at);
     assert_eq!(s.version, 1);
-    // Standard der Change Detection aus der Konfiguration des Current-Crawls (M2-C02 wertet aus).
+    // TECH-300: explizit leere Elementauswahl deaktiviert Change Detection; fehlendes Feld nutzt Konfigurationsdefaults.
     assert_eq!(s.content_threshold_pct, 10);
-    assert!(!s.elements.is_empty());
+    assert!(s.elements.is_empty());
     assert!(!s.elements.iter().any(|e| e == "content"));
     // Existierende (gecrawlte) URLs je Crawl.
     assert!(s.current.urls > s.previous.urls);
```

## Platzhalter-Ablösung (K2-71): normaler Overview

`crates/os-export/src/reports/overview.rs`, TECH-300/301: Im normalen Crawl-Overview wird die ausschließlich für Vergleiche verfügbare Change-Detection-Sektion nicht ausgegeben. Allgemeine Registry-Zähler behalten ihren bisherigen Vertrag. Der zusätzliche Fixture-Test `ordinary_crawl_overview_has_no_change_detection_section` in der eigenen `commands/compare.rs` prüft den Report.

```diff
diff --git a/crates/os-export/src/reports/overview.rs b/crates/os-export/src/reports/overview.rs
index f4404e6f..300d1431 100644
--- a/crates/os-export/src/reports/overview.rs
+++ b/crates/os-export/src/reports/overview.rs
@@ -445,7 +445,11 @@ fn response_codes(
 // 5 Tabs
 
 fn tab_filters(counts: &[FilterCount], rows: &mut Vec<OverviewRow>) {
-    for c in counts {
+    // TECH-300: Change Detection gehört ausschließlich zum Vergleichskontext.
+    for c in counts
+        .iter()
+        .filter(|c| c.tab != os_analyze::compare::CHANGE_DETECTION_TAB)
+    {
         // Grundgesamtheit: der Filter „All“ desselben Tabs (nur, wenn er eine Zahl hat).
         let base = counts
             .iter()
```

## Erwartungsänderungen (K2-63): Overview

`crates/os-export/tests/reports_overview.rs`, TECH-300: Die erwartete Abschnittsliste für einen normalen Crawl-Overview schließt Change Detection aus. Die Vergleiche von Reihenfolge, Filterinhalten und Zählern bleiben unverändert; keine Änderung der Testlogik.

```diff
diff --git a/crates/os-export/tests/reports_overview.rs b/crates/os-export/tests/reports_overview.rs
index 5ebac57d..7d1473fa 100644
--- a/crates/os-export/tests/reports_overview.rs
+++ b/crates/os-export/tests/reports_overview.rs
@@ -686,7 +686,7 @@ async fn placeholder_tabs_give_every_tab_with_all_filters_in_ui_order() {
     // Stub-Filter von M2 (M2-F02) tragen noch keine Zähler und erscheinen nicht.
     let all: Vec<_> = tabs::configured(&cfg)
         .into_iter()
-        .filter(|t| !t.is_stub())
+        .filter(|t| !t.is_stub() && t.id != "Change Detection")
         .collect();
     let tab_ids: Vec<&str> = all.iter().map(|t| t.id).collect();
     let secs = sections(&t);
```

## Fremdergänzungen (K2-61): Arbeitsmappen-Status

`crates/os-export/src/xlsx.rs`: strikt additive Methode `has_tables()`. Der C02-Aufgabenexport prüft damit den tatsächlich beschriebenen Writer statt der Anzahl übersprungener Exportauswahlen. Gemischte leere/gefüllte Sammelexporte bleiben erhalten; der Fixture-Test `mixed_empty_and_populated_exports_keep_the_shared_workbook` liegt in der eigenen `commands/compare.rs`.

```diff
diff --git a/crates/os-export/src/xlsx.rs b/crates/os-export/src/xlsx.rs
index 5096582f..75e55a9f 100644
--- a/crates/os-export/src/xlsx.rs
+++ b/crates/os-export/src/xlsx.rs
@@ -285,6 +285,11 @@ impl XlsxWriter {
         self.cancel = Some(cancel);
     }
 
+    /// Ob bereits mindestens eine Tabellenkopfzeile geschrieben wurde (auch bei übersprungenen leeren Tabellen).
+    pub fn has_tables(&self) -> bool {
+        self.header.is_some() || self.tables > 1
+    }
+
     /// Namen der bisher angelegten Blätter.
     pub fn sheet_names(&self) -> &[String] {
         &self.sheet_names
```

## Abschlussprüfung

Alle geforderten lokalen Gates sind auf dem finalen Quellstand grün:

- `cargo fmt --all --check`: Exit 0.
- `cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings`: Exit 0.
- `cargo test --workspace --exclude openspider-desktop --locked`: Exit 0; 3575 bestanden, 0 fehlgeschlagen, 67 bereits bestehende Ignores, 472 Testziele einschließlich Doc-Tests. Summe der vom Test-Runner ausgewiesenen Ziel-Laufzeiten: 990.10 s. Keine neuen Ignores.
- `npm ci`, `npm run -w apps/desktop typecheck`, `npm run -w apps/desktop test`: grün; 700 Tests in 72 Dateien, 56,73 s für Vitest.
- `cargo deny check licenses bans sources`: grün.
- `npm run -w apps/desktop check:licenses`: 474 Pakete unter der Positivliste.
- `npm run -w apps/desktop check:static`: grün, 28 Tests.
- Lokale CLI-/Scheduling-Smokes: grün (drei CLI-Crawls, zwei Aufgabenläufe mit Vergleichs-Arbeitsmappe, Skip-Empty und Projektvalidierung).

Plattform: macOS arm64. `OPENSPIDER_SECRET_STORE=memory` für die Rust-Prüfungen; kein echter Schlüsselbundzugriff. Keine plattformübergreifende CI gestartet, kein Push/Merge/PR. Keine offenen Fachfragen; die oben abgegrenzte SMTP- und OUT-150-Verwaltungsintegration bleibt bei deren Paketen.
