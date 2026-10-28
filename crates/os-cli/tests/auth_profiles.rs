//! Machine-bound CLI profiles use the injected store, never the OS keychain.
use std::path::{Path, PathBuf};
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/out-m2-cli")
        .join(name)
}

// spec: OUT-187
// spec: OUT-223
// spec: CRAWL-121
#[test]
fn machine_bound_profiles_use_only_the_supplied_secret_store_and_missing_keys_fail() {
    let dir = tempfile::tempdir().unwrap();
    let paths = os_store::paths::app_paths_with_home(Some(dir.path().as_os_str()));
    let mut profile: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixture("profile.json")).unwrap()).unwrap();
    profile["profile"]["host"] = serde_json::json!("https://site.test/");
    let key = os_store::auth_profiles::new_key();
    let bytes = os_store::auth_profiles::seal(
        &profile,
        os_store::auth_profiles::Protection::Machine {
            key_ref: "auth/fixture",
            key: &key,
        },
    )
    .unwrap();
    let file = dir.path().join("machine.osauth");
    std::fs::write(&file, bytes).unwrap();
    let mut cfg = os_model::CrawlConfig::default();
    cfg.auth
        .profile_refs
        .push(file.to_string_lossy().into_owned());
    let store = os_fetch::MemorySecretStore::new();
    let rep = os_cli::reporter::Reporter::new(false);
    assert!(os_cli::auth::load_with(&cfg, &paths, &rep, &store, None)
        .unwrap_err()
        .message
        .contains("Schlüssel"));
    store.set("auth/fixture", &key);
    let session = os_cli::auth::load_with(&cfg, &paths, &rep, &store, None).unwrap();
    assert!(!format!("{session:?}").contains("fixture-secret"));
    let envelope = os_store::auth_profiles::seal(
        &profile,
        os_store::auth_profiles::Protection::Machine {
            key_ref: "auth/fixture",
            key: &key,
        },
    )
    .unwrap();
    std::fs::write(&file, envelope).unwrap();
    let args = os_cli::RunArgs {
        crawl: Some("https://site.test/".into()),
        auth_config: Some(file),
        ..Default::default()
    };
    let plan =
        os_cli::plan::build_for_task_with_secrets(&args, paths, &rep, Default::default(), &store)
            .unwrap();
    assert!(!serde_json::to_string(&plan.config)
        .unwrap()
        .contains("fixture-secret"));
}

struct NonceIds;
impl os_model::IdSource for NonceIds {
    fn new_crawl_id(&self, _: os_model::Timestamp) -> os_model::CrawlId {
        "019b76da-a800-7000-8081-828384858687".parse().unwrap()
    }
}
fn clock() -> std::sync::Arc<dyn os_model::Clock> {
    std::sync::Arc::new(os_model::ManualClock::new(
        os_model::Timestamp::from_unix_millis(1_767_225_600_000),
    ))
}

