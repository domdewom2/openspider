use super::{page, protocol, trust, LaunchOpts, RenderError, RenderOpts, RenderedPage};
use chromiumoxide::cdp::browser_protocol::{
    browser::{SetDownloadBehaviorBehavior, SetDownloadBehaviorParams},
    system_info::GetProcessInfoParams,
};
use chromiumoxide::{Browser, BrowserConfig};
use futures_util::StreamExt;
use os_model::{ProxyKind, ProxyMode};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::Duration,
};
use tokio::sync::{RwLock, Semaphore};

#[derive(Debug)]
pub struct Renderer {
    opts: LaunchOpts,
    session: RwLock<Option<Session>>,
    tabs: Semaphore,
    generations: AtomicU64,
}
impl Drop for Renderer {
    fn drop(&mut self) {
        if let Some(mut session) = self.session.get_mut().take() {
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn(async move {
                    shutdown(&mut session).await;
                });
            }
            // Without a runtime Browser's kill-on-drop still terminates the child.
        }
    }
}
#[derive(Debug)]
struct Session {
    browser: Browser,
    handler: tokio::task::JoinHandle<()>,
    pages: AtomicU64,
    retired: AtomicBool,
    _profile: Profile,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.handler.abort();
    }
}
#[derive(Debug)]
struct Profile(Option<PathBuf>);
impl Drop for Profile {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}
impl Profile {
    fn new() -> Result<Self, RenderError> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "openspider-chromium-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            #[cfg_attr(not(unix), allow(unused_mut))] // Unix-only permission configuration below.
            let mut builder = std::fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&path) {
                Ok(()) => return Ok(Self(Some(path))),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(RenderError::Launch(e.to_string())),
            }
        }
    }
}

/// Explicit path wins over environment; an invalid explicit/environment path never
/// silently falls back to another installation.
pub(super) fn executable(opts: &LaunchOpts) -> Result<PathBuf, RenderError> {
    if let Some(path) = opts
        .chromium_path
        .clone()
        .or_else(|| std::env::var_os("OPENSPIDER_CHROMIUM").map(PathBuf::from))
    {
        return if path.is_file() {
            Ok(path)
        } else {
            Err(RenderError::ChromiumMissing {
                path: path.display().to_string(),
            })
        };
    }
    let mut candidates = Vec::new();
    #[cfg(target_os = "macos")]
    {
        for root in [
            Some(PathBuf::from("/Applications")),
            dirs::home_dir().map(|h| h.join("Applications")),
        ]
        .into_iter()
        .flatten()
        {
            candidates.push(root.join("Google Chrome.app/Contents/MacOS/Google Chrome"));
            candidates.push(root.join("Chromium.app/Contents/MacOS/Chromium"));
        }
    }
    #[cfg(target_os = "windows")]
    for key in ["PROGRAMFILES", "PROGRAMFILES(X86)", "LOCALAPPDATA"] {
        if let Some(root) = std::env::var_os(key) {
            candidates.push(
                PathBuf::from(&root)
                    .join("Google")
                    .join("Chrome")
                    .join("Application")
                    .join("chrome.exe"),
            );
            candidates.push(
                PathBuf::from(root)
                    .join("Chromium")
                    .join("Application")
                    .join("chrome.exe"),
            );
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            for name in [
                "chromium",
                "chromium-browser",
                "google-chrome",
                "google-chrome-stable",
                "chrome.exe",
            ] {
                candidates.push(dir.join(name));
            }
        }
    }
    candidates
        .into_iter()
        .find(|p| p.is_file())
        .ok_or_else(|| RenderError::ChromiumMissing {
            path: "OPENSPIDER_CHROMIUM / Systeminstallation".into(),
        })
}

