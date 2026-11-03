//! Absturzberichte ansehen und senden (OUT-247) gegen einen Mock-Server für `/api/crash`; Netzwerkmitschnitt über
//! dessen Anfrageprotokoll. Die echte Adresse `openspider.de` wird in keinem Test kontaktiert.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "session_common.rs"]
mod common;

use os_app::crash::send;
use os_app::dto::{CrashOverview, CrashSendFailure, CrashSendResult};
use os_app::runtime::lifecycle_bus::CrawlEvent;
use os_app::update::policy::{PolicySource, PolicyValue};
use os_app::{App, AppConfig, CallCtx};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const FIXTURE: &str = include_str!("../../../tests/fixtures/crash/crash-panic.json");
const FIXTURE_ID: &str = "6f1c2d3e-4b5a-4c7d-8e9f-0a1b2c3d4e5f";

#[derive(Clone)]
enum Reply {
    Ok(&'static str),
    Status(u16, &'static str),
    Hang,
}

#[derive(Clone, Debug)]
struct Recorded {
    head: String,
    body: Vec<u8>,
}

/// Minimaler HTTP-Mock für `/api/crash` (auch als HTTP-Proxy nutzbar: protokolliert die Anfragezeile).
struct Mock {
    addr: std::net::SocketAddr,
    log: Arc<Mutex<Vec<Recorded>>>,
    replies: Arc<Mutex<VecDeque<Reply>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Mock {
    async fn start() -> Mock {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        let log = Arc::new(Mutex::new(Vec::new()));
        let replies: Arc<Mutex<VecDeque<Reply>>> = Arc::new(Mutex::new(VecDeque::new()));
        let (l, r) = (log.clone(), replies.clone());
        let task = tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let (log, replies) = (l.clone(), r.clone());
                tokio::spawn(async move {
                    let mut head = Vec::new();
                    let mut byte = [0u8; 1];
                    while !head.ends_with(b"\r\n\r\n") {
                        if stream.read_exact(&mut byte).await.is_err() {
                            return;
                        }
                        head.push(byte[0]);
                    }
                    let head = String::from_utf8_lossy(&head).into_owned();
                    let len = head
                        .lines()
                        .find_map(|l| {
                            let (k, v) = l.split_once(':')?;
                            k.eq_ignore_ascii_case("content-length")
                                .then(|| v.trim().parse::<usize>().ok())?
                        })
                        .unwrap_or(0);
                    let mut body = vec![0u8; len];
                    if stream.read_exact(&mut body).await.is_err() {
                        return;
                    }
                    log.lock().unwrap().push(Recorded { head, body });
                    let reply = replies
                        .lock()
                        .unwrap()
                        .pop_front()
                        .unwrap_or(Reply::Ok("ABCD2345"));
                    let (status, text) = match reply {
                        Reply::Ok(r) => (200, format!("{{\"ok\":true,\"ref\":\"{r}\"}}")),
                        Reply::Status(s, e) => (s, format!("{{\"ok\":false,\"error\":\"{e}\"}}")),
                        Reply::Hang => {
                            tokio::time::sleep(Duration::from_secs(60)).await;
                            return;
                        }
                    };
                    let response = format!(
                        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
                        text.len()
                    );
                    let _ = stream.write_all(response.as_bytes()).await;
                    let _ = stream.shutdown().await;
                });
            }
        });
        Mock {
            addr,
            log,
            replies,
            task,
        }
    }
    fn url(&self) -> String {
        format!("http://{}/api/crash", self.addr)
    }
    fn reply(&self, replies: &[Reply]) {
        self.replies.lock().unwrap().extend(replies.iter().cloned());
    }
    fn requests(&self) -> Vec<Recorded> {
        self.log.lock().unwrap().clone()
    }
}

impl Drop for Mock {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn crashes(home: &Path) -> PathBuf {
    home.join("logs").join("crashes")
}

/// Legt die Fixture als Bericht eines früheren Laufs ab (optional mit anderer Kennung und Zeit).
fn place(home: &Path, id: &str, stamp: &str) -> PathBuf {
    let dir = crashes(home);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("crash-{stamp}-panic.json"));
    std::fs::write(&path, FIXTURE.replace(FIXTURE_ID, id)).unwrap();
    path
}

