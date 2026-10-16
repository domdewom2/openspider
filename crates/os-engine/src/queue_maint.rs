//! Hintergrundarbeit für das RAM-Fenster der Warteschlange (K-78 b): Auslagern und Nachladen.
//!
//! Die Steuerschleife ([`crate::crawl`]) fragt bei jedem Durchlauf unter der Kern-Sperre
//! [`crate::frontier::Core::next_queue_job`] und startet dann [`run`] als eigenen Task. Die Sperre wird nur für die
//! kurzen Rechenschritte im RAM gehalten. Der Auftrag wartet zuerst, bis der Writer alles bisher Gesendete
//! geschrieben hat (`flush`), und liest dann auf einer eigenen Leseverbindung (`spawn_blocking`) die Zeilenzahl bzw.
//! den nächsten Block der Tabelle `queue`. Es läuft höchstens ein Auftrag zugleich.
//!
//! Schlägt das Nachladen dreimal in Folge fehl, endet der Crawl mit einem Fehler (die ausgelagerten URLs wären sonst
//! verloren). Das Auslagern ist optional: Nach drei Fehlschlägen bleibt die Warteschlange im RAM.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

use os_fetch::Fetch;

use crate::diag;
use crate::error::EngineError;
use crate::queue::{JobData, JobKind, JobPlan};
use crate::queue_disk;
use crate::worker::Shared;

/// Fehlgeschlagene Aufträge in Folge, bevor der Crawl abbricht (Nachladen) bzw. nicht mehr ausgelagert wird.
const MAX_FAILURES: u32 = 3;

/// Dateiarbeit eines Auftrags (ohne Sperre).
pub(crate) fn read(plan: &JobPlan) -> Result<JobData, String> {
    let file = queue_disk::open_ro(&plan.path)?;
    match plan.kind {
        JobKind::Activate => Ok(JobData::Counted(queue_disk::count_rows(&file)?)),
        JobKind::Refill => Ok(JobData::Block(queue_disk::read_chunk(
            &file,
            &plan.after,
            plan.limit,
        )?)),
    }
}

/// Führt den Auftrag aus (als eigener Task der Steuerschleife).
pub(crate) async fn run<F: Fetch>(sh: Arc<Shared<F>>, plan: JobPlan, failures: Arc<AtomicU32>) {
    let kind = plan.kind;
    let result = work(&sh, plan).await;
    let failed = match result {
        Ok(data) => {
            failures.store(0, Ordering::SeqCst);
            sh.core
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .frontier
                .finish_job(data);
            None
        }
        Err(e) => {
            let mut core = sh.core.lock().unwrap_or_else(|p| p.into_inner());
            core.frontier.abort_job(Instant::now());
            let n = failures.fetch_add(1, Ordering::SeqCst) + 1;
            if n >= MAX_FAILURES && kind == JobKind::Activate {
                core.frontier.stop_spilling();
            }
            Some((e, n))
        }
    };
    if let Some((e, n)) = failed {
        tracing::warn!("Warteschlange ({kind:?}): {e}");
        if n >= MAX_FAILURES && kind == JobKind::Refill {
            sh.fail(EngineError::Join(format!(
                "Warteschlange konnte nicht aus der Crawl-Datei nachgeladen werden: {e}"
            )));
        }
    }
    sh.ctl.notify.notify_one();
}

/// Diagnoseschalter `OS_ENGINE_REFILL_NOFLUSH=1` (N22): das Nachladen wartet nicht auf `writer.flush()`.
fn skip_refill_flush() -> bool {
    static SKIP: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *SKIP.get_or_init(|| std::env::var_os("OS_ENGINE_REFILL_NOFLUSH").is_some_and(|v| v == "1"))
}

async fn work<F: Fetch>(sh: &Arc<Shared<F>>, plan: JobPlan) -> Result<JobData, String> {
    let started = Instant::now();
    let kind = plan.kind;
    // Alles, was vor dem Start des Auftrags eingeplant wurde, muss in der Datei stehen.
    if !(kind == JobKind::Refill && skip_refill_flush()) {
        sh.writer.flush().await.map_err(|e| e.to_string())?;
    }
    let wait_ns = diag::nanos(started);
    let read_started = Instant::now();
    let data = tokio::task::spawn_blocking(move || read(&plan))
        .await
        .map_err(|e| e.to_string())??;
    let read_ns = diag::nanos(read_started);
    match (&data, kind) {
        (JobData::Block(rows), JobKind::Refill) => {
            let index = diag::STATS.q_refill_jobs.fetch_add(1, Ordering::Relaxed);
            diag::record_refill(
                index,
                diag::nanos(started),
                wait_ns,
                read_ns,
                rows.len() as u64,
            );
        }
        (_, JobKind::Activate) => {
            diag::inc(&diag::STATS.q_activate_n);
            diag::STATS
                .q_activate_ns
                .fetch_add(diag::nanos(started), Ordering::Relaxed);
            diag::STATS
                .q_activate_wait_ns
                .fetch_add(wait_ns, Ordering::Relaxed);
        }
        _ => {}
    }
    Ok(data)
}
