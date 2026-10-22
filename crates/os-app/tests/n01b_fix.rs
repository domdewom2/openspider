#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "session_common.rs"]
mod common;
use common::*;
use serde_json::json;
use std::time::Duration;

// spec: INTEG-002 [M2]
#[tokio::test]
async fn encrypted_profile_roundtrip_authenticates_before_importing_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let secrets = std::sync::Arc::new(os_fetch::MemorySecretStore::new());
    secrets.set("api/psi", "sk-TEST-INTEG-002");
    let app = os_app::App::new(os_app::AppConfig::new(dir.path()).with_secrets(secrets)).unwrap();
    let config = os_model::CrawlConfig::load_str(include_str!(
        "../../../tests/fixtures/config/commented.osconfig"
    ))
    .unwrap()
    .config;
    call(&app, "profile.save", json!({"name":"source","config":{"schemaVersion":config.schema_version,"generator":config.generator,"config":config},"overwrite":false})).await;
    let expected = call(&app, "profile.load", json!({"name":"source"})).await["config"].clone();
    let path = dir.path().join("roundtrip.osconfig");
    call(
        &app,
        "profile.export",
        json!({"name":"source","path":path,"includeSecrets":true,"password":"fixture-password"}),
    )
    .await;
    let dest = tempfile::tempdir().unwrap();
    let restored = std::sync::Arc::new(os_fetch::MemorySecretStore::new());
    let reader =
        os_app::App::new(os_app::AppConfig::new(dest.path()).with_secrets(restored.clone()))
            .unwrap();
    for password in [None, Some("wrong")] {
        assert!(reader
            .dispatch(
                ctx(),
                "profile.import",
                json!({"path":path,"password":password})
            )
            .await
            .is_err());
        assert_eq!(
            os_fetch::SecretStore::try_get(restored.as_ref(), "api/psi").unwrap(),
            None
        );
        assert_eq!(call(&reader, "profile.list", json!(null)).await, json!([]));
    }
    let mut bytes = std::fs::read(&path).unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    let tampered = dir.path().join("tampered.osconfig");
    std::fs::write(&tampered, bytes).unwrap();
    assert!(reader
        .dispatch(
            ctx(),
            "profile.import",
            json!({"path":tampered,"password":"fixture-password"})
        )
        .await
        .is_err());
    assert_eq!(
        os_fetch::SecretStore::try_get(restored.as_ref(), "api/psi").unwrap(),
        None
    );
    call(
        &reader,
        "profile.import",
        json!({"path":path,"password":"fixture-password"}),
    )
    .await;
    assert_eq!(
        os_fetch::SecretStore::try_get(restored.as_ref(), "api/psi")
            .unwrap()
            .as_deref(),
        Some("sk-TEST-INTEG-002")
    );
    assert_eq!(
        call(&reader, "profile.load", json!({"name":"roundtrip"})).await["config"],
        expected
    );
    let stored = std::fs::read_to_string(
        reader
            .config()
            .paths
            .settings
            .join("profiles/roundtrip.osconfig"),
    )
    .unwrap();
    assert!(!stored.contains("sk-TEST-INTEG-002"));
}

// spec: CRAWL-126 [M2]
// spec: TECH-068
#[tokio::test]
async fn config_failure_after_recompute_rolls_back_facts_config_and_analysis() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let file = create(&dir.path().join("rollback.ospider"), cfg());
    file.conn()
        .execute_batch(include_str!(
            "../../../tests/fixtures/out-m2-export/crawl.sql"
        ))
        .unwrap();
    file.conn()
        .execute_batch("UPDATE urls SET effective_index=0; INSERT OR REPLACE INTO analysis_steps(step,state,finished_at,config_hash) VALUES('links','done','fixture-time','fixture-hash');")
        .unwrap();
    file.set_meta("state", "Completed").unwrap();
    open(&app, file.path()).await;
    let snapshot = || {
        let mut stmt = file.conn().prepare("SELECT json_array(id,in_tables,indexability,indexability_status,change_seq) FROM urls ORDER BY id").unwrap();
        let facts = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let mut stmt = file.conn().prepare("SELECT json_array(step,state,finished_at,config_hash) FROM analysis_steps ORDER BY step").unwrap();
        let stamps = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        (
            file.config().unwrap(),
            facts,
            stamps,
            file.get_meta("change_seq").unwrap(),
            file.get_meta("analysis_state").unwrap(),
            file.get_meta("analysis_stale").unwrap(),
        )
    };
    file.set_meta("analysis_state", "completed").unwrap();
    let before = snapshot();
    // Fails only after D06 changed facts and stamps: config storage is the later write.
    file.conn().execute_batch("CREATE TRIGGER fail_config BEFORE UPDATE ON crawl_config BEGIN SELECT RAISE(ABORT,'fixture failure after recompute'); END;").unwrap();
    let mut after = before.0.clone();
    after.advanced.respect_noindex = !after.advanced.respect_noindex;
    let args =
        json!({"schemaVersion":after.schema_version,"generator":after.generator,"config":after});
    assert!(app
        .dispatch(ctx(), "config.set", args.clone())
        .await
        .is_err());
    assert_eq!(snapshot(), before);
    file.conn()
        .execute_batch("DROP TRIGGER fail_config")
        .unwrap();
    call(&app, "config.set", args).await;
    assert_ne!(
        snapshot().1,
        before.1,
        "the fixture really exercises recomputation"
    );
}

