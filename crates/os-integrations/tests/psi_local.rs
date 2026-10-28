//! M2-I04: Lokaler Lighthouse-Lauf (INTEG-132). Der Läufer wird gegen einen Ersatzprozess geprüft: das Testbinary
//! selbst startet mit dem Test `fake_lighthouse_entry` und verhält sich je nach Umgebungsvariable wie Lighthouse
//! (Bericht schreiben, hängen, abstürzen, Version ausgeben). Ein Lauf gegen eine echte Lighthouse-Installation ist bis
//! zur Bündelungsentscheidung (Klärung K2-09) zurückgestellt.
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use os_fetch::MemorySecretStore;
use os_integrations::psi::{
    lighthouse::{parse_lighthouse, parse_psi, Cell},
    local::{
        basic_auth_header, config_headers, max_parallel, parse_version, Job, Launcher,
        LocalOptions, LocalRunner, NodeLighthouse, RUNTIME_MISSING,
    },
    PsiSource,
};
use os_integrations::{ApiCtx, ApiError, ApiSource, UrlRef};
use os_model::{config::HeaderEntry, CrawlConfig, PsiDevice};

const MODE: &str = "OS_FAKE_LH_MODE";
const OUT: &str = "OS_FAKE_LH_OUT";
const URL_VAR: &str = "OS_FAKE_LH_URL";

/// Ersatz-Lighthouse; ohne Umgebungsvariable ein leerer Test.
#[test]
fn fake_lighthouse_entry() {
    let Ok(mode) = std::env::var(MODE) else {
        return;
    };
    let url = std::env::var(URL_VAR).unwrap_or_default();
    let out = std::env::var(OUT).map(PathBuf::from);
    let report = |lhr: serde_json::Value| {
        std::fs::write(out.clone().unwrap(), lhr.to_string()).unwrap();
    };
    match mode.as_str() {
        "version" => println!("13.0.0"),
        "hang" => std::thread::sleep(Duration::from_secs(120)),
        "crash" => std::process::exit(70),
        "slow" => {
            std::thread::sleep(Duration::from_millis(400));
            report(os_testsite::mocks::psi::lighthouse_fast(&url, false));
        }
        "errored" => {
            let mut lhr = os_testsite::mocks::psi::lighthouse_fast(&url, false);
            lhr["runtimeError"] = serde_json::json!({
                "code": "ERRORED_DOCUMENT_REQUEST",
                "message": "The page could not be loaded (status code 401)."
            });
            report(lhr);
        }
        _ => report(os_testsite::mocks::psi::lighthouse_heavy(
            &url,
            mode == "desktop",
        )),
    }
}

/// Startet das Testbinary als Lighthouse; der Modus hängt am letzten Pfadabschnitt der URL.
#[derive(Default)]
struct Fake {
    headers: Mutex<Vec<serde_json::Value>>,
}

