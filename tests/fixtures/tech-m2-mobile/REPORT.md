# M2-D23 – Paketbericht

Ausgangsstand `e6f70783`, Branch `m2/d23`; Worktree beim Start sauber. Clean-Room eingehalten.

## Umgesetzt

TECH-230, TECH-231, TECH-232, TECH-233, TECH-234, TECH-235, TECH-236, TECH-237,
INTEG-140, INTEG-146, INTEG-147 sowie UI-064 [P1].

Mobile-Tab mit neun Standardspalten und sechs Fachfiltern, interne HTML-200-Population,
Scores 0–100 und Teil-Scores, Fehler-/Konfigurations-/Verfügbarkeitshinweise,
Lokal-/PSI-Wege, ältere tap-targets-Audits, Plugin-DOM-Rückfall und Mobil-Alternativen
unabhängig von PSI. Bereits von E01 gespeicherte Mobil-Alternativlinks älterer Crawls bleiben ohne Neuabruf nutzbar, einschließlich Reihenfolge und Anzahl. Details aus den Mobile-Audits bleiben als mobile.details:* in api_values erhalten;
Font-Display-Erklärung und Alternate-Anzahl stehen über die Fach-API bereit.
Issue-Katalog mit eigener Herleitung, eigenen DE/EN-Texten und sechs generierten Dokumentations-Goldens.

## Prüfungen

Lokal grün auf macOS (Abschlusslauf, nach der Legacy-Ergänzung):

| Prüfung | Ergebnis | Laufzeit |
|---|---|---|
| `cargo fmt --all --check` | ok | 2.54 s |
| `cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings` | ok | 81.35 s |
| `cargo test --workspace --exclude openspider-desktop --locked` | ok; 3644 bestanden, 53 bestehende Ignores | 1254.88 s einschließlich Build |
| `cargo deny check licenses bans sources` | bans, licenses, sources ok | – |
| `npm run -w apps/desktop check:licenses` | 474 Pakete, alle erlaubt | – |
| `bash scripts/check-neutral.sh` | keine Fundstellen | – |
| `git diff --check` | ok | – |

Eigene Fachprüfungen: 9 Mobile-Crawl-/View-Tests (inklusive Legacy-Dateipfad),
8 Mobile-PSI-/Lighthouse-Tests und 1 Issue-Katalog-Benchmark, alle grün.
Kein `apps/desktop/src` geändert; die zusätzliche Frontend-Prüffolge aus dem Auftrag ist daher nicht erforderlich.
Target-Größe nach Abschluss: 19G.
Die Fachtests umfassen reale Fixture-Crawls, einen Render-Hook, PSI ausschließlich auf Loopback,
einen lokalen Lighthouse-Ersatzprozess und synthetische Lighthouse-JSON-Fixtures (K2-09).
Keine neuen Ignores oder Abhängigkeiten.

## Abweichungen und offene Fragen

- TECH-230 erklärt sich ausdrücklich als führend für den Mobile-Tab und verlangt bei abgewählter Gruppe
  leere Prüfspalten plus Hinweise. INTEG-133 fordert dagegen das Entfernen der Prüfspalten/-filter.
  Umsetzung folgt TECH-230; die neun Spalten und Filter bleiben erhalten. Der Planer sollte den
  widersprüchlichen Satz in INTEG-133 angleichen. Keine blockierende Rückfrage.
- K2-09: die eigenen D23-Tests benötigen keine echte Node-/Chrome-/Lighthouse-Installation; lokale Verarbeitung gegen
  JSON-Fixtures und Ersatzprozess. Keine Aussage über tatsächlich gemessene Audit-Ergebnisse einer
  konkreten installierten Lighthouse-Version. Bündelung bleibt wie geplant M3.