// spec: INTEG-082 [M2]
#[test]
fn cancelled_outer_transaction_rolls_back_nested_fact_updates() {
    let dir = tempfile::tempdir().unwrap();
    let mut file = create(&dir.path().join("cancel.ospider"), cfg());
    file.conn()
        .execute_batch(include_str!(
            "../../../tests/fixtures/out-m2-export/crawl.sql"
        ))
        .unwrap();
    let before = file.config().unwrap();
    let sequence = file.get_meta("change_seq").unwrap();
    file.set_meta("analysis_state", "completed").unwrap();
    let cancel = os_model::CancelToken::new();
    let result: Result<(), os_store::StoreError> = os_store::profile::config_transaction(
        &mut file,
        |e| e,
        |file| {
            os_store::writer::apply_fact_updates(
                file,
                &os_store::writer::FactUpdates {
                    word_count: vec![(os_model::UrlId(1), Some(42))],
                    ..Default::default()
                },
            )?;
            let mut changed = before.clone();
            changed.speed.threads += 1;
            file.set_config(&changed)?;
            file.set_meta("analysis_state", "stale")?;
            cancel.cancel();
            cancel
                .checkpoint()
                .map_err(|_| os_store::StoreError::InvalidInput("cancelled".into()))
        },
    );
    assert!(result.is_err());
    assert_eq!(file.config().unwrap(), before);
    assert_eq!(
        file.get_meta("analysis_state").unwrap().as_deref(),
        Some("completed")
    );
    assert_eq!(file.get_meta("change_seq").unwrap(), sequence);
    assert_eq!(
        file.conn()
            .query_row("SELECT word_count FROM urls WHERE id=1", [], |r| r
                .get::<_, Option<u32>>(0))
            .unwrap(),
        None
    );
}

