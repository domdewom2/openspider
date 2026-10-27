# M2-R02 – Implementierungsbericht

Basis: `4ae23c8d`, Branch `m2/r02`. Clean-Room: eigene synthetische
Fixtures, keine Einsicht in das Referenzprodukt. Keine neuen Abhängigkeiten.

## Umsetzung und Abnahme

- **Eigene Spec-IDs:** TECH-101, TECH-102, TECH-103, TECH-104, TECH-105,
  TECH-106, TECH-107, TECH-108, TECH-109, TECH-110, TECH-111, TECH-114,
  ONPAGE-053, ONPAGE-057, OUT-043.
- **Zusätzliche Abnahme:** TECH-015 [M2], ONPAGE-090 [M2], ONPAGE-170 [M2],
  ONPAGE-172, ONPAGE-178, ONPAGE-180, ONPAGE-181.
- JavaScript: Population, alle 25 Spalten, Wortänderungen und Prozentwerte,
  eindeutige JS-Hyperlink-Ziele, alle P1-Filter, 14 Issue-Einträge mit eigenen
  deutschen und englischen Erklärungen.
- Direktiven: Original-noindex bleibt maßgeblich; Original-nofollow/none wird
  auch nach Entfernung durch JavaScript auf die ausgehenden Links angewendet.
- Canonicals: reine HTML-Hinweise werden separat persistiert; HTTP-Hinweise
  maskieren den HTML-Vergleich nicht. Vergleich nutzt TECH-002-Normalisierung.
- Ressourcen: robots-Sperren, Ladefehler und Crawl-/Tracking-Ausschlüsse
  bleiben unterscheidbar. Response Codes zeigt blockierte Ressourcen getrennt
  von normalen robots-Sperren sowie JavaScript-Navigationen.
- Bilder: echte Chromium-Abnahme von Lazy Loading, Real/Display Dimensions,
  unsichtbaren Instanzen, CSS-/Stylesheet-Hintergründen, dynamischen Bildern,
  Savings und CSV/XLSX-Bulk-Export. Icons und Social-Image-Metadaten bleiben
  außerhalb der Inhaltsbild-Instanzen.
- Exporte: gespeicherte PNGs nach `screenshots/`, bereinigte URL plus
  achtstelliger Hash, `index.csv`; CLI, Batch und App verwenden den lokalen
  Dateiexport. Der Fehlerexport bewahrt alle Konsolenmeldungen der Fehlerseiten.

## Prüfungen

- `cargo fmt --all --check`: grün.
- `cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings`: grün.
- `cargo test --workspace --exclude openspider-desktop --locked`: grün, 3646 bestanden,
  0 fehlgeschlagen, 50 bestehende Ignores; 479 Testprogramme/Doc-Test-Läufe.
  Laufzeit des vollständigen Abschlusslaufs: ca. 1233 Sekunden (Log-Zeitspanne).
- `cargo deny --offline check licenses bans sources`: grün.
- `bash scripts/check-neutral.sh` und `git diff --check`: grün.
- Die neu aktivierten ONPAGE-090/170/057-Tests und alle neuen R02-Tests sind aktiv
  und bestanden; keine neuen Ignores.


Kein Frontend geändert; daher keine npm-Prüfungen erforderlich. Kein Push,
Merge oder PR, folglich keine neue CI-Lauf-ID. Plattform: macOS arm64.

Zwei erste Workspace-Builds scheiterten ausschließlich an vollem Build-Volume
beim Linken. Der worktree-eigene Build-Cache wurde bereinigt; der Wiederholungslauf
nutzt `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=line-tables-only` und
`CARGO_PROFILE_TEST_DEBUG=line-tables-only`. Bereits erzeugte Testprogramme wurden
lokal von Debug-/Symboltabellen befreit und erneut ad-hoc signiert; die beiden
Backtrace-Testprogramme behalten ihre Symbole. Diese
Cache-Anpassung ändert keine Repository-Datei.

## Abweichungen und offene Fragen

Keine offenen fachlichen Fragen. Der PNG-Export setzt PNG-Aufnahme voraus.
Die optionale JPEG-Aufnahme ist eine OS-Erweiterung (CRAWL-082); JPEG-zu-PNG-
Konvertierung und hypothetische Mehrkachel-Aufnahmen werden vor der Ausgabe
explizit abgelehnt. Der vorhandene Renderer speichert genau eine Kachel je Seite.
TECH-112/113 und OUT-058 sind P2 und bleiben außerhalb dieses Pakets.

## Fremdergänzungen (K2-61)

Strikt additive Erweiterungen; bestehende Signaturen bleiben erhalten:

- `os-render/src/api.rs`: leere Default-Vektoren für berechnete CSS-Bildreferenzen
  und davon getrennte Icon-/Social-Image-URLs.
- `os-render/src/page.rs`: zusätzliche Erfassung aus berechneten Styles inklusive
  Pseudoelementen; keine Veränderung des gespeicherten DOM.
- `os-engine/src/render_glue.rs`: zusätzliche normalisierte HTML-Canonical-Fakten
  sowie eine neue Funktion zur Übergabe geladener CSS-/dynamischer Bilder an den
  vorhandenen Bildpfad (TECH-107/108, ONPAGE-170/178).
- `os-engine/src/worker.rs`: zusätzlicher Übergabeaufruf für diese Bilder.
- `os-export/src/pipeline.rs`: zusätzliche lokale Screenshot-Routen für Einzel-
  und Batch-Export (OUT-043).
- `os-app/src/commands/export.rs`: zusätzlicher Screenshot-Aufruf innerhalb des
  bestehenden ReadPool-Snapshots mit Staging und Abbruch (OUT-043).

`tests/golden/out-cli/list_exports.txt` ergänzt ausschließlich die beiden neuen
Exportnamen `JavaScript:Pages With JavaScript Issues` (TECH-114) und
`JavaScript:Blocked Resources` (TECH-102).