- macOS lokal; kein Push, Merge oder PR und daher kein CI-Lauf / keine Windows- oder Linux-Abnahme.
- Der erste vollständige Workspace-Build scheiterte beim Linken an Plattenmangel (errno=28).
  Danach wurde ausschließlich das eigene Target-Verzeichnis bereinigt. Ein weiterer Lauf mit entfernten
  Symbolen wurde bei erneut stark wachsendem Platzbedarf kontrolliert abgebrochen. Der Abschlusslauf verwendet
  CARGO_INCREMENTAL=0, CARGO_PROFILE_{DEV,TEST}_DEBUG=0, CARGO_PROFILE_{DEV,TEST}_STRIP=symbols,
  CARGO_PROFILE_TEST_OPT_LEVEL=1, CARGO_BUILD_JOBS=4 und MACOSX_DEPLOYMENT_TARGET=13.0.
  Die Panic-/Logging-Tests von os-app und os-cli prüfen Funktionsnamen in Backtraces. Deshalb behalten diese beiden Pakete
  ihre Symbole über `--config 'profile.test.package.os-app.strip="debuginfo"'` und
  `--config 'profile.test.package.os-cli.strip="debuginfo"'`; für die übrigen Pakete gilt strip=symbols.
  Der vorherige Versuch ohne diese Overrides scheiterte ausschließlich am namensprüfenden App-Backtrace-Test.
  Testumfang und Quellcode bleiben gleich; Debug-Assertions bleiben aktiviert.

## Fremdergänzungen (K2-61)

Jede Ergänzung ist unten mit Datei, Begründung und vollständigem Diff aufgeführt.
Bestehende öffentliche Signaturen bleiben unverändert; neue Traitmethoden haben einen kompatiblen Default.

### `crates/os-parse/src/lib.rs`

TECH-236/237, INTEG-146/147: zusätzliche Modulregistrierung und Aufruf am vorhandenen DOM.

```diff
diff --git a/crates/os-parse/src/lib.rs b/crates/os-parse/src/lib.rs
index 9b432e89..d4d77e07 100644
--- a/crates/os-parse/src/lib.rs
+++ b/crates/os-parse/src/lib.rs
@@ -27,0 +28 @@ pub mod links;
+pub mod mobile;
@@ -251,0 +253 @@ pub fn parse_html(input: &ParseInput) -> ParsedPage {
+    mobile::run(&ctx, &mut page);
```

### `crates/os-parse/src/mobile.rs`

TECH-236/237: neues Parsermodul; schreibt ausschließlich Fakten im eigenen Namensraum mobile.*. Keine neuen Abhängigkeiten.

```diff
diff --git a/crates/os-parse/src/mobile.rs b/crates/os-parse/src/mobile.rs
new file mode 100644
index 00000000..726a8f65
--- /dev/null
+++ b/crates/os-parse/src/mobile.rs
@@ -0,0 +1,102 @@
+//! DOM-Fakten für Mobile (TECH-236/237), unabhängig von der PSI-Anbindung.
+use crate::{alternates, dom, PageCtx};
+use os_model::{ExtKey, ParsedPage, Value};
+
+fn plugin_url(value: &str) -> bool {
+    let path = value
+        .split(['?', '#'])
+        .next()
+        .unwrap_or_default()
+        .trim()
+        .to_ascii_lowercase();
+    [".swf", ".xap", ".class", ".jar"]
+        .iter()
+        .any(|ext| path.ends_with(ext))
+}
+
+/// Prüft den bereits aufgebauten Baum; Skripttexte zählen nicht als Elemente.
+pub fn run(ctx: &PageCtx<'_>, page: &mut ParsedPage) {
+    let failed = ctx
+        .doc
+        .html()
+        .tree
+        .nodes()
+        .filter_map(scraper::ElementRef::wrap)
+        .any(|el| {
+            if dom::in_template(el) {
+                return false;
+            }
+            let e = el.value();
+            match e.name() {
+                "applet" => true,
+                "object" | "embed" => {
+                    let mime = e
+                        .attr("type")
+                        .unwrap_or_default()
+                        .trim()
+                        .to_ascii_lowercase();
+                    let class = e
+                        .attr("classid")
+                        .unwrap_or_default()
+                        .trim()
+                        .to_ascii_lowercase();
+                    matches!(
+                        mime.as_str(),
+                        "application/x-shockwave-flash" | "application/futuresplash"
+                    ) || mime.starts_with("application/x-silverlight")
+                        || mime.starts_with("application/x-java-applet")
+                        || [
+                            "d27cdb6e-ae6d-11cf-96b8-444553540000",
+                            "dfeaf541-f3e1-4c24-acac-99c30715084a",
+                            "8ad9c840-044e-11d1-b3e9-00805f499d93",
+                            "cafeefac-",
+                        ]
+                        .iter()
+                        .any(|id| {
+                            class
+                                .strip_prefix("clsid:")
+                                .unwrap_or(&class)
+                                .starts_with(id)
+                        })
+                        || ["src", "data"]
+                            .iter()
+                            .any(|a| e.attr(a).is_some_and(plugin_url))
+                }
+                "param" => {
+                    e.attr("name")
+                        .is_some_and(|n| n.eq_ignore_ascii_case("movie"))
+                        && e.attr("value").is_some_and(|v| {
+                            v.split(['?', '#'])
+                                .next()
+                                .unwrap_or_default()
+                                .to_ascii_lowercase()
+                                .ends_with(".swf")
+                        })
+                }
+                _ => false,
+            }
+        });
+    page.ext.push((
+        ExtKey::from_static("mobile.plugins"),
+        Value::Int(if failed { 0 } else { 100 }),
+    ));
+    if ctx.cfg.crawl.spider.mobile_alternate.store {
+        let html = alternates::extract(ctx.doc, ctx.base, ctx.cfg);
+        let headers = alternates::from_headers(ctx.url, ctx.headers, ctx.cfg);
+        let targets: Vec<_> = html
+            .iter()
+            .chain(&headers)
+            .filter_map(|l| l.resolved.as_ref())
+            .collect();
+        if let Some(first) = targets.first() {
+            page.ext.push((
+                ExtKey::from_static("mobile.alternate"),
+                Value::Text((*first).clone()),
+            ));
+        }
+        page.ext.push((
+            ExtKey::from_static("mobile.alternate_count"),
+            Value::Int(targets.len() as i64),
+        ));
+    }
+}
```