async fn start(opts: &LaunchOpts) -> Result<Session, RenderError> {
    let profile = if opts.user_data_dir.is_none() {
        Profile::new()?
    } else {
        Profile(None)
    };
    let mut config = BrowserConfig::builder()
        .chrome_executable(executable(opts)?)
        .user_data_dir(
            opts.user_data_dir
                .as_ref()
                .or(profile.0.as_ref())
                .expect("profile"),
        )
        .respect_https_errors()
        .disable_https_first()
        .disable_cache()
        .viewport(None)
        .request_timeout(Duration::from_secs(3_665))
        .launch_timeout(Duration::from_millis(opts.launch_timeout_ms.into()))
        .arg("disable-notifications")
        .arg("disable-background-networking")
        .arg((
            "disable-features",
            "ServiceWorker,PushMessaging,Geolocation",
        ));
    if opts.headless {
        config = config.new_headless_mode();
    } else {
        config = config.with_head();
    }
    if opts.no_sandbox {
        config = config.no_sandbox();
    }
    let proxy = &opts.network.proxy;
    match proxy.mode {
        ProxyMode::System => {}
        ProxyMode::None => config = config.arg("no-proxy-server"),
        ProxyMode::Manual => {
            if proxy.host.is_empty()
                || proxy.port == 0
                || proxy.host.contains(['/', '@', ';', '\n', '\r'])
            {
                return Err(RenderError::Launch("Ungültige Proxy-Adresse".into()));
            }
            tokio::time::timeout(
                Duration::from_millis(opts.launch_timeout_ms.into()),
                tokio::net::TcpStream::connect((proxy.host.as_str(), proxy.port)),
            )
            .await
            .map_err(|_| RenderError::Launch("Proxy nicht erreichbar (Timeout)".into()))?
            .map_err(|_| RenderError::Launch("Proxy nicht erreichbar".into()))?;
            let scheme = match proxy.kind {
                ProxyKind::Http => "http",
                ProxyKind::Https => "https",
                ProxyKind::Socks5 => "socks5",
            };
            config = config.arg((
                "proxy-server",
                format!(
                    "{scheme}://{}:{}",
                    if proxy.host.contains(':') && !proxy.host.starts_with('[') {
                        format!("[{}]", proxy.host)
                    } else {
                        proxy.host.clone()
                    },
                    proxy.port
                )
                .as_str(),
            ));
            // Chromium otherwise bypasses loopback implicitly, even for an empty list.
            let bypass = std::iter::once("<-loopback>".to_owned())
                .chain(proxy.bypass.clone())
                .collect::<Vec<_>>()
                .join(";");
            config = config.arg(("proxy-bypass-list", bypass.as_str()));
        }
    }
    if !opts.host_resolver_rules.is_empty() {
        config = config.arg((
            "host-resolver-rules",
            opts.host_resolver_rules.join(",").as_str(),
        ));
    }
    trust::validate(&opts.network)?;
    let (browser, mut handler) = Browser::launch(config.build().map_err(RenderError::Launch)?)
        .await
        .map_err(|e| RenderError::Launch(e.to_string()))?;
    let handler = tokio::spawn(async move {
        while let Some(event) = handler.next().await {
            if event.is_err() {
                break;
            }
        }
    });
    let session = Session {
        browser,
        handler,
        pages: AtomicU64::new(0),
        retired: AtomicBool::new(false),
        _profile: profile,
    };
    session
        .browser
        .execute(SetDownloadBehaviorParams::new(
            SetDownloadBehaviorBehavior::Deny,
        ))
        .await
        .map_err(protocol)?;
    Ok(session)
}

impl Renderer {
    pub async fn launch(opts: LaunchOpts) -> Result<Self, RenderError> {
        if opts.max_tabs == 0 || opts.max_tabs > 1024 || opts.launch_timeout_ms == 0 {
            return Err(RenderError::Launch(
                "Ungültige Tab-Anzahl oder Startzeitgrenze".into(),
            ));
        }
        let session = start(&opts).await?;
        Ok(Self {
            tabs: Semaphore::new(opts.max_tabs),
            opts,
            session: RwLock::new(Some(session)),
            generations: AtomicU64::new(1),
        })
    }
    /// Observable generation for diagnostics and restart tests.
    pub fn generation(&self) -> u64 {
        self.generations.load(Ordering::Acquire)
    }