14 neue `docs/issues/javascript-*.md`-Seiten sind ausschließlich generierte
Ausgaben der eigenen neuen Issue-Einträge (UI-064, TECH-102–111/114).
Bestehende fremde Doku-/Golden-Dateien wurden nicht zum Schreiben geöffnet oder überschrieben.

### `tests/golden/out-cli/list_exports.txt`

```diff
diff --git a/tests/golden/out-cli/list_exports.txt b/tests/golden/out-cli/list_exports.txt
index 987b86cc..1ce784df 100644
--- a/tests/golden/out-cli/list_exports.txt
+++ b/tests/golden/out-cli/list_exports.txt
@@ -447,0 +448,2 @@ Sitemaps:Orphan URLs Details
+JavaScript:Pages With JavaScript Issues
+JavaScript:Blocked Resources
```
### `crates/os-render/src/api.rs`

```diff
diff --git a/crates/os-render/src/api.rs b/crates/os-render/src/api.rs
index 74fcf984..70fc7dff 100644
--- a/crates/os-render/src/api.rs
+++ b/crates/os-render/src/api.rs
@@ -298,0 +315,4 @@ pub struct RenderedPage {
+    /// Computed CSS image references, including stylesheet and pseudo-element rules (ONPAGE-178).
+    pub resource_images: Vec<os_model::ImageInstance>,
+    /// Icons and social image metadata are separate from content image instances (ONPAGE-170).
+    pub non_content_image_urls: Vec<String>,
```
### `crates/os-render/src/page.rs`

```diff
diff --git a/crates/os-render/src/page.rs b/crates/os-render/src/page.rs
index e51e4f81..efc5b189 100644
--- a/crates/os-render/src/page.rs
+++ b/crates/os-render/src/page.rs
@@ -431,0 +432,4 @@ async fn run(
+    let resource_images = resource_images(page, opts).await?;
+    let non_content_image_urls = read_document(opts, || eval(page,
+        r#"Array.from(document.querySelectorAll('link[rel~="icon"],meta[property="og:image"]'), e => {try{return new URL(e.href || e.content,document.baseURI).href;}catch{return '';}})"#
+    )).await?.as_array().into_iter().flatten().filter_map(|v|v.as_str().map(str::to_owned)).collect();
@@ -518,0 +523,2 @@ async fn run(
+        resource_images,
+        non_content_image_urls,
@@ -520,0 +527,45 @@ async fn run(
+
+/// Collect CSS references from computed styles without changing the serialized DOM.
+async fn resource_images(
+    page: &Page,
+    opts: &RenderOpts,
+) -> Result<Vec<os_model::ImageInstance>, RenderError> {
+    let values = read_document(opts, || eval(page, r#"(() => {
+      const path = e => { const parts=[]; while(e && e.nodeType===1) {
+        const name=e.localName; let n=1; for(let p=e.previousElementSibling;p;p=p.previousElementSibling) if(p.localName===name)n++;
+        parts.unshift(name+'['+n+']'); e=e.parentElement;
+      } return '/'+parts.join('/'); };
+      const rows=[]; const seen=new Set();
+      for(const e of document.querySelectorAll('*')) {
+        for(const pseudo of [null,'::before','::after']) {
+          const style=getComputedStyle(e,pseudo);
+          for(const value of [style.backgroundImage,style.content]) {
+            for(const match of value.matchAll(/url\(["']?([^"')]+)["']?\)/g)) {
+              let url; try {url=new URL(match[1],document.baseURI).href;} catch {continue;}
+              if(!/^https?:/.test(url))continue;
+              const key=url+'\n'+path(e);
+              if(seen.has(key))continue;
+              seen.add(key);rows.push({url,path:path(e)});
+            }
+          }
+        }
+      }
+      return rows;
+    })()"#)).await?;
+    Ok(values
+        .as_array()
+        .into_iter()
+        .flatten()
+        .filter_map(|v| {
+            let url = v["url"].as_str()?.to_string();
+            Some(os_model::ImageInstance {
+                src_raw: url.clone(),
+                resolved: Some(url),
+                source: os_model::ImageSource::Css,
+                xpath: v["path"].as_str().unwrap_or_default().into(),
+                origin: os_model::LinkOrigin::RenderedHtml,
+                ..Default::default()
+            })
+        })
+        .collect())
+}
```
### `crates/os-engine/src/render_glue.rs`

