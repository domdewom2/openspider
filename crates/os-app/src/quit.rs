//! Shared shutdown path for window close, menu and keyboard shortcuts (F-11).
pub use crate::dto::QuitDecision;
use crate::{
    dto::{ErrorCode, McpStatus, QuitReason},
    runtime::{error::parse_args, hooks::QuitHook},
    session::SessionService,
    App, CallCtx, CmdError,
};
use futures_util::future::BoxFuture;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// Pause, checkpoint and WAL merge before exit (CRAWL-126: ≤ 3 s).
const QUIT_BUDGET: std::time::Duration = std::time::Duration::from_secs(3);

/// View-cache writes while quitting (CRAWL-126, OS-Festlegung vom 01.11.2026): writes queued before quitting began
/// still run, none are added afterwards, so no write follows the final WAL merge.
#[derive(Default)]
pub(crate) struct CacheGate {
    /// Set when quitting begins: no new view-cache submissions.
    closing: AtomicBool,
    /// Set on the writer thread once the jobs queued before it have run: a submission that raced with `closing` is
    /// skipped when it runs.
    sealed: Arc<AtomicBool>,
}

/// Stores derivable view facts (N21 counts and memberships, sidebar aggregates) through the write queue, unless the
/// app is quitting (CRAWL-126). Callers treat a skipped write like a failed one: the cache is rebuilt later.
pub(crate) async fn submit_cache(
    app: &App,
    key: &crate::pool::CrawlKey,
    f: impl FnOnce(&mut os_store::CrawlFile) -> Result<(), CmdError> + Send + 'static,
) -> Result<(), CmdError> {
    let gate = app.ext::<CacheGate>();
    if gate.closing.load(Ordering::SeqCst) {
        return Ok(());
    }
    let sealed = gate.sealed.clone();
    app.write_queue()
        .submit_file(key, move |file| {
            if sealed.load(Ordering::SeqCst) {
                return Ok(());
            }
            f(file)
        })
        .await
}

/// Quitting begins: no new view-cache writes from here on (CRAWL-126).
fn close_cache(app: &App) {
    app.ext::<CacheGate>().closing.store(true, Ordering::SeqCst);
}

/// Quitting failed or was refused: the view cache may be written again.
fn reopen_cache(app: &App) {
    let gate = app.ext::<CacheGate>();
    gate.sealed.store(false, Ordering::SeqCst);
    gate.closing.store(false, Ordering::SeqCst);
}

/// CRAWL-126: first the app's own pending writes (write queue, view cache), then a WAL merge of every writable open
/// file, waited for within the rest of the quit budget. A merge still running at the end is logged; the file stays
/// consistent and is merged when it is opened next.
async fn drain(app: &App, started: std::time::Instant) {
    let sealed = app.ext::<CacheGate>().sealed.clone();
    // Runs after every job queued so far (strict order on the writer thread), like `flush`.
    let _ = app
        .write_queue()
        .submit_admin(move || {
            sealed.store(true, Ordering::SeqCst);
            Ok(())
        })
        .await;
    for s in app.sessions().all() {
        if s.writable {
            crate::session::merge::start(app, &s.key, s.path.clone());
        }
    }
    if !crate::session::merge::wait_all(app, QUIT_BUDGET.saturating_sub(started.elapsed())).await {
        tracing::warn!(
            "WAL merge still running at quit: the file is merged when it is opened next"
        );
    }
}

pub struct SessionQuit;
impl QuitHook for SessionQuit {
    fn request_quit(&self, app: App) -> BoxFuture<'static, QuitDecision> {
        Box::pin(async move {
            let mut reasons = Vec::new();
            if app.sessions().all().iter().any(|s| {
                s.engine
                    .as_ref()
                    .is_some_and(|e| e.handle.lock().unwrap_or_else(|p| p.into_inner()).is_some())
            }) {
                reasons.push(QuitReason::Crawl);
            }
            for op in app.ops().active_statuses() {
                let reason = match op.label_key.as_str() {
                    "op-analysis" => QuitReason::Analysis,
                    "op-export" => QuitReason::Export,
                    _ => QuitReason::Crawl,
                };
                if !reasons.contains(&reason) {
                    reasons.push(reason);
                }
            }
            if mcp_connected(&app).await {
                reasons.push(QuitReason::McpSessions);
            }
            if reasons.is_empty() {
                // CRAWL-126: pending own writes first, then every writable file is merged before the app exits.
                let started = std::time::Instant::now();
                close_cache(&app);
                drain(&app, started).await;
                QuitDecision::Quit
            } else {
                QuitDecision::Confirm { reasons }
            }
        })
    }
}
async fn mcp_connected(app: &App) -> bool {
    app.dispatch(
        CallCtx::for_window("main"),
        "mcp.status",
        serde_json::Value::Null,
    )
    .await
    .ok()
    .and_then(|v| parse_args::<McpStatus>(v).ok())
    .is_some_and(|s| matches!(s,McpStatus::Connected { clients,.. } if clients > 0))
}

