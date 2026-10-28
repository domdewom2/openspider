use super::*;
use crate::{LaunchOpts, RenderOpts, Renderer};
use os_model::{CustomJsConfig, JsSnippet, JsSnippetKind};
use os_testsite::TestSite;
use std::time::Duration;

fn config() -> CrawlConfig {
    let mut cfg = CrawlConfig::default();
    cfg.rendering.mode = RenderingMode::JavaScript;
    cfg.custom_js = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/integ-custom-js/extraction.json"
    ))
    .unwrap();
    cfg
}
fn options(cfg: &CrawlConfig) -> RenderOpts {
    RenderOpts {
        snippets: RunOptions::for_response(cfg, true, 200, "text/html"),
        ajax_timeout_ms: 100,
        screenshots: false,
        ..Default::default()
    }
}

// spec: INTEG-072
#[test]
fn output_directory_is_optional_and_propagated_to_render_options() {
    let mut cfg = config();
    assert!(options(&cfg).snippets.output_dir.is_none());
    let directory = tempfile::tempdir().unwrap();
    cfg.custom_js.output_dir = Some(directory.path().to_path_buf());
    assert_eq!(
        options(&cfg).snippets.output_dir.as_deref(),
        Some(directory.path())
    );
}

// spec: INTEG-060
// spec: INTEG-061
// spec: INTEG-062
// spec: INTEG-072
#[test]
fn management_is_transactional_and_imports_require_preview_and_confirmation() {
    let cfg = config();
    let mut manager = SnippetManager::new(cfg.custom_js.clone()).unwrap();
    let original = manager.config().clone();
    let import = PendingImport::new(JsSnippet {
        name: "import".into(),
        code: "window.importRan=true".into(),
        ..Default::default()
    });
    assert_eq!(import.preview().code, "window.importRan=true");
    assert_eq!(manager.config(), &original); // An unconfirmed import is not runnable.
    import.confirm(&mut manager).unwrap();
    assert_eq!(manager.config().snippets.len(), original.snippets.len() + 1);
    manager.move_to(0, 2).unwrap();
    assert_eq!(manager.config().snippets[2].name, "count");
    manager.remove(2).unwrap();
    assert!(manager
        .replace(
            0,
            JsSnippet {
                kind: JsSnippetKind::Action,
                timeout_s: 0,
                ..Default::default()
            }
        )
        .is_err());
    assert!(manager
        .add(JsSnippet {
            name: "LIST".into(),
            ..Default::default()
        })
        .is_err());
    let mut editor = SnippetManager::default();
    editor
        .add_code(
            "default timeout".into(),
            JsSnippetKind::Extraction,
            "return 1".into(),
            None,
        )
        .unwrap();
    assert_eq!(editor.config().snippets[0].timeout_s, 0);
    assert!(editor
        .add_code(
            "missing timeout".into(),
            JsSnippetKind::Action,
            String::new(),
            None
        )
        .is_err());
    let mut manager = SnippetManager::default();
    for i in 0..100 {
        manager
            .add(JsSnippet {
                name: format!("s{i}"),
                ..Default::default()
            })
            .unwrap();
    }
    assert_eq!(
        manager.add(JsSnippet::default()).unwrap_err(),
        "Maximal 100 Snippets"
    );
    assert_eq!(manager.config().snippets.len(), 100);
    assert_eq!(snippet_name(&JsSnippet::default(), 5), "Snippet 6");
    let manager = SnippetManager::new(CustomJsConfig {
        snippets: vec![
            JsSnippet::default(),
            JsSnippet {
                name: "Snippet 1".into(),
                ..Default::default()
            },
        ],
        ..Default::default()
    });
    assert!(manager.is_err());
    assert_eq!(network_warning(&cfg).len(), 1);
    assert_eq!(JsSnippet::default().timeout_s, 0);
    let missing_action: JsSnippet =
        serde_json::from_str(r#"{"kind":"action","code":"window.scrollTo(0,100)"}"#).unwrap();
    assert!(SnippetManager::new(CustomJsConfig {
        snippets: vec![missing_action],
        ..Default::default()
    })
    .is_err());
}

// spec: INTEG-060
#[test]
fn response_gate_handles_scope_status_mode_pdf_and_comma_separated_mime() {
    let mut cfg = config();
    cfg.custom_js.snippets.truncate(1);
    cfg.custom_js.snippets[0].content_types = vec!["image/png, TEXT/HTML".into()];
    for (internal, status, mime, expected) in [
        (true, 200, "text/html; charset=UTF-8", 1),
        (true, 200, "image/png", 1),
        (true, 201, "text/html", 0),
        (true, 404, "text/html", 0),
        (false, 200, "text/html", 0),
        (true, 200, "application/pdf", 0),
        (true, 200, "text/plain", 0),
    ] {
        assert_eq!(
            RunOptions::for_response(&cfg, internal, status, mime)
                .snippets
                .len(),
            expected
        );
    }
    cfg.custom_js.snippets[0].content_types = vec!["*".into()];
    assert_eq!(
        RunOptions::for_response(&cfg, true, 200, "text/plain")
            .snippets
            .len(),
        1
    );
    assert!(
        RunOptions::for_response(&cfg, true, 200, "APPLICATION/PDF; x=y")
            .snippets
            .is_empty()
    );
    cfg.rendering.mode = RenderingMode::Text;
    assert!(RunOptions::for_response(&cfg, true, 200, "text/html")
        .snippets
        .is_empty());
    assert!(cfg
        .validate()
        .warnings
        .iter()
        .any(|i| i.path == "custom_js.snippets"));
}

// spec: INTEG-060
// spec: INTEG-061
// spec: INTEG-063
// spec: INTEG-072
#[tokio::test]
async fn requires_chromium_extracts_promises_errors_aliases_and_recovers_after_infinite_loop() {
    let site = TestSite::start("integ-custom-js").await.unwrap();
    let renderer = Renderer::launch(LaunchOpts::default()).await.unwrap();
    let cfg = config();
    let opts = options(&cfg);
    let page = renderer
        .render(&site.url("/page.html"), &opts)
        .await
        .unwrap();
    let results = &page.snippets;
    assert_eq!(results.len(), 15);
    assert_eq!(results[0].values, [Some("3".into())]);
    assert_eq!(
        results[1].values,
        [
            Some("Alpha".into()),
            Some("Beta".into()),
            Some("Gamma".into())
        ]
    );
    assert_eq!(results[2].values, [Some("later".into())]);
    for (i, e) in [
        (3, "Snippet returned no value"),
        (4, "Timeout"),
        (5, "fixture failure"),
        (7, "kein Preis"),
        (13, "File access is not enabled"),
    ] {
        assert_eq!(results[i].error.as_deref(), Some(e));
    }
    assert_eq!(
        results[6].values,
        [Some("count".into()), Some("third".into())]
    );
    for i in [8, 9, 10] {
        assert!(results[i].error.is_none());
    }
    assert_eq!(results[8].values, [None]);
    assert_eq!(results[9].values, [None]);
    assert!(results[10].values.is_empty());
    assert_eq!(results[11].values, [Some("{\"price\":42}".into())]);
    assert_eq!(
        results[12].values,
        [
            Some("undefined".into()),
            Some("undefined".into()),
            Some("undefined".into()),
            Some("true".into())
        ]
    );
    assert_eq!(results[14].values, [Some("true".into())]);
    assert!(site.requests().iter().any(|r| r.path_and_query == "/api"));
    assert!(!site
        .requests()
        .iter()
        .any(|r| r.path_and_query == "/passive.png"));
    assert!(results
        .windows(2)
        .all(|w| w[0].finished_us < w[1].started_us));
    assert_eq!(
        writes(UrlId(1), results)
            .iter()
            .filter(|w| w.def_index == 10)
            .count(),
        1
    );
    let mut cfg = cfg;
    cfg.custom_js.snippets.truncate(1);
    cfg.custom_js.snippets[0].code =
        "return openspider.data(document.querySelectorAll('h2').length);".into();
    let next = renderer
        .render(&site.url("/charset"), &options(&cfg))
        .await
        .unwrap();
    assert_eq!(next.snippets[0].values, [Some("1".into())]);
    renderer.close().await;
}

// spec: INTEG-060
// spec: INTEG-062
#[tokio::test]
async fn requires_chromium_actions_precede_extraction_and_wait_for_the_maximum_timeout() {
    let site = TestSite::start("integ-custom-js").await.unwrap();
    let renderer = Renderer::launch(LaunchOpts::default()).await.unwrap();
    let mut cfg = config();
    cfg.custom_js.snippets = vec![JsSnippet { name: "extract".into(), code: "return openspider.data([document.querySelector('#tooltip').hidden, document.querySelectorAll('a').length, window.actionData]);".into(), ..Default::default() },
        JsSnippet { kind: JsSnippetKind::Action, timeout_s:3, code: "window.dispatchEvent(new Event('scroll')); document.querySelector('#hover').dispatchEvent(new Event('mouseover')); window.actionData=(await (await fetch('/api')).json()).ok;".into(), ..Default::default() },
        JsSnippet { kind: JsSnippetKind::Action, timeout_s:8, code: "await new Promise(resolve => { document.body.addEventListener('action-ready', resolve, {once:true}); requestAnimationFrame(() => { document.body.dataset.delayed='yes'; document.body.dispatchEvent(new Event('action-ready')); }); });".into(), ..Default::default() }];
    let page = renderer
        .render(&site.url("/page.html"), &options(&cfg))
        .await
        .unwrap();
    assert_eq!(
        page.snippets
            .iter()
            .map(|r| r.def_index)
            .collect::<Vec<_>>(),
        [1, 2, 0]
    );
    assert!(page.snippets[..2]
        .iter()
        .all(|r| r.error.is_none() && r.values.is_empty()));
    assert_eq!(
        page.snippets[2].values,
        [Some("false".into()), Some("3".into()), Some("true".into())]
    );
    assert!(page.dom_html.contains("href=\"/p2\""));
    assert!(page.dom_html.contains("href=\"/p3\""));
    assert!(page.dom_html.contains("data-delayed=\"yes\""));
    assert_eq!(writes(UrlId(1), &page.snippets).len(), 3);
    let plain = renderer
        .render(
            &site.url("/page.html"),
            &RenderOpts {
                ajax_timeout_ms: 100,
                screenshots: false,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(plain.dom_html.matches("id=\"p2\"").count(), 1); // Only the fixture script literal.
    renderer.close().await;
}

// spec: INTEG-061
#[tokio::test]
async fn requires_chromium_inherited_timeout_stops_a_pending_promise() {
    let site = TestSite::start("integ-custom-js").await.unwrap();
    let renderer = Renderer::launch(LaunchOpts::default()).await.unwrap();
    let mut cfg = config();
    cfg.rendering.page_timeout_s = 1;
    let mut manager = SnippetManager::default();
    manager
        .add_code(
            "pending".into(),
            JsSnippetKind::Extraction,
            "await new Promise(() => {});".into(),
            None,
        )
        .unwrap();
    manager
        .add_code(
            "after".into(),
            JsSnippetKind::Extraction,
            "return openspider.data('still running');".into(),
            None,
        )
        .unwrap();
    cfg.custom_js = manager.config().clone();
    let page = renderer
        .render(&site.url("/page.html"), &options(&cfg))
        .await
        .unwrap();
    assert_eq!(page.snippets[0].error.as_deref(), Some("Timeout"));
    assert!(page.snippets[0].finished_us >= 1_000_000);
    assert_eq!(page.snippets[1].values, [Some("still running".into())]);
    renderer.close().await;
}

// spec: INTEG-061
// spec: INTEG-072
#[tokio::test]
async fn requires_chromium_parallel_pages_do_not_share_resource_freezes() {
    let site = TestSite::start("integ-custom-js").await.unwrap();
    let renderer = std::sync::Arc::new(
        Renderer::launch(LaunchOpts {
            max_tabs: 2,
            ..Default::default()
        })
        .await
        .unwrap(),
    );
    let mut cfg = config();
    cfg.custom_js.snippets = vec![JsSnippet {
        code: "if (location.pathname === '/page.html') { await fetch('/api?marker'); await new Promise(r=>setTimeout(r,800)); } return openspider.data(document.querySelectorAll('h2').length);".into(),
        ..Default::default()
    }];
    let opts = options(&cfg);
    let first = {
        let renderer = renderer.clone();
        let opts = opts.clone();
        let url = site.url("/page.html");
        tokio::spawn(async move { renderer.render(&url, &opts).await.unwrap() })
    };
    tokio::time::timeout(Duration::from_secs(10), async {
        while !site
            .requests()
            .iter()
            .any(|r| r.path_and_query == "/api?marker")
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let second = tokio::time::timeout(
        Duration::from_secs(10),
        renderer.render(&site.url("/overlap"), &opts),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(second.snippets[0].values, [Some("1".into())]);
    assert_eq!(first.await.unwrap().snippets[0].values, [Some("3".into())]);
    renderer.close().await;
}

// spec: INTEG-060
// spec: INTEG-061
// spec: INTEG-062
#[tokio::test]
async fn injected_clock_drives_shared_budget_timeouts_and_persisted_trace() {
    use super::{clock::Clock, runtime::SnippetPage};
    use os_model::{ManualClock, Timestamp};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    };
    struct TestClock {
        time: ManualClock,
        waits: Mutex<Vec<Duration>>,
    }
    impl Clock for TestClock {
        fn now(&self) -> Duration {
            os_model::Clock::monotonic(&self.time)
        }
        async fn sleep(&self, duration: Duration) {
            self.waits.lock().unwrap().push(duration);
            self.time.advance(duration);
        }
    }
    struct TestPage<'a> {
        clock: &'a TestClock,
        calls: AtomicUsize,
        terminated: AtomicUsize,
        stopped: Mutex<Vec<Duration>>,
        pending_actions: bool,
    }
    impl SnippetPage for TestPage<'_> {
        fn target(&self) -> String {
            "fixture-page".into()
        }
        async fn stop_loading(&self) -> Result<(), crate::RenderError> {
            self.stopped.lock().unwrap().push(self.clock.now());
            Ok(())
        }
        async fn terminate(&self) -> Result<(), crate::RenderError> {
            self.terminated.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        async fn evaluate(&self, source: String) -> Result<serde_json::Value, String> {
            let (marker, seconds) = match self.calls.fetch_add(1, Ordering::SeqCst) {
                0 => ("first", 1),
                1 => ("second", 2),
                2 => {
                    assert!(source.contains("pending"));
                    return std::future::pending().await;
                }
                3 => ("after", 1),
                _ => panic!("unexpected evaluation"),
            };
            assert!(source.contains(marker));
            if self.pending_actions && marker != "after" {
                return std::future::pending().await;
            }
            self.clock.time.advance(Duration::from_secs(seconds));
            Ok(serde_json::json!({"values":[marker], "error":null}))
        }
    }
    let mut cfg = config();
    cfg.custom_js = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/integ-custom-js/timing.json"
    ))
    .unwrap();
    cfg.rendering.page_timeout_s = 4;
    let opts = options(&cfg).snippets;
    assert_eq!(opts.page_budget_ms(1000), 15_000); // 1 + max(3,8) + 4 + 2.
    let clock = TestClock {
        time: ManualClock::new(Timestamp::UNIX_EPOCH),
        waits: Mutex::default(),
    };
    let page = TestPage {
        clock: &clock,
        calls: AtomicUsize::new(0),
        terminated: AtomicUsize::new(0),
        stopped: Mutex::default(),
        pending_actions: false,
    };
    let results = runtime::execute(&page, &opts, &clock).await.unwrap();
    assert_eq!(
        results
            .iter()
            .map(|r| (r.def_index, r.started_us, r.finished_us))
            .collect::<Vec<_>>(),
        [
            (1, 0, 1_000_000),
            (2, 1_000_001, 3_000_000),
            (0, 8_000_000, 12_000_000),
            (3, 12_000_001, 13_000_000),
        ]
    );
    assert_eq!(
        *clock.waits.lock().unwrap(),
        [Duration::from_secs(5), Duration::from_secs(4)]
    );
    assert_eq!(*page.stopped.lock().unwrap(), [Duration::from_secs(8)]);
    assert_eq!(page.terminated.load(Ordering::SeqCst), 1);
    assert_eq!(results[2].error.as_deref(), Some("Timeout"));
    assert_eq!(results[3].values, [Some("after".into())]);
    let trace = trace(UrlId(1), &results);
    assert_eq!(trace[0].value, Value::Int(4));
    assert_eq!(trace[1].value, Value::Text("[[1,true,0,1000000,null],[2,true,1000001,3000000,null],[0,false,8000000,12000000,\"Timeout\"],[3,false,12000001,13000000,null]]".into()));
    assert!(!opts.blocks_resource("fixture-page", "Image"));

    // Hanging actions share the same deadline: first 3 s, then only the
    // remaining 5 s of the second action's 8 s window, never another 8 s.
    let clock = TestClock {
        time: ManualClock::new(Timestamp::UNIX_EPOCH),
        waits: Mutex::default(),
    };
    let page = TestPage {
        clock: &clock,
        calls: AtomicUsize::new(0),
        terminated: AtomicUsize::new(0),
        stopped: Mutex::default(),
        pending_actions: true,
    };
    let results = runtime::execute(&page, &opts, &clock).await.unwrap();
    assert_eq!(results[0].finished_us, 3_000_000);
    assert_eq!(results[1].finished_us, 8_000_000);
    assert_eq!(results[2].finished_us, 12_000_000);
    assert!(results[..3]
        .iter()
        .all(|r| r.error.as_deref() == Some("Timeout")));
    assert_eq!(page.terminated.load(Ordering::SeqCst), 3);
    assert_eq!(
        *clock.waits.lock().unwrap(),
        [
            Duration::from_secs(3),
            Duration::from_secs(5),
            Duration::ZERO,
            Duration::from_secs(4)
        ]
    );
    assert_eq!(*page.stopped.lock().unwrap(), [Duration::from_secs(8)]);
    assert_eq!(results[3].values, [Some("after".into())]);
}
