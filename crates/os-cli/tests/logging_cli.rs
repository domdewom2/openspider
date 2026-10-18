mod common;
use common::{site, Env};
use std::fs;
fn log_text(env: &Env) -> String {
    fs::read_dir(env.home.path().join("logs"))
        .unwrap()
        .map(|e| fs::read_to_string(e.unwrap().path()).unwrap())
        .collect()
}
// spec: OUT-240
// spec: OUT-241
// spec: OUT-243
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn crawl_writes_structured_start_end_and_fetch_status_only_at_debug() {
    let site = site("out-basic-site").await;
    for level in ["info", "debug"] {
        let env = Env::new();
        let run = env
            .run_async(vec![
                "--crawl".into(),
                site.base_url(),
                "--set".into(),
                "limits.max_urls=2".into(),
                "--log-format".into(),
                "json".into(),
                "--log-level".into(),
                level.into(),
            ])
            .await;
        assert_eq!(run.code, 0, "{}", run.dump());
        let logs = log_text(&env);
        assert!(logs.contains("Crawl gestartet"));
        assert!(logs.contains("Crawl beendet"));
        assert!(logs.contains("Crawl-Konfiguration Start"));
        assert!(logs.contains("Crawl-Konfiguration Ende"));
        assert!(logs.contains("rendering"));
        assert!(logs.contains("limits"));
        assert_eq!(logs.contains("Abruf abgeschlossen"), level == "debug");
        for line in logs.lines() {
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            assert_eq!(v["timestamp"].as_str().unwrap().len(), 29);
        }
        if level == "debug" {
            assert!(logs.contains("response_time_ms"));
            assert!(logs.contains("status"));
        }
    }
}
// spec: OUT-241
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn env_module_filter_only_increases_fetch_level() {
    let site = site("out-basic-site").await;
    let env = Env::new();
    let mut cmd = env.command();
    cmd.env("OPENSPIDER_LOG", "os_fetch=trace").args([
        "--crawl",
        &site.base_url(),
        "--set",
        "limits.max_urls=1",
    ]);
    let output = tokio::task::spawn_blocking(move || cmd.output().unwrap())
        .await
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let log = log_text(&env);
    assert!(log.contains("Crawl gestartet"));
    assert!(log.contains("Abruf abgeschlossen"));
    assert!(!log.contains("DEBUG os_engine"));
}

// spec: OUT-240
// spec: OUT-243
#[test]
fn panic_writes_a_redacted_backtrace_before_exit() {
    const CHILD: &str = "OPENSPIDER_O01_PANIC_CHILD";
    if std::env::var_os(CHILD).is_some() {
        os_cli::logging::init(&os_cli::RunArgs::default());
        panic!("password=PANIC-FIXTURE");
    }
    let home = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "panic_writes_a_redacted_backtrace_before_exit"])
        .env(CHILD, "1")
        .env("OPENSPIDER_HOME", home.path())
        .env("OPENSPIDER_SECRET_STORE", "memory")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let raw: String = fs::read_dir(home.path().join("logs"))
        .unwrap()
        .map(|e| fs::read_to_string(e.unwrap().path()).unwrap())
        .collect();
    assert!(raw.contains("ERROR panic:"));
    assert!(raw.contains("panic_writes_a_redacted_backtrace_before_exit"));
    assert!(!raw.contains("PANIC-FIXTURE"));
    assert_eq!(raw.lines().count(), 1);
}

// spec: OUT-241
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_requests_also_record_the_status_and_duration() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    // The fixture accepts and immediately closes one connection without sending HTTP bytes.
    let accept = tokio::task::spawn_blocking(move || {
        let (stream, _) = listener.accept().unwrap();
        drop(stream);
    });
    let env = Env::new();
    let _run = env
        .run_async(vec![
            "--crawl".into(),
            format!("http://{address}/"),
            "--log-level".into(),
            "debug".into(),
            "--set".into(),
            "robots.retries=0".into(),
        ])
        .await;
    accept.await.unwrap();
    let logs = log_text(&env);
    assert!(logs.contains("No Response"));
    assert!(logs.contains("response_time_ms"));
}
