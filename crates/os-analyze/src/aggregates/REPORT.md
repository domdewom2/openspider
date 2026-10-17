# M2-K03 – Paketbericht

Umgebung: macOS arm64, Branch `m2/k03`, Basis `m2/f05-int`. Beim Start war der Worktree sauber. Clean-Room: ausschließlich Projektcode und Spezifikationen verwendet.

## Umgesetzte Spec-IDs

- **UI-066:** Verzeichnisbaum aus normalisierten URLs ohne Query/Fragment; HTML-Indexierbarkeit unabhängig vom unteren View; vorberechnete Tiefenaufteilungen nach Indexierbarkeit und Statusklasse. Tiefe 0 entfällt, Tiefe >= 10 fällt in Bucket 10. Begrenzter Baum (maximal 500 Knoten), drei anfänglich aufgeklappte Verzeichnisebenen unter dem Host und Paging direkter Kinder.
- **UI-069:** Elf Antwortzeit-Buckets mit inklusiver Untergrenze und exklusiver Obergrenze; `toMs: null` für 10+. Grundmengen alle sichtbaren URLs / internes HTML. Fehlgeschlagene Abrufe ohne Antwort stehen separat in `noResponse`; Warteschlange und robots-Blockaden sind keine Zeitmessungen. HTTP-Antworten ohne gespeicherte Zeit werden ebenfalls nicht als Antwortfehler ausgegeben.
- **UI-082 [M2]:** Gespeicherte Ergebniszustände plus separate Hinweise für Veraltung; schrittspezifische F05-Konfigurationshashes, persistierte Invalidierung über den vorhandenen ConfigChanged-Bus. Near-Duplicate-Schwelle betrifft nur NearDuplicates; Segmentregeln verändern keinen Analysezustand. Bestehende Ergebnisse bleiben gespeichert.
- **ONPAGE-140 [M2]:** Geänderter Inhaltsbereich ohne vollständig gespeichertes HTML ergibt `needsRecrawl` für betroffene Content-Schritte. Prüfung der tatsächlichen HTML-Abdeckung, auch vor dem ersten Analyselauf; im JS-Modus wird gerendertes HTML geprüft. Die Inhaltsbereichs-Provenienz bleibt nach dem Speichern der neuen Konfiguration erhalten. Die eigentliche Wortzahl-Neuberechnung bleibt bei D02.
- **UI-145:** Verbindungsgebundener SQLite-Temp-Cache mit 4-MiB-Seitencache, ohne crawlweiten RAM-Baum und ohne Schreibzugriff auf die Crawl-Datei. Erstaufbau beim Öffnen/Start/Abschluss im Blocking-Lesepool; danach Delta-Pflege über `change_seq`, Löschabgleich bei `reset_seq`, idempotente `apply_changes`, transaktionaler Rollback und Abbruch. Unveränderte Aufrufe und View-Wechsel lesen nur Aggregate. Die Zeitbudgets bei 5 Mio. URLs sind als separater Benchmark hinterlegt, noch nicht auf ruhiger Referenzhardware gemessen.

## Schnittstelle für K04 / K07

`aggregates::status::analysis_status(file, cfg)` behält die F05-Signatur und das Feld `steps`. Ergänzt sind `stale`, `needs_recrawl` und `error`. `try_analysis_status` bietet direkte Fehlerweitergabe. `commands::sidebar_ext::step_statuses` bildet diese Informationen auf `AnalysisStepStatus` / `StepStateDto::NeedsRecrawl` ab. K04 bindet dies in seinen Command `analysis.status` ein.

Der vorhandene F05b-Aufruf `sidebar_ext::on_config_changed` wird ausgefüllt und persistiert Invalidierung über die WriteQueue. K07 bleibt Besitzer von `config.set`. Zwei vollständige Integrationstests stehen gemäß C9/K2-26 in `crates/os-app/tests/pending_w1_k03.rs` mit `wartet auf M2-K04` bzw. `wartet auf M2-K07`. Die Fachprüfungen selbst sind nicht ignoriert.

## Tests

Der abschließende Gesamtlauf auf dem unveränderten Quellstand dauerte **515,26 s (8:35 min), einschließlich Kompilierung**; Ergebnis: 2.324 bestanden, 0 Fehler, 43 vorgesehene Ignores.