// spec: OUT-187
// spec: OUT-223
// spec: CRAWL-121
#[tokio::test]
async fn imported_digest_and_ntlm_profiles_unlock_fixtures_without_basic_or_cross_host_leaks() {
    use os_fetch::{Fetch, FetchOutcome};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let vectors: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixture("challenges.json")).unwrap()).unwrap();
    for scheme in ["digest", "ntlm"] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/", listener.local_addr().unwrap());
        let expected = vectors[scheme].clone();
        let ntlm = scheme == "ntlm";
        let server = tokio::spawn(async move {
            let mut connections = tokio::task::JoinSet::new();
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let expected = expected.clone();
                connections.spawn(async move {
                    let mut stream = BufReader::new(stream);
                    let mut stage = 0;
                    loop {
                        let mut line = String::new();
                        if stream.read_line(&mut line).await.unwrap() == 0 { break; }
                        let foreign = line.contains(" /foreign ");
                        let mut authorization = None;
                        loop {
                            line.clear();
                            stream.read_line(&mut line).await.unwrap();
                            if line == "\r\n" { break; }
                            if let Some((name, value)) = line.split_once(':') {
                                if name.eq_ignore_ascii_case("authorization") { authorization = Some(value.trim().to_owned()); }
                            }
                        }
                        // Only an exact independently generated proof/MIC is accepted.
                        let authenticated = (!ntlm || stage == 2) && authorization.as_deref() == expected["authorization"].as_str();
                        let challenge = if ntlm && stage == 0 { "NTLM" } else { expected["challenge"].as_str().unwrap() };
                        if ntlm && stage == 1 {
                            assert_eq!(authorization.as_deref(), expected["negotiate"].as_str());
                        }
                        if stage == 0 { assert!(authorization.is_none(), "credentials before challenge: {authorization:?}"); }
                        let (status, extra) = if foreign { assert!(authorization.is_none()); (200, String::new()) }
                            else if authenticated { (200, String::new()) }
                            else { (401, format!("WWW-Authenticate: {challenge}\r\n")) };
                        let body = "protected fixture";
                        stream.get_mut().write_all(format!("HTTP/1.1 {status} Fixture\r\n{extra}Content-Length: {}\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
                        stage += 1;
                    }
                });
            }
        });
        let dir = tempfile::tempdir().unwrap();
        let paths = os_store::paths::app_paths_with_home(Some(dir.path().as_os_str()));
        let mut profile: serde_json::Value =
            serde_json::from_slice(&std::fs::read(fixture("profile.json")).unwrap()).unwrap();
        profile["profile"]["host"] = serde_json::json!(format!("{base}protected"));
        profile["profile"]["kind"] = serde_json::json!(scheme);
        profile["profile"]["username"] = vectors[scheme]["username"].clone();
        profile["profile"]["secret"] = serde_json::json!("Password");
        let key = os_store::auth_profiles::new_key();
        let store = os_fetch::MemorySecretStore::new();
        store.set("fixture/key", &key);
        let path = dir.path().join("imported.osauth");
        std::fs::write(
            &path,
            os_store::auth_profiles::seal(
                &profile,
                os_store::auth_profiles::Protection::Machine {
                    key_ref: "fixture/key",
                    key: &key,
                },
            )
            .unwrap(),
        )
        .unwrap();
        let args = os_cli::RunArgs {
            crawl: Some(format!("{base}protected")),
            auth_config: Some(path),
            ..Default::default()
        };
        // Scheduling and CLI consume this same plan/profile bridge.
        let plan = os_cli::plan::build_for_task_with_secrets(
            &args,
            paths,
            &os_cli::reporter::Reporter::new(false),
            Default::default(),
            &store,
        )
        .unwrap();
        let fetcher = plan
            .auth
            .fetcher_with_ids(&plan.config, clock(), std::sync::Arc::new(NonceIds))
            .unwrap();
        let request = |path: &str| os_fetch::FetchRequest {
            url: format!("{base}{path}").parse().unwrap(),
            purpose: os_fetch::FetchPurpose::Page,
            cookies: None,
            body_limit: Default::default(),
            attempt: 0,
            max_body_bytes: 1024 * 1024,
        };
        let result = fetcher.fetch(request("protected")).await;
        match result.outcome {
            FetchOutcome::Response(response) => assert_eq!(response.status, 200, "{scheme}"),
            other => panic!("{scheme}: {other:?}"),
        }
        assert!(!format!("{:?}", fetcher.render_config()).contains("Password"));
        let foreign = fetcher.fetch(request("foreign")).await;
        assert!(matches!(foreign.outcome, FetchOutcome::Response(ref r) if r.status == 200));
        let other_host = base.replace("127.0.0.1", "localhost");
        let foreign_host = fetcher
            .fetch(os_fetch::FetchRequest {
                url: format!("{other_host}protected").parse().unwrap(),
                ..request("protected")
            })
            .await;
        assert!(matches!(foreign_host.outcome, FetchOutcome::Response(ref r) if r.status == 401));
        // The fixture rejects a structurally valid response made with the wrong password.
        profile["profile"]["secret"] = serde_json::json!("wrong-password");
        let path = args.auth_config.as_ref().unwrap();
        std::fs::write(
            path,
            os_store::auth_profiles::seal(
                &profile,
                os_store::auth_profiles::Protection::Machine {
                    key_ref: "fixture/key",
                    key: &key,
                },
            )
            .unwrap(),
        )
        .unwrap();
        let rep = os_cli::reporter::Reporter::new(false);
        let wrong_session = os_cli::auth::load_with_clock(
            &plan.config,
            &plan.paths,
            &rep,
            &store,
            None,
            clock().as_ref(),
        )
        .unwrap();
        let wrong_fetcher = wrong_session
            .fetcher_with_ids(&plan.config, clock(), std::sync::Arc::new(NonceIds))
            .unwrap();
        let wrong = wrong_fetcher.fetch(request("protected")).await;
        assert!(matches!(wrong.outcome, FetchOutcome::Response(ref r) if r.status == 401));
        server.abort();
    }
}

