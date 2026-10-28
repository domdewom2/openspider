//! Snapshot-consistent, bounded feed; overflow and reset_seq request a rebuild.
use super::SessionService;
use crate::{dto::*, pool::CrawlKey, runtime::sessions::SessionInfo, App, CmdError};
use os_model::{CrawlState, Progress, UrlId};
use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

pub const CHANGE_LIMIT: u32 = 4096;
#[derive(Clone, Default)]
pub(crate) struct Cursor {
    seq: u64,
    reset: u64,
    last: Option<Duration>,
    rows_at: Option<Duration>,
    pending: Option<ChangeSet>,
    pub(super) status: Option<CrawlStatus>,
    delays: std::collections::HashMap<String, (f64, bool)>,
}
#[derive(Clone, Debug)]
pub struct ChangeSet {
    pub key: CrawlKey,
    pub seq: u64,
    pub ids: Vec<UrlId>,
    /// Deletion, new session or batch larger than CHANGE_LIMIT: rebuild the view.
    pub reset: bool,
}
pub type ChangeListener = dyn Fn(&App, &ChangeSet) + Send + Sync;
pub fn subscribe(app: &App, f: Arc<ChangeListener>) {
    app.ext::<SessionService>()
        .listeners
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .push(f);
}

pub fn phase(s: CrawlState) -> CrawlPhase {
    match s {
        CrawlState::Running => CrawlPhase::Running,
        CrawlState::Paused => CrawlPhase::Paused,
        CrawlState::Completed => CrawlPhase::Completed,
        CrawlState::Incomplete => CrawlPhase::Incomplete,
        CrawlState::Failed => CrawlPhase::Failed,
    }
}
/// Prozent eines abgeschlossenen Crawls (UI-013: 100 % erst bei leerer Warteschlange). Ein gestoppter Crawl
/// (`EndReason::Stopped`) mit Rest zeigt den Anteil der erledigten URLs, höchstens 99; andere Gründe (Warteschlange
/// leer, Grenze erreicht) behalten den Wert der Engine, eine leere Warteschlange ergibt 100.
pub(crate) fn completed_percent(
    engine: u8,
    end_reason: Option<os_model::EndReason>,
    crawled: u64,
    queued: u64,
) -> u8 {
    if queued == 0 {
        return 100;
    }
    if end_reason == Some(os_model::EndReason::Stopped) {
        let total = crawled.saturating_add(queued);
        return (u128::from(crawled) * 100 / u128::from(total)).min(99) as u8;
    }
    engine
}
pub fn mode(m: os_model::CrawlMode) -> CrawlModeDto {
    match m {
        os_model::CrawlMode::Spider => CrawlModeDto::Spider,
        os_model::CrawlMode::List => CrawlModeDto::List,
        os_model::CrawlMode::Serp => CrawlModeDto::Serp,
    }
}
pub fn from_progress(id: String, m: CrawlModeDto, owner: CrawlOwner, p: &Progress) -> CrawlStatus {
    CrawlStatus {
        crawl_id: id,
        phase: phase(p.state),
        mode: m,
        owner,
        end_reason: p.end_reason.map(|r| r.as_str().into()),
        crawled: p.crawled,
        queued: p.queued,
        discovered: p.discovered,
        percent: p.percent,
        rate: p.rate,
        avg_rate: p.avg_rate,
        avg_response_time: p.avg_response_time,
        elapsed_s: p.elapsed_s,
        paused_s: p.paused_s,
        eta_s: p.eta_s,
        active_threads: p.active_threads,
        memory_stage: p.memory_stage.map(|s| match s {
            os_model::MemoryStage::Normal => MemoryStageDto::Normal,
            os_model::MemoryStage::Warning => MemoryStageDto::Warning,
            os_model::MemoryStage::Paused => MemoryStageDto::Paused,
        }),
        file_bytes: p.file_bytes,
        memory_used_bytes: p.memory_used_bytes,
        memory_limit_bytes: p.memory_limit_bytes,
    }
}

/// Read metadata through the pool; never opens a writing connection for foreign crawls.
pub async fn snapshot(
    app: &App,
    key: CrawlKey,
) -> Result<(CrawlSummaryDto, os_model::CrawlConfig, CrawlState), CmdError> {
    let clock = app.config().clock.clone();
    app.read_pool()
        .run(key, move |f, _| {
            let m = f.meta()?;
            let state = f.effective_state(clock.as_ref())?;
            let pid = f.get_meta("owner_pid")?.and_then(|s| s.parse().ok());
            let live = state == CrawlState::Running;
            let owner = CrawlOwner {
                kind: match f.get_meta("owner_kind")?.as_deref() {
                    Some("cli") => OwnerKind::Cli,
                    Some("mcp-stdio") => OwnerKind::Mcp,
                    Some("scheduler") => OwnerKind::Scheduler,
                    _ => OwnerKind::App,
                },
                pid,
                live,
                controllable: !live || pid == Some(std::process::id()),
            };
            let urls = os_store::queries::session_counts(f.conn())?.discovered;
            let file_bytes = std::fs::metadata(f.path())
                .map_err(|e| os_store::StoreError::Io {
                    path: f.path().to_path_buf(),
                    source: e,
                })?
                .len();
            Ok((
                CrawlSummaryDto {
                    crawl_id: m.crawl_id.to_string(),
                    file: f.path().to_string_lossy().into_owned(),
                    name: m.name,
                    phase: phase(state),
                    mode: if f.get_meta("source_mode")?.as_deref() == Some("apis") {
                        CrawlModeDto::Apis
                    } else {
                        mode(m.mode)
                    },
                    started_at: m.started_at.unwrap_or_default().to_db(),
                    finished_at: m.ended_at.map(|t| t.to_db()),
                    urls,
                    file_bytes,
                    schema_version: m.schema_version,
                    owner,
                },
                f.config()?,
                state,
            ))
        })
        .await
}