```diff
diff --git a/crates/os-engine/src/render_glue.rs b/crates/os-engine/src/render_glue.rs
index 9b028746..65491429 100644
--- a/crates/os-engine/src/render_glue.rs
+++ b/crates/os-engine/src/render_glue.rs
@@ -619,0 +676,12 @@ pub(crate) fn comparison(raw: &ParsedPage, dom: &ParsedPage) -> Vec<(ExtKey, Val
+        // TECH-107/108: HTTP hints must not mask the HTML-only comparison.
+        let html_canonical = page
+            .canonicals
+            .iter()
+            .filter(|c| c.source == os_model::HintSource::Html && c.resolved.is_some())
+            .min_by_key(|c| !c.in_head)
+            .and_then(|c| c.resolved.as_deref())
+            .and_then(os_url::compare::compare_key);
+        facts.push((
+            ExtKey::new("render", &format!("{prefix}_html_canonical")).expect("static key"),
+            html_canonical.map(Value::Text).unwrap_or(Value::Null),
+        ));
@@ -638,0 +707,86 @@ pub(crate) fn comparison(raw: &ParsedPage, dom: &ParsedPage) -> Vec<(ExtKey, Val
+
+/// ONPAGE-170/178: add loaded CSS and markup-free image instances to the normal image path.
+pub(crate) fn add_resource_images(
+    dom: &mut ParsedPage,
+    rendered: &RenderedPage,
+    cfg: &CrawlConfig,
+    origins: &mut HashMap<(LinkType, String), LinkOrigin>,
+) {
+    let key = |url: &str| os_url::compare::compare_key(url).unwrap_or_else(|| url.into());
+    let loaded: HashSet<_> = rendered
+        .network
+        .iter()
+        .filter(|n| n.resource_type.eq_ignore_ascii_case("image") && (200..300).contains(&n.status))
+        .map(|n| key(&n.url))
+        .collect();
+    let mut extra = rendered
+        .resource_images
+        .iter()
+        .filter(|i| {
+            i.resolved
+                .as_deref()
+                .is_some_and(|url| loaded.contains(&key(url)))
+        })
+        .cloned()
+        .collect::<Vec<_>>();
+    let non_content: HashSet<_> = rendered
+        .non_content_image_urls
+        .iter()
+        .map(|url| key(url))
+        .collect();
+    let referenced: HashSet<_> = dom
+        .images
+        .iter()
+        .chain(&extra)
+        .filter_map(|i| i.resolved.as_deref().map(key))
+        .collect();
+    for n in &rendered.network {
+        if !n.resource_type.eq_ignore_ascii_case("image")
+            || !(200..300).contains(&n.status)
+            || referenced.contains(&key(&n.url))
+            || non_content.contains(&key(&n.url))
+            || extra.iter().any(|i| {
+                i.resolved
+                    .as_deref()
+                    .is_some_and(|url| key(url) == key(&n.url))
+            })
+        {
+            continue;
+        }
+        extra.push(os_model::ImageInstance {
+            src_raw: n.url.clone(),
+            resolved: Some(n.url.clone()),
+            source: os_model::ImageSource::Css,
+            origin: LinkOrigin::DynamicallyLoaded,
+            ..Default::default()
+        });
+    }
+    for mut image in extra {
+        if dom.images.iter().any(|i| {
+            i.source == image.source && i.resolved == image.resolved && i.xpath == image.xpath
+        }) {
+            continue;
+        }
+        image.doc_order = dom.images.len().min(u32::MAX as usize) as u32;
+        let position = os_parse::link_position::classify(
+            &image.xpath,
+            os_model::LinkPosition::Content,
+            &cfg.link_positions,
+        );
+        image.link_position = Some(position.clone());
+        let link = ExtractedLink {
+            link_type: LinkType::Image,
+            href_raw: image.src_raw.clone(),
+            resolved: image.resolved.clone(),
+            xpath: image.xpath.clone(),
+            position,
+            path_type: os_model::PathType::Absolute,
+            crawlable: true,
+            doc_order: image.doc_order,
+            ..Default::default()
+        };
+        image.origin = *origins.entry(link_key(&link)).or_insert(image.origin);
+        dom.links.push(link);
+        dom.images.push(image);
+    }
+}
```
### `crates/os-engine/src/worker.rs`

```diff
diff --git a/crates/os-engine/src/worker.rs b/crates/os-engine/src/worker.rs
index c53c7c32..6d484cff 100644
--- a/crates/os-engine/src/worker.rs
+++ b/crates/os-engine/src/worker.rs
@@ -813,0 +826 @@ async fn render_page<F: Fetch>(
+            crate::render_glue::add_resource_images(&mut dom, &rendered, &sh.cfg, &mut a.origins);
```
### `crates/os-export/src/pipeline.rs`

```diff
diff --git a/crates/os-export/src/pipeline.rs b/crates/os-export/src/pipeline.rs
index 63ec68fe..a5e7165f 100644
--- a/crates/os-export/src/pipeline.rs
+++ b/crates/os-export/src/pipeline.rs
@@ -423,0 +424,3 @@ fn export_bulk_in_snapshot(
+    if canonical == catalog::bulk_screenshots::NAME {
+        return catalog::bulk_screenshots::export(file, cfg, out_dir, None);
+    }
@@ -795,0 +799,26 @@ fn export_batch_in(
+                if name == catalog::bulk_screenshots::NAME {
+                    let progress = |rows| (ctx.progress)(done.get() + rows);
+                    let screenshot_ctx = crate::types::ExportCtx {
+                        cancel: ctx.cancel,
+                        progress: &progress,
+                        skip_empty: ctx.skip_empty,
+                        workbook: None,
+                        skipped: vec![],
+                    };
+                    let files = catalog::bulk_screenshots::export(
+                        file,
+                        cfg,
+                        options.directory,
+                        Some(&screenshot_ctx),
+                    )?;
+                    for info in files {
+                        done.set(done.get() + info.rows);
+                        result.warnings.extend(info.warnings.clone());
+                        if info.rows == 0 && ctx.skip_empty {
+                            result.skipped.push(name.clone());
+                        } else {
+                            result.files.push(info);
+                        }
+                    }
+                    continue;
+                }
```
### `crates/os-app/src/commands/export.rs`

