use os_render::api::{Credentials, MainResponse, ResourcePolicy, ScreenshotFormat};
use os_render::{LaunchOpts, RenderError, RenderOpts, Renderer};
use os_testsite::TestSite;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

// Real browser tests are mandatory; missing Chromium is a failure, never a skip.
static CHROMIUM: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
async fn requires_chromium(opts: LaunchOpts) -> Renderer {
    Renderer::launch(opts)
        .await
        .expect("requires_chromium: install Chrome or set OPENSPIDER_CHROMIUM")
}
fn quick() -> RenderOpts {
    RenderOpts {
        ajax_timeout_ms: 100,
        screenshots: false,
        ..RenderOpts::default()
    }
}
fn png_size(bytes: &[u8]) -> (u32, u32) {
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    (
        u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
        u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
    )
}

#[tokio::test]
// Spec-Fixture `crawl-js` (paketeigen unter render-r01).
// spec: CRAWL-077 [P1]
async fn missing_chromium_is_actionable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing-chromium");
    let error = Renderer::launch(LaunchOpts {
        chromium_path: Some(path.clone()),
        ..Default::default()
    })
    .await
    .unwrap_err();
    assert!(matches!(error, RenderError::ChromiumMissing { .. }));
    assert!(error.to_string().contains("err-chromium-missing"));
    assert!(error.to_string().contains(path.to_str().unwrap()));
    assert!(error.to_string().contains("OPENSPIDER_CHROMIUM"));
}

#[tokio::test]
// Spec-Fixture `crawl-js` (paketeigen unter render-r01).
// spec: CRAWL-078
async fn requires_chromium_renders_and_reuses_engine_response() {
    let _lock = CHROMIUM.lock().await;
    let site = TestSite::start("render-r01").await.unwrap();
    let renderer = requires_chromium(LaunchOpts::default()).await;
    let mut opts = quick();
    opts.user_agent = Some("OpenSpider-Fixture".into());
    opts.headers.insert("X-Fixture".into(), "render".into());
    opts.cookies.push(os_render::api::Cookie {
        name: "session".into(),
        value: "fixture".into(),
        path: Some("/".into()),
        http_only: true,
        expires: Some(4_102_444_800),
        ..Default::default()
    });
    let result = renderer
        .render(&site.url("/csr.html"), &opts)
        .await
        .unwrap();
    assert!(result.dom_html.contains("<title>rendered</title>"));
    assert!(result.dom_html.contains("href=\"/generated\""));
    let req = site
        .requests()
        .into_iter()
        .find(|r| r.path_and_query == "/csr.html")
        .unwrap();
    assert_eq!(req.header("user-agent").unwrap(), "OpenSpider-Fixture");
    assert_eq!(req.header("x-fixture").unwrap(), "render");
    assert!(req.header("cookie").unwrap().contains("session=fixture"));
    opts.main_response = Some(MainResponse {
        status: 200,
        headers: vec![("Content-Type".into(), "text/html".into())],
        body: include_bytes!("../../../tests/fixtures/render-r01/csr.html").to_vec(),
    });
    let replay = renderer
        .render(&site.url("/reuse.html"), &opts)
        .await
        .unwrap();
    assert!(replay.dom_html.contains("<title>rendered</title>"));
    assert!(!site
        .requests()
        .iter()
        .any(|r| r.path_and_query == "/reuse.html"));
    assert!(replay
        .network
        .iter()
        .any(|r| r.url.ends_with("/reuse.html") && r.status == 200));
    // K2-82: Berechtigungsabfragen und Worker-Registrierung laufen über den Browserprozess
    // und lösen ihr Promise erst nach `load` auf; 100 ms reichen auf belasteten CI-Runnern
    // nicht sicher. Geprüft wird das Ergebnis, nicht die Zeit (CRAWL-078 nennt keine).
    let security = renderer
        .render(
            &site.url("/security.html"),
            &RenderOpts {
                ajax_timeout_ms: 2000,
                ..quick()
            },
        )
        .await
        .unwrap();
    assert!(
        security
            .dom_html
            .contains("geo=denied,notifications=denied,worker=blocked"),
        "{}",
        security.dom_html
    );
    renderer.close().await;
}