fn start(home: &Path, mock: &Mock) -> App {
    let app = App::new(AppConfig::new(home)).unwrap();
    os_app::crash::service::set_transport_for_tests(
        &app,
        &mock.url(),
        Some(Duration::from_secs(2)),
    );
    app
}

async fn call(app: &App, name: &str, args: Value) -> Result<Value, os_app::CmdError> {
    app.dispatch(CallCtx::for_window("main"), name, args).await
}

async fn overview(app: &App) -> CrashOverview {
    serde_json::from_value(call(app, "crash.overview", Value::Null).await.unwrap()).unwrap()
}

async fn send_ids(app: &App, ids: &[&str], note: &str, email: &str) -> CrashSendResult {
    serde_json::from_value(
        call(
            app,
            "crash.send",
            json!({"ids": ids, "note": note, "email": email}),
        )
        .await
        .unwrap(),
    )
    .unwrap()
}

/// Rohbytes des Werts `report` im Körper (zwischen `{"report":` und `,"note":`).
fn raw_report(body: &[u8]) -> String {
    let text = String::from_utf8(body.to_vec()).unwrap();
    let start = "{\"report\":".len();
    let end = text.rfind(",\"note\":").unwrap();
    text[start..end].to_string()
}

// spec: OUT-247
#[tokio::test]
async fn view_shows_byte_for_byte_the_report_that_is_sent_and_success_records_reference() {
    let mock = Mock::start().await;
    let home = tempfile::tempdir().unwrap();
    place(home.path(), FIXTURE_ID, "20261102-091500");
    let app = start(home.path(), &mock);
    let o = overview(&app).await;
    assert_eq!(o.pending, [FIXTURE_ID]);
    let shown = call(&app, "crash.read", json!({"id": FIXTURE_ID}))
        .await
        .unwrap();
    let shown = shown.as_str().unwrap().to_string();
    assert_eq!(shown, FIXTURE);
    let result = send_ids(
        &app,
        &[FIXTURE_ID],
        "Export gestartet, dann \"hängt\"",
        "ops@example.com",
    )
    .await;
    assert_eq!(result.results.len(), 1);
    assert_eq!(result.results[0].server_ref.as_deref(), Some("ABCD2345"));
    let requests = mock.requests();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert!(
        request.head.starts_with("POST /api/crash HTTP/1.1\r\n"),
        "{}",
        request.head
    );
    assert!(request
        .head
        .to_ascii_lowercase()
        .contains("content-type: application/json"));
    assert_eq!(
        raw_report(&request.body),
        shown,
        "angezeigter Text = gesendeter `report`"
    );
    let body: Value = serde_json::from_slice(&request.body).unwrap();
    let mut keys: Vec<&String> = body.as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(keys, ["email", "note", "report"]);
    assert_eq!(body["note"], "Export gestartet, dann \"hängt\"");
    assert_eq!(body["email"], "ops@example.com");
    // Bericht hat danach `sent` und `server_ref`; Notiz und E-Mail werden nicht gespeichert.
    let stored = call(&app, "crash.read", json!({"id": FIXTURE_ID}))
        .await
        .unwrap();
    let stored: Value = serde_json::from_str(stored.as_str().unwrap()).unwrap();
    assert_eq!(stored["server_ref"], "ABCD2345");
    assert!(stored["sent"].is_string());
    assert!(
        !stored.to_string().contains("ops@example.com")
            && !stored.to_string().contains("Export gestartet")
    );
    let o = overview(&app).await;
    assert!(o.pending.is_empty());
    assert_eq!(o.reports[0].server_ref.as_deref(), Some("ABCD2345"));
    // Keine Kennung des Nutzers im Kopf
    assert!(
        request
            .head
            .contains("user-agent: OpenSpider-CrashReport\r\n"),
        "{}",
        request.head
    );
    assert!(!request.head.to_ascii_lowercase().contains("cookie"));
}

