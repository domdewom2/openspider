//! Gemeinsamer Netzwerkpfad: app-weite Proxy-/Truststore-Einstellungen, keine Redirects
//! mit Zugangsdaten, Quellenlimits und wiederholbare HTTP-Fehler.
use crate::ApiError;
use os_model::{config::ApiLimitsConfig, settings::NetworkSettings, Timestamp};
use std::{collections::VecDeque, sync::Arc, time::Duration};
use tokio::sync::{Mutex, Semaphore};

pub fn base_url(
    kind: os_model::ApiKind,
    configured: Option<&str>,
    production: &str,
    allow: bool,
) -> String {
    resolve_base(
        configured,
        std::env::var(format!(
            "OPENSPIDER_API_BASE_{}",
            kind.to_string().to_uppercase()
        ))
        .ok()
        .as_deref(),
        production,
        cfg!(debug_assertions),
        allow,
    )
}
pub fn resolve_base(
    configured: Option<&str>,
    environment: Option<&str>,
    production: &str,
    debug: bool,
    allow: bool,
) -> String {
    if debug || allow {
        environment.or(configured).unwrap_or(production).to_string()
    } else {
        production.to_string()
    }
}
pub fn client(network: &NetworkSettings) -> Result<reqwest::Client, ApiError> {
    os_fetch::net::client_builder(network)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|_| ApiError::Transport("API-Netzwerkclient nicht verfügbar".into()))
}
pub struct Transport {
    pub client: reqwest::Client,
    limits: ApiLimitsConfig,
    semaphore: Semaphore,
    sent: Mutex<VecDeque<tokio::time::Instant>>,
    rate_clock: Arc<dyn Fn() -> tokio::time::Instant + Send + Sync>,
    cooldown: Mutex<Option<tokio::time::Instant>>,
}
impl Transport {
    pub fn new(network: &NetworkSettings, limits: ApiLimitsConfig) -> Result<Arc<Self>, ApiError> {
        Self::with_client(client(network)?, limits)
    }
    pub fn for_ctx(ctx: &crate::ApiCtx, limits: ApiLimitsConfig) -> Result<Arc<Self>, ApiError> {
        Self::with_client(client_for(ctx)?, limits)
    }
    /// Injects the monotonic rate-limit clock; normal clients use Tokio's clock.
    pub fn with_rate_clock(
        network: &NetworkSettings,
        limits: ApiLimitsConfig,
        clock: impl Fn() -> tokio::time::Instant + Send + Sync + 'static,
    ) -> Result<Arc<Self>, ApiError> {
        Self::with_client_and_clock(client(network)?, limits, Arc::new(clock))
    }
    fn with_client(
        client: reqwest::Client,
        limits: ApiLimitsConfig,
    ) -> Result<Arc<Self>, ApiError> {
        Self::with_client_and_clock(client, limits, Arc::new(tokio::time::Instant::now))
    }
    fn with_client_and_clock(
        client: reqwest::Client,
        limits: ApiLimitsConfig,
        rate_clock: Arc<dyn Fn() -> tokio::time::Instant + Send + Sync>,
    ) -> Result<Arc<Self>, ApiError> {
        if !(1..=50).contains(&limits.concurrency)
            || limits.window_s == 0
            || limits.requests_per_window == Some(0)
            || limits.retries > 20
        {
            return Err(ApiError::Transport("Ungültige API-Limits".into()));
        }
        Ok(Arc::new(Self {
            client,
            semaphore: Semaphore::new(limits.concurrency as usize),
            limits,
            sent: Mutex::default(),
            rate_clock,
            cooldown: Mutex::default(),
        }))
    }
    async fn acquire_rate(&self) {
        loop {
            let now = (self.rate_clock)();
            let cooldown = *self.cooldown.lock().await;
            if let Some(until) = cooldown.filter(|until| *until > now) {
                tokio::time::sleep_until(until).await;
                continue;
            }
            let Some(limit) = self.limits.requests_per_window else {
                return;
            };
            let mut sent = self.sent.lock().await;
            let window = Duration::from_secs(u64::from(self.limits.window_s));
            while sent
                .front()
                .is_some_and(|t| now.duration_since(*t) >= window)
            {
                sent.pop_front();
            }
            if sent.len() < limit as usize {
                sent.push_back(now);
                return;
            }
            let until = *sent.front().expect("nonempty window") + window;
            drop(sent);
            tokio::time::sleep_until(until).await;
        }
    }
    /// Das Future kann durch Drop/Cancel abgebrochen werden; keine Hintergrund-Wiederholung.
    pub async fn execute(
        &self,
        request: reqwest::RequestBuilder,
        now: Timestamp,
        seed: u64,
    ) -> Result<reqwest::Response, ApiError> {
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| ApiError::Transport("API geschlossen".into()))?;
        let start = tokio::time::Instant::now();
        for attempt in 0..=self.limits.retries {
            self.acquire_rate().await;
            let response = request
                .try_clone()
                .ok_or_else(|| ApiError::Transport("API-Anfrage nicht wiederholbar".into()))?
                .send()
                .await
                .map_err(|_| ApiError::Transport("API-Verbindung fehlgeschlagen".into()))?;
            let status = response.status().as_u16();
            if response.status().is_success() {
                return Ok(response);
            }
            let retry = matches!(status, 429 | 500 | 502 | 503 | 504);
            let delay = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| retry_after(v, now.plus(start.elapsed())))
                .unwrap_or_else(|| backoff(attempt, seed));
            if matches!(status, 403 | 429) {
                let bytes = response
                    .bytes()
                    .await
                    .map_err(|_| ApiError::Transport("API-Antwort unlesbar".into()))?;
                let quota = serde_json::from_slice::<serde_json::Value>(&bytes)
                    .ok()
                    .is_some_and(|value| {
                        value.pointer("/error/status").and_then(|v| v.as_str())
                            == Some("RESOURCE_EXHAUSTED")
                            || value
                                .pointer("/error/errors")
                                .and_then(|v| v.as_array())
                                .is_some_and(|errors| {
                                    errors
                                        .iter()
                                        .any(|error| error["reason"] == "quotaExceeded")
                                })
                    });
                if quota {
                    return Err(ApiError::QuotaExceeded(
                        "Anbieterkontingent erschöpft".into(),
                    ));
                }
            }
            // Nie Anbietertexte übernehmen: sie können Schlüssel, URL-Parameter oder Tokens spiegeln.
            if !retry || attempt == self.limits.retries {
                return Err(if matches!(status, 401 | 403) {
                    ApiError::Unauthorized(format!("HTTP {status}: Zugang verweigert"))
                } else if status == 429 {
                    ApiError::RateLimited("HTTP 429: Anfragelimit erreicht".into())
                } else {
                    ApiError::Transport(format!("HTTP {status}: API-Anfrage fehlgeschlagen"))
                });
            }
            let until = tokio::time::Instant::now() + delay;
            let mut cooldown = self.cooldown.lock().await;
            *cooldown = Some(cooldown.map_or(until, |old| old.max(until)));
            drop(cooldown);
            tokio::time::sleep_until(until).await;
        }
        unreachable!()
    }
}
pub fn backoff(attempt: u32, seed: u64) -> Duration {
    let x = seed
        .wrapping_add(u64::from(attempt))
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    let jitter = 0.8 + (x % 4001) as f64 / 10000.0;
    Duration::from_secs_f64((2_f64.powi(attempt.min(20) as i32) * jitter).min(60.0))
}
pub fn retry_after(value: &str, now: Timestamp) -> Option<Duration> {
    if let Ok(secs) = value.trim().parse::<u64>() {
        return Some(Duration::from_secs(secs));
    }
    let format = time::format_description::parse_borrowed::<2>(
        "[weekday repr:short], [day] [month repr:short] [year] [hour]:[minute]:[second] GMT",
    )
    .ok()?;
    let date = time::PrimitiveDateTime::parse(value, &format)
        .ok()?
        .assume_utc();
    Some(Duration::from_millis(
        (date.unix_timestamp_nanos() / 1_000_000 - i128::from(now.unix_millis())).max(0) as u64,
    ))
}