### `crates/os-integrations/src/psi/mod.rs`

TECH-230, INTEG-140: Mobile-Modul registriert; Zusatzkategorien und lokaler Mobile-Lauf nur bei gewählter Gruppe, Konfigurationsfilterung der neuen Werte.

```diff
diff --git a/crates/os-integrations/src/psi/mod.rs b/crates/os-integrations/src/psi/mod.rs
index 2c6b9f80..70c0ea52 100644
--- a/crates/os-integrations/src/psi/mod.rs
+++ b/crates/os-integrations/src/psi/mod.rs
@@ -16,0 +17 @@ pub mod local;
+pub mod mobile;
@@ -192,0 +194 @@ impl PsiSource {
+        let url = mobile::request_categories(url, &ctx.cfg);
@@ -257,0 +260,5 @@ impl PsiSource {
+        if mobile::selected(&ctx.cfg) {
+            return Ok(runner
+                .run_selected(page, ctx.cfg.api.psi.device, &headers, true)
+                .await);
+        }
@@ -266,0 +274,4 @@ impl PsiSource {
+        let outcome = outcome.map(|mut out| {
+            mobile::apply_config(&mut out, &ctx.cfg);
+            out
+        });
```

### `crates/os-integrations/src/psi/lighthouse.rs`

INTEG-140: zusätzlicher Mobile-Mapper am gemeinsamen Parser; Remote-Inhaltsbreite ausschließlich für die neue Mobile-Ausgabe entfernt.

```diff
diff --git a/crates/os-integrations/src/psi/lighthouse.rs b/crates/os-integrations/src/psi/lighthouse.rs
index 32861240..c55a2e35 100644
--- a/crates/os-integrations/src/psi/lighthouse.rs
+++ b/crates/os-integrations/src/psi/lighthouse.rs
@@ -90,0 +91 @@ pub fn parse_psi(body: &Value) -> Outcome {
+        super::mobile::remote(&mut out);
@@ -114,0 +116 @@ pub fn parse_lighthouse(lh: &Value) -> Outcome {
+    super::mobile::append(lh, &mut out);
```

### `crates/os-integrations/src/psi/local.rs`

