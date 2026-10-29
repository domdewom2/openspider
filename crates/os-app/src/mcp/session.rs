//! The HTTP server owns ordinary app engine sessions, with connection-time target protection.
use crate::{
    pool::CrawlKey,
    runtime::{
        lifecycle_bus::CrawlEvent,
        sessions::{EngineControl, SessionInfo},
        WeakApp,
    },
    session::{self, control, SessionService},
    App, CallCtx, WindowId,
};
use os_mcp::{
    permissions::DomainPolicy, target::ProtectedFetcher, SessionAccess, SessionCrawl, SessionError,
    SessionSource, SessionStart, SessionStatus,
};
use os_model::{CrawlConfig, CrawlState};
use std::sync::{Arc, Mutex};

pub struct Access(pub WeakApp);
impl std::fmt::Debug for Access {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AppSessionAccess")
    }
}
fn err(e: impl std::fmt::Display) -> SessionError {
    SessionError::Internal(e.to_string())
}
fn dto(s: SessionInfo) -> SessionCrawl {
    SessionCrawl {
        crawl_id: s.key.0,
        name: "App crawl".into(),
        project: String::new(),
        file: s.path,
        state: s.state,
        controllable: s.writable,
    }
}
impl Access {
    fn app(&self) -> Result<App, SessionError> {
        self.0.upgrade().ok_or_else(|| err("App closed"))
    }
    async fn launch(
        app: &App,
        source: Option<SessionSource>,
        config: CrawlConfig,
        policy: DomainPolicy,
        resume: Option<SessionInfo>,
    ) -> Result<SessionCrawl, SessionError> {
        let service = app.ext::<SessionService>();
        let _guard = service.mutation.lock().await;
        if service.quitting.load(std::sync::atomic::Ordering::SeqCst) || app.ops().running() > 0 {
            return Err(SessionError::Invalid("App is busy or shutting down".into()));
        }
        let network = app.settings_snapshot().network;
        let mut fetch_config = os_fetch::FetchConfig::from_crawl_config(
            &config,
            app.config().secrets.as_ref(),
            app.config().clock.clone(),
        )
        .map_err(err)?
        .with_network(&network);
        fetch_config.proxy_password = app
            .config()
            .secrets
            .try_get(os_fetch::net::PROXY_PASSWORD)
            .map_err(err)?;
        let fetcher = Arc::new(
            ProtectedFetcher::new(
                fetch_config,
                policy.clone(),
                policy.allows_private_targets(),
            )
            .map_err(err)?,
        );
        let mut deps = control::deps(app);
        if config.rendering.mode != os_model::RenderingMode::Text {
            deps.renderer = Some(
                os_mcp::render_guard::launch(&config, policy.clone())
                    .await
                    .map_err(err)?,
            );
        }
        let runtime = app.handle();
        let (handle, file) = if let Some(s) = resume {
            control::ensure_idle(app, &s).await.map_err(err)?;
            let file = s.path;
            let path = file.clone();
            let h = tokio::task::spawn_blocking(move || {
                runtime.block_on(os_engine::resume_crawl(fetcher, path, None, deps))
            })
            .await
            .map_err(err)?
            .map_err(err)?;
            (h, file)
        } else {
            let id = app.config().ids.new_crawl_id(app.config().clock.now());
            struct Fixed(os_model::CrawlId);
            impl os_model::IdSource for Fixed {
                fn new_crawl_id(&self, _: os_model::Timestamp) -> os_model::CrawlId {
                    self.0
                }
            }
            deps.ids = Arc::new(Fixed(id));
            let dir =
                crate::runtime::info::effective_crawls_dir(&app.settings_snapshot(), app.config());
            let file = os_store::registry::path_for(&dir, &id);
            let source = match source.ok_or_else(|| err("Missing source"))? {
                SessionSource::Spider { url } => {
                    policy.check_url(&url).map_err(SessionError::Invalid)?;
                    os_engine::CrawlSource::Spider { start_url: url }
                }
                SessionSource::List { items, label } => {
                    for url in &items {
                        policy.check_url(url).map_err(SessionError::Invalid)?;
                    }
                    os_engine::CrawlSource::List { items, label }
                }
            };
            let mut config = config;
            config.custom_js.output_dir = Some(dir);
            let request = os_engine::CrawlRequest {
                config,
                source,
                file: file.clone(),
                project: String::new(),
                name: "MCP".into(),
            };
            let h = tokio::task::spawn_blocking(move || {
                runtime.block_on(os_engine::start_crawl(fetcher, request, deps))
            })
            .await
            .map_err(err)?
            .map_err(err)?;
            (h, file)
        };
        let key = CrawlKey(handle.crawl_id().to_string());
        app.register_crawl(key.clone(), file.clone());
        let (_, cfg, _) = session::feed::snapshot(app, key.clone())
            .await
            .map_err(err)?;
        let s = SessionInfo {
            key: key.clone(),
            path: file,
            config: Arc::new(cfg),
            state: CrawlState::Running,
            writable: true,
            engine: Some(Arc::new(EngineControl {
                handle: Mutex::new(Some(handle)),
            })),
        };
        app.sessions()
            .set_current(WindowId(format!("mcp:{}", key.0)), s.clone());
        let main = WindowId("main".into());
        if app
            .sessions()
            .current(&main)
            .is_none_or(|old| old.state != CrawlState::Running)
        {
            app.sessions().set_current(main, s.clone());
        }
        session::activate(app);
        session::feed::invalidate(app, &key);
        app.crawl_events().emit(app, CrawlEvent::Started(key));
        Ok(dto(s))
    }
}
impl SessionAccess for Access {
    fn current_config(&self) -> Option<CrawlConfig> {
        let app = self.app().ok()?;
        app.handle()
            .block_on(control::config(&app, &CallCtx::for_window("main"), None))
            .ok()
    }
    fn crawls(&self) -> Vec<SessionCrawl> {
        let Ok(app) = self.app() else { return vec![] };
        let mut seen = std::collections::HashSet::new();
        app.sessions()
            .all()
            .into_iter()
            .filter(|s| seen.insert(s.key.clone()))
            .map(dto)
            .collect()
    }
    fn status(&self, id: &str) -> Option<SessionStatus> {
        let app = self.app().ok()?;
        let s = app.sessions().all().into_iter().find(|s| s.key.0 == id)?;
        let progress = s.engine.and_then(|e| {
            e.handle
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .as_ref()
                .map(|h| h.progress())
        });
        Some(SessionStatus {
            // The terminal watch can precede the durable writer acknowledgement.
            state: if progress.is_some() {
                CrawlState::Running
            } else {
                s.state
            },
            progress,
        })
    }
    fn start_crawl(&self, start: SessionStart) -> Result<SessionCrawl, SessionError> {
        let app = self.app()?;
        let cfg = app
            .handle()
            .block_on(control::config(
                &app,
                &CallCtx::for_window("main"),
                start.profile.clone(),
            ))
            .map_err(err)?;
        self.start_configured(start, cfg, DomainPolicy::default())
    }
    fn start_configured(
        &self,
        start: SessionStart,
        config: CrawlConfig,
        policy: DomainPolicy,
    ) -> Result<SessionCrawl, SessionError> {
        let app = self.app()?;
        app.handle()
            .block_on(Self::launch(&app, Some(start.source), config, policy, None))
    }
    fn stop_crawl(&self, id: &str) -> Result<(), SessionError> {
        let app = self.app()?;
        let s = app
            .sessions()
            .all()
            .into_iter()
            .find(|s| s.key.0 == id)
            .ok_or_else(|| SessionError::NotFound(id.into()))?;
        if !s.writable {
            return Err(SessionError::NotControllable(id.into()));
        }
        app.handle().block_on(async {
            let service = app.ext::<SessionService>();
            let _guard = service.mutation.lock().await;
            control::settle(&app, &s, Some(true)).await.map_err(err)
        })
    }
    fn resume_crawl(&self, id: &str, policy: DomainPolicy) -> Result<SessionCrawl, SessionError> {
        let app = self.app()?;
        let s = app
            .sessions()
            .all()
            .into_iter()
            .find(|s| s.key.0 == id)
            .ok_or_else(|| SessionError::NotFound(id.into()))?;
        app.handle().block_on(Self::launch(
            &app,
            None,
            (*s.config).clone(),
            policy,
            Some(s),
        ))
    }
}