Die Store-Fixtures liegen als eigener Testhelfer in `aggregates_fixture.rs`; keine externen Requests oder Referenzdaten. Geprüft werden unter anderem Tiefen 0–12, noindex/404, Nicht-HTML, externe URLs, Bucket-Grenzen, Baum-Paging, Updates/Löschungen/Wiederholungen, fehlendes und gerendertes HTML, selektive Invalidierung, Read-only-Nutzung, fehlender Vollscan bei Cache-Hits, Pool-Affinität, exklusive Cache-Freigabe, Vorberechnung per Lifecycle-Bus, Abbruch und Rollback.

Grün: 17 eigene Fach-/Command-Tests (6 Aggregate, 5 Status, 6 App), `cargo fmt --all --check`, `cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings`, `cargo deny --offline check licenses bans sources`, `python3 -I docs/plan/M2-check.py` und `scripts/check-neutral.sh`. Die Neutralitätsprüfung meldet nur vier bereits bestehende verwaiste Ausnahmen in der fremden Allow-Datei. Frontendprüfungen sind nicht erforderlich, da keine Dateien unter `apps/desktop/src` geändert wurden.

`cargo test --workspace --exclude openspider-desktop --locked`: grün, einschließlich Doc-Tests; 2.324 bestandene Tests, 0 Fehler, 43 vorgesehene Ignores über den gesamten Workspace (davon die drei hier beschriebenen K03-Ignores).

Benchmark-Aufruf für den Orchestrator:

```sh
cargo test -p os-analyze --test aggregates_benchmark five_million_precomputed_sidebar_reads -- --ignored --nocapture
```

Er misst nach dem getrennt ausgewiesenen Erstaufbau fünfmal die vorbereiteten Abfragen (<= 1 s) und Tiefen-/View-Wechsel (<= 200 ms). Der Erstaufbau wird nicht als Cache-Hit ausgegeben.

## Abweichungen und offene Punkte

Keine nicht-additive Fremdänderung erforderlich. Segmentmetriken und die optionale Segmentfilterung benötigen K14; segmentierte Site-Structure-Anfragen melden diesen Besitzer ausdrücklich. Keine Änderung an Frontend, DTO-Vertrag, Manifesten, Schema-Migrationen oder Goldens. Zwei geplante C9-Integrationstests und der Benchmark bleiben ignoriert. Kein Push, Merge oder PR; deshalb keine neue CI-Lauf-ID. Keine zusätzliche Klärungsfrage an den Nutzer.

## Stub-Ablösung (K2-60)

- `crates/os-engine/f05a-stubs.toml`: ausschließlich die beiden K03-Einträge für `aggregates/live.rs::apply_changes` und `aggregates/status.rs::analysis_status` entfernt.
- `crates/os-app/tests/manifest_contract.rs`: ausschließlich K03 aus der Liste noch vorhandener Stub-Besitzer entfernt (anschließend rustfmt).

## Fremdergänzungen (K2-61)

Der F05-Lesepool hatte keine benannte Verbindung für verbindungsgebundene Disk-Caches. Eine beliebige Verbindung je Aufruf würde unter konkurrierender Last den gesamten Crawl erneut scannen. Ergänzt sind ein initial leerer Cache-Slot-Speicher, die neue Methode `ReadPool::run_cached` und die Freigabe ausschließlich dieser neuen Slots beim exklusiven Zugriff. Bestehende Methoden behalten ihre Signaturen und ihr Verhalten für normale Leseaufträge. Die neue Methode verwendet die vorhandenen Unterbrechungs-, Blocking- und Exklusivsperren. Registerwechsel und Schließen geben die neuen Slots über den bestehenden Slot-Lebenszyklus frei.

Zusätzlich wird der eigene Vorberechnungs-Hörer in der App-Initialisierung registriert. Der vorhandene F05-Konfigurations-Hörer bleibt unverändert.

Vollständiger additiver Diff dieser beiden fremden Dateien:

