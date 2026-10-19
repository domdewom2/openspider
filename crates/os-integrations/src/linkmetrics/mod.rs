//! Linkmetriken (INTEG-150–154/157). Alle Zugriffe benutzen den gemeinsamen Netzwerk- und Secret-Pfad.
mod batch;
type Outcome = Result<Vec<ApiValueWrite>, ApiError>;
type Grouped = BTreeMap<(Option<os_model::UrlId>, String), Outcome>;
pub mod catalog;
pub mod oauth;
pub use batch::batch_source;

use crate::{
    credentials::{read_account, Credential},
    http::{self, Transport},
    ApiCtx, ApiError, ApiSource, ApiState, ApiStatus, BoxFuture, ProviderQuota, UrlRef,
};
use os_model::{config::LinkMetricsConfig, ApiKind, ApiLevel, ApiValueWrite, Clock, SystemClock};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::Mutex as AsyncMutex;
use url::Url;

pub fn config(ctx: &ApiCtx, kind: ApiKind) -> &LinkMetricsConfig {
    match kind {
        ApiKind::Majestic => &ctx.cfg.api.majestic,
        ApiKind::Ahrefs => &ctx.cfg.api.ahrefs,
        ApiKind::Moz => &ctx.cfg.api.moz,
        _ => unreachable!("Linkmetrik-Anbieter"),
    }
}
pub fn is_linkmetrics(kind: ApiKind) -> bool {
    matches!(kind, ApiKind::Majestic | ApiKind::Ahrefs | ApiKind::Moz)
}
pub fn options_hint(cfg: &LinkMetricsConfig) -> String {
    let options = cfg.options.clone();
    format!(
        "Ahrefs: {}, {}",
        options.country.as_deref().unwrap_or("Alle Länder"),
        if options.average_volume {
            "Durchschnittsvolumen"
        } else {
            "Monatliches Volumen"
        }
    )
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Target {
    pub level: ApiLevel,
    pub query: String,
    pub protocol: String,
}
pub fn target(address: &str, level: ApiLevel) -> Result<Target, ApiError> {
    let mut url = Url::parse(address).map_err(|_| invalid())?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(invalid());
    }
    url.set_fragment(None);
    let host = url.host_str().ok_or_else(invalid)?;
    let query = match level {
        ApiLevel::Domain => os_fetch::registrable_domain(host).ok_or_else(invalid)?,
        ApiLevel::Subdomain => host.to_string(),
        _ => url.to_string(),
    };
    let protocol = if level == ApiLevel::Url {
        url.scheme()
    } else {
        "both"
    }
    .into();
    // Bei beiden Protokollen müssen http und https denselben Cache-/Gruppenschlüssel besitzen.
    let query = if level == ApiLevel::UrlBoth {
        url.set_scheme("https").map_err(|_| invalid())?;
        url.to_string()
    } else {
        query
    };
    Ok(Target {
        level,
        query,
        protocol,
    })
}
fn invalid() -> ApiError {
    ApiError::Transport("Ungültige Linkmetrik-Konfiguration oder Anbieterantwort".into())
}

