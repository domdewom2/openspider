//! Robots-Tester und Live-Datei (CRAWL-102); dieselbe Auswertung wie im Crawl.
use crate::dto::{CmdError, ErrorCode, RobotsLoadArgs, RobotsTestArgs, RobotsTestResult};
use crate::registry::{CmdResult, CommandMeta, Registry};
use crate::runtime::error::{parse_args, to_json};
use crate::runtime::{App, CallCtx};
use futures_util::future::BoxFuture;

fn config(app: &App, ctx: &CallCtx) -> os_model::CrawlConfig {
    app.sessions()
        .current(&ctx.window)
        .map(|s| (*s.config).clone())
        .unwrap_or_default()
}

fn test<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: RobotsTestArgs = parse_args(args)?;
        let norm = os_url::parse_start_url(&args.url)
            .map_err(|e| CmdError::invalid_input("err-url", "url").with_detail(e.to_string()))?;
        let cfg = config(app, &ctx);
        let user_agent = os_fetch::user_agents::resolve(&cfg.user_agent).robots_token;
        let content = match args
            .content
            .or_else(|| cfg.robots.custom_files.get(&norm.origin).cloned())
            .or_else(|| {
                let selected = args
                    .subdomain
                    .as_deref()
                    .unwrap_or_else(|| norm.url.host_str().unwrap_or_default());
                cfg.robots
                    .custom_files
                    .iter()
                    .find(|(host, _)| host.eq_ignore_ascii_case(selected))
                    .map(|(_, text)| text.clone())
            }) {
            Some(text) => text,
            None => load_text(app, &cfg, &norm.origin).await?,
        };
        let txt = os_fetch::robots::parse(&content);
        let path = match norm.url.query() {
            Some(q) => format!("{}?{q}", norm.url.path()),
            None => norm.url.path().to_string(),
        };
        let matched = txt.evaluate(&user_agent, &path);
        to_json(&RobotsTestResult {
            allowed: matched.as_ref().is_none_or(|m| m.allowed),
            matched_line: matched.map(|m| m.line_no),
            user_agent,
        })
    })
}

async fn load_text(app: &App, cfg: &os_model::CrawlConfig, host: &str) -> Result<String, CmdError> {
    let url = os_url::parse_start_url(host)
        .map_err(|e| CmdError::invalid_input("err-url", "host").with_detail(e.to_string()))?
        .url;
    let fetch_cfg = os_fetch::FetchConfig::from_crawl_config(
        cfg,
        &*app.config().secrets,
        app.config().clock.clone(),
    )
    .map_err(|e| {
        CmdError::new(ErrorCode::InvalidInput, "err-fetch-config").with_detail(e.to_string())
    })?;
    let fetcher = os_fetch::HttpFetcher::new(fetch_cfg)
        .map_err(|e| CmdError::new(ErrorCode::Internal, "err-fetch").with_detail(e.to_string()))?;
    let mut gate_cfg = os_fetch::robots::RobotsGateConfig::from_crawl_config(cfg);
    gate_cfg.mode = os_model::RobotsMode::Respect;
    gate_cfg.custom_files.clear();
    let check = os_fetch::robots::RobotsGate::new(gate_cfg)
        .check(&fetcher, &url)
        .await;
    let write = check
        .robots_fetch
        .map(|(_, w)| w)
        .ok_or_else(|| CmdError::new(ErrorCode::Internal, "err-robots-load"))?;
    write.body.ok_or_else(|| {
        CmdError::new(ErrorCode::InvalidInput, "err-robots-load")
            .with_detail(write.note.unwrap_or_default())
    })
}

fn load<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: RobotsLoadArgs = parse_args(args)?;
        let cfg = config(app, &ctx);
        let body = load_text(app, &cfg, &args.host).await?;
        to_json(&body)
    })
}

pub fn register(r: &mut Registry) {
    r.register(
        CommandMeta {
            name: "config.testRobots",
            args: "RobotsTestArgs",
            ret: "RobotsTestResult",
            long_running: false,
        },
        test,
    );
    r.register(
        CommandMeta {
            name: "config.loadRobots",
            args: "RobotsLoadArgs",
            ret: "string",
            long_running: false,
        },
        load,
    );
}
