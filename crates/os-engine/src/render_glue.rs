//! Browser integration, bounded concurrency and comparison facts (M2-E05).
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, OnceLock, Weak};

use futures_util::future::BoxFuture;
use os_fetch::Fetch;
use os_model::{CrawlConfig, ExtKey, ExtractedLink, LinkOrigin, LinkType, ParsedPage, Value};
pub use os_render::{RenderError, RenderOpts, RenderedPage};
use url::Url;

/// Linux exposes threads as processes in sysinfo by default. Summing their
/// shared address spaces would charge Chromium's RSS once per thread.
pub(crate) fn process_memory_sample() -> crate::memory::Sample {
    use crate::memory::Sample;
    use std::time::{Duration, Instant};
    static SYSTEM: OnceLock<std::sync::Mutex<(sysinfo::System, Option<Instant>, Sample)>> =
        OnceLock::new();
    let mut state = SYSTEM
        .get_or_init(|| std::sync::Mutex::new((sysinfo::System::new(), None, Sample::default())))
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if state
        .1
        .is_some_and(|at| at.elapsed() < Duration::from_secs(1))
    {
        return state.2;
    }
    let sys = &mut state.0;
    sys.refresh_memory();
    sys.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::All,
        true,
        sysinfo::ProcessRefreshKind::nothing()
            .with_memory()
            .without_tasks(),
    );
    let pid = sysinfo::Pid::from_u32(std::process::id());
    let mut descendants = HashSet::from([pid]);
    loop {
        let before = descendants.len();
        for (id, process) in sys.processes() {
            if process.parent().is_some_and(|p| descendants.contains(&p)) {
                descendants.insert(*id);
            }
        }
        if before == descendants.len() {
            break;
        }
    }
    let sample = Sample {
        engine_bytes: sys.process(pid).map_or(0, |p| p.memory()),
        renderer_bytes: sys
            .processes()
            .iter()
            .filter(|(id, _)| **id != pid && descendants.contains(id))
            .map(|(_, p)| p.memory())
            .sum(),
        physical_bytes: sys.total_memory(),
        available_bytes: sys.available_memory(),
        injected_limit: None,
    };
    state.1 = Some(Instant::now());
    state.2 = sample;
    sample
}

pub trait Renderer: Send + Sync {
    fn render<'a>(
        &'a self,
        url: &'a str,
        opts: &'a RenderOpts,
    ) -> BoxFuture<'a, Result<RenderedPage, RenderError>>;
}
impl Renderer for os_render::Renderer {
    fn render<'a>(
        &'a self,
        url: &'a str,
        opts: &'a RenderOpts,
    ) -> BoxFuture<'a, Result<RenderedPage, RenderError>> {
        Box::pin(os_render::Renderer::render(self, url, opts))
    }
}

/// K2-36: explicit tabs still obey the process and memory budgets.
pub fn tab_limit(cfg: &CrawlConfig, process_limit: u32, free_budget: u64) -> usize {
    cfg.rendering
        .tabs
        .unwrap_or(cfg.speed.threads)
        .min(process_limit)
        .min((free_budget / (300 * 1024 * 1024)).min(u64::from(u32::MAX)) as u32)
        .max(1) as usize
}

