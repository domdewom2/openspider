//! HTTP-Abrufer (`HttpFetcher`) und seine Konfiguration (CRAWL-011/012/013/022/088/095/113/117/123).
//!
//! Aufbau eines Abrufs:
//!
//! 1. Rate-Limit abwarten (gilt für alle Zwecke, auch robots.txt); die Wartezeit zählt nicht zur Antwortzeit.
//! 2. `GET` mit festem Header-Satz (`User-Agent`, `Accept`, `Accept-Encoding: gzip, deflate, br`,
//!    `Connection: keep-alive`), dazu `Authorization` (nur zum Präfix passend) und `Cookie` (je nach Modus).
//! 3. Keine automatische Dekompression und keine Weiterleitungen: Die Engine verfolgt `Location` selbst
//!    und reicht den Cookie-Speicher weiter.
//! 4. Body in Stücken lesen (bis `max_body_bytes`), die gezählten Bytes sind `Übertragen`; danach selbst
//!    dekomprimieren. Defekte Kompression setzt `decode_error`.
//! 5. Zeitgrenzen: Verbindungsaufbau `min(10 s, Antwort-Timeout)`, gesamte Antwort einschließlich eines
//!    tröpfelnden Bodys `Antwort-Timeout` (+ 100 ms Vorlauf, damit ein hängender Verbindungsaufbau vor der
//!    Gesamtgrenze als „Verbindungs-Timeout“ erkannt wird).

mod classify;
mod decode;
mod hsts;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use md5::{Digest, Md5};
use os_model::{Clock, CookieMode, CrawlConfig, NoResponseReason};
use reqwest::header::{self, HeaderMap, HeaderValue};

use crate::auth::AuthMatcher;
use crate::cookies::CookieJar;
use crate::error::FetchError;
use crate::http::{
    status_text, version_label, Fetch, FetchOutcome, FetchPurpose, FetchRequest, FetchResult,
    HttpResponse,
};
use crate::rate::RateLimiter;
use crate::redact;
use crate::resolve::HostResolver;
use crate::secrets::SecretStore;
use crate::user_agents;

use classify::{classify, Failure};
use decode::parse_encodings;

/// Obergrenze des Verbindungsaufbaus (CRAWL-095, OS-Default).
const MAX_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Vorlauf der Gesamtgrenze vor dem Verbindungs-Timeout (siehe Modulkopf, Punkt 5).
const OVERALL_SLACK: Duration = Duration::from_millis(100);

const ACCEPT_PAGE: &str = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8";
const ACCEPT_ANY: &str = "*/*";
const ACCEPT_ENCODING: &str = "gzip, deflate, br";

/// Basic-Auth-Eintrag mit aufgelöstem Passwort.
#[derive(Clone)]
pub struct BasicAuthEntry {
    /// URL-Präfix, für das der Eintrag gilt.
    pub prefix: String,
    /// Benutzername.
    pub username: String,
    /// Passwort aus dem `SecretStore`.
    pub password: String,
}

// Nie das Passwort ausgeben (OUT-225/OUT-243).
impl std::fmt::Debug for BasicAuthEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BasicAuthEntry")
            .field("prefix", &redact::url(&self.prefix))
            .field("username", &self.username)
            .field("password", &redact::REDACTED)
            .finish()
    }
}

/// Konfiguration des Abrufers, abgeleitet aus der Crawl-Konfiguration.
#[derive(Clone)]
pub struct FetchConfig {
    /// HTTP-Fachkonfiguration.
    pub http: os_model::HttpConfig,
    /// App-weite Netzwerkeinstellungen, beim Start eingefroren.
    pub network: os_model::settings::NetworkSettings,
    /// Aufgelöstes Proxy-Passwort (nie in Debug-Ausgaben).
    pub proxy_password: Option<String>,
    /// HTTP-User-Agent.
    pub user_agent: String,
    /// Gesamtzeit für eine Antwort.
    pub response_timeout: Duration,
    /// Cookie-Verhalten.
    pub cookies: CookieMode,
    /// Basic-Auth-Einträge.
    pub basic_auth: Vec<BasicAuthEntry>,
    /// Resolve-Regeln `host=ip[:port]` (`*.domain` erlaubt).
    pub resolve: Vec<String>,
    /// Höchstzahl Abrufe pro Sekunde (für alle Abrufe inkl. robots.txt).
    pub max_urls_per_sec: Option<f64>,
    /// Höchste Body-Größe in Bytes.
    pub max_body_bytes: u64,
    /// Zusätzliche Wurzelzertifikate (PEM), nur Tests.
    pub extra_root_certs: Vec<Vec<u8>>,
    /// Uhr für Zeitstempel und Antwortzeiten.
    pub clock: Arc<dyn Clock>,
}

