# M2-I06 – API-Daten im Crawl

Ausführung: macOS arm64, Branch `m2/i06`. Worktree zu Beginn sauber. Clean-Room: ausschließlich Repository-Spec und bestehender OpenSpider-Code; kein Referenzprodukt eingesehen.

## Umsetzung

- INTEG-012 / CRAWL-037: APIs-Modus ohne Zielseiten-/robots.txt-Requests; deduplizierte Eingaben, lokale Dateien und verschachtelte Sitemap-Quellen; GA4, Search Analytics, URL Inspection, PSI und Linkmetriken; kein Start ohne mindestens eine verbundene ausgewählte Integration.
- INTEG-013: Internal übernimmt die Definitionen von Custom Search, Custom JavaScript, GA4, GSC und PSI. Vorhandene Internal-Quellen für Custom Extraction und Linkmetriken bleiben maßgeblich; keine doppelten Linkmetriken. Ohne konfigurierte bzw. gespeicherte Integration keine zusätzlichen leeren API-Spalten. Internal-CSV enthält die Daten.
- INTEG-015 / OUT-099: getrennte GA4-/GSC-Optionen `crawl_new_urls` (Default false); gleicher Host, bestehende Include-/Exclude- und robots-Prüfungen der Frontier. Orphan-Filter erfordern Option und Crawl-Analyse. Report vereinigt GA, GSC und Sitemap einschließlich aller sieben Quellkombinationen; Normalisierung entfernt Fragmente/Standardports, erhält Protokoll- und Slash-Unterschiede; rohe API-Herkunft wird getrennt von der toleranteren Metrikzuordnung gespeichert; bekannte verlinkte URLs werden ausgeschlossen. Bestehende Sitemap-Folge-Orphans bleiben erhalten.
- INTEG-017 / OUT-201: CLI-Flags und Aufgaben-Auswahl aktivieren dieselbe Quellen-Laufzeit. OAuth-Konto, Analytics-Konto, Property und Stream werden berücksichtigt; GA4-Property-Zugehörigkeit zum gewählten Analytics-Konto wird geprüft. Fehlende Zugangsdaten warnen, `--require-apis` verweigert den Start. Umgebungs-Schlüssel werden ausschließlich in einer flüchtigen SecretStore-Sicht verwendet. API-Ergebnisse sind vor Abschluss-Hook/Analyse/Export gespeichert.

## Abgrenzung und offene Fragen

- OUT-208 (`--mode apis`) gehört laut Paketplan M2-L02; hier steht der gemeinsame Ablauf für dessen CLI-Anbindung und der vorhandene `crawl.start`-Einstieg.
- KI-Prompts/Embeddings sind im M2-Plan M4 zugeordnet und in diesem Branch noch keine ausführbare Quelle. Das KI-Teil-Kriterium von INTEG-012 (`No content – APIs mode` / URL-Prompts) bleibt bei deren Integration umzusetzen; hier wird kein KI-Lauf vorgetäuscht.
- Custom JavaScript ist in diesem Basisstand noch der D22-Stub. Internal fragt dessen vorhandene `column_source()` ab; konkrete Snippet-Spalten erscheinen mit D22.
- Keine zusätzliche Spec-Klärung erforderlich; die vorstehenden Folgepakete bleiben zuständig.
- Kein Push, Merge oder PR; deshalb kein CI-Lauf und keine CI-Matrix-Abnahme. Kein Frontend geändert.

## Prüfungen

- `cargo fmt --all --check`: grün.
- `cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings`: grün.
- `cargo test --workspace --exclude openspider-desktop --locked`: grün auf dem abschließenden Code-Stand `45c20b16`; 3.567 bestanden, 0 fehlgeschlagen, 67 bestehende Ignores, 470 Test-/Doc-Test-Targets; Laufzeit 1.645,98 s. Der vorherige vollständige Lauf war ebenfalls grün.
- Neue Spec-Regressionen: 6 `crawl_glue_lifecycle`-, 4 `tab_internal_api`- und 2 Orphan-Report-Tests; alle grün, mit Spec-ID und lokalen Fixtures/Mocks, keine neuen Ignores. OUT-201 [P1] prüft Auswahl, fehlende Pflicht-Zugangsdaten, genaue Konto-/Property-/Stream-Requests und Umgebungs-Schlüssel; OUT-099 [P1] prüft alle sieben Quellenkombinationen, Link-Ausschluss sowie URL-Normalisierung einschließlich Protokoll-/Slash-Unterschieden.
- Gezielte Regression: 69 os-export-Unit-Tests, 33 os-cli-Unit-Tests, 14 CLI-Basistests und 4 neue Internal/API-Tests grün; Filterregister und GA4/GSC-Regressionen ebenfalls grün.
- CLI-Probe gegen lokalen HTTP-Server mit leerem MemorySecretStore: `--require-apis` → Exit 2 / 0 Ziel-Requests; ohne Pflichtschalter → Exit 0 / 2 Requests (robots + Start); `--strict` → Exit 4 / 2 Requests.
- `cargo deny check licenses bans sources`: grün; keine neuen Abhängigkeiten.
- `scripts/check-neutral.sh`: grün.
- `python3 docs/plan/M2-check.py`: grün (111 Pakete, 590 Spec-IDs, Ausgabe `OK`).
- Keine Änderungen unter `apps/desktop/src`; npm-Checks daher nicht erforderlich.
- Fixture-Datei unter eigenem Besitz: `crates/os-integrations/src/crawl_glue/apis-mode-list.txt`; weitere lokale Szenarien direkt in den eigenen Tests.


## Fremdergänzungen (K2-61)

Alle zusätzlichen Felder sind per bestehendem `serde(default)`/`Default` rückwärtskompatibel. Keine neue Abhängigkeit oder Migration. Das Schema-Golden wurde ausschließlich für die neuen Felder regeneriert.

### `crates/os-model/src/config/v2.rs`

INTEG-015 / OUT-201: zwei Opt-in-Schalter und optionales Analytics-Konto.

```diff
diff --git a/crates/os-model/src/config/v2.rs b/crates/os-model/src/config/v2.rs
index 964e9445..a0e9eb80 100644
--- a/crates/os-model/src/config/v2.rs
+++ b/crates/os-model/src/config/v2.rs
@@ -636,6 +636,10 @@ pub struct DateRangeConfig {
 #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
 #[serde(default)]
 pub struct Ga4Config {
+    /// Selected GA4 Analytics account (OUT-201), distinct from the OAuth account.
+    pub analytics_account: Option<String>,
+    /// Crawl same-host URLs discovered in this API (INTEG-015); disabled by default.
+    pub crawl_new_urls: bool,
     /// Quelle beim Crawl abfragen.
     pub enabled: bool,
     /// Alternative Basis-URL (nur Debug-/Testbuilds oder `--allow-api-override`, INTEG-019).
@@ -664,6 +668,8 @@ pub struct Ga4Config {
 #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
 #[serde(default)]
 pub struct GscConfig {
+    /// Crawl same-host URLs discovered in this API (INTEG-015); disabled by default.
+    pub crawl_new_urls: bool,
     /// Quelle beim Crawl abfragen.
     pub enabled: bool,
     /// Alternative Basis-URL (INTEG-019).
```