TECH-230, INTEG-140: Launcher::mobile_command mit kompatibler Standardimplementierung, zusätzlicher run_selected-Lauf. Die vorhandenen Methoden behalten Signatur und Verhalten: run delegiert mit mobile=false; execute verwendet denselben Prozessablauf in execute_command. Keine Veränderung der alten Performance-Auswahl.

```diff
diff --git a/crates/os-integrations/src/psi/local.rs b/crates/os-integrations/src/psi/local.rs
index 18949df2..44eae1ce 100644
--- a/crates/os-integrations/src/psi/local.rs
+++ b/crates/os-integrations/src/psi/local.rs
@@ -94,0 +95,4 @@ pub trait Launcher: Send + Sync {
+    /// Mobiler Lauf; vorhandene Test-/Spezialläufer behalten standardmäßig ihr Verhalten.
+    fn mobile_command(&self, job: &Job) -> Command {
+        self.command(job)
+    }
@@ -185,0 +190,11 @@ impl Launcher for NodeLighthouse {
+
+    fn mobile_command(&self, job: &Job) -> Command {
+        let mut cmd = Command::new(&self.program);
+        cmd.args(&self.prefix).args(
+            Self::args(job)
+                .into_iter()
+                .filter(|a| a != "--only-categories=performance"),
+        );
+        cmd.arg("--only-categories=performance,seo,accessibility");
+        cmd
+    }
@@ -319,0 +335,11 @@ impl LocalRunner {
+        self.run_selected(url, device, headers, false).await
+    }
+
+    /// Lauf mit zusätzlicher Mobile-Audit-Auswahl (TECH-230, INTEG-140).
+    pub async fn run_selected(
+        &self,
+        url: &str,
+        device: PsiDevice,
+        headers: &[(String, String)],
+        mobile: bool,
+    ) -> Outcome {
@@ -344 +370,5 @@ impl LocalRunner {
-        let outcome = self.execute(&job).await;
+        let outcome = if mobile {
+            self.execute_mobile(&job).await
+        } else {
+            self.execute(&job).await
+        };
@@ -355,0 +386,9 @@ impl LocalRunner {
+        self.execute_command(job, &mut cmd).await
+    }
+
+    async fn execute_mobile(&self, job: &Job) -> Outcome {
+        let mut cmd = self.launcher.mobile_command(job);
+        self.execute_command(job, &mut cmd).await
+    }
+
+    async fn execute_command(&self, job: &Job, cmd: &mut Command) -> Outcome {
```

### `crates/os-analyze/src/defs.rs`

TECH-230: ColumnSource::file_filters als zusätzliche Methode mit leerem Default für dateiabhängige Hinweise.

```diff
diff --git a/crates/os-analyze/src/defs.rs b/crates/os-analyze/src/defs.rs
index 37203806..f17a67f8 100644
--- a/crates/os-analyze/src/defs.rs
+++ b/crates/os-analyze/src/defs.rs
@@ -233,0 +234,4 @@ pub trait ColumnSource: Send + Sync {
+    /// Zusätzliche Filterdefinitionen mit dateiabhängigen Voraussetzungen (TECH-230).
+    fn file_filters(&self, _cfg: &CrawlConfig, _file: &os_store::CrawlFile) -> Vec<FilterDef> {
+        Vec::new()
+    }
```

### `crates/os-analyze/src/tabs/mod.rs`

TECH-230/231: Mobile-Quelle in der Registerliste und zusätzliche Anwendung der optionalen file_filters; bestehende Quellen liefern hierfür unverändert die leere Liste.

```diff
diff --git a/crates/os-analyze/src/tabs/mod.rs b/crates/os-analyze/src/tabs/mod.rs
index c3e94f5a..6ac3ded6 100644
--- a/crates/os-analyze/src/tabs/mod.rs
+++ b/crates/os-analyze/src/tabs/mod.rs
@@ -184,0 +185 @@ pub fn column_sources() -> Vec<(&'static str, Box<dyn ColumnSource>)> {
+    add("Mobile", mobile::column_source());
@@ -1146,0 +1148,9 @@ fn configure_sources(
+            if let Some(file) = file {
+                for filter in source.file_filters(cfg, file) {
+                    if let Some(slot) = tab.filters.iter_mut().find(|f| f.id == filter.id) {
+                        *slot = filter;
+                    } else {
+                        tab.filters.push(filter);
+                    }
+                }
+            }
```

