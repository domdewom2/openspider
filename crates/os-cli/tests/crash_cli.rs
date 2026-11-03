//! Absturzberichte der CLI (OUT-246): Panic → Bericht `process = cli`, Pfad auf stderr; Datenschutz-Test mit
//! Geheimnissen, Hostname, E-Mail-Adresse und `OPENSPIDER_HOME`. Die CLI zeigt keinen Dialog und sendet nie.
mod common;
use common::{resolve, site, Env};
use serde_json::Value;
use std::path::PathBuf;

fn reports(env: &Env) -> Vec<(PathBuf, Value)> {
    let dir = env.home.path().join("logs").join("crashes");
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
                    let text = std::fs::read_to_string(&p).unwrap();
                    (p, serde_json::from_str(&text).unwrap())
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

// spec: OUT-246
#[cfg(debug_assertions)]
#[test]
fn cli_panic_writes_the_same_report_and_prints_its_path() {
    let env = Env::new();
    let mut cmd = env.command();
    cmd.env("OPENSPIDER_CRASH_TEST", "panic")
        .args(["--crawl", "http://127.0.0.1:9/"]);
    let output = cmd.output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    let reports = reports(&env);
    assert_eq!(reports.len(), 1, "{stderr}");
    let (path, r) = &reports[0];
    let line = stderr
        .lines()
        .find_map(|l| l.strip_prefix("Absturzbericht gespeichert: "))
        .expect("Pfad auf stderr");
    assert_eq!(
        std::fs::canonicalize(line).unwrap(),
        std::fs::canonicalize(path).unwrap()
    );
    assert_eq!(r["kind"], "panic");
    assert_eq!(r["process"], "cli");
    assert_eq!(r["crawl_running"], true);
    assert!(r["message"]
        .as_str()
        .unwrap()
        .contains("OPENSPIDER_CRASH_TEST=panic"));
    assert!(
        r["location"]
            .as_str()
            .unwrap()
            .starts_with("os-cli/src/logging.rs:"),
        "{r}"
    );
    assert!(!r["backtrace"].as_array().unwrap().is_empty());
    let keys: Vec<&String> = r.as_object().unwrap().keys().collect();
    assert_eq!(keys.len(), 19);
    // Kein Dialog, kein Senden: die CLI hat dafür keinen Weg; Senden gibt es nur in der App (OUT-247).
    assert!(r["sent"].is_null() && r["server_ref"].is_null());
}

// spec: OUT-246
// spec: OUT-243 [M3]
#[cfg(debug_assertions)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn privacy_no_host_secret_email_or_home_path_in_the_report() {
    let site = site("out-basic-site").await;
    let env = Env::new();
    let home = env.home.path().to_string_lossy().into_owned();
    let mut cmd = env.command();
    cmd.env("OPENSPIDER_CRASH_TEST", "panic-after:Abruf abgeschlossen")
        .args([
            "--crawl",
            "http://crawler:pw-SEKRET-246@crash-secret.test/?token=SEKRET-246",
            "--resolve",
            &resolve(&site),
            "--log-level",
            "debug",
            "--set",
            "limits.max_urls=6",
            "--set",
            "speed.max_urls_per_sec=2",
        ]);
    let output = tokio::task::spawn_blocking(move || cmd.output().unwrap())
        .await
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    // Der Panic lief in einem Nebenthread; der Crawl lief weiter.
    assert!(stderr.contains("Absturzbericht gespeichert: "), "{stderr}");
    let reports = reports(&env);
    assert_eq!(reports.len(), 1, "{stderr}");
    let raw = std::fs::read_to_string(&reports[0].0).unwrap();
    for needle in [
        "crash-secret.test",
        "SEKRET-246",
        "pw-SEKRET-246",
        "ops-246@",
        home.as_str(),
    ] {
        assert!(!raw.contains(needle), "{needle} im Bericht:\n{raw}");
    }
    // auch JSON-maskierte Schreibweise des Heimatpfads (Windows)
    assert!(!raw.contains(&home.replace('\\', "\\\\")), "{raw}");
    assert!(raw.contains("<url>"), "{raw}");
    assert!(raw.contains('~'), "{raw}");
    assert!(raw.contains("<email>"), "{raw}");
    let r = &reports[0].1;
    assert_eq!(r["thread"], "crash-test");
    assert!(r["log_tail"].as_array().unwrap().len() <= 200);
    // Das Log selbst enthält die E-Mail (nur der Bericht wird bereinigt).
    let logs: String = std::fs::read_dir(env.home.path().join("logs"))
        .unwrap()
        .flatten()
        .filter(|e| e.file_type().unwrap().is_file())
        .map(|e| std::fs::read_to_string(e.path()).unwrap())
        .collect();
    assert!(logs.contains("ops-246@crash-secret.test"));
}
