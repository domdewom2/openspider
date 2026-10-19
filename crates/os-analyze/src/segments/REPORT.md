# M2-K14 – Paketbericht

## Umgesetzte Spec-IDs

- **CRAWL-134:** validierte Segmentregeln mit UND/ODER-Gruppen und allen nativen Operatoren, case-sensitive Textvergleich, numerische NULL-Semantik, gemeinsame begrenzte Backtracking-Regex-Engine, Mehrfach-/verschränkte Spalten, Issue-Regeln, Verweise nur auf vorangehende Segmente, exklusive Priorität, Restmenge, JSON-Roundtrip und Speicherung in der Crawl-Konfiguration. Vorschau ohne Schreibzugriff; vollständige und inkrementelle Neuzuordnung mit Savepoint, Abbruch und Änderungsfeed. Nachlauf-/API-Werte bleiben bis zum Abschluss ausstehend. Automatische Aktualisierung in App und CLI, ohne neue HTTP-Requests.
- **OUT-011:** gemeinsame gespeicherte Zuordnung für Tab-, Bulk- und Report-Exporte; Inlinks nach Ziel, All Outlinks nach Quelle; Segmentsuffix, `--segment <Name>` und `--segment all` mit zusätzlicher ungefilterter Ausgabe. Getrennte Veröffentlichung verhindert Überschreiben der ungefilterten Datei. URL-Reports erhalten den vollständigen Graphkontext, Summenreports verwenden die eingeschränkte Population.
- **UI-020 [P1]:** persistierte Segmentspalte und Bitmap-Quelle für K01-View-Sitzungen; Neuzuordnung veröffentlicht `change_seq` und `reset_seq` für einen neuen View-Stand.
- **UI-067 [P1]:** Kennzahlen mit internem Nenner, eindeutigen URL-Mengen je Issue-Typ/Priorität, Tiefe 0 bis 10+, Antwortklassen, Indexierbarkeit und abschließender Restzeile. `sidebar.segments` verwendet dieselben Kernzahlen.
- **UI-068 [P1]:** `segment.list/save/delete/validate/preview/reassign` nach dem eingefrorenen Command-Vertrag, stabile Command-IDs und Regelbaum-Roundtrip; Fehlerpfade und Regex-Positionen, ausstehende Bedingungen als Validierungswarnung, abbrechbare Vorschau/Neuzuordnung über Operations.

## Schnittstellen und Festlegungen

- `url_segments.segment_id` ist der nullbasierte Index der gespeicherten Liste; Namen/Farben kommen aus `segments.definitions` im Crawl. Liste, Zuordnung und Konfiguration werden atomar gespeichert. Command-IDs und der ursprüngliche Regelbaum stehen zusätzlich in `segments.dto`.
- Segmentverweise lesen die Regel-Treffermengen vor der exklusiven Vorrangentscheidung. Die endgültige Zuordnung bleibt eindeutig: das erste passende Segment gewinnt. „In keinem Segment“ verwendet die Vereinigung aller vorherigen Treffermengen.
- Der vorhandene DTO-Regelbaum stellt Segmentbezüge als `column: "Segment"` mit `equals` bzw. `isEmpty` dar, Issue-Bezüge als `column: "Issues"` mit `equals`/`notEquals`. Native `>=`/`<=` werden beim DTO-Lesen als ODER aus strengem Vergleich und Gleichheit dargestellt. Die nativen Konfigurationsoperatoren bleiben vollständig verfügbar.
- Der App-Dienst verwendet ausschließlich Lese-Pool und Schreibwarteschlange; der headless Beobachter hält eine eigene Store-Verbindung auf seinem Worker. Leere Segmentkonfigurationen starten keinen CLI-Beobachter.

## Tests

- `cargo fmt --all --check`: grün.
- `cargo clippy --locked --workspace --exclude openspider-desktop --all-targets -- -D warnings`: grün.
- `cargo test --locked --workspace --exclude openspider-desktop`: grün (Exit-Code 0; 2824 erfolgreiche Tests, 67 vorhandene Ignores; einschließlich Doc-Tests).
- 21 paketbezogene Tests (eine private Regel-Fixture, zwölf Kern-Integrationstests, drei Command- und fünf Exporttests) einschließlich aktiver 100.000-URL-Budgetprüfung.
- CLI-Smoke mit lokaler kopierter Schema-v2-Fixture: `--segment all` → 40 ungefilterte CSV-Zeilen plus genau eine Segmentzeile; `--segment First --export-format xlsx` → eine Datei mit einer Datenzeile; unbekannter Segmentname → Exit-Code 2. Kein HTTP-Crawl für diese Exporte.
- Keine Frontend-Datei geändert; die zusätzliche npm-Prüfung ist damit nicht ausgelöst.
- Bestehende Paket-/Benchmark-Ignores wurden nicht verändert; keine neuen Ignores in K14.
- 1-Mio.-URL-Referenzhardware-Benchmark nicht ausgeführt; der aktive Pakettest misst 100.000 URLs gegen das verbindliche Fünf-Sekunden-Budget.

