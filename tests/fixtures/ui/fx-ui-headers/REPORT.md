# M2-D18 – Umsetzungsbericht

## Umfang

Umgesetzt: OUT-045, OUT-046, OUT-048, OUT-049, OUT-110 und OUT-111.
Mitgeprüft: CRAWL-062, CRAWL-063, OUT-112 [M2] und ONPAGE-165.
Der Startzustand des Worktrees war sauber. Branch: `m2/d18`; Plattform: macOS arm64.
Clean-Room: ausschließlich Projektspecs und eigene Fixtures verwendet.

- `Web:All Page Source` schreibt `page_source/*.html` und `index.csv`, bytegetreu auch bei anderen Zeichensätzen.
- `Web:All Rendered Page Source` (Alias `Web:All Page Source:Rendered`) wählt gespeichertes DOM; ohne DOM oder passende Speicherschalter ist die Auswahl deaktiviert.
- `Web:All Page Text` schreibt `page_text/*.txt` und `index.csv`; `Content:All Page Text` liefert dieselben Inhalte als CSV/XLSX-Tabelle. Nur interne HTML-Seiten mit Status 200; Inhaltsbereich aus der Exportkonfiguration.
- Header-Exporte bewahren gespeicherte Schreibweise und Reihenfolge einschließlich mehrfacher Header. Request-Zugangsdaten sind standardmäßig geschwärzt; `export.show_credentials = true` gibt sie ausdrücklich frei.
- Cookie-Bulk und Summary verwenden die Spalten/Werte aus UI-043, RFC-Standardpfade, UTC-Ablaufzeiten und die bestehende Public-Suffix-Liste. JavaScript-Cookies erscheinen nur bei Rendering.
- Ordnerexporte funktionieren auch im Multi-Export neben einem gemeinsamen Workbook, mit Abbruchprüfung und Überspringen leerer Ergebnisse.

## Fachliche Festlegungen und Abweichungen

Keine zurückgestellten Paketkriterien. Die zusätzlichen Katalognamen machen die Original/Rendered-Auswahl sowie den Content-Tab-Export über dieselbe vorhandene CLI-/Service-Schnittstelle erreichbar.
Die Spec legt bei Gleichstand des häufigsten Header-Werts keine Reihenfolge fest: binäre Wertsortierung dient als deterministischer Tie-Breaker; Häufigkeit zählt Header-Vorkommen, `URLs` stets verschiedene URLs.
Die Cookie-Summary übernimmt `Persistence` und `Sample URL` von der alphabetisch ersten URL eines Cookie-Schlüssels; bei gemischter First-/Third-Party-Verwendung ist `First-Party=False`. Die Spec beschreibt diese Mischfälle nicht. Die Summary hält eine Aggregatzeile pro unterschiedlichem Cookie-Schlüssel, keine Seitenkörper.

## Tests

- `cargo fmt --all --check`: grün.
- `cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings`: grün.
- `cargo test -p os-export --test bulk_web_m2 --test report_headers_m2 --test report_cookies_m2 --locked`: 13 bestanden, 0 ignoriert; 1,15 s Testlaufzeit (ohne Build).
- `cargo test --workspace --exclude openspider-desktop --locked --no-fail-fast`: grün; 2816 bestanden, 0 fehlgeschlagen, 67 bestehende Ignores über 361 Testziele einschließlich Doc-Tests. Gesamtdauer einschließlich Build: ca. 651 s.
- `cargo deny check licenses bans sources`: grün (keine neuen Bibliotheksversionen).
- `scripts/check-neutral.sh`: grün; bestehende Warnungen zu verwaisten CI-Ausnahmen bleiben unverändert.
- `python3 docs/plan/M2-check.py`: grün.
- Schema-/Default-Regeneration: gezielte `os-model`-Tests mit `UPDATE_GOLDEN=1`, jeweils grün.
- CLI-Listen-Golden um die zwei eigenen Einträge ergänzt; gezielter CLI-Test grün.
- Erwartung des Asset-Manifest-Pfads im vorhandenen Multi-Export-Test nach OUT-045 angepasst; gezielter Test grün.
- `git diff --check`: grün.