// spec: CRAWL-122, CRAWL-126 [M2]
#[tokio::test]
async fn paused_crawl_keeps_proxy_a_and_new_crawl_uses_b() {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    async fn proxy(
        seen: Arc<std::sync::Mutex<Vec<String>>>,
    ) -> (u16, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let count = Arc::new(AtomicUsize::new(0));
        let requests = count.clone();
        let task = tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let requests = requests.clone();
                let seen = seen.clone();
                tokio::spawn(async move {
                    let mut buf = [0; 8192];
                    let n = socket.read(&mut buf).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    seen.lock()
                        .unwrap()
                        .push(String::from_utf8_lossy(&buf[..n]).to_lowercase());
                    requests.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    let body = include_str!("../../../tests/fixtures/ui/fx-ui-basic/index.html");
                    let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        });
        (port, count, task)
    }
    async fn wait_requests(count: &AtomicUsize, minimum: usize) {
        tokio::time::timeout(Duration::from_secs(15), async {
            while count.load(Ordering::SeqCst) < minimum {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    }
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let (a, count_a, task_a) = proxy(seen.clone()).await;
    let (b, count_b, task_b) = proxy(Default::default()).await;
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(os_fetch::MemorySecretStore::new());
    secrets.set("fixture/auth", "pass");
    secrets.set(os_fetch::net::PROXY_PASSWORD, "start-pass");
    let app =
        os_app::App::new(os_app::AppConfig::new(dir.path()).with_secrets(secrets.clone())).unwrap();
    let mut config = cfg();
    config.speed.threads = 1;
    config.robots.mode = os_model::RobotsMode::Ignore;
    config.spider.external.crawl = false;
    call(
        &app,
        "config.set",
        json!({"schemaVersion":config.schema_version,"generator":config.generator,"config":config}),
    )
    .await;
    let proxy_settings = |port| json!({"patch":{"network":{"proxy":{"mode":"manual","kind":"http","host":"127.0.0.1","port":port,"username":"proxy-user"}}}});
    call(&app, "settings.set", proxy_settings(a)).await;
    let start = json!({"source":{"kind":"spider","url":"http://fixture.test/"},"scope":"domain","profile":null});
    call(&app, "crawl.start", start.clone()).await;
    wait_requests(&count_a, 2).await;
    call(&app, "crawl.pause", json!(null)).await;
    let paused = count_a.load(Ordering::SeqCst);
    call(&app, "settings.set", proxy_settings(b)).await;
    secrets.set(os_fetch::net::PROXY_PASSWORD, "changed-pass");
    config.user_agent.http = Some("Resume-Fixture".into());
    config.http.headers.push(os_model::HeaderEntry {
        name: "X-Resume".into(),
        value: "updated".into(),
        host: None,
    });
    config.auth.basic.push(os_model::BasicAuth {
        prefix: "http://fixture.test/".into(),
        username: "user".into(),
        secret_ref: "fixture/auth".into(),
    });
    call(
        &app,
        "config.set",
        json!({"schemaVersion":config.schema_version,"generator":config.generator,"config":config}),
    )
    .await;
    call(&app, "crawl.resume", json!(null)).await;
    wait_requests(&count_a, paused + 1).await;
    call(&app, "crawl.pause", json!(null)).await;
    assert_eq!(count_b.load(Ordering::SeqCst), 0);
    for request in &seen.lock().unwrap()[paused..] {
        assert!(request.contains("user-agent: resume-fixture"), "{request}");
        assert!(request.contains("x-resume: updated"), "{request}");
        assert!(
            request.contains("authorization: basic dxnlcjpwyxnz"),
            "{request}"
        );
        assert!(
            request.contains("proxy-authorization: basic chjvehktdxnlcjpzdgfydc1wyxnz"),
            "{request}"
        );
    }
    call(&app, "crawl.start", start).await;
    wait_requests(&count_b, 1).await;
    call(&app, "crawl.pause", json!(null)).await;
    task_a.abort();
    task_b.abort();
}

// spec: INTEG-002 [M2], OUT-221
#[tokio::test]
async fn failed_secret_import_restores_values_and_can_be_retried() {
    use os_fetch::secrets::{SecretBackend, SecretError};
    use os_fetch::{SecretStore, SecretStoreMut};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    #[derive(Default)]
    struct FailSecond {
        values: os_fetch::MemorySecretStore,
        writes: AtomicUsize,
    }
    impl SecretStore for FailSecond {
        fn get(&self, key: &str) -> Option<String> {
            self.values.get(key)
        }
    }
    impl SecretStoreMut for FailSecond {
        fn set(&self, key: &str, value: &str) -> Result<(), SecretError> {
            self.values.set(key, value);
            if self.writes.fetch_add(1, Ordering::SeqCst) == 1 {
                return Err(SecretError::new("fixture write failure"));
            }
            Ok(())
        }
        fn delete(&self, key: &str) -> Result<(), SecretError> {
            SecretStoreMut::delete(&self.values, key)
        }
        fn backend(&self) -> SecretBackend {
            SecretBackend::Memory
        }
    }
    for existing in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let source = Arc::new(os_fetch::MemorySecretStore::new());
        source.set("api/ga4", "new-ga4");
        source.set("api/psi", "new-psi");
        let app =
            os_app::App::new(os_app::AppConfig::new(dir.path()).with_secrets(source)).unwrap();
        let config = os_model::CrawlConfig::load_str(include_str!(
            "../../../tests/fixtures/config/commented.osconfig"
        ))
        .unwrap()
        .config;
        call(&app, "profile.save", json!({"name":"source","config":{"schemaVersion":config.schema_version,"generator":config.generator,"config":config},"overwrite":false})).await;
        let path = dir.path().join("retry.osconfig");
        call(&app, "profile.export", json!({"name":"source","path":path,"includeSecrets":true,"password":"fixture-password"})).await;
        let dest = tempfile::tempdir().unwrap();
        let secrets = Arc::new(FailSecond::default());
        if existing {
            secrets.values.set("api/ga4", "old-ga4");
            secrets.values.set("api/psi", "old-psi");
        }
        let reader =
            os_app::App::new(os_app::AppConfig::new(dest.path()).with_secrets(secrets.clone()))
                .unwrap();
        let args = json!({"path":path,"password":"fixture-password"});
        assert!(reader
            .dispatch(ctx(), "profile.import", args.clone())
            .await
            .is_err());
        assert_eq!(call(&reader, "profile.list", json!(null)).await, json!([]));
        for (key, old) in [("api/ga4", "old-ga4"), ("api/psi", "old-psi")] {
            assert_eq!(secrets.get(key), existing.then(|| old.to_owned()));
        }
        call(&reader, "profile.import", args).await;
        assert_eq!(secrets.get("api/ga4").as_deref(), Some("new-ga4"));
        assert_eq!(secrets.get("api/psi").as_deref(), Some("new-psi"));
        assert_eq!(
            call(&reader, "profile.list", json!(null))
                .await
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }
}