### `docs/spec/schemas/osconfig.schema.json`

INTEG-015 / OUT-201: generiertes Schema-Golden ausschließlich für diese neuen Felder.

```diff
diff --git a/docs/spec/schemas/osconfig.schema.json b/docs/spec/schemas/osconfig.schema.json
index afc1a9bf..e7a38b56 100644
--- a/docs/spec/schemas/osconfig.schema.json
+++ b/docs/spec/schemas/osconfig.schema.json
@@ -193,7 +193,9 @@
           "$ref": "#/$defs/Ga4Config",
           "default": {
             "account": null,
+            "analytics_account": null,
             "base_url": null,
+            "crawl_new_urls": false,
             "date_range": {
               "from": null,
               "preset": "last_30_days",
@@ -224,6 +226,7 @@
           "default": {
             "account": null,
             "base_url": null,
+            "crawl_new_urls": false,
             "date_range": {
               "from": null,
               "preset": "last_30_days",
@@ -1135,6 +1138,14 @@
             "null"
           ]
         },
+        "analytics_account": {
+          "default": null,
+          "description": "Selected GA4 Analytics account (OUT-201), distinct from the OAuth account.",
+          "type": [
+            "string",
+            "null"
+          ]
+        },
         "base_url": {
           "default": null,
           "description": "Alternative Basis-URL (nur Debug-/Testbuilds oder `--allow-api-override`, INTEG-019).",
@@ -1143,6 +1154,11 @@
             "null"
           ]
         },
+        "crawl_new_urls": {
+          "default": false,
+          "description": "Crawl same-host URLs discovered in this API (INTEG-015); disabled by default.",
+          "type": "boolean"
+        },
         "date_range": {
           "$ref": "#/$defs/DateRangeConfig",
           "default": {
@@ -1257,6 +1273,11 @@
             "null"
           ]
         },
+        "crawl_new_urls": {
+          "default": false,
+          "description": "Crawl same-host URLs discovered in this API (INTEG-015); disabled by default.",
+          "type": "boolean"
+        },
         "date_range": {
           "$ref": "#/$defs/DateRangeConfig",
           "default": {
@@ -3422,7 +3443,9 @@
         },
         "ga4": {
           "account": null,
+          "analytics_account": null,
           "base_url": null,
+          "crawl_new_urls": false,
           "date_range": {
             "from": null,
             "preset": "last_30_days",
@@ -3449,6 +3472,7 @@
         "gsc": {
           "account": null,
           "base_url": null,
+          "crawl_new_urls": false,
           "date_range": {
             "from": null,
             "preset": "last_30_days",
```

### `crates/os-fetch/src/http.rs`

INTEG-017: zusätzliche Default-Methode liefert optional den Netzwerk-Snapshot.

```diff
diff --git a/crates/os-fetch/src/http.rs b/crates/os-fetch/src/http.rs
index 6d7cb8c7..01151489 100644
--- a/crates/os-fetch/src/http.rs
+++ b/crates/os-fetch/src/http.rs
@@ -15,6 +15,11 @@ pub use url::Url as FetchUrl;

 /// Abruf eines einzelnen URL-Objekts. Redirects verfolgt der Aufrufer (die Engine), nicht der Fetcher.
 pub trait Fetch: Send + Sync + 'static {
+    /// Network snapshot shared with crawl-bound API clients (INTEG-017).
+    fn network_settings(&self) -> Option<os_model::settings::NetworkSettings> {
+        None
+    }
+
     /// Optional policy denial before and after robots checks (AGENT-020).
     /// Default leaves existing fetchers unchanged. A denial contains no network response.
     fn target_denial<'a>(
```

### `crates/os-fetch/src/client.rs`

INTEG-017: zusätzliche Implementierung gibt den vorhandenen Snapshot zurück.

```diff
diff --git a/crates/os-fetch/src/client.rs b/crates/os-fetch/src/client.rs
index e859982d..40620f90 100644
--- a/crates/os-fetch/src/client.rs
+++ b/crates/os-fetch/src/client.rs
@@ -887,6 +887,9 @@ impl HttpFetcher {
     }
 }
 impl Fetch for HttpFetcher {
+    fn network_settings(&self) -> Option<os_model::settings::NetworkSettings> {
+        Some(self.cfg.network.clone())
+    }
     fn fetch(&self, req: FetchRequest) -> Pin<Box<dyn Future<Output = FetchResult> + Send + '_>> {
         Box::pin(self.fetch_impl(req))
     }
```

### `crates/os-engine/src/crawl.rs`

INTEG-015/017: zusätzliche API-Initialisierung, Frontier-Aufnahme und Abschlussbarriere vor dem bestehenden Hook; zusätzlicher interner Kontext und Helfer.

