//! Laufzeit der Service-Schicht: `App`, Aufruf-Kontext und Dispatcher (`docs/plan/M2.md` 3.6, M2-F03).
//!
//! * [`App::dispatch`] schlägt den Command in der Registry nach, ruft den Handler und fängt Panics ab
//!   (`Internal`). Tauri-Commands, die E2E-Bridge und der HTTP-MCP rufen Commands ausschließlich hierüber.
//! * Datenbankzugriffe laufen über [`ReadPool`] (Lesen) und [`WriteQueue`] (Schreiben, ein Thread), lang laufende
//!   Arbeiten als Operation über [`Ops`] (Abbruch binnen 100 ms, SQLite-Interrupt).
//! * Ein Ereigniskanal je Fenster ([`App::subscribe`]); Ticks baut [`App::assemble_tick`] zusammen, der Takt kommt
//!   vom Coalescer (M2-K04).
//! * Einstellungen (`settings.toml`) und UI-Zustand (`ui-state.json`) liegen im Einstellungsordner.
//!
//! Die Abbruchmarke des laufenden Aufrufs steht über [`current_cancel`] zur Verfügung, auch ohne sie
//! weiterzureichen; so ist jeder `ReadPool::run` innerhalb eines Commands oder einer Operation abbrechbar.

#[cfg(feature = "e2e-bridge")]
pub mod bridge;
pub mod bridge_proto;
pub mod error;
pub mod hooks;
pub mod info;
pub mod interrupt;
pub mod windows;

use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::FutureExt;
use os_analyze::analysis::CancelToken;
use os_fetch::secrets::KeyringSecretStore;
use os_fetch::{MemorySecretStore, SecretStore};
use os_model::settings::{AppSettings, LIVE_INTERVAL_RANGE_MS};
use os_model::{Clock, IdSource, RandomIds, SystemClock};
use os_store::paths::{app_paths, app_paths_with_home, AppPaths, HOME_ENV};
use tokio::runtime::{Handle, Runtime};

use crate::dto::{AppSettingsDto, CmdError, ErrorCode, Notice, SubscriptionId, Tick, UiEvent};
use crate::events::{EventSink, TickAssembler, TickParts, TickSource};
use crate::ops::{Ops, OpsDeps};
use crate::pool::{CrawlKey, ReadPool, DEFAULT_READ_CONNECTIONS};
use crate::quit::QuitDecision;
use crate::registry::{default_registry, CmdResult, Registry};
use crate::settings_store::SettingsStore;
use crate::uistate::UiStateStore;
use crate::write_queue::WriteQueue;

pub use hooks::{AssetResolver, AssetResponse, QuitHook};

use self::interrupt::InterruptHub;
use self::windows::Windows;

tokio::task_local! {
    static CALL_CANCEL: CancelToken;
}

/// Abbruchmarke des laufenden Aufrufs oder der laufenden Operation; außerhalb davon eine neue, nie abgebrochene
/// Marke.
pub fn current_cancel() -> CancelToken {
    CALL_CANCEL.try_with(Clone::clone).unwrap_or_default()
}

/// Führt `fut` mit `token` als Marke des laufenden Aufrufs aus.
pub(crate) fn with_cancel<F: Future>(
    token: CancelToken,
    fut: F,
) -> impl Future<Output = F::Output> {
    CALL_CANCEL.scope(token, fut)
}

/// Kennung eines Fensters (`main`, `settings`, `viz-<n>`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WindowId(pub String);

/// Kontext eines Aufrufs: aufrufendes Fenster und Abbruchmarke.
#[derive(Debug, Clone)]
pub struct CallCtx {
    /// Fenster.
    pub window: WindowId,
    /// Abbruchmarke (`op.cancel`, SQLite-Interrupt).
    pub cancel: CancelToken,
}

impl CallCtx {
    /// Kontext für ein Fenster mit neuer Marke.
    pub fn for_window(window: impl Into<String>) -> CallCtx {
        CallCtx {
            window: WindowId(window.into()),
            cancel: CancelToken::new(),
        }
    }
}

