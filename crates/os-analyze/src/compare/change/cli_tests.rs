//! Öffentliche CLI-/Scheduling-Abläufe; eingebunden von os-cli::exec.
use super::*;
use clap::Parser;
use os_analyze::compare::change::{self, auto};
use os_model::{CancelToken, CrawlConfig};
use os_store::paths::{app_paths_with_home, AppPaths};

#[path = "../../../tests/change_detection_fixture.rs"]
mod fixture;

async fn invoke(paths: &AppPaths, args: Vec<String>) -> (Outcome, Arc<Reporter>) {
    let cli = crate::Cli::try_parse_from(std::iter::once("openspider".into()).chain(args)).unwrap();
    let rep = Reporter::new(false);
    let plan = crate::plan::build(&cli.run, paths.clone(), &rep).unwrap();
    let result = execute(plan, rep.clone(), Signals::manual(CancelToken::new())).await;
    (result, rep)
}
fn args(path: &Path, flag: &str) -> Vec<String> {
    let mut args = vec![
        "--load-crawl".into(),
        path.to_string_lossy().into_owned(),
        "--project-name".into(),
        "project".into(),
        flag.into(),
    ];
    if flag == "--project-crawl-comparison" {
        args.push("true".into());
    }
    args
}
fn paths(home: &Path) -> AppPaths {
    let paths = app_paths_with_home(Some(home.as_os_str()));
    paths.create_all().unwrap();
    paths
}

// spec: OUT-152
// spec: TECH-308
// spec: CRAWL-036
#[tokio::test]
async fn public_cli_flags_have_identical_saved_results_and_first_crawl_succeeds() {
    let home = tempfile::tempdir().unwrap();
    let paths = paths(home.path());
    let old = fixture::fixture(&paths.crawls, 1, 1, CrawlConfig::default());
    let first = invoke(&paths, args(old.path(), "--compare-with-previous"))
        .await
        .0;
    assert_eq!(first.exit, Exit::Ok, "{}", first.summary);
    assert_eq!(
        old.get_meta("compare.auto_state").unwrap().as_deref(),
        Some("no_previous")
    );
    let new = fixture::fixture(&paths.crawls, 2, 2, CrawlConfig::default());
    for flag in ["--compare-with-previous", "--project-crawl-comparison"] {
        let result = invoke(&paths, args(new.path(), flag)).await.0;
        assert_eq!(result.exit, Exit::Ok, "{}", result.summary);
    }
    let artifacts = std::fs::read_dir(paths.crawls.join("comparisons"))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(artifacts.len(), 1);
    let (saved, cfg) = auto::load(&artifacts[0].path(), &CancelToken::new()).unwrap();
    let manual = fixture::comparison(
        CrawlFile::open(new.path(), OpenMode::ReadOnly).unwrap(),
        CrawlFile::open(old.path(), OpenMode::ReadOnly).unwrap(),
    );
    change::calculate(&manual, &cfg.change_detection, &CancelToken::new()).unwrap();
    assert_eq!(
        change::counts(&saved, &cfg.change_detection, false).unwrap(),
        change::counts(&manual, &cfg.change_detection, false).unwrap()
    );
}

// spec: OUT-152
#[tokio::test]
async fn comparison_write_and_corrupt_artifact_fail_even_without_strict_and_never_fall_back() {
    let home = tempfile::tempdir().unwrap();
    let paths = paths(home.path());
    let _old = fixture::fixture(&paths.crawls, 1, 1, CrawlConfig::default());
    let new = fixture::fixture(&paths.crawls, 2, 2, CrawlConfig::default());
    let output = home.path().join("exports");
    let mut args = args(new.path(), "--compare-with-previous");
    args.extend([
        "--output-folder".into(),
        output.to_string_lossy().into_owned(),
        "--export-tabs".into(),
        "Internal:All".into(),
        "--overwrite".into(),
    ]);
    let comparisons = paths.crawls.join("comparisons");
    std::fs::write(&comparisons, "blocks directory creation").unwrap();
    let (result, rep) = invoke(&paths, args.clone()).await;
    assert_eq!(result.exit, Exit::ExportFailed);
    assert!(rep.exported_files().is_empty());
    std::fs::remove_file(&comparisons).unwrap();
    assert_eq!(invoke(&paths, args.clone()).await.0.exit, Exit::Ok);
    let artifact = std::fs::read_dir(&comparisons)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    std::fs::write(artifact, "damaged comparison database").unwrap();
    let (result, rep) = invoke(&paths, args.clone()).await;
    assert_eq!(result.exit, Exit::ExportFailed);
    assert!(rep.exported_files().is_empty());
    // Auch ohne Export muss der angeforderte Vergleich den Exit-Code beeinflussen.
    args.truncate(5);
    assert_eq!(invoke(&paths, args).await.0.exit, Exit::ExportFailed);
}

