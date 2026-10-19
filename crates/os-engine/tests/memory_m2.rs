use os_engine::{
    memory::{self, MemoryAction, MemoryInputs, Sample},
    testing::{test_deps, SimFetcher, SimSite, TokioClock},
    CrawlRequest, CrawlSource,
};
use os_model::{settings::StorageSettings, CrawlConfig, CrawlState, MemoryStage};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
const MIB: u64 = 1 << 20;
struct Reset;
impl Drop for Reset {
    fn drop(&mut self) {
        memory::set_probe(None);
        memory::apply_settings(&StorageSettings::default());
    }
}

// spec: CRAWL-129, CRAWL-128, OUT-267
#[tokio::test(start_paused = true)]
async fn resource_stages_renderer_accounting_pause_resume_and_live_limits() {
    let _reset = Reset;
    let used = Arc::new(AtomicU64::new(100 * MIB));
    let limit = Arc::new(AtomicU64::new(300 * MIB));
    let available = Arc::new(AtomicU64::new(4 << 30));
    memory::set_probe(Some({
        let used = used.clone();
        let limit = limit.clone();
        let available = available.clone();
        Arc::new(move || Sample {
            engine_bytes: 100 * MIB,
            renderer_bytes: used.load(Ordering::SeqCst).saturating_sub(100 * MIB),
            physical_bytes: 8 << 30,
            available_bytes: available.load(Ordering::SeqCst),
            injected_limit: Some(limit.load(Ordering::SeqCst)),
        })
    }));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("memory.ospider");
    let fetcher = SimFetcher::new(SimSite::tree("site.test", 50).delay(Duration::from_millis(50)));
    let mut deps = test_deps();
    deps.clock = TokioClock::new();
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = 2;
    let handle = os_engine::start_crawl(
        fetcher.clone(),
        CrawlRequest {
            config: cfg.clone(),
            source: CrawlSource::Spider {
                start_url: "https://site.test/".into(),
            },
            file: path.clone(),
            project: "fixture".into(),
            name: "memory".into(),
        },
        deps.clone(),
    )
    .await
    .unwrap();
    used.store(240 * MIB, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(handle.progress().memory_stage, Some(MemoryStage::Warning));
    assert!(handle.progress().file_bytes.unwrap() > 0);
    assert_eq!(handle.progress().memory_used_bytes, Some(240 * MIB));
    // Live reduction; the engine alone remains below the critical threshold.
    limit.store(260 * MIB, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert_eq!(handle.wait().await.unwrap().state, CrawlState::Paused);
    let calls = fetcher.requests().len();
    let again = os_engine::resume_crawl(fetcher.clone(), path.clone(), None, deps.clone())
        .await
        .unwrap();
    assert_eq!(again.wait().await.unwrap().state, CrawlState::Paused);
    assert_eq!(
        fetcher.requests().len(),
        calls,
        "resume under pressure starts no request"
    );
    used.store(100 * MIB, Ordering::SeqCst);
    let resumed = os_engine::resume_crawl(fetcher, path.clone(), None, deps)
        .await
        .unwrap();
    available.store(1, Ordering::SeqCst);
    assert_eq!(
        memory::tick(&MemoryInputs {
            progress: &resumed.progress(),
            config: &cfg
        }),
        MemoryAction::Throttle(1)
    );
    available.store(4 << 30, Ordering::SeqCst);
    assert_eq!(resumed.wait().await.unwrap().state, CrawlState::Completed);
    // Analyse/Export/Vergleich laufen mit denselben pausierbaren Operation-Tokens.
    used.store(280 * MIB, Ordering::SeqCst);
    let token = os_model::CancelToken::new();
    let guard = memory::watch_operation(token.clone());
    tokio::task::yield_now().await;
    let (done, mut result) = tokio::sync::oneshot::channel();
    let worker_token = token.clone();
    let worker = std::thread::spawn(move || {
        let _ = done.send(worker_token.checkpoint());
    });
    tokio::task::yield_now().await;
    assert!(result.try_recv().is_err());
    assert!(!token.is_cancelled());
    used.store(100 * MIB, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert!(result.await.unwrap().is_ok());
    worker.join().unwrap();
    drop(guard);
    // Zehn Schutzpausen, jede Datei anschließend konsistent.
    for n in 0..10 {
        used.store(280 * MIB, Ordering::SeqCst);
        let stress_path = dir.path().join(format!("stress-{n}.ospider"));
        let handle = os_engine::start_crawl(
            SimFetcher::new(SimSite::tree("site.test", 5)),
            CrawlRequest {
                config: cfg.clone(),
                source: CrawlSource::Spider {
                    start_url: "https://site.test/".into(),
                },
                file: stress_path.clone(),
                project: "fixture".into(),
                name: "stress".into(),
            },
            test_deps(),
        )
        .await
        .unwrap();
        assert_eq!(handle.wait().await.unwrap().state, CrawlState::Paused);
        let file = os_store::CrawlFile::open(&stress_path, os_store::OpenMode::ReadOnly).unwrap();
        let check: String = file
            .conn()
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .unwrap();
        assert_eq!(check, "ok");
    }
    used.store(100 * MIB, Ordering::SeqCst);
    memory::apply_settings(&StorageSettings {
        cache_limit_mb: Some(256),
        ..Default::default()
    });
    let init = os_store::CrawlInit {
        crawl_id: test_deps()
            .ids
            .new_crawl_id(os_model::Timestamp::UNIX_EPOCH),
        mode: os_model::CrawlMode::Spider,
        start_url: Some("https://site.test/".into()),
        list_source: None,
        project: "fixture".into(),
        name: "cache".into(),
        config: cfg.clone(),
        created_at: os_model::Timestamp::UNIX_EPOCH,
    };
    let cache = os_store::CrawlFile::create(
        &dir.path().join("cache.ospider"),
        &init,
        os_store::StorageProfile::MaxSpeed,
    )
    .unwrap();
    assert_eq!(
        os_store::profile::read(cache.conn()).unwrap().cache_size,
        -262_144
    );
    memory::apply_settings(&StorageSettings {
        render_process_limit: Some(2),
        ..Default::default()
    });
    let fake = Arc::new(FakeRenderer::default());
    let mut deps = test_deps();
    deps.renderer = Some(fake.clone());
    let renderer = memory::resource_deps(deps).renderer.unwrap();
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..8 {
        let renderer = renderer.clone();
        tasks.spawn(async move {
            renderer
                .render("https://site.test/", &os_render::RenderOpts::default())
                .await
        });
    }
    while let Some(result) = tasks.join_next().await {
        result.unwrap().unwrap();
    }
    assert_eq!(fake.peak.load(Ordering::SeqCst), 2);
    let file = os_store::CrawlFile::open(&path, os_store::OpenMode::ReadOnly).unwrap();
    let check: String = file
        .conn()
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(check, "ok");
    assert_eq!(memory::stage(239 * MIB, 300 * MIB), MemoryStage::Normal);
    assert_eq!(memory::stage(270 * MIB, 300 * MIB), MemoryStage::Paused);
}

// spec: CRAWL-129, OUT-267
#[test]
fn physical_ram_defaults_and_hardware_bounds() {
    let settings = StorageSettings::default();
    assert_eq!(
        memory::limits(&settings, 16 << 30, 24),
        (8 << 30, 4 << 30, 8)
    );
    assert_eq!(
        memory::limits(&settings, 3 << 30, 1),
        (1 << 30, 768 << 20, 1)
    );
    let settings = StorageSettings {
        ram_limit_gb: Some(32.0),
        cache_limit_mb: Some(256),
        render_process_limit: Some(3),
        ..Default::default()
    };
    assert_eq!(
        memory::limits(&settings, 8 << 30, 24),
        (6 << 30, 256 << 20, 3)
    );
}

#[derive(Default)]
struct FakeRenderer {
    active: AtomicU64,
    peak: AtomicU64,
}
impl os_engine::render_glue::Renderer for FakeRenderer {
    fn render<'a>(
        &'a self,
        _: &'a str,
        _: &'a os_render::RenderOpts,
    ) -> futures_util::future::BoxFuture<'a, Result<os_render::RenderedPage, os_render::RenderError>>
    {
        Box::pin(async move {
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.peak.fetch_max(active, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(100)).await;
            self.active.fetch_sub(1, Ordering::SeqCst);
            Ok(os_render::RenderedPage::default())
        })
    }
}