```diff
diff --git a/crates/os-engine/src/crawl.rs b/crates/os-engine/src/crawl.rs
index e9170cbd..2a599b9f 100644
--- a/crates/os-engine/src/crawl.rs
+++ b/crates/os-engine/src/crawl.rs
@@ -678,6 +678,12 @@ pub(crate) fn launch<F: Fetch>(setup: Setup<F>) -> CrawlHandle {
         paused_total,
     );
     tracker.track_file(&file);
+    let api_ctx = os_integrations::ApiCtx {
+        cfg: (*cfg).clone(),
+        secrets: os_integrations::crawl_glue::environment_view_for(deps.secrets.clone(), &cfg),
+        network: fetcher.network_settings().unwrap_or_default(),
+        allow_api_override: false,
+    };
     let (sh, control) = make_shared(SharedInit {
         fetcher,
         cfg,
@@ -712,6 +718,7 @@ pub(crate) fn launch<F: Fetch>(setup: Setup<F>) -> CrawlHandle {
         progress_tx,
         crawl_id,
         file,
+        api_ctx,
         hook: deps.on_finished.clone(),
         seeds,
         clock: deps.clock.clone(),
@@ -727,6 +734,7 @@ pub(crate) fn launch<F: Fetch>(setup: Setup<F>) -> CrawlHandle {
 }

 struct RunParts<F: Fetch> {
+    api_ctx: os_integrations::ApiCtx,
     sh: Arc<Shared<F>>,
     writer: WriterThread,
     progress_tx: watch::Sender<Progress>,
@@ -786,6 +794,7 @@ fn rule_events<F: Fetch>(sh: &Shared<F>, batch: &mut WriteBatch) {

 async fn run<F: Fetch>(parts: RunParts<F>) -> Result<CrawlSummary, EngineError> {
     let RunParts {
+        api_ctx,
         sh,
         writer,
         progress_tx,
@@ -806,6 +815,9 @@ async fn run<F: Fetch>(parts: RunParts<F>) -> Result<CrawlSummary, EngineError>
         }
     }

+    // INTEG-015/017: sitemap seeds are known; workers still use the regular frontier/robots gate.
+    let api_run = initialize_apis(&file, api_ctx, clock.clone(), &writer, &sh).await?;
+
     let mut set: JoinSet<()> = JoinSet::new();
     let mut progress_tick = interval(PROGRESS_INTERVAL);
     progress_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
@@ -1039,6 +1051,20 @@ async fn run<F: Fetch>(parts: RunParts<F>) -> Result<CrawlSummary, EngineError>
         end_reason: reason,
         crawled,
     };
+    if state == CrawlState::Completed {
+        if let Some(api_run) = api_run {
+            let path = summary.file.clone();
+            let runtime = tokio::runtime::Handle::current();
+            tokio::task::spawn_blocking(move || {
+                let file = CrawlFile::open(&path, os_store::OpenMode::ReadWrite)?;
+                runtime
+                    .block_on(api_run.collect(&file))
+                    .map_err(|e| EngineError::Invalid(e.public_message()))
+            })
+            .await
+            .map_err(|e| EngineError::Join(e.to_string()))??;
+        }
+    }
     // CRAWL-141: Hook nach regulärem Ende, Limit oder Stopp – nach dem Checkpoint, vor der Auflösung von
     // `wait()`; nie bei Pause oder Fehler.
     if state == CrawlState::Completed {
@@ -1051,3 +1077,80 @@ async fn run<F: Fetch>(parts: RunParts<F>) -> Result<CrawlSummary, EngineError>
     }
     Ok(summary)
 }
+
+// K2-61: API work runs off the asynchronous worker threads; SQLite connections stay on that thread.
+async fn initialize_apis<F: Fetch>(
+    path: &Path,
+    ctx: os_integrations::ApiCtx,
+    clock: Arc<dyn Clock>,
+    writer: &WriterThread,
+    sh: &Arc<Shared<F>>,
+) -> Result<Option<os_integrations::crawl_glue::ApiRun>, EngineError> {
+    if os_integrations::crawl_glue::enabled(&ctx.cfg).is_empty() {
+        return Ok(None);
+    }
+    writer
+        .flush()
+        .await
+        .map_err(|_| EngineError::Join("API writer flush failed".into()))?;
+    let path = path.to_path_buf();
+    let runtime = tokio::runtime::Handle::current();
+    let (run, candidates) = tokio::task::spawn_blocking(move || {
+        let mut run = os_integrations::crawl_glue::ApiRun::new(
+            ctx,
+            clock,
+            path.with_extension("api-quotas.json"),
+        );
+        for warning in run
+            .validate(false)
+            .map_err(|e| EngineError::Invalid(e.to_string()))?
+        {
+            tracing::warn!("{warning}");
+        }
+        let file = CrawlFile::open(&path, os_store::OpenMode::ReadWrite)?;
+        let candidates = match runtime.block_on(run.discover(&file)) {
+            Ok(urls) => urls,
+            Err(error) => {
+                tracing::warn!(message = %error.public_message(), "API discovery failed");
+                Vec::new()
+            }
+        };
+        Ok::<_, EngineError>((run, candidates))
+    })
+    .await
+    .map_err(|e| EngineError::Join(e.to_string()))??;
+    let mut batch = WriteBatch::default();
+    {
+        let mut core = sh.lock_core();
+        for address in candidates {
+            let Ok(url) = Url::parse(&address) else {
+                continue;
+            };
+            let norm = os_url::normalize(&url);
+            core.discover(
+                &Cand {
+                    norm: &norm,
+                    internal: true,
+                    kind: DiscoveryKind::Link,
+                    store: true,
+                    crawl: true,
+                    reason: None,
+                    resource_like: false,
+                    apply_rules: true,
+                    purpose: Purpose::Page,
+                    source: None,
+                },
+                Some(0),
+                0,
+                0,
+                &sh.rules,
+                &mut batch,
+            );
+        }
+    }
+    sh.writer
+        .send(batch)
+        .await
+        .map_err(|_| EngineError::Join("API frontier write failed".into()))?;
+    Ok(Some(run))
+}
```

### `crates/os-engine/src/list_input.rs`

INTEG-012: zusätzliche Funktion api_items zur Auflösung von Dateien und Sitemap-Listen ohne Seiten-Crawl.

```diff
diff --git a/crates/os-engine/src/list_input.rs b/crates/os-engine/src/list_input.rs
index 9f3e03bd..34f245ae 100644
--- a/crates/os-engine/src/list_input.rs
+++ b/crates/os-engine/src/list_input.rs
@@ -430,6 +430,50 @@ pub fn list_config(mut cfg: CrawlConfig) -> CrawlConfig {
     cfg
 }

+/// Expand files and sitemap sources for an API-only run, without requesting target pages (INTEG-012).
+pub async fn api_items<F: os_fetch::Fetch>(
+    items: &[String],
+    cfg: &CrawlConfig,
+    fetcher: &F,
+) -> Result<Vec<String>, EngineError> {
+    let mut expanded = Vec::new();
+    for item in items {
+        if !item.contains("://") && Path::new(item).is_file() {
+            expanded.extend(read_items(Path::new(item))?);
+        } else {
+            expanded.push(item.clone());
+        }
+    }
+    let planned = plan(&expanded, cfg)?;
+    let mut seeds = Vec::new();
+    let mut output = Vec::new();
+    for entry in planned.entries {
+        if let Some(url) = entry.url {
+            match entry.outcome {
+                ListOutcome::Ok => output.push(url.encoded),
+                ListOutcome::Sitemap => seeds.push(crate::sitemaps::SitemapSeed {
+                    url: url.url,
+                    found_via: "list".into(),
+                }),
+                _ => {}
+            }
+        }
+    }
+    if !seeds.is_empty() {
+        for map in crate::sitemaps::ingest(fetcher, seeds, 3).await {
+            let Some(doc) = map.doc else {
+                return Err(EngineError::Invalid(
+                    "API-Listen-Sitemap konnte nicht gelesen werden".into(),
+                ));
+            };
+            if doc.kind == os_model::SitemapKind::Urlset {
+                output.extend(doc.locs.into_iter().filter_map(|loc| loc.url));
+            }
+        }
+    }
+    Ok(output)
+}
+
 #[cfg(test)]
 mod tests {
     use super::*;
```

### `crates/os-app/src/commands/api.rs`

INTEG-012/017: zusätzlicher Kontext-Getter für Sitzungsschlüssel und App-Netzwerk.

```diff
diff --git a/crates/os-app/src/commands/api.rs b/crates/os-app/src/commands/api.rs
index 0ac47e17..00891357 100644
--- a/crates/os-app/src/commands/api.rs
+++ b/crates/os-app/src/commands/api.rs
@@ -1827,3 +1827,8 @@ pub fn register(r: &mut Registry) {
 pub fn export_credentials(app: &App) -> Arc<Credentials> {
     app.ext::<ApiState>().credentials(app).clone()
 }
+
+/// Context for the APIs-only lifecycle, preserving session credentials and app network settings.
+pub(crate) fn crawl_context(app: &App, cfg: CrawlConfig) -> ApiCtx {
+    api_ctx(app, cfg)
+}
```

