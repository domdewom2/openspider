//! View-Sitzungen der App auf den Bitmaps und Permutationen von `os-analyze` (K-1, K-20).
//!
//! Jede Tab-View (`ViewSpec::Tab`) besitzt eine [`ViewSession`] aus `os-analyze`. Aufbau und Pflege laufen auf den
//! schreibgeschützten Verbindungen des Lese-Pools, nie auf einem Tokio-Worker (2.3 Regel 1); `view_cache` bleibt
//! dadurch ein reiner Speicher-Cache (K-1a „bei schreibgeschützter Datei nur im Speicher“).
//!
//! * **Doppelpuffer:** Zeilen kommen immer aus dem zuletzt veröffentlichten [`ViewSnapshot`]. Ein laufender,
//!   fehlgeschlagener oder abgebrochener Neuaufbau ändert die sichtbare View nicht; ein Abbruch gibt sofort zurück,
//!   ohne auf die unterbrochene SQLite-Anweisung zu warten (UI-141, ≤ 100 ms).
//! * **Version:** `ViewHandle.version` bleibt der `change_seq`, auf dem der veröffentlichte Stand beruht (gleiche API
//!   wie M2-K06; `export.view` vergleicht genau diesen Wert). Die interne Generation der Kernsitzung wird getrennt
//!   geführt.
//! * **Pflege (K-1c):** Der Änderungsfeed der Sitzung (`session::feed`) liefert geänderte `UrlId`s im Takt der
//!   Tabellen (K2-39); betroffene Views werden über `ViewSession::apply_changes` nachgeführt bzw. bei `reset`
//!   neu aufgebaut und als `ViewChanged` im nächsten Tick gemeldet (UI-018, UI-142).
//! * **Freigabe:** Nach `Budget::idle_release` (60 s) ohne Zugriff gibt eine View ihren Speicher frei, der nächste
//!   Zugriff baut lazy wieder auf (UI-142); das Gesamtbudget setzt `release_unused` aus `os-analyze` durch.
//! * **Segmente/Vergleich:** [`RestrictProvider`] liefert Bitmaps (`FilterSource`) für `segment`/`compare`
//!   (Schnittstelle für M2-K14/M2-C01); ohne Anbieter bleiben beide wie bisher `Unsupported`.
//!
//! Der seitenweise Cursor aus `ViewSession::page` ist prozesslokal und wird hier nicht verwendet: die App adressiert
//! Zeilen per Offset und Version; MCP/CLI behalten den M1-Cursor.

use crate::{
    commands::{
        catalog::{expanded_columns, view_error},
        view::session_spec,
    },
    dto::*,
    events::{TickParts, TickSource},
    pool::CrawlKey,
    runtime::{error::internal, sessions::SessionInfo, App},
    session::feed::ChangeSet,
};
use os_analyze::{
    search::FilterSource,
    views::session::{release_unused, Budget, BuildControl, RowsReq, ViewSession, ViewSnapshot},
};
use os_model::{time::Clock, CancelToken, FilterState, UrlId, Value};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, Weak,
    },
    time::Duration,
};

/// Bitmap-Lieferant für Segment- (K-14) und Vergleichseinschränkungen (K-15) einer Tab-View.
pub trait RestrictProvider: Send + Sync {
    /// Einschränkungen für `crawl`; ein Fehler lehnt `view.open`/`view.update` ab.
    fn restrict(
        &self,
        crawl: &CrawlKey,
        segment: Option<&SegmentFilter>,
        compare: Option<&CompareClass>,
    ) -> Result<Vec<Arc<dyn FilterSource>>, CmdError>;
}

/// Meldet den Anbieter für `segment`/`compare` an (ersetzt einen früheren).
pub fn set_restrict_provider(app: &App, provider: Arc<dyn RestrictProvider>) {
    *lock(&shared(app).provider) = Some(provider);
}

/// Setzt das Speicherbudget für alle künftig geöffneten Views (Standard 512 MB, 60 s Leerlauf).
pub fn set_budget(app: &App, budget: Budget) {
    *lock(&shared(app).budget) = Some(budget);
}

