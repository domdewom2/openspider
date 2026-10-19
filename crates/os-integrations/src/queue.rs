//! Unabhängige Quellenwarteschlangen, Pause und Abschlussbarriere.
use crate::{ApiCtx, ApiError, ApiSource, ApiState, ApiStatus, UrlRef};
use futures_util::{stream, StreamExt};
use os_model::{ApiValueWrite, Timestamp};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tokio::sync::Notify;

#[derive(Default)]
pub struct Queue {
    paused: AtomicBool,
    wake: Notify,
    busy: AtomicBool,
    finished: Notify,
    status: Mutex<Option<ApiStatus>>,
}
impl Queue {
    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
    }
    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
        self.wake.notify_waiters();
    }
    pub fn status(&self) -> Option<ApiStatus> {
        self.status
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
    pub async fn wait(&self) {
        loop {
            let notified = self.finished.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if !self.busy.load(Ordering::SeqCst) {
                return;
            }
            notified.await;
        }
    }
    pub async fn ready(&self) {
        loop {
            let notified = self.wake.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if !self.paused.load(Ordering::SeqCst) {
                return;
            }
            notified.await;
        }
    }
    /// Einzel-URLs werden unabhängig abgeschlossen; Neuanforderung ersetzt den alten Fehlerstand.
    pub async fn run(
        &self,
        source: Arc<dyn ApiSource>,
        urls: &[UrlRef],
        ctx: &ApiCtx,
        concurrency: usize,
    ) -> Result<QueueResult, ApiError> {
        let mut result = QueueResult::default();
        self.run_with(source, urls, ctx, concurrency, |url, outcome, _| {
            match outcome {
                Ok(values) => {
                    result.values.extend(values);
                    result.succeeded.push(url);
                }
                Err(e) => result.errors.push((url, e)),
            }
            std::future::ready(Ok(()))
        })
        .await?;
        Ok(result)
    }
    /// Speichert jede abgeschlossene URL vor der Freigabe der Abschlussbarriere.
    pub async fn run_with<F, Fut>(
        &self,
        source: Arc<dyn ApiSource>,
        urls: &[UrlRef],
        ctx: &ApiCtx,
        concurrency: usize,
        mut complete: F,
    ) -> Result<(), ApiError>
    where
        F: FnMut(UrlRef, Result<Vec<ApiValueWrite>, ApiError>, ApiStatus) -> Fut,
        Fut: std::future::Future<Output = Result<(), ApiError>>,
    {
        if self.busy.swap(true, Ordering::SeqCst) {
            return Err(ApiError::Transport("API-Abruf läuft bereits".into()));
        }
        let _guard = RunGuard(self);
        let mut unique = BTreeMap::new();
        for u in urls {
            unique.insert(u.url.clone(), u.clone());
        }
        let total = unique.len() as u64;
        *self.status.lock().unwrap_or_else(|p| p.into_inner()) = Some(ApiStatus {
            kind: source.kind(),
            state: ApiState::Fetching,
            urls_total: total,
            urls_done: 0,
            urls_failed: 0,
            detail: None,
        });
        let mut requests = stream::iter(unique.into_values().map(|url| {
            let source = source.clone();
            async move {
                self.ready().await;
                let result = source.fetch(std::slice::from_ref(&url), ctx).await;
                (url, result)
            }
        }))
        .buffer_unordered(concurrency.clamp(1, 50));
        // Ein begrenzter Kanal entkoppelt Netzwerk-Polling von langsamen Store-Commits.
        // try_join beendet beim Schreibfehler auch den Produzenten und lässt keine Tasks zurück.
        let (tx, mut rx) = tokio::sync::mpsc::channel(concurrency.clamp(1, 50));
        let produce = async move {
            while let Some(result) = requests.next().await {
                tx.send(result)
                    .await
                    .map_err(|_| ApiError::Transport("API-Ergebniskanal geschlossen".into()))?;
            }
            Ok::<_, ApiError>(())
        };
        let consume = async {
            while let Some((url, outcome)) = rx.recv().await {
                let current = {
                    let mut status = self.status.lock().unwrap_or_else(|p| p.into_inner());
                    let status = status.as_mut().expect("initialized");
                    status.urls_done += 1;
                    status.urls_failed += u64::from(outcome.is_err());
                    if status.urls_done == status.urls_total {
                        status.state = ApiState::Idle;
                    }
                    status.clone()
                };
                complete(url, outcome, current).await?;
            }
            Ok::<_, ApiError>(())
        };
        tokio::try_join!(produce, consume)?;
        if let Some(status) = self
            .status
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_mut()
        {
            status.state = ApiState::Idle;
        }
        Ok(())
    }
}
struct RunGuard<'a>(&'a Queue);
impl Drop for RunGuard<'_> {
    fn drop(&mut self) {
        if let Some(status) = self
            .0
            .status
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_mut()
        {
            if status.state == ApiState::Fetching {
                status.state = ApiState::Paused;
                status.detail = Some("Abruf unterbrochen".into());
            }
        }
        self.0.busy.store(false, Ordering::SeqCst);
        self.0.finished.notify_waiters();
    }
}
#[derive(Default)]
pub struct QueueResult {
    pub values: Vec<ApiValueWrite>,
    pub succeeded: Vec<UrlRef>,
    pub errors: Vec<(UrlRef, ApiError)>,
}

/// Google-Tagesgrenze: US-Pazifik mit den seit 2007 geltenden DST-Regeln.
pub fn pacific_day(now: Timestamp) -> String {
    pacific_date(now).to_string()
}
pub(crate) fn pacific_date(now: Timestamp) -> time::Date {
    use time::{Date, Month};
    let year = now.0.year();
    let sunday = |month: Month, n: u8| {
        let first = Date::from_calendar_date(year, month, 1).expect("valid month");
        let delta = (7 - first.weekday().number_days_from_sunday()) % 7;
        Date::from_calendar_date(year, month, 1 + delta + (n - 1) * 7).expect("valid DST date")
    };
    let start = sunday(Month::March, 2)
        .with_hms(10, 0, 0)
        .expect("DST start")
        .assume_utc();
    let end = sunday(Month::November, 1)
        .with_hms(9, 0, 0)
        .expect("DST end")
        .assume_utc();
    let offset = if now.0 >= start && now.0 < end {
        -7
    } else {
        -8
    };
    (now.0 + time::Duration::hours(offset)).date()
}