#[tokio::test]
// Spec-Fixture `crawl-js` (paketeigen unter render-r01).
// spec: CRAWL-080
// spec: CRAWL-095
async fn requires_chromium_waits_after_load_and_preserves_partial_dom() {
    let _lock = CHROMIUM.lock().await;
    let site = TestSite::start("render-r01").await.unwrap();
    let renderer = requires_chromium(LaunchOpts::default()).await;
    let mut opts = quick();
    opts.ajax_timeout_ms = 1000;
    let early = renderer
        .render(&site.url("/delayed.html"), &opts)
        .await
        .unwrap();
    assert!(early.dom_html.contains("id=\"status\">waiting"));
    opts.ajax_timeout_ms = 5000;
    let late = renderer
        .render(&site.url("/delayed.html"), &opts)
        .await
        .unwrap();
    assert!(late.dom_html.contains("id=\"status\">arrived"));
    assert!(late.timings.load_ms >= 650);
    opts.ajax_timeout_ms = 100;
    // K2-82: Das Dokument muss innerhalb des Antwort-Timeouts ankommen, erst `load` bleibt
    // aus. 500 ms reichten auf langsamen Runnern nicht immer für Antwort und Commit; die
    // Spec nennt hier keine Zahl. Geprüft bleibt Antwort-Timeout + AJAX-Timeout.
    opts.response_timeout_ms = 3000;
    let partial = renderer
        .render(&site.url("/never-load.html"), &opts)
        .await
        .unwrap();
    assert!(partial.load_timed_out);
    assert!(partial.dom_html.contains("partial DOM"));
    assert!(partial.dom_html.contains("data-ran=\"yes\""));
    assert!(partial.timings.total_ms >= opts.response_timeout_ms + opts.ajax_timeout_ms);
    assert_eq!(partial.timings.screenshot_ms, 0);
    // K2-82: Kommt innerhalb des Antwort-Timeouts gar keine Antwort, gibt es kein DOM
    // dieser URL. Ergebnis ist ein Timeout, nicht about:blank als gerenderte Seite.
    opts.response_timeout_ms = 300;
    let late = renderer
        .render(&site.url("/late-document.html"), &opts)
        .await;
    assert!(
        matches!(late, Err(RenderError::Timeout(_))),
        "{:?}",
        late.map(|page| (page.final_url, page.load_timed_out, page.dom_html))
    );
    assert!(site
        .requests()
        .iter()
        .any(|r| r.path_and_query == "/late-document.html"));
    opts.response_timeout_ms = 3000;
    opts.ajax_timeout_ms = 1;
    assert!(renderer
        .render(&site.url("/csr.html"), &opts)
        .await
        .is_err());
    renderer.close().await;
}

#[tokio::test]
// Spec-Fixture `crawl-js` (paketeigen unter render-r01).
// spec: CRAWL-081
async fn requires_chromium_emulates_devices_and_expands_google_viewport() {
    let _lock = CHROMIUM.lock().await;
    let site = TestSite::start("render-r01").await.unwrap();
    let renderer = requires_chromium(LaunchOpts::default()).await;
    let mut opts = quick();
    opts.ajax_timeout_ms = 2000;
    let mobile = renderer
        .render(&site.url("/viewport.html"), &opts)
        .await
        .unwrap();
    assert!(
        mobile.dom_html.contains("411x731 touch=1"),
        "{}",
        mobile.dom_html
    );
    assert!(mobile.dom_html.contains("data-height=\"6000\""));
    assert!(mobile.images[0].1.real_w.is_some());
    site.reset_request_log();
    let fixed = RenderOpts {
        ajax_timeout_ms: 500,
        screenshots: false,
        ..RenderOpts::fixed_viewport(375, 667)
    };
    let page = renderer
        .render(&site.url("/viewport.html"), &fixed)
        .await
        .unwrap();
    assert!(page.dom_html.contains("375x667 touch=0"));
    assert!(!site
        .requests()
        .iter()
        .any(|r| r.path_and_query == "/pixel.png"));
    assert_eq!(page.images[0].1.in_viewport, Some(false));
    let desktop = RenderOpts {
        ajax_timeout_ms: 100,
        screenshots: false,
        ..RenderOpts::googlebot_desktop()
    };
    let page = renderer
        .render(&site.url("/viewport.html"), &desktop)
        .await
        .unwrap();
    assert!(page.dom_html.contains("1024x768 touch=0"));
    renderer.close().await;
}