// spec: OUT-152
// spec: TECH-301
#[tokio::test]
async fn cli_comparison_exports_intersect_saved_segment_for_normal_and_change_tabs() {
    let home = tempfile::tempdir().unwrap();
    let paths = paths(home.path());
    let cfg = CrawlConfig { segments: serde_json::from_value(serde_json::json!({"list":[{"name":"Selected","color":"#123456","groups":[{"conditions":[{"field":"Address","op":"equals","value":"https://site.test/a"}]}]}]})).unwrap(), ..Default::default() };
    let _old = fixture::fixture(&paths.crawls, 1, 1, cfg.clone());
    let new = fixture::fixture(&paths.crawls, 2, 2, cfg);
    let output = home.path().join("exports");
    let mut args = args(new.path(), "--compare-with-previous");
    args.extend([
        "--output-folder".into(),
        output.to_string_lossy().into_owned(),
        "--export-tabs".into(),
        "Internal:All,Change Detection:All".into(),
        "--segment".into(),
        "Selected".into(),
    ]);
    let (result, rep) = invoke(&paths, args).await;
    assert_eq!(result.exit, Exit::Ok, "{}", result.summary);
    assert_eq!(rep.exported_files().len(), 2);
    for path in rep.exported_files() {
        let mut csv = csv::Reader::from_path(path).unwrap();
        let rows = csv.records().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(&rows[0][0], "https://site.test/a");
    }
}