pub(crate) struct Session {
    pub injected: OnceLock<Arc<dyn Renderer>>,
    browser: tokio::sync::OnceCell<os_render::Renderer>,
    tabs: tokio::sync::Semaphore,
    launch: os_render::LaunchOpts,
}
impl Session {
    pub fn new(cfg: &CrawlConfig, fetch: Option<&os_fetch::FetchConfig>) -> Self {
        // Text crawls allocate no browser resources and must not prime the
        // process-wide memory sample with unrelated browser startup activity.
        let tabs = if cfg.rendering.mode == os_model::RenderingMode::JavaScript {
            let (limit, _, processes) = crate::memory::resource_limits();
            let sample = crate::memory::sample();
            let budget = limit
                .saturating_sub(sample.used())
                .min(sample.available_bytes);
            tab_limit(cfg, processes, budget)
        } else {
            1
        };
        let mut launch = os_render::LaunchOpts {
            chromium_path: cfg.rendering.chromium_path.as_ref().map(Into::into),
            max_tabs: tabs,
            suppress_default_accept_language: true,
            disable_cookie_storage: cfg.advanced.cookies == os_model::CookieMode::None,
            restart_after_pages: cfg.rendering.restart_after_pages.into(),
            memory_limit_mb: cfg.rendering.restart_memory_mb.into(),
            host_resolver_rules: cfg
                .network
                .resolve
                .iter()
                .filter_map(|rule| {
                    let (host, address) = rule.split_once('=')?;
                    Some(format!("MAP {host} {address}"))
                })
                .collect(),
            ..Default::default()
        };
        if let Some(fetch) = fetch {
            launch.network = fetch.network.clone();
            if let (Some(username), Some(password)) =
                (&fetch.network.proxy.username, &fetch.proxy_password)
            {
                launch.proxy_credentials = Some(os_render::api::Credentials {
                    username: username.clone(),
                    password: password.clone(),
                });
            }
        }
        Self {
            injected: OnceLock::new(),
            browser: tokio::sync::OnceCell::new(),
            tabs: tokio::sync::Semaphore::new(tabs),
            launch,
        }
    }
    pub async fn render(&self, url: &str, opts: &RenderOpts) -> Result<RenderedPage, RenderError> {
        let _permit = self
            .tabs
            .acquire()
            .await
            .map_err(|e| RenderError::Protocol(e.to_string()))?;
        if let Some(renderer) = self.injected.get() {
            // Test/custom renderers have the same hard deadline as Chromium.
            for attempt in 0..2 {
                let result = tokio::time::timeout(
                    std::time::Duration::from_millis(
                        opts.snippets
                            .page_budget_ms(u64::from(opts.page_timeout_ms)),
                    ),
                    renderer.render(url, opts),
                )
                .await
                .map_err(|_| RenderError::Timeout(url.into()))?;
                if attempt == 0 && matches!(result, Err(RenderError::Crashed(_))) {
                    continue;
                }
                return result;
            }
            unreachable!("bounded retry")
        }
        self.browser
            .get_or_try_init(|| os_render::Renderer::launch(self.launch.clone()))
            .await?
            .render(url, opts)
            .await
    }

    pub async fn close(&self) {
        if let Some(browser) = self.browser.get() {
            browser.close().await;
        }
    }
}

struct Policy<F: Fetch>(Weak<crate::worker::Shared<F>>);
impl<F: Fetch> std::fmt::Debug for Policy<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CrawlResourcePolicy")
    }
}
impl<F: Fetch> os_render::api::ResourcePolicy for Policy<F> {
    fn allowed_resource<'a>(&'a self, url: &'a str, resource_type: &'a str) -> BoxFuture<'a, bool> {
        Box::pin(async move {
            let (Some(sh), Ok(target)) = (self.0.upgrade(), Url::parse(url)) else {
                return false;
            };
            let scope = sh.scope.classify(&os_url::normalize(&target));
            if scope == os_url::ScopeClass::External && !sh.cfg.spider.external.crawl {
                return false;
            }
            let kind = match resource_type {
                "Image" => Some(LinkType::Image),
                "Stylesheet" | "Font" | "Other" => Some(LinkType::Css),
                "Script" => Some(LinkType::JavaScript),
                "Media" => Some(LinkType::Media),
                "Document" => Some(LinkType::Iframe),
                "Navigation" => Some(LinkType::Hyperlink),
                _ => None,
            };
            if let Some(kind) = kind {
                let link = ExtractedLink {
                    link_type: kind,
                    resolved: Some(url.into()),
                    crawlable: true,
                    ..Default::default()
                };
                if !crate::decide::decide(&link, scope, &sh.cfg, true).crawl {
                    return false;
                }
            }
            self.allowed(url).await
        })
    }

    fn allowed<'a>(&'a self, url: &'a str) -> BoxFuture<'a, bool> {
        Box::pin(async move {
            let (Some(sh), Ok(url)) = (self.0.upgrade(), Url::parse(url)) else {
                return false;
            };
            if !sh.cfg.extraction.cookies && analytics_host(url.host_str().unwrap_or_default()) {
                return false;
            }
            if sh.fetcher.target_denial(&url).await.is_some() {
                return false;
            }
            let check = sh.gate.check(&*sh.fetcher, &url).await;
            if let Some((result, write)) = check.robots_fetch {
                if crate::robots_glue::record_robots(&sh, result, write)
                    .await
                    .is_err()
                {
                    return false;
                }
            }
            !matches!(
                check.decision,
                os_fetch::robots::RobotsDecision::Blocked { .. }
            )
        })
    }
}
pub(crate) fn analytics_host(host: &str) -> bool {
    [
        "google-analytics.com",
        "googletagmanager.com",
        "analytics.google.com",
    ]
    .iter()
    .any(|known| {
        host == *known
            || host
                .strip_suffix(known)
                .is_some_and(|prefix| prefix.ends_with('.'))
    })
}