Implementierungscommits: `3717b23` (additive Erweiterungspunkte), `0144fc1` (Exportlogik, Fixtures und Tests).

Keine Frontend-Dateien geändert; die bedingten npm-Prüfungen sind nicht erforderlich.
Kein Push, Merge oder PR; daher keine CI-Lauf-ID. Plattformübergreifende CI ist hier nicht ausgelöst worden.

## Fremdergänzungen (K2-61)

Alle folgenden Änderungen sind additiv. Bestehende Signaturen und fremde Testlogik bleiben erhalten.
`os-parse` und `os-url` sind vorhandene Workspace-Abhängigkeiten; es kommen keine neuen Bibliotheksversionen hinzu. Die Ergänzungen erlauben die Wiederverwendung der bestehenden Text- und Domainregeln.
Die zwei generierten Konfigurationsdateien ändern sich ausschließlich wegen des neuen, standardmäßig deaktivierten `export.show_credentials`-Felds (OUT-048).

### `Cargo.lock`

```diff
diff --git a/Cargo.lock b/Cargo.lock
index f7ef6ce..adab888 100644
--- a/Cargo.lock
+++ b/Cargo.lock
@@ -5111,8 +5111,10 @@ dependencies = [
  "os-analyze",
  "os-engine",
  "os-model",
+ "os-parse",
  "os-store",
  "os-testsite",
+ "os-url",
  "quick-xml 0.42.0",
  "rust_xlsxwriter",
  "serde",
```

### `crates/os-export/Cargo.toml`

```diff
diff --git a/crates/os-export/Cargo.toml b/crates/os-export/Cargo.toml
index d4b5695..83e455b 100644
--- a/crates/os-export/Cargo.toml
+++ b/crates/os-export/Cargo.toml
@@ -15,6 +15,8 @@ toml.workspace = true
 os-model.workspace = true
 os-store.workspace = true
 os-analyze.workspace = true
+os-parse.workspace = true
+os-url.workspace = true
 thiserror.workspace = true
 tracing.workspace = true
 serde.workspace = true
```

### `crates/os-export/src/pipeline.rs`

```diff
diff --git a/crates/os-export/src/pipeline.rs b/crates/os-export/src/pipeline.rs
index 2537f4d..779311b 100644
--- a/crates/os-export/src/pipeline.rs
+++ b/crates/os-export/src/pipeline.rs
@@ -421,6 +421,9 @@ fn export_bulk_in_snapshot(
         ExportRef::Bulk(canonical) => canonical,
         other => unreachable!("Bulk-Auflösung lieferte {other:?}"),
     };
+    if let Some(kind) = catalog::bulk_web::page_export(&canonical) {
+        return catalog::bulk_web::export_pages(file, cfg, kind, out_dir, None);
+    }
     if !cfg.export.inlinks_for_all_filters && catalog::bulk_generic::is_generic_name(&canonical) {
         return Err(ExportError::Unsupported(format!(
             "{canonical}: enable export.inlinks_for_all_filters to export generic inlinks"
@@ -694,6 +697,28 @@ fn export_batch_in(
                 ));
                 continue;
             }
+            if kind == ExportKind::Bulk {
+                if let Some(page_kind) = catalog::bulk_web::page_export(&name) {
+                    let files = catalog::bulk_web::export_pages(
+                        file,
+                        cfg,
+                        page_kind,
+                        options.directory,
+                        Some(ctx),
+                    )?;
+                    let rows = files.last().map_or(0, |info| info.rows);
+                    if rows == 0 && ctx.skip_empty {
+                        result.skipped.push(name);
+                    }
+                    done.set(done.get() + rows);
+                    (ctx.progress)(done.get());
+                    for info in files {
+                        result.warnings.extend(info.warnings.clone());
+                        result.files.push(info);
+                    }
+                    continue;
+                }
+            }
             let path =
                 options
                     .directory
```

