//! M2-N31: Die Aufbewahrung beim App-Start (OUT-229) belegt nicht die Schreibwarteschlange der Nutzeraufträge.
//!
//! Eigene Testdatei (eigener Prozess), weil der Test einen globalen `tracing`-Subscriber setzt: Er hält den Thread an,
//! der die erste Löschung der Aufbewahrung meldet, bis der Test ihn freigibt. Die Aufbewahrung dauert damit
//! beliebig lange, ohne dass der Test auf Wanduhrzeit synchronisiert.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "runtime_common.rs"]
mod common;

use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;

use os_app::{
    dto::CrawlConfigDto, pool::CrawlKey, runtime::sessions::SessionInfo,
    settings_store::SettingsStore, WindowId,
};
use os_model::{CrawlConfig, CrawlState, IdSource, SeededIds};
use os_store::{CrawlFile, OpenMode};
use serde_json::{json, Value};
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};

/// Haltepunkt in der Löschmeldung der Aufbewahrung (`Crawl durch Aufbewahrung gelöscht`, Feld `age_days`).
#[derive(Default)]
struct Gate {
    /// Name des angehaltenen Threads, sobald die Aufbewahrung den Haltepunkt erreicht.
    entered: Mutex<Option<String>>,
    released: Mutex<bool>,
    changed: Condvar,
}

impl Gate {
    fn release(&self) {
        *self.released.lock().unwrap_or_else(|p| p.into_inner()) = true;
        self.changed.notify_all();
    }
}

fn gate() -> &'static Gate {
    static GATE: OnceLock<Gate> = OnceLock::new();
    GATE.get_or_init(Gate::default)
}

struct HoldRetention;

impl<S: tracing::Subscriber> Layer<S> for HoldRetention {
    fn on_event(&self, event: &tracing::Event<'_>, _: Context<'_, S>) {
        let meta = event.metadata();
        if !meta.target().starts_with("os_app::commands::crawls")
            || meta.fields().field("age_days").is_none()
        {
            return;
        }
        let gate = gate();
        *gate.entered.lock().unwrap_or_else(|p| p.into_inner()) = Some(
            std::thread::current()
                .name()
                .unwrap_or("unnamed")
                .to_string(),
        );
        gate.changed.notify_all();
        let mut released = gate.released.lock().unwrap_or_else(|p| p.into_inner());
        while !*released {
            released = gate
                .changed
                .wait(released)
                .unwrap_or_else(|p| p.into_inner());
        }
    }
}

/// Gibt die Aufbewahrung frei, auch wenn der Test vorher scheitert (sonst wartet das Beenden der App auf sie).
struct ReleaseOnDrop;

impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        gate().release();
    }
}

/// Wartet, bis die Aufbewahrung am Haltepunkt steht (Ereignis; die Grenze ist nur Hängeschutz).
async fn retention_held() -> String {
    tokio::task::spawn_blocking(|| {
        let gate = gate();
        let mut entered = gate.entered.lock().unwrap_or_else(|p| p.into_inner());
        let deadline = std::time::Instant::now() + Duration::from_secs(60);
        while entered.is_none() {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            assert!(!left.is_zero(), "retention never reached the deletion");
            entered = gate
                .changed
                .wait_timeout(entered, left)
                .unwrap_or_else(|p| p.into_inner())
                .0;
        }
        entered.clone().unwrap()
    })
    .await
    .unwrap()
}

fn dto(cfg: &CrawlConfig) -> Value {
    serde_json::to_value(CrawlConfigDto {
        schema_version: cfg.schema_version,
        generator: cfg.generator.clone(),
        config: serde_json::to_value(cfg).unwrap(),
    })
    .unwrap()
}

/// Abgeschlossener Crawl im Crawl-Ordner, Ende im Jahr 2000: fällt unter „1 Woche“.
fn expired_crawl(crawls: &std::path::Path) -> std::path::PathBuf {
    std::fs::create_dir_all(crawls).unwrap();
    let id = SeededIds::new(311)
        .new_crawl_id(common::created_at())
        .to_string();
    let path = common::make_crawl(crawls, &format!("{id}.ospider"));
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    file.set_meta("crawl_id", &id).unwrap();
    file.set_meta("state", CrawlState::Completed.as_str())
        .unwrap();
    file.set_meta("heartbeat_at", &common::created_at().to_db())
        .unwrap();
    file.set_meta("ended_at", "2000-01-01T00:00:00.000Z")
        .unwrap();
    path
}

// spec: OUT-229
// spec: CRAWL-126 [M2]
#[tokio::test]
async fn user_write_does_not_wait_behind_startup_retention() {
    tracing::subscriber::set_global_default(tracing_subscriber::registry().with(HoldRetention))
        .expect("eigener Prozess: erster globaler Subscriber");
    let dir = tempfile::tempdir().unwrap();
    // Richtlinie vor dem Start: Die Aufbewahrung beim Start hat etwas zu löschen.
    let (settings, _) = SettingsStore::open(dir.path());
    settings
        .apply_patch(&json!({"retention":{"policy":"one_week"}}))
        .unwrap();
    drop(settings);
    let expired = expired_crawl(&dir.path().join("crawls"));
    // Der Start reiht die Aufbewahrung ein; im Single-Thread-Laufzeitsystem läuft sie erst beim ersten `await`.
    let app = common::new_app(dir.path());
    let _release = ReleaseOnDrop;
    assert_eq!(app.config().paths.crawls, dir.path().join("crawls"));
    // Geöffneter, pausierter Crawl außerhalb des Crawl-Ordners: Ziel des Nutzerauftrags.
    let path = common::make_crawl(dir.path(), "paused.ospider");
    let key = CrawlKey("paused".into());
    app.register_crawl(key.clone(), path.clone());
    app.sessions().set_current(
        WindowId("main".into()),
        SessionInfo {
            key,
            path: path.clone(),
            config: Arc::new(CrawlConfig::default()),
            state: CrawlState::Paused,
            writable: true,
            engine: None,
        },
    );

    // Die Startaufbewahrung steht mitten in ihrer Runde (nach dem Löschen, vor dem Ende des Auftrags).
    let thread = retention_held().await;
    assert!(!expired.exists(), "retention deleted the expired crawl");

    // Nutzerauftrag über die Schreibwarteschlange: Konfigurationsänderung in der Pause (CRAWL-126).
    let mut cfg = CrawlConfig::default();
    cfg.title.max_chars = 50;
    // Hängeschutz: Auf der Basis wartet der Auftrag, bis der Test die Aufbewahrung freigibt (nie).
    let set = tokio::time::timeout(
        Duration::from_secs(10),
        app.dispatch(common::ctx(), "config.set", dto(&cfg)),
    )
    .await;
    assert!(
        set.is_ok(),
        "config.set waited behind the start-up retention (held on thread {thread})"
    );
    set.unwrap().unwrap();
    assert_eq!(
        CrawlFile::open(&path, OpenMode::ReadOnly)
            .unwrap()
            .config()
            .unwrap()
            .title
            .max_chars,
        50
    );

    // Danach darf die Aufbewahrung zu Ende laufen; eine weitere Runde wartet auf sie (strikte Reihenfolge).
    gate().release();
    assert_eq!(
        tokio::time::timeout(
            Duration::from_secs(60),
            os_app::commands::crawls::run_retention(&app)
        )
        .await
        .expect("retention finishes after release")
        .unwrap(),
        0
    );
}