/// Einschränkungen einer Tab-View; ohne Anbieter wie M2-K06 `Unsupported`.
pub(crate) fn restrictions(
    app: &App,
    crawl: &CrawlKey,
    segment: Option<&SegmentFilter>,
    compare: Option<&CompareClass>,
) -> Result<Vec<Arc<dyn FilterSource>>, CmdError> {
    if segment.is_none() && compare.is_none() {
        return Ok(Vec::new());
    }
    let provider = lock(&shared(app).provider).clone();
    match provider {
        Some(p) => p.restrict(crawl, segment, compare),
        None => crate::commands::catalog::check_scope(&segment.cloned(), &compare.cloned())
            .map(|_| Vec::new()),
    }
}

/// Gibt Views frei, die länger als ihr Leerlaufbudget nicht benutzt wurden, und setzt danach das Gesamtbudget durch.
/// Läuft sekündlich im Hintergrund; öffentlich für Tests und Wartung.
pub fn release_idle(app: &App) {
    let shared = shared(app);
    let views = live(&shared);
    let now = app.config().clock.monotonic();
    let mut max = Budget::default().max_bytes;
    for view in &views {
        max = view.budget.max_bytes;
        let mut state = lock(&view.state);
        if state.snapshot.is_some() && now.saturating_sub(state.used) >= view.budget.idle_release {
            state.snapshot = None;
        }
    }
    release_unused(max);
}

/// Belegter Speicher aller Views der App (Positionslisten, Umkehrabbildungen, Bitmaps, Permutationen).
pub fn retained_bytes(app: &App) -> u64 {
    live(&shared(app))
        .iter()
        .map(|v| {
            let held = lock(&v.state).snapshot.is_some();
            // try_lock: a running build must not block the measurement
            let core = v
                .session
                .try_lock()
                .ok()
                .and_then(|s| s.as_ref().map(ViewSession::memory_bytes))
                .unwrap_or(0);
            core + u64::from(held)
        })
        .sum()
}

#[derive(Default)]
struct Shared {
    init: AtomicBool,
    provider: Mutex<Option<Arc<dyn RestrictProvider>>>,
    budget: Mutex<Option<Budget>>,
    views: Mutex<Vec<Weak<TabView>>>,
    changed: Mutex<Vec<ViewChanged>>,
}
impl TickSource for Shared {
    fn collect(&self, parts: &mut TickParts) {
        for change in std::mem::take(&mut *lock(&self.changed)) {
            parts.views.retain(|v| v.view_id != change.view_id);
            parts.views.push(change);
        }
    }
}
fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}
fn shared(app: &App) -> Arc<Shared> {
    let shared = app.ext::<Shared>();
    if !shared.init.swap(true, Ordering::SeqCst) {
        app.add_tick_source(shared.clone());
        crate::session::feed::subscribe(app, Arc::new(on_change));
        let weak = app.downgrade();
        app.handle().spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                let Some(app) = weak.upgrade() else { break };
                release_idle(&app);
            }
        });
    }
    shared
}
fn live(shared: &Shared) -> Vec<Arc<TabView>> {
    let mut views = lock(&shared.views);
    views.retain(|v| v.strong_count() > 0);
    views.iter().filter_map(Weak::upgrade).collect()
}

/// Veröffentlichter Stand einer Tab-View.
#[derive(Clone)]
pub(crate) struct Published {
    pub session: SessionInfo,
    pub spec: ViewSpec,
    /// Angefragte Spalten (`view.open`).
    pub columns: Vec<String>,
    /// Spalten der Projektion.
    pub meta: Vec<ColumnMeta>,
    pub version: u64,
    pub total: u64,
}
/// Ergebnis eines Aufbaus.
pub(crate) struct Built {
    pub version: u64,
    pub total: u64,
    pub meta: Vec<ColumnMeta>,
    pub state: FilterState,
}
/// Ein Zeilenfenster aus dem veröffentlichten Stand.
pub(crate) struct Window {
    pub meta: Vec<ColumnMeta>,
    pub rows: Vec<Vec<Value>>,
    pub keys: Vec<i64>,
}

struct State {
    published: Option<Published>,
    snapshot: Option<ViewSnapshot>,
    /// Generation der Kernsitzung, zu der `snapshot` gehört.
    core: u64,
    used: Duration,
}
#[derive(Default)]
struct Pending {
    ids: Vec<UrlId>,
    reset: bool,
}