```diff
diff --git a/crates/os-app/src/commands/export.rs b/crates/os-app/src/commands/export.rs
index 5d62c308..4fadf6d3 100644
--- a/crates/os-app/src/commands/export.rs
+++ b/crates/os-app/src/commands/export.rs
@@ -677,0 +678,51 @@ fn named<'a>(
+                                // OUT-043 uses local files through the same read snapshot and staging area.
+                                if kind == ExportKind::Bulk && name == core::bulk_screenshots::NAME
+                                {
+                                    let report = |rows| progress.report(rows, None);
+                                    let context = ExportCtx {
+                                        cancel: &work_token,
+                                        progress: &report,
+                                        skip_empty: false,
+                                        workbook: None,
+                                        skipped: vec![],
+                                    };
+                                    core::bulk_screenshots::export(
+                                        file,
+                                        &s.config,
+                                        &stage.0,
+                                        Some(&context),
+                                    )
+                                    .map_err(export_error)?;
+                                    checked(&work_token)?;
+                                    let source = stage.0.join("screenshots");
+                                    let target = dir.join("screenshots");
+                                    std::fs::create_dir_all(&target)
+                                        .map_err(|e| io_error(&target, e))?;
+                                    let paths = std::fs::read_dir(&source)
+                                        .map_err(|e| io_error(&source, e))?;
+                                    let index = source.join("index.csv");
+                                    let mut bytes = 0;
+                                    for entry in paths {
+                                        checked(&work_token)?;
+                                        let path = entry.map_err(|e| io_error(&source, e))?.path();
+                                        if path == index {
+                                            continue;
+                                        }
+                                        let destination =
+                                            target.join(path.file_name().ok_or_else(|| {
+                                                CmdError::new(
+                                                    ErrorCode::Internal,
+                                                    "err-export-path",
+                                                )
+                                            })?);
+                                        bytes +=
+                                            stage.publish(&path, &destination, &work_token)?.bytes;
+                                    }
+                                    bytes += stage
+                                        .publish(&index, &target.join("index.csv"), &work_token)?
+                                        .bytes;
+                                    return to_json(&FileResult {
+                                        path: target.display().to_string(),
+                                        bytes,
+                                    });
+                                }
```
### `docs/issues/javascript-contains-javascript-links.md`

```diff
diff --git a/docs/issues/javascript-contains-javascript-links.md b/docs/issues/javascript-contains-javascript-links.md
new file mode 100644
index 00000000..1bed111b
--- /dev/null
+++ b/docs/issues/javascript-contains-javascript-links.md
@@ -0,0 +1,19 @@
+# JavaScript: Contains JavaScript Links
+
+Typ: Warning (T-W1) · Priorität: Medium (P-M2)
+
+## Deutsch
+
+**Was bedeutet das?** Einige Hyperlinks erscheinen erst im gerenderten Dokument.
+
+**Warum ist es relevant?** Ihre Ziele lassen sich erst nach der JavaScript-Ausführung entdecken.
+
+**Wie beheben?** Stelle wichtige Navigationslinks bereits im ausgelieferten HTML bereit.
+
+## English
+
+**What does it mean?** Some hyperlinks appear only in the rendered document.
+
+**Why does it matter?** Discovering their destinations requires JavaScript execution.
+
+**How to fix** Provide important navigation as ordinary links in the delivered HTML.
```
### `docs/issues/javascript-h1-only-in-rendered-html.md`

```diff
diff --git a/docs/issues/javascript-h1-only-in-rendered-html.md b/docs/issues/javascript-h1-only-in-rendered-html.md
new file mode 100644
index 00000000..7e73c829
--- /dev/null
+++ b/docs/issues/javascript-h1-only-in-rendered-html.md
@@ -0,0 +1,19 @@
+# JavaScript: H1 Only In Rendered HTML
+
+Typ: Warning (T-W1) · Priorität: Medium (P-M2)
+
+## Deutsch
+
+**Was bedeutet das?** H1 wird durch JavaScript ergänzt oder geändert.
+
+**Warum ist es relevant?** Suchmaschinen müssen die Seite für den endgültigen Wert rendern.
+
+**Wie beheben?** Liefere den beabsichtigten Wert im Original-HTML und halte beide Fassungen konsistent.
+
+## English
+
+**What does it mean?** The H1 is added or changed by JavaScript.
+
+**Why does it matter?** Search engines must render the page to see the final value.
+
+**How to fix** Deliver the intended value in the original HTML and keep it consistent.
```
### `docs/issues/javascript-noindex-only-in-original-html.md`

```diff
diff --git a/docs/issues/javascript-noindex-only-in-original-html.md b/docs/issues/javascript-noindex-only-in-original-html.md
new file mode 100644
index 00000000..cd5bb8f5
--- /dev/null
+++ b/docs/issues/javascript-noindex-only-in-original-html.md
@@ -0,0 +1,19 @@
+# JavaScript: Noindex Only In Original HTML
+
+Typ: Issue (T-I1) · Priorität: High (P-H1)
+
+## Deutsch
+
+**Was bedeutet das?** Das Original enthält noindex, im gerenderten Dokument fehlt die Direktive.
+
+**Warum ist es relevant?** Die Einschränkung des Originals bleibt trotz der Skriptänderung wirksam.
+
+**Wie beheben?** Setze die beabsichtigten Robots-Direktiven im ausgelieferten HTML.
+
+## English
+
+**What does it mean?** The original HTML contains noindex, but the rendered document removes it.
+
+**Why does it matter?** The original restriction remains effective despite the script change.
+
+**How to fix** Set the intended robots directives in the delivered HTML.
```
### `docs/issues/javascript-meta-description-only-in-rendered-html.md`

```diff
diff --git a/docs/issues/javascript-meta-description-only-in-rendered-html.md b/docs/issues/javascript-meta-description-only-in-rendered-html.md
new file mode 100644
index 00000000..1357ea91
--- /dev/null
+++ b/docs/issues/javascript-meta-description-only-in-rendered-html.md
@@ -0,0 +1,19 @@
+# JavaScript: Meta Description Only In Rendered HTML
+
+Typ: Warning (T-W1) · Priorität: Medium (P-M2)
+
+## Deutsch
+
+**Was bedeutet das?** Meta Description wird durch JavaScript ergänzt oder geändert.
+
+**Warum ist es relevant?** Suchmaschinen müssen die Seite für den endgültigen Wert rendern.
+
+**Wie beheben?** Liefere den beabsichtigten Wert im Original-HTML und halte beide Fassungen konsistent.
+
+## English
+
+**What does it mean?** The Meta Description is added or changed by JavaScript.
+
+**Why does it matter?** Search engines must render the page to see the final value.
+
+**How to fix** Deliver the intended value in the original HTML and keep it consistent.
```
### `docs/issues/javascript-page-title-updated-by-javascript.md`