### `crates/os-cli/src/plan.rs`

INTEG-017 / OUT-201: zusätzliche Laufzeitfelder api_secrets (Default None) und require_apis (Default false), zusätzliche API-Konfigurationsübernahme. Die Secret-Vorprüfung erfolgt im Ausführungspfad vor dem Crawl, damit der übergebene Store einer Aufgabe berücksichtigt wird.

```diff
diff --git a/crates/os-cli/src/plan.rs b/crates/os-cli/src/plan.rs
index 6b5eb1f5..a6414706 100644
--- a/crates/os-cli/src/plan.rs
+++ b/crates/os-cli/src/plan.rs
@@ -54,6 +54,10 @@ pub struct BatchSettings {
 /// Ablaufplan nach der Validierung.
 #[derive(Debug)]
 pub struct Plan {
+    /// Optional API credential view supplied by a scheduled host (default None).
+    pub api_secrets: Option<os_integrations::crawl_glue::SecretSnapshot>,
+    /// Require every requested API; default false.
+    pub require_apis: bool,
     /// Stapelmodus der Exporte (geplante Aufgaben); `None` = einzeln wie auf der Kommandozeile.
     pub batch: Option<BatchSettings>,
     /// Optionaler Segmentkontext; ohne Flag bleibt der Export ungefiltert.
@@ -154,6 +158,8 @@ fn build_config(
         os_fetch::resolve::ResolveRule::parse(rule).map_err(CliError::usage)?;
         loaded.config.network.resolve.push(rule.clone());
     }
+    // OUT-201 / INTEG-017: apply selections before validating and before creating a crawl.
+    apply_api_options(&mut loaded.config, args)?;
     let report = loaded.validate();
     for w in &report.warnings {
         rep.warn(format!("Konfiguration: {w}"));
@@ -493,6 +499,8 @@ fn build_impl(
         }
     }
     Ok(Plan {
+        api_secrets: None,
+        require_apis: args.require_apis,
         batch,
         segment: args.segment.clone(),
         source,
@@ -512,6 +520,20 @@ fn build_impl(
     })
 }

+fn apply_api_options(cfg: &mut CrawlConfig, args: &RunArgs) -> Result<(), CliError> {
+    os_integrations::crawl_glue::configure_google(
+        cfg,
+        args.use_google_analytics_4.as_deref(),
+        args.use_google_search_console.as_deref(),
+    )
+    .map_err(|e| CliError::usage(e.to_string()))?;
+    cfg.api.psi.enabled |= args.use_pagespeed;
+    cfg.api.ahrefs.enabled |= args.use_ahrefs;
+    cfg.api.majestic.enabled |= args.use_majestic;
+    cfg.api.moz.enabled |= args.use_mozscape;
+    Ok(())
+}
+
 #[cfg(test)]
 mod tests {
     use super::*;
```

Zusätzliche Aufrufe in gemeinsam mit K2-71 geänderten Dateien (vollständige Diffs folgen unter K2-71):

`crates/os-app/src/session/control.rs` – INTEG-012, Kontext/Prüfung vor Dateianlage und zusätzliche Listenauflösung:

```diff
+    let api_ctx = crate::commands::api::crawl_context(app, cfg.clone());
+    os_integrations::crawl_glue::validate_apis(&api_ctx)
+        .map_err(|e| CmdError::invalid_input("err-api-disconnected", "source").with_detail(e.to_string()))?;
+    let api_clock = app.config().clock.clone();
+    let items = os_engine::list_input::api_items(&items, &cfg, fetcher(app, &cfg)?.as_ref())
+        .await
+        .map_err(engine_error)?;
```

`crates/os-cli/src/run_task.rs` – INTEG-017/OUT-201, zusätzliche Auswahl der Aufgaben-APIs und Weitergabe des injizierten SecretStores (vollständiger Diff unten):

```diff
+    for source in &task.apis {
+        args.set.push(format!("api.{source}.enabled=true"));
+    }
+    plan.api_secrets = Some(os_integrations::crawl_glue::SecretSnapshot(ctx.secrets.clone()));
```


### `crates/os-cli/src/exec.rs`

INTEG-017 / OUT-201 / C13: zusätzliche Vorprüfung vor Crawl-Start und zusätzlicher deps_with_apis-Initialisierungshelfer. Der bestehende deps-Helfer und HTTP-Fetcher bleiben als innere Aufrufe erhalten; die neue API-Laufzeit erhält den Store aus dem Laufplan statt eines fest angenommenen OS-Stores.

```diff
diff --git a/crates/os-cli/src/exec.rs b/crates/os-cli/src/exec.rs
index 4c0bc3f1..832695ea 100644
--- a/crates/os-cli/src/exec.rs
+++ b/crates/os-cli/src/exec.rs
@@ -99,6 +99,11 @@ async fn run(plan: &Plan, rep: &Arc<Reporter>, signals: &Signals) -> Result<Outc
         ));
     }

+    // INTEG-017 / OUT-201: credentials are validated against the host's store before any crawl.
+    if plan.resume || !matches!(plan.source, Source::Load { .. }) {
+        preflight_apis(plan, rep)?;
+    }
+
     let crawled = match (&plan.source, plan.resume) {
         (Source::Load { path }, false) => open_only(path, rep)?,
         _ => {
@@ -259,6 +264,32 @@ fn deps(id: Option<CrawlId>, hook: FinishedHook) -> EngineDeps {
     }
 }

+fn api_secrets(plan: &Plan) -> Arc<dyn SecretStore> {
+    plan.api_secrets
+        .as_ref()
+        .map(|s| s.0.clone())
+        .unwrap_or_else(|| os_fetch::secret_store_from_env())
+}
+fn deps_with_apis(plan: &Plan, id: Option<CrawlId>, hook: FinishedHook) -> EngineDeps {
+    let mut value = deps(id, hook);
+    value.secrets = api_secrets(plan);
+    value
+}
+fn preflight_apis(plan: &Plan, rep: &Reporter) -> Result<(), CliError> {
+    let ctx = os_integrations::ApiCtx {
+        cfg: plan.config.clone(),
+        secrets: os_integrations::crawl_glue::environment_view_for(api_secrets(plan), &plan.config),
+        network: Default::default(),
+        allow_api_override: false,
+    };
+    for warning in os_integrations::crawl_glue::preflight(&ctx, plan.require_apis)
+        .map_err(|e| CliError::usage(e.to_string()))?
+    {
+        rep.warn(warning);
+    }
+    Ok(())
+}
+
 /// Hook am Crawl-Ende (CRAWL-141): Die Crawl-Analyse läuft sofort danach, vor der Auflösung von `wait()`.
 fn analysis_hook(rep: &Arc<Reporter>, signals: &Signals) -> FinishedHook {
     let rep = Arc::clone(rep);
@@ -328,7 +359,7 @@ async fn crawl_phase(
                 fetcher,
                 path.clone(),
                 plan.resume_config.clone(),
-                deps(None, hook),
+                deps_with_apis(plan, None, hook),
             )
             .await
             .map_err(|e| engine_error(e, true))?;
@@ -361,7 +392,7 @@ async fn crawl_phase(
                 project: plan.project.clone(),
                 name: plan.name.clone(),
             };
-            let handle = start_crawl(fetcher, request, deps(Some(id), hook))
+            let handle = start_crawl(fetcher, request, deps_with_apis(plan, Some(id), hook))
                 .await
                 .map_err(|e| engine_error(e, false))?;
             (handle, path, id)
```

