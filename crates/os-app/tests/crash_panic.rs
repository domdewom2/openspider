//! Panic in der App → genau ein Bericht `kind = panic`, geschrieben bevor der Prozess endet (OUT-246). Jeder Fall
//! läuft in einem eigenen Kindprozess (dieselbe Testdatei mit gesetzter Umgebungsvariable).
use os_app::{App, AppConfig};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const CHILD: &str = "OPENSPIDER_C01_PANIC_CHILD";
const HOME: &str = "OPENSPIDER_C01_PANIC_HOME";

fn child(test: &str, home: &Path, env: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(std::env::current_exe().unwrap());
    cmd.args(["--exact", test, "--nocapture", "--test-threads=1"])
        .env(CHILD, "1")
        .env(HOME, home)
        .env("OPENSPIDER_SECRET_STORE", "memory")
        .env_remove("OPENSPIDER_CRASH_TEST")
        .env_remove("OPENSPIDER_CRASH_SESSION");
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.output().unwrap()
}

fn in_child() -> Option<PathBuf> {
    std::env::var_os(CHILD).map(|_| PathBuf::from(std::env::var_os(HOME).unwrap()))
}

fn reports(home: &Path) -> Vec<(PathBuf, Value)> {
    let dir = home.join("logs").join("crashes");
    let mut out: Vec<(PathBuf, Value)> = std::fs::read_dir(&dir)
        .map(|d| {
            d.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with("crash-")
                })
                .map(|p| {
                    let v = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
                    (p, v)
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

// spec: OUT-246
// spec: OUT-240 [M3]
#[test]
fn panic_writes_exactly_one_report_before_the_process_ends() {
    if let Some(home) = in_child() {
        let app = App::new(AppConfig::new(&home)).unwrap();
        tracing::info!("child-run-marker visited https://crash-secret.test/?token=SEKRET-246");
        let worker = std::thread::Builder::new()
            .name("crash-worker".into())
            .spawn(|| {
                panic!("worker failed for ops-246@crash-secret.test at https://crash-secret.test/x?token=SEKRET-246")
            })
            .unwrap();
        let _ = worker.join();
        drop(app);
        // Prozessende direkt nach dem Panic: der Bericht muss schon auf der Platte liegen.
        std::process::exit(7);
    }
    let home = tempfile::tempdir().unwrap();
    let output = child(
        "panic_writes_exactly_one_report_before_the_process_ends",
        home.path(),
        &[],
    );
    assert_eq!(
        output.status.code(),
        Some(7),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let reports = reports(home.path());
    assert_eq!(reports.len(), 1, "genau ein Bericht");
    let (path, r) = &reports[0];
    assert!(path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .ends_with("-panic.json"));
    assert_eq!(r["kind"], "panic");
    assert_eq!(r["process"], "app");
    assert_eq!(r["format"], 1);
    assert_eq!(r["thread"], "crash-worker");
    let message = r["message"].as_str().unwrap();
    assert!(
        message.starts_with("worker failed for <email> at <url>"),
        "{message}"
    );
    let location = r["location"].as_str().unwrap();
    assert!(
        location.starts_with("os-app/tests/crash_panic.rs:"),
        "{location}"
    );
    let backtrace = r["backtrace"].as_array().unwrap();
    assert!(!backtrace.is_empty() && backtrace.len() <= 64);
    assert!(backtrace
        .iter()
        .all(|f| !f.as_str().unwrap().contains('/') || f.as_str().unwrap().contains("<url>")));
    let tail: Vec<&str> = r["log_tail"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l.as_str().unwrap())
        .collect();
    assert!(
        tail.iter()
            .any(|l| l.contains("child-run-marker visited <url>")),
        "{tail:?}"
    );
    let text = r.to_string();
    assert!(
        !text.contains("crash-secret.test") && !text.contains("SEKRET-246"),
        "{text}"
    );
    assert!(r["sent"].is_null() && r["server_ref"].is_null());
    // OUT-240: Der Backtrace-Eintrag im Log bleibt bestehen.
    let log = std::fs::read_to_string(home.path().join("logs/openspider.log")).unwrap();
    assert!(log.contains("ERROR panic:"), "{log}");
}

// spec: OUT-246
#[cfg(debug_assertions)]
#[test]
fn test_command_triggers_a_panic_report_in_debug_builds() {
    if let Some(home) = in_child() {
        let _app = App::new(AppConfig::new(&home)).unwrap();
        let dir = home.join("logs").join("crashes");
        for _ in 0..200 {
            if std::fs::read_dir(&dir).is_ok_and(|mut d| d.next().is_some()) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        std::process::exit(0);
    }
    let home = tempfile::tempdir().unwrap();
    let output = child(
        "test_command_triggers_a_panic_report_in_debug_builds",
        home.path(),
        &[("OPENSPIDER_CRASH_TEST", "panic")],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let reports = reports(home.path());
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].1["thread"], "crash-test");
    assert!(reports[0].1["message"]
        .as_str()
        .unwrap()
        .contains("OPENSPIDER_CRASH_TEST=panic"));
}

// spec: OUT-246
#[test]
fn write_failure_does_not_cause_a_second_panic() {
    if let Some(home) = in_child() {
        let app = App::new(AppConfig::new(&home)).unwrap();
        let _ = std::thread::spawn(|| panic!("panic with unwritable crash folder")).join();
        drop(app);
        std::process::exit(7);
    }
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join("logs")).unwrap();
    // `crashes` ist eine Datei: der Ordner lässt sich nicht anlegen (wie schreibgeschützt bzw. Platte voll).
    std::fs::write(home.path().join("logs/crashes"), "blocked").unwrap();
    let output = child(
        "write_failure_does_not_cause_a_second_panic",
        home.path(),
        &[],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(7),
        "kein Abbruch durch einen zweiten Panic: {stderr}"
    );
    assert!(
        !stderr.contains("panicked while processing panic"),
        "{stderr}"
    );
    assert_eq!(
        std::fs::read_to_string(home.path().join("logs/crashes")).unwrap(),
        "blocked"
    );
    let log = std::fs::read_to_string(home.path().join("logs/openspider.log")).unwrap();
    assert!(log.contains("panic with unwritable crash folder"));
}

/// Zählt Verbindungen und antwortet wie `/api/crash` (Netzwerkmitschnitt ohne Fremdhost).
fn counting_mock() -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/api/crash", listener.local_addr().unwrap());
    let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let seen = count.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let mut buf = vec![0u8; 128 * 1024];
            let mut read = 0;
            while let Ok(n) = stream.read(&mut buf[read..]) {
                read += n;
                let text = String::from_utf8_lossy(&buf[..read]);
                if let Some(head_end) = text.find("\r\n\r\n") {
                    let len = text[..head_end]
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                        })
                        .unwrap_or(0);
                    if read >= head_end + 4 + len {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            let body = r#"{"ok":true,"ref":"PANIC234"}"#;
            let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        }
    });
    (url, count)
}