// spec: OUT-247
#[tokio::test]
async fn failures_keep_the_report_unsent_and_retry_sends_again() {
    let mock = Mock::start().await;
    let home = tempfile::tempdir().unwrap();
    place(home.path(), FIXTURE_ID, "20261102-091500");
    let app = start(home.path(), &mock);
    mock.reply(&[
        Reply::Status(500, "server"),
        Reply::Status(429, "rate"),
        Reply::Status(400, "invalid"),
        Reply::Hang,
    ]);
    for expected in [
        CrashSendFailure::Http(500),
        CrashSendFailure::Http(429),
        CrashSendFailure::Http(400),
        CrashSendFailure::Timeout,
    ] {
        let result = send_ids(&app, &[FIXTURE_ID], "", "").await;
        assert_eq!(result.results[0].failure, Some(expected));
        let o = overview(&app).await;
        assert_eq!(o.pending, [FIXTURE_ID], "bleibt ungesendet");
        assert!(o.reports[0].sent.is_none());
    }
    assert_eq!(
        mock.requests().len(),
        4,
        "kein automatischer Wiederholversuch"
    );
    // „Erneut versuchen“
    let result = send_ids(&app, &[FIXTURE_ID], "", "").await;
    assert_eq!(result.results[0].server_ref.as_deref(), Some("ABCD2345"));
    assert_eq!(mock.requests().len(), 5);
    // Antwort 200 ohne gültige Referenz
    place(
        home.path(),
        "11111111-2222-4333-8444-555555555555",
        "20261102-091600",
    );
    mock.reply(&[Reply::Ok("x y")]);
    let result = send_ids(&app, &["11111111-2222-4333-8444-555555555555"], "", "").await;
    assert_eq!(result.results[0].failure, Some(CrashSendFailure::Response));
    // Keine Verbindung
    let closed = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap()
    };
    os_app::crash::service::set_transport_for_tests(
        &app,
        &format!("http://{closed}/api/crash"),
        None,
    );
    let result = send_ids(&app, &["11111111-2222-4333-8444-555555555555"], "", "").await;
    assert_eq!(result.results[0].failure, Some(CrashSendFailure::Network));
}

// spec: OUT-247
#[test]
fn sending_uses_a_fifteen_second_limit_and_the_fixed_endpoint() {
    assert_eq!(send::TIMEOUT, Duration::from_secs(15));
    assert_eq!(send::ENDPOINT, "https://openspider.de/api/crash");
    assert_eq!(send::MAX_BATCH, 5);
}

// spec: OUT-247
// spec: OUT-246
#[tokio::test]
async fn network_capture_no_request_without_send_click_then_one_request_per_report() {
    let mock = Mock::start().await;
    let home = tempfile::tempdir().unwrap();
    let ids: Vec<String> = (0..7)
        .map(|n| format!("00000000-0000-4000-8000-00000000000{n}"))
        .collect();
    for (n, id) in ids.iter().enumerate() {
        place(home.path(), id, &format!("20261102-0915{n:02}"));
    }
    {
        // Start mit Hinweis, Ansehen, „Nicht senden“
        let app = start(home.path(), &mock);
        let o = overview(&app).await;
        assert_eq!(o.pending.len(), 7);
        assert_eq!(o.pending[0], ids[6], "neueste zuerst");
        for id in &o.pending {
            call(&app, "crash.read", json!({"id": id})).await.unwrap();
        }
        call(&app, "crash.dismiss", json!({"ids": o.pending}))
            .await
            .unwrap();
        assert!(overview(&app).await.pending.is_empty());
        // Ein Fehler in einer Fehlergrenze erzeugt einen Bericht, sendet aber nichts.
        call(
            &app,
            "crash.reportFrontend",
            json!({"message":"x","componentStack":"","region":"table"}),
        )
        .await
        .unwrap();
    }
    // Erneuter Start: verworfene Berichte bleiben verborgen, der neue frontend-Bericht erscheint.
    let app = start(home.path(), &mock);
    let o = overview(&app).await;
    assert_eq!(o.reports.len(), 8);
    assert_eq!(o.pending.len(), 1);
    assert_eq!(
        mock.requests().len(),
        0,
        "ohne Klick auf Senden keine Verbindung"
    );
    // „Senden“ mit mehreren Berichten: die neuesten 5, je eine Anfrage.
    let all: Vec<&str> = ids.iter().map(String::as_str).collect();
    let result = send_ids(&app, &all, "", "").await;
    assert_eq!(result.results.len(), 5);
    let sent: Vec<&str> = result.results.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(
        sent,
        [&ids[6], &ids[5], &ids[4], &ids[3], &ids[2]].map(String::as_str)
    );
    assert_eq!(mock.requests().len(), 5);
    let o = overview(&app).await;
    let unsent: Vec<&str> = o
        .reports
        .iter()
        .filter(|r| r.sent.is_none() && ids.contains(&r.id))
        .map(|r| r.id.as_str())
        .collect();
    assert_eq!(
        unsent,
        [&ids[1], &ids[0]].map(String::as_str),
        "ältere bleiben ungesendet"
    );
}

