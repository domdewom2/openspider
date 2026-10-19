//! Engine commands share a mutation lock; pause waits for the durable checkpoint.
use super::{feed, SessionService};
use crate::{
    dto::*,
    pool::CrawlKey,
    runtime::{
        error::{internal, parse_args},
        lifecycle_bus::CrawlEvent,
        sessions::{EngineControl, SessionInfo},
    },
    App, CallCtx, CmdError,
};
use os_engine::{CrawlHandle, EngineDeps};
use os_model::{CrawlConfig, CrawlState};
use std::{path::PathBuf, sync::Arc};

pub fn engine_error(e: os_engine::EngineError) -> CmdError {
    match e {
        os_engine::EngineError::Store(e) => crate::runtime::error::store_error(&e),
        os_engine::EngineError::Cancelled => CmdError::new(ErrorCode::Cancelled, "err-cancelled"),
        os_engine::EngineError::NotImplemented(owner) => CmdError::unsupported("crawl", owner),
        other => CmdError::new(ErrorCode::InvalidInput, "err-crawl").with_detail(other.to_string()),
    }
}
pub fn deps(app: &App) -> EngineDeps {
    EngineDeps {
        clock: app.config().clock.clone(),
        ids: app.config().ids.clone(),
        profile: match app.settings_snapshot().storage.performance_profile {
            os_model::PerformanceProfile::Balanced => os_store::StorageProfile::Balanced,
            os_model::PerformanceProfile::MaxSpeed => os_store::StorageProfile::MaxSpeed,
            os_model::PerformanceProfile::MinMemory => os_store::StorageProfile::MinMemory,
        },
        secrets: app.config().secrets.clone(),
        sink_factory: None,
        on_finished: None,
        owner_kind: "app",
        renderer: None,
    }
}
pub fn fetcher(app: &App, cfg: &CrawlConfig) -> Result<Arc<os_fetch::HttpFetcher>, CmdError> {
    os_fetch::FetchConfig::from_crawl_config(
        cfg,
        app.config().secrets.as_ref(),
        app.config().clock.clone(),
    )
    .and_then(os_fetch::HttpFetcher::new)
    .map(Arc::new)
    .map_err(|e| {
        CmdError::new(ErrorCode::InvalidInput, "err-fetch-config").with_detail(e.to_string())
    })
}
pub async fn config(
    app: &App,
    ctx: &CallCtx,
    profile: Option<String>,
) -> Result<CrawlConfig, CmdError> {
    if let Some(name) = profile {
        let dto: CrawlConfigDto = parse_args(
            app.dispatch(
                ctx.clone(),
                "profile.load",
                serde_json::json!({"name":name}),
            )
            .await?,
        )?;
        return parse_args(dto.config);
    }
    if let Some(s) = app.sessions().current(&ctx.window) {
        return Ok((*s.config).clone());
    }
    let path = app.config().paths.settings.clone();
    tokio::task::spawn_blocking(move || {
        CrawlConfig::load_default(&path)
            .map(|c| c.config)
            .map_err(|e| {
                CmdError::new(ErrorCode::InvalidInput, "err-profile").with_detail(e.to_string())
            })
    })
    .await
    .map_err(|e| internal("load config", e.to_string()))?
}
pub async fn ensure_idle(app: &App, s: &SessionInfo) -> Result<(), CmdError> {
    super::writable(s)?;
    if s.engine
        .as_ref()
        .is_some_and(|e| e.handle.lock().unwrap_or_else(|p| p.into_inner()).is_some())
    {
        return Err(super::busy());
    }
    let (_, _, state) = feed::snapshot(app, s.key.clone()).await?;
    if state == CrawlState::Running {
        return Err(super::busy());
    }
    Ok(())
}
pub async fn replace_allowed(app: &App, ctx: &CallCtx) -> Result<(), CmdError> {
    if app
        .ext::<SessionService>()
        .quitting
        .load(std::sync::atomic::Ordering::SeqCst)
        || app.ops().running() > 0
    {
        return Err(super::busy());
    }
    if let Some(s) = app.sessions().current(&ctx.window) {
        // Foreign sessions can be detached, but an owned engine must first stop.
        if s.engine
            .as_ref()
            .is_some_and(|e| e.handle.lock().unwrap_or_else(|p| p.into_inner()).is_some())
        {
            return Err(super::busy());
        }
    }
    Ok(())
}