### `crates/os-export/src/reports/prereq.rs`

```diff
diff --git a/crates/os-export/src/reports/prereq.rs b/crates/os-export/src/reports/prereq.rs
index 24095c7..4e36996 100644
--- a/crates/os-export/src/reports/prereq.rs
+++ b/crates/os-export/src/reports/prereq.rs
@@ -104,6 +104,7 @@ pub fn availability(
         Vec::new()
     };
     let mut warnings = requirement_warnings(name, requires, cfg, &pending);
+    warnings.extend(crate::catalog::bulk_web::availability_warnings(name, file));
     for req in requires {
         let missing = match req {
             ReportRequirement::SitemapProcessed => !file
```

### `crates/os-model/src/config.rs`

```diff
diff --git a/crates/os-model/src/config.rs b/crates/os-model/src/config.rs
index d6549db..97d2116 100644
--- a/crates/os-model/src/config.rs
+++ b/crates/os-model/src/config.rs
@@ -1109,6 +1109,8 @@ impl Default for ValidationConfig {
 #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
 #[serde(default)]
 pub struct ExportConfig {
+    /// Zugangsdaten in Request-Header-Exporten anzeigen (OUT-048, Standard aus).
+    pub show_credentials: bool,
     /// UTF-8-BOM in CSV-Dateien.
     pub utf8_bom: bool,
     /// Zeilenende in CSV-Dateien.
@@ -1120,6 +1122,7 @@ pub struct ExportConfig {
 impl Default for ExportConfig {
     fn default() -> Self {
         ExportConfig {
+            show_credentials: false,
             inlinks_for_all_filters: true,

             utf8_bom: true,
```

### `crates/os-store/src/queries/mod.rs`

```diff
diff --git a/crates/os-store/src/queries/mod.rs b/crates/os-store/src/queries/mod.rs
index 2130bdc..91b0867 100644
--- a/crates/os-store/src/queries/mod.rs
+++ b/crates/os-store/src/queries/mod.rs
@@ -598,6 +598,22 @@ pub fn stored_html(file: &crate::CrawlFile, url: UrlId) -> Result<Option<StoredH
     row.map(|(charset, bytes, compression)| decode_html(charset, bytes, compression))
         .transpose()
 }
+/// Gespeichertes gerendertes DOM, dekomprimiert als UTF-8 (OUT-045).
+pub fn stored_rendered_html(
+    file: &crate::CrawlFile,
+    url: UrlId,
+) -> Result<Option<StoredHtml>, StoreError> {
+    let row = file
+        .conn()
+        .query_row(
+            "SELECT html, compression FROM render_pages WHERE url_id = ?1 AND html IS NOT NULL",
+            [url.0],
+            |r| Ok((r.get(0)?, r.get(1)?)),
+        )
+        .optional()?;
+    row.map(|(bytes, compression)| decode_html(Some("UTF-8".into()), bytes, compression))
+        .transpose()
+}
 pub fn for_each_stored_html(
     file: &crate::CrawlFile,
     mut f: impl FnMut(UrlId, StoredHtml) -> Result<(), StoreError>,
```

### `docs/spec/schemas/osconfig.schema.json`

```diff
diff --git a/docs/spec/schemas/osconfig.schema.json b/docs/spec/schemas/osconfig.schema.json
index ee4e82f..9782e66 100644
--- a/docs/spec/schemas/osconfig.schema.json
+++ b/docs/spec/schemas/osconfig.schema.json
@@ -858,6 +858,11 @@
           "default": "crlf",
           "description": "Zeilenende in CSV-Dateien."
         },
+        "show_credentials": {
+          "default": false,
+          "description": "Zugangsdaten in Request-Header-Exporten anzeigen (OUT-048, Standard aus).",
+          "type": "boolean"
+        },
         "utf8_bom": {
           "default": true,
           "description": "UTF-8-BOM in CSV-Dateien.",
@@ -3518,6 +3523,7 @@
       "default": {
         "inlinks_for_all_filters": true,
         "line_ending": "crlf",
+        "show_credentials": false,
         "utf8_bom": true
       },
       "description": "Export."
```