### `crates/os-model/tests/v2_support/mod.rs`

INTEG-015 / OUT-201: Fixture-Builder ausschließlich um die drei eigenen neuen Felder mit Nicht-Default-Werten ergänzt. Die vorhandene Rundreise-/Schema-Testlogik bleibt unverändert.

```diff
diff --git a/crates/os-model/tests/v2_support/mod.rs b/crates/os-model/tests/v2_support/mod.rs
index 704e3eb2..ebfb518c 100644
--- a/crates/os-model/tests/v2_support/mod.rs
+++ b/crates/os-model/tests/v2_support/mod.rs
@@ -169,6 +169,8 @@ pub fn change_all_v2_fields(c: &mut CrawlConfig) {
     };
     c.api = ApiConfig {
         ga4: Ga4Config {
+            analytics_account: Some("accounts/123".into()),
+            crawl_new_urls: true,
             enabled: true,
             base_url: Some("http://127.0.0.1:9/ga4".into()),
             property: Some("properties/123".into()),
@@ -183,6 +185,7 @@ pub fn change_all_v2_fields(c: &mut CrawlConfig) {
             ..Default::default()
         },
         gsc: GscConfig {
+            crawl_new_urls: true,
             enabled: true,
             base_url: Some("http://127.0.0.1:9/gsc".into()),
             site: Some("sc-domain:a.test".into()),
```

### `tests/fixtures/config/default.osconfig`

INTEG-015: Default-Golden mit UPDATE_GOLDEN=1 neu erzeugt; der Diff enthält ausschließlich die beiden neuen false-Schalter. Optionale Analytics-Konto-Referenz bleibt als None im TOML ausgelassen.

```diff
diff --git a/tests/fixtures/config/default.osconfig b/tests/fixtures/config/default.osconfig
index fc5f4a72..e6c13d82 100644
--- a/tests/fixtures/config/default.osconfig
+++ b/tests/fixtures/config/default.osconfig
@@ -406,6 +406,7 @@ respect_hsts = true
 ignore_certificate_errors = false

 [api.ga4]
+crawl_new_urls = false
 enabled = false
 metrics = []

@@ -422,6 +423,7 @@ ignore_trailing_slash = true
 ignore_case = true

 [api.gsc]
+crawl_new_urls = false
 enabled = false
 search_type = "web"
 url_inspection = false
```

## Platzhalter-Ablösung (K2-71)

### `crates/os-cli/src/args.rs`

INTEG-017 / OUT-201: genau die sieben API-Optionen aus der Unsupported-Liste und aus den entsprechenden Hilfetext-Markierungen entfernt.

```diff
diff --git a/crates/os-cli/src/args.rs b/crates/os-cli/src/args.rs
index 6417ef62..c65b0b4d 100644
--- a/crates/os-cli/src/args.rs
+++ b/crates/os-cli/src/args.rs
@@ -134,31 +134,31 @@ pub struct RunArgs {
     pub compare_with_previous: bool,

     // ---- APIs ---------------------------------------------------------------------------------
-    /// [noch nicht unterstützt] Google Analytics 4.
+    /// Google Analytics 4.
     #[arg(long, num_args = 0.., value_name = "KONTO ACCOUNT PROPERTY STROM", help_heading = "APIs")]
     pub use_google_analytics_4: Option<Vec<String>>,

-    /// [noch nicht unterstützt] Google Search Console.
+    /// Google Search Console.
     #[arg(long, num_args = 0.., value_name = "KONTO WEBSITE", help_heading = "APIs")]
     pub use_google_search_console: Option<Vec<String>>,

-    /// [noch nicht unterstützt] PageSpeed Insights.
+    /// PageSpeed Insights.
     #[arg(long, help_heading = "APIs")]
     pub use_pagespeed: bool,

-    /// [noch nicht unterstützt] Ahrefs.
+    /// Ahrefs.
     #[arg(long, help_heading = "APIs")]
     pub use_ahrefs: bool,

-    /// [noch nicht unterstützt] Majestic.
+    /// Majestic.
     #[arg(long, help_heading = "APIs")]
     pub use_majestic: bool,

-    /// [noch nicht unterstützt] Moz.
+    /// Moz.
     #[arg(long, help_heading = "APIs")]
     pub use_mozscape: bool,

-    /// [noch nicht unterstützt] Angeforderte Integrationen werden verpflichtend.
+    /// Angeforderte Integrationen werden verpflichtend.
     #[arg(long, help_heading = "APIs")]
     pub require_apis: bool,

@@ -284,7 +284,7 @@ pub fn long_option_names() -> Vec<String> {
 impl RunArgs {
     /// Die noch nicht unterstützten Optionen, die der Aufruf enthält (Namen mit `--`).
     pub fn unsupported(&self) -> Vec<&'static str> {
-        let checks: [(bool, &'static str); 17] = [
+        let checks: [(bool, &'static str); 10] = [
             (self.crawl_google_sheet.is_some(), "--crawl-google-sheet"),
             (
                 self.google_drive_account.is_some(),
@@ -297,19 +297,6 @@ impl RunArgs {
                 "--project-crawl-comparison",
             ),
             (self.compare_with_previous, "--compare-with-previous"),
-            (
-                self.use_google_analytics_4.is_some(),
-                "--use-google-analytics-4",
-            ),
-            (
-                self.use_google_search_console.is_some(),
-                "--use-google-search-console",
-            ),
-            (self.use_pagespeed, "--use-pagespeed"),
-            (self.use_ahrefs, "--use-ahrefs"),
-            (self.use_majestic, "--use-majestic"),
-            (self.use_mozscape, "--use-mozscape"),
-            (self.require_apis, "--require-apis"),
             (self.create_sitemap, "--create-sitemap"),
             (self.create_images_sitemap, "--create-images-sitemap"),
             (self.email_on_complete.is_some(), "--email-on-complete"),
```

### `crates/os-cli/src/run_task.rs`

INTEG-017 / OUT-201: Platzhalter-Warnung über angeblich fehlende Headless-APIs abgelöst; zusätzliche API-Auswahl läuft im Engine-Pfad vor Exporten.