// spec: OUT-247
// spec: OUT-246
#[tokio::test]
async fn reports_of_the_running_process_appear_only_at_the_next_start() {
    let mock = Mock::start().await;
    let home = tempfile::tempdir().unwrap();
    let app = start(home.path(), &mock);
    assert!(overview(&app).await.pending.is_empty());
    let written = call(&app, "crash.reportFrontend", json!({"message":"boom","componentStack":"\n    at Table (http://localhost:1420/src/table.tsx:1:1)","region":"table"}))
        .await
        .unwrap();
    assert_eq!(written, json!(true));
    let o = overview(&app).await;
    assert_eq!(o.reports.len(), 1);
    assert!(o.pending.is_empty(), "laufender Prozess: kein Hinweis");
    drop(app);
    let app = start(home.path(), &mock);
    assert_eq!(overview(&app).await.pending.len(), 1);
    assert_eq!(mock.requests().len(), 0);
}

// spec: OUT-246
#[tokio::test]
async fn frontend_reports_are_capped_at_five_and_deduplicated() {
    let mock = Mock::start().await;
    let home = tempfile::tempdir().unwrap();
    let app = start(home.path(), &mock);
    let stack = "\n    at Exploding (http://localhost:1420/src/x.tsx:3:9)\n    at RegionBoundary (http://localhost:1420/src/app/ErrorBoundary.tsx:9:1)\n    Detail@tauri://localhost/assets/index.js:1:2";
    let report =
        |message: &str| json!({"message": message, "componentStack": stack, "region": "detail"});
    assert_eq!(
        call(&app, "crash.reportFrontend", report("same"))
            .await
            .unwrap(),
        json!(true)
    );
    assert_eq!(
        call(&app, "crash.reportFrontend", report("same"))
            .await
            .unwrap(),
        json!(false)
    );
    for n in 0..6 {
        call(&app, "crash.reportFrontend", report(&format!("other {n}")))
            .await
            .unwrap();
    }
    let o = overview(&app).await;
    assert_eq!(o.reports.len(), 5, "höchstens 5 je Lauf");
    assert!(o.reports.iter().all(|r| r.kind == "frontend"));
    let stored = call(
        &app,
        "crash.read",
        json!({"id": o.reports.last().unwrap().id}),
    )
    .await
    .unwrap();
    let stored: Value = serde_json::from_str(stored.as_str().unwrap()).unwrap();
    assert_eq!(stored["message"], "same");
    assert_eq!(
        stored["backtrace"],
        json!(["Exploding", "RegionBoundary", "Detail"])
    );
    assert_eq!(stored["thread"], "ui:detail");
    assert_eq!(stored["process"], "app");
    assert_eq!(stored["location"], "");
}