// spec: OUT-246
// spec: OUT-247
#[test]
fn network_capture_over_panic_restart_and_hint_only_send_makes_a_request() {
    if let Some(home) = in_child() {
        let app = App::new(AppConfig::new(&home)).unwrap();
        os_app::crash::service::set_transport_for_tests(
            &app,
            &std::env::var("C01_MOCK").unwrap(),
            None,
        );
        let _ = std::thread::spawn(|| panic!("panic before restart")).join();
        std::process::exit(7);
    }
    let (url, connections) = counting_mock();
    let home = tempfile::tempdir().unwrap();
    let output = child(
        "network_capture_over_panic_restart_and_hint_only_send_makes_a_request",
        home.path(),
        &[("C01_MOCK", url.as_str())],
    );
    assert_eq!(output.status.code(), Some(7));
    // Neustart (im Elternprozess) und Öffnen des Hinweises.
    let rt = tokio::runtime::Runtime::new().unwrap();
    let app = App::new(AppConfig::new(home.path())).unwrap();
    os_app::crash::service::set_transport_for_tests(&app, &url, None);
    let ctx = || os_app::CallCtx::for_window("main");
    let overview = rt
        .block_on(app.dispatch(ctx(), "crash.overview", Value::Null))
        .unwrap();
    let pending = overview["pending"].as_array().unwrap().clone();
    assert_eq!(pending.len(), 1);
    rt.block_on(app.dispatch(ctx(), "crash.read", serde_json::json!({"id": pending[0]})))
        .unwrap();
    assert_eq!(
        connections.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "Panic, Neustart, Hinweis: keine Verbindung"
    );
    let result = rt
        .block_on(app.dispatch(
            ctx(),
            "crash.send",
            serde_json::json!({"ids": pending, "note": "", "email": ""}),
        ))
        .unwrap();
    assert_eq!(result["results"][0]["serverRef"], "PANIC234");
    assert_eq!(
        connections.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "genau eine Anfrage je Bericht"
    );
}
