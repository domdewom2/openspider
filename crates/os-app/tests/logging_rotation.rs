use os_app::diag::{self, LogFile, LogLayer};
use serde_json::json;
use std::{
    fs,
    time::{Duration, SystemTime},
};
use tracing_subscriber::{prelude::*, EnvFilter};

// spec: OUT-242
#[test]
fn bounded_rotation_and_oldest_first_cleanup_leave_other_files_alone() {
    let home = tempfile::tempdir().unwrap();
    let p = home.path();
    let log = LogFile::with_limits(p.join("openspider.log"), true, 5, 2);
    for n in 0..5 {
        log.write_line(&format!("{n}{n}{n}{n}")).unwrap();
    }
    assert_eq!(
        fs::read_to_string(p.join("openspider.log")).unwrap(),
        "4444\n"
    );
    assert_eq!(
        fs::read_to_string(p.join("openspider.1.log")).unwrap(),
        "3333\n"
    );
    assert_eq!(
        fs::read_to_string(p.join("openspider.2.log")).unwrap(),
        "2222\n"
    );
    assert!(!p.join("openspider.3.log").exists());
    fs::write(p.join("crawl.ospider"), "CRAWL-DATA").unwrap();
    for (name, age) in [("cli-old.log", 31), ("task-new.log", 1)] {
        fs::write(p.join(name), "abc").unwrap();
        let f = fs::File::options().write(true).open(p.join(name)).unwrap();
        f.set_modified(SystemTime::now() - Duration::from_secs(age * 86400))
            .unwrap();
    }
    diag::cleanup(p, SystemTime::now(), 30, 500).unwrap();
    assert!(!p.join("cli-old.log").exists());
    assert!(p.join("task-new.log").exists());
    diag::cleanup(p, SystemTime::now(), 30, 15).unwrap();
    assert!(!p.join("task-new.log").exists());
    assert_eq!(
        diag::log_files(p).unwrap().iter().map(|f| f.2).sum::<u64>(),
        15
    );
    assert_eq!(
        fs::read_to_string(p.join("crawl.ospider")).unwrap(),
        "CRAWL-DATA"
    );
}
// spec: OUT-240
// spec: OUT-241
// spec: OUT-243
#[test]
fn json_lines_have_millisecond_offset_module_filters_and_redaction() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("cli-fixture.log");
    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::new("info,os_fetch=trace"))
        .with(LogLayer {
            file: LogFile::new(path.clone(), false),
            json: true,
            stderr: false,
        });
    tracing::subscriber::with_default(subscriber, || {
        tracing::debug!(target:"os_engine","HIDDEN");
        tracing::trace!(target:"os_fetch",password="PASSWORD-FIXTURE",url="https://user:PASS-FIXTURE@fixture.test/?token=TOKEN-FIXTURE", "fetch");
        tracing::info!("visible");
    });
    let raw = fs::read_to_string(path).unwrap();
    assert!(!raw.contains("HIDDEN"));
    for s in ["PASSWORD-FIXTURE", "PASS-FIXTURE", "TOKEN-FIXTURE"] {
        assert!(!raw.contains(s), "{raw}");
    }
    let lines: Vec<serde_json::Value> = raw
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["module"], "os_fetch");
    assert_eq!(lines[0]["level"], "TRACE");
    let stamp = lines[0]["timestamp"].as_str().unwrap();
    assert_eq!(stamp.len(), 29);
    assert_eq!(&stamp[19..20], ".");
    assert!(matches!(&stamp[23..24], "+" | "-"));
    let format = time::format_description::parse_borrowed::<2>("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3][offset_hour sign:mandatory]:[offset_minute]").unwrap();
    let parsed = time::OffsetDateTime::parse(stamp, &format).unwrap();
    if let Ok(local) = time::UtcOffset::current_local_offset() {
        assert_eq!(parsed.offset(), local);
    } else {
        assert_eq!(parsed.offset(), time::UtcOffset::UTC);
    }
}
// spec: OUT-241
#[test]
fn level_reload_takes_effect_without_restart() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("openspider.log");
    let (layer, reload) = tracing_subscriber::reload::Layer::new(EnvFilter::new("info"));
    let subscriber = tracing_subscriber::registry().with(layer).with(LogLayer {
        file: LogFile::new(path.clone(), true),
        json: false,
        stderr: false,
    });
    tracing::subscriber::with_default(subscriber, || {
        tracing::debug!("BEFORE");
        reload.reload(EnvFilter::new("debug")).unwrap();
        tracing::debug!("AFTER");
    });
    let raw = fs::read_to_string(path).unwrap();
    assert!(!raw.contains("BEFORE"));
    assert!(raw.contains("AFTER"));
}
// spec: OUT-243
#[test]
fn structured_headers_and_free_text_cover_all_sensitive_types() {
    let value = json!({"Authorization":"AUTH-FIXTURE","Proxy-Authorization":"PROXY-FIXTURE","Cookie":"COOKIE-FIXTURE","Set-Cookie":"SET-FIXTURE","api_key":"KEY-FIXTURE","access_token":"OAUTH-FIXTURE","passphrase":"PHRASE-FIXTURE","headers":[{"name":"Authorization","value":"HEADER-FIXTURE"}],"message":"Authorization: Bearer BEARER-FIXTURE"});
    let clean = diag::redact(&value).to_string();
    assert!(!clean.contains("FIXTURE"), "{clean}");
    assert_eq!(
        diag::redact_text("password=SECRET status=200 token=OTHER; duration=17"),
        "password=<redacted> status=200 token=<redacted>; duration=17"
    );
    assert!(
        diag::redact_text("request https://u:p@fixture.test/?api_key=SECRET")
            .contains("<redacted>")
    );
}