// spec: OUT-152
// spec: TECH-305
// spec: OUT-145
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cli_crawl_content_uses_the_content_area_and_reclassifies_partial_changes() {
    let home = tempfile::tempdir().unwrap();
    let site_dir = tempfile::tempdir().unwrap();
    std::fs::write(site_dir.path().join("scenario.toml"), "").unwrap();
    std::fs::write(site_dir.path().join("index.html"), "<html><body><a href='/partial.html'>Partial</a><a href='/footer.html'>Footer</a></body></html>").unwrap();
    let words = |prefix: &str| {
        (0..30)
            .map(|n| format!("{prefix}{n}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let common = words("common");
    let different = words("different");
    let page = |main: &str, footer: &str| {
        format!("<html><head><title>Content</title></head><body><main>{main}</main><footer>{footer}</footer></body></html>")
    };
    std::fs::write(
        site_dir.path().join("partial.html"),
        page(&format!("{common} {common}"), "old footer"),
    )
    .unwrap();
    std::fs::write(
        site_dir.path().join("footer.html"),
        page(&common, "old footer"),
    )
    .unwrap();
    let site = os_testsite::TestSite::start_dir(site_dir.path(), 0)
        .await
        .unwrap();
    let paths = paths(home.path());
    let crawl = || {
        vec![
            "--crawl".into(),
            site.base_url(),
            "--headless".into(),
            "--project-name".into(),
            "project".into(),
            "--set".into(),
            "robots.retries=0".into(),
            "--set".into(),
            "threads=1".into(),
        ]
    };
    let first = invoke(&paths, crawl()).await.0;
    assert_eq!(first.exit, Exit::Ok, "{}", first.summary);
    std::fs::write(
        site_dir.path().join("partial.html"),
        page(&format!("{common} {different}"), "old footer"),
    )
    .unwrap();
    std::fs::write(
        site_dir.path().join("footer.html"),
        page(&common, "completely changed footer text"),
    )
    .unwrap();
    let second = invoke(&paths, crawl()).await.0;
    assert_eq!(second.exit, Exit::Ok, "{}", second.summary);
    let c = fixture::comparison(
        CrawlFile::open(second.crawl_path.as_ref().unwrap(), OpenMode::ReadOnly).unwrap(),
        CrawlFile::open(first.crawl_path.as_ref().unwrap(), OpenMode::ReadOnly).unwrap(),
    );
    let mut cfg = fixture::all();
    change::calculate(&c, &cfg, &CancelToken::new()).unwrap();
    let pct = |path: &str| {
        c.current().file.conn().query_row("SELECT pct FROM temp.c02_cells JOIN urls u ON u.id=url_id WHERE element='content' AND u.address LIKE ?1",[format!("%/{path}.html")],|r|r.get::<_,f64>(0)).unwrap()
    };
    assert_eq!(pct("footer"), 0.0);
    assert!(
        (35.0..=65.0).contains(&pct("partial")),
        "{}",
        pct("partial")
    );
    assert_eq!(change::ids(&c, "Content", true).unwrap().len(), 1);
    cfg.content_threshold_pct = 65;
    change::calculate(&c, &cfg, &CancelToken::new()).unwrap();
    assert!(change::ids(&c, "Content", true).unwrap().is_empty());
}

// spec: OUT-152
// spec: TECH-308
// spec: CRAWL-036
// spec: OUT-282
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn scheduled_task_runs_persist_exactly_one_comparison_of_the_last_two_crawls() {
    scheduled_task_mail(false).await;
}

// spec: OUT-152
// spec: TECH-308
// spec: CRAWL-036
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn standard_run_task_sends_configured_smtp_comparison_mail() {
    scheduled_task_mail(true).await;
}

// spec: OUT-152
#[test]
fn standard_task_entry_child() {
    if std::env::var_os("OPENSPIDER_C02_SMTP_CHILD").is_none() {
        return;
    }
    // Nur der Testprozess befüllt den prozessweiten Memory-Store. Kein Kennwort in argv oder Dateien.
    os_fetch::secret_store_from_env()
        .set("mail/c02", "fixture-secret-never-in-mail")
        .unwrap();
    assert_eq!(
        crate::main_entry(["openspider", "--run-task", "c02-auto"]),
        0
    );
}

// spec: OUT-152
// spec: TECH-308
#[test]
fn configured_smtp_requires_credentials_and_confirmation_without_leaking_secrets() {
    use os_sched::mail::completion::{SmtpAccount, SmtpCompletionSender, SmtpTls};
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join("mail")).unwrap();
    let secrets = os_fetch::MemorySecretStore::new();
    let mut account = SmtpAccount {
        host: "127.0.0.1".into(),
        port: 25,
        tls: SmtpTls::None,
        username: "fixture-user".into(),
        from: "sender@example.test".into(),
        allow_insecure: false,
    };
    let write = |account: &SmtpAccount| {
        std::fs::write(
            home.path().join("mail").join("c02.json"),
            serde_json::to_vec(account).unwrap(),
        )
        .unwrap();
    };
    write(&account);
    let error = SmtpCompletionSender::from_account(home.path(), "c02", &secrets)
        .err()
        .unwrap();
    assert!(error.contains("explicit confirmation"));
    account.allow_insecure = true;
    write(&account);
    assert!(
        SmtpCompletionSender::from_account(home.path(), "c02", &secrets)
            .err()
            .unwrap()
            .contains("credential missing")
    );
    secrets.set("mail/c02", "fixture-secret-never-in-mail");
    for tls in [SmtpTls::None, SmtpTls::Tls, SmtpTls::Starttls] {
        account.tls = tls;
        write(&account);
        assert!(SmtpCompletionSender::from_account(home.path(), "c02", &secrets).is_ok());
    }
    for id in ["../c02", "", "/c02", "c02/other"] {
        assert!(
            SmtpCompletionSender::from_account(home.path(), id, &secrets)
                .err()
                .unwrap()
                .contains("invalid account ID")
        );
    }
    std::fs::write(
        home.path().join("mail").join("c02.json"),
        r#"{"password":"fixture-secret-never-in-mail"}"#,
    )
    .unwrap();
    let error = SmtpCompletionSender::from_account(home.path(), "c02", &secrets)
        .err()
        .unwrap();
    assert!(error.contains("invalid SMTP account configuration"));
    assert!(!error.contains("fixture-secret-never-in-mail"));
}

async fn scheduled_task_mail(standard_process: bool) {
    use crate::run_task::{comparison_mail::ComparisonMailHooks, run_in, RunCtx};
    use os_sched::tasks::{Task, TaskStore};
    let home = tempfile::tempdir().unwrap();
    let paths = paths(home.path());
    let site_dir = tempfile::tempdir().unwrap();
    std::fs::write(site_dir.path().join("scenario.toml"), "").unwrap();
    std::fs::write(
        site_dir.path().join("index.html"),
        "<html><body><a href='/a.html'>A</a><a href='/i.html'>I</a></body></html>",
    )
    .unwrap();
    std::fs::write(
        site_dir.path().join("a.html"),
        "<html><head><title>Before</title></head><body>A</body></html>",
    )
    .unwrap();
    std::fs::write(
        site_dir.path().join("i.html"),
        "<html><head><title>I</title></head><body>I</body></html>",
    )
    .unwrap();
    let site = os_testsite::TestSite::start_dir(site_dir.path(), 0)
        .await
        .unwrap();
    let (port, smtp) = smtp_fixture(standard_process).await;
    if standard_process {
        std::fs::create_dir_all(paths.settings.join("mail")).unwrap();
        let account = os_sched::mail::completion::SmtpAccount {
            host: "127.0.0.1".into(),
            port,
            tls: os_sched::mail::completion::SmtpTls::None,
            username: "fixture-user".into(),
            from: "configured-sender@example.test".into(),
            allow_insecure: true,
        };
        // spec: OUT-282 — C02 accepts the account metadata persisted by T02.
        let mut account = serde_json::to_value(&account).unwrap();
        account["id"] = serde_json::json!("c02");
        account["label"] = serde_json::json!("Integration SMTP");
        account["auth"] = serde_json::json!("password");
        account["clientId"] = serde_json::Value::Null;
        std::fs::write(
            paths.settings.join("mail").join("c02.json"),
            serde_json::to_vec(&account).unwrap(),
        )
        .unwrap();
    }
    let hooks = ComparisonMailHooks {
        crawls: paths.crawls.clone(),
        sender: Arc::new(
            os_sched::mail::completion::SmtpCompletionSender::loopback(port, "sender@example.test")
                .unwrap(),
        ),
    };
    let task: Task = serde_json::from_value(serde_json::json!({
        "id":"c02-auto","name":"C02 Auto","project":"project",
        "schedule":{"kind":"daily","time":"03:00"},
        "crawl":{"source":{"kind":"spider","url":site.base_url()},"overrides":{"threads":"1","robots.retries":"0"}},
        "execution":{"auto_compare":true},
        "notify":{"on_success":true,"account":"c02","to":["receiver@example.test"]}
    })).unwrap();
    let task = TaskStore::in_settings(&paths.settings).save(task).unwrap();
    let clock = Arc::new(os_model::ManualClock::new(
        os_model::Timestamp::from_unix_millis(1_800_000_000_000),
    ));
    let secrets = Arc::new(os_fetch::MemorySecretStore::new());
    secrets.set("mail/c02", "fixture-secret-never-in-mail");
    let ctx = RunCtx {
        paths: paths.clone(),
        clock: clock.clone(),
        secrets,
        run_id: None,
        manual: false,
        log_path: None,
        export_dir: home.path().join("exports"),
        json_progress: false,
        heartbeat: std::time::Duration::from_secs(60),
        poll: std::time::Duration::from_millis(20),
        timeout_scale: 1.0,
    };
    let mut ids = Vec::new();
    for version in 0..2 {
        if version == 1 {
            std::fs::write(
                site_dir.path().join("a.html"),
                "<html><head><title>After</title></head><body>A</body></html>",
            )
            .unwrap();
            std::fs::write(site_dir.path().join("i.html"), "<html><head><title>I</title><meta name=robots content=noindex></head><body>I</body></html>").unwrap();
        }
        let record = if standard_process {
            // Eigener Prozess: echter CLI-Dispatcher, run(), DefaultHooks, Konto-Datei und SecretStore.
            // Die Umgebung des parallelen Testprozesses bleibt unverändert.
            let output = tokio::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "exec::change_tests::standard_task_entry_child",
                    "--nocapture",
                ])
                .env("OPENSPIDER_C02_SMTP_CHILD", "1")
                .env("OPENSPIDER_HOME", home.path())
                .env("OPENSPIDER_SECRET_STORE", "memory")
                .env_remove(crate::run_task::RUN_ID_ENV)
                .output()
                .await
                .unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
            assert!(!String::from_utf8_lossy(&output.stderr)
                .contains("Benachrichtigung nicht gesendet"));
            os_sched::history::HistoryStore::in_settings(&paths.settings)
                .list(Some(&task.id), 0, 10)
                .into_iter()
                .next()
                .unwrap()
        } else {
            let run = run_in(
                &ctx,
                &task.id,
                &hooks,
                Signals::manual(CancelToken::new()),
                Reporter::for_task(false),
            )
            .await;
            assert_eq!(run.exit, Exit::Ok, "{}", run.summary);
            run.record.unwrap()
        };
        assert!(record.warnings.is_empty(), "{:?}", record.warnings);
        ids.push(record.crawl_id.unwrap());
        clock.advance(std::time::Duration::from_secs(60));
    }
    let artifacts = std::fs::read_dir(paths.crawls.join("comparisons"))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(artifacts.len(), 1);
    let (comparison, config) = auto::load(&artifacts[0].path(), &CancelToken::new()).unwrap();
    if standard_process {
        assert!(comparison.previous().meta.started_at < comparison.current().meta.started_at);
    } else {
        assert_eq!(
            comparison.previous().meta.started_at,
            Some(os_model::Timestamp::from_unix_millis(1_800_000_000_000))
        );
        assert_eq!(
            comparison.current().meta.started_at,
            Some(os_model::Timestamp::from_unix_millis(1_800_000_060_000))
        );
    }
    assert_eq!(comparison.previous().meta.crawl_id.to_string(), ids[0]);
    assert_eq!(comparison.current().meta.crawl_id.to_string(), ids[1]);
    assert_eq!(
        auto::saved_counts(&artifacts[0].path(), false).unwrap(),
        change::counts(&comparison, &config.change_detection, false).unwrap()
    );
    let messages = smtp.await.unwrap();
    assert_eq!(messages.len(), 2);
    for message in &messages {
        if standard_process {
            assert!(message.contains("From: configured-sender@example.test"));
            assert!(message.contains("To: receiver@example.test"));
        }
        assert!(message.contains("multipart/alternative"));
        assert!(message.contains("text/plain"));
        assert!(message.contains("text/html"));
        assert!(!message.contains("fixture-secret-never-in-mail"));
    }
    let first_text = smtp_body(&messages[0], "text/plain");
    let second_text = smtp_body(&messages[1], "text/plain");
    let second_html = smtp_body(&messages[1], "text/html");
    assert!(smtp_body(&messages[0], "text/html").contains("kein vorheriger Crawl"));
    assert!(first_text.contains("kein vorheriger Crawl"));
    assert!(!first_text.contains("Previous\tCurrent"));
    assert!(!second_text.contains("kein vorheriger Crawl"));
    let mut manual = fixture::comparison(
        CrawlFile::open(comparison.current().file.path(), OpenMode::ReadOnly).unwrap(),
        CrawlFile::open(comparison.previous().file.path(), OpenMode::ReadOnly).unwrap(),
    );
    change::calculate(&manual, &config.change_detection, &CancelToken::new()).unwrap();
    let expected =
        change::notification::rows(&mut manual, &config.change_detection, false).unwrap();
    assert_eq!(
        expected
            .iter()
            .find(|r| r.label == "Indexability")
            .unwrap()
            .change,
        1
    );
    assert_eq!(
        expected
            .iter()
            .find(|r| r.label == "Page Titles")
            .unwrap()
            .change,
        1
    );
    let expected_lines: Vec<_> = expected
        .iter()
        .map(|row| {
            let pct = row.percent.map(|p| format!("{p:.2}")).unwrap_or_default();
            format!(
                "{}\t{}\t{}\t{}\t{pct}",
                row.label, row.previous, row.current, row.change
            )
        })
        .collect();
    let table = second_text
        .split_once("Issue / Filter\tPrevious\tCurrent\tChange\tChange %\r\n")
        .unwrap()
        .1;
    let actual_lines: Vec<_> = table.split_once("URLs:").unwrap().0.lines().collect();
    assert_eq!(actual_lines, expected_lines);
    for row in expected {
        let pct = row.percent.map(|p| format!("{p:.2}")).unwrap_or_default();
        assert!(second_html.contains(&format!(
            "</td><td>{}</td><td>{}</td><td>{}</td><td>{pct}</td></tr>",
            row.previous, row.current, row.change
        )));
        assert!(
            second_text.contains(&format!(
                "{}\t{}\t{}\t{}\t{pct}\r\n",
                row.label, row.previous, row.current, row.change
            )),
            "missing mail row: {row:?}"
        );
    }
}