#[tokio::test]
// Spec-Fixture `crawl-js` (paketeigen unter render-r01).
// spec: CRAWL-082
async fn requires_chromium_captures_sized_png_jpeg_and_content_cap() {
    let _lock = CHROMIUM.lock().await;
    let site = TestSite::start("render-r01").await.unwrap();
    let renderer = requires_chromium(LaunchOpts::default()).await;
    let mut opts = RenderOpts {
        ajax_timeout_ms: 0,
        ..RenderOpts::fixed_viewport(800, 600)
    };
    opts.screenshot.size = Some((1024, 768));
    let result = renderer
        .render(&site.url("/tall.html"), &opts)
        .await
        .unwrap();
    assert_eq!(png_size(&result.screenshot_tiles[0].data), (1024, 768));
    assert!(result.timings.screenshot_ms >= opts.screenshot.resize_wait_ms);
    assert!(result.timings.total_ms >= result.timings.screenshot_ms);
    opts.device_scale_factor = 2.0;
    let result = renderer
        .render(&site.url("/tall.html"), &opts)
        .await
        .unwrap();
    assert_eq!(png_size(&result.screenshot_tiles[0].data), (2048, 1536));
    opts.device_scale_factor = 1.0;
    opts.screenshot.fit_content = true;
    for height in [3000, 20000] {
        let result = renderer
            .render(&site.url(&format!("/tall.html?height={height}")), &opts)
            .await
            .unwrap();
        assert_eq!(
            png_size(&result.screenshot_tiles[0].data),
            (1024, height.min(8192))
        );
    }
    opts.screenshot.format = ScreenshotFormat::Jpeg;
    opts.screenshot.fit_content = false;
    let result = renderer
        .render(&site.url("/tall.html"), &opts)
        .await
        .unwrap();
    assert_eq!(&result.screenshot_tiles[0].data[..3], &[0xff, 0xd8, 0xff]);
    opts.screenshots = false;
    assert!(renderer
        .render(&site.url("/tall.html"), &opts)
        .await
        .unwrap()
        .screenshot_tiles
        .is_empty());
    renderer.close().await;
}

#[derive(Debug)]
struct Robots;
impl ResourcePolicy for Robots {
    fn allowed<'a>(
        &'a self,
        url: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + 'a>> {
        Box::pin(async move { !url.contains("/private-js/") })
    }
}
#[tokio::test]
// Spec-Fixture `crawl-js` (paketeigen unter render-r01).
// spec: CRAWL-083
// spec: CRAWL-084
async fn requires_chromium_blocks_before_network_and_records_http_errors() {
    let _lock = CHROMIUM.lock().await;
    let site = TestSite::start("render-r01").await.unwrap();
    let renderer = requires_chromium(LaunchOpts::default()).await;
    let mut opts = quick();
    opts.resource_policy = Some(Arc::new(Robots));
    let result = renderer
        .render(&site.url("/blocked.html"), &opts)
        .await
        .unwrap();
    assert!(result
        .blocked
        .iter()
        .any(|b| b.reason == "robots" && b.url.ends_with("/private-js/app.js")));
    for status in [404, 500] {
        assert!(
            result.blocked.iter().any(|b| b.status == Some(status)),
            "{:?}",
            result.blocked
        );
    }
    assert!(!site
        .requests()
        .iter()
        .any(|r| r.path_and_query == "/private-js/app.js"));
    assert!(
        result.console.is_empty(),
        "interne 404/500-Meldungen: {:?}",
        result.console
    );
    opts.resource_policy = None; // Engine gibt die Ressource über ihre robots-Richtlinie frei.
    let result = renderer
        .render(&site.url("/blocked.html"), &opts)
        .await
        .unwrap();
    assert!(!result.blocked.iter().any(|b| b.reason == "robots"));
    assert!(result.dom_html.contains("data-private-script=\"loaded\""));
    renderer.close().await;
}

