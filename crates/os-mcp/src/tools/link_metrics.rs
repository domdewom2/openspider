//! Paid provider requests require a separate explicit confirmation (AGENT-003, K2-08).
use super::common::{internal, open_crawl, reject_unknown, req_str};
use crate::{Tool, ToolClass, ToolContext, ToolError, ToolOutput};
use os_integrations::{linkmetrics::LinkMetrics, ApiCtx, ApiSource, UrlRef};
use os_model::ApiKind;
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Clone)]
pub struct ApiEnvironment {
    pub secrets: Arc<dyn os_fetch::SecretStore>,
    pub network: os_model::settings::NetworkSettings,
    pub allow_api_override: bool,
}
impl std::fmt::Debug for ApiEnvironment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ApiEnvironment(<redacted>)")
    }
}
pub struct FetchLinkMetrics;
impl Tool for FetchLinkMetrics {
    fn name(&self) -> &'static str {
        "fetch_link_metrics"
    }
    fn class(&self) -> ToolClass {
        ToolClass::Cost
    }
    fn description(&self) -> &'static str {
        "Fetch link metrics from majestic, ahrefs or moz for up to 500 URLs. First call returns an estimate in provider usage units without contacting the provider. Repeat the same arguments with confirm: true to execute. Credentials and provider configuration come only from the saved crawl and local secret store."
    }
    fn input_schema(&self) -> Value {
        json!({"type":"object","required":["crawl_id","provider","urls"],"properties":{"crawl_id":{"type":"string"},"provider":{"enum":["majestic","ahrefs","moz"]},"urls":{"type":"array","minItems":1,"maxItems":500,"items":{"type":"string"}},"confirm":{"type":"boolean"}},"additionalProperties":false})
    }
    fn call(&self, ctx: &ToolContext, args: Value) -> Result<ToolOutput, ToolError> {
        reject_unknown(&args, &["crawl_id", "provider", "urls", "confirm"])?;
        let kind = match req_str(&args, "provider")? {
            "majestic" => ApiKind::Majestic,
            "ahrefs" => ApiKind::Ahrefs,
            "moz" => ApiKind::Moz,
            _ => {
                return Err(ToolError::InvalidArguments(
                    "provider: majestic, ahrefs, moz".into(),
                ))
            }
        };
        let urls = args["urls"]
            .as_array()
            .filter(|a| !a.is_empty() && a.len() <= 500)
            .ok_or_else(|| ToolError::InvalidArguments("urls requires 1–500 entries".into()))?
            .iter()
            .map(|v| {
                v.as_str()
                    .map(|url| UrlRef {
                        url_id: None,
                        url: url.into(),
                    })
                    .ok_or_else(|| ToolError::InvalidArguments("URLs must be strings".into()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let crawl = open_crawl(ctx, &args)?;
        let cfg = match kind {
            ApiKind::Majestic => &crawl.cfg.api.majestic,
            ApiKind::Ahrefs => &crawl.cfg.api.ahrefs,
            _ => &crawl.cfg.api.moz,
        };
        let mut targets = std::collections::BTreeSet::new();
        for url in &urls {
            for level in &cfg.levels {
                targets.insert(
                    os_integrations::linkmetrics::target(&url.url, *level).map_err(internal)?,
                );
            }
        }
        if args.get("confirm").is_some_and(|v| !v.is_boolean()) {
            return Err(ToolError::InvalidArguments(
                "confirm must be boolean".into(),
            ));
        }
        let estimate = json!({"provider":args["provider"],"targets":targets.len(),"unit":"provider targets","monetary_cost":null,"note":"Usage depends on selected metrics and your provider plan"});
        if args.get("confirm").and_then(Value::as_bool) != Some(true) {
            return Ok(ToolOutput {
                summary: json!({"estimate":estimate,"confirmation_required":true}),
                untrusted: None,
                total: None,
                next_cursor: None,
            });
        }
        let environment = ctx
            .api_environment
            .clone()
            .unwrap_or_else(|| ApiEnvironment {
                secrets: os_fetch::secret_store_from_env(),
                network: Default::default(),
                allow_api_override: false,
            });
        let api = ApiCtx {
            cfg: crawl.cfg,
            secrets: environment.secrets,
            network: environment.network,
            allow_api_override: environment.allow_api_override,
            quota_ledger: None,
        };
        let values = tokio::runtime::Handle::current()
            .block_on(LinkMetrics::new(kind).fetch(&urls, &api))
            .map_err(internal)?;
        Ok(ToolOutput {
            summary: json!({"estimate":estimate,"confirmation_required":false,"values":values.len()}),
            untrusted: Some(
                json!({"metrics":values.iter().map(|v| json!({"url":v.url,"source":v.source,"metric":v.metric,"level":v.level,"value":v.v_real,"text":v.v_text})).collect::<Vec<_>>()}),
            ),
            total: None,
            next_cursor: None,
        })
    }
}