```diff
diff --git a/crates/os-app/src/pool.rs b/crates/os-app/src/pool.rs
index 8c5f1ee..8291bb3 100644
--- a/crates/os-app/src/pool.rs
+++ b/crates/os-app/src/pool.rs
@@ -32,6 +32,7 @@ pub const DEFAULT_READ_CONNECTIONS: usize = 4;
 pub struct CrawlKey(pub String);

 struct Slot {
+    pinned: Mutex<HashMap<&'static str, Arc<Slot>>>,
     path: PathBuf,
     idle: Mutex<Vec<CrawlFile>>,
     permits: Arc<Semaphore>,
@@ -83,6 +84,15 @@ impl ReadPool {
             Some(slot) => {
                 let guard = slot.access.clone().write_owned().await;
                 slot.idle.lock().unwrap_or_else(|p| p.into_inner()).clear();
+                let mut pinned = slot.pinned.lock().unwrap_or_else(|p| p.into_inner());
+                for cached in pinned.values() {
+                    cached
+                        .idle
+                        .lock()
+                        .unwrap_or_else(|p| p.into_inner())
+                        .clear();
+                }
+                pinned.clear();
                 Some(guard)
             }
             None => None,
@@ -121,6 +131,7 @@ impl ReadPool {
     /// (leerlaufende Verbindungen werden geschlossen, laufende Arbeiten beenden ihre Verbindung).
     pub fn register(&self, key: CrawlKey, path: impl Into<PathBuf>) {
         let slot = Arc::new(Slot {
+            pinned: Mutex::new(HashMap::new()),
             path: path.into(),
             idle: Mutex::new(Vec::new()),
             permits: Arc::new(Semaphore::new(self.inner.capacity)),
@@ -162,6 +173,47 @@ impl ReadPool {
         self.run_with(crawl, token, f).await
     }

+    /// Verbindungsgebundener Cache (z. B. Sidebar-Temp-Tabellen): je Name eine
+    /// serialisierte Leseverbindung. Nutzt dieselben Abbruch- und Exklusivsperren
+    /// wie `run_with`; Registerwechsel und Schließen verwerfen auch diesen Cache.
+    pub async fn run_cached<T: Send + 'static>(
+        &self,
+        crawl: CrawlKey,
+        cache: &'static str,
+        token: CancelToken,
+        f: impl FnOnce(&CrawlFile, &CancelToken) -> Result<T, StoreError> + Send + 'static,
+    ) -> Result<T, CmdError> {
+        let slot = self
+            .inner
+            .lock()
+            .get(&crawl)
+            .cloned()
+            .ok_or_else(|| no_crawl(&crawl.0))?;
+        let cached = slot
+            .pinned
+            .lock()
+            .unwrap_or_else(|p| p.into_inner())
+            .entry(cache)
+            .or_insert_with(|| {
+                Arc::new(Slot {
+                    pinned: Mutex::new(HashMap::new()),
+                    path: slot.path.clone(),
+                    idle: Mutex::new(Vec::new()),
+                    permits: Arc::new(Semaphore::new(1)),
+                    access: slot.access.clone(),
+                })
+            })
+            .clone();
+        let pool = ReadPool {
+            inner: Arc::new(Inner {
+                crawls: Mutex::new(HashMap::from([(crawl.clone(), cached)])),
+                hub: self.inner.hub.clone(),
+                capacity: 1,
+            }),
+        };
+        pool.run_with(crawl, token, f).await
+    }
+
     /// Wie [`run`](Self::run) mit ausdrücklicher Marke.
     pub async fn run_with<T: Send + 'static>(
         &self,
diff --git a/crates/os-app/src/runtime/services.rs b/crates/os-app/src/runtime/services.rs
index 63135d6..cacf24c 100644
--- a/crates/os-app/src/runtime/services.rs
+++ b/crates/os-app/src/runtime/services.rs
@@ -1,6 +1,7 @@
 //! Feste Start- und Änderungsreihenfolge der App-Dienste.
 use super::{lifecycle_bus::CrawlEvent, App};
 pub fn start(app: &App) {
+    crate::commands::sidebar_ext::on_startup(app);
     crate::logging::init_for_app(app);
     crate::commands::api::on_startup(app);
     crate::commands::crawls::start_retention(app);
```