#[tokio::test]
// Spec-Fixture `crawl-js` (paketeigen unter render-r01).
// spec: CRAWL-084
async fn requires_chromium_bounds_console_with_locations_and_cutoff() {
    let _lock = CHROMIUM.lock().await;
    let site = TestSite::start("render-r01").await.unwrap();
    let renderer = requires_chromium(LaunchOpts::default()).await;
    let mut opts = quick();
    opts.ajax_timeout_ms = 100;
    // Screenshot wait crosses the late message; it must not enter the console.
    opts.screenshots = true;
    opts.screenshot.resize_wait_ms = 1200;
    let result = renderer
        .render(&site.url("/errors.html"), &opts)
        .await
        .unwrap();
    assert_eq!(result.console.len(), 3, "{:?}", result.console);
    let exception = result
        .console
        .iter()
        .find(|c| c.text.contains("fixture exception"))
        .unwrap();
    assert_eq!(exception.level, "error");
    assert!(exception.source.as_ref().unwrap().ends_with("/errors.html"));
    assert!(exception.line.unwrap() > 0 && exception.column.unwrap() > 0);
    assert!(exception.timestamp_ms.is_some());
    opts.screenshots = false;
    let flood = renderer
        .render(&site.url("/flood.html"), &opts)
        .await
        .unwrap();
    assert_eq!(flood.console.len(), 500);
    assert_eq!(flood.console_dropped, 20);
    opts.js_error_reporting = false;
    let disabled = renderer
        .render(&site.url("/flood.html"), &opts)
        .await
        .unwrap();
    assert!(disabled.console.is_empty());
    assert_eq!(disabled.console_dropped, 0);
    renderer.close().await;
}

#[tokio::test]
// Spec-Fixture `crawl-js` (paketeigen unter render-r01).
// spec: PERF-006
async fn requires_chromium_recycles_and_recovers_after_hanging_js() {
    let _lock = CHROMIUM.lock().await;
    let site = TestSite::start("render-r01").await.unwrap();
    // K2-82: Dieser Teil prüft den Neustart nach N Seiten und die Erholung nach Endlos-JS.
    // Der Standard-Speicherwächter (1500 MB) zog unter Last während der Endlosschleife die
    // Generation zurück (Summen-RSS ~1,6 GB, lokal unter 8-facher CPU-Last gemessen) und
    // verfälschte so den Generationenzähler. Der Speicherwächter wird unten separat geprüft.
    let renderer = requires_chromium(LaunchOpts {
        restart_after_pages: 3,
        max_tabs: 2,
        memory_limit_mb: 0,
        ..Default::default()
    })
    .await;
    for _ in 0..4 {
        renderer
            .render(&site.url("/csr.html"), &quick())
            .await
            .unwrap();
    }
    assert_eq!(renderer.generation(), 2);
    let opts = RenderOpts {
        response_timeout_ms: 300,
        page_timeout_ms: 900,
        ..quick()
    };
    let start = Instant::now();
    // K2-82: Kommt das Dokument auf einem langsamen Runner erst nach dem Antwort-Timeout an,
    // ist das Ergebnis ebenfalls ein Timeout (CRAWL-095), nicht mehr about:blank.
    let hung = renderer.render(&site.url("/hang.html"), &opts).await;
    assert!(
        matches!(hung, Err(RenderError::Timeout(_))),
        "{:?}",
        hung.map(|page| (page.final_url, page.load_timed_out, page.dom_html))
    );
    // K2-82: Obergrenze aus den Produktgrenzen statt 4 s: Kontextaufbau und -abbau sind
    // je auf 5 s begrenzt, dazu die Seitenzeitgrenze. Sie bleibt klar unter dem
    // Standard-Antwort-Timeout (20 s) und der festen CDP-Befehlsgrenze (30 s).
    assert!(
        start.elapsed() < Duration::from_millis(u64::from(opts.page_timeout_ms) + 10_000),
        "{:?}",
        start.elapsed()
    );
    assert!(renderer
        .render(&site.url("/csr.html"), &quick())
        .await
        .unwrap()
        .dom_html
        .contains("<title>rendered</title>"));
    assert_eq!(renderer.generation(), 2);
    renderer.close().await;
    let memory = requires_chromium(LaunchOpts {
        memory_limit_mb: 1,
        ..Default::default()
    })
    .await;
    tokio::time::timeout(Duration::from_secs(30), async {
        while memory.generation() == 1 {
            memory
                .render(&site.url("/csr.html"), &quick())
                .await
                .unwrap();
        }
    })
    .await
    .expect("Speichermonitor zieht die benutzte Generation zurück");
    assert_eq!(memory.generation(), 2);
    memory.close().await;
}