impl std::fmt::Debug for FetchConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FetchConfig")
            .field("user_agent", &self.user_agent)
            .field("response_timeout", &self.response_timeout)
            .field("cookies", &self.cookies)
            .field("basic_auth", &self.basic_auth)
            .field("resolve", &self.resolve)
            .field("max_urls_per_sec", &self.max_urls_per_sec)
            .field("max_body_bytes", &self.max_body_bytes)
            .field("extra_root_certs", &self.extra_root_certs.len())
            .finish()
    }
}

impl FetchConfig {
    /// Leitet die Abruf-Konfiguration aus der Crawl-Konfiguration ab.
    ///
    /// Fehlt ein Passwort im `SecretStore` (anderer Rechner, gelöschter Eintrag), ist das ein
    /// Konfigurationsfehler mit verständlichem Text, kein Absturz (OUT-225).
    pub fn from_crawl_config(
        cfg: &CrawlConfig,
        secrets: &dyn SecretStore,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, FetchError> {
        let basic_auth = cfg
            .auth
            .basic
            .iter()
            .map(|b| {
                secrets
                    .get(&b.secret_ref)
                    .map(|password| BasicAuthEntry {
                        prefix: b.prefix.clone(),
                        username: b.username.clone(),
                        password,
                    })
                    .ok_or_else(|| {
                        FetchError::Config(format!(
                            "Passwort für Benutzer {:?} ({}) nicht verfügbar: Schlüsselbund-Eintrag {:?} fehlt (nicht verbunden)",
                            b.username,
                            redact::url(&b.prefix),
                            b.secret_ref
                        ))
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let ua = user_agents::resolve(&cfg.user_agent);
        if let Some(w) = &ua.warning {
            tracing::warn!("{w}");
        }
        if let Some(rate) = cfg.speed.max_urls_per_sec {
            if !rate.is_finite() || rate <= 0.0 {
                return Err(FetchError::Config(format!(
                    "speed.max_urls_per_sec muss größer als 0 sein (ist {rate})"
                )));
            }
        }
        Ok(FetchConfig {
            http: cfg.http.clone(),
            network: os_model::settings::NetworkSettings::default(),
            proxy_password: None,
            user_agent: ua.http,
            response_timeout: Duration::from_secs(u64::from(cfg.advanced.response_timeout_s)),
            cookies: cfg.advanced.cookies,
            basic_auth,
            resolve: cfg.network.resolve.clone(),
            max_urls_per_sec: cfg.speed.max_urls_per_sec,
            max_body_bytes: cfg.limits.max_page_kb.saturating_mul(1024),
            extra_root_certs: Vec::new(),
            clock,
        })
    }
}

/// HTTP-Abrufer.
pub struct HttpFetcher {
    cfg: FetchConfig,
    hsts: hsts::Policies,
    connect_timeout: Duration,
    client: reqwest::Client,
    auth: AuthMatcher,
    digest: crate::auth::digest::DigestAuth,
    challenge_digest: AuthMatcher,
    challenge_ntlm: AuthMatcher,
    auth_ids: Arc<dyn os_model::IdSource>,
    limiter: Option<RateLimiter>,
    /// Gemeinsamer Speicher für `CookieMode::Persistent`, wenn die Anfrage keinen mitbringt.
    global_jar: Arc<CookieJar>,
}

impl std::fmt::Debug for HttpFetcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpFetcher")
            .field("cfg", &self.cfg)
            .finish()
    }
}

impl HttpFetcher {
    /// Opt-in profile authentication: never emits preemptive Basic credentials.
    /// Existing clients retain their original authentication behavior.
    pub fn with_challenge_auth(
        mut self,
        digest: &[BasicAuthEntry],
        ntlm: &[BasicAuthEntry],
    ) -> Result<Self, FetchError> {
        self.challenge_digest = AuthMatcher::new(digest).map_err(FetchError::Config)?;
        self.challenge_ntlm = AuthMatcher::new(ntlm).map_err(FetchError::Config)?;
        Ok(self)
    }

    /// Injects the authentication nonce source; deterministic tests use SeededIds.
    pub fn with_auth_ids(mut self, ids: Arc<dyn os_model::IdSource>) -> Self {
        self.digest.set_ids(ids.clone(), self.cfg.clock.clone());
        self.auth_ids = ids;
        self
    }

    /// Network snapshot for resume; crawl-specific request options are rebuilt separately.
    /// The password must remain in memory and must never be logged or persisted.
    pub fn network_snapshot(&self) -> (os_model::settings::NetworkSettings, Option<String>) {
        (self.cfg.network.clone(), self.cfg.proxy_password.clone())
    }

    /// Baut den Abrufer (HTTP-Client, Auflösung, Auth-Zuordnung, Rate-Limit).
    pub fn new(cfg: FetchConfig) -> Result<Self, FetchError> {
        if cfg.response_timeout.is_zero() {
            return Err(FetchError::Config(
                "advanced.response_timeout_s muss größer als 0 sein".to_string(),
            ));
        }
        let user_agent = HeaderValue::from_str(&cfg.user_agent).map_err(|_| {
            FetchError::Config("User-Agent enthält unzulässige Zeichen".to_string())
        })?;
        let connect_timeout = cfg.response_timeout.min(MAX_CONNECT_TIMEOUT);
        let resolver =
            HostResolver::new(&cfg.resolve, connect_timeout).map_err(FetchError::Config)?;
        let auth = AuthMatcher::new(&cfg.basic_auth).map_err(FetchError::Config)?;
        let limiter = cfg
            .max_urls_per_sec
            .map(|rate| RateLimiter::new(rate, cfg.clock.clone()))
            .transpose()
            .map_err(FetchError::Config)?;

        validate_headers(&cfg.http)?;
        crate::net::certificates::roots(&cfg.network).map_err(FetchError::Config)?;
        let mut builder =
            crate::net::client_builder_with_password(&cfg.network, cfg.proxy_password.as_deref())
                .tls_danger_accept_invalid_certs(cfg.http.ignore_certificate_errors)
                .retry(reqwest::retry::never())
                .user_agent(user_agent)
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(connect_timeout)
                .tcp_nodelay(true)
                .dns_resolver(Arc::new(resolver));
        if cfg.http.version == os_model::HttpVersion::Http1 {
            builder = builder.http1_only();
        }
        let mut certs = Vec::new();
        for pem in &cfg.extra_root_certs {
            certs.extend(
                reqwest::Certificate::from_pem_bundle(pem)
                    .map_err(|e| FetchError::Config(format!("ungültiges Zertifikat: {e}")))?,
            );
        }
        if !certs.is_empty() {
            builder = builder.tls_certs_merge(certs);
        }
        let client = builder
            .build()
            .map_err(|e| FetchError::Config(redact::text(&e.to_string())))?;
        Ok(HttpFetcher {
            hsts: hsts::Policies::default(),
            cfg,
            connect_timeout,
            client,
            auth,
            digest: crate::auth::digest::DigestAuth::default(),
            challenge_digest: AuthMatcher::default(),
            challenge_ntlm: AuthMatcher::default(),
            auth_ids: Arc::new(os_model::RandomIds),
            limiter,
            global_jar: Arc::new(CookieJar::new()),
        })
    }

    /// Fetch using addresses already resolved and approved by the caller (AGENT-020).
    /// The client is pinned to that exact address set: no second DNS lookup, redirects or proxy.
    /// Existing fetch paths are unchanged. Rate limiting and persistent cookies remain shared.
    pub async fn fetch_pinned(
        &self,
        req: FetchRequest,
        addresses: &[std::net::SocketAddr],
    ) -> Result<FetchResult, FetchError> {
        let host = req
            .url
            .host_str()
            .ok_or_else(|| FetchError::Config("missing host".into()))?;
        if addresses.is_empty() {
            return Err(FetchError::Config("empty resolved address set".into()));
        }
        let mut builder = reqwest::Client::builder()
            .user_agent(&self.cfg.user_agent)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(self.connect_timeout)
            .tcp_nodelay(true)
            .resolve_to_addrs(host, addresses);
        let mut certs = Vec::new();
        for pem in &self.cfg.extra_root_certs {
            certs.extend(
                reqwest::Certificate::from_pem_bundle(pem)
                    .map_err(|e| FetchError::Config(format!("invalid certificate: {e}")))?,
            );
        }
        if !certs.is_empty() {
            builder = builder.tls_certs_merge(certs);
        }
        let pinned = Self {
            cfg: self.cfg.clone(),
            connect_timeout: self.connect_timeout,
            client: builder
                .build()
                .map_err(|e| FetchError::Config(redact::text(&e.to_string())))?,
            auth: AuthMatcher::new(&self.cfg.basic_auth).map_err(FetchError::Config)?,
            digest: crate::auth::digest::DigestAuth::default(),
            challenge_digest: AuthMatcher::default(),
            challenge_ntlm: AuthMatcher::default(),
            auth_ids: Arc::new(os_model::RandomIds),
            hsts: hsts::Policies::default(),
            limiter: None,
            global_jar: self.global_jar.clone(),
        };
        if let Some(limiter) = &self.limiter {
            limiter.acquire().await;
        }
        Ok(pinned.fetch_impl(req).await)
    }

    /// Create a reusable client pinned to an approved address set. Shares persistent cookies.
    /// Call `fetch_with_pinned` on the parent to retain its shared rate limit.
    pub fn pinned_client(
        &self,
        host: &str,
        addresses: &[std::net::SocketAddr],
    ) -> Result<Self, FetchError> {
        if addresses.is_empty() {
            return Err(FetchError::Config("empty resolved address set".into()));
        }
        let mut builder = reqwest::Client::builder()
            .user_agent(&self.cfg.user_agent)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(self.connect_timeout)
            .tcp_nodelay(true)
            .resolve_to_addrs(host, addresses);
        let mut certs = Vec::new();
        for pem in &self.cfg.extra_root_certs {
            certs.extend(
                reqwest::Certificate::from_pem_bundle(pem)
                    .map_err(|e| FetchError::Config(format!("invalid certificate: {e}")))?,
            );
        }
        if !certs.is_empty() {
            builder = builder.tls_certs_merge(certs);
        }
        Ok(Self {
            cfg: self.cfg.clone(),
            connect_timeout: self.connect_timeout,
            client: builder
                .build()
                .map_err(|e| FetchError::Config(redact::text(&e.to_string())))?,
            auth: AuthMatcher::new(&self.cfg.basic_auth).map_err(FetchError::Config)?,
            digest: crate::auth::digest::DigestAuth::default(),
            challenge_digest: AuthMatcher::default(),
            challenge_ntlm: AuthMatcher::default(),
            auth_ids: Arc::new(os_model::RandomIds),
            hsts: hsts::Policies::default(),
            limiter: None,
            global_jar: self.global_jar.clone(),
        })
    }

    /// Fetch with a cached pinned client, retaining the parent's rate limiter.
    pub async fn fetch_with_pinned(&self, req: FetchRequest, pinned: &Self) -> FetchResult {
        if let Some(limiter) = &self.limiter {
            limiter.acquire().await;
        }
        pinned.fetch_impl(req).await
    }

    /// Die wirksame Konfiguration.
    pub fn config(&self) -> &FetchConfig {
        &self.cfg
    }

    /// Zeitgrenze für den Verbindungsaufbau: `min(10 s, Antwort-Timeout)` (CRAWL-095).
    pub fn connect_timeout(&self) -> Duration {
        self.connect_timeout
    }

    /// Cookie-Speicher für einen neuen Seed: `None` bei `CookieMode::None`, ein frischer Speicher bei
    /// `SessionOnly` (die Engine reicht ihn an Weiterleitungsziele weiter), der gemeinsame bei `Persistent`.
    pub fn cookie_jar_for_new_seed(&self) -> Option<Arc<CookieJar>> {
        match self.cfg.cookies {
            CookieMode::None => None,
            CookieMode::SessionOnly => Some(Arc::new(CookieJar::new())),
            CookieMode::Persistent => Some(self.global_jar.clone()),
        }
    }

    fn jar_for(&self, req: &FetchRequest) -> Option<Arc<CookieJar>> {
        match self.cfg.cookies {
            CookieMode::None => None,
            CookieMode::SessionOnly => req.cookies.clone(),
            CookieMode::Persistent => Some(
                req.cookies
                    .clone()
                    .unwrap_or_else(|| self.global_jar.clone()),
            ),
        }
    }

    async fn fetch_impl(&self, req: FetchRequest) -> FetchResult {
        let clock = &self.cfg.clock;
        let mut url = req.url.clone();
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return FetchResult {
                started_at: clock.now(),
                outcome: FetchOutcome::NoResponse {
                    reason: NoResponseReason::Malformed,
                    detail: format!("nicht abrufbare Adresse: {}", redact::url(url.as_str())),
                    elapsed_ms: 0,
                },
                url: req.url,
            };
        }
        // Zugangsdaten stammen ausschließlich aus der Konfiguration, nie aus der Adresse.
        let _ = url.set_username("");
        let _ = url.set_password(None);

        if self.cfg.http.respect_hsts && !self.hsts.is_empty() {
            if let Some(target) = self.hsts.upgrade(&url, clock.monotonic()) {
                return FetchResult {
                    url: req.url,
                    started_at: clock.now(),
                    outcome: FetchOutcome::Response(HttpResponse {
                        status: 307,
                        status_text: "HSTS Policy".into(),
                        headers: vec![("location".into(), target.to_string())],
                        synthetic: Some(crate::SyntheticResponse::Hsts),
                        attempts: req.attempt.saturating_add(1),
                        request_headers: vec![],
                        set_cookies: vec![],
                        content_length: None,
                        untruncated_bytes: Some(0),
                        reason_phrase: None,
                        http_version: String::new(),
                        body: bytes::Bytes::new(),
                        size_bytes: 0,
                        transferred_bytes: 0,
                        response_time_ms: 0,
                        truncated: false,
                        decode_error: None,
                        md5_hex: String::new(),
                    }),
                };
            }
        }
        if let Some(limiter) = &self.limiter {
            limiter.acquire().await;
        }
        let started_at = clock.now();
        let t0 = clock.monotonic();
        let max_body = if req.body_limit.max_decoded_bytes > 0 {
            req.body_limit.max_decoded_bytes
        } else if req.max_body_bytes > 0 {
            req.max_body_bytes
        } else if self.cfg.max_body_bytes > 0 {
            self.cfg.max_body_bytes
        } else {
            // CRAWL-075 (K2-68): a page size limit of 0 means unlimited, never "truncate at zero bytes".
            u64::MAX
        };
        let jar = self.jar_for(&req);
        tracing::debug!(url = %redact::url(url.as_str()), purpose = ?req.purpose, "Abruf beginnt");

        let budget = self.cfg.response_timeout + OVERALL_SLACK;
        let exchange = self.exchange(
            &url,
            req.purpose,
            jar.as_deref(),
            max_body,
            req.body_limit.abort_media_after,
        );
        let result = match tokio::time::timeout(budget, exchange).await {
            Ok(r) => r,
            Err(_) => Err(Failure {
                proxy_auth_required: false,
                reason: NoResponseReason::ResponseTimeout,
                detail: format!(
                    "keine vollständige Antwort innerhalb von {} s",
                    self.cfg.response_timeout.as_secs_f64()
                ),
            }),
        };
        let outcome = match result {
            Ok(ex) => {
                let response_time_ms = millis(ex.finished.saturating_sub(t0));
                let mut response = finish_response(ex, response_time_ms);
                response.attempts = req.attempt.saturating_add(1);
                if self.cfg.http.respect_hsts && !self.cfg.http.ignore_certificate_errors {
                    if let Some(value) = response.header("strict-transport-security") {
                        self.hsts.learn(&url, value, clock.monotonic());
                    }
                }
                FetchOutcome::Response(response)
            }
            Err(failure) if failure.proxy_auth_required => FetchOutcome::Response(HttpResponse {
                status: 407,
                status_text: "Proxy-Authentifizierung fehlgeschlagen".into(),
                request_headers: vec![],
                set_cookies: vec![],
                content_length: None,
                untruncated_bytes: Some(0),
                synthetic: None,
                attempts: req.attempt.saturating_add(1),
                reason_phrase: None,
                http_version: String::new(),
                headers: vec![],
                body: bytes::Bytes::new(),
                size_bytes: 0,
                transferred_bytes: 0,
                response_time_ms: millis(clock.monotonic().saturating_sub(t0)),
                truncated: false,
                decode_error: None,
                md5_hex: String::new(),
            }),
            Err(failure) => {
                let elapsed_ms = millis(clock.monotonic().saturating_sub(t0));
                tracing::debug!(
                    url = %redact::url(url.as_str()),
                    reason = ?failure.reason,
                    detail = %failure.detail,
                    "Abruf ohne Antwort"
                );
                FetchOutcome::NoResponse {
                    reason: failure.reason,
                    detail: failure.detail,
                    elapsed_ms,
                }
            }
        };
        // OUT-241 (K2-61): additional completion event, with the URL redacted before logging.
        if let FetchOutcome::Response(response) = &outcome {
            tracing::debug!(url = %redact::url(url.as_str()), status = response.status, response_time_ms = response.response_time_ms, "Abruf abgeschlossen");
        }
        if let FetchOutcome::NoResponse { elapsed_ms, .. } = &outcome {
            tracing::debug!(url = %redact::url(url.as_str()), status = "No Response", response_time_ms = elapsed_ms, "Abruf abgeschlossen");
        }
        FetchResult {
            url: req.url,
            started_at,
            outcome,
        }
    }

    /// Anfrage senden und den Body unverändert (komprimiert) einlesen.
    async fn exchange(
        &self,
        url: &url::Url,
        purpose: FetchPurpose,
        jar: Option<&CookieJar>,
        max_body: u64,
        media_limit: Option<u64>,
    ) -> Result<Exchange, Failure> {
        let accept = match purpose {
            FetchPurpose::Page | FetchPurpose::External => ACCEPT_PAGE,
            FetchPurpose::Resource | FetchPurpose::RobotsTxt | FetchPurpose::Sitemap => ACCEPT_ANY,
        };
        let mut headers = HeaderMap::new();
        headers.insert(header::ACCEPT, HeaderValue::from_static(accept));
        headers.insert(
            header::ACCEPT_ENCODING,
            HeaderValue::from_static(ACCEPT_ENCODING),
        );
        headers.insert(header::CONNECTION, HeaderValue::from_static("keep-alive"));
        if let Some(value) = self.auth.authorization_for(url) {
            if let Ok(mut v) = HeaderValue::from_str(value) {
                v.set_sensitive(true);
                headers.insert(header::AUTHORIZATION, v);
            }
        }
        if let Some(value) = jar.and_then(|j| j.header_for(url)) {
            if let Ok(mut v) = HeaderValue::from_str(&value) {
                v.set_sensitive(true);
                headers.insert(header::COOKIE, v);
            }
        }
        self.custom_headers(url, purpose, &mut headers);
        for (name, value) in &headers {
            tracing::trace!(
                header = %name,
                value = %redact::header_value(name.as_str(), &String::from_utf8_lossy(value.as_bytes())),
                "Anfrage-Header"
            );
        }

        let credentials = self
            .challenge_digest
            .rendering_credentials(url)
            .or_else(|| self.auth.rendering_credentials(url));
        let credentials = if self.challenge_ntlm.rendering_credentials(url).is_some() {
            None
        } else {
            credentials.as_ref()
        };
        let digest_key =
            credentials.map(|c| format!("{}|{}", url.origin().ascii_serialization(), c.prefix));
        if let (Some(c), Some(key)) = (credentials, &digest_key) {
            if let Some(value) = self
                .digest
                .authorization(key, url, &c.username, &c.password)
            {
                headers.insert(
                    header::AUTHORIZATION,
                    HeaderValue::from_str(&value).map_err(|_| Failure {
                        proxy_auth_required: false,
                        reason: NoResponseReason::Malformed,
                        detail: "Ungültige Digest-Zugangsdaten".into(),
                    })?,
                );
            }
        }
        // NTLM is connection-bound: each exchange owns a fresh HTTP/1 client/pool.
        // Parallel crawl requests cannot mix identities or handshake messages.
        let ntlm = self.challenge_ntlm.rendering_credentials(url);
        let dedicated = if ntlm.is_some() {
            let mut cfg = self.cfg.clone();
            cfg.http.version = os_model::HttpVersion::Http1;
            Some(Self::new(cfg).map_err(|_| Failure {
                proxy_auth_required: false,
                reason: NoResponseReason::Malformed,
                detail: "NTLM-Client konnte nicht erstellt werden".into(),
            })?)
        } else {
            None
        };
        let client = dedicated.as_ref().map_or(&self.client, |f| &f.client);
        if ntlm.is_some() {
            headers.remove(header::AUTHORIZATION);
        }
        let mut response = client
            .get(url.clone())
            .headers(headers.clone())
            .send()
            .await
            .map_err(|e| classify(&e))?;
        if let Some(c) = ntlm {
            if response.status().as_u16() == 401
                && response
                    .headers()
                    .get_all(header::WWW_AUTHENTICATE)
                    .iter()
                    .filter_map(|v| v.to_str().ok())
                    .any(|v| v.eq_ignore_ascii_case("NTLM") || v.starts_with("NTLM "))
            {
                // Drain each challenge body to return this exact connection to its private pool.
                crate::auth::ntlm::drain(response)
                    .await
                    .map_err(|e| classify(&e))?;
                let negotiate = crate::auth::ntlm::negotiate();
                let mut value = HeaderValue::from_str(&format!(
                    "NTLM {}",
                    crate::auth::base64_encode(&negotiate)
                ))
                .unwrap();
                value.set_sensitive(true);
                headers.insert(header::AUTHORIZATION, value);
                response = client
                    .get(url.clone())
                    .headers(headers.clone())
                    .send()
                    .await
                    .map_err(|e| classify(&e))?;
                if response.status().as_u16() == 401 {
                    let challenge = response
                        .headers()
                        .get_all(header::WWW_AUTHENTICATE)
                        .iter()
                        .filter_map(|v| v.to_str().ok())
                        .find_map(|v| v.strip_prefix("NTLM ").and_then(crate::auth::ntlm::decode));
                    if let Some(challenge) = challenge {
                        let now = self.cfg.clock.now();
                        let id = self.auth_ids.new_crawl_id(now);
                        let nonce = id.0.as_bytes()[8..16].try_into().unwrap();
                        let time = (i128::from(now.unix_millis()) + 11_644_473_600_000) * 10_000;
                        if let Some(auth) = crate::auth::ntlm::authenticate(
                            &challenge,
                            &c.username,
                            &c.password,
                            nonce,
                            time as u64,
                        ) {
                            crate::auth::ntlm::drain(response)
                                .await
                                .map_err(|e| classify(&e))?;
                            let mut value = HeaderValue::from_str(&format!(
                                "NTLM {}",
                                crate::auth::base64_encode(&auth)
                            ))
                            .unwrap();
                            value.set_sensitive(true);
                            headers.insert(header::AUTHORIZATION, value);
                            response = client
                                .get(url.clone())
                                .headers(headers.clone())
                                .send()
                                .await
                                .map_err(|e| classify(&e))?;
                        }
                    }
                }
            }
        }
        // Höchstens eine Challenge und eine stale-Nonce-Erneuerung je Abruf.
        for attempt in 0..2 {
            if response.status().as_u16() != 401 {
                break;
            }
            let (Some(c), Some(key)) = (credentials, &digest_key) else {
                break;
            };
            let challenge = response
                .headers()
                .get_all(header::WWW_AUTHENTICATE)
                .iter()
                .filter_map(|v| v.to_str().ok())
                .find(|v| {
                    v.get(..7)
                        .is_some_and(|v| v.eq_ignore_ascii_case("digest "))
                });
            let Some(challenge) = challenge else {
                break;
            };
            if attempt > 0 && !crate::auth::digest::is_stale(challenge) {
                break;
            }
            if !self.digest.challenge(key, challenge) {
                break;
            }
            let Some(value) = self
                .digest
                .authorization(key, url, &c.username, &c.password)
            else {
                break;
            };
            let mut value = HeaderValue::from_str(&value).map_err(|_| Failure {
                proxy_auth_required: false,
                reason: NoResponseReason::Malformed,
                detail: "Ungültige Digest-Challenge".into(),
            })?;
            value.set_sensitive(true);
            headers.insert(header::AUTHORIZATION, value);
            response = self
                .client
                .get(url.clone())
                .headers(headers.clone())
                .send()
                .await
                .map_err(|e| classify(&e))?;
        }
        let request_headers = headers
            .iter()
            .map(|(n, v)| {
                (
                    n.to_string(),
                    redact::header_value(n.as_str(), &String::from_utf8_lossy(v.as_bytes())),
                )
            })
            .collect();
        let status = response.status().as_u16();
        let version = version_label(response.version());
        // hyper legt die Reason-Phrase nur ab, wenn sie vom IANA-Text abweicht; HTTP/2 hat keine, und Phrasen
        // mit Nicht-ASCII-Bytes meldet httparse als leer (dann gilt der IANA-Text).
        let reason_phrase = response
            .extensions()
            .get::<hyper::ext::ReasonPhrase>()
            .and_then(|r| std::str::from_utf8(r.as_bytes()).ok())
            .map(str::trim)
            .filter(|r| !r.is_empty())
            .map(str::to_string);
        let raw_headers: Vec<(String, String)> = response
            .headers()
            .iter()
            .map(|(n, v)| {
                (
                    n.as_str().to_string(),
                    String::from_utf8_lossy(v.as_bytes()).into_owned(),
                )
            })
            .collect();
        let encodings = parse_encodings(
            raw_headers
                .iter()
                .filter(|(n, _)| n == "content-encoding")
                .map(|(_, v)| v.as_str()),
        );

        let media = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("audio/") || v.starts_with("video/"));
        let max_body = if media {
            max_body.min(media_limit.unwrap_or(64 * 1024))
        } else {
            max_body
        };
        let (decoded, wire_body_bytes) = decode::stream(&mut response, &encodings, max_body)
            .await
            .map_err(|e| classify(&e))?;
        let raw_body = decoded.body;
        let truncated = decoded.truncated;
        let decode_error = decoded.error;
        let finished = self.cfg.clock.monotonic();

        // Cookies erst nach vollständig empfangener Antwort übernehmen.
        if let Some(jar) = jar {
            let set_cookies: Vec<String> = response
                .headers()
                .get_all(header::SET_COOKIE)
                .iter()
                .map(|v| String::from_utf8_lossy(v.as_bytes()).into_owned())
                .collect();
            if !set_cookies.is_empty() {
                jar.store_set_cookie(url, set_cookies.iter().map(String::as_str));
            }
        }

        Ok(Exchange {
            request_headers,
            status,
            reason_phrase,
            version,
            headers: raw_headers,
            decode_error,
            raw_body,
            wire_body_bytes,
            truncated,
            finished,
        })
    }
}

/// Rohergebnis eines Abrufs vor der Dekompression.
struct Exchange {
    request_headers: Vec<(String, String)>,
    status: u16,
    /// Vom Server gesendete Reason-Phrase (nur wenn sie vom IANA-Text abweicht).
    reason_phrase: Option<String>,
    version: String,
    headers: Vec<(String, String)>,
    decode_error: Option<String>,
    raw_body: Vec<u8>,
    wire_body_bytes: u64,
    truncated: bool,
    /// Uhr-Zeit nach dem letzten empfangenen Body-Byte.
    finished: Duration,
}

/// Dekomprimiert, zählt Bytes, bildet den MD5 und redigiert Header.
fn finish_response(ex: Exchange, response_time_ms: u32) -> HttpResponse {
    let Exchange {
        request_headers,
        status,
        reason_phrase,
        version,
        headers,
        decode_error,
        raw_body,
        wire_body_bytes,
        truncated: input_truncated,
        ..
    } = ex;

    // Gesendete Reason-Phrase, sonst IANA-Text, für unbekannte Codes leer (CRAWL-012).
    let wire_text = status_text(status, reason_phrase.as_deref());
    // K-02: `Transferred` sind die Body-Bytes auf der Leitung (komprimiert, ohne Statuszeile,
    // Header, Chunk-Rahmen und 1xx-Zwischenantworten); ohne Body (HEAD, 204, 304) 0.
    let transferred_bytes = wire_body_bytes;

    let decoded = decode::Decoded {
        body: raw_body,
        truncated: input_truncated,
        error: decode_error,
    };
    let body = bytes::Bytes::from(decoded.body);
    let md5_hex = hex::encode(Md5::digest(&body));
    let decode_error = decoded.error.map(|e| redact::text(&e));
    let shown_text = if decode_error.is_some() {
        NoResponseReason::ConnectionError.status_text().to_string()
    } else {
        wire_text
    };
    let set_cookies = headers
        .iter()
        .filter(|(n, _)| n.eq_ignore_ascii_case("set-cookie"))
        .map(|(_, v)| v.clone())
        .collect();
    let content_length = headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.parse().ok());
    // Content-Length describes encoded bytes. A cut decoded stream has no known
    // total decoded length (review decision for CRAWL-075/140).
    let compressed = !parse_encodings(
        headers
            .iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case("content-encoding"))
            .map(|(_, v)| v.as_str()),
    )
    .is_empty();
    let untruncated_bytes = if decoded.truncated {
        if compressed {
            None
        } else {
            content_length.or(Some(body.len() as u64))
        }
    } else {
        Some(body.len() as u64)
    };
    let headers = headers
        .into_iter()
        .map(|(n, v)| {
            let v = redact::header_value(&n, &v);
            (n, v)
        })
        .collect();
    HttpResponse {
        request_headers,
        set_cookies,
        content_length,
        untruncated_bytes,
        synthetic: None,
        attempts: 1,
        status,
        reason_phrase,
        status_text: shown_text,
        http_version: version,
        headers,
        size_bytes: untruncated_bytes.unwrap_or(body.len() as u64),
        body,
        transferred_bytes,
        response_time_ms,
        truncated: decoded.truncated || input_truncated,
        decode_error,
        md5_hex,
    }
}