/// Konfiguration beim Start.
#[derive(Clone)]
pub struct AppConfig {
    /// Basisordner der App (`OPENSPIDER_HOME` bzw. Einstellungsordner des Benutzers).
    pub home: PathBuf,
    /// Alle Ordner (Einstellungen, Crawls, Logs, Cache).
    pub paths: AppPaths,
    /// Wert von `OPENSPIDER_HOME`, falls gesetzt (nur zur Anzeige in „Hilfe → Debug“).
    pub home_override: Option<String>,
    /// „Dokumente“-Ordner für den Standard-Exportordner (OUT-222); `None` = Ordner laut Betriebssystem.
    pub documents_dir: Option<PathBuf>,
    /// Uhr.
    pub clock: Arc<dyn Clock>,
    /// Quelle der Kennungen (Operationen).
    pub ids: Arc<dyn IdSource>,
    /// Geheimnisse (Schlüsselbund). Tests und die Bridge verwenden immer `MemorySecretStore` (C13).
    /// STUB(M2-F02): wechselt auf `Arc<dyn SecretStoreMut>`, sobald M2-F02 den Typ liefert.
    pub secrets: Arc<dyn SecretStore>,
    /// Synthetische 5-Mio.-View aktivieren (Feature `stub-views`, M2-S01).
    pub stub_views: bool,
    /// Leseverbindungen je Crawl im Lese-Pool.
    pub read_connections: usize,
}

impl std::fmt::Debug for AppConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppConfig")
            .field("home", &self.home)
            .field("paths", &self.paths)
            .field("home_override", &self.home_override)
            .field("documents_dir", &self.documents_dir)
            .field("stub_views", &self.stub_views)
            .field("read_connections", &self.read_connections)
            .finish_non_exhaustive()
    }
}

impl AppConfig {
    /// Alles unter `home` (wie `OPENSPIDER_HOME`: Einstellungen direkt in `home`, dazu `crawls`, `logs`, `cache`;
    /// „Dokumente“ ist `home/Documents`), Systemuhr, zufällige Kennungen, Geheimnisse im Arbeitsspeicher. Der
    /// Konstruktor für Tests und die Bridge.
    pub fn new(home: impl Into<PathBuf>) -> AppConfig {
        let home = home.into();
        AppConfig {
            paths: app_paths_with_home(Some(home.as_os_str())),
            documents_dir: Some(home.join("Documents")),
            home,
            home_override: None,
            clock: Arc::new(SystemClock),
            ids: Arc::new(RandomIds),
            secrets: Arc::new(MemorySecretStore::new()),
            stub_views: false,
            read_connections: DEFAULT_READ_CONNECTIONS,
        }
    }

    /// Orte laut Betriebssystem bzw. `OPENSPIDER_HOME` (OUT-227); Geheimnisse im Schlüsselbund, außer
    /// `OPENSPIDER_SECRET_STORE=memory` (C13).
    pub fn from_env() -> AppConfig {
        let paths = app_paths();
        let home_override = std::env::var(HOME_ENV).ok().filter(|h| !h.is_empty());
        let memory = std::env::var("OPENSPIDER_SECRET_STORE").is_ok_and(|v| v == "memory");
        let secrets: Arc<dyn SecretStore> = if memory {
            Arc::new(MemorySecretStore::new())
        } else {
            Arc::new(KeyringSecretStore::new())
        };
        AppConfig {
            home: paths.settings.clone(),
            documents_dir: home_override
                .as_ref()
                .map(|h| PathBuf::from(h).join("Documents")),
            paths,
            home_override,
            clock: Arc::new(SystemClock),
            ids: Arc::new(RandomIds),
            secrets,
            stub_views: false,
            read_connections: DEFAULT_READ_CONNECTIONS,
        }
    }