```diff
diff --git a/docs/issues/javascript-page-title-updated-by-javascript.md b/docs/issues/javascript-page-title-updated-by-javascript.md
new file mode 100644
index 00000000..af8df8ba
--- /dev/null
+++ b/docs/issues/javascript-page-title-updated-by-javascript.md
@@ -0,0 +1,19 @@
+# JavaScript: Page Title Updated By JavaScript
+
+Typ: Warning (T-W1) · Priorität: Medium (P-M2)
+
+## Deutsch
+
+**Was bedeutet das?** Page Title wird durch JavaScript ergänzt oder geändert.
+
+**Warum ist es relevant?** Suchmaschinen müssen die Seite für den endgültigen Wert rendern.
+
+**Wie beheben?** Liefere den beabsichtigten Wert im Original-HTML und halte beide Fassungen konsistent.
+
+## English
+
+**What does it mean?** The Page Title is added or changed by JavaScript.
+
+**Why does it matter?** Search engines must render the page to see the final value.
+
+**How to fix** Deliver the intended value in the original HTML and keep it consistent.
```
### `docs/issues/javascript-canonical-mismatch.md`

```diff
diff --git a/docs/issues/javascript-canonical-mismatch.md b/docs/issues/javascript-canonical-mismatch.md
new file mode 100644
index 00000000..1d5f91b7
--- /dev/null
+++ b/docs/issues/javascript-canonical-mismatch.md
@@ -0,0 +1,19 @@
+# JavaScript: Canonical Mismatch
+
+Typ: Issue (T-I1) · Priorität: High (P-H1)
+
+## Deutsch
+
+**Was bedeutet das?** Original und gerendertes HTML nennen verschiedene Canonical-URLs.
+
+**Warum ist es relevant?** Widersprüchliche Hinweise können zur falschen kanonischen Auswahl führen.
+
+**Wie beheben?** Verwende in beiden Fassungen dieselbe beabsichtigte Canonical-URL.
+
+## English
+
+**What does it mean?** Original and rendered HTML specify different canonical URLs.
+
+**Why does it matter?** Conflicting hints can lead to an unintended canonical selection.
+
+**How to fix** Use the same intended canonical URL in both versions.
```
### `docs/issues/javascript-meta-description-updated-by-javascript.md`

```diff
diff --git a/docs/issues/javascript-meta-description-updated-by-javascript.md b/docs/issues/javascript-meta-description-updated-by-javascript.md
new file mode 100644
index 00000000..c5a03a57
--- /dev/null
+++ b/docs/issues/javascript-meta-description-updated-by-javascript.md
@@ -0,0 +1,19 @@
+# JavaScript: Meta Description Updated By JavaScript
+
+Typ: Warning (T-W1) · Priorität: Medium (P-M2)
+
+## Deutsch
+
+**Was bedeutet das?** Meta Description wird durch JavaScript ergänzt oder geändert.
+
+**Warum ist es relevant?** Suchmaschinen müssen die Seite für den endgültigen Wert rendern.
+
+**Wie beheben?** Liefere den beabsichtigten Wert im Original-HTML und halte beide Fassungen konsistent.
+
+## English
+
+**What does it mean?** The Meta Description is added or changed by JavaScript.
+
+**Why does it matter?** Search engines must render the page to see the final value.
+
+**How to fix** Deliver the intended value in the original HTML and keep it consistent.
```
### `docs/issues/javascript-page-title-only-in-rendered-html.md`

```diff
diff --git a/docs/issues/javascript-page-title-only-in-rendered-html.md b/docs/issues/javascript-page-title-only-in-rendered-html.md
new file mode 100644
index 00000000..346bebed
--- /dev/null
+++ b/docs/issues/javascript-page-title-only-in-rendered-html.md
@@ -0,0 +1,19 @@
+# JavaScript: Page Title Only In Rendered HTML
+
+Typ: Warning (T-W1) · Priorität: Medium (P-M2)
+
+## Deutsch
+
+**Was bedeutet das?** Page Title wird durch JavaScript ergänzt oder geändert.
+
+**Warum ist es relevant?** Suchmaschinen müssen die Seite für den endgültigen Wert rendern.
+
+**Wie beheben?** Liefere den beabsichtigten Wert im Original-HTML und halte beide Fassungen konsistent.
+
+## English
+
+**What does it mean?** The Page Title is added or changed by JavaScript.
+
+**Why does it matter?** Search engines must render the page to see the final value.
+
+**How to fix** Deliver the intended value in the original HTML and keep it consistent.
```
### `docs/issues/javascript-contains-javascript-content.md`

```diff
diff --git a/docs/issues/javascript-contains-javascript-content.md b/docs/issues/javascript-contains-javascript-content.md
new file mode 100644
index 00000000..fa3b323c
--- /dev/null
+++ b/docs/issues/javascript-contains-javascript-content.md
@@ -0,0 +1,19 @@
+# JavaScript: Contains JavaScript Content
+
+Typ: Warning (T-W1) · Priorität: Medium (P-M2)
+
+## Deutsch
+
+**Was bedeutet das?** Das gerenderte Dokument enthält mehr Wörter als das Original.
+
+**Warum ist es relevant?** Zusätzliche Inhalte hängen vom erfolgreichen Rendern ab.
+
+**Wie beheben?** Liefere wichtige Texte im Original-HTML und prüfe das gerenderte Ergebnis.
+
+## English
+
+**What does it mean?** The rendered document contains more words than the original HTML.
+
+**Why does it matter?** Access to this additional content depends on rendering.
+
+**How to fix** Deliver important text in the original HTML and verify the rendered result.
```
### `docs/issues/javascript-pages-with-blocked-resources.md`