impl Launcher for Fake {
    fn command(&self, job: &Job) -> Command {
        if let Some(file) = &job.headers_file {
            let raw = std::fs::read_to_string(file).unwrap();
            self.headers
                .lock()
                .unwrap()
                .push(serde_json::from_str(&raw).unwrap());
        }
        let mode = job.url.rsplit('/').next().unwrap_or("heavy").to_string();
        let mut cmd = Command::new(std::env::current_exe().unwrap());
        cmd.args([
            "fake_lighthouse_entry",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(MODE, mode)
        .env(OUT, &job.output)
        .env(URL_VAR, &job.url);
        cmd
    }

    fn version_command(&self) -> Command {
        let mut cmd = Command::new(std::env::current_exe().unwrap());
        cmd.args([
            "fake_lighthouse_entry",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(MODE, "version");
        cmd
    }
}

fn runner(parallel: u32, timeout: Duration) -> (Arc<Fake>, Arc<LocalRunner>) {
    let fake = Arc::new(Fake::default());
    let opts = LocalOptions {
        parallel,
        timeout,
        ..LocalOptions::default()
    };
    (fake.clone(), Arc::new(LocalRunner::new(fake, &opts)))
}

fn local_ctx() -> ApiCtx {
    let mut cfg = CrawlConfig::default();
    cfg.api.psi.enabled = true;
    cfg.api.psi.local = true;
    ApiCtx {
        cfg,
        secrets: Arc::new(MemorySecretStore::new()),
        network: Default::default(),
        allow_api_override: false,
        quota_ledger: None,
    }
}

fn text(out: &os_integrations::psi::lighthouse::Outcome, name: &str) -> String {
    match out.get(name) {
        Some(Cell::Text(t)) => t.clone(),
        other => panic!("{name}: {other:?}"),
    }
}

// spec: INTEG-132
#[tokio::test]
async fn without_runtime_local_cannot_connect() {
    let err = NodeLighthouse::discover(&LocalOptions::default()).unwrap_err();
    assert_eq!(err, ApiError::NotConnected(RUNTIME_MISSING.into()));
    assert_eq!(
        RUNTIME_MISSING,
        "Fehler: Lighthouse-Laufzeit nicht gefunden"
    );
    let missing = LocalOptions {
        lighthouse: Some(PathBuf::from("does-not-exist").join("cli").join("index.js")),
        ..LocalOptions::default()
    };
    assert!(LocalRunner::discover(&missing).is_err());
    // Die Quelle meldet denselben Status beim Verbinden und beim Abruf.
    let ctx = local_ctx();
    let source = PsiSource::new();
    let cred = os_integrations::credentials::Credential {
        account_id: "local".into(),
        label: String::new(),
        api_key: None,
        access_token: None,
        refresh_token: None,
        expires_at: None,
        client_secret: None,
        scopes: vec![],
    };
    assert_eq!(
        source.test_connection(&ctx, &cred).await.unwrap_err(),
        ApiError::NotConnected(RUNTIME_MISSING.into())
    );
    let page = UrlRef {
        url_id: None,
        url: "https://site.test/".into(),
    };
    assert!(source.fetch(&[page], &ctx).await.is_err());
}

// spec: INTEG-132
#[test]
fn dialog_allows_at_most_half_the_cores_and_validates_the_timeout() {
    assert_eq!(max_parallel(4), 2);
    assert_eq!(max_parallel(1), 1);
    assert_eq!(max_parallel(16), 8);
    let mut opts = LocalOptions::default();
    assert_eq!((opts.parallel, opts.timeout), (1, Duration::from_secs(90)));
    assert!(opts.validate(4).is_ok());
    opts.parallel = 3;
    assert!(opts.validate(4).is_err());
    opts.parallel = 2;
    opts.timeout = Duration::from_secs(20);
    assert!(opts.validate(4).is_err());
    opts.timeout = Duration::from_secs(600);
    assert!(opts.validate(4).is_ok());
}

// spec: INTEG-132
#[test]
fn command_line_uses_own_chrome_device_emulation_and_header_file() {
    let job = Job {
        url: "https://site.test/".into(),
        device: PsiDevice::Desktop,
        headers_file: Some(PathBuf::from("h.json")),
        output: PathBuf::from("r.json"),
    };
    let args: Vec<String> = NodeLighthouse::args(&job)
        .into_iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert_eq!(args[0], "https://site.test/");
    assert!(args.contains(&"--output=json".to_string()));
    assert!(args.contains(&"--output-path=r.json".to_string()));
    assert!(args.contains(&"--preset=desktop".to_string()));
    assert!(args.contains(&"--extra-headers=h.json".to_string()));
    assert!(args
        .iter()
        .any(|a| a.starts_with("--chrome-flags=--headless")));
    let mobile = Job {
        device: PsiDevice::Mobile,
        headers_file: None,
        ..job
    };
    let args: Vec<String> = NodeLighthouse::args(&mobile)
        .into_iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert!(args.contains(&"--form-factor=mobile".to_string()));
    assert!(!args.iter().any(|a| a.starts_with("--extra-headers")));
}

// spec: INTEG-132
#[tokio::test]
async fn parallelism_one_and_three_are_never_exceeded() {
    for parallel in [1u32, 3] {
        let (_, runner) = runner(parallel, Duration::from_secs(60));
        let sampler = {
            let runner = runner.clone();
            tokio::spawn(async move {
                let mut max = 0;
                loop {
                    max = max.max(runner.active());
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    if runner.mean_run().is_some() && runner.active() == 0 {
                        return max;
                    }
                }
            })
        };
        let jobs = (0..6).map(|i| {
            let runner = runner.clone();
            async move {
                runner
                    .run(
                        &format!("https://site.test/{i}/slow"),
                        PsiDevice::Mobile,
                        &[],
                    )
                    .await
            }
        });
        let results = futures_util::future::join_all(jobs).await;
        assert!(results.iter().all(|r| r.is_success()), "{results:?}");
        let sampled = sampler.await.unwrap();
        assert!(sampled <= parallel as usize, "{sampled} > {parallel}");
        assert!(runner.peak() <= parallel as usize);
        assert_eq!(
            runner.peak(),
            parallel as usize,
            "Parallelität wird genutzt"
        );
        assert_eq!(runner.active(), 0);
    }
}

// spec: INTEG-132
#[tokio::test]
async fn a_page_that_never_loads_times_out_and_the_next_url_runs_normally() {
    let (_, runner) = runner(1, Duration::from_secs(2));
    let out = runner
        .run("https://site.test/never-loads/hang", PsiDevice::Mobile, &[])
        .await;
    assert_eq!(text(&out, "PSI Status"), "Error");
    assert_eq!(text(&out, "PSI Error"), "Timeout");
    assert_eq!(runner.active(), 0, "Prozess des Laufs ist beendet");
    let next = runner
        .run("https://site.test/heavy", PsiDevice::Mobile, &[])
        .await;
    assert!(next.is_success());
}

// spec: INTEG-132
#[tokio::test]
async fn a_crashed_lighthouse_process_only_fails_its_own_url() {
    let (_, runner) = runner(2, Duration::from_secs(60));
    let (a, b, c) = tokio::join!(
        runner.run("https://site.test/a/heavy", PsiDevice::Mobile, &[]),
        runner.run("https://site.test/b/crash", PsiDevice::Mobile, &[]),
        runner.run("https://site.test/c/heavy", PsiDevice::Mobile, &[]),
    );
    assert!(a.is_success() && c.is_success());
    assert_eq!(text(&b, "PSI Status"), "Error");
    assert!(
        text(&b, "PSI Error").starts_with("LIGHTHOUSE_CRASHED"),
        "{b:?}"
    );
}

// spec: INTEG-132
#[tokio::test]
async fn lighthouse_version_is_shown_in_the_dialog_and_the_column() {
    let (_, runner) = runner(1, Duration::from_secs(60));
    let version = runner.version().await.unwrap();
    assert_eq!(version, "13.0.0");
    let out = runner
        .run("https://site.test/heavy", PsiDevice::Mobile, &[])
        .await;
    assert_eq!(text(&out, "Lighthouse Version"), version);
    let source = PsiSource::new().with_local(runner);
    let cred = os_integrations::credentials::Credential {
        account_id: "local".into(),
        label: String::new(),
        api_key: None,
        access_token: None,
        refresh_token: None,
        expires_at: None,
        client_secret: None,
        scopes: vec![],
    };
    let connected = source.test_connection(&local_ctx(), &cred).await.unwrap();
    assert_eq!(connected.label, "Lighthouse 13.0.0");
    assert_eq!(parse_version("Lighthouse v12.8.2\n"), Some("12.8.2".into()));
    assert_eq!(parse_version("running 1 test"), None);
}

// spec: INTEG-132
#[tokio::test]
async fn crawl_authentication_headers_are_passed_to_the_local_run() {
    let (fake, runner) = runner(1, Duration::from_secs(60));
    let mut ctx = local_ctx();
    ctx.cfg.http.headers = vec![
        HeaderEntry {
            name: "X-Staging".into(),
            value: "1".into(),
            host: None,
        },
        HeaderEntry {
            name: "X-Other-Host".into(),
            value: "nein".into(),
            host: Some("other.test".into()),
        },
    ];
    assert_eq!(
        config_headers(&ctx.cfg, "https://site.test/"),
        vec![("X-Staging".to_string(), "1".to_string())]
    );
    let source = PsiSource::new()
        .with_local(runner)
        .with_local_headers(vec![basic_auth_header("user", "pass")]);
    let values = source
        .fetch(
            &[UrlRef {
                url_id: Some(os_model::UrlId(4)),
                url: "https://site.test/auth/heavy".into(),
            }],
            &ctx,
        )
        .await
        .unwrap();
    assert!(values
        .iter()
        .any(|v| v.metric == "Performance Score" && v.v_real == Some(42.0)));
    let seen = fake.headers.lock().unwrap().clone();
    assert_eq!(
        seen,
        vec![serde_json::json!({"X-Staging": "1", "Authorization": "Basic dXNlcjpwYXNz"})]
    );
    // Ohne Zugangsdaten liefert die Seite einen Lighthouse-Fehler (Ersatzprozess: Modus `errored`).
    let (_, plain) = self::runner(1, Duration::from_secs(60));
    let out = plain
        .run("https://site.test/auth/errored", PsiDevice::Mobile, &[])
        .await;
    assert_eq!(text(&out, "PSI Status"), "Error");
    assert!(text(&out, "PSI Error").starts_with("ERRORED_DOCUMENT_REQUEST"));
}

// spec: INTEG-132
#[tokio::test]
#[ignore = "M3: Lauf gegen eine echte Lighthouse-Installation (OPENSPIDER_LIGHTHOUSE, Klärung K2-09), Bündelung in M3"]
async fn real_lighthouse_installation_scores_a_basic_auth_page() {
    let cli = std::env::var_os("OPENSPIDER_LIGHTHOUSE").expect("Pfad zur Lighthouse-CLI");
    let runner = LocalRunner::discover(&LocalOptions {
        lighthouse: Some(cli.into()),
        ..LocalOptions::default()
    })
    .unwrap();
    let url =
        std::env::var("OPENSPIDER_LIGHTHOUSE_AUTH_URL").expect("Fixture-URL hinter Basic Auth");
    let ok = runner
        .run(
            &url,
            PsiDevice::Mobile,
            &[basic_auth_header("user", "pass")],
        )
        .await;
    assert!(ok.get("Performance Score").is_some(), "{ok:?}");
    let denied = runner.run(&url, PsiDevice::Mobile, &[]).await;
    assert!(text(&denied, "PSI Error").starts_with("ERRORED_DOCUMENT_REQUEST"));
}

// spec: TECH-230
#[tokio::test]
async fn local_and_remote_yield_identical_values_for_the_same_lighthouse_result() {
    let (_, runner) = runner(1, Duration::from_secs(60));
    let url = "https://site.test/heavy";
    let local = runner.run(url, PsiDevice::Mobile, &[]).await;
    let remote = parse_psi(&os_testsite::mocks::psi::psi_heavy(url, false));
    let without_crux: Vec<_> = remote
        .cells
        .iter()
        .filter(|(n, _)| !n.contains("CrUX") && n != "Core Web Vitals Assessment")
        .cloned()
        .collect();
    assert_eq!(local.cells, without_crux);
    assert_eq!(
        local,
        parse_lighthouse(&os_testsite::mocks::psi::lighthouse_heavy(url, false))
    );
    // Lokal: keine CrUX-Spalten.
    assert!(!local.cells.iter().any(|(n, _)| n.contains("CrUX")));
}

// spec: INTEG-142
#[tokio::test]
async fn the_daily_limit_does_not_apply_to_the_local_source() {
    let (_, runner) = runner(1, Duration::from_secs(60));
    let mut ctx = local_ctx();
    ctx.cfg.api.psi.limits.quota_limit = Some(1);
    let source = PsiSource::new().with_local(runner);
    for i in 0..2 {
        let out = source
            .fetch_outcome(&format!("https://site.test/{i}/heavy"), &ctx)
            .await
            .unwrap();
        assert!(out.is_success(), "Lauf {i}: {out:?}");
    }
    assert_eq!(source.daily_used(), 0);
}