pub async fn start(app: &App, ctx: CallCtx, req: StartRequest) -> Result<CrawlRef, CmdError> {
    let mut cfg = config(app, &ctx, req.profile.clone()).await?;
    cfg.scope.all_subdomains = req.scope == StartScope::Subdomains;
    cfg.scope.crawl_outside_start_folder = req.scope != StartScope::Folder;
    if let StartSource::List { .. } = &req.source {
        cfg = os_engine::list_input::list_config(cfg);
    }
    let fetch = fetcher(app, &cfg)?;
    start_with_fetcher(app, ctx, req, cfg, fetch).await
}

/// The same production lifecycle with an injected fetcher for deterministic fixture tests.
pub async fn start_with_fetcher<F: os_fetch::Fetch>(
    app: &App,
    ctx: CallCtx,
    req: StartRequest,
    cfg: CrawlConfig,
    fetch: Arc<F>,
) -> Result<CrawlRef, CmdError> {
    let service = app.ext::<SessionService>();
    let _guard = service.mutation.lock().await;
    replace_allowed(app, &ctx).await?;
    let dir = crate::runtime::info::effective_crawls_dir(&app.settings_snapshot(), app.config());
    let id = app.config().ids.new_crawl_id(app.config().clock.now());
    let file = os_store::registry::path_for(&dir, &id);
    let source = match req.source {
        StartSource::Spider { url } => os_engine::CrawlSource::Spider {
            start_url: os_url::parse_start_url(&url)
                .map_err(|e| {
                    CmdError::invalid_input("err-start-url", "url").with_detail(e.to_string())
                })?
                .encoded,
        },
        StartSource::List { items, label } => os_engine::CrawlSource::List { items, label },
        StartSource::Apis { items } => return start_apis(app, ctx, cfg, file, id, items).await,
    };
    let name = match &source {
        os_engine::CrawlSource::Spider { start_url } => start_url.clone(),
        os_engine::CrawlSource::List { label, .. } => label.clone(),
    };
    let request = os_engine::CrawlRequest {
        config: cfg,
        source,
        file: file.clone(),
        project: String::new(),
        name,
    };
    let mut deps = deps(app);
    deps.ids = Arc::new(FixedId(id));
    let runtime = app.handle();
    let h = tokio::task::spawn_blocking(move || {
        runtime.block_on(os_engine::start_crawl(fetch, request, deps))
    })
    .await
    .map_err(|e| internal("start crawl", e.to_string()))?
    .map_err(engine_error)?;
    let key = CrawlKey(h.crawl_id().to_string());
    app.register_crawl(key.clone(), file.clone());
    let (_, cfg, _) = feed::snapshot(app, key.clone()).await?;
    super::close(app, &ctx.window, false);
    attach(
        app,
        ctx,
        key.clone(),
        file.clone(),
        cfg,
        h,
        CrawlEvent::Started(key.clone()),
    );
    Ok(CrawlRef {
        crawl_id: key.0,
        file: file.to_string_lossy().into_owned(),
    })
}
struct FixedId(os_model::CrawlId);
impl os_model::IdSource for FixedId {
    fn new_crawl_id(&self, _: os_model::Timestamp) -> os_model::CrawlId {
        self.0
    }
}

fn attach(
    app: &App,
    ctx: CallCtx,
    key: CrawlKey,
    path: PathBuf,
    cfg: CrawlConfig,
    h: CrawlHandle,
    ev: CrawlEvent,
) {
    app.sessions().set_current(
        ctx.window,
        SessionInfo {
            key: key.clone(),
            path,
            config: Arc::new(cfg),
            state: CrawlState::Running,
            writable: true,
            engine: Some(Arc::new(EngineControl {
                handle: std::sync::Mutex::new(Some(h)),
            })),
        },
    );
    super::activate(app);
    feed::invalidate(app, &key);
    app.crawl_events().emit(app, ev);
}