```diff
diff --git a/docs/issues/javascript-pages-with-blocked-resources.md b/docs/issues/javascript-pages-with-blocked-resources.md
new file mode 100644
index 00000000..3cde0186
--- /dev/null
+++ b/docs/issues/javascript-pages-with-blocked-resources.md
@@ -0,0 +1,19 @@
+# JavaScript: Pages With Blocked Resources
+
+Typ: Warning (T-W2) · Priorität: High (P-H2)
+
+## Deutsch
+
+**Was bedeutet das?** Beim Rendern wurden Ressourcen durch robots.txt gesperrt.
+
+**Warum ist es relevant?** Gesperrte Ressourcen können Inhalte verbergen oder die Darstellung verändern.
+
+**Wie beheben?** Prüfe die robots.txt-Regeln und erlaube die für das Rendern benötigten Ressourcen.
+
+## English
+
+**What does it mean?** Rendering was prevented from loading resources by robots.txt.
+
+**Why does it matter?** Blocked resources may hide content or alter the rendered page.
+
+**How to fix** Review robots.txt rules for the listed resources and allow those needed for rendering.
```
### `docs/issues/javascript-canonical-only-in-rendered-html.md`

```diff
diff --git a/docs/issues/javascript-canonical-only-in-rendered-html.md b/docs/issues/javascript-canonical-only-in-rendered-html.md
new file mode 100644
index 00000000..2cf95307
--- /dev/null
+++ b/docs/issues/javascript-canonical-only-in-rendered-html.md
@@ -0,0 +1,19 @@
+# JavaScript: Canonical Only In Rendered HTML
+
+Typ: Warning (T-W1) · Priorität: Low (P-L1)
+
+## Deutsch
+
+**Was bedeutet das?** Ein HTML-Canonical wird erst beim Rendern ergänzt.
+
+**Warum ist es relevant?** Die Erkennung des Canonicals hängt von JavaScript ab.
+
+**Wie beheben?** Ergänze den Canonical-Link im Head des ausgelieferten HTML.
+
+## English
+
+**What does it mean?** An HTML canonical is added during rendering.
+
+**Why does it matter?** Canonical discovery depends on JavaScript execution.
+
+**How to fix** Include the canonical link in the delivered HTML head.
```
### `docs/issues/javascript-pages-with-javascript-errors.md`

```diff
diff --git a/docs/issues/javascript-pages-with-javascript-errors.md b/docs/issues/javascript-pages-with-javascript-errors.md
new file mode 100644
index 00000000..e37d1c90
--- /dev/null
+++ b/docs/issues/javascript-pages-with-javascript-errors.md
@@ -0,0 +1,19 @@
+# JavaScript: Pages With JavaScript Errors
+
+Typ: Warning (T-W1) · Priorität: Low (P-L1)
+
+## Deutsch
+
+**Was bedeutet das?** Beim Rendern wurde ein Konsolenfehler oder eine unbehandelte Ausnahme erfasst.
+
+**Warum ist es relevant?** Fehlerhafte Skripte können Seitenfunktionen verhindern.
+
+**Wie beheben?** Prüfe den vollständigen Konsolenkontext und korrigiere das betroffene Skript.
+
+## English
+
+**What does it mean?** Rendering recorded a console error or uncaught exception.
+
+**Why does it matter?** A failing script may prevent page features from working.
+
+**How to fix** Inspect the full console context and fix the failing script.
```
### `docs/issues/javascript-h1-updated-by-javascript.md`

```diff
diff --git a/docs/issues/javascript-h1-updated-by-javascript.md b/docs/issues/javascript-h1-updated-by-javascript.md
new file mode 100644
index 00000000..f3d2ddf8
--- /dev/null
+++ b/docs/issues/javascript-h1-updated-by-javascript.md
@@ -0,0 +1,19 @@
+# JavaScript: H1 Updated By JavaScript
+
+Typ: Warning (T-W1) · Priorität: Medium (P-M2)
+
+## Deutsch
+
+**Was bedeutet das?** H1 wird durch JavaScript ergänzt oder geändert.
+
+**Warum ist es relevant?** Suchmaschinen müssen die Seite für den endgültigen Wert rendern.
+
+**Wie beheben?** Liefere den beabsichtigten Wert im Original-HTML und halte beide Fassungen konsistent.
+
+## English
+
+**What does it mean?** The H1 is added or changed by JavaScript.
+
+**Why does it matter?** Search engines must render the page to see the final value.
+
+**How to fix** Deliver the intended value in the original HTML and keep it consistent.
```
### `docs/issues/javascript-nofollow-only-in-original-html.md`

```diff
diff --git a/docs/issues/javascript-nofollow-only-in-original-html.md b/docs/issues/javascript-nofollow-only-in-original-html.md
new file mode 100644
index 00000000..94cbf148
--- /dev/null
+++ b/docs/issues/javascript-nofollow-only-in-original-html.md
@@ -0,0 +1,19 @@
+# JavaScript: Nofollow Only In Original HTML
+
+Typ: Issue (T-I1) · Priorität: High (P-H1)
+
+## Deutsch
+
+**Was bedeutet das?** Das Original enthält nofollow, im gerenderten Dokument fehlt die Direktive.
+
+**Warum ist es relevant?** Die Einschränkung des Originals bleibt trotz der Skriptänderung wirksam.
+
+**Wie beheben?** Setze die beabsichtigten Robots-Direktiven im ausgelieferten HTML.
+
+## English
+
+**What does it mean?** The original HTML contains nofollow, but the rendered document removes it.
+
+**Why does it matter?** The original restriction remains effective despite the script change.
+
+**How to fix** Set the intended robots directives in the delivered HTML.
```

## Fremdänderung (K2-72)

`os-engine/src/worker.rs`: analog zur noindex-Behandlung die maßgeblichen
Original-nofollow/none-Direktiven übernehmen; TECH-106.

### `crates/os-engine/src/worker.rs`