struct Fixed(PolicyValue, PolicyValue, Option<&'static str>);
impl PolicySource for Fixed {
    fn machine(&self, _: &str) -> PolicyValue {
        self.0
    }
    fn user(&self, _: &str) -> PolicyValue {
        self.1
    }
    fn env(&self, _: &str) -> Option<String> {
        self.2.map(str::to_string)
    }
}

// spec: OUT-247
// spec: OUT-253 [M3]
#[tokio::test]
async fn setting_off_hides_the_prompt_and_policy_blocks_prompt_and_every_request() {
    use PolicyValue::{Missing, Set};
    let mock = Mock::start().await;
    let home = tempfile::tempdir().unwrap();
    place(home.path(), FIXTURE_ID, "20261102-091500");
    let app = start(home.path(), &mock);
    // Einstellung aus: kein Hinweis, Bericht bleibt in der Liste und ist sendbar.
    call(
        &app,
        "settings.set",
        json!({"patch": {"crashReports": {"prompt": false}}}),
    )
    .await
    .unwrap();
    let persisted = std::fs::read_to_string(app.settings().path()).unwrap();
    assert!(
        persisted.contains("[crash_reports]") && persisted.contains("prompt = false"),
        "{persisted}"
    );
    let o = overview(&app).await;
    assert!(!o.prompt_enabled && o.pending.is_empty() && o.reports.len() == 1);
    call(
        &app,
        "settings.set",
        json!({"patch": {"crashReports": {"prompt": true}}}),
    )
    .await
    .unwrap();
    assert_eq!(overview(&app).await.pending, [FIXTURE_ID]);
    // Rangfolge über den gemeinsamen Resolver: HKLM > HKCU > Umgebung.
    for (machine, user, env, disabled) in [
        (Set(true), Missing, None, true),
        (Set(false), Set(true), Some("1"), false),
        (Missing, Set(true), Some("0"), true),
        (Missing, Missing, Some("1"), true),
        (Missing, Missing, Some("0"), false),
        (Missing, Missing, None, false),
    ] {
        os_app::crash::service::set_policy_source_for_tests(
            &app,
            Arc::new(Fixed(machine, user, env)),
        );
        let o = overview(&app).await;
        assert_eq!(
            o.disabled_by_policy, disabled,
            "{machine:?} {user:?} {env:?}"
        );
        assert_eq!(o.pending.is_empty(), disabled);
    }
    os_app::crash::service::set_policy_source_for_tests(
        &app,
        Arc::new(Fixed(Set(true), Missing, None)),
    );
    let result = send_ids(&app, &[FIXTURE_ID], "", "").await;
    assert_eq!(result.results[0].failure, Some(CrashSendFailure::Disabled));
    assert_eq!(
        mock.requests().len(),
        0,
        "Richtlinie: keine Anfrage, auch nicht über das Hilfe-Menü"
    );
    assert_eq!(
        overview(&app).await.reports.len(),
        1,
        "lokales Speichern bleibt"
    );
}

// spec: OUT-247
#[test]
fn environment_variable_disables_crash_reports() {
    // Eigener Prozess: Umgebungsvariablen gelten prozessweit.
    const CHILD: &str = "OPENSPIDER_C01_POLICY_CHILD";
    if std::env::var_os(CHILD).is_some() {
        assert!(os_app::update::policy::crash_reports_disabled(
            &os_app::update::policy::SystemPolicy
        ));
        return;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "environment_variable_disables_crash_reports",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .env("OPENSPIDER_DISABLE_CRASH_REPORTS", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// spec: OUT-247
#[tokio::test]
async fn invalid_note_or_email_is_rejected_before_any_request() {
    let mock = Mock::start().await;
    let home = tempfile::tempdir().unwrap();
    place(home.path(), FIXTURE_ID, "20261102-091500");
    let app = start(home.path(), &mock);
    let err = call(
        &app,
        "crash.send",
        json!({"ids":[FIXTURE_ID],"note":"x".repeat(2001),"email":""}),
    )
    .await
    .unwrap_err();
    assert_eq!(err.field.as_deref(), Some("note"));
    let err = call(
        &app,
        "crash.send",
        json!({"ids":[FIXTURE_ID],"note":"","email":"kein@"}),
    )
    .await
    .unwrap_err();
    assert_eq!(err.field.as_deref(), Some("email"));
    assert!(mock.requests().is_empty());
}

// spec: OUT-247
// spec: CRAWL-122 [M3]
#[tokio::test]
async fn sending_uses_the_app_wide_proxy() {
    let proxy = Mock::start().await;
    let home = tempfile::tempdir().unwrap();
    place(home.path(), FIXTURE_ID, "20261102-091500");
    let app = App::new(AppConfig::new(home.path())).unwrap();
    // Nicht auflösbarer Host: ohne Proxy wäre die Anfrage unmöglich.
    os_app::crash::service::set_transport_for_tests(
        &app,
        "http://crash-endpoint.invalid/api/crash",
        None,
    );
    call(
        &app,
        "settings.set",
        json!({"patch": {"network": {"proxy": {"mode": "manual", "kind": "http", "host": "127.0.0.1", "port": proxy.addr.port(), "bypass": []}}}}),
    )
    .await
    .unwrap();
    let result = send_ids(&app, &[FIXTURE_ID], "", "").await;
    assert_eq!(
        result.results[0].server_ref.as_deref(),
        Some("ABCD2345"),
        "{result:?}"
    );
    let requests = proxy.requests();
    assert_eq!(requests.len(), 1);
    assert!(
        requests[0]
            .head
            .starts_with("POST http://crash-endpoint.invalid/api/crash HTTP/1.1"),
        "{}",
        requests[0].head
    );
}

// spec: OUT-247
// spec: OUT-246
#[tokio::test]
async fn sending_runs_off_the_ui_thread_and_leaves_a_running_crawl_alone() {
    let site = os_testsite::TestSite::start_dir(common::fixture(), 0)
        .await
        .unwrap();
    let mock = Mock::start().await;
    let dir = tempfile::tempdir().unwrap();
    place(dir.path(), FIXTURE_ID, "20261102-091500");
    let app = start(dir.path(), &mock);
    common::cfg()
        .save_default(&app.config().paths.settings)
        .unwrap();
    let transitions = Arc::new(Mutex::new(0usize));
    let observed = transitions.clone();
    app.crawl_events().subscribe(Box::new(move |_, event| {
        if matches!(
            event,
            CrawlEvent::Started(_)
                | CrawlEvent::Paused(_)
                | CrawlEvent::Resumed(_)
                | CrawlEvent::Stopped(_)
        ) {
            *observed.lock().unwrap() += 1;
        }
    }));
    common::call(
        &app,
        "crawl.start",
        serde_json::to_value(common::request(site.url("/"))).unwrap(),
    )
    .await;
    assert_eq!(
        app.sessions().current(&common::window()).unwrap().state,
        os_model::CrawlState::Running
    );
    let before = *transitions.lock().unwrap();
    // Während des Crawls: Frontend-Bericht kennt den laufenden Crawl.
    call(
        &app,
        "crash.reportFrontend",
        json!({"message":"during crawl","componentStack":"","region":"table"}),
    )
    .await
    .unwrap();
    let id = overview(&app)
        .await
        .reports
        .iter()
        .find(|r| r.kind == "frontend")
        .unwrap()
        .id
        .clone();
    let text = call(&app, "crash.read", json!({"id": id})).await.unwrap();
    let report: Value = serde_json::from_str(text.as_str().unwrap()).unwrap();
    assert_eq!(report["crawl_running"], true);
    // Senden ist ein eigener Task der App-Laufzeit; der aufrufende Thread bleibt frei.
    let handle = tokio::spawn({
        let app = app.clone();
        async move { send_ids(&app, &[FIXTURE_ID], "", "").await }
    });
    let result = handle.await.unwrap();
    assert!(result.results[0].server_ref.is_some());
    assert_eq!(
        app.sessions().current(&common::window()).unwrap().state,
        os_model::CrawlState::Running
    );
    assert_eq!(
        *transitions.lock().unwrap() - before,
        0,
        "keine Pause, kein Neustart"
    );
    common::call(&app, "crawl.pause", Value::Null).await;
}

// spec: OUT-246
// spec: OUT-244 [M3]
#[tokio::test]
async fn diagnostics_bundle_contains_the_stored_reports() {
    let home = tempfile::tempdir().unwrap();
    place(home.path(), FIXTURE_ID, "20261102-091500");
    let app = App::new(AppConfig::new(home.path())).unwrap();
    let preview = call(&app, "diag.previewBundle", Value::Null).await.unwrap();
    let files: Vec<String> = serde_json::from_value(preview["files"].clone()).unwrap();
    assert!(
        files.contains(&"crashes/crash-20261102-091500-panic.json".to_string()),
        "{files:?}"
    );
    let entries = os_app::diag::bundle_entries(
        &home.path().join("logs"),
        json!({}),
        json!({}),
        json!([]),
        std::time::SystemTime::now(),
    )
    .unwrap();
    let report = entries
        .iter()
        .find(|e| e.0.starts_with("crashes/"))
        .unwrap();
    let parsed: Value = serde_json::from_slice(&report.1).unwrap();
    assert_eq!(parsed["id"], FIXTURE_ID);
    // Sitzungsmarken und verworfene Kennungen gehören nicht hinein.
    assert!(entries
        .iter()
        .all(|e| !e.0.contains("session-") && !e.0.contains("dismissed")));
}

// spec: OUT-247
#[tokio::test]
async fn delete_removes_a_report_and_unknown_ids_are_not_found() {
    let mock = Mock::start().await;
    let home = tempfile::tempdir().unwrap();
    let path = place(home.path(), FIXTURE_ID, "20261102-091500");
    let app = start(home.path(), &mock);
    call(&app, "crash.dismiss", json!({"ids":[FIXTURE_ID]}))
        .await
        .unwrap();
    call(&app, "crash.delete", json!({"id": FIXTURE_ID}))
        .await
        .unwrap();
    assert!(!path.exists());
    assert!(overview(&app).await.reports.is_empty());
    let err = call(&app, "crash.read", json!({"id": FIXTURE_ID}))
        .await
        .unwrap_err();
    assert_eq!(err.code, os_app::ErrorCode::NotFound);
    let result = send_ids(&app, &[FIXTURE_ID], "", "").await;
    assert_eq!(result.results[0].failure, Some(CrashSendFailure::NotFound));
    assert!(mock.requests().is_empty());
}

// spec: OUT-246
#[tokio::test]
async fn app_log_tail_has_no_page_content_from_foreign_crates() {
    use std::io::Write;
    let mock = Mock::start().await;
    let home = tempfile::tempdir().unwrap();
    let app = start(home.path(), &mock);
    // Zeilen dieses Laufs (nach dem Start-Versatz): eigene Module jedes Levels, Fremd-Crates mit Seiteninhalt.
    std::fs::create_dir_all(home.path().join("logs")).unwrap();
    let mut log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(home.path().join("logs").join("openspider.log"))
        .unwrap();
    writeln!(log, "2026-11-03T10:00:00.000+00:00 DEBUG os_engine::crawl: Seite abgerufen [url=\"https://kunde.example/about.html\"]").unwrap();
    for _ in 0..300 {
        writeln!(log, "2026-11-03T10:00:00.001+00:00 DEBUG selectors::matching: Matching complex selector for <meta content=\"Home page of the basic OpenSpider fixture site.\"> []").unwrap();
    }
    writeln!(log, "2026-11-03T10:00:00.002+00:00 WARN html5ever::tree_builder: Unexpected token in /about.html <title>Kunde</title> []").unwrap();
    drop(log);
    call(
        &app,
        "crash.reportFrontend",
        json!({"message":"boom","componentStack":"","region":"table"}),
    )
    .await
    .unwrap();
    let id = overview(&app).await.reports[0].id.clone();
    let text = call(&app, "crash.read", json!({"id": id})).await.unwrap();
    let text = text.as_str().unwrap();
    for content in [
        "Home page of the basic OpenSpider fixture site",
        "/about.html",
        "<title",
        "kunde.example",
    ] {
        assert!(!text.contains(content), "{content} im Bericht:\n{text}");
    }
    let report: Value = serde_json::from_str(text).unwrap();
    let tail: Vec<&str> = report["log_tail"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l.as_str().unwrap())
        .collect();
    assert!(tail.contains(&"2026-11-03T10:00:00.000+00:00 DEBUG os_engine::crawl: Seite abgerufen [url=\"<url>\"]"), "{tail:?}");
    assert!(
        tail.contains(&"2026-11-03T10:00:00.002+00:00 WARN html5ever::tree_builder: <omitted>"),
        "{tail:?}"
    );
    assert!(
        tail.iter().all(|l| !l.contains("selectors::matching")),
        "{tail:?}"
    );
}