/// Ein einzelner Prüfabruf; Wiederholungen gehören ausschließlich zum Datenabruf.
pub async fn probe(
    kind: os_model::ApiKind,
    ctx: &crate::ApiCtx,
    token: Option<&str>,
) -> Result<(), ApiError> {
    use os_model::ApiKind;
    let (configured, production, path) = match kind {
        ApiKind::Ga4 => (
            ctx.cfg.api.ga4.base_url.as_deref(),
            "https://analyticsadmin.googleapis.com",
            "/v1beta/accountSummaries",
        ),
        ApiKind::Gsc => (
            ctx.cfg.api.gsc.base_url.as_deref(),
            "https://www.googleapis.com",
            "/webmasters/v3/sites",
        ),
        ApiKind::Psi => (
            ctx.cfg.api.psi.base_url.as_deref(),
            "https://www.googleapis.com",
            "/pagespeedonline/v5/runPagespeed?url=https%3A%2F%2Fexample.com",
        ),
        ApiKind::Majestic => (
            ctx.cfg.api.majestic.base_url.as_deref(),
            "https://api.majestic.com",
            "/api/json?cmd=GetSubscriptionInfo",
        ),
        ApiKind::Ahrefs => (
            ctx.cfg.api.ahrefs.base_url.as_deref(),
            "https://api.ahrefs.com",
            "/v3/subscription-info/limits-and-usage",
        ),
        ApiKind::Moz => (
            ctx.cfg.api.moz.base_url.as_deref(),
            "https://api.moz.com",
            "/v2/usage_data",
        ),
        _ => return Err(ApiError::NotImplemented("Google Drive")),
    };
    let base = base_url(kind, configured, production, ctx.allow_api_override);
    let mut url = url::Url::parse(&format!("{}{path}", base.trim_end_matches('/')))
        .map_err(|_| ApiError::Transport("Ungültige API-Basis-URL".into()))?;
    if let Some(token) = token {
        if kind == ApiKind::Psi {
            url.query_pairs_mut().append_pair("key", token);
        } else if kind == ApiKind::Majestic {
            url.query_pairs_mut().append_pair("app_api_key", token);
        }
    }
    let mut request = client_for(ctx)?.get(url);
    if let Some(token) = token {
        if !matches!(kind, ApiKind::Psi | ApiKind::Majestic) {
            request = request.bearer_auth(token);
        }
    }
    let response = request
        .send()
        .await
        .map_err(|_| ApiError::Transport("API-Verbindung fehlgeschlagen".into()))?;
    let code = response.status().as_u16();
    if response.status().is_success() {
        Ok(())
    } else {
        let bytes = response
            .bytes()
            .await
            .map_err(|_| ApiError::Transport("API-Antwort unlesbar".into()))?;
        let json = serde_json::from_slice::<serde_json::Value>(&bytes).unwrap_or_default();
        let mut message = json
            .pointer("/error/message")
            .or_else(|| json.get("message"))
            .and_then(|v| v.as_str())
            .or_else(|| json.get("error").and_then(|v| v.as_str()))
            .unwrap_or("API-Zugang verweigert")
            .to_string();
        if let Some(token) = token.filter(|t| !t.is_empty()) {
            message = message.replace(token, "[redacted]");
        }
        message = message
            .chars()
            .filter(|c| !c.is_control())
            .take(300)
            .collect();
        Err(ApiError::Provider {
            status: code,
            message,
        })
    }
}