struct Identity<F: Fetch> {
    sh: Weak<crate::worker::Shared<F>>,
    auth: os_fetch::auth::AuthMatcher,
    cookies: Option<Arc<os_fetch::CookieJar>>,
}
impl<F: Fetch> std::fmt::Debug for Identity<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CrawlRequestIdentity")
    }
}
impl<F: Fetch> os_render::api::RequestIdentity for Identity<F> {
    fn auth_response<'a>(
        &'a self,
        url: &'a str,
    ) -> BoxFuture<'a, Option<os_render::api::MainResponse>> {
        Box::pin(async move {
            let sh = self.sh.upgrade()?;
            let target = Url::parse(url).ok()?;
            self.auth.rendering_credentials(&target)?;
            let fetched = sh
                .fetcher
                .fetch(os_fetch::FetchRequest {
                    url: target,
                    purpose: os_fetch::FetchPurpose::Resource,
                    cookies: self.cookies.clone(),
                    max_body_bytes: crate::worker::page_body_limit(&sh.cfg),
                    attempt: 1,
                    body_limit: Default::default(),
                })
                .await;
            let os_fetch::FetchOutcome::Response(response) = fetched.outcome else {
                return None;
            };
            Some(browser_response(
                &response,
                sh.cfg.advanced.cookies != os_model::CookieMode::None,
            ))
        })
    }
    fn headers(&self, url: &str) -> BTreeMap<String, String> {
        let mut headers = BTreeMap::new();
        let (Some(sh), Ok(url)) = (self.sh.upgrade(), Url::parse(url)) else {
            return headers;
        };
        let internal = sh.scope.classify(&os_url::normalize(&url)) == os_url::ScopeClass::Internal;
        if internal {
            for h in &sh.cfg.http.headers {
                if h.host
                    .as_ref()
                    .is_none_or(|host| Some(host.as_str()) == url.host_str())
                {
                    headers.insert(h.name.clone(), h.value.clone());
                }
            }
        }
        if let Some(language) = &sh.cfg.http.accept_language {
            headers.insert("Accept-Language".into(), language.clone());
        }
        if let Some(auth) = self.auth.authorization_for(&url) {
            headers.insert("Authorization".into(), auth.into());
        }
        headers
    }
}

pub(crate) fn options<F: Fetch>(
    sh: &Arc<crate::worker::Shared<F>>,
    item: &crate::frontier::QueueItem,
    response: &os_fetch::HttpResponse,
) -> RenderOpts {
    let cfg = &sh.cfg;
    let r = &cfg.rendering;
    let snapshot = sh.fetcher.render_config();
    let auth = snapshot
        .as_ref()
        .and_then(|f| os_fetch::auth::AuthMatcher::new(&f.basic_auth).ok())
        .unwrap_or_default();
    let mut opts = RenderOpts {
        snippets: os_render::snippets::RunOptions::for_response(
            cfg,
            item.purpose != crate::frontier::Purpose::External,
            response.status,
            response
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
                .map_or("", |(_, v)| v.as_str()),
        ),
        retry_document_context: true,
        disable_cookies: cfg.advanced.cookies == os_model::CookieMode::None,
        return_cookies: cfg.advanced.cookies != os_model::CookieMode::None,
        accept_language: cfg.http.accept_language.clone(),
        capture_cookie_assignments: true,
        viewport: (r.viewport.width, r.viewport.height),
        device: match r.viewport.preset.as_str() {
            "googlebot_mobile" => os_render::api::Device::GooglebotMobile,
            "googlebot_desktop" => os_render::api::Device::GooglebotDesktop,
            _ => os_render::api::Device::Fixed,
        },
        device_scale_factor: r.viewport.scale_factor,
        mobile: r.viewport.mobile,
        touch: r.viewport.touch,
        ajax_timeout_ms: (r.ajax_timeout_s * 1000.0).round() as u32,
        page_timeout_ms: r.page_timeout_s.saturating_mul(1000),
        response_timeout_ms: cfg.advanced.response_timeout_s.saturating_mul(1000),
        screenshots: r.screenshots.enabled,
        js_error_reporting: r.js_error_reporting,
        screenshot: os_render::api::ScreenshotOpts {
            size: Some((
                r.screenshots.width.unwrap_or(r.viewport.width),
                r.screenshots.height.unwrap_or(r.viewport.height),
            )),
            fit_content: r.screenshots.fit_to_content,
            resize_wait_ms: r.screenshots.wait_ms,
            format: if r.screenshots.format == os_model::ScreenshotFormat::Png {
                os_render::api::ScreenshotFormat::Png
            } else {
                os_render::api::ScreenshotFormat::Jpeg
            },
            jpeg_quality: r.screenshots.jpeg_quality,
        },
        user_agent: snapshot.map(|f| f.user_agent).or_else(|| {
            response
                .request_headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("user-agent"))
                .map(|(_, v)| v.clone())
        }),
        resource_policy: Some(Arc::new(Policy(Arc::downgrade(sh)))),
        request_identity: Some(Arc::new(Identity {
            sh: Arc::downgrade(sh),
            auth,
            cookies: cookie_jar(sh, item.session),
        })),
        main_response: Some(browser_response(
            response,
            cfg.advanced.cookies != os_model::CookieMode::None,
        )),
        ..Default::default()
    };
    // The browser receives every scoped session cookie before any scripts execute.
    if let Some(jar) = cookie_jar(sh, item.session) {
        opts.cookies = jar
            .browser_snapshot()
            .into_iter()
            .map(|c| os_render::api::Cookie {
                name: c.name,
                value: c.value,
                domain: Some(c.domain),
                path: Some(c.path),
                secure: c.secure,
                http_only: c.http_only,
                expires: c.expires,
            })
            .collect();
    }
    opts
}