### Generierte Dokumentations-Goldens

UI-064 verlangt für jeden neuen Katalogeintrag die aus eigenem Katalog/Text generierte Dokumentationsseite. Ausschließlich neue Mobile-Ausgabe:

#### `docs/issues/mobile-content-not-sized-correctly.md`

```diff
diff --git a/docs/issues/mobile-content-not-sized-correctly.md b/docs/issues/mobile-content-not-sized-correctly.md
new file mode 100644
index 00000000..d5691908
--- /dev/null
+++ b/docs/issues/mobile-content-not-sized-correctly.md
@@ -0,0 +1,21 @@
+# Mobile: Content Not Sized Correctly
+
+Typ: Issue (T-W2) · Priorität: High (P-H3)
+
+Abweichung von der Herleitung: benchmark-parity: TECH-234 / UI-064 stufen den ausführungsabhängigen Ausschluss mobiler Nutzer als Issue ein.
+
+## Deutsch
+
+**Was bedeutet das?** Der Inhalt ist laut lokalem Lighthouse-Audit breiter als der Viewport. Der Score für Content Width liegt unter 100.
+
+**Warum ist es relevant?** Horizontales Scrollen erschwert die Bedienung auf kleinen Geräten und kann wesentliche Inhalte außerhalb des sichtbaren Bereichs verbergen.
+
+**Wie beheben?** Entfernen Sie starre Inhaltsbreiten, begrenzen Sie Bilder und Einbettungen auf die verfügbare Breite und prüfen Sie das responsive Layout.
+
+## English
+
+**What does it mean?** The local Lighthouse audit reports content wider than the viewport. The Content Width score is below 100.
+
+**Why does it matter?** Horizontal scrolling makes small screens difficult to use and can hide essential content outside the visible area.
+
+**How to fix** Remove fixed content widths, constrain images and embeds to the available width and check the responsive layout.
```

#### `docs/issues/mobile-illegible-font-size.md`

```diff
diff --git a/docs/issues/mobile-illegible-font-size.md b/docs/issues/mobile-illegible-font-size.md
new file mode 100644
index 00000000..6336ccb4
--- /dev/null
+++ b/docs/issues/mobile-illegible-font-size.md
@@ -0,0 +1,21 @@
+# Mobile: Illegible Font Size
+
+Typ: Issue (T-W2) · Priorität: High (P-H3)
+
+Abweichung von der Herleitung: benchmark-parity: TECH-235 / UI-064 stufen die ausführungsabhängige Unlesbarkeit als Issue ein.
+
+## Deutsch
+
+**Was bedeutet das?** Lighthouse meldet einen zu großen Anteil kleiner Schrift. Die kompatibel benannte Spalte Font Display zeigt einen Score unter 100.
+
+**Warum ist es relevant?** Zu kleine Schrift schließt Nutzer mit eingeschränktem Sehvermögen aus und macht längere Texte auf mobilen Geräten schwer lesbar.
+
+**Wie beheben?** Verwenden Sie gut lesbare Schriftgrößen. Prüfen Sie besonders längere Textbereiche unter 12 CSS-Pixeln im mobilen Layout.
+
+## English
+
+**What does it mean?** Lighthouse reports an excessive share of small text. The compatibility column named Font Display contains a score below 100.
+
+**Why does it matter?** Small text excludes users with reduced vision and makes extended reading on mobile devices difficult.
+
+**How to fix** Use readable font sizes and inspect substantial text areas below 12 CSS pixels in the mobile layout.
```

#### `docs/issues/mobile-mobile-alternate-link.md`

