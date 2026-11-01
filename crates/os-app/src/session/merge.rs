//! Background WAL merge after a pause or an end (CRAWL-126, OS-Festlegung vom 01.11.2026).
//!
//! The engine reports "Paused" once queue and metadata are durable; its final checkpoint waits at most
//! [`os_store::writer::CHECKPOINT_READER_WAIT`] for readers. Whatever remains in the WAL is merged here in short,
//! cancellable attempts. Quitting and "Show in Finder" wait for the merge; resuming cancels it first.
use crate::{pool::CrawlKey, session::SessionService, App};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc, Mutex},
    time::Duration,
};
use tokio::sync::watch;

#[derive(Default)]
pub(crate) struct Merges(Mutex<HashMap<CrawlKey, Merge>>);

struct Merge {
    cancel: Arc<AtomicBool>,
    done: watch::Receiver<bool>,
}

impl Merges {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<CrawlKey, Merge>> {
        self.0.lock().unwrap_or_else(|p| p.into_inner())
    }
    /// Whether a merge for this crawl is still running (status line).
    pub(crate) fn active(&self, crawl_id: &str) -> bool {
        self.lock()
            .iter()
            .any(|(k, m)| k.0 == crawl_id && !*m.done.borrow())
    }
}

/// Starts the merge when the WAL still holds frames; a running merge for the crawl is kept.
pub(crate) fn start(app: &App, key: &CrawlKey, path: PathBuf) {
    if os_store::open::wal_bytes(&path) == 0 {
        return;
    }
    let service = app.ext::<SessionService>();
    let mut merges = service.merges.lock();
    if merges.get(key).is_some_and(|m| !*m.done.borrow()) {
        return;
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let (tx, done) = watch::channel(false);
    merges.insert(
        key.clone(),
        Merge {
            cancel: cancel.clone(),
            done,
        },
    );
    drop(merges);
    let weak = app.downgrade();
    let key = key.clone();
    app.handle().spawn(async move {
        let merged =
            tokio::task::spawn_blocking(move || os_store::open::merge_wal(&path, &cancel)).await;
        match merged {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => tracing::warn!(error = %e, "WAL merge failed"),
            Err(e) => tracing::warn!(error = %e, "WAL merge task failed"),
        }
        let _ = tx.send(true);
        if let Some(app) = weak.upgrade() {
            finished(&app, &key);
        }
    });
}

/// Publishes the status without the merge note once the merge has ended.
fn finished(app: &App, key: &CrawlKey) {
    let service = app.ext::<SessionService>();
    let status = service
        .feeds
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(key)
        .and_then(|c| c.status.clone())
        // A stale running status (merge faster than the settle publication) is never republished.
        .filter(|s| s.phase != crate::dto::CrawlPhase::Running);
    if let Some(status) = status {
        service
            .parts
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .crawl = Some(status);
        app.flush_tick();
    }
}

fn receiver(app: &App, key: &CrawlKey) -> Option<watch::Receiver<bool>> {
    app.ext::<SessionService>()
        .merges
        .lock()
        .get(key)
        .map(|m| m.done.clone())
}

async fn done(mut rx: watch::Receiver<bool>) {
    let _ = rx.wait_for(|done| *done).await;
}

/// Waits until the WAL of this crawl has been merged (or the merge was cancelled).
pub async fn wait_merged(app: &App, key: &CrawlKey) {
    if let Some(rx) = receiver(app, key) {
        done(rx).await;
    }
}

/// Waits for every running merge, at most `limit`; `false` if one is still running.
pub async fn wait_all(app: &App, limit: Duration) -> bool {
    let all: Vec<_> = app
        .ext::<SessionService>()
        .merges
        .lock()
        .values()
        .map(|m| m.done.clone())
        .collect();
    tokio::time::timeout(limit, async {
        for rx in all {
            done(rx).await;
        }
    })
    .await
    .is_ok()
}

/// Stops a running merge before the engine writes to the file again (resume) and waits for it.
pub async fn cancel(app: &App, key: &CrawlKey) {
    let rx = {
        let service = app.ext::<SessionService>();
        let merges = service.merges.lock();
        merges.get(key).map(|m| {
            m.cancel.store(true, std::sync::atomic::Ordering::SeqCst);
            m.done.clone()
        })
    };
    if let Some(rx) = rx {
        done(rx).await;
    }
}
