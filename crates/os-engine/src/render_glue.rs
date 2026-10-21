//! Browser integration, bounded concurrency and comparison facts (M2-E05).
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, OnceLock, Weak};

use futures_util::future::BoxFuture;
use os_fetch::Fetch;
use os_model::{CrawlConfig, ExtKey, ExtractedLink, LinkOrigin, LinkType, ParsedPage, Value};
pub use os_render::{RenderError, RenderOpts, RenderedPage};
use url::Url;

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
        let (limit, _, processes) = crate::memory::resource_limits();
        let sample = crate::memory::sample();
        let budget = limit
            .saturating_sub(sample.used())
            .min(sample.available_bytes);
        let tabs = tab_limit(cfg, processes, budget);
        let mut launch = os_render::LaunchOpts {
            chromium_path: cfg.rendering.chromium_path.as_ref().map(Into::into),
            max_tabs: tabs,
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
                    std::time::Duration::from_millis(u64::from(opts.page_timeout_ms)),
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
}

struct Policy<F: Fetch>(Weak<crate::worker::Shared<F>>);
impl<F: Fetch> std::fmt::Debug for Policy<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CrawlResourcePolicy")
    }
}
impl<F: Fetch> os_render::api::ResourcePolicy for Policy<F> {
    fn allowed<'a>(&'a self, url: &'a str) -> BoxFuture<'a, bool> {
        Box::pin(async move {
            let (Some(sh), Ok(url)) = (self.0.upgrade(), Url::parse(url)) else {
                return false;
            };
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
struct Identity<F: Fetch> {
    sh: Weak<crate::worker::Shared<F>>,
    auth: os_fetch::auth::AuthMatcher,
}
impl<F: Fetch> std::fmt::Debug for Identity<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CrawlRequestIdentity")
    }
}
impl<F: Fetch> os_render::api::RequestIdentity for Identity<F> {
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
    let credentials = Url::parse(&item.url)
        .ok()
        .and_then(|url| auth.rendering_credentials(&url))
        .map(|c| os_render::api::Credentials {
            username: c.username,
            password: c.password,
        });
    let mut opts = RenderOpts {
        credentials,
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
        })),
        main_response: Some(os_render::api::MainResponse {
            status: response.status,
            // Persisted header values may be redacted; cookies have a separate
            // unredacted transport field. Never replay placeholder Set-Cookie.
            headers: response
                .headers
                .iter()
                .filter(|(name, _)| !name.eq_ignore_ascii_case("set-cookie"))
                .cloned()
                .chain(
                    response
                        .set_cookies
                        .iter()
                        .filter(|_| cfg.advanced.cookies != os_model::CookieMode::None)
                        .map(|raw| ("Set-Cookie".into(), raw.clone())),
                )
                .collect(),
            body: response.body.to_vec(),
        }),
        ..Default::default()
    };
    // The browser receives every scoped session cookie before any scripts execute.
    if let Some(jar) = sh.lock_core().session_jar(item.session) {
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
