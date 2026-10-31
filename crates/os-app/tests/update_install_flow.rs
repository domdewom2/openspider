use os_app::{
    commands::update::UpdateService,
    update::{
        self,
        download::{Progress, UpdateBackend},
        InstallWhen, UpdateManifest, UpdateState,
    },
    App, AppConfig,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
struct Backend {
    interrupted: bool,
    install_fails: bool,
    installs: Arc<AtomicUsize>,
}
impl UpdateBackend for Backend {
    fn download(
        &self,
        _: UpdateManifest,
        progress: Progress,
    ) -> futures_util::future::BoxFuture<'static, Result<Vec<u8>, String>> {
        let interrupted = self.interrupted;
        Box::pin(async move {
            progress(5, Some(10));
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
            if interrupted {
                Err("mock abort".into())
            } else {
                Ok(b"verified mock artifact".to_vec())
            }
        })
    }
    fn install(
        &self,
        _: UpdateManifest,
        bytes: Vec<u8>,
        _: bool,
    ) -> futures_util::future::BoxFuture<'static, Result<(), String>> {
        let installs = self.installs.clone();
        let install_fails = self.install_fails;
        Box::pin(async move {
            assert_eq!(bytes, b"verified mock artifact");
            installs.fetch_add(1, Ordering::SeqCst);
            if install_fails {
                Err("mock install failure".into())
            } else {
                Ok(())
            }
        })
    }
}
fn setup(interrupted: bool) -> (tempfile::TempDir, App, Arc<AtomicUsize>) {
    let temp = tempfile::tempdir().unwrap();
    let app = App::new(AppConfig::new(temp.path())).unwrap();
    let installs = Arc::new(AtomicUsize::new(0));
    update::download::set_backend(
        &app,
        Arc::new(Backend {
            interrupted,
            install_fails: false,
            installs: installs.clone(),
        }),
    );
    let manifest: UpdateManifest = serde_json::from_value(serde_json::json!({"version":"9.0.0","notes":"Fixture release notes", "pub_date":null,"platforms":{}})).unwrap();
    *app.ext::<UpdateService>().manifest.lock().unwrap() = Some(manifest);
    update::download::state(
        &app,
        UpdateState::Available(update::AvailableUpdate {
            version: "9.0.0".into(),
            notes: "Fixture release notes".into(),
            pub_date: None,
        }),
    );
    (temp, app, installs)
}
async fn finish(app: &App) -> UpdateState {
    for _ in 0..100 {
        let state = app.ext::<UpdateService>().state.lock().unwrap().clone();
        if !matches!(state, UpdateState::Downloading { .. }) {
            return state;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("download did not settle")
}
// spec: OUT-251
#[tokio::test]
async fn interrupted_mock_never_installs_and_restart_discards_bytes() {
    let (temp, app, installs) = setup(true);
    assert!(matches!(
        update::download::download(&app),
        UpdateState::Downloading { .. }
    ));
    assert!(matches!(finish(&app).await, UpdateState::Failed(_)));
    update::install::install(&app, InstallWhen::Now).await;
    assert_eq!(installs.load(Ordering::SeqCst), 0);
    drop(app);
    let restarted = App::new(AppConfig::new(temp.path())).unwrap();
    assert!(matches!(
        *restarted.ext::<UpdateService>().state.lock().unwrap(),
        UpdateState::Idle
    ));
    assert!(matches!(
        update::install::execute(&restarted, true).await,
        UpdateState::Idle
    ));
}
// spec: OUT-251
#[tokio::test]
async fn complete_download_on_quit_is_installed_only_once() {
    let (_temp, app, installs) = setup(false);
    update::download::download(&app);
    assert!(matches!(finish(&app).await, UpdateState::Ready(_)));
    update::install::install(&app, InstallWhen::OnQuit).await;
    assert_eq!(installs.load(Ordering::SeqCst), 0);
    os_app::quit::confirm(&app, false).await.unwrap();
    assert_eq!(installs.load(Ordering::SeqCst), 0);
    assert!(update::install::on_quit(&app).await);
    assert!(update::install::on_quit(&app).await);
    assert_eq!(installs.load(Ordering::SeqCst), 1);
}

#[path = "session_common.rs"]
mod common;
// spec: OUT-251
// spec: CRAWL-142 [M3]
#[tokio::test]
async fn download_keeps_crawl_running_and_paused_checkpoint_reopens_and_resumes() {
    let site = os_testsite::TestSite::start_dir(common::fixture(), 0)
        .await
        .unwrap();
    let (temp, app, installs) = setup(false);
    common::cfg()
        .save_default(&app.config().paths.settings)
        .unwrap();
    common::call(
        &app,
        "crawl.start",
        serde_json::to_value(common::request(site.url("/"))).unwrap(),
    )
    .await;
    let transitions = Arc::new(AtomicUsize::new(0));
    let observed = transitions.clone();
    app.crawl_events().subscribe(Box::new(move |_, event| {
        if matches!(
            event,
            os_app::runtime::lifecycle_bus::CrawlEvent::Paused(_)
                | os_app::runtime::lifecycle_bus::CrawlEvent::Resumed(_)
        ) {
            observed.fetch_add(1, Ordering::SeqCst);
        }
    }));
    update::download::download(&app);
    assert!(matches!(finish(&app).await, UpdateState::Ready(_)));
    assert_eq!(transitions.load(Ordering::SeqCst), 0);
    assert_eq!(
        app.sessions().current(&common::window()).unwrap().state,
        os_model::CrawlState::Running
    );
    update::install::install(&app, InstallWhen::Now).await;
    assert_eq!(installs.load(Ordering::SeqCst), 0);
    update::install::install(&app, InstallWhen::AfterCrawl).await;
    assert_eq!(installs.load(Ordering::SeqCst), 0);
    common::call(&app, "crawl.pause", serde_json::Value::Null).await;
    tokio::time::sleep(std::time::Duration::from_millis(600)).await;
    assert_eq!(
        installs.load(Ordering::SeqCst),
        0,
        "after crawl must not install on pause"
    );
    let path = app.sessions().current(&common::window()).unwrap().path;
    update::install::install(&app, InstallWhen::Now).await;
    assert_eq!(installs.load(Ordering::SeqCst), 1);
    // Release file owners before simulating the replacement process.
    drop(app);
    let restarted = App::new(AppConfig::new(temp.path())).unwrap();
    update::pending::reopen(&restarted).await;
    // Reopening canonicalizes (macOS /private/var, Windows \\?\ and 8.3 names): compare the same file.
    assert_eq!(
        std::fs::canonicalize(
            restarted
                .sessions()
                .current(&common::window())
                .unwrap()
                .path
        )
        .unwrap(),
        std::fs::canonicalize(&path).unwrap()
    );
    assert_eq!(
        restarted
            .sessions()
            .current(&common::window())
            .unwrap()
            .state,
        os_model::CrawlState::Paused
    );
    common::call(&restarted, "crawl.resume", serde_json::Value::Null).await;
    assert_eq!(
        restarted
            .sessions()
            .current(&common::window())
            .unwrap()
            .state,
        os_model::CrawlState::Running
    );
    common::call(&restarted, "crawl.pause", serde_json::Value::Null).await;
}
// spec: OUT-251
#[tokio::test]
async fn queued_after_crawl_installs_on_completion() {
    let (temp, app, installs) = setup(false);
    let path = temp.path().join("completed.ospider");
    drop(common::create(&path, common::cfg()));
    common::open(&app, &path).await;
    app.sessions()
        .update_state(&common::key(&app), os_model::CrawlState::Running);
    update::download::download(&app);
    finish(&app).await;
    update::install::install(&app, InstallWhen::AfterCrawl).await;
    assert_eq!(installs.load(Ordering::SeqCst), 0);
    app.sessions()
        .update_state(&common::key(&app), os_model::CrawlState::Completed);
    app.crawl_events().emit(
        &app,
        os_app::runtime::lifecycle_bus::CrawlEvent::Completed(common::key(&app)),
    );
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while installs.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(installs.load(Ordering::SeqCst), 1);
}

// spec: OUT-251
#[tokio::test]
async fn concurrent_completion_and_quit_callbacks_never_install_twice() {
    let (_temp, app, installs) = setup(false);
    update::download::download(&app);
    finish(&app).await;
    let (first, second) = tokio::join!(
        update::install::execute(&app, true),
        update::install::execute(&app, false)
    );
    assert!(matches!(first, UpdateState::Idle));
    assert!(matches!(second, UpdateState::Idle));
    assert_eq!(installs.load(Ordering::SeqCst), 1);
}

// spec: OUT-251
#[tokio::test]
async fn install_now_waits_for_active_writers_without_losing_the_request() {
    let (_temp, app, installs) = setup(false);
    update::download::download(&app);
    finish(&app).await;
    let (done, wait) = tokio::sync::oneshot::channel::<()>();
    let operation = app.ops().spawn("op-export", false, |_, _| {
        Box::pin(async move {
            wait.await.unwrap();
            Ok(serde_json::Value::Null)
        })
    });
    assert!(matches!(
        update::install::install(&app, InstallWhen::Now).await,
        UpdateState::Ready(_)
    ));
    assert_eq!(installs.load(Ordering::SeqCst), 0);
    done.send(()).unwrap();
    app.ops().wait(&operation.op_id).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while installs.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(installs.load(Ordering::SeqCst), 1);
}

// spec: OUT-251
#[tokio::test]
async fn install_error_does_not_block_quit_or_leave_reopen_ticket() {
    let (temp, app, installs) = setup(false);
    let path = temp.path().join("failed-update.ospider");
    drop(common::create(&path, common::cfg()));
    common::open(&app, &path).await;
    app.sessions()
        .update_state(&common::key(&app), os_model::CrawlState::Paused);
    update::download::set_backend(
        &app,
        Arc::new(Backend {
            interrupted: false,
            install_fails: true,
            installs: installs.clone(),
        }),
    );
    update::download::download(&app);
    assert!(matches!(finish(&app).await, UpdateState::Ready(_)));
    update::install::install(&app, InstallWhen::OnQuit).await;
    assert!(
        update::install::on_quit(&app).await,
        "Install-Fehler blockiert Quit nicht"
    );
    assert!(app
        .ext::<update::pending::Pending>()
        .when
        .lock()
        .unwrap()
        .is_none());
    assert_eq!(installs.load(Ordering::SeqCst), 1);
    let state = app.ext::<UpdateService>().state.lock().unwrap().clone();
    assert!(matches!(state, UpdateState::Failed(_)), "{state:?}");
    assert!(!app
        .config()
        .paths
        .settings
        .join("update-reopen.json")
        .exists());
    assert!(update::install::on_quit(&app).await);
    assert_eq!(installs.load(Ordering::SeqCst), 1);
    drop(app);
    let restarted = App::new(AppConfig::new(temp.path())).unwrap();
    update::pending::reopen(&restarted).await;
    assert!(restarted.sessions().current(&common::window()).is_none());
}