pub async fn confirm(app: &App, accepted: bool) -> Result<(), CmdError> {
    if !accepted {
        return Ok(());
    }
    let service = app.ext::<SessionService>();
    if service.quitting.swap(true, Ordering::SeqCst) {
        return Err(crate::session::busy());
    }
    let started = std::time::Instant::now();
    close_cache(app);
    let result = async {
        let active = app.ops().active_statuses();
        app.ops().cancel_all();
        // Do not exit while an exporter/analysis is still writing. Fail safely if it
        // cannot cooperate; the user can retry. Request drain is handled below.
        for op in active {
            tokio::time::timeout(std::time::Duration::from_secs(3), app.ops().wait(&op.op_id))
                .await
                .map_err(|_| CmdError::new(ErrorCode::Busy, "err-quit-operation-running"))?;
        }
        // Stop scheduling work immediately, even while a background feed holds
        // the session lock. Durability is still awaited below before exit.
        for s in app.sessions().all() {
            if let Some(engine) = s.engine {
                if let Some(handle) = engine
                    .handle
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .as_ref()
                {
                    handle.pause();
                }
            }
        }
        crate::session::interrupt_poll(app);
        let _guard = service.mutation.lock().await;
        for s in app.sessions().all() {
            if s.engine
                .as_ref()
                .is_some_and(|e| e.handle.lock().unwrap_or_else(|p| p.into_inner()).is_some())
            {
                crate::session::control::settle(app, &s, Some(false)).await?;
            }
        }
        if mcp_connected(app).await {
            app.dispatch(
                CallCtx::for_window("main"),
                "mcp.stop",
                serde_json::Value::Null,
            )
            .await?;
        }
        // CRAWL-126: exit only after the own pending writes ran and the files are merged, within the quit budget.
        drain(app, started).await;
        Ok(())
    }
    .await;
    if result.is_ok() {
        if let Some(host) = app.window_host() {
            host.exit();
        }
    } else {
        reopen_cache(app);
    }
    service.quitting.store(false, Ordering::SeqCst);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppConfig;
    use futures_util::FutureExt;
    use os_store::view_cache::{self, Entry};

    fn store(key: &'static str) -> impl FnOnce(&mut os_store::CrawlFile) -> Result<(), CmdError> {
        move |file| {
            let entry = Entry {
                step_stamp: "s".into(),
                built_seq: 1,
                data: vec![1],
            };
            view_cache::save(file, key, "p", &entry)
                .map(drop)
                .map_err(|e| {
                    CmdError::new(ErrorCode::Store, "err-store").with_detail(e.to_string())
                })
        }
    }

    fn stored(path: &std::path::Path, key: &str) -> bool {
        let file = os_store::CrawlFile::open(path, os_store::OpenMode::ReadOnly).unwrap();
        view_cache::load(&file, key, "p").unwrap().is_some()
    }

    // spec: CRAWL-126
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn view_cache_writes_queued_before_quitting_run_and_later_ones_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gate.ospider");
        let file = os_store::CrawlFile::create(
            &path,
            &os_store::CrawlInit {
                crawl_id: "00000000-0000-0000-0000-000000000126".parse().unwrap(),
                mode: os_model::CrawlMode::Spider,
                start_url: Some("https://site.test/".into()),
                list_source: None,
                project: "gate".into(),
                name: "gate".into(),
                config: os_model::CrawlConfig::default(),
                created_at: os_model::Timestamp::UNIX_EPOCH,
            },
            os_store::StorageProfile::Balanced,
        )
        .unwrap();
        file.set_meta("state", "Completed").unwrap();
        drop(file);
        let app = App::new(AppConfig::new(dir.path())).unwrap();
        app.dispatch(
            CallCtx::for_window("main"),
            "crawl.open",
            serde_json::json!({"kind": "path", "path": path}),
        )
        .await
        .unwrap();
        let key = app.sessions().all()[0].key.clone();

        // Hold the writer thread so that the first write is still queued when quitting begins.
        let (release, held) = std::sync::mpsc::channel::<()>();
        let mut blocker = Box::pin(app.write_queue().submit_file(&key, move |_| {
            held.recv().ok();
            Ok(())
        }));
        assert!(
            (&mut blocker).now_or_never().is_none(),
            "the writer thread is held"
        );
        let mut queued = Box::pin(submit_cache(&app, &key, store("queued")));
        assert!(
            (&mut queued).now_or_never().is_none(),
            "queued behind the held job"
        );

        close_cache(&app);
        submit_cache(&app, &key, store("after-close"))
            .await
            .unwrap();
        release.send(()).unwrap();
        blocker.await.unwrap();
        queued.await.unwrap();
        assert!(
            stored(&path, "queued"),
            "a write queued before quitting runs"
        );
        assert!(
            !stored(&path, "after-close"),
            "no new write once quitting began"
        );

        // A submission that passed the check just before quitting began runs after the seal and is skipped too.
        let gate = app.ext::<CacheGate>();
        gate.closing.store(false, Ordering::SeqCst);
        gate.sealed.store(true, Ordering::SeqCst);
        submit_cache(&app, &key, store("raced")).await.unwrap();
        assert!(!stored(&path, "raced"));

        // A refused or failed quit opens the cache again.
        reopen_cache(&app);
        submit_cache(&app, &key, store("reopened")).await.unwrap();
        assert!(stored(&path, "reopened"));
    }
}