```diff
diff --git a/crates/os-cli/src/run_task.rs b/crates/os-cli/src/run_task.rs
index b3c7d227..30a5ddaf 100644
--- a/crates/os-cli/src/run_task.rs
+++ b/crates/os-cli/src/run_task.rs
@@ -49,13 +49,9 @@ pub const RUN_ID_ENV: &str = "OPENSPIDER_TASK_RUN_ID";
 /// „Completed with warnings“); Tests und spätere Pakete setzen eigene Umsetzungen ein.
 pub trait TaskHooks: Send + Sync {
     /// API-Quellen der Aufgabe (OUT-164): Abruf für den fertigen Crawl; Rückgabe = Warnungen.
-    fn fetch_apis(&self, task: &Task, _crawl: Option<&Path>) -> Vec<String> {
-        task.apis
-            .iter()
-            .map(|api| {
-                format!("API source '{api}' is not available in headless runs of this version")
-            })
-            .collect()
+    fn fetch_apis(&self, _task: &Task, _crawl: Option<&Path>) -> Vec<String> {
+        // INTEG-017: the engine completes these requests before analysis and exports.
+        Vec::new()
     }

     /// Vergleich mit dem vorherigen Crawl des Projekts (OUT-152).
@@ -330,6 +326,10 @@ fn to_run_args(task: &Task, paths: &AppPaths, exports: &ExportPlan, export_dir:
         .iter()
         .map(|(k, v)| format!("{k}={v}"))
         .collect();
+    // INTEG-017: use the same profile settings and lifecycle as interactive/CLI runs.
+    for source in &task.apis {
+        args.set.push(format!("api.{source}.enabled=true"));
+    }
     for (kind, name) in &exports.items {
         let list = vec![escape_name(name)];
         match kind.to_ascii_lowercase().as_str() {
@@ -589,6 +589,9 @@ pub async fn run_in(
         Ok(plan) => plan,
         Err(e) => return finish(&live, RunResult::Failed, e.exit, Some(e.message)),
     };
+    plan.api_secrets = Some(os_integrations::crawl_glue::SecretSnapshot(
+        ctx.secrets.clone(),
+    ));
     if let Some(profile) = task
         .crawl
         .auth_profile
```

### `crates/os-app/src/session/control.rs`

INTEG-012/017: vorhandenen Stub-Aufruf durch die explizite Laufzeit mit App-Kontext ersetzt; zusätzliche Vorprüfung vor Dateianlage und Listenauflösung. Engine erhält für APIs die bestehende Credentials-Sicht (delegiert sonstige Secret-Referenzen unverändert an den Backend-Store).

```diff
diff --git a/crates/os-app/src/session/control.rs b/crates/os-app/src/session/control.rs
index 210da4ed..8b2284dc 100644
--- a/crates/os-app/src/session/control.rs
+++ b/crates/os-app/src/session/control.rs
@@ -31,7 +31,7 @@ pub fn deps(app: &App) -> EngineDeps {
             os_model::PerformanceProfile::MaxSpeed => os_store::StorageProfile::MaxSpeed,
             os_model::PerformanceProfile::MinMemory => os_store::StorageProfile::MinMemory,
         },
-        secrets: app.config().secrets.clone(),
+        secrets: crate::commands::api::crawl_context(app, CrawlConfig::default()).secrets,
         sink_factory: None,
         on_finished: None,
         owner_kind: "app",
@@ -434,6 +434,15 @@ async fn start_apis(
     id: os_model::CrawlId,
     items: Vec<String>,
 ) -> Result<CrawlRef, CmdError> {
+    let api_ctx = crate::commands::api::crawl_context(app, cfg.clone());
+    os_integrations::crawl_glue::validate_apis(&api_ctx).map_err(|e| {
+        CmdError::invalid_input("err-api-disconnected", "source").with_detail(e.to_string())
+    })?;
+    let api_clock = app.config().clock.clone();
+    let items = os_engine::list_input::api_items(&items, &cfg, fetcher(app, &cfg)?.as_ref())
+        .await
+        .map_err(engine_error)?;
+
     let key = CrawlKey(id.to_string());
     let init = os_store::CrawlInit {
         crawl_id: id,
@@ -484,7 +493,9 @@ async fn start_apis(
                     .map_err(|e| internal("API inputs", e.to_string()))?,
             )
             .map_err(|e| crate::runtime::error::store_error(&e))?;
-            let result = runtime.block_on(os_integrations::crawl_glue::start_apis_mode(f, &cfg));
+            let result = runtime.block_on(os_integrations::crawl_glue::start_apis_mode_with(
+                f, api_ctx, api_clock,
+            ));
             f.set_meta(
                 "state",
                 if result.is_ok() {
```

### `crates/os-analyze/src/tabs/analytics.rs`

INTEG-012: starre HTML-Population sperrte API-only-Zeilen; ausschließlich im APIs-Modus geöffnet.

```diff
diff --git a/crates/os-analyze/src/tabs/analytics.rs b/crates/os-analyze/src/tabs/analytics.rs
index 1d4e79aa..b7f6ce57 100644
--- a/crates/os-analyze/src/tabs/analytics.rs
+++ b/crates/os-analyze/src/tabs/analytics.rs
@@ -28,7 +28,7 @@ use crate::defs::{
 };

 /// Grundmenge des Tabs.
-const POPULATION: &str = "u.is_internal = 1 AND u.in_tables = 1 AND u.content_class = 'HTML'";
+const POPULATION: &str = "u.is_internal = 1 AND u.in_tables = 1 AND (u.content_class = 'HTML' OR EXISTS(SELECT 1 FROM crawl_meta WHERE key='source_mode' AND value='apis'))";

 /// Die URL hat mindestens eine GA4-Zeile.
 macro_rules! has_ga {
```

### `crates/os-analyze/src/tabs/pagespeed.rs`

INTEG-012: starre Status-200-/HTML-Population sperrte API-only-Zeilen; ausschließlich im APIs-Modus geöffnet.

```diff
diff --git a/crates/os-analyze/src/tabs/pagespeed.rs b/crates/os-analyze/src/tabs/pagespeed.rs
index 639129d2..fcba4803 100644
--- a/crates/os-analyze/src/tabs/pagespeed.rs
+++ b/crates/os-analyze/src/tabs/pagespeed.rs
@@ -20,7 +20,7 @@ use crate::defs::{

 /// Grundmenge (INTEG-130): interne HTML-URLs mit Status 200 – genau die URLs, die abgefragt werden.
 pub const POPULATION: &str =
-    "u.is_internal = 1 AND u.in_tables = 1 AND u.content_class = 'HTML' AND u.status_code = 200";
+    "u.is_internal = 1 AND u.in_tables = 1 AND ((u.content_class = 'HTML' AND u.status_code = 200) OR EXISTS(SELECT 1 FROM crawl_meta WHERE key='source_mode' AND value='apis'))";

 /// Die URL hat einen jüngeren Abruffehler in `api_errors`.
 const HAS_ERROR: &str =
```

### `crates/os-analyze/src/state.rs`

INTEG-015: zusätzliche Laufzeit-Validierung vor der bestehenden Coverage-Auswertung. API-Orphans benötigen Opt-in und frische Analyse; die kanonischen Live-SQL-Filter und ihre Registermetadaten bleiben erhalten. Die bisherige unbedingte Bereitschaft dieser beiden Filter war mit INTEG-015 unvereinbar. Keine Änderung für andere Filter.