Der gesamte abschließende Lauf ist lokal auf macOS ausgeführt worden; kein CI-Lauf, da kein Push beauftragt ist.

Die Pakettests liegen in `segments_rules.rs`, `cmd_segment_commands.rs`, `segment_exports.rs` und im privaten Regelmodul. Sie verwenden lokale SQLite-/SQL-Fixtures und tragen Spec-Tags. Der aktive 100.000-URL-Test prüft die Neuzuordnung unter fünf Sekunden. Frontend-Dateien wurden nicht geändert.

## Abweichungen und Integrationsgrenzen

Keine neue Crate, keine Manifest-/Lockfile-Änderung, keine Referenzprodukt-Inspektion. Die UI-Darstellung gehört den UI-Paketen; K14 liefert Daten und Filterquelle. Auf diesem Branch sind `view.*`, die alten `export.*`-Commands und Profil-Commands weiterhin fremde K06/K07-Stubs. K14 ersetzt diese fremden Handler nicht; die Kern-APIs und `segment.*` sind ausführbar. Globale Auswahl/Badge/Farbkontrast sind Frontend-Verantwortung, kein Frontend-Code in diesem Paket.

## Stub-Ablösung (K2-60)

`crates/os-app/tests/manifest_contract.rs`: ausschließlich `M2-K14` aus der Liste der noch erwarteten Stub-Besitzer entfernt; Command-Namen, DTO-Verträge und Long-Running-Markierungen unverändert.

## Fremdergänzungen (K2-61)

Alle folgenden Änderungen sind reine Ergänzungen. Es wurde keine bestehende Signatur geändert und kein bestehender Exportdispatcher entfernt. Neue CLI-Auswahlfelder sind optional (Default `None`); der Segmentpfad ist über das neue Flag erreichbar. Die Initialisierung des Beobachters und einer noch fehlenden Zuordnung ist auf konfigurierte Segmente beschränkt.

### `crates/os-analyze/src/tabs/mod.rs`

```diff
diff --git a/crates/os-analyze/src/tabs/mod.rs b/crates/os-analyze/src/tabs/mod.rs
index a5e7d39..7ff43b8 100644
--- a/crates/os-analyze/src/tabs/mod.rs
+++ b/crates/os-analyze/src/tabs/mod.rs
@@ -1142,6 +1142,10 @@ fn configure_sources(
             apply_meta_defaults(&mut tab.columns);
         }
     }
+    for tab in &mut tabs {
+        tab.columns.extend(crate::segments::columns(cfg));
+        apply_meta_defaults(&mut tab.columns);
+    }
     if cfg.ui.parity_filters {
         for tab in &mut tabs {
             if tab.id == "Internal" {
```

### `crates/os-app/src/runtime/services.rs`

```diff
diff --git a/crates/os-app/src/runtime/services.rs b/crates/os-app/src/runtime/services.rs
index a3000fb..c1ff44a 100644
--- a/crates/os-app/src/runtime/services.rs
+++ b/crates/os-app/src/runtime/services.rs
@@ -2,6 +2,7 @@
 use super::{lifecycle_bus::CrawlEvent, App};
 pub fn start(app: &App) {
     crate::session::init(app);
+    crate::commands::segment::on_startup(app);
     crate::commands::sidebar_ext::on_startup(app);
     crate::logging::init_for_app(app);
     crate::commands::api::on_startup(app);
```

### `crates/os-cli/src/args.rs`

```diff
diff --git a/crates/os-cli/src/args.rs b/crates/os-cli/src/args.rs
index 2974cd3..f65830b 100644
--- a/crates/os-cli/src/args.rs
+++ b/crates/os-cli/src/args.rs
@@ -152,6 +152,10 @@ pub struct RunArgs {
     pub require_apis: bool,
 
     // ---- Exporte ------------------------------------------------------------------------------
+    /// Beschränkt Exporte auf ein gespeichertes Segment; `all` exportiert zusätzlich jedes Segment einzeln.
+    #[arg(long, value_name = "NAME|all", help_heading = "Exporte")]
+    pub segment: Option<String>,
+
     /// Tab-Exporte `"Tab:Filter,…"` (z. B. `"Internal:All,Response Codes:Client Error (4xx)"`); Kommas im
     /// Namen mit `\,` maskieren.
     #[arg(long, value_name = "TAB:FILTER,…", help_heading = "Exporte")]
```

### `crates/os-cli/src/plan.rs`

