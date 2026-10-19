//! Headless crawl handles; the existing engine owns lifecycle, persistence and heartbeat.
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use os_analyze::analysis::{run_after_crawl, CancelToken};
use os_engine::{CrawlHandle, CrawlRequest, CrawlSource, EngineDeps};
use os_fetch::FetchConfig;
use os_model::{Clock, CrawlConfig, CrawlId, IdSource, RandomIds, SystemClock};
use os_store::{registry, StorageProfile};
use serde_json::{json, Value};

use crate::target::ProtectedFetcher;
use crate::{ToolContext, ToolError};

#[derive(Default)]
pub(crate) struct CrawlService {
    handles: Mutex<HashMap<String, CrawlHandle>>,
    finished: Mutex<HashSet<String>>,
}
impl std::fmt::Debug for CrawlService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CrawlService")
    }
}

fn invalid(e: impl std::fmt::Display) -> ToolError {
    ToolError::InvalidArguments(e.to_string())
}

impl CrawlService {
    fn renderer(
        cfg: &CrawlConfig,
        ctx: &ToolContext,
    ) -> Result<Option<Arc<dyn os_engine::Renderer>>, ToolError> {
        if cfg.rendering.mode == os_model::RenderingMode::Text {
            return Ok(None);
        }
        if !ctx.target_policy.allows_private_targets() {
            return Err(invalid("Protected JavaScript rendering is unavailable; use rendering.mode=text. Private targets can be permitted only in local MCP settings"));
        }
        let renderer = tokio::runtime::Handle::current()
            .block_on(os_render::Renderer::launch(os_render::LaunchOpts {
                chromium_path: cfg
                    .rendering
                    .chromium_path
                    .as_ref()
                    .map(std::path::PathBuf::from),
                headless: true,
                ..Default::default()
            }))
            .map_err(invalid)?;
        Ok(Some(Arc::new(renderer)))
    }

    fn deps(&self, id: Option<CrawlId>) -> EngineDeps {
        struct Fixed(CrawlId);
        impl IdSource for Fixed {
            fn new_crawl_id(&self, _: os_model::Timestamp) -> CrawlId {
                self.0
            }
        }
        EngineDeps {
            clock: Arc::new(SystemClock),
            ids: id.map_or_else(
                || Arc::new(RandomIds) as Arc<dyn IdSource>,
                |id| Arc::new(Fixed(id)),
            ),
            profile: StorageProfile::Balanced,
            secrets: os_fetch::secrets::secret_store_from_env(),
            sink_factory: None,
            owner_kind: "mcp-stdio",
            renderer: None,
            on_finished: Some(Arc::new(|summary| {
                if let Err(e) = run_after_crawl(&summary.file, &CancelToken::new()) {
                    tracing::error!(error=%e,"crawl analysis failed");
                }
            })),
        }
    }

    pub fn start(
        &self,
        ctx: &ToolContext,
        source: CrawlSource,
        cfg: CrawlConfig,
    ) -> Result<Value, ToolError> {
        let id = RandomIds.new_crawl_id(SystemClock.now());
        std::fs::create_dir_all(&ctx.crawl_dir).map_err(invalid)?;
        let mut deps = self.deps(Some(id));
        deps.renderer = Self::renderer(&cfg, ctx)?;
        let fetch_cfg = FetchConfig::from_crawl_config(&cfg, &*deps.secrets, deps.clock.clone())
            .map_err(invalid)?;
        let fetcher = ProtectedFetcher::new(
            fetch_cfg,
            ctx.target_policy.clone(),
            ctx.target_policy.allows_private_targets(),
        )
        .map_err(invalid)?;
        let req = CrawlRequest {
            config: cfg,
            source,
            file: registry::path_for(&ctx.crawl_dir, &id),
            project: String::new(),
            name: format!("MCP {id}"),
        };
        let handle = tokio::runtime::Handle::current()
            .block_on(os_engine::start_crawl(Arc::new(fetcher), req, deps))
            .map_err(invalid)?;
        let mut handles = self.handles.lock().map_err(invalid)?;
        let mut finished = self.finished.lock().map_err(invalid)?;
        handles.retain(|id, h| {
            let running = h.progress().state == os_model::CrawlState::Running;
            if !running {
                finished.insert(id.clone());
            }
            running
        });
        handles.insert(id.to_string(), handle);
        Ok(json!({"crawl_id":id.to_string(),"state":"Running"}))
    }

    pub fn status(&self, id: &str) -> Option<Value> {
        let handles = self.handles.lock().ok()?;
        handles
            .get(id)
            .map(|h| json!({"crawl_id":id,"progress":h.progress()}))
    }

    pub fn stop(&self, id: &str) -> Result<(), ToolError> {
        let handles = self.handles.lock().map_err(invalid)?;
        if self.finished.lock().map_err(invalid)?.contains(id)
            || handles
                .get(id)
                .is_some_and(|h| h.progress().state != os_model::CrawlState::Running)
        {
            return Err(ToolError::InvalidArguments(format!(
                "Crawl {id} already finished"
            )));
        }
        let handle = handles.get(id).ok_or_else(|| {
            ToolError::NotFound(format!("Crawl {id} is not controlled by this MCP process"))
        })?;
        handle.stop();
        Ok(())
    }

    pub fn resume(&self, ctx: &ToolContext, id: &str) -> Result<Value, ToolError> {
        let crawl = super::tools::common::open_crawl(ctx, &json!({"crawl_id":id}))?;
        let mut deps = self.deps(None);
        deps.renderer = Self::renderer(&crawl.cfg, ctx)?;
        let fetch_cfg =
            FetchConfig::from_crawl_config(&crawl.cfg, &*deps.secrets, deps.clock.clone())
                .map_err(invalid)?;
        let fetcher = ProtectedFetcher::new(
            fetch_cfg,
            ctx.target_policy.clone(),
            ctx.target_policy.allows_private_targets(),
        )
        .map_err(invalid)?;
        let path = crawl.file.path().to_path_buf();
        drop(crawl);
        let handle = tokio::runtime::Handle::current()
            .block_on(os_engine::resume_crawl(Arc::new(fetcher), path, None, deps))
            .map_err(invalid)?;
        self.finished.lock().map_err(invalid)?.remove(id);
        self.handles
            .lock()
            .map_err(invalid)?
            .insert(id.to_owned(), handle);
        Ok(json!({"crawl_id":id,"state":"Running"}))
    }
}