```diff
diff --git a/crates/os-analyze/src/state.rs b/crates/os-analyze/src/state.rs
index 485f3389..5c0b7a15 100644
--- a/crates/os-analyze/src/state.rs
+++ b/crates/os-analyze/src/state.rs
@@ -202,6 +202,17 @@ pub(crate) fn filter_states_for_file(
         .iter()
         .map(|def| filter_state(def, cfg, cfg_json, analysis))
         .collect();
+    // INTEG-015: API orphan filters retain their canonical live-SQL definitions,
+    // while the file-bound state supplies the required configuration/analysis hints.
+    for (def, state) in defs.iter().zip(&mut states) {
+        if state.yields_rows() {
+            if let Some(required) =
+                crate::tabs::dynamic_api::orphan_state(def, tab, cfg, cfg_json, analysis)
+            {
+                *state = required;
+            }
+        }
+    }
     let markers: std::collections::BTreeSet<_> = defs
         .iter()
         .zip(&states)
```

## Erwartungsänderungen (K2-63)

### `crates/os-analyze/tests/tab_analytics.rs`

INTEG-015: bei ausgeschalteter neuer Option statt Orphan-Zahlen/-Zeilen NeedsConfig bzw. leere Zeilen erwarten; nur Assertions geändert.

```diff
diff --git a/crates/os-analyze/tests/tab_analytics.rs b/crates/os-analyze/tests/tab_analytics.rs
index 3c778c55..1cedc875 100644
--- a/crates/os-analyze/tests/tab_analytics.rs
+++ b/crates/os-analyze/tests/tab_analytics.rs
@@ -151,7 +151,14 @@ async fn five_filters_match_the_expected_fixture_counts() {
     let cfg = config();
     let (_dir, fx) = crawl(&cfg).await;
     // Vor der Crawl-Analyse bleibt „Orphan URLs“ leer.
-    assert_eq!(count(&fx, &cfg, "Orphan URLs"), 0);
+    assert_eq!(
+        counts(&fx, &cfg)
+            .into_iter()
+            .find(|(f, ..)| f == "Orphan URLs")
+            .unwrap()
+            .2,
+        FilterState::NeedsConfig("api.ga4.crawl_new_urls".into())
+    );
     analyse(&fx, &cfg);
     for (filter, expected) in EXPECTED_FILTERS {
         assert_eq!(count(&fx, &cfg, filter), *expected, "{filter}");
@@ -162,7 +169,7 @@ async fn five_filters_match_the_expected_fixture_counts() {
         addresses(&fx, &cfg, "Non-Indexable with GA Data"),
         ["/noindex"]
     );
-    assert_eq!(addresses(&fx, &cfg, "Orphan URLs"), ["/orphan"]);
+    assert!(addresses(&fx, &cfg, "Orphan URLs").is_empty());
     let mut above = addresses(&fx, &cfg, "Sessions Above 0");
     above.sort();
     assert_eq!(
@@ -200,10 +207,24 @@ async fn orphan_filter_respects_ignore_non_indexable_but_non_indexable_filter_do
         )
         .unwrap();
     assert!(cfg.issues.ignore_non_indexable);
-    assert_eq!(count(&fx, &cfg, "Orphan URLs"), 0);
+    assert_eq!(
+        counts(&fx, &cfg)
+            .into_iter()
+            .find(|(f, ..)| f == "Orphan URLs")
+            .unwrap()
+            .2,
+        FilterState::NeedsConfig("api.ga4.crawl_new_urls".into())
+    );
     assert_eq!(count(&fx, &cfg, "Non-Indexable with GA Data"), 2);
     cfg.issues.ignore_non_indexable = false;
-    assert_eq!(count(&fx, &cfg, "Orphan URLs"), 1);
+    assert_eq!(
+        counts(&fx, &cfg)
+            .into_iter()
+            .find(|(f, ..)| f == "Orphan URLs")
+            .unwrap()
+            .2,
+        FilterState::NeedsConfig("api.ga4.crawl_new_urls".into())
+    );
     assert_eq!(count(&fx, &cfg, "Non-Indexable with GA Data"), 2);
 }
```

### `crates/os-analyze/tests/tab_search_console.rs`

INTEG-015: ausgeschaltete neue Option liefert keine Orphan-Zeilen; nur erwartete Mengen geändert.

```diff
diff --git a/crates/os-analyze/tests/tab_search_console.rs b/crates/os-analyze/tests/tab_search_console.rs
index 4b3f537f..1644a73a 100644
--- a/crates/os-analyze/tests/tab_search_console.rs
+++ b/crates/os-analyze/tests/tab_search_console.rs
@@ -327,7 +327,7 @@ async fn search_analytics_filters_match_the_expected_sets() {
             "Non-Indexable with Search Analytics Data",
             set(&["/noindex", "/orphan-noindex"]),
         ),
-        ("Orphan URLs", set(&["/orphan"])),
+        ("Orphan URLs", set(&[])),
     ]);
     for (filter, want) in &expected {
         assert_eq!(&f.hits(&cfg, filter), want, "{filter}");
@@ -343,7 +343,11 @@ async fn search_analytics_filters_match_the_expected_sets() {
             .find(|c| c.tab == "Search Console" && c.filter == *filter)
             .unwrap()
             .count;
-        assert_eq!(count, Some(want.len() as u64), "{filter}");
+        assert_eq!(
+            count,
+            (*filter != "Orphan URLs").then_some(want.len() as u64),
+            "{filter}"
+        );
     }
     // Ohne verbundene Search Console sind die Filter nicht anwendbar.
     let mut off = cfg.clone();
@@ -364,12 +368,9 @@ async fn search_analytics_filters_match_the_expected_sets() {
 async fn orphan_filter_respects_ignore_non_indexable() {
     let on = config(true);
     let f = Fixture::crawl(&on).await;
-    assert_eq!(f.hits(&on, "Orphan URLs"), set(&["/orphan"]));
+    assert_eq!(f.hits(&on, "Orphan URLs"), set(&[]));
     let off = config(false);
-    assert_eq!(
-        f.hits(&off, "Orphan URLs"),
-        set(&["/orphan", "/orphan-noindex"])
-    );
+    assert_eq!(f.hits(&off, "Orphan URLs"), set(&[]));
     // Die Spaltenwerte bleiben in „All“ sichtbar; Filter für nicht indexierbare Seiten sind nie eingeschränkt.
     assert!(f.hits(&on, "All").contains("/orphan-noindex"));
     assert!(f
```

### `crates/os-testsite/src/mocks/ga4.rs`

INTEG-015: die einzige eigene Orphan-Erwartung aus der Liste numerischer Default-Zähler entfernt, da sie jetzt einen Konfigurationshinweis liefert; keine Mock-Logik geändert.