    /// Ersetzt die Uhr.
    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> AppConfig {
        self.clock = clock;
        self
    }

    /// Ersetzt die Quelle der Kennungen.
    pub fn with_ids(mut self, ids: Arc<dyn IdSource>) -> AppConfig {
        self.ids = ids;
        self
    }

    /// Ersetzt den Speicher für Geheimnisse.
    pub fn with_secrets(mut self, secrets: Arc<dyn SecretStore>) -> AppConfig {
        self.secrets = secrets;
        self
    }
}

/// Laufzeit der Operationen: die des Aufrufers, sonst eine eigene.
struct Exec {
    handle: Handle,
    owned: Mutex<Option<Runtime>>,
}

impl Exec {
    fn new() -> Result<Exec, CmdError> {
        if let Ok(handle) = Handle::try_current() {
            return Ok(Exec {
                handle,
                owned: Mutex::new(None),
            });
        }
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("os-app-rt")
            .enable_all()
            .build()
            .map_err(|e| {
                CmdError::new(ErrorCode::Internal, "err-runtime").with_detail(e.to_string())
            })?;
        Ok(Exec {
            handle: rt.handle().clone(),
            owned: Mutex::new(Some(rt)),
        })
    }
}

struct Inner {
    config: AppConfig,
    registry: Registry,
    read_pool: ReadPool,
    write_queue: WriteQueue,
    ops: Ops,
    settings: SettingsStore,
    uistate: UiStateStore,
    windows: Windows,
    assembler: Mutex<TickAssembler>,
    tick_sources: Mutex<Vec<Arc<dyn TickSource>>>,
    notices: Mutex<Vec<Notice>>,
    live_interval_ms: AtomicU32,
    quit_hook: Mutex<Option<Arc<dyn QuitHook>>>,
    asset_resolver: Mutex<Option<Arc<dyn AssetResolver>>>,
    exec: Exec,
    // zuletzt: der Wächter-Thread endet erst, wenn Pool und Operationen weg sind
    hub: Arc<InterruptHub>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        let rt = self
            .exec
            .owned
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take();
        if let Some(rt) = rt {
            rt.shutdown_background();
        }
    }
}

/// Die Anwendung ohne Tauri: ein Exemplar je Prozess, geteilt von Tauri-Hülle, E2E-Bridge und HTTP-MCP.
#[derive(Clone)]
pub struct App(Arc<Inner>);

fn clamp_interval(ms: u32) -> u32 {
    ms.clamp(LIVE_INTERVAL_RANGE_MS.0, LIVE_INTERVAL_RANGE_MS.1)
}

impl App {
    /// Erzeugt die Anwendung: lädt `settings.toml` (eine unlesbare Datei wird gesichert, die Standardwerte gelten und
    /// eine Meldung erscheint im ersten Tick), startet Schreib-Thread und Interrupt-Wächter. Läuft kein Tokio-Runtime,
    /// legt die App eine eigene an.
    pub fn new(config: AppConfig) -> Result<App, CmdError> {
        App::with_registry(config, default_registry())
    }

    /// Wie [`App::new`] mit einem eigenen Register (Tests und Einbettungen mit zusätzlichen Commands).
    pub fn with_registry(config: AppConfig, registry: Registry) -> Result<App, CmdError> {
        let exec = Exec::new()?;
        let hub = Arc::new(InterruptHub::new());
        let (settings, startup_notice) = SettingsStore::open(config.paths.settings.clone());
        let interval = clamp_interval(settings.get().appearance.live_interval_ms);
        let notices: Vec<Notice> = startup_notice.into_iter().collect();
        let ops = Ops::new(OpsDeps {
            handle: exec.handle.clone(),
            hub: Arc::clone(&hub),
            ids: Arc::clone(&config.ids),
            clock: Arc::clone(&config.clock),
        });
        Ok(App(Arc::new(Inner {
            read_pool: ReadPool::new(Arc::clone(&hub), config.read_connections),
            write_queue: WriteQueue::new(),
            ops,
            uistate: UiStateStore::new(config.paths.settings.clone()),
            settings,
            windows: Windows::default(),
            assembler: Mutex::new(TickAssembler::new()),
            tick_sources: Mutex::new(Vec::new()),
            notices: Mutex::new(notices),
            live_interval_ms: AtomicU32::new(interval),
            quit_hook: Mutex::new(None),
            asset_resolver: Mutex::new(None),
            registry,
            config,
            exec,
            hub,
        })))
    }