```diff
diff --git a/crates/os-engine/src/worker.rs b/crates/os-engine/src/worker.rs
index c53c7c32..6d484cff 100644
--- a/crates/os-engine/src/worker.rs
+++ b/crates/os-engine/src/worker.rs
@@ -789,0 +790,12 @@ async fn render_page<F: Fetch>(
+            // TECH-106: original nofollow/none remains authoritative after rendering.
+            if !raw.effective_follow {
+                dom.effective_follow = false;
+                for d in &raw.directives {
+                    if d.applies
+                        && d.tokens.iter().any(|t| t == "nofollow" || t == "none")
+                        && !dom.directives.contains(d)
+                    {
+                        dom.directives.push(d.clone());
+                    }
+                }
+            }
```

## Fremdänderung (K2-73)

TECH-102 / CRAWL-083: Default-Trait-Methode `blocked_reason`, Übernahme dieses
Grunds in der Request-Interception, präzise Engine-Policy und ergänzter
Modellkommentar. `allowed` und `allowed_resource` bleiben unverändert; die
bisherige Hauptdokument-Entscheidung über `allowed` bleibt erhalten.

### `crates/os-render/src/api.rs`

```diff
diff --git a/crates/os-render/src/api.rs b/crates/os-render/src/api.rs
index 74fcf984..70fc7dff 100644
--- a/crates/os-render/src/api.rs
+++ b/crates/os-render/src/api.rs
@@ -147,0 +148,16 @@ pub trait ResourcePolicy: Send + Sync + std::fmt::Debug {
+    /// Reason for denying a resource; standalone policies retain their robots default.
+    fn blocked_reason<'a>(
+        &'a self,
+        url: &'a str,
+        resource_type: &'a str,
+    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<&'static str>> + Send + 'a>>
+    {
+        Box::pin(async move {
+            if self.allowed_resource(url, resource_type).await {
+                None
+            } else {
+                Some("robots")
+            }
+        })
+    }
+
```
### `crates/os-render/src/intercept.rs`

```diff
diff --git a/crates/os-render/src/intercept.rs b/crates/os-render/src/intercept.rs
index a72bbd33..fa8e37be 100644
--- a/crates/os-render/src/intercept.rs
+++ b/crates/os-render/src/intercept.rs
@@ -393,3 +393,13 @@ impl Intercept {
-                        let allowed = match &policy {
-                            Some(policy) if resource_url != main_url => {
-                                policy.allowed_resource(resource_url, resource_type).await
+                        let blocked_reason = match &policy {
+                            Some(policy) if resource_url == main_url => {
+                                if policy.allowed(resource_url).await {
+                                    None
+                                } else {
+                                    policy
+                                        .blocked_reason(resource_url, resource_type)
+                                        .await
+                                        .or(Some("robots"))
+                                }
+                            }
+                            Some(policy) => {
+                                policy.blocked_reason(resource_url, resource_type).await
@@ -397,2 +407 @@ impl Intercept {
-                            Some(policy) => policy.allowed(resource_url).await,
-                            None => true,
+                            None => None,
@@ -400,6 +409,4 @@ impl Intercept {
-                        if !allowed {
-                            data.lock().expect("capture").block(
-                                resource_url.into(),
-                                "robots",
-                                None,
-                            );
+                        if let Some(reason) = blocked_reason {
+                            data.lock()
+                                .expect("capture")
+                                .block(resource_url.into(), reason, None);
```
### `crates/os-model/src/write_m2.rs`

```diff
diff --git a/crates/os-model/src/write_m2.rs b/crates/os-model/src/write_m2.rs
index e44981a6..f2ddedf3 100644
--- a/crates/os-model/src/write_m2.rs
+++ b/crates/os-model/src/write_m2.rs
@@ -62 +62 @@ pub struct BlockedResource {
-    /// Grund: `robots` oder `error`.
+    /// Grund: `robots`, `error` oder `excluded` (Crawl-/Scope-Ausschluss).
```
### `crates/os-engine/src/render_glue.rs`

```diff
diff --git a/crates/os-engine/src/render_glue.rs b/crates/os-engine/src/render_glue.rs
index 9b028746..65491429 100644
--- a/crates/os-engine/src/render_glue.rs
+++ b/crates/os-engine/src/render_glue.rs
@@ -192,0 +193,56 @@ impl<F: Fetch> os_render::api::ResourcePolicy for Policy<F> {
+    fn blocked_reason<'a>(
+        &'a self,
+        url: &'a str,
+        resource_type: &'a str,
+    ) -> BoxFuture<'a, Option<&'static str>> {
+        Box::pin(async move {
+            let (Some(sh), Ok(target)) = (self.0.upgrade(), Url::parse(url)) else {
+                return Some("excluded");
+            };
+            let scope = sh.scope.classify(&os_url::normalize(&target));
+            if scope == os_url::ScopeClass::External && !sh.cfg.spider.external.crawl {
+                return Some("excluded");
+            }
+            let kind = match resource_type {
+                "Image" => Some(LinkType::Image),
+                "Stylesheet" | "Font" | "Other" => Some(LinkType::Css),
+                "Script" => Some(LinkType::JavaScript),
+                "Media" => Some(LinkType::Media),
+                "Document" => Some(LinkType::Iframe),
+                "Navigation" => Some(LinkType::Hyperlink),
+                _ => None,
+            };
+            if let Some(kind) = kind {
+                let link = ExtractedLink {
+                    link_type: kind,
+                    resolved: Some(url.into()),
+                    crawlable: true,
+                    ..Default::default()
+                };
+                if !crate::decide::decide(&link, scope, &sh.cfg, true).crawl {
+                    return Some("excluded");
+                }
+            }
+            if !sh.cfg.extraction.cookies && analytics_host(target.host_str().unwrap_or_default()) {
+                return Some("excluded");
+            }
+            if sh.fetcher.target_denial(&target).await.is_some() {
+                return Some("excluded");
+            }
+            let check = sh.gate.check(&*sh.fetcher, &target).await;
+            if let Some((result, write)) = check.robots_fetch {
+                if crate::robots_glue::record_robots(&sh, result, write)
+                    .await
+                    .is_err()
+                {
+                    return Some("excluded");
+                }
+            }
+            matches!(
+                check.decision,
+                os_fetch::robots::RobotsDecision::Blocked { .. }
+            )
+            .then_some("robots")
+        })
+    }
+
```