#[tokio::test]
// Spec-Fixture `crawl-js` (paketeigen unter render-r01).
// spec: CRAWL-122
async fn custom_truststore_and_authenticated_socks_fail_explicitly_until_n01() {
    let site = TestSite::start("render-r01").await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let cert = dir.path().join("ca.pem");
    std::fs::write(&cert, site.ca_cert_pem()).unwrap();
    let mut launch = LaunchOpts::default();
    launch
        .network
        .trusted_certificates
        .push(cert.to_string_lossy().into());
    assert!(
        matches!(Renderer::launch(launch).await, Err(RenderError::NotImplemented(message)) if message.contains("M2-N01") && message.contains("Truststore"))
    );
    let mut launch = LaunchOpts::default();
    launch.network.proxy.mode = os_model::ProxyMode::Manual;
    launch.network.proxy.kind = os_model::ProxyKind::Socks5;
    launch.proxy_credentials = Some(Credentials {
        username: "fixture".into(),
        password: "secret".into(),
    });
    assert!(
        matches!(Renderer::launch(launch).await, Err(RenderError::NotImplemented(message)) if message.contains("SOCKS5"))
    );
}

#[tokio::test]
// Spec-Fixture `crawl-js` (paketeigen unter render-r01).
// spec: CRAWL-122
async fn requires_chromium_rejects_untrusted_tls() {
    let _lock = CHROMIUM.lock().await;
    let site = TestSite::start("render-r01").await.unwrap();
    let renderer = requires_chromium(LaunchOpts::default()).await;
    assert!(renderer
        .render(&format!("{}/csr.html", site.https_base_url()), &quick())
        .await
        .is_err());
    renderer.close().await;
}

#[tokio::test]
// Spec-Fixture `crawl-js` (paketeigen unter render-r01).
// spec: CRAWL-078
async fn requires_chromium_uses_http_auth_credentials() {
    let _lock = CHROMIUM.lock().await;
    let site = TestSite::start("render-r01").await.unwrap();
    let renderer = requires_chromium(LaunchOpts::default()).await;
    let opts = RenderOpts {
        credentials: Some(Credentials {
            username: "fixture".into(),
            password: "password".into(),
        }),
        ..quick()
    };
    let result = renderer.render(&site.url("/auth"), &opts).await.unwrap();
    assert!(result.dom_html.contains("authenticated"));
    assert!(site.requests().iter().any(|r| r
        .header("authorization")
        .is_some_and(|v| v == "Basic Zml4dHVyZTpwYXNzd29yZA==")));
    renderer.close().await;
}