    /// Führt den Command `name` mit `args` aus (Tauri, Bridge, HTTP-MCP). Ein Panic im Handler wird zu `Internal`.
    pub async fn dispatch(&self, ctx: CallCtx, name: &str, args: serde_json::Value) -> CmdResult {
        let Some((_, handler)) = self.0.registry.get(name) else {
            return Err(
                CmdError::new(ErrorCode::NotFound, "err-unknown-command").with_arg("command", name)
            );
        };
        if ctx.cancel.is_cancelled() {
            return Err(CmdError::new(ErrorCode::Cancelled, "err-cancelled"));
        }
        let token = ctx.cancel.clone();
        let call = match std::panic::catch_unwind(AssertUnwindSafe(|| handler(self, ctx, args))) {
            Ok(call) => call,
            Err(p) => return Err(self.panicked(name, p.as_ref())),
        };
        match with_cancel(token, AssertUnwindSafe(call).catch_unwind()).await {
            Ok(result) => result,
            Err(p) => Err(self.panicked(name, p.as_ref())),
        }
    }

    fn panicked(&self, name: &str, p: &(dyn std::any::Any + Send)) -> CmdError {
        let text = error::panic_text(p);
        tracing::error!(command = name, panic = %text, "Panic im Command");
        error::internal(name, text)
    }

    /// Das Register dieser Anwendung.
    pub fn registry(&self) -> &Registry {
        &self.0.registry
    }

    /// Konfiguration.
    pub fn config(&self) -> &AppConfig {
        &self.0.config
    }

    /// Laufzeit der Operationen.
    pub fn handle(&self) -> Handle {
        self.0.exec.handle.clone()
    }

    /// Die App-Einstellungen.
    pub fn settings(&self) -> &SettingsStore {
        &self.0.settings
    }

    /// Der UI-Zustand.
    pub fn uistate(&self) -> &UiStateStore {
        &self.0.uistate
    }

    /// Abonniert die Ereignisse eines Fensters (ein Kanal je Fenster; ein früheres Abonnement wird ersetzt).
    pub fn subscribe(&self, window: WindowId, sink: Arc<dyn EventSink>) -> SubscriptionId {
        self.0.windows.subscribe(window, sink)
    }

    /// Beendet ein Abonnement.
    pub fn unsubscribe(&self, id: &SubscriptionId) -> bool {
        self.0.windows.unsubscribe(id)
    }

    /// Fenster mit Abonnement.
    pub fn subscribed_windows(&self) -> Vec<WindowId> {
        self.0.windows.subscribed()
    }

    /// Meldet Sichtbarkeit eines Fensters aus den Fensterereignissen der Hülle (UI-143, Quelle 2 von 2.5).
    pub fn set_visibility(&self, window: WindowId, visible: bool) {
        self.0.windows.set_shell_visible(window, visible);
    }

    /// Meldet Sichtbarkeit eines Fensters aus dem Frontend (`visibilitychange`, Quelle 1; Command
    /// `events.setVisibility`).
    pub fn set_frontend_visibility(&self, window: WindowId, visible: bool) {
        self.0.windows.set_frontend_visible(window, visible);
    }

    /// Ein Fenster wurde geschlossen: sein Abonnement und seine Sichtbarkeitsmeldungen werden vergessen.
    pub fn window_closed(&self, window: &WindowId) {
        self.0.windows.forget(window);
    }