/// Lokaler SMTP-Empfänger: prüft den tatsächlichen DATA-Inhalt, kein Transport-Mock.
async fn smtp_fixture(require_auth: bool) -> (u16, tokio::task::JoinHandle<Vec<String>>) {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        tokio::time::timeout(std::time::Duration::from_secs(30), async move {
            let mut messages = Vec::new();
            while messages.len() < 2 {
                let (socket, _) = listener.accept().await.unwrap();
                let mut stream = BufReader::new(socket);
                stream
                    .get_mut()
                    .write_all(b"220 fixture ESMTP\r\n")
                    .await
                    .unwrap();
                let mut authenticated = false;
                loop {
                    let mut line = String::new();
                    if stream.read_line(&mut line).await.unwrap() == 0 {
                        break;
                    }
                    let reply = if line.starts_with("EHLO") {
                        "250-fixture\r\n250-AUTH PLAIN\r\n250 8BITMIME\r\n"
                    } else if line.starts_with("AUTH PLAIN ") {
                        // Base64(\0fixture-user\0fixture-secret-never-in-mail).
                        assert_eq!(
                            line.trim(),
                            "AUTH PLAIN AGZpeHR1cmUtdXNlcgBmaXh0dXJlLXNlY3JldC1uZXZlci1pbi1tYWls"
                        );
                        authenticated = true;
                        "235 authenticated\r\n"
                    } else if line.starts_with("DATA") {
                        assert!(!require_auth || authenticated);
                        stream
                            .get_mut()
                            .write_all(b"354 send mail\r\n")
                            .await
                            .unwrap();
                        let mut data = String::new();
                        loop {
                            let mut part = String::new();
                            assert!(stream.read_line(&mut part).await.unwrap() > 0);
                            if part == ".\r\n" {
                                break;
                            }
                            data.push_str(part.strip_prefix('.').unwrap_or(&part));
                        }
                        messages.push(data);
                        "250 accepted\r\n"
                    } else if line.starts_with("QUIT") {
                        stream.get_mut().write_all(b"221 bye\r\n").await.unwrap();
                        break;
                    } else {
                        "250 ok\r\n"
                    };
                    stream.get_mut().write_all(reply.as_bytes()).await.unwrap();
                }
            }
            messages
        })
        .await
        .expect("SMTP fixture timed out")
    });
    (port, handle)
}

/// Decodiert den tatsächlich übertragenen Quoted-Printable-MIME-Part.
fn smtp_body(message: &str, mime: &str) -> String {
    let part = message
        .split(&format!("Content-Type: {mime}"))
        .nth(1)
        .unwrap();
    let (headers, body) = part.split_once("\r\n\r\n").unwrap();
    assert!(headers.contains("quoted-printable"));
    let body = body.split("\r\n--").next().unwrap().as_bytes();
    let mut decoded = Vec::new();
    let mut n = 0;
    while n < body.len() {
        if body[n..].starts_with(b"=\r\n") {
            n += 3;
        } else if body[n] == b'=' {
            let pair = std::str::from_utf8(&body[n + 1..n + 3]).unwrap();
            decoded.push(u8::from_str_radix(pair, 16).unwrap());
            n += 3;
        } else {
            decoded.push(body[n]);
            n += 1;
        }
    }
    String::from_utf8(decoded).unwrap()
}