// spec: OUT-187
// spec: OUT-223
#[test]
fn cookie_expiry_boundaries_use_the_injected_clock() {
    use os_fetch::Fetch;
    let dir = tempfile::tempdir().unwrap();
    let paths = os_store::paths::app_paths_with_home(Some(dir.path().as_os_str()));
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixture("cookie-boundaries.json")).unwrap()).unwrap();
    let path = dir.path().join("cookies.osauth");
    std::fs::write(
        &path,
        os_store::auth_profiles::seal(
            &value,
            os_store::auth_profiles::Protection::Passphrase("fixture"),
        )
        .unwrap(),
    )
    .unwrap();
    let mut config = os_model::CrawlConfig::default();
    config
        .auth
        .profile_refs
        .push(path.to_string_lossy().into_owned());
    let rep = os_cli::reporter::Reporter::new(false);
    let clock = clock();
    let session = os_cli::auth::load_with_clock(
        &config,
        &paths,
        &rep,
        &os_fetch::MemorySecretStore::new(),
        Some("fixture"),
        clock.as_ref(),
    )
    .unwrap();
    let fetcher = session.fetcher(&config, clock).unwrap();
    let jar = fetcher.render_cookie_jar(None).unwrap();
    let header = jar
        .header_for(&"https://site.test/".parse().unwrap())
        .unwrap();
    assert!(header.contains("valid=yes"));
    // The 1-ms cookie is checked through import decisions below, never the wall-clock jar.
    assert!(!header.contains("expired="));
    assert_eq!(rep.warning_messages(), ["authentication cookies expired"]);
    assert!(jar
        .header_for(&"https://foreign.test/".parse().unwrap())
        .is_none());
    let mut near_only = value.clone();
    let secret: serde_json::Value =
        serde_json::from_str(near_only["profile"]["secret"].as_str().unwrap()).unwrap();
    near_only["profile"]["secret"] = serde_json::json!(
        serde_json::json!({"cookies":[secret["cookies"][2].clone()]}).to_string()
    );
    std::fs::write(
        &path,
        os_store::auth_profiles::seal(
            &near_only,
            os_store::auth_profiles::Protection::Passphrase("fixture"),
        )
        .unwrap(),
    )
    .unwrap();
    for (millis, expired) in [(1_767_225_600_000, false), (1_767_225_600_001, true)] {
        let rep = os_cli::reporter::Reporter::new(false);
        os_cli::auth::load_with_clock(
            &config,
            &paths,
            &rep,
            &os_fetch::MemorySecretStore::new(),
            Some("fixture"),
            &os_model::ManualClock::new(os_model::Timestamp::from_unix_millis(millis)),
        )
        .unwrap();
        assert_eq!(!rep.warning_messages().is_empty(), expired);
    }
    // The scheduler passes its clock all the way through plan construction.
    let key = os_store::auth_profiles::new_key();
    let store = os_fetch::MemorySecretStore::new();
    store.set("fixture/cookies", &key);
    std::fs::write(
        &path,
        os_store::auth_profiles::seal(
            &value,
            os_store::auth_profiles::Protection::Machine {
                key_ref: "fixture/cookies",
                key: &key,
            },
        )
        .unwrap(),
    )
    .unwrap();
    let args = os_cli::RunArgs {
        crawl: Some("https://site.test/".into()),
        auth_config: Some(path),
        ..Default::default()
    };
    let rep = os_cli::reporter::Reporter::new(false);
    let task_clock =
        os_model::ManualClock::new(os_model::Timestamp::from_unix_millis(1_767_225_599_999));
    let _plan = os_cli::plan::build_for_task_with_clock(
        &args,
        paths,
        &rep,
        Default::default(),
        &store,
        &task_clock,
    )
    .unwrap();
    assert!(
        rep.warning_messages().is_empty(),
        "one millisecond before the boundary all fixture cookies are valid"
    );
}