```diff
diff --git a/crates/os-testsite/src/mocks/ga4.rs b/crates/os-testsite/src/mocks/ga4.rs
index f2c025d2..78484add 100644
--- a/crates/os-testsite/src/mocks/ga4.rs
+++ b/crates/os-testsite/src/mocks/ga4.rs
@@ -155,7 +155,6 @@ pub const EXPECTED_FILTERS: &[(&str, u64)] = &[
     ("Bounce Rate Above 70%", 1),
     ("No GA Data", 1),
     ("Non-Indexable with GA Data", 1),
-    ("Orphan URLs", 1),
 ];

 impl SiteRow {
```

### `crates/os-analyze/tests/filters_registry.rs`

INTEG-013: genau eine zusätzliche Internal-Spaltenquelle in der erwarteten Registerliste.

```diff
diff --git a/crates/os-analyze/tests/filters_registry.rs b/crates/os-analyze/tests/filters_registry.rs
index 37be6618..fbc72255 100644
--- a/crates/os-analyze/tests/filters_registry.rs
+++ b/crates/os-analyze/tests/filters_registry.rs
@@ -438,6 +438,7 @@ fn dynamic_column_sources_are_registered_per_tab_and_names_are_interned() {
             "Internal",
             "Internal",
             "Internal",
+            "Internal",
             "Link Metrics",
             "PageSpeed",
             "Structured Data"
```

### `crates/os-cli/tests/cli_basics.rs`

INTEG-017 / OUT-201: gültiges --use-pagespeed ohne Schlüssel erwartet Warnung/Exit 0 und Crawl-Requests statt pauschalem Unsupported/Exit 2; nur Assertions und Kommentar angepasst.

```diff
diff --git a/crates/os-cli/tests/cli_basics.rs b/crates/os-cli/tests/cli_basics.rs
index 5a2f8c30..b33a6bea 100644
--- a/crates/os-cli/tests/cli_basics.rs
+++ b/crates/os-cli/tests/cli_basics.rs
@@ -246,14 +246,18 @@ async fn invalid_names_are_rejected_before_any_request_in_under_a_second() {
         assert!(run.stderr.contains(needle), "{fmt}: {}", run.dump());
     }

-    // Nicht unterstützte Google-/API-Optionen enden ebenfalls vor dem Netz mit Exit-Code 2.
-    let run = env.run_async(base(&["--use-pagespeed"])).await;
-    assert_eq!(run.code, 2, "{}", run.dump());
-
     assert_eq!(
         site.total_requests(),
         0,
-        "vor der Validierung darf kein Request erfolgen"
+        "Aufruffehler müssen vor jedem Request enden"
+    );
+    // OUT-201 / INTEG-017: eine optionale API ohne Schlüssel warnt; der Crawl läuft.
+    let run = env.run_async(base(&["--use-pagespeed"])).await;
+    assert_eq!(run.code, 0, "{}", run.dump());
+
+    assert!(
+        site.total_requests() > 0,
+        "der gültige Aufruf mit optionaler API muss crawlen"
     );
     assert!(!env.out().exists(), "kein Ausgabeordner bei Aufruffehlern");
 }
```

### `crates/os-cli/tests/run_task_inproc.rs`

INTEG-017 / OUT-201: die bisherigen synthetischen Legacy-Zugangsdaten sind keine gültigen Credentials für den echten API-Lauf. Erwartungen wechseln von Completed zur spec-gemäßen CompletedWithWarnings-Meldung mit benannter fehlender Integration; Hook-Aufrufe, Eingaben und Testablauf bleiben unverändert.

```diff
diff --git a/crates/os-cli/tests/run_task_inproc.rs b/crates/os-cli/tests/run_task_inproc.rs
index 4b797ddb..5521770d 100644
--- a/crates/os-cli/tests/run_task_inproc.rs
+++ b/crates/os-cli/tests/run_task_inproc.rs
@@ -207,7 +207,11 @@ async fn api_sources_comparison_and_notification_are_handed_to_the_hooks_after_t
         .await;
     assert_eq!(run.exit.code(), 0, "{}", run.summary);
     let record = run.record.unwrap();
-    assert!(common::is_completed(&record), "{record:?}");
+    assert_eq!(
+        record.result,
+        RunResult::CompletedWithWarnings,
+        "{record:?}"
+    );
     assert_eq!(
         calls.apis.lock().unwrap().as_slice(),
         [(vec!["gsc".to_string(), "psi".to_string()], true)]
@@ -215,7 +219,7 @@ async fn api_sources_comparison_and_notification_are_handed_to_the_hooks_after_t
     assert_eq!(*calls.compare.lock().unwrap(), 1);
     let notified = calls.notified.lock().unwrap();
     assert_eq!(notified.len(), 1);
-    assert!(common::is_completed(&notified[0]));
+    assert_eq!(notified[0].result, RunResult::CompletedWithWarnings);
     assert_eq!(notified[0].run_id, record.run_id);
 }

@@ -242,7 +246,7 @@ async fn without_extensions_requested_apis_end_as_completed_with_warnings_and_ex
         record
             .warnings
             .iter()
-            .any(|w| w.contains("API source 'gsc'")),
+            .any(|w| w.contains("gsc: Konto Standard nicht verbunden")),
         "{:?}",
         record.warnings
     );
```

## Eigener Stub-Vertrag (K2-60)

### `crates/os-app/tests/crawl_cmd_analysis.rs`

INTEG-012: genau der Vertragstest für den ehemaligen M2-I06-Stub erwartet jetzt InvalidInput mit Integrationshinweis sowie weder Sitzung noch Crawl-Datei. Alle anderen Tests unverändert.

```diff
diff --git a/crates/os-app/tests/crawl_cmd_analysis.rs b/crates/os-app/tests/crawl_cmd_analysis.rs
index 1b4f75ef..74551a47 100644
--- a/crates/os-app/tests/crawl_cmd_analysis.rs
+++ b/crates/os-app/tests/crawl_cmd_analysis.rs
@@ -107,7 +107,22 @@ async fn api_source_delegates_to_i06_and_never_starts_http_crawl() {
     let dir = tempfile::tempdir().unwrap();
     let app = app(dir.path());
     let error=app.dispatch(ctx(),"crawl.start",json!({"source":{"kind":"apis","items":["https://fixture.test/"]},"scope":"domain","profile":null})).await.unwrap_err();
-    assert_eq!(error.code, os_app::ErrorCode::Unsupported);
-    assert_eq!(error.args["owner"], "M2-I06");
+    assert_eq!(error.code, os_app::ErrorCode::InvalidInput);
+    assert_eq!(error.message_key, "err-api-disconnected");
+    assert!(error
+        .detail
+        .as_deref()
+        .unwrap()
+        .contains("Mindestens eine Integration"));
+    assert!(
+        !app.config().paths.crawls.exists()
+            || std::fs::read_dir(&app.config().paths.crawls)
+                .unwrap()
+                .all(|entry| entry
+                    .unwrap()
+                    .path()
+                    .extension()
+                    .is_none_or(|ext| ext != "ospider"))
+    );
     assert!(app.sessions().current(&window()).is_none());
 }
```