/// Sitzung einer Tab-View der App.
pub(crate) struct TabView {
    view_id: String,
    key: CrawlKey,
    clock: Arc<dyn Clock>,
    budget: Budget,
    session: Arc<Mutex<Option<ViewSession>>>,
    builder: tokio::sync::Mutex<()>,
    state: Mutex<State>,
    pending: Mutex<Option<Pending>>,
    worker: AtomicBool,
}

impl TabView {
    /// Neue, noch nicht aufgebaute View (der Aufbau folgt mit [`TabView::build`]).
    pub fn new(app: &App, view_id: String, key: CrawlKey) -> Arc<TabView> {
        let shared = shared(app);
        let budget = lock(&shared.budget).unwrap_or_default();
        let view = Arc::new(TabView {
            view_id,
            key,
            clock: app.config().clock.clone(),
            budget,
            session: Arc::new(Mutex::new(None)),
            builder: tokio::sync::Mutex::new(()),
            state: Mutex::new(State {
                published: None,
                snapshot: None,
                core: 0,
                used: app.config().clock.monotonic(),
            }),
            pending: Mutex::new(None),
            worker: AtomicBool::new(false),
        });
        lock(&shared.views).push(Arc::downgrade(&view));
        view
    }

    pub fn published(&self) -> Option<Published> {
        lock(&self.state).published.clone()
    }

    /// Baut `spec` privat auf und veröffentlicht erst bei Erfolg (Doppelpuffer). Ein Abbruch über `token` gibt
    /// sofort `Cancelled` zurück; die bisherige View bleibt unverändert lesbar.
    pub async fn build(
        &self,
        app: &App,
        session: SessionInfo,
        spec: ViewSpec,
        columns: Vec<String>,
        token: CancelToken,
    ) -> Result<Built, CmdError> {
        let core_spec = session_spec(app, &session, &spec, &columns, token.clone())?;
        let _builder = self.builder.lock().await;
        let slot = self.session.clone();
        let cfg = session.config.clone();
        let clock = self.clock.clone();
        let budget = self.budget;
        let control = BuildControl {
            cancel: token.clone(),
        };
        let (tab, filter) = match &spec {
            ViewSpec::Tab { tab, filter, .. } => (tab.clone(), filter.clone()),
            _ => return Err(CmdError::invalid_input("err-tab-required", "spec")),
        };
        let work = app
            .read_pool()
            .run_with(session.key.clone(), token.clone(), move |file, _| {
                let mut guard = lock(&slot);
                let result = (|| {
                    match guard.as_mut() {
                        Some(s) => s.update_controlled(file, &cfg, core_spec, &control)?,
                        None => {
                            let s =
                                ViewSession::open_with_clock(file, &cfg, core_spec, budget, clock)?;
                            s.prepare(file, &control)?;
                            *guard = Some(s);
                        }
                    }
                    let s = guard.as_ref().expect("session");
                    let snapshot = s.snapshot(file)?;
                    let head = snapshot.rows(
                        file,
                        RowsReq {
                            version: s.version(),
                            offset: 0,
                            limit: 1,
                        },
                    )?;
                    let registry = os_analyze::tabs::configured_for_file(&cfg, file);
                    let state =
                        os_analyze::views::state_of_in(&registry, file, &cfg, &tab, &filter)?;
                    let def = os_analyze::names::find_tab(&registry, &tab).expect("validated tab");
                    let meta = expanded_columns(def, &head.columns, &cfg);
                    let seq = s.built_seq().unwrap_or(0);
                    Ok::<_, os_analyze::ViewError>((
                        snapshot,
                        s.version(),
                        seq,
                        head.total,
                        meta,
                        state,
                    ))
                })();
                Ok(result)
            });
        let (snapshot, core, version, total, meta, state) =
            until_cancelled(&token, work).await??.map_err(view_error)?;
        let built = Built {
            version,
            total,
            meta: meta.clone(),
            state,
        };
        let mut st = lock(&self.state);
        st.snapshot = Some(snapshot);
        st.core = core;
        st.used = self.clock.monotonic();
        st.published = Some(Published {
            session,
            spec,
            columns,
            meta,
            version,
            total,
        });
        Ok(built)
    }