```diff
diff --git a/docs/issues/mobile-mobile-alternate-link.md b/docs/issues/mobile-mobile-alternate-link.md
new file mode 100644
index 00000000..3d80c275
--- /dev/null
+++ b/docs/issues/mobile-mobile-alternate-link.md
@@ -0,0 +1,19 @@
+# Mobile: Mobile Alternate Link
+
+Typ: Warning (T-W1) · Priorität: High (P-H2)
+
+## Deutsch
+
+**Was bedeutet das?** Die Seite verweist über ein Alternate-Link-Element oder einen HTTP-Link-Header auf eine separate mobile Version.
+
+**Warum ist es relevant?** Getrennte mobile URLs können beabsichtigt sein. Unterschiedliche Inhalte und widersprüchliche Zuordnungssignale können jedoch die falsche Version ausliefern.
+
+**Wie beheben?** Prüfen Sie die Erreichbarkeit und Inhaltsgleichheit beider Versionen sowie ihre Canonical- und Alternate-Verweise. Erwägen Sie ein responsives Layout.
+
+## English
+
+**What does it mean?** The page points to a separate mobile version through an alternate link element or an HTTP Link header.
+
+**Why does it matter?** Separate mobile URLs can be intentional, but differing content and conflicting mapping signals can cause the wrong version to be served.
+
+**How to fix** Check that both versions are reachable and equivalent and verify their canonical and alternate links. Consider a responsive layout.
```

#### `docs/issues/mobile-target-size.md`

```diff
diff --git a/docs/issues/mobile-target-size.md b/docs/issues/mobile-target-size.md
new file mode 100644
index 00000000..68d9503c
--- /dev/null
+++ b/docs/issues/mobile-target-size.md
@@ -0,0 +1,21 @@
+# Mobile: Target Size
+
+Typ: Issue (T-W2) · Priorität: Medium (P-M1)
+
+Abweichung von der Herleitung: benchmark-parity: TECH-233 / UI-064 ordnen die ausführungsabhängige Bedienbarkeitsprüfung als Issue ein.
+
+## Deutsch
+
+**Was bedeutet das?** Lighthouse beanstandet zu kleine oder zu dicht benachbarte interaktive Ziele. Der Score für Tap Targets liegt unter 100.
+
+**Warum ist es relevant?** Kleine Ziele lassen sich auf Touchscreens schwer treffen. Nutzer können versehentlich das benachbarte Bedienelement auslösen.
+
+**Wie beheben?** Vergrößern Sie interaktive Ziele auf mindestens 24 mal 24 CSS-Pixel oder schaffen Sie ausreichenden Abstand zu benachbarten Zielen.
+
+## English
+
+**What does it mean?** Lighthouse reports interactive targets that are too small or too close together. The Tap Targets score is below 100.
+
+**Why does it matter?** Small targets are hard to hit on touchscreens and users may activate an adjacent control by mistake.
+
+**How to fix** Make interactive targets at least 24 by 24 CSS pixels or leave sufficient space between neighboring targets.
```

#### `docs/issues/mobile-unsupported-plugins.md`

```diff
diff --git a/docs/issues/mobile-unsupported-plugins.md b/docs/issues/mobile-unsupported-plugins.md
new file mode 100644
index 00000000..568e1703
--- /dev/null
+++ b/docs/issues/mobile-unsupported-plugins.md
@@ -0,0 +1,19 @@
+# Mobile: Unsupported Plugins
+
+Typ: Issue (T-I2) · Priorität: High (P-H3)
+
+## Deutsch
+
+**Was bedeutet das?** Die Seite enthält ein nicht unterstütztes Browser-Plugin oder das Lighthouse-Audit Plugins meldet einen Score unter 100.
+
+**Warum ist es relevant?** Flash, Silverlight und Java-Applets werden auf mobilen Geräten nicht ausgeführt. Inhalte und Funktionen bleiben dadurch unzugänglich.
+
+**Wie beheben?** Ersetzen Sie das Plugin durch HTML, CSS und moderne Browser-APIs und stellen Sie wesentliche Inhalte ohne Plugin bereit.
+
+## English
+
+**What does it mean?** The page contains an unsupported browser plugin or the Lighthouse Plugins audit reports a score below 100.
+
+**Why does it matter?** Flash, Silverlight and Java applets do not run on mobile devices, leaving their content and functionality inaccessible.
+
+**How to fix** Replace the plugin with HTML, CSS and current browser APIs and make essential content available without plugins.
```

#### `docs/issues/mobile-viewport-not-set.md`