pub struct LinkMetrics {
    kind: ApiKind,
    clock: Arc<dyn Clock>,
    serial: AsyncMutex<()>,
    status: Mutex<ApiStatus>,
    quotas: Mutex<Vec<ProviderQuota>>,
    transport: AsyncMutex<Option<(String, Arc<Transport>)>>,
    last_free: AsyncMutex<Option<tokio::time::Instant>>,
}
impl LinkMetrics {
    pub fn new(kind: ApiKind) -> Self {
        Self::with_clock(kind, Arc::new(SystemClock))
    }
    pub fn with_clock(kind: ApiKind, clock: Arc<dyn Clock>) -> Self {
        assert!(is_linkmetrics(kind));
        Self {
            kind,
            clock,
            serial: AsyncMutex::new(()),
            status: Mutex::new(ApiStatus {
                kind,
                state: ApiState::Disconnected,
                urls_total: 0,
                urls_done: 0,
                urls_failed: 0,
                detail: None,
            }),
            quotas: Mutex::new(vec![]),
            transport: AsyncMutex::new(None),
            last_free: AsyncMutex::new(None),
        }
    }
    fn base(&self, ctx: &ApiCtx) -> String {
        let production = match self.kind {
            ApiKind::Majestic => "https://api.majestic.com",
            ApiKind::Ahrefs => "https://api.ahrefs.com",
            _ => "https://api.moz.com",
        };
        http::base_url(
            self.kind,
            config(ctx, self.kind).base_url.as_deref(),
            production,
            ctx.allow_api_override,
        )
    }
    async fn transport(&self, ctx: &ApiCtx) -> Result<Arc<Transport>, ApiError> {
        let cfg = config(ctx, self.kind);
        let key = serde_json::to_string(&(
            cfg,
            &ctx.network,
            &ctx.cfg.network.resolve,
            ctx.allow_api_override,
        ))
        .map_err(|_| invalid())?;
        let mut slot = self.transport.lock().await;
        if let Some((previous, transport)) = slot.as_ref().filter(|(previous, _)| *previous == key)
        {
            let _ = previous;
            return Ok(transport.clone());
        }
        let mut limits = cfg.limits.clone();
        let options = cfg.options.clone();
        if self.kind == ApiKind::Moz && !options.moz_paid {
            let configured = limits
                .requests_per_window
                .map(|n| limits.window_s.div_ceil(n.max(1)))
                .unwrap_or(0);
            limits.requests_per_window = Some(1);
            limits.window_s = configured.max(options.moz_free_interval_s);
        }
        let transport = Transport::for_ctx(ctx, limits)?;
        *slot = Some((key, transport.clone()));
        Ok(transport)
    }
    fn credential_token<'a>(&self, credential: &'a Credential) -> Result<&'a str, ApiError> {
        if self.kind == ApiKind::Ahrefs
            && credential
                .expires_at
                .is_some_and(|at| at <= self.clock.now())
        {
            return Err(ApiError::Unauthorized(
                "Ahrefs-Token abgelaufen – bitte neu verbinden".into(),
            ));
        }
        let token = if self.kind == ApiKind::Ahrefs {
            credential.access_token.as_deref()
        } else {
            credential.api_key.as_deref()
        };
        token
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| ApiError::NotConnected(self.kind.to_string()))
    }
    async fn response(
        &self,
        ctx: &ApiCtx,
        request: reqwest::RequestBuilder,
        probe: bool,
        token: &str,
    ) -> Result<Value, ApiError> {
        let response = if probe {
            request.send().await.map_err(|_| invalid())?
        } else {
            self.transport(ctx)
                .await?
                .execute(request, self.clock.now(), 150)
                .await?
        };
        let status = response.status().as_u16();
        if self.kind == ApiKind::Ahrefs && status == 401 {
            // Auch eine leere 401-Antwort muss als widerrufene Anmeldung erkennbar sein.
            return Err(ApiError::Unauthorized("Ahrefs-Zugang widerrufen".into()));
        }
        if self.kind == ApiKind::Ahrefs {
            if let Some(cost) = response
                .headers()
                .get("x-api-units-cost-total-actual")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
            {
                for quota in self
                    .quotas
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .iter_mut()
                {
                    quota.used = quota.used.saturating_add(cost);
                }
            }
        }
        let value: Value = serde_json::from_slice(&response.bytes().await.map_err(|_| invalid())?)
            .map_err(|_| invalid())?;
        if status >= 300
            || value.get("error").is_some()
            || value.get("Code").is_some_and(|v| v != "OK")
        {
            let text = value["ErrorMessage"]
                .as_str()
                .or(value.pointer("/error/message").and_then(Value::as_str))
                .unwrap_or("API-Zugang verweigert");
            let mut safe = text.replace(token, "[redacted]");
            if let Some(private) = ctx
                .secrets
                .get("api/majestic/openapp_private_key")
                .filter(|s| !s.is_empty())
            {
                safe = safe.replace(&private, "[redacted]");
            }
            let message = safe.chars().filter(|c| !c.is_control()).take(300).collect();
            return Err(ApiError::Provider {
                status: if status < 300 { 403 } else { status },
                message,
            });
        }
        if let Some(q) = value.get("limits_and_usage") {
            let mut quotas = vec![];
            for scope in ["api_key", "workspace"] {
                if let Some(used) = q[format!("units_usage_{scope}")].as_u64() {
                    quotas.push(ProviderQuota {
                        dimension: format!("units_{scope}"),
                        used,
                        limit: q[format!("units_limit_{scope}")].as_u64(),
                        resets_at: None,
                    });
                }
            }
            *self.quotas.lock().unwrap_or_else(|p| p.into_inner()) = quotas;
        }
        Ok(value)
    }
    fn check_quota(&self) -> Result<(), ApiError> {
        if self.quotas().iter().any(|quota| {
            quota.limit.is_some_and(|limit| quota.used >= limit)
                && quota.resets_at.is_none_or(|reset| reset > self.clock.now())
        }) {
            return Err(ApiError::QuotaExceeded(
                "Anbieterkontingent erschöpft".into(),
            ));
        }
        Ok(())
    }
    async fn fetch_inner(&self, urls: &[UrlRef], ctx: &ApiCtx) -> Result<Grouped, ApiError> {
        let cfg = config(ctx, self.kind);
        let opts = cfg.options.clone();
        validate(self.kind, cfg)?;
        let credential = read_account(
            ctx.secrets.as_ref(),
            self.kind,
            if self.kind == ApiKind::Ahrefs {
                opts.workspace.as_deref().or(cfg.account.as_deref())
            } else {
                cfg.account.as_deref()
            },
        )?;
        let token = self.credential_token(&credential)?;
        let mut groups: BTreeMap<Target, Vec<&UrlRef>> = BTreeMap::new();
        for url in urls {
            for level in &cfg.levels {
                groups
                    .entry(target(&url.url, *level)?)
                    .or_default()
                    .push(url);
            }
        }
        let targets: Vec<_> = groups.keys().cloned().collect();
        let batch_size = if self.kind == ApiKind::Moz {
            if opts.moz_paid {
                50
            } else {
                1
            }
        } else {
            100
        };
        let transport = self.transport(ctx).await?;
        let base = self.base(ctx);
        let mut output: Grouped = urls
            .iter()
            .map(|url| ((url.url_id, url.url.clone()), Ok(vec![])))
            .collect();
        for chunk in targets.chunks(batch_size) {
            let traffic_groups = if self.kind == ApiKind::Ahrefs {
                vec![false, true]
            } else {
                vec![false]
            };
            for traffic in traffic_groups {
                let metrics: Vec<_> = cfg
                    .metrics
                    .iter()
                    .filter(|id| {
                        self.kind != ApiKind::Moz || id.as_str() != "brand_authority_score"
                    })
                    .filter(|id| {
                        self.kind != ApiKind::Ahrefs
                            || catalog::catalog(self.kind)
                                .iter()
                                .any(|m| m.id == *id && m.traffic == traffic)
                    })
                    .collect();
                if metrics.is_empty() {
                    continue;
                }
                if self.kind == ApiKind::Moz && !opts.moz_paid {
                    let mut last = self.last_free.lock().await;
                    if let Some(at) = *last {
                        tokio::time::sleep_until(
                            at + Duration::from_secs(u64::from(opts.moz_free_interval_s)),
                        )
                        .await;
                    }
                    *last = Some(tokio::time::Instant::now());
                }
                let request = match self.kind {
                    ApiKind::Majestic => {
                        let mut form = vec![
                            ("cmd".to_string(), "GetIndexItemInfo".into()),
                            ("accesstoken".into(), token.into()),
                            (
                                "datasource".into(),
                                if opts.historic { "historic" } else { "fresh" }.into(),
                            ),
                            ("items".into(), chunk.len().to_string()),
                        ];
                        let private = ctx
                            .secrets
                            .get("api/majestic/openapp_private_key")
                            .ok_or_else(|| {
                                ApiError::NotConnected("Majestic OpenApp-Kennung fehlt".into())
                            })?;
                        form.push(("privatekey".into(), private));
                        form.extend(
                            chunk
                                .iter()
                                .enumerate()
                                .map(|(i, t)| (format!("item{i}"), t.query.clone())),
                        );
                        transport
                            .client
                            .post(format!("{base}/api_command"))
                            .header("Content-Type", "application/x-www-form-urlencoded")
                            .body(
                                url::form_urlencoded::Serializer::new(String::new())
                                    .extend_pairs(
                                        form.iter().map(|(k, v)| (k.as_str(), v.as_str())),
                                    )
                                    .finish(),
                            )
                    }
                    ApiKind::Ahrefs => {
                        let targets: Vec<_> = chunk.iter().map(|t| json!({"url": if matches!(t.level, ApiLevel::Domain | ApiLevel::Subdomain) { format!("https://{}", t.query) } else { t.query.clone() }, "mode": match t.level { ApiLevel::Domain => "subdomains", ApiLevel::Subdomain => "domain", _ => "exact" }, "protocol": t.protocol})).collect();
                        let selected: std::collections::BTreeSet<&str> = metrics
                            .iter()
                            .flat_map(|id| {
                                if id.as_str() == "org_keywords_top10" {
                                    vec!["org_keywords_1_3", "org_keywords_4_10"]
                                } else {
                                    vec![id.as_str()]
                                }
                            })
                            .collect();
                        let mut body = json!({"targets":targets,"select":selected,"output":"json"});
                        if traffic {
                            body["volume_mode"] = json!(if opts.average_volume {
                                "average"
                            } else {
                                "monthly"
                            });
                            if let Some(country) = opts.country.as_ref().filter(|s| !s.is_empty()) {
                                body["country"] = json!(country.to_ascii_lowercase());
                            }
                        }
                        transport
                            .client
                            .post(format!("{base}/v3/batch-analysis/batch-analysis"))
                            .bearer_auth(token)
                            .header("Content-Type", "application/json")
                            .body(body.to_string())
                    }
                    _ => {
                        let queries: Vec<_> = chunk
                            .iter()
                            .map(|t| json!({"query":t.query,"scope":t.level.to_string()}))
                            .collect();
                        transport.client.post(format!("{base}/jsonrpc")).header("x-moz-token", token).header("Content-Type", "application/json").body(json!({"jsonrpc":"2.0","id":oauth2::CsrfToken::new_random().secret(),"method":"data.site.metrics.fetch.multiple","params":{"data":{"site_queries":queries}}}).to_string())
                    }
                };
                let response = match self.check_quota() {
                    Ok(()) => self.response(ctx, request, false, token).await,
                    Err(error) => Err(error),
                };
                let data = match response {
                    Ok(data) => data,
                    Err(error) => {
                        for t in chunk {
                            for url in &groups[t] {
                                output.insert((url.url_id, url.url.clone()), Err(error.clone()));
                            }
                        }
                        continue;
                    }
                };
                let rows = match self.kind {
                    ApiKind::Majestic => data.pointer("/DataTables/Results/Data"),
                    ApiKind::Ahrefs => data.get("targets"),
                    _ => data.pointer("/result/results_by_site"),
                }
                .and_then(Value::as_array);
                for (index, t) in chunk.iter().enumerate() {
                    let row = rows.into_iter().flatten().find(|row| match self.kind {
                        ApiKind::Majestic => row["Item"].as_str() == Some(t.query.as_str()),
                        ApiKind::Ahrefs => row["index"].as_u64() == Some(index as u64),
                        _ => {
                            row.pointer("/site_query/original_site_query/query")
                                .or_else(|| row.pointer("/site_query/query"))
                                .and_then(Value::as_str)
                                == Some(t.query.as_str())
                                && row.pointer("/site_query/scope").and_then(Value::as_str)
                                    == Some(t.level.to_string().as_str())
                        }
                    });
                    let parsed = (|| -> Outcome {
                        let row = row.ok_or_else(invalid)?;
                        if (self.kind == ApiKind::Majestic
                            && row.get("ResultCode").is_some_and(|code| code != "OK"))
                            || (self.kind == ApiKind::Moz
                                && data
                                    .pointer("/result/errors_by_site")
                                    .and_then(Value::as_array)
                                    .is_some_and(|errors| {
                                        errors.iter().any(|error| {
                                            let query = error
                                                .pointer("/site_query/original_site_query/query")
                                                .or_else(|| error.pointer("/site_query/query"))
                                                .and_then(Value::as_str);
                                            query == Some(t.query.as_str())
                                                && error
                                                    .pointer("/site_query/scope")
                                                    .and_then(Value::as_str)
                                                    == Some(t.level.to_string().as_str())
                                        })
                                    }))
                        {
                            return Err(invalid());
                        }
                        let mut values = vec![];
                        let row = if self.kind == ApiKind::Moz {
                            &row["site_metrics"]
                        } else {
                            row
                        };
                        for metric in &metrics {
                            let field = moz_field(metric, t.level);
                            let derived = if metric.as_str() == "org_keywords_top10" {
                                row["org_keywords_1_3"]
                                    .as_i64()
                                    .zip(row["org_keywords_4_10"].as_i64())
                                    .and_then(|(a, b)| a.checked_add(b))
                                    .map_or(Value::Null, |sum| json!(sum))
                            } else {
                                Value::Null
                            };
                            let value = if metric.as_str() == "org_keywords_top10" {
                                &derived
                            } else {
                                &row[if self.kind == ApiKind::Moz {
                                    field.as_str()
                                } else {
                                    metric.as_str()
                                }]
                            };
                            if value.is_null() {
                                return Err(invalid());
                            }
                            let meta = catalog::catalog(self.kind)
                                .iter()
                                .find(|m| m.id == metric.as_str())
                                .ok_or_else(invalid)?;
                            let (real, text) = parse_value(meta, value)?;
                            for url in &groups[t] {
                                values.push(ApiValueWrite {
                                    url_id: url.url_id,
                                    url: url.url.clone(),
                                    source: self.kind.to_string(),
                                    metric: (*metric).clone(),
                                    level: t.level.to_string(),
                                    v_real: real,
                                    v_text: text.clone(),
                                    fetched_at: self.clock.now(),
                                });
                            }
                        }
                        Ok(values)
                    })();
                    match parsed {
                        Ok(values) => {
                            for value in values {
                                if let Some(Ok(stored)) =
                                    output.get_mut(&(value.url_id, value.url.clone()))
                                {
                                    stored.push(value);
                                }
                            }
                        }
                        Err(error) => {
                            for url in &groups[t] {
                                output.insert((url.url_id, url.url.clone()), Err(error.clone()));
                            }
                        }
                    }
                }
            }
        }
        if self.kind == ApiKind::Moz && cfg.metrics.iter().any(|id| id == "brand_authority_score") {
            // Brand Authority hat in v3 einen eigenen, ausschließlich domainweiten Endpunkt.
            let mut domains: BTreeMap<String, Vec<&UrlRef>> = BTreeMap::new();
            for url in urls {
                domains
                    .entry(target(&url.url, ApiLevel::Domain)?.query)
                    .or_default()
                    .push(url);
            }
            for (domain, urls) in domains {
                let request = transport.client.post(format!("{base}/jsonrpc")).header("x-moz-token", token).header("Content-Type", "application/json").body(json!({"jsonrpc":"2.0","id":oauth2::CsrfToken::new_random().secret(),"method":"data.site.metrics.brand.authority.fetch","params":{"data":{"site_query":{"query":domain,"scope":"domain"}}}}).to_string());
                let parsed = async {
                    self.check_quota()?;
                    let data = self.response(ctx, request, false, token).await?;
                    let value = data
                        .pointer("/result/site_metrics/brand_authority_score")
                        .ok_or_else(invalid)?;
                    let meta = catalog::catalog(self.kind)
                        .iter()
                        .find(|m| m.id == "brand_authority_score")
                        .expect("catalog");
                    parse_value(meta, value)
                }
                .await;
                let (v_real, v_text) = match parsed {
                    Ok(value) => value,
                    Err(error) => {
                        for url in urls {
                            output.insert((url.url_id, url.url.clone()), Err(error.clone()));
                        }
                        continue;
                    }
                };
                for url in urls {
                    for level in &cfg.levels {
                        if let Some(Ok(values)) = output.get_mut(&(url.url_id, url.url.clone())) {
                            values.push(ApiValueWrite {
                                url_id: url.url_id,
                                url: url.url.clone(),
                                source: self.kind.to_string(),
                                metric: "brand_authority_score".into(),
                                level: level.to_string(),
                                v_real,
                                v_text: v_text.clone(),
                                fetched_at: self.clock.now(),
                            });
                        }
                    }
                }
            }
        }
        Ok(output)
    }
}
fn moz_field(metric: &str, level: ApiLevel) -> String {
    let suffix = match level {
        ApiLevel::Domain => "root_domain",
        ApiLevel::Subdomain => "subdomain",
        _ => "page",
    };
    match metric {
        "root_domains_linking" => format!("root_domains_to_{suffix}"),
        "external_pages_linking" => format!("external_pages_to_{suffix}"),
        "pages_linking" => format!("pages_to_{suffix}"),
        "nofollow_root_domains" => format!("nofollow_root_domains_to_{suffix}"),
        _ => metric.into(),
    }
}
fn parse_value(
    meta: &catalog::Metric,
    value: &Value,
) -> Result<(Option<f64>, Option<String>), ApiError> {
    if meta.id == "last_crawled" {
        return Ok((None, Some(value.as_str().ok_or_else(invalid)?.into())));
    }
    let number = value.as_f64().ok_or_else(invalid)?;
    let range = match meta.id {
        "TrustFlow" | "CitationFlow" | "spam_score" => Some((0., 100.)),
        "page_authority" | "domain_authority" => Some((1., 100.)),
        _ => None,
    };
    if meta.id == "spam_score" && number == -1. {
        return Ok((None, None));
    }
    if number < 0. {
        return Err(invalid());
    }
    if range.is_some_and(|(low, high)| number < low || number > high) {
        return Err(invalid());
    }
    if meta.decimal {
        Ok((
            Some(if meta.id == "org_cost" {
                number / 100.0
            } else {
                number
            }),
            None,
        ))
    } else {
        // Das Schema v3 hat kein INTEGER-Feld für API-Werte. Dezimaltext erhält alle 64 Bits;
        // die Spaltenquelle liest ihn mit CAST AS INTEGER (kein verlustbehafteter REAL-Umweg).
        let integer = value
            .as_i64()
            .or_else(|| {
                (number.fract() == 0. && number >= 0. && number < i64::MAX as f64)
                    .then_some(number as i64)
            })
            .ok_or_else(invalid)?;
        Ok((None, Some(integer.to_string())))
    }
}
pub fn validate(kind: ApiKind, cfg: &LinkMetricsConfig) -> Result<(), ApiError> {
    let opts = cfg.options.clone();
    if cfg
        .levels
        .iter()
        .any(|level| !catalog::supports_level(kind, *level))
        || cfg.metrics.iter().any(|id| {
            !catalog::catalog(kind)
                .iter()
                .any(|m| m.id == id && (kind != ApiKind::Moz || opts.moz_paid || m.free))
        })
        || opts.moz_free_interval_s == 0
        || opts
            .country
            .as_ref()
            .is_some_and(|c| c.len() != 2 || !c.bytes().all(|b| b.is_ascii_alphabetic()))
    {
        return Err(invalid());
    }
    Ok(())
}
/// Kostenklasse `cost` für M2-K17: konservative Schätzung anhand deduplizierter Ziele.
pub fn estimate_cost(
    kind: ApiKind,
    urls: &[UrlRef],
    cfg: &LinkMetricsConfig,
) -> Result<u64, ApiError> {
    validate(kind, cfg)?;
    let mut targets = std::collections::BTreeSet::new();
    for url in urls {
        for level in &cfg.levels {
            targets.insert(target(&url.url, *level)?);
        }
    }
    let rows = targets.len() as u64;
    if cfg.metrics.is_empty() {
        return Ok(0);
    }
    Ok(if kind == ApiKind::Ahrefs {
        let link = cfg.metrics.iter().any(|id| {
            catalog::catalog(kind)
                .iter()
                .any(|m| m.id == id && !m.traffic)
        });
        let traffic = cfg.metrics.iter().any(|id| {
            catalog::catalog(kind)
                .iter()
                .any(|m| m.id == id && m.traffic)
        });
        rows.saturating_mul(cfg.metrics.len() as u64 * 10).max(
            if cfg.limits.quota_limit.is_some() {
                rows.saturating_mul(50)
            } else {
                rows.div_ceil(100) * 50
            },
        ) * (u64::from(link) + u64::from(traffic))
    } else {
        rows
    })
}
impl ApiSource for LinkMetrics {
    fn kind(&self) -> ApiKind {
        self.kind
    }
    fn status(&self) -> ApiStatus {
        self.status
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
    fn quotas(&self) -> Vec<ProviderQuota> {
        self.quotas
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
    fn fetch<'a>(
        &'a self,
        urls: &'a [UrlRef],
        ctx: &'a ApiCtx,
    ) -> BoxFuture<'a, Result<Vec<ApiValueWrite>, ApiError>> {
        Box::pin(async move {
            let mut values = vec![];
            for (_, result) in self.fetch_outcomes(urls, ctx).await {
                values.extend(result?);
            }
            Ok(values)
        })
    }
    fn fetch_outcomes<'a>(
        &'a self,
        urls: &'a [UrlRef],
        ctx: &'a ApiCtx,
    ) -> BoxFuture<'a, Vec<(UrlRef, Outcome)>> {
        Box::pin(async move {
            let _serial = self.serial.lock().await;
            {
                let mut status = self.status.lock().unwrap_or_else(|p| p.into_inner());
                status.state = ApiState::Fetching;
                status.urls_total = urls.len() as u64;
                status.urls_done = 0;
                status.urls_failed = 0;
                status.detail = None;
            }
            let result = self.fetch_inner(urls, ctx).await;
            let outcomes: Vec<_> = urls
                .iter()
                .map(|url| {
                    let outcome = match &result {
                        Ok(values) => values
                            .get(&(url.url_id, url.url.clone()))
                            .cloned()
                            .unwrap_or_else(|| Err(invalid())),
                        Err(error) => Err(error.clone()),
                    };
                    (url.clone(), outcome)
                })
                .collect();
            let mut status = self.status.lock().unwrap_or_else(|p| p.into_inner());
            status.urls_total = urls.len() as u64;
            status.urls_done = urls.len() as u64;
            status.urls_failed = outcomes
                .iter()
                .filter(|(_, outcome)| outcome.is_err())
                .count() as u64;
            let first_error = outcomes
                .iter()
                .find_map(|(_, outcome)| outcome.as_ref().err());
            status.state = match first_error {
                None => ApiState::Idle,
                Some(
                    ApiError::Unauthorized(_)
                    | ApiError::RateLimited(_)
                    | ApiError::QuotaExceeded(_),
                ) => ApiState::Paused,
                Some(_) => ApiState::Failed,
            };
            status.detail = first_error.map(ApiError::public_message);
            outcomes
        })
    }
    fn test_connection<'a>(
        &'a self,
        ctx: &'a ApiCtx,
        credential: &'a Credential,
    ) -> BoxFuture<'a, Result<Credential, ApiError>> {
        Box::pin(async move {
            let token = self.credential_token(credential)?;
            let base = self.base(ctx);
            let client = http::client_for(ctx)?;
            let request = match self.kind {
                ApiKind::Majestic => {
                    let private = ctx.secrets.get("api/majestic/openapp_private_key").ok_or_else(|| ApiError::NotConnected("Majestic OpenApp-Kennung fehlt".into()))?;
                    client.post(format!("{base}/api_command")).header("Content-Type", "application/x-www-form-urlencoded").body(url::form_urlencoded::Serializer::new(String::new()).extend_pairs([("cmd", "GetSubscriptionInfo"), ("accesstoken", token), ("privatekey", private.as_str())]).finish())
                },
                ApiKind::Ahrefs => client.get(format!("{base}/v3/subscription-info/limits-and-usage")).bearer_auth(token),
                _ => client.post(format!("{base}/jsonrpc")).header("x-moz-token", token).header("Content-Type", "application/json").body(json!({"jsonrpc":"2.0","id":oauth2::CsrfToken::new_random().secret(),"method":"quota.lookup","params":{"data":{"path":"api.limits.data.rows"}}}).to_string()),
            };
            self.response(ctx, request, true, token).await?;
            self.status.lock().unwrap_or_else(|p| p.into_inner()).state = ApiState::Idle;
            Ok(credential.clone())
        })
    }
}