    /// Veröffentlichter Stand mit gültigem Schnappschuss; nach einer Leerlauf-Freigabe lazy neu aufgebaut. Hat sich
    /// der Datenstand inzwischen geändert, wird der neue Stand veröffentlicht und `Stale` gemeldet.
    async fn snapshot(&self, app: &App, version: u64) -> Result<(ViewSnapshot, u64), CmdError> {
        {
            let mut st = lock(&self.state);
            if st.published.as_ref().map(|p| p.version) != Some(version) {
                return Err(crate::commands::view::stale());
            }
            st.used = self.clock.monotonic();
            if let Some(s) = &st.snapshot {
                return Ok((s.clone(), st.core));
            }
        }
        let _builder = self.builder.lock().await;
        let slot = self.session.clone();
        let (snapshot, core, seq, total) = app
            .read_pool()
            .run(self.key.clone(), move |file, _| {
                let guard = lock(&slot);
                let s = guard.as_ref().ok_or_else(|| {
                    os_store::StoreError::InvalidInput("View ohne Sitzung".into())
                })?;
                Ok((|| {
                    let snapshot = s.snapshot(file)?;
                    let total = snapshot
                        .rows(
                            file,
                            RowsReq {
                                version: s.version(),
                                offset: 0,
                                limit: 1,
                            },
                        )?
                        .total;
                    Ok::<_, os_analyze::ViewError>((
                        snapshot,
                        s.version(),
                        s.built_seq().unwrap_or(0),
                        total,
                    ))
                })())
            })
            .await?
            .map_err(view_error)?;
        let mut st = lock(&self.state);
        st.snapshot = Some(snapshot.clone());
        st.core = core;
        st.used = self.clock.monotonic();
        let published = st.published.as_mut().expect("published");
        if published.version != seq || published.total != total {
            published.version = seq;
            published.total = total;
            drop(st);
            self.announce(app, seq, total);
            return Err(crate::commands::view::stale());
        }
        Ok((snapshot, core))
    }

    /// Zeilenfenster der veröffentlichten Version `version`.
    pub async fn rows(
        &self,
        app: &App,
        version: u64,
        offset: u64,
        limit: u32,
    ) -> Result<Window, CmdError> {
        if !(1..=500).contains(&limit) {
            return Err(CmdError::invalid_input("err-invalid-limit", "limit"));
        }
        let (snapshot, core) = self.snapshot(app, version).await?;
        let meta = self.published().map(|p| p.meta).unwrap_or_default();
        app.read_pool()
            .run(self.key.clone(), move |file, _| {
                Ok(snapshot.rows(
                    file,
                    RowsReq {
                        version: core,
                        offset,
                        limit,
                    },
                ))
            })
            .await?
            .map(|r| Window {
                meta,
                rows: r.rows,
                keys: r.row_keys.iter().map(|id| id.0).collect(),
            })
            .map_err(view_error)
    }

    /// Zeilenindex einer URL (Umkehrabbildung, ohne Datenbankzugriff).
    pub async fn locate(
        &self,
        app: &App,
        version: u64,
        url_id: i64,
    ) -> Result<Option<u64>, CmdError> {
        let (snapshot, _) = self.snapshot(app, version).await?;
        Ok(snapshot.locate(UrlId(url_id)))
    }

    fn announce(&self, app: &App, version: u64, total: u64) {
        lock(&shared(app).changed).push(ViewChanged {
            view_id: self.view_id.clone(),
            version,
            total: Some(total),
        });
    }

    /// Merkt Änderungen vor und startet höchstens einen Pflege-Lauf je View.
    fn enqueue(self: &Arc<Self>, app: &App, change: &ChangeSet) {
        {
            let mut pending = lock(&self.pending);
            let p = pending.get_or_insert_with(Pending::default);
            p.reset |= change.reset;
            if p.reset {
                p.ids.clear();
            } else {
                p.ids.extend_from_slice(&change.ids);
            }
        }
        if self.worker.swap(true, Ordering::SeqCst) {
            return;
        }
        let view = self.clone();
        let weak = app.downgrade();
        app.handle().spawn(async move {
            loop {
                let Some(app) = weak.upgrade() else { break };
                let next = lock(&view.pending).take();
                let Some(next) = next else {
                    view.worker.store(false, Ordering::SeqCst);
                    // A change queued after `take` but before the flag reset starts its own worker or is seen here.
                    if lock(&view.pending).is_some() && !view.worker.swap(true, Ordering::SeqCst) {
                        continue;
                    }
                    break;
                };
                if let Err(e) = view.maintain(&app, next).await {
                    tracing::warn!(error = ?e, view = %view.view_id, "View maintenance failed");
                }
            }
        });
    }