### `tests/fixtures/config/default.osconfig`

```diff
diff --git a/tests/fixtures/config/default.osconfig b/tests/fixtures/config/default.osconfig
index 70e1987..bfce990 100644
--- a/tests/fixtures/config/default.osconfig
+++ b/tests/fixtures/config/default.osconfig
@@ -333,6 +333,7 @@ html_doc_max_bytes = 2097152
 resource_max_bytes = 2097152

 [export]
+show_credentials = false
 utf8_bom = true
 line_ending = "crlf"
 inlinks_for_all_filters = true
```

### `tests/golden/out-cli/list_exports.txt`

OUT-045 und ONPAGE-165 ergänzen ausschließlich die zwei neuen eigenen Katalognamen in der CLI-Liste. Die bestehende Testlogik bleibt unverändert. Diese Golden-Ergänzung ist nach K2-61 als eigene additive Ausgabeänderung erfasst.

```diff
diff --git a/tests/golden/out-cli/list_exports.txt b/tests/golden/out-cli/list_exports.txt
index 02a39cf..4b7aadd 100644
--- a/tests/golden/out-cli/list_exports.txt
+++ b/tests/golden/out-cli/list_exports.txt
@@ -457,6 +457,8 @@ Accessibility:Best Practice
 Accessibility:Incomplete
 Web:All Page Source
 Web:All Page Text
+Web:All Rendered Page Source
+Content:All Page Text
 Web:All HTTP Request Headers
 Web:All HTTP Response Headers
 Web:All Cookies
```

## Stub-Ablösung (K2-60)

Aus `crates/os-engine/f05a-stubs.toml` ausschließlich die drei eigenen D18-Einträge für `bulk_web::entries`, `reports::cookies::entries` und `reports::http_headers::entries` entfernt. Die übrigen Stub-Verträge bleiben erhalten.

## Erwartungsänderungen (K2-63)

Die CLI-Erwartung `tests/golden/out-cli/list_exports.txt` enthält zusätzlich `Web:All Rendered Page Source` (OUT-045) und `Content:All Page Text` (ONPAGE-165). Keine Testlogik geändert. Diese ausschließlich additive Ausgabeänderung ist einschließlich Diff auch unter K2-61 erfasst; ebenso Schema und Default-Konfigurationsfixture.

### `crates/os-export/tests/pipeline_m2.rs`

OUT-045 schreibt einen Ordner mit `index.csv` unabhängig vom gewählten Tabellenformat. Daher erwartet der vorhandene Multi-Export-Test jetzt `page_source/index.csv` anstelle des früheren Stub-Manifests `web_all_page_source.xlsx`. Nur der erwartete Pfad wurde geändert, keine Testlogik.

```diff
diff --git a/crates/os-export/tests/pipeline_m2.rs b/crates/os-export/tests/pipeline_m2.rs
index b50ca66..2446b83 100644
--- a/crates/os-export/tests/pipeline_m2.rs
+++ b/crates/os-export/tests/pipeline_m2.rs
@@ -624,7 +624,7 @@ fn reports_share_the_workbook_and_asset_manifests_stay_separate() {
             .height(),
         2
     );
-    assert!(dir.path().join("web_all_page_source.xlsx").is_file());
+    assert!(dir.path().join("page_source/index.csv").is_file());
 }

 // spec: OUT-075
```

## Offene Fragen

Keine blockierenden Fragen. Die oben benannten, in der Spec nicht beschriebenen Aggregationsmischfälle können bei einer späteren Spec-Präzisierung angepasst werden.