## Stub-Ablösung (K2-60)

Genau drei eigene R02-Einträge aus der F05a-Stub-Liste entfernt.
In `filters_registry.rs` zählt der JavaScript-Tab jetzt als umgesetzt: 5 → 4 Stub-Tabs.

### `crates/os-engine/f05a-stubs.toml`

```diff
diff --git a/crates/os-engine/f05a-stubs.toml b/crates/os-engine/f05a-stubs.toml
index 219f36ca..0f4d18e4 100644
--- a/crates/os-engine/f05a-stubs.toml
+++ b/crates/os-engine/f05a-stubs.toml
@@ -136,18 +135,0 @@ signature = "fn main()"
-
-[[stub]]
-owner = "M2-R02"
-file = "crates/os-analyze/src/tabs/javascript.rs"
-function = "tab"
-signature = "pub fn tab() -> TabDef"
-
-[[stub]]
-owner = "M2-R02"
-file = "crates/os-export/src/catalog/bulk_javascript.rs"
-function = "entries"
-signature = "pub fn entries() -> &'static [TableEntry]"
-
-[[stub]]
-owner = "M2-R02"
-file = "crates/os-export/src/catalog/bulk_screenshots.rs"
-function = "entries"
-signature = "pub fn entries() -> &'static [TableEntry]"
```
### `crates/os-analyze/tests/filters_registry.rs`

```diff
diff --git a/crates/os-analyze/tests/filters_registry.rs b/crates/os-analyze/tests/filters_registry.rs
index 26e8678d..5a97b63c 100644
--- a/crates/os-analyze/tests/filters_registry.rs
+++ b/crates/os-analyze/tests/filters_registry.rs
@@ -291 +298 @@ fn every_tab_is_a_real_definition_or_a_marked_stub() {
-        stubs, 5,
+        stubs, 4,
```

## Platzhalter-Ablösung (K2-71)

Keine zusätzlichen fremden Ablehnungen mussten abgelöst werden. Eigene Tab-
und Export-Stubs sowie die drei ausdrücklich zugewiesenen M1-Test-Ignores
wurden im eigenen Dateibesitz ersetzt.

## Erwartungsanpassungen (K2-63)

`crates/os-analyze/tests/filters_registry.rs`: für JavaScript:Pages With JavaScript Errors zusätzlich `rendering.js_error_reporting` erwarten (TECH-114: expliziter Konfigurationshinweis bei deaktivierter Erfassung). Zusätzlich erwartet `framework_registry.rs` für JavaScript:All den Zustand NeedsRendering und keinen Prozentwert (TECH-101). `os-mcp/tests/tools_read.rs` erwartet neue JavaScript-Issues als needs_rendering statt needs_crawl_analysis (TECH-102–111/114). Testlogik unverändert; die einzige bestehende Golden-Ergänzung steht unter K2-61.

### `crates/os-analyze/tests/framework_registry.rs`

```diff
diff --git a/crates/os-analyze/tests/framework_registry.rs b/crates/os-analyze/tests/framework_registry.rs
index 9b530347..dac9bcea 100644
--- a/crates/os-analyze/tests/framework_registry.rs
+++ b/crates/os-analyze/tests/framework_registry.rs
@@ -144,2 +144,7 @@ fn overview_labels_use_the_current_thresholds_and_states_are_complete() {
-            FilterState::Ready,
-            "{}:All ist immer bereit",
+            // TECH-101 / K2-63: the implemented JavaScript population requires rendering.
+            if c.tab == "JavaScript" {
+                FilterState::NeedsRendering
+            } else {
+                FilterState::Ready
+            },
+            "{}:All Zustand",
@@ -150 +155,5 @@ fn overview_labels_use_the_current_thresholds_and_states_are_complete() {
-            Some(true),
+            if c.tab == "JavaScript" {
+                None
+            } else {
+                Some(true)
+            },
```
### `crates/os-mcp/tests/tools_read.rs`

```diff
diff --git a/crates/os-mcp/tests/tools_read.rs b/crates/os-mcp/tests/tools_read.rs
index b19ec9ad..6af0027a 100644
--- a/crates/os-mcp/tests/tools_read.rs
+++ b/crates/os-mcp/tests/tools_read.rs
@@ -1745,0 +1746,2 @@ async fn issues_that_need_the_crawl_analysis_are_named_instead_of_counted() {
+            // K2-63, TECH-102–111/114: JavaScript issues require rendered pages.
+            (Some("JavaScript"), _) => "needs_rendering",
```
### `crates/os-analyze/tests/filters_registry.rs`

```diff
diff --git a/crates/os-analyze/tests/filters_registry.rs b/crates/os-analyze/tests/filters_registry.rs
index 26e8678d..5a97b63c 100644
--- a/crates/os-analyze/tests/filters_registry.rs
+++ b/crates/os-analyze/tests/filters_registry.rs
@@ -177 +177,7 @@ fn filters_match_the_registry_files_in_id_order_kind_and_requirements() {
-                actual_config, config,
+                actual_config,
+                // TECH-114 / K2-63: disabled collection must expose NeedsConfig.
+                if tab.id == "JavaScript" && f.id == "Pages With JavaScript Errors" {
+                    vec!["rendering.js_error_reporting".to_string()]
+                } else {
+                    config
+                },
@@ -179 +185,2 @@ fn filters_match_the_registry_files_in_id_order_kind_and_requirements() {
-                tab.id, f.id
+                tab.id,
+                f.id
```