    pub async fn render(&self, url: &str, opts: &RenderOpts) -> Result<RenderedPage, RenderError> {
        opts.validate()?;
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err(protocol("Nur HTTP(S)-Seiten können gerendert werden"));
        }
        let _permit = self
            .tabs
            .acquire()
            .await
            .map_err(|_| protocol("Renderer geschlossen"))?;
        for attempt in 0..2 {
            let guard = loop {
                self.ensure_session().await?;
                let guard = self.session.read().await;
                let session = guard
                    .as_ref()
                    .ok_or_else(|| protocol("Renderer geschlossen"))?;
                if !session.retired.load(Ordering::Acquire)
                    && reserve_page(&session.pages, self.opts.restart_after_pages)
                {
                    break guard;
                }
            };
            let session = guard.as_ref().expect("active session");
            // A cancelled render also retires the generation, so an orphan/hung tab
            // cannot consume capacity forever. Normal completion disarms the guard.
            struct InFlight<'a>(&'a AtomicBool, bool);
            impl Drop for InFlight<'_> {
                fn drop(&mut self) {
                    if self.1 {
                        self.0.store(true, Ordering::Release);
                    }
                }
            }
            let mut in_flight = InFlight(&session.retired, true);
            let result = tokio::select! {
                result = page::render(&session.browser, url, opts, &self.opts) => result,
                () = watch_memory(session, self.opts.memory_limit_mb) => unreachable!("memory monitor runs until cancelled"),
            };
            in_flight.1 = false;
            let unhealthy = matches!(
                result,
                Err(RenderError::Timeout(_)) | Err(RenderError::Crashed(_))
            ) || session.handler.is_finished();
            if unhealthy {
                session.retired.store(true, Ordering::Release);
            }
            // Retry a crashed target once, not an endlessly executing script.
            if attempt == 0 && matches!(result, Err(RenderError::Crashed(_))) {
                continue;
            }
            return result;
        }
        unreachable!()
    }
    async fn ensure_session(&self) -> Result<(), RenderError> {
        {
            let guard = self.session.read().await;
            let s = guard
                .as_ref()
                .ok_or_else(|| protocol("Renderer geschlossen"))?;
            if !s.retired.load(Ordering::Acquire)
                && !s.handler.is_finished()
                && (self.opts.restart_after_pages == 0
                    || s.pages.load(Ordering::Acquire) < self.opts.restart_after_pages)
            {
                return Ok(());
            }
        }
        let mut guard = self.session.write().await;
        let Some(s) = guard.as_ref() else {
            return Err(protocol("Renderer geschlossen"));
        };
        if !s.retired.load(Ordering::Acquire)
            && !s.handler.is_finished()
            && (self.opts.restart_after_pages == 0
                || s.pages.load(Ordering::Acquire) < self.opts.restart_after_pages)
        {
            return Ok(());
        }
        if let Some(mut old) = guard.take() {
            shutdown(&mut old).await;
        }
        *guard = Some(start(&self.opts).await?);
        self.generations.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
    /// Waits for active renders, reaps Chromium and removes the temporary profile.
    pub async fn close(&self) {
        self.tabs.close();
        if let Some(mut session) = self.session.write().await.take() {
            shutdown(&mut session).await;
        }
    }
}
async fn shutdown(session: &mut Session) {
    let _ = tokio::time::timeout(Duration::from_secs(2), session.browser.close()).await;
    let _ = session.browser.kill().await;
    let _ = session.browser.wait().await;
}