    /// Sichtbar = beide Quellen sichtbar.
    pub fn is_visible(&self, window: &WindowId) -> bool {
        self.0.windows.is_visible(window)
    }

    /// Sendet ein Ereignis an ein Fenster; `false`, wenn es kein offenes Abonnement gibt.
    pub fn send_event(&self, window: &WindowId, ev: UiEvent) -> bool {
        self.0.windows.send_to(window, ev)
    }

    /// Sendet ein Ereignis an alle abonnierten Fenster; Anzahl der erreichten Fenster.
    pub fn broadcast(&self, ev: UiEvent) -> usize {
        self.0.windows.broadcast(&ev)
    }

    /// Live-Intervall der Oberfläche in Millisekunden (UI-018, 250 – 5000).
    pub fn live_interval_ms(&self) -> u32 {
        self.0.live_interval_ms.load(Ordering::SeqCst)
    }

    /// Setzt das Live-Intervall (`events.setInterval`, wirkt bis zum nächsten Ändern der Einstellung).
    pub fn set_live_interval_ms(&self, ms: u32) -> Result<(), CmdError> {
        let (lo, hi) = LIVE_INTERVAL_RANGE_MS;
        if !(lo..=hi).contains(&ms) {
            return Err(CmdError::invalid_input("err-interval-out-of-range", "ms")
                .with_arg("min", lo)
                .with_arg("max", hi));
        }
        self.0.live_interval_ms.store(ms, Ordering::SeqCst);
        Ok(())
    }

    /// Taktabstand eines Fensters: Intervall, bei verdecktem Fenster mindestens eine Sekunde.
    pub fn window_period(&self, window: &WindowId) -> Duration {
        crate::events::effective_period(self.live_interval_ms(), self.is_visible(window))
    }

    /// Trägt einen Beitragenden zu den Ticks ein (Sitzung, MCP).
    pub fn add_tick_source(&self, source: Arc<dyn TickSource>) {
        self.0
            .tick_sources
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(source);
    }

    /// Merkt eine Meldung für den nächsten Tick vor (seltene Ereignisse werden nicht sofort gesendet).
    pub fn push_notice(&self, notice: Notice) {
        self.0
            .notices
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(notice);
    }

    /// Baut den nächsten Tick aus den Beiträgen, den vorgemerkten Meldungen und den Ständen der Operationen. `None`,
    /// wenn sich nichts geändert hat.
    pub fn assemble_tick(&self) -> Option<Tick> {
        let sources: Vec<Arc<dyn TickSource>> = self
            .0
            .tick_sources
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let mut parts = TickParts::default();
        for s in &sources {
            s.collect(&mut parts);
        }
        parts.notices.extend(std::mem::take(
            &mut *self.0.notices.lock().unwrap_or_else(|p| p.into_inner()),
        ));
        let ops = self.0.ops.take_updates();
        let at = self.0.config.clock.now().to_db();
        self.0
            .assembler
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .assemble(at, parts, ops)
    }

    /// Baut einen Tick und sendet ihn an alle abonnierten Fenster; Anzahl der erreichten Fenster (0 = nichts zu
    /// melden oder kein Fenster). Ohne abonniertes Fenster bleibt alles vorgemerkt (Meldungen gehen nicht verloren).
    pub fn flush_tick(&self) -> usize {
        if self.0.windows.subscribed().is_empty() {
            return 0;
        }
        match self.assemble_tick() {
            Some(tick) => self.broadcast(UiEvent::Tick {
                tick: Box::new(tick),
            }),
            None => 0,
        }
    }