pub async fn refresh(app: &App, s: &SessionInfo) -> Result<(), CmdError> {
    let service = app.ext::<SessionService>();
    let mut old = service
        .feeds
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(&s.key)
        .cloned()
        .unwrap_or_default();
    let now = app.config().clock.monotonic();
    if old
        .last
        .is_some_and(|t| now.saturating_sub(t) < Duration::from_millis(50))
    {
        return Ok(());
    }
    let before = old.clone();
    let clock = app.config().clock.clone();
    let cfg = s.config.clone();
    let progress = s.engine.as_ref().and_then(|e| {
        e.handle
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .map(|h| h.progress())
    });
    let (seq, reset, ids, mut status) = app
        .read_pool()
        .run(s.key.clone(), move |f, _| {
            let tx = f.conn().unchecked_transaction()?;
            let seq = os_store::queries::change_seq(&tx)?;
            let reset = f
                .get_meta("reset_seq")?
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let ids = if seq != before.seq {
                os_store::queries::changed_since(&tx, before.seq, CHANGE_LIMIT + 1)?
            } else {
                Vec::new()
            };
            let m = f.meta()?;
            // Resume returns before its first metadata batch is committed. The
            // local handle is authoritative until its durable completion is joined.
            let local = progress.is_some();
            let state = match &progress {
                Some(p) => p.state,
                None => f.effective_state(clock.as_ref())?,
            };
            let end_reason = match &progress {
                Some(p) => p.end_reason,
                None => m.end_reason,
            };
            let pid = if local {
                Some(std::process::id())
            } else {
                f.get_meta("owner_pid")?.and_then(|v| v.parse().ok())
            };
            let owner = CrawlOwner {
                kind: match f.get_meta("owner_kind")?.as_deref() {
                    Some("cli") => OwnerKind::Cli,
                    Some("mcp-stdio") => OwnerKind::Mcp,
                    Some("scheduler") => OwnerKind::Scheduler,
                    _ => OwnerKind::App,
                },
                pid,
                live: state == CrawlState::Running,
                controllable: state != CrawlState::Running || pid == Some(std::process::id()),
            };
            let p = progress.or(f
                .get_meta("progress_snapshot")?
                .and_then(|v| serde_json::from_str::<Progress>(&v).ok()));
            let mut status = if let Some(p) = p {
                from_progress(m.crawl_id.to_string(), mode(m.mode), owner, &p)
            } else {
                let mut st = before.status.unwrap_or(CrawlStatus {
                    crawl_id: m.crawl_id.to_string(),
                    phase: phase(state),
                    mode: if f.get_meta("source_mode")?.as_deref() == Some("apis") {
                        CrawlModeDto::Apis
                    } else {
                        mode(m.mode)
                    },
                    end_reason: None,
                    crawled: 0,
                    queued: 0,
                    discovered: 0,
                    percent: 0,
                    rate: 0.0,
                    avg_rate: 0.0,
                    avg_response_time: None,
                    elapsed_s: 0,
                    paused_s: 0,
                    eta_s: None,
                    active_threads: 0,
                    memory_stage: None,
                    file_bytes: None,
                    owner: owner.clone(),
                    memory_used_bytes: None,
                    memory_limit_bytes: None,
                });
                if before.last.is_none() || seq != before.seq {
                    let counts = os_store::queries::session_counts(&tx)?;
                    st.discovered = counts.discovered;
                    st.queued = counts.queued;
                    st.crawled = counts.crawled;
                }
                st.owner = owner;
                st
            };
            if state != CrawlState::Running && status.phase == CrawlPhase::Running {
                let counts = os_store::queries::session_counts(&tx)?;
                status.discovered = counts.discovered;
                status.queued = counts.queued;
                status.crawled = counts.crawled;
            }
            if state == CrawlState::Completed {
                status.percent =
                    completed_percent(status.percent, end_reason, status.crawled, status.queued);
            }
            if f.get_meta("source_mode")?.as_deref() == Some("apis") {
                status.mode = CrawlModeDto::Apis;
            }
            status.phase = phase(state);
            status.end_reason = end_reason.map(|r| r.as_str().into());
            if state != CrawlState::Running {
                status.active_threads = 0;
                status.rate = 0.0;
            }
            tx.commit()?;
            if seq != before.seq && ids.len() <= CHANGE_LIMIT as usize {
                // K03 maintains its read-side aggregate cache with its own transaction,
                // directly after the snapshot read above has been closed (nested
                // transactions are not possible on one connection).
                os_analyze::aggregates::live::apply_changes(f, &cfg, &ids)
                    .map_err(|e| os_store::StoreError::InvalidInput(e.to_string()))?;
            }
            Ok((seq, reset, ids, status))
        })
        .await?;
    status.owner.controllable &= s.writable;
    let state = match status.phase {
        CrawlPhase::Running => CrawlState::Running,
        CrawlPhase::Paused => CrawlState::Paused,
        CrawlPhase::Completed => CrawlState::Completed,
        CrawlPhase::Failed => CrawlState::Failed,
        _ => CrawlState::Incomplete,
    };
    super::state(app, &s.key, state);
    let changed = old.last.is_none() || old.seq != seq || old.reset != reset;
    if changed {
        let token = os_fetch::user_agents::resolve(&s.config.user_agent).robots_token;
        let bodies = app
            .read_pool()
            .run(s.key.clone(), |f, _| {
                os_analyze::aggregates::status::robots_bodies(f)
            })
            .await?;
        for (origin, body) in bodies {
            let Some(seconds) = os_fetch::robots::parse(&body).crawl_delay(&token) else {
                continue;
            };
            let applied = s.config.robots.respect_crawl_delay;
            if old.delays.insert(origin.clone(), (seconds, applied)) != Some((seconds, applied)) {
                app.push_notice(Notice {
                    level: NoticeLevel::Info,
                    code: "crawl-delay".into(),
                    args: [
                        ("origin".into(), serde_json::json!(origin)),
                        ("seconds".into(), serde_json::json!(seconds)),
                        ("applied".into(), serde_json::json!(applied)),
                    ]
                    .into(),
                });
            }
        }
        let rebuild = old.last.is_none()
            || old.reset != reset
            || seq < old.seq
            || ids.len() > CHANGE_LIMIT as usize;
        let change = ChangeSet {
            key: s.key.clone(),
            seq,
            ids: if rebuild { Vec::new() } else { ids },
            reset: rebuild,
        };
        if let Some(pending) = old.pending.as_mut() {
            pending.seq = change.seq;
            pending.reset |= change.reset;
            if !pending.reset {
                pending.ids.extend(change.ids);
                pending.ids.sort_unstable();
                pending.ids.dedup();
                pending.reset = pending.ids.len() > CHANGE_LIMIT as usize;
            }
            if pending.reset {
                pending.ids.clear();
            }
        } else {
            old.pending = Some(change);
        }
        let version = service.version.fetch_add(1, Ordering::SeqCst) + 1;
        let mut parts = service.parts.lock().unwrap_or_else(|p| p.into_inner());
        parts.counts_version = Some(version);
        parts.issues_version = Some(version);
        parts.sidebar_version = Some(version);
    }
    if old.status.as_ref() != Some(&status) {
        service
            .parts
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .crawl = Some(status.clone());
    }
    old.status = Some(status);
    old.seq = seq;
    old.reset = reset;
    old.last = Some(now);
    // Counters above are independent of the configurable table cadence (K2-39).
    // Accumulate IDs between row publications; overflow becomes one full reset.
    if old.pending.is_some()
        && old.rows_at.is_none_or(|t| {
            now.saturating_sub(t) >= Duration::from_millis(u64::from(app.live_interval_ms()))
        })
    {
        let change = old.pending.take().expect("pending change");
        let listeners = service
            .listeners
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        for f in listeners {
            f(app, &change);
        }
        old.rows_at = Some(now);
    }
    service
        .feeds
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(s.key.clone(), old);
    Ok(())
}

pub fn invalidate(app: &App, key: &CrawlKey) {
    if let Some(cursor) = app
        .ext::<SessionService>()
        .feeds
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get_mut(key)
    {
        cursor.last = None;
    }
}

#[cfg(test)]
mod tests {
    use super::completed_percent;
    use os_model::EndReason;

    // spec: UI-013
    #[test]
    fn a_stopped_crawl_with_a_remaining_queue_never_reports_one_hundred_percent() {
        assert_eq!(completed_percent(100, Some(EndReason::Stopped), 11, 16), 40);
        assert_eq!(completed_percent(100, Some(EndReason::Stopped), 999, 1), 99);
        assert_eq!(completed_percent(100, Some(EndReason::Stopped), 11, 0), 100);
        assert_eq!(
            completed_percent(100, Some(EndReason::QueueEmpty), 27, 0),
            100
        );
        // Grenze erreicht: der Wert der Engine bleibt (CRAWL-130)
        assert_eq!(
            completed_percent(100, Some(EndReason::LimitReached), 10, 5),
            100
        );
    }
}