    /// K-1c: geänderte URLs gegen Filter, Suche und Sortierung nachführen bzw. neu aufbauen.
    async fn maintain(&self, app: &App, mut pending: Pending) -> Result<(), CmdError> {
        let Some(published) = self.published() else {
            return Ok(());
        };
        let _builder = self.builder.lock().await;
        let expected_core = lock(&self.state).core;
        // A cancelled build may have finished in the core after all: then rebuild the published specification.
        // try_lock: a detached, cancelled build may still hold the session on a pool thread.
        let diverged = self
            .session
            .try_lock()
            .map(|s| s.as_ref().is_none_or(|s| s.version() != expected_core))
            .unwrap_or(true);
        pending.reset |= diverged;
        pending.ids.sort_unstable();
        pending.ids.dedup();
        let rebuild = pending
            .reset
            .then(|| {
                session_spec(
                    app,
                    &published.session,
                    &published.spec,
                    &published.columns,
                    CancelToken::default(),
                )
            })
            .transpose()?;
        let slot = self.session.clone();
        let cfg = published.session.config.clone();
        let clock = self.clock.clone();
        let budget = self.budget;
        let (snapshot, core, seq, total) = app
            .read_pool()
            .run(self.key.clone(), move |file, _| {
                let mut guard = lock(&slot);
                Ok((|| {
                    match (guard.as_mut(), rebuild) {
                        (Some(s), Some(spec)) => s.update(file, &cfg, spec)?,
                        (None, Some(spec)) => {
                            *guard = Some(ViewSession::open_with_clock(
                                file, &cfg, spec, budget, clock,
                            )?)
                        }
                        (Some(s), None) => s.apply_changes(file, &pending.ids)?,
                        (None, None) => unreachable!("a missing session implies a rebuild"),
                    }
                    let s = guard.as_ref().expect("session");
                    let snapshot = s.snapshot(file)?;
                    let total = snapshot
                        .rows(
                            file,
                            RowsReq {
                                version: s.version(),
                                offset: 0,
                                limit: 1,
                            },
                        )?
                        .total;
                    Ok::<_, os_analyze::ViewError>((
                        snapshot,
                        s.version(),
                        s.built_seq().unwrap_or(0),
                        total,
                    ))
                })())
            })
            .await?
            .map_err(view_error)?;
        let mut st = lock(&self.state);
        st.snapshot = Some(snapshot);
        st.core = core;
        let published = st
            .published
            .as_mut()
            .ok_or_else(|| internal("views::maintain", "nicht veröffentlicht"))?;
        let changed = published.version != seq || published.total != total;
        published.version = seq;
        published.total = total;
        drop(st);
        if changed {
            self.announce(app, seq, total);
        }
        Ok(())
    }
}

fn on_change(app: &App, change: &ChangeSet) {
    for view in live(&shared(app)) {
        if view.key == change.key && view.published().is_some() {
            view.enqueue(app, change);
        }
    }
}

/// Wartet auf `work`, gibt bei Abbruch aber sofort `Cancelled` zurück; die unterbrochene Arbeit endet im Hintergrund.
async fn until_cancelled<T>(
    token: &CancelToken,
    work: impl std::future::Future<Output = T>,
) -> Result<T, CmdError> {
    let notify = Arc::new(tokio::sync::Notify::new());
    let signal = notify.clone();
    let _registration = token.on_cancel(move || signal.notify_one());
    if token.is_cancelled() {
        return Err(CmdError::new(ErrorCode::Cancelled, "err-cancelled"));
    }
    tokio::select! {
        r = work => Ok(r),
        _ = notify.notified() => Err(CmdError::new(ErrorCode::Cancelled, "err-cancelled")),
    }
}