```diff
diff --git a/docs/issues/mobile-viewport-not-set.md b/docs/issues/mobile-viewport-not-set.md
new file mode 100644
index 00000000..82375ad2
--- /dev/null
+++ b/docs/issues/mobile-viewport-not-set.md
@@ -0,0 +1,19 @@
+# Mobile: Viewport Not Set
+
+Typ: Issue (T-I2) · Priorität: High (P-H3)
+
+## Deutsch
+
+**Was bedeutet das?** Der Seite fehlt eine wirksame Viewport-Angabe für die mobile Darstellung. Lighthouse meldet dafür einen Score unter 100.
+
+**Warum ist es relevant?** Ohne passenden Viewport wird das Desktop-Layout verkleinert. Inhalte und Bedienelemente können auf kleinen Geräten unbenutzbar werden.
+
+**Wie beheben?** Setzen Sie im Head ein Viewport-Meta mit width=device-width und prüfen Sie das Layout auf einem schmalen Bildschirm.
+
+## English
+
+**What does it mean?** The page has no effective viewport declaration for mobile display. Lighthouse reports a score below 100 for this audit.
+
+**Why does it matter?** Without an appropriate viewport the desktop layout is scaled down, which can make content and controls unusable on small devices.
+
+**How to fix** Add a viewport meta tag with width=device-width in the head and check the layout on a narrow screen.
```

## Stub-Ablösung (K2-60) und Erwartungsänderungen (K2-63)

- `crates/os-engine/f05a-stubs.toml`: nur die zwei Beschreibungen von Mobile::tab (M2-D23 und ursprünglicher M2-F02-Eintrag) entfernt; TECH-231 ersetzt diesen eigenen Stub.
- `crates/os-analyze/tests/filters_registry.rs`: Stubzahl 5 → 4 (K2-60, TECH-231); erwartete Quellenliste um Mobile ergänzt (K2-63, TECH-230/231). Nur Erwartungen, unveränderte Testlogik.

### `crates/os-engine/f05a-stubs.toml`

```diff
diff --git a/crates/os-engine/f05a-stubs.toml b/crates/os-engine/f05a-stubs.toml
index 219f36ca..b159b7e5 100644
--- a/crates/os-engine/f05a-stubs.toml
+++ b/crates/os-engine/f05a-stubs.toml
@@ -41,6 +40,0 @@ signature = "pub fn tab() -> TabDef"
-[[stub]]
-owner = "M2-D23"
-file = "crates/os-analyze/src/tabs/mobile.rs"
-function = "tab"
-signature = "pub fn tab() -> TabDef"
-
@@ -89,6 +82,0 @@ signature = "pub fn tab() -> TabDef"
-[[stub]]
-owner = "M2-F02"
-file = "crates/os-analyze/src/tabs/mobile.rs"
-function = "tab"
-signature = "pub fn tab() -> TabDef"
-
```

### `crates/os-analyze/tests/filters_registry.rs`

```diff
diff --git a/crates/os-analyze/tests/filters_registry.rs b/crates/os-analyze/tests/filters_registry.rs
index 26e8678d..508e78a1 100644
--- a/crates/os-analyze/tests/filters_registry.rs
+++ b/crates/os-analyze/tests/filters_registry.rs
@@ -291 +291 @@ fn every_tab_is_a_real_definition_or_a_marked_stub() {
-        stubs, 5,
+        stubs, 4,
@@ -449,0 +450 @@ fn dynamic_column_sources_are_registered_per_tab_and_names_are_interned() {
+            "Mobile",
```

## Platzhalter-Ablösung (K2-71)

Nicht benötigt. Die Integration erfolgt durch additive Erweiterungspunkte und die eigene Stub-Ablösung nach K2-60.

## Umsetzungscommits

- `9ca79dec` – Parserfakten und eigene Fixtures.
- `0de6ef41` – Mobile-Audits, Quellen und Metrikauswahl.
- `2ed77475` – Mobile-Tab, Filter, Issue-Katalog, Tests und Dokumentations-Goldens.
- `d2d3e824` – Mobil-Alternativen aus bereits vorhandenen Crawl-Links.

Alle Commits enthalten den geforderten Co-Authored-By-Trailer. Der abschließende Berichtscommit wird in der Abgabe genannt.