fn browser_response(
    response: &os_fetch::HttpResponse,
    cookies: bool,
) -> os_render::api::MainResponse {
    os_render::api::MainResponse {
        status: response.status,
        // Persisted headers may be redacted; use unredacted cookie transport data.
        headers: response
            .headers
            .iter()
            .filter(|(name, _)| !name.eq_ignore_ascii_case("set-cookie"))
            .cloned()
            .chain(
                response
                    .set_cookies
                    .iter()
                    .filter(|_| cookies)
                    .map(|raw| ("Set-Cookie".into(), raw.clone())),
            )
            .collect(),
        body: response.body.to_vec(),
    }
}

pub(crate) fn cookie_jar<F: Fetch>(
    sh: &crate::worker::Shared<F>,
    session: u32,
) -> Option<Arc<os_fetch::CookieJar>> {
    sh.fetcher
        .render_cookie_jar(sh.lock_core().session_jar(session))
}

/// Apply only changed browser cookies: parallel tabs cannot erase unrelated writes.
pub(crate) fn sync_cookies(
    jar: &os_fetch::CookieJar,
    before: &[os_render::api::Cookie],
    after: &[os_render::api::Cookie],
) {
    let key = |c: &os_render::api::Cookie| (c.name.clone(), c.domain.clone(), c.path.clone());
    for cookie in before
        .iter()
        .filter(|c| !after.iter().any(|a| key(c) == key(a)))
    {
        store_cookie(jar, cookie, true);
    }
    for cookie in after.iter().filter(|c| !before.contains(c)) {
        store_cookie(jar, cookie, false);
    }
}
fn store_cookie(jar: &os_fetch::CookieJar, cookie: &os_render::api::Cookie, deleted: bool) {
    let Some(domain) = &cookie.domain else {
        return;
    };
    let mut url = Url::parse(if cookie.secure {
        "https://cookie.invalid/"
    } else {
        "http://cookie.invalid/"
    })
    .expect("static URL");
    if url.set_host(Some(domain.trim_start_matches('.'))).is_err() {
        return;
    }
    let mut raw = format!(
        "{}={}; Path={}",
        cookie.name,
        cookie.value,
        cookie.path.as_deref().unwrap_or("/")
    );
    if domain.starts_with('.') {
        raw.push_str(&format!("; Domain={domain}"));
    }
    if cookie.secure {
        raw.push_str("; Secure");
    }
    if cookie.http_only {
        raw.push_str("; HttpOnly");
    }
    if deleted {
        raw.push_str("; Max-Age=0");
    } else if let Some(expires) = cookie.expires {
        let age = expires
            .saturating_sub(time::OffsetDateTime::now_utc().unix_timestamp())
            .max(0);
        raw.push_str(&format!("; Max-Age={age}"));
    }
    jar.insert_raw(&url, &raw);
}

