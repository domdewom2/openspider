//! Crawl sessions, bounded change feed and engine lifecycle (M2-K04).
pub mod analysis;
pub mod control;
pub mod feed;
pub mod selection;

use crate::{
    dto::*,
    events::{TickParts, TickSource},
    pool::CrawlKey,
    runtime::{lifecycle_bus::CrawlEvent, sessions::SessionInfo},
    App, CmdError, WindowId,
};
use os_model::CrawlState;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Default)]
pub struct SessionService {
    pub(crate) mutation: Arc<tokio::sync::Mutex<()>>,
    pub(crate) feeds: Mutex<HashMap<CrawlKey, feed::Cursor>>,
    pub(crate) parts: Mutex<TickParts>,
    pub(crate) analysis: Mutex<HashMap<CrawlKey, AnalysisStatus>>,
    pub(crate) version: std::sync::atomic::AtomicU64,
    pub(crate) listeners: Mutex<Vec<Arc<feed::ChangeListener>>>,
    pub(crate) active: std::sync::atomic::AtomicBool,
    pub(crate) quitting: std::sync::atomic::AtomicBool,
}
impl TickSource for SessionService {
    fn collect(&self, parts: &mut TickParts) {
        let p = std::mem::take(&mut *self.parts.lock().unwrap_or_else(|p| p.into_inner()));
        if p.crawl.is_some() {
            parts.crawl = p.crawl;
        }
        if p.analysis.is_some() {
            parts.analysis = p.analysis;
        }
        if p.counts_version.is_some() {
            parts.counts_version = p.counts_version;
        }
        if p.issues_version.is_some() {
            parts.issues_version = p.issues_version;
        }
        if p.sidebar_version.is_some() {
            parts.sidebar_version = p.sidebar_version;
        }
        parts.views.extend(p.views);
        parts.notices.extend(p.notices);
    }
}

/// Installed once by the service initializer. Weak ownership lets App shut down.
pub fn init(app: &App) {
    app.add_tick_source(app.ext::<SessionService>());
    app.set_quit_hook(Arc::new(crate::quit::SessionQuit));
    let weak = app.downgrade();
    app.handle().spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let Some(app) = weak.upgrade() else { break };
            if !app
                .ext::<SessionService>()
                .active
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                continue;
            }
            if let Err(e) = poll(&app).await {
                tracing::warn!(error = ?e, "Session poll failed");
            }
            app.flush_tick();
        }
    });
}

pub fn current(app: &App, window: &WindowId) -> Result<SessionInfo, CmdError> {
    app.sessions()
        .current(window)
        .ok_or_else(|| crate::runtime::error::no_crawl(&window.0))
}
pub fn writable(s: &SessionInfo) -> Result<(), CmdError> {
    if s.writable {
        Ok(())
    } else {
        Err(CmdError::new(
            ErrorCode::Unsupported,
            "err-crawl-foreign-owner",
        ))
    }
}
pub fn busy() -> CmdError {
    CmdError::new(ErrorCode::Busy, "err-crawl-busy").retryable()
}

/// Updates every window pointing at this crawl (including in-app MCP sessions).
pub fn state(app: &App, key: &CrawlKey, state: CrawlState) {
    app.sessions().update_state(key, state);
}

pub async fn poll(app: &App) -> Result<(), CmdError> {
    let service = app.ext::<SessionService>();
    let Ok(_guard) = service.mutation.try_lock() else {
        return Ok(());
    };
    let mut seen = std::collections::HashSet::new();
    for s in app.sessions().all() {
        if !seen.insert(s.key.clone()) {
            continue;
        }
        let finished = s.engine.as_ref().is_some_and(|e| {
            e.handle
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .as_ref()
                .is_some_and(|h| h.progress().state != CrawlState::Running)
        });
        if finished {
            control::settle(app, &s, None).await?;
        }
        feed::refresh(app, &s).await?;
    }
    Ok(())
}

pub fn close(app: &App, window: &WindowId, cleared: bool) {
    if let Some(s) = app.sessions().current(window) {
        app.sessions().clear(window);
        if cleared {
            let svc = app.ext::<SessionService>();
            let status = svc
                .feeds
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .get(&s.key)
                .and_then(|c| c.status.clone());
            let mut parts = svc.parts.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(mut status) = status {
                status.crawl_id.clear();
                status.phase = CrawlPhase::Idle;
                status.crawled = 0;
                status.queued = 0;
                status.discovered = 0;
                status.percent = 0;
                status.rate = 0.0;
                status.avg_rate = 0.0;
                status.active_threads = 0;
                status.end_reason = None;
                status.eta_s = None;
                status.owner.live = false;
                parts.crawl = Some(status);
            }
            parts.analysis = Some(AnalysisStatus {
                steps: Vec::new(),
                running: false,
                current: None,
                percent: None,
            });
            let version = svc
                .version
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                + 1;
            parts.counts_version = Some(version);
            parts.issues_version = Some(version);
            parts.sidebar_version = Some(version);
        }
        app.crawl_events().emit(
            app,
            if cleared {
                CrawlEvent::Cleared(s.key.clone())
            } else {
                CrawlEvent::Closed(s.key.clone())
            },
        );
        if !app.sessions().all().iter().any(|other| other.key == s.key) {
            app.unregister_crawl(&s.key);
            app.ext::<SessionService>()
                .feeds
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .remove(&s.key);
        }
    }
}

/// Activate event delivery when the first crawl session is attached.
pub fn activate(app: &App) {
    if !app
        .ext::<SessionService>()
        .active
        .swap(true, std::sync::atomic::Ordering::SeqCst)
    {
        app.set_tick_policy(Arc::new(crate::events::coalescer::Coalescer::default()));
    }
}