// spec: OUT-241
#[test]
fn app_settings_reload_the_installed_filter() {
    const CHILD: &str = "OPENSPIDER_O01_RELOAD_HOME";
    if let Some(home) = std::env::var_os(CHILD) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let app = os_app::App::new(os_app::AppConfig::new(&home)).unwrap();
            tracing::debug!("BEFORE-SETTINGS");
            app.apply_settings_patch(&json!({"logging":{"level":"debug"}}))
                .unwrap();
            tracing::debug!("AFTER-SETTINGS");
        });
        return;
    }
    let home = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "app_settings_reload_the_installed_filter"])
        .env(CHILD, home.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let raw = fs::read_to_string(home.path().join("logs/openspider.log")).unwrap();
    assert!(!raw.contains("BEFORE-SETTINGS"));
    assert!(raw.contains("AFTER-SETTINGS"));
}

// spec: OUT-243
#[test]
fn redaction_preserves_delimited_fields_and_masks_quoted_multiword_values() {
    for (input, expected) in [
        (
            "password=SECRET&status=200",
            "password=<redacted>&status=200",
        ),
        (
            "passphrase=SECRET;status=200",
            "passphrase=<redacted>;status=200",
        ),
        (
            r#"password="multi word SECRET" status=200"#,
            r#"password="<redacted>" status=200"#,
        ),
        (
            r#"{"password": "SECRET", "status": 200}"#,
            r#"{"password": "<redacted>", "status": 200}"#,
        ),
        (
            "password='multi word SECRET';status=200",
            "password='<redacted>';status=200",
        ),
        ("password=秘密 status=200", "password=<redacted> status=200"),
        (
            "password=SEC,RET}] status=200",
            "password=<redacted> status=200",
        ),
        ("Authorization: Bearer SECRET", "Authorization: <redacted>"),
        ("Cookie: session=SECRET; other=OTHER", "Cookie: <redacted>"),
        (
            "url=https://user:SECRET@fixture.test/?token=OTHER&status=200 duration=17",
            "url=https://<redacted>@fixture.test/?token=<redacted>&status=200 duration=17",
        ),
    ] {
        assert_eq!(diag::redact_text(input), expected, "{input}");
    }
}

// spec: OUT-242
#[test]
fn ordinary_lines_do_not_scan_directory_but_rotation_enforces_folder_cap() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("openspider.log");
    let log = LogFile::with_limits(path.clone(), true, 6, 2);
    log.write_line("aa").unwrap();
    // Sparse oversized fixture: it must survive a normal write and disappear on rotation.
    let old = home.path().join("cli-old.log");
    let file = fs::File::create(&old).unwrap();
    file.set_len(500 * 1024 * 1024 + 1).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(60))
        .unwrap();
    drop(file);
    log.write_line("bb").unwrap();
    assert!(old.exists(), "normal events must not invoke folder cleanup");
    log.write_line("cc").unwrap();
    assert!(!old.exists(), "rotation still enforces the folder cap");
    assert_eq!(fs::read_to_string(path).unwrap(), "cc\n");
    assert_eq!(
        fs::read_to_string(home.path().join("openspider.1.log")).unwrap(),
        "aa\nbb\n"
    );
}

// spec: OUT-240
// spec: OUT-243
#[test]
fn app_panic_logs_backtrace_and_calls_the_previous_hook() {
    const CHILD: &str = "OPENSPIDER_O01_APP_PANIC_HOME";
    if let Some(home) = std::env::var_os(CHILD) {
        let old = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            eprintln!("PREVIOUS-APP-HOOK");
            old(info);
        }));
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _app = rt.block_on(async { os_app::App::new(os_app::AppConfig::new(home)).unwrap() });
        panic!("password=PANIC-FIXTURE");
    }
    let home = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "app_panic_logs_backtrace_and_calls_the_previous_hook",
            "--nocapture",
        ])
        .env(CHILD, home.path())
        .env("OPENSPIDER_SECRET_STORE", "memory")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("PREVIOUS-APP-HOOK"));
    let raw = fs::read_to_string(home.path().join("logs/openspider.log")).unwrap();
    assert!(raw.contains("ERROR panic:"));
    assert!(raw.contains("app_panic_logs_backtrace_and_calls_the_previous_hook"));
    assert!(!raw.contains("PANIC-FIXTURE"));
}

// spec: OUT-240
#[test]
fn ordinary_lines_reuse_the_open_handle() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("openspider.log");
    let log = LogFile::new(path.clone(), false);
    log.write_line("first").unwrap();
    // Replacing the directory entry must not cause another open for the next event.
    let moved = home.path().join("moved.log");
    fs::rename(&path, &moved).unwrap();
    fs::write(&path, "replacement\n").unwrap();
    log.write_line("second").unwrap();
    assert_eq!(fs::read_to_string(&moved).unwrap(), "first\nsecond\n");
    assert_eq!(fs::read_to_string(path).unwrap(), "replacement\n");
}
