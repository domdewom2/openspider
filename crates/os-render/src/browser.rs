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
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::{RwLock, Semaphore};

#[derive(Debug)]
pub struct Renderer {
    opts: LaunchOpts,
    session: RwLock<Option<Session>>,
    tabs: Semaphore,
    generations: AtomicU64,
    #[cfg(test)]
    targets: std::sync::Mutex<Option<super::page::TargetSender>>,
    #[cfg(test)]
    completions: std::sync::Mutex<Option<super::page::CompletionSender>>,
}
impl Drop for Renderer {
    fn drop(&mut self) {
        if let Some(mut session) = self.session.get_mut().take() {
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn(async move {
                    shutdown(&mut session).await;
                });
            }
            // Ohne Laufzeit beendet Browser das Kind weiterhin beim Drop.
        }
    }
}
#[derive(Debug)]
struct Session {
    browser: Arc<Browser>,
    handler: tokio::task::JoinHandle<()>,
    pages: Arc<AtomicU64>,
    retired: Arc<AtomicBool>,
    memory_monitor: Option<tokio::task::JoinHandle<()>>,
    _profile: Profile,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.handler.abort();
        if let Some(monitor) = &self.memory_monitor {
            monitor.abort();
        }
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
            #[cfg_attr(not(unix), allow(unused_mut))]
            // Rechtekonfiguration unten gilt nur unter Unix.
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

/// Expliziter Pfad hat Vorrang vor der Umgebung; ungültige Vorgaben führen
/// niemals still zu einer anderen Installation.
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
    trust::validate(opts)?;
    let profile = if opts.user_data_dir.is_none() {
        Profile::new()?
    } else {
        Profile(None)
    };
    if opts.disable_cookie_storage {
        let root = profile.0.as_ref().ok_or_else(|| {
            RenderError::Launch("Cookie-Sperre benötigt ein temporäres Browserprofil".into())
        })?;
        let directory = root.join("Default");
        std::fs::create_dir(&directory).map_err(|e| RenderError::Launch(e.to_string()))?;
        std::fs::write(
            directory.join("Preferences"),
            r#"{"profile":{"default_content_setting_values":{"cookies":2}}}"#,
        )
        .map_err(|e| RenderError::Launch(e.to_string()))?;
    }
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
    if opts.protected_network {
        config = config
            .arg("disable-quic")
            .arg(("force-webrtc-ip-handling-policy", "disable_non_proxied_udp"))
            .arg("disable-extensions");
    }
    if opts.suppress_default_accept_language {
        config = config.arg(("accept-lang", ""));
    }
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
            // Chromium umgeht sonst Loopback implizit, auch bei leerer Ausnahmeliste.
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
    let browser = Arc::new(browser);
    let retired = Arc::new(AtomicBool::new(false));
    let pages = Arc::new(AtomicU64::new(0));
    let memory_monitor = if opts.memory_limit_mb > 0 {
        Some(tokio::spawn(watch_memory(
            browser.clone(),
            retired.clone(),
            pages.clone(),
            opts.memory_limit_mb,
        )))
    } else {
        None
    };
    let session = Session {
        browser,
        handler,
        pages,
        retired,
        memory_monitor,
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
            #[cfg(test)]
            targets: std::sync::Mutex::new(None),
            #[cfg(test)]
            completions: std::sync::Mutex::new(None),
        })
    }
    /// Dedicated login tab in this renderer's temporary browser profile (CRAWL-120).
    pub async fn open_login_page(&self) -> Result<(chromiumoxide::Page, String), RenderError> {
        self.ensure_session().await?;
        let guard = self.session.read().await;
        let session = guard
            .as_ref()
            .ok_or_else(|| protocol("Browser geschlossen"))?;
        let context = session
            .browser
            .create_browser_context(
                chromiumoxide::cdp::browser_protocol::target::CreateBrowserContextParams::builder()
                    .dispose_on_detach(true)
                    .build(),
            )
            .await
            .map_err(protocol)?;
        let page = session
            .browser
            .new_page(
                chromiumoxide::cdp::browser_protocol::target::CreateTargetParams::builder()
                    .url("about:blank")
                    .browser_context_id(context)
                    .build()
                    .map_err(protocol)?,
            )
            .await
            .map_err(protocol)?;
        Ok((page, session.browser.websocket_address().clone()))
    }

    /// Sichtbare Generation für Diagnose und Neustarttests.
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
            #[cfg(test)]
            let targets = self.targets.lock().expect("targets").clone();
            #[cfg(test)]
            let completions = self.completions.lock().expect("completions").clone();
            let result = page::render(
                &session.browser,
                url,
                opts,
                &self.opts,
                session.retired.clone(),
                #[cfg(test)]
                targets,
                #[cfg(test)]
                completions,
            )
            .await;
            let unhealthy =
                matches!(result, Err(RenderError::Crashed(_))) || session.handler.is_finished();
            if unhealthy {
                session.retired.store(true, Ordering::Release);
            }
            // Ein abgestürztes Ziel wird einmal wiederholt; eine JS-Endlosschleife nicht.
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
    /// Wartet laufende Renderaufträge ab, beendet Chromium und entfernt das temporäre Profil.
    pub async fn close(&self) {
        self.tabs.close();
        if let Some(mut session) = self.session.write().await.take() {
            shutdown(&mut session).await;
        }
    }
}
async fn shutdown(session: &mut Session) {
    if let Some(monitor) = session.memory_monitor.take() {
        monitor.abort();
        let _ = monitor.await;
    }
    let browser = Arc::get_mut(&mut session.browser).expect("Sitzung besitzt Browser exklusiv");
    let _ = tokio::time::timeout(Duration::from_secs(2), browser.close()).await;
    let _ = browser.kill().await;
    let _ = browser.wait().await;
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
async fn watch_memory(
    browser: Arc<Browser>,
    retired: Arc<AtomicBool>,
    pages: Arc<AtomicU64>,
    limit_mb: u64,
) {
    loop {
        // Eine frische Generation muss mindestens einen Auftrag annehmen können,
        // selbst wenn bereits der Chromium-Grundbedarf die Grenze überschreitet.
        if pages.load(Ordering::Acquire) == 0 {
            tokio::time::sleep(Duration::from_millis(250)).await;
            continue;
        }
        if let Ok(Ok(info)) = tokio::time::timeout(
            Duration::from_secs(1),
            browser.execute(GetProcessInfoParams::default()),
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
                retired.store(true, Ordering::Release);
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::super::intercept::TargetWatch;
    use super::*;
    use std::sync::Arc;

    // Echte Browsertests nacheinander: parallele Chromium-Starts überschreiten auf dem Windows-Runner die Startzeit
    // ("Timeout while resolving websocket URL"). Gleiche Regel wie in tests/render_chromium.rs.
    static CHROMIUM: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[derive(Debug, Default)]
    struct HoldFirstResource {
        path: &'static str,
        held: AtomicBool,
        started: tokio::sync::Notify,
    }
    impl super::super::ResourcePolicy for HoldFirstResource {
        fn allowed<'a>(
            &'a self,
            url: &'a str,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + 'a>> {
            Box::pin(async move {
                if url.ends_with(self.path) && !self.held.swap(true, Ordering::AcqRel) {
                    self.started.notify_one();
                    std::future::pending::<()>().await;
                }
                true
            })
        }
    }

    async fn watch_target(target: page::TestTarget) -> TargetWatch {
        tokio::time::timeout(
            Duration::from_secs(30),
            TargetWatch::start(&target.websocket, &target.id),
        )
        .await
        .expect("Crash-Steuerkanal bereit")
        .unwrap()
    }

    // Unter Unix beendet SIGKILL den Renderer unabhängig vom Absturzbehandler, den Page.crash
    // anstößt (Chrome 155 unter Linux meldete danach nichts); sonst bleibt Page.crash.
    async fn crash_target(mut crashed: TargetWatch) {
        #[cfg(unix)]
        let result = crashed.kill_renderers(Duration::from_secs(60)).await;
        #[cfg(not(unix))]
        let result = crashed.crash(Duration::from_secs(60)).await;
        result.expect("Renderer beenden und Crash-Ereignis empfangen");
    }

    #[tokio::test(flavor = "multi_thread")]
    // Spec-Fixture `crawl-js`: erster Crash nach Navigationsabschluss, während Fetch
    // auf die Richtlinie wartet; weitere Crashes vor Navigation.
    // spec: CRAWL-078
    async fn requires_chromium_retries_target_crash_exactly_once() {
        let _chromium = CHROMIUM.lock().await;
        let site = os_testsite::TestSite::start("render-r01").await.unwrap();
        let renderer = Arc::new(
            Renderer::launch(LaunchOpts {
                memory_limit_mb: 0,
                ..Default::default()
            })
            .await
            .expect("requires_chromium"),
        );
        let (tx, mut targets) = tokio::sync::mpsc::unbounded_channel();
        *renderer.targets.lock().unwrap() = Some(tx);
        let policy = Arc::new(HoldFirstResource {
            path: "/slow.js",
            ..Default::default()
        });
        let opts = RenderOpts {
            ajax_timeout_ms: 100,
            screenshots: false,
            page_timeout_ms: 120_000,
            resource_policy: Some(policy.clone()),
            ..Default::default()
        };
        let r = renderer.clone();
        let url = site.url("/crash-active.html");
        let first_opts = opts.clone();
        let mut task = tokio::spawn(async move { r.render(&url, &first_opts).await });
        let (page, resume) = tokio::time::timeout(Duration::from_secs(60), targets.recv())
            .await
            .unwrap()
            .unwrap();
        // Steuerkanal und Überwachung stehen vor der Navigation bereit.
        let crashed = watch_target(page).await;
        resume.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(30), policy.started.notified())
            .await
            .expect("Ausgeführtes Dokument erreicht Ressourcenrichtlinie");
        crash_target(crashed).await;
        let (_page, resume) = tokio::time::timeout(Duration::from_secs(60), async {
            tokio::select! {
                target = targets.recv() => target,
                result = &mut task => panic!("Rendern endet vor der Wiederholung: {result:?}"),
            }
        })
        .await
        .unwrap()
        .unwrap();
        resume.send(()).unwrap();
        let result = tokio::time::timeout(Duration::from_secs(60), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(result.dom_html.contains("<title>rendered</title>"));
        assert_eq!(renderer.generation(), 2);
        let r = renderer.clone();
        let url = site.url("/csr.html");
        let task = tokio::spawn(async move { r.render(&url, &opts).await });
        for _ in 0..2 {
            let (page, _resume) = tokio::time::timeout(Duration::from_secs(60), targets.recv())
                .await
                .unwrap()
                .unwrap();
            crash_target(watch_target(page).await).await;
        }
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(60), task)
                .await
                .unwrap()
                .unwrap(),
            Err(RenderError::Crashed(_))
        ));
        assert_eq!(renderer.generation(), 3);
        assert!(targets.try_recv().is_err(), "keine dritte Verarbeitung");
        assert_eq!(
            site.requests()
                .iter()
                .filter(|r| r.path_and_query == "/crash-active.html")
                .count(),
            2
        );
        renderer.close().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    // Spec-Fixture `crawl-js`: eine noch nicht freigegebene Hauptnavigation darf
    // nicht als erfolgreicher Crash gelten oder einen ereignislosen Timeout erzeugen.
    // spec: CRAWL-078
    async fn requires_chromium_reports_rejected_crash_command() {
        let _chromium = CHROMIUM.lock().await;
        let site = os_testsite::TestSite::start("render-r01").await.unwrap();
        let renderer = Arc::new(
            Renderer::launch(LaunchOpts {
                memory_limit_mb: 0,
                ..Default::default()
            })
            .await
            .expect("requires_chromium"),
        );
        let (tx, mut targets) = tokio::sync::mpsc::unbounded_channel();
        *renderer.targets.lock().unwrap() = Some(tx);
        let policy = Arc::new(HoldFirstResource {
            path: "/csr.html",
            ..Default::default()
        });
        let opts = RenderOpts {
            screenshots: false,
            response_timeout_ms: 120_000,
            page_timeout_ms: 120_000,
            resource_policy: Some(policy.clone()),
            ..Default::default()
        };
        let r = renderer.clone();
        let url = site.url("/csr.html");
        let task = tokio::spawn(async move { r.render(&url, &opts).await });
        let (target, resume) = tokio::time::timeout(Duration::from_secs(30), targets.recv())
            .await
            .unwrap()
            .unwrap();
        let mut watch = watch_target(target).await;
        resume.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(30), policy.started.notified())
            .await
            .expect("Hauptnavigation wird angehalten");
        let error = watch.crash(Duration::from_secs(10)).await.unwrap_err();
        assert!(
            matches!(error, RenderError::Protocol(ref detail)
                if detail.contains("Page.crash") && detail.contains("pending navigations")),
            "{error:?}"
        );
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        drop(watch);
        renderer.close().await;
    }

    #[tokio::test]
    // Spec-Fixture `crawl-js` (paketeigen unter render-r01).
    // spec: CRAWL-078
    async fn requires_chromium_denies_downloads_in_each_context() {
        let _chromium = CHROMIUM.lock().await;
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
        let renderer = Arc::new(renderer);
        let (tx, mut completions) = tokio::sync::mpsc::unbounded_channel();
        *renderer.completions.lock().unwrap() = Some(tx);
        let mut canceled = std::collections::HashSet::new();
        for _ in 0..2 {
            let r = renderer.clone();
            let url = site.url("/download.html");
            let render = tokio::spawn(async move {
                r.render(
                    &url,
                    &RenderOpts {
                        ajax_timeout_ms: 500,
                        screenshots: false,
                        page_timeout_ms: 120_000,
                        ..Default::default()
                    },
                )
                .await
            });
            // Der Kontext bleibt bis zur Beobachtung des Ereignisses geöffnet.
            let resume = tokio::time::timeout(Duration::from_secs(120), completions.recv())
                .await
                .expect("Renderabschluss")
                .expect("Test-Hook offen");
            tokio::time::timeout(Duration::from_secs(60), async {
                while let Some(event) = events.next().await {
                    if event.state == DownloadProgressState::Canceled {
                        assert!(canceled.insert(event.guid.clone()), "doppeltes Ereignis");
                        return;
                    }
                    assert_ne!(event.state, DownloadProgressState::Completed);
                }
                panic!("missing cancelled download event");
            })
            .await
            .expect("Download-Abbruch");
            resume.send(()).unwrap();
            tokio::time::timeout(Duration::from_secs(30), render)
                .await
                .expect("Kontextabbau")
                .unwrap()
                .unwrap();
        }
        renderer.close().await;
    }
    #[tokio::test(flavor = "multi_thread")]
    // Spec-Fixture `crawl-js`: ein aktiver Nachbartab darf die Erholung nicht blockieren.
    // spec: PERF-006
    async fn requires_chromium_timeout_keeps_parallel_session_available() {
        let _chromium = CHROMIUM.lock().await;
        let site = os_testsite::TestSite::start("render-r01").await.unwrap();
        let renderer = Arc::new(
            Renderer::launch(LaunchOpts {
                max_tabs: 3,
                memory_limit_mb: 0,
                restart_after_pages: 0,
                ..Default::default()
            })
            .await
            .expect("requires_chromium"),
        );
        let (tx, mut targets) = tokio::sync::mpsc::unbounded_channel();
        *renderer.targets.lock().unwrap() = Some(tx);
        let opts = RenderOpts {
            screenshots: false,
            ajax_timeout_ms: 0,
            page_timeout_ms: 120_000,
            ..Default::default()
        };
        let r = renderer.clone();
        let url = site.url("/csr.html");
        let held_opts = opts.clone();
        let held = tokio::spawn(async move { r.render(&url, &held_opts).await });
        let (_held_page, resume_held) =
            tokio::time::timeout(Duration::from_secs(30), targets.recv())
                .await
                .unwrap()
                .unwrap();
        let r = renderer.clone();
        let url = site.url("/hang.html");
        let hang = tokio::spawn(async move {
            r.render(
                &url,
                &RenderOpts {
                    screenshots: false,
                    page_timeout_ms: 15_000,
                    ..Default::default()
                },
            )
            .await
        });
        let (_page, resume) = tokio::time::timeout(Duration::from_secs(30), targets.recv())
            .await
            .unwrap()
            .unwrap();
        resume.send(()).unwrap();
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(30), hang)
                .await
                .unwrap()
                .unwrap(),
            Err(RenderError::Timeout(_))
        ));
        let r = renderer.clone();
        let url = site.url("/csr.html");
        let next = tokio::spawn(async move { r.render(&url, &opts).await });
        let (_page, resume) = tokio::time::timeout(Duration::from_secs(15), targets.recv())
            .await
            .expect("Folgerender wartet nicht auf Nachbartab")
            .unwrap();
        resume.send(()).unwrap();
        assert!(tokio::time::timeout(Duration::from_secs(15), next)
            .await
            .unwrap()
            .unwrap()
            .unwrap()
            .dom_html
            .contains("<title>rendered</title>"));
        assert_eq!(renderer.generation(), 1);
        resume_held.send(()).unwrap();
        held.await.unwrap().unwrap();
        renderer.close().await;
    }

    #[tokio::test]
    // Spec-Fixture `crawl-js` (paketeigen unter render-r01).
    // spec: PERF-006
    async fn requires_chromium_cancellation_closes_idle_hung_target() {
        let _chromium = CHROMIUM.lock().await;
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