pub async fn open(
    app: &App,
    ctx: CallCtx,
    arg: OpenCrawlArgs,
) -> Result<CrawlSummaryDto, CmdError> {
    let service = app.ext::<SessionService>();
    let _guard = service.mutation.lock().await;
    let path = match arg {
        OpenCrawlArgs::Path { path } => PathBuf::from(path),
        OpenCrawlArgs::Id { crawl_id } => {
            let id = crawl_id
                .parse()
                .map_err(|_| CmdError::invalid_input("err-crawl-id", "crawlId"))?;
            let dir =
                crate::runtime::info::effective_crawls_dir(&app.settings_snapshot(), app.config());
            tokio::task::spawn_blocking(move || os_store::registry::find(&dir, &id))
                .await
                .map_err(|e| internal("find crawl", e.to_string()))?
                .ok_or_else(|| CmdError::new(ErrorCode::NotFound, "err-crawl-not-found"))?
        }
    };
    let path = tokio::task::spawn_blocking(move || {
        std::fs::canonicalize(&path).map_err(|e| {
            crate::runtime::error::store_error(&os_store::StoreError::Io { path, source: e })
        })
    })
    .await
    .map_err(|e| internal("open", e.to_string()))??;
    if let Some(s) = app
        .sessions()
        .current(&ctx.window)
        .filter(|s| s.path == path)
    {
        let (summary, cfg, state) = feed::snapshot(app, s.key.clone()).await?;
        if s.writable != summary.owner.controllable {
            let mut updated = s;
            updated.writable = summary.owner.controllable;
            updated.state = state;
            updated.config = Arc::new(cfg);
            if updated.writable {
                app.write_queue()
                    .register(updated.key.clone(), updated.path.clone());
            } else {
                app.write_queue().unregister(&updated.key);
            }
            app.sessions().set_current(ctx.window, updated);
        }
        return Ok(summary);
    }
    replace_allowed(app, &ctx).await?;
    let temporary = CrawlKey(format!("opening:{}", path.display()));
    let report = app
        .read_pool()
        .prepare_open(temporary.clone(), path)
        .await?;
    let result = feed::snapshot(app, temporary.clone()).await;
    app.read_pool().unregister(&temporary);
    let (summary, cfg, state) = result?;
    let key = CrawlKey(summary.crawl_id.clone());
    let existing = app.sessions().all().into_iter().find(|s| s.key == key);
    super::close(app, &ctx.window, false);
    if let Some(s) = existing {
        app.sessions().set_current(ctx.window, s);
    } else {
        app.read_pool().register(key.clone(), report.path.clone());
        if summary.owner.controllable {
            app.write_queue().register(key.clone(), report.path.clone());
        }
        app.sessions().set_current(
            ctx.window,
            SessionInfo {
                key: key.clone(),
                path: report.path,
                config: Arc::new(cfg),
                state,
                writable: summary.owner.controllable,
                engine: None,
            },
        );
    }
    super::activate(app);
    app.crawl_events().emit(app, CrawlEvent::Opened(key));
    Ok(summary)
}

pub async fn resume(app: &App, ctx: CallCtx) -> Result<(), CmdError> {
    let service = app.ext::<SessionService>();
    let _guard = service.mutation.lock().await;
    let s = super::current(app, &ctx.window)?;
    ensure_idle(app, &s).await?;
    if app.ops().running() > 0 {
        return Err(super::busy());
    }
    let cfg = app
        .read_pool()
        .run(s.key.clone(), |f, _| f.config())
        .await?;
    let runtime = app.handle();
    let fetch = fetcher(app, &cfg)?;
    let path = s.path.clone();
    let deps = deps(app);
    let h = tokio::task::spawn_blocking(move || {
        runtime.block_on(os_engine::resume_crawl(fetch, path, None, deps))
    })
    .await
    .map_err(|e| internal("resume crawl", e.to_string()))?
    .map_err(engine_error)?;
    if *s.config != cfg {
        app.sessions().update_config(&s.key, Arc::new(cfg.clone()));
        app.crawl_events().emit(
            app,
            CrawlEvent::ConfigChanged {
                key: s.key.clone(),
                old: s.config.clone(),
                new: Arc::new(cfg.clone()),
            },
        );
    }
    attach(
        app,
        ctx,
        s.key.clone(),
        s.path,
        cfg,
        h,
        CrawlEvent::Resumed(s.key),
    );
    Ok(())
}

pub async fn pause_or_stop(app: &App, ctx: CallCtx, stop: bool) -> Result<(), CmdError> {
    let service = app.ext::<SessionService>();
    let _guard = service.mutation.lock().await;
    let s = super::current(app, &ctx.window)?;
    super::writable(&s)?;
    settle(app, &s, Some(stop)).await?;
    feed::invalidate(app, &s.key);
    feed::refresh(app, &s).await
}