    /// Wendet einen Merge-Patch auf die Einstellungen an (`settings.set`), meldet `settingsChanged` an alle Fenster
    /// und liefert den neuen Stand mit den Schlüsseln, die einen Neustart erfordern.
    pub fn apply_settings_patch(
        &self,
        patch: &serde_json::Value,
    ) -> Result<AppSettingsDto, CmdError> {
        let change = self.0.settings.apply_patch(patch)?;
        if !change.changed.is_empty() {
            self.0.live_interval_ms.store(
                clamp_interval(change.settings.appearance.live_interval_ms),
                Ordering::SeqCst,
            );
            self.broadcast(UiEvent::SettingsChanged {
                keys: change.changed.clone(),
            });
        }
        Ok(crate::settings_store::settings_to_dto(
            &change.settings,
            &change.restart_required,
        ))
    }

    /// Liest `settings.toml` erneut (Änderung von außen) und meldet `settingsChanged`.
    pub fn reload_settings(&self) -> Result<Vec<String>, CmdError> {
        let changed = self.0.settings.reload()?;
        if !changed.is_empty() {
            self.0.live_interval_ms.store(
                clamp_interval(self.0.settings.get().appearance.live_interval_ms),
                Ordering::SeqCst,
            );
            self.broadcast(UiEvent::SettingsChanged {
                keys: changed.clone(),
            });
        }
        Ok(changed)
    }

    /// Aktuelle Einstellungen.
    pub fn settings_snapshot(&self) -> AppSettings {
        self.0.settings.get()
    }

    /// Macht eine Crawl-Datei für Lese-Pool und Schreibwarteschlange verfügbar.
    pub fn register_crawl(&self, key: CrawlKey, path: impl Into<PathBuf>) {
        let path = path.into();
        self.0.read_pool.register(key.clone(), path.clone());
        self.0.write_queue.register(key, path);
    }

    /// Nimmt eine Crawl-Datei wieder weg.
    pub fn unregister_crawl(&self, key: &CrawlKey) {
        self.0.read_pool.unregister(key);
        self.0.write_queue.unregister(key);
    }

    /// Hängt die Entscheidung beim Beenden ein (Sitzung, M2-K04).
    pub fn set_quit_hook(&self, hook: Arc<dyn QuitHook>) {
        *self.0.quit_hook.lock().unwrap_or_else(|p| p.into_inner()) = Some(hook);
    }

    /// Hängt die Auflösung von `ospider://shot|img/…` ein (M2-K09).
    pub fn set_asset_resolver(&self, resolver: Arc<dyn AssetResolver>) {
        *self
            .0
            .asset_resolver
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = Some(resolver);
    }

    /// Löst eine Anfrage an das URI-Schema `ospider://` auf (404, solange kein Auflöser eingehängt ist).
    pub async fn resolve_asset(&self, uri: &str) -> AssetResponse {
        let resolver = self
            .0
            .asset_resolver
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        match resolver {
            Some(r) => r.resolve(self.clone(), uri.to_string()).await,
            None => AssetResponse::not_found(),
        }
    }

    /// Beenden anfragen (F-11). Ohne eingehängte Sitzung: laufende Operationen abbrechen, eingereihte Schreibaufträge
    /// (höchstens 2 s) abwarten und `Quit` melden.
    pub async fn request_quit(&self) -> QuitDecision {
        let hook = self
            .0
            .quit_hook
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let decision = match hook {
            Some(h) => h.request_quit(self.clone()).await,
            None => QuitDecision::Quit,
        };
        if matches!(decision, QuitDecision::Quit) {
            self.0.ops.cancel_all();
            let _ = tokio::time::timeout(Duration::from_secs(2), self.0.write_queue.flush()).await;
        }
        decision
    }

    /// Lese-Pool für alle Datenbankzugriffe der App.
    pub fn read_pool(&self) -> &ReadPool {
        &self.0.read_pool
    }

    /// Serialisierte Schreibwarteschlange (K-20).
    pub fn write_queue(&self) -> &WriteQueue {
        &self.0.write_queue
    }

    /// Operationen mit Abbruch.
    pub fn ops(&self) -> &Ops {
        &self.0.ops
    }

    /// Interrupt-Wächter (Tests).
    pub fn interrupt_hub(&self) -> &InterruptHub {
        &self.0.hub
    }
}