fn millis(d: Duration) -> u32 {
    u32::try_from(d.as_millis()).unwrap_or(u32::MAX)
}

fn validate_headers(http: &os_model::HttpConfig) -> Result<(), FetchError> {
    for h in &http.headers {
        if h.name.eq_ignore_ascii_case("user-agent") {
            return Err(FetchError::Config(
                "User-Agent über CRAWL-114 konfigurieren".into(),
            ));
        }
        reqwest::header::HeaderName::from_bytes(h.name.as_bytes())
            .map_err(|_| FetchError::Config("Ungültiger Header-Name".into()))?;
        HeaderValue::from_str(&h.value)
            .map_err(|_| FetchError::Config("Ungültiger Header-Wert".into()))?;
    }
    if let Some(value) = &http.accept_language {
        HeaderValue::from_str(value)
            .map_err(|_| FetchError::Config("Ungültiges Accept-Language".into()))?;
    }
    Ok(())
}
impl HttpFetcher {
    fn custom_headers(&self, url: &url::Url, purpose: FetchPurpose, headers: &mut HeaderMap) {
        if purpose == FetchPurpose::External {
            return;
        }
        let host = url.host_str().unwrap_or("");
        if let Some(value) = &self.cfg.http.accept_language {
            headers.insert(
                header::ACCEPT_LANGUAGE,
                HeaderValue::from_str(value).unwrap(),
            );
        }
        for h in &self.cfg.http.headers {
            if h.host
                .as_ref()
                .is_some_and(|h| !h.eq_ignore_ascii_case(host))
            {
                continue;
            }
            let name = reqwest::header::HeaderName::from_bytes(h.name.as_bytes()).unwrap();
            let value = if name == header::COOKIE {
                headers
                    .get(header::COOKIE)
                    .and_then(|v| v.to_str().ok())
                    .map_or_else(|| h.value.clone(), |v| format!("{v}; {}", h.value))
            } else {
                h.value.clone()
            };
            headers.insert(name, HeaderValue::from_str(&value).unwrap());
        }
    }
}
impl Fetch for HttpFetcher {
    fn render_cookie_jar(&self, session: Option<Arc<CookieJar>>) -> Option<Arc<CookieJar>> {
        match self.cfg.cookies {
            CookieMode::None => None,
            CookieMode::SessionOnly => session,
            CookieMode::Persistent => Some(session.unwrap_or_else(|| self.global_jar.clone())),
        }
    }

    fn render_config(&self) -> Option<FetchConfig> {
        Some(self.cfg.clone())
    }

    fn fetch(&self, req: FetchRequest) -> Pin<Box<dyn Future<Output = FetchResult> + Send + '_>> {
        Box::pin(self.fetch_impl(req))
    }
}

impl FetchConfig {
    /// Ergänzt den App-Snapshot; spätere Einstellungsänderungen verändern diesen Crawl nicht.
    pub fn with_network(mut self, settings: &os_model::settings::NetworkSettings) -> Self {
        self.network = settings.clone();
        self
    }
}