pub async fn settle(app: &App, s: &SessionInfo, action: Option<bool>) -> Result<(), CmdError> {
    let h = s
        .engine
        .as_ref()
        .and_then(|e| e.handle.lock().unwrap_or_else(|p| p.into_inner()).take());
    let Some(h) = h else {
        return if action.is_some() {
            Err(CmdError::new(
                ErrorCode::InvalidInput,
                "err-crawl-not-running",
            ))
        } else {
            Ok(())
        };
    };
    if let Some(stop) = action {
        if stop {
            h.stop();
        } else {
            h.pause();
        }
    }
    let watch = h.watch();
    let result = h.wait().await.map_err(engine_error);
    let mut p = watch.borrow().clone();
    let state = result.as_ref().map_or(CrawlState::Failed, |r| r.state);
    super::state(app, &s.key, state);
    p.state = state;
    if result.is_err() {
        app.push_notice(Notice {
            level: NoticeLevel::Error,
            code: "crawl-failed".into(),
            args: Default::default(),
        });
    }
    // The engine's periodic snapshot may predate the final batch. Publish the final
    // progress only after wait() has acknowledged the durable checkpoint.
    let text = serde_json::to_string(&p).map_err(|e| internal("progress", e.to_string()))?;
    app.write_queue()
        .submit_file(&s.key, move |f| {
            f.set_meta("progress_snapshot", &text)
                .map_err(|e| crate::runtime::error::store_error(&e))
        })
        .await?;
    let summary = result?;
    let event = match summary.state {
        CrawlState::Paused => CrawlEvent::Paused(s.key.clone()),
        _ if action == Some(true) => CrawlEvent::Stopped(s.key.clone()),
        _ => CrawlEvent::Completed(s.key.clone()),
    };
    app.crawl_events().emit(app, event);
    if summary.state == CrawlState::Completed
        && s.config.analysis.auto_at_end
        && !app
            .ext::<SessionService>()
            .quitting
            .load(std::sync::atomic::Ordering::SeqCst)
    {
        super::analysis::start_operation(app, s.clone(), None, true)?;
    }
    Ok(())
}

async fn start_apis(
    app: &App,
    ctx: CallCtx,
    cfg: CrawlConfig,
    path: PathBuf,
    id: os_model::CrawlId,
    items: Vec<String>,
) -> Result<CrawlRef, CmdError> {
    let key = CrawlKey(id.to_string());
    let init = os_store::CrawlInit {
        crawl_id: id,
        mode: os_model::CrawlMode::List,
        start_url: None,
        list_source: Some("APIs".into()),
        project: String::new(),
        name: "APIs".into(),
        config: cfg.clone(),
        created_at: app.config().clock.now(),
    };
    let plan = os_engine::list_input::plan(&items, &cfg).map_err(engine_error)?;
    let mut initial = os_model::WriteBatch::default();
    for entry in plan
        .entries
        .into_iter()
        .filter(|e| e.outcome == os_engine::list_input::ListOutcome::Ok)
    {
        if let Some(url) = entry.url {
            initial.new_urls.push(os_model::NewUrl {
                id: os_model::UrlId(initial.new_urls.len() as i64 + 1),
                folder_depth: os_url::folder_depth(&url.url),
                url_length: os_url::url_length(&url.display),
                url: url.encoded,
                address: url.display,
                origin: url.origin,
                is_internal: true,
                in_tables: true,
                discovered_via: os_model::DiscoveryKind::ListInput,
                crawl_depth: None,
                state: os_model::UrlState::Discovered,
                not_crawled_reason: Some(os_model::NotCrawledReason::CrawlDisabled),
            });
        }
    }
    initial.meta.push(("source_mode".into(), "apis".into()));
    app.write_queue()
        .create_file(key.clone(), path.clone(), init, initial)
        .await?;
    let runtime = app.handle();
    let stored_cfg = cfg.clone();
    let result = app
        .write_queue()
        .submit_file(&key, move |f| {
            f.set_meta(
                "api_input_urls",
                &serde_json::to_string(&items)
                    .map_err(|e| internal("API inputs", e.to_string()))?,
            )
            .map_err(|e| crate::runtime::error::store_error(&e))?;
            let result = runtime.block_on(os_integrations::crawl_glue::start_apis_mode(f, &cfg));
            f.set_meta(
                "state",
                if result.is_ok() {
                    "Completed"
                } else {
                    "Failed"
                },
            )
            .map_err(|e| crate::runtime::error::store_error(&e))?;
            result.map_err(|e| match e {
                os_integrations::IntegError::NotImplemented(owner) => {
                    CmdError::unsupported("crawl.start", owner)
                }
                other => internal("API crawl", other.to_string()),
            })
        })
        .await;
    if let Err(e) = result {
        app.write_queue().unregister(&key);
        return Err(e);
    }
    super::close(app, &ctx.window, false);
    app.read_pool().register(key.clone(), path.clone());
    app.sessions().set_current(
        ctx.window,
        SessionInfo {
            key: key.clone(),
            path: path.clone(),
            config: Arc::new(stored_cfg),
            state: CrawlState::Completed,
            writable: true,
            engine: None,
        },
    );
    super::activate(app);
    app.crawl_events()
        .emit(app, CrawlEvent::Started(key.clone()));
    app.crawl_events()
        .emit(app, CrawlEvent::Completed(key.clone()));
    Ok(CrawlRef {
        crawl_id: key.0,
        file: path.to_string_lossy().into_owned(),
    })
}