```diff
diff --git a/crates/os-cli/src/plan.rs b/crates/os-cli/src/plan.rs
index 9a6c8ee..2c6973e 100644
--- a/crates/os-cli/src/plan.rs
+++ b/crates/os-cli/src/plan.rs
@@ -44,6 +44,8 @@ pub enum Source {
 /// Ablaufplan nach der Validierung.
 #[derive(Debug)]
 pub struct Plan {
+    /// Optionaler Segmentkontext; ohne Flag bleibt der Export ungefiltert.
+    pub segment: Option<String>,
     /// Quelle.
     pub source: Source,
     /// Wirksame Konfiguration (bei `Load` ohne `--resume` die gespeicherte).
@@ -442,7 +444,13 @@ pub fn build(args: &RunArgs, paths: AppPaths, rep: &Reporter) -> Result<Plan, Cl
         .or(project_default_name)
         .unwrap_or_default();
 
+    if let Some(segment) = args.segment.as_deref() {
+        if segment != "all" && !config.segments.list.iter().any(|s| s.name == segment) {
+            return Err(CliError::usage(format!("Unbekanntes Segment: {segment}")));
+        }
+    }
     Ok(Plan {
+        segment: args.segment.clone(),
         source,
         config,
         resume_config,
```

### `crates/os-cli/src/exec.rs`

```diff
diff --git a/crates/os-cli/src/exec.rs b/crates/os-cli/src/exec.rs
index 12de143..c51a6c0 100644
--- a/crates/os-cli/src/exec.rs
+++ b/crates/os-cli/src/exec.rs
@@ -343,6 +343,14 @@ async fn crawl_phase(
         }
     };
 
+    let _segment_observer = if plan.config.segments.list.is_empty() {
+        None
+    } else {
+        Some(
+            os_analyze::segments::observe(path.clone(), signals.cancel_token())
+                .map_err(|error| CliError::internal(format!("Segmentbeobachter: {error}")))?,
+        )
+    };
     let json_errors = rep.json();
     let summary = monitor(handle, signals, rep, &path, json_errors)
         .await
@@ -537,10 +545,34 @@ fn export_phase(plan: &Plan, rep: &Reporter, signals: &Signals, crawled: &Crawle
         rep.phase("export");
     }
 
+    // Initialize configured segments once; subsequent exports only consume the saved assignment.
+    if !cfg.segments.list.is_empty() && matches!(file.get_meta("segments.definitions"), Ok(None)) {
+        if let Err(error) = os_analyze::segments::reassign(&file, &cfg, &signals.cancel_token()) {
+            rep.error(format!("Segmentzuordnung fehlgeschlagen: {error}"));
+            return ExportEnd::Done {
+                failures: 1,
+                files: 0,
+            };
+        }
+    }
+
     for item in &plan.exports {
         if signals.is_interrupted() {
             return ExportEnd::Interrupted;
         }
+        if let Some(segment) = plan.segment.as_deref() {
+            match run_segment_export(&file, &cfg, item, out, plan.format, segment, rep) {
+                Ok(n) => files += n,
+                Err(message) => {
+                    failures += 1;
+                    rep.error(format!(
+                        "Export '{}' fehlgeschlagen: {message}",
+                        describe(item)
+                    ));
+                }
+            }
+            continue;
+        }
         match run_export(&file, &cfg, item, out, plan.format, rep) {
             Ok(n) => files += n,
             Err(message) => {
@@ -627,3 +659,29 @@ fn discard(path: &Path, rep: &Reporter) {
         }
     }
 }
+
+/// Segment-aware companion to the unfiltered export dispatcher (OUT-011).
+fn run_segment_export(
+    file: &CrawlFile,
+    cfg: &CrawlConfig,
+    item: &Resolved,
+    out: &Path,
+    format: Format,
+    segment: &str,
+    rep: &Reporter,
+) -> Result<usize, String> {
+    let (kind, name) = match item {
+        Resolved::Tab { tab, filter } => ("tab", format!("{tab}:{filter}")),
+        Resolved::Bulk(name) => ("bulk", name.clone()),
+        Resolved::Issues => ("bulk", os_export::catalog::bulk_issues::BULK_NAME.into()),
+        Resolved::Report(name) => ("report", name.clone()),
+    };
+    let selection = os_export::types::ExportSelection {
+        kind: kind.into(),
+        name,
+    };
+    let files = os_export::segment::export_all(file, cfg, &selection, Some(segment), out, format)
+        .map_err(|e| e.to_string())?;
+    files.iter().for_each(|info| report_file(rep, info));
+    Ok(files.len())
+}
```

## Erwartungsänderungen (K2-63)

Bislang keine fremden Erwartungen oder Goldens geändert. Die Entfernung des eigenen Stub-Besitzers ist K2-60.

## Offene Fragen

Keine fachliche Rückfrage. Die oben genannten UI-/K06-/K07-Anbindungen bleiben bei ihren Paketbesitzern.

## Commits

- `768934c` – `analyze: implement persisted segments and live reassignment`
- `cfb83fe` – `export: scope tables bulk exports and reports to saved segments`
- `210c7d0` – `app: implement segment commands previews and sidebar metrics`
- `9a6750e` – `segments: keep stored rules and canonical export names consistent`

Alle Implementierungs-Commits mit `Co-Authored-By: OpenAI Codex <noreply@openai.com>`. Kein Push, Merge oder PR. Der abschließende Dokumentationscommit enthält diesen Bericht.