/// Target identity for provenance and JS counts is independent of XPath/text changes.
pub(crate) fn link_key(link: &ExtractedLink) -> (LinkType, String) {
    (
        link.link_type,
        link.resolved
            .as_deref()
            .and_then(|s| Url::parse(s).ok())
            .map(|u| os_url::compare::compare_key(u.as_str()).unwrap_or_else(|| u.to_string()))
            .unwrap_or_else(|| link.href_raw.clone()),
    )
}

/// Match repeated occurrences one-to-one; keep raw-only links for crawling.
pub(crate) fn merge(
    raw: &ParsedPage,
    dom: &mut ParsedPage,
) -> HashMap<(LinkType, String), LinkOrigin> {
    let keys = |page: &ParsedPage| {
        let mut keys: HashSet<_> = page.links.iter().map(link_key).collect();
        let mut add = |kind, resolved: &Option<String>, raw: &str| {
            keys.insert(link_key(&ExtractedLink {
                link_type: kind,
                resolved: resolved.clone(),
                href_raw: raw.into(),
                ..Default::default()
            }));
        };
        for c in &page.canonicals {
            add(LinkType::Canonical, &c.resolved, &c.raw);
        }
        for h in &page.hreflang {
            add(LinkType::Hreflang, &h.resolved, &h.href_raw);
        }
        for p in &page.pagination {
            add(
                if p.rel_next {
                    LinkType::RelNext
                } else {
                    LinkType::RelPrev
                },
                &p.resolved,
                &p.raw,
            );
        }
        for r in &page.refresh {
            add(
                if r.source == os_model::RefreshSource::Meta {
                    LinkType::MetaRefresh
                } else {
                    LinkType::HttpRefresh
                },
                &r.target,
                r.target_raw.as_deref().unwrap_or(&r.raw),
            );
        }
        keys
    };
    let raw_keys = keys(raw);
    let dom_keys = keys(dom);
    let origins = raw_keys
        .union(&dom_keys)
        .map(|key| {
            (
                key.clone(),
                match (raw_keys.contains(key), dom_keys.contains(key)) {
                    (true, true) => LinkOrigin::Both,
                    (true, false) => LinkOrigin::Html,
                    _ => LinkOrigin::RenderedHtml,
                },
            )
        })
        .collect();
    let mut remaining = HashMap::<_, usize>::new();
    for link in &dom.links {
        *remaining.entry(link_key(link)).or_default() += 1;
    }
    for link in &raw.links {
        let count = remaining.entry(link_key(link)).or_default();
        if *count > 0 {
            *count -= 1;
        } else {
            dom.links.push(link.clone());
        }
    }
    origins
}

/// Scalar pairs are persisted even when HTML retention is disabled.
pub(crate) fn comparison(raw: &ParsedPage, dom: &ParsedPage) -> Vec<(ExtKey, Value)> {
    let mut facts = Vec::new();
    for (prefix, page) in [("raw", raw), ("rendered", dom)] {
        let canonical =
            crate::worker::pick_canonical(&page.canonicals).map(|i| &page.canonicals[i]);
        let robots = page
            .directives
            .iter()
            .filter(|d| d.source == os_model::DirectiveSource::MetaRobots)
            .flat_map(|d| d.tokens.clone())
            .collect::<Vec<_>>()
            .join(",");
        for (name, value) in [
            ("title", page.titles.first().map(|t| t.text.clone())),
            (
                "description",
                page.meta_descriptions.first().map(|t| t.text.clone()),
            ),
            ("h1", page.h1.first().map(|t| t.text.clone())),
            ("canonical", canonical.and_then(|c| c.resolved.clone())),
            ("robots", Some(robots)),
        ] {
            facts.push((
                ExtKey::new("render", &format!("{prefix}_{name}")).expect("static key"),
                value.map(Value::Text).unwrap_or(Value::Null),
            ));
        }
        facts.push((
            ExtKey::new("render", &format!("{prefix}_word_count")).expect("static key"),
            page.word_count
                .map(|n| Value::Int(n.into()))
                .unwrap_or(Value::Null),
        ));
        let links: std::collections::BTreeSet<_> = page
            .links
            .iter()
            .filter(|l| l.link_type == LinkType::Hyperlink)
            .map(|l| link_key(l).1)
            .collect();
        facts.push((
            ExtKey::new("render", &format!("{prefix}_links")).expect("static key"),
            Value::Text(serde_json::to_string(&links).expect("strings")),
        ));
    }
    facts
}