#[tokio::test]
// Spec-Fixture `crawl-js` (paketeigen unter render-r01).
// spec: PERF-006
async fn requires_chromium_limits_tabs_and_recovers_from_cancellation() {
    let _lock = CHROMIUM.lock().await;
    let site = TestSite::start("render-r01").await.unwrap();
    let renderer = Arc::new(
        requires_chromium(LaunchOpts {
            max_tabs: 2,
            restart_after_pages: 0,
            memory_limit_mb: 0,
            ..Default::default()
        })
        .await,
    );
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..4 {
        let renderer = renderer.clone();
        let url = site.url("/tab-limit");
        tasks.spawn(async move { renderer.render(&url, &quick()).await.unwrap() });
    }
    while let Some(result) = tasks.join_next().await {
        result.unwrap();
    }
    assert_eq!(site.max_concurrency(), 2);
    let r = renderer.clone();
    let url = site.url("/hang.html");
    let task = tokio::spawn(async move { r.render(&url, &quick()).await });
    // K2-82: Fehlerschranke, kein Zeitkriterium; gewartet wird auf den eingegangenen Request.
    tokio::time::timeout(Duration::from_secs(30), async {
        while !site
            .requests()
            .iter()
            .any(|r| r.path_and_query == "/hang.html")
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    task.abort();
    let _ = task.await;
    renderer
        .render(&site.url("/csr.html"), &quick())
        .await
        .unwrap();
    assert_eq!(renderer.generation(), 1);
    renderer.close().await;
}

#[tokio::test]
// Spec-Fixture `crawl-js` (paketeigen unter render-r01).
// spec: CRAWL-077 [P1]
async fn missing_environment_path_does_not_fall_back_to_system_chrome() {
    const CHILD: &str = "OPENSPIDER_R01_MISSING_PATH_CHILD";
    if std::env::var_os(CHILD).is_some() {
        let error = Renderer::launch(LaunchOpts::default()).await.unwrap_err();
        assert!(matches!(error, RenderError::ChromiumMissing { .. }));
        assert!(error.to_string().contains("err-chromium-missing"));
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "missing_environment_path_does_not_fall_back_to_system_chrome",
        ])
        .env(CHILD, "1")
        .env("OPENSPIDER_CHROMIUM", dir.path().join("absent-chrome"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[tokio::test]
// Spec-Fixture `crawl-js` (paketeigen unter render-r01).
// spec: CRAWL-082
async fn requires_chromium_screenshot_wait_captures_late_paint_pixels() {
    use base64::{prelude::BASE64_STANDARD, Engine};
    let _lock = CHROMIUM.lock().await;
    let site = TestSite::start("render-r01").await.unwrap();
    let renderer = requires_chromium(LaunchOpts::default()).await;
    let mut opts = RenderOpts {
        ajax_timeout_ms: 0,
        ..RenderOpts::fixed_viewport(800, 600)
    };
    opts.screenshot.size = Some((200, 200));
    // K2-82: Die Zeiten 200/300/0 ms gibt CRAWL-082 vor. Der 0-ms-Fall läuft nicht mehr als
    // erste Aufnahme des frisch gestarteten Browsers, damit einmalige Anlaufkosten der
    // Bildaufnahme (Software-Compositor ohne GPU auf CI-Runnern) nicht in das
    // 200-ms-Fenster fallen. Beide Fälle werden unverändert geprüft.
    for (wait, color) in [(300, "255,0,0,255"), (0, "0,0,255,255")] {
        opts.screenshot.resize_wait_ms = wait;
        let rendered = renderer
            .render(&site.url("/late-paint.html"), &opts)
            .await
            .unwrap();
        let png = BASE64_STANDARD.encode(&rendered.screenshot_tiles[0].data);
        // Decode with Chromium's PNG implementation and inspect the actual pixel,
        // not the DOM color or a screenshot byte inequality.
        let html=format!("<!doctype html><body><script>const img=new Image();img.onload=()=>{{const c=document.createElement('canvas');c.width=c.height=1;const ctx=c.getContext('2d');ctx.drawImage(img,0,0);document.body.dataset.pixel=Array.from(ctx.getImageData(0,0,1,1).data).join(',')}};img.src='data:image/png;base64,{png}';</script></body>");
        let readback = RenderOpts {
            main_response: Some(MainResponse {
                status: 200,
                headers: vec![("Content-Type".into(), "text/html".into())],
                body: html.into_bytes(),
            }),
            ..quick()
        };
        let page = renderer
            .render(&site.url("/pixel-readback"), &readback)
            .await
            .unwrap();
        assert!(
            page.dom_html.contains(&format!("data-pixel=\"{color}\"")),
            "Wartezeit {wait} ms, erwartet {color}, Screenshot-Phase {} ms: {}",
            rendered.timings.screenshot_ms,
            page.dom_html.chars().take(200).collect::<String>()
        );
    }
    renderer.close().await;
}

#[tokio::test]
#[ignore = "Benchmark: 100 pages with/without screenshots, run on a quiet machine"]
// Spec-Fixture `crawl-js` (paketeigen unter render-r01).
// spec: CRAWL-082
async fn requires_chromium_screenshot_overhead_100_pages() {
    let site = TestSite::start("render-r01").await.unwrap();
    let renderer = requires_chromium(LaunchOpts {
        restart_after_pages: 0,
        memory_limit_mb: 0,
        ..Default::default()
    })
    .await;
    let mut totals = [Duration::ZERO; 2];
    for (i, screenshots) in [false, true].into_iter().enumerate() {
        let opts = RenderOpts {
            screenshots,
            ajax_timeout_ms: 0,
            ..Default::default()
        };
        let start = Instant::now();
        for _ in 0..100 {
            renderer
                .render(&site.url("/csr.html"), &opts)
                .await
                .unwrap();
        }
        totals[i] = start.elapsed();
    }
    renderer.close().await;
    eprintln!(
        "100 pages: screenshots off={:?}, on={:?}, reduction={:.1}%",
        totals[0],
        totals[1],
        (1.0 - totals[0].as_secs_f64() / totals[1].as_secs_f64()) * 100.0
    );
    assert!(
        totals[0].as_secs_f64() <= totals[1].as_secs_f64() * 0.9,
        "without={:?}, with={:?}",
        totals[0],
        totals[1]
    );
}

#[tokio::test]
// Spec-Fixture `crawl-js`: fünf parallele Ressourcen mit Sitzungscookie und Set-Cookie.
// spec: CRAWL-078
async fn requires_chromium_preserves_cookies_for_parallel_resources() {
    let _lock = CHROMIUM.lock().await;
    let site = TestSite::start("render-r01").await.unwrap();
    let renderer = requires_chromium(LaunchOpts::default()).await;
    let opts = RenderOpts {
        ajax_timeout_ms: 1000,
        cookies: vec![os_render::api::Cookie {
            name: "session".into(),
            value: "fixture".into(),
            http_only: true,
            ..Default::default()
        }],
        ..quick()
    };
    let page = renderer
        .render(&site.url("/cookie-resources.html"), &opts)
        .await
        .unwrap();
    assert!(page.dom_html.contains("data-finished=\"yes\""));
    assert!(
        site.max_concurrency() >= 5,
        "Ressourcen werden parallel angefordert"
    );
    let requests = site.requests();
    for i in 0..5 {
        let request = requests
            .iter()
            .find(|r| r.path_and_query == format!("/parallel-{i}.js"))
            .unwrap();
        assert!(request
            .header("cookie")
            .unwrap()
            .contains("session=fixture"));
    }
    let echoed = requests
        .iter()
        .find(|r| r.path_and_query == "/echo")
        .unwrap()
        .header("cookie")
        .unwrap();
    for i in 0..5 {
        assert!(echoed.contains(&format!("resource{i}=present")));
    }
    renderer.close().await;
}

#[tokio::test]
// Spec-Fixture `crawl-js`: eingespieltes Hauptdokument bei normalisierter URL.
// spec: CRAWL-078
async fn requires_chromium_replays_normalized_main_url() {
    let _lock = CHROMIUM.lock().await;
    let site = TestSite::start("render-r01").await.unwrap();
    let renderer = requires_chromium(LaunchOpts {
        host_resolver_rules: vec![format!("MAP replay.fixture.test 127.0.0.1:{}", site.port())],
        ..Default::default()
    })
    .await;
    let opts = RenderOpts {
        main_response: Some(MainResponse {
            status: 200,
            headers: vec![("Content-Type".into(), "text/html".into())],
            body: b"<!doctype html><link rel='icon' href='data:,'><title>replayed</title>".to_vec(),
        }),
        ..quick()
    };
    for url in [
        "http://replay.fixture.test:80#fragment".to_owned(),
        site.base_url().to_owned(),
    ] {
        let page = renderer.render(&url, &opts).await.unwrap();
        assert!(page.dom_html.contains("<title>replayed</title>"));
    }
    assert!(site.requests().iter().all(|r| r.path_and_query != "/"));
    renderer.close().await;
}