fn reserve_page(pages: &AtomicU64, limit: u64) -> bool {
    let mut n = pages.load(Ordering::Acquire);
    loop {
        if limit != 0 && n >= limit {
            return false;
        }
        match pages.compare_exchange_weak(
            n,
            n.saturating_add(1),
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => return true,
            Err(actual) => n = actual,
        }
    }
}
async fn watch_memory(session: &Session, limit_mb: u64) {
    if limit_mb == 0 {
        std::future::pending::<()>().await;
    }
    loop {
        if let Ok(Ok(info)) = tokio::time::timeout(
            Duration::from_secs(1),
            session.browser.execute(GetProcessInfoParams::default()),
        )
        .await
        {
            let rss: u64 = info
                .result
                .process_info
                .iter()
                .filter_map(|p| super::memory::resident_bytes(p.id as u32))
                .sum();
            if rss >= limit_mb.saturating_mul(1024 * 1024) {
                session.retired.store(true, Ordering::Release);
            }
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chromiumoxide::cdp::browser_protocol::page::CrashParams;
    use std::sync::Arc;

    async fn crash_current(
        renderer: &Renderer,
        site: &os_testsite::TestSite,
        minimum_requests: usize,
    ) {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if site
                    .requests()
                    .iter()
                    .filter(|r| r.path_and_query == "/csr.html")
                    .count()
                    >= minimum_requests
                {
                    let guard = renderer.session.read().await;
                    if let Some(session) = guard.as_ref() {
                        for page in session.browser.pages().await.unwrap_or_default() {
                            if page
                                .url()
                                .await
                                .ok()
                                .flatten()
                                .is_some_and(|u| u.ends_with("/csr.html"))
                            {
                                let _ = tokio::time::timeout(
                                    Duration::from_millis(100),
                                    page.execute(CrashParams::default()),
                                )
                                .await;
                                return;
                            }
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("render target appeared");
    }

    #[tokio::test]
    // spec: CRAWL-078 [M2]
    async fn requires_chromium_retries_target_crash_exactly_once() {
        let site = os_testsite::TestSite::start("render-r01").await.unwrap();
        let renderer = Arc::new(
            Renderer::launch(LaunchOpts::default())
                .await
                .expect("requires_chromium"),
        );
        let opts = RenderOpts {
            ajax_timeout_ms: 1000,
            screenshots: false,
            ..Default::default()
        };
        let r = renderer.clone();
        let url = site.url("/csr.html");
        let first_opts = opts.clone();
        let task = tokio::spawn(async move { r.render(&url, &first_opts).await });
        crash_current(&renderer, &site, 1).await;
        let result = tokio::time::timeout(Duration::from_secs(15), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(result.dom_html.contains("<title>rendered</title>"));
        assert_eq!(renderer.generation(), 2);
        let r = renderer.clone();
        let url = site.url("/csr.html");
        let task = tokio::spawn(async move { r.render(&url, &opts).await });
        crash_current(&renderer, &site, 3).await;
        crash_current(&renderer, &site, 4).await;
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(15), task)
                .await
                .unwrap()
                .unwrap(),
            Err(RenderError::Crashed(_))
        ));
        assert_eq!(
            site.requests()
                .iter()
                .filter(|r| r.path_and_query == "/csr.html")
                .count(),
            4
        );
        renderer.close().await;
    }
    #[tokio::test]
    // spec: CRAWL-078 [M2]
    async fn requires_chromium_denies_downloads_in_each_context() {
        use chromiumoxide::cdp::browser_protocol::browser::{
            DownloadProgressState, EventDownloadProgress,
        };
        let site = os_testsite::TestSite::start("render-r01").await.unwrap();
        let renderer = Renderer::launch(LaunchOpts::default())
            .await
            .expect("requires_chromium");
        let mut events = renderer
            .session
            .read()
            .await
            .as_ref()
            .unwrap()
            .browser
            .event_listener::<EventDownloadProgress>()
            .await
            .unwrap();
        renderer
            .render(
                &site.url("/download.html"),
                &RenderOpts {
                    ajax_timeout_ms: 500,
                    screenshots: false,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            while let Some(event) = events.next().await {
                if event.state == DownloadProgressState::Canceled {
                    return;
                }
                assert_ne!(event.state, DownloadProgressState::Completed);
            }
            panic!("missing cancelled download event");
        })
        .await
        .unwrap();
        renderer.close().await;
    }
    #[tokio::test]
    // spec: PERF-006
    async fn requires_chromium_cancellation_closes_idle_hung_target() {
        let site = os_testsite::TestSite::start("render-r01").await.unwrap();
        let renderer = Arc::new(
            Renderer::launch(LaunchOpts {
                memory_limit_mb: 0,
                ..Default::default()
            })
            .await
            .expect("requires_chromium"),
        );
        let r = renderer.clone();
        let url = site.url("/hang.html");
        let task = tokio::spawn(async move { r.render(&url, &RenderOpts::default()).await });
        let target = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let guard = renderer.session.read().await;
                for page in guard.as_ref().unwrap().browser.pages().await.unwrap() {
                    if page
                        .url()
                        .await
                        .unwrap()
                        .is_some_and(|u| u.ends_with("/hang.html"))
                    {
                        return page.target_id().clone();
                    }
                }
                drop(guard);
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        task.abort();
        let _ = task.await;
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let guard = renderer.session.read().await;
                if !guard
                    .as_ref()
                    .unwrap()
                    .browser
                    .pages()
                    .await
                    .unwrap()
                    .iter()
                    .any(|page| page.target_id() == &target)
                {
                    break;
                }
                drop(guard);
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("cancelled target closed without a subsequent render");
        assert_eq!(renderer.generation(), 1);
        renderer.close().await;
    }
}