/// Der API-Resolver ist im Release nur mit expliziter Freigabe aktiv (CRAWL-123).
pub fn client_with_resolve(
    network: &NetworkSettings,
    rules: &[String],
    debug: bool,
    allow: bool,
) -> Result<reqwest::Client, ApiError> {
    build_client(network, rules, debug, allow, None)
}
fn build_client(
    network: &NetworkSettings,
    rules: &[String],
    debug: bool,
    allow: bool,
    secrets: Option<&dyn os_fetch::SecretStore>,
) -> Result<reqwest::Client, ApiError> {
    let effective = if debug || allow { rules } else { &[] };
    let resolver = os_fetch::resolve::HostResolver::new(effective, Duration::from_secs(10))
        .map_err(|_| ApiError::Transport("Ungültige API-Host-Auflösung".into()))?;
    let builder =
        if network.proxy.mode == os_model::ProxyMode::Manual && network.proxy.username.is_some() {
            let mut direct = network.clone();
            direct.proxy.mode = os_model::ProxyMode::None;
            let scheme = match network.proxy.kind {
                os_model::ProxyKind::Http => "http",
                os_model::ProxyKind::Https => "https",
                os_model::ProxyKind::Socks5 => "socks5h",
            };
            let host = if network.proxy.host.contains(':') && !network.proxy.host.starts_with('[') {
                format!("[{}]", network.proxy.host)
            } else {
                network.proxy.host.clone()
            };
            let password = secrets
                .and_then(|s| {
                    s.get("openspider/proxy/password")
                        .or_else(|| s.get("proxy/password"))
                })
                .unwrap_or_default();
            let proxy = reqwest::Proxy::all(format!("{scheme}://{host}:{}", network.proxy.port))
                .map_err(|_| ApiError::Transport("Ungültiger API-Proxy".into()))?
                .basic_auth(
                    network.proxy.username.as_deref().unwrap_or_default(),
                    &password,
                )
                .no_proxy(reqwest::NoProxy::from_string(
                    &network.proxy.bypass.join(","),
                ));
            os_fetch::net::client_builder(&direct).proxy(proxy)
        } else {
            os_fetch::net::client_builder(network)
        };
    builder
        .dns_resolver(Arc::new(resolver))
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|_| ApiError::Transport("API-Netzwerkclient nicht verfügbar".into()))
}
pub fn client_for(ctx: &crate::ApiCtx) -> Result<reqwest::Client, ApiError> {
    build_client(
        &ctx.network,
        &ctx.cfg.network.resolve,
        cfg!(debug_assertions),
        ctx.allow_api_override,
        Some(ctx.secrets.as_ref()),
    )
}
