//! Completion integration against real crawl files and loopback SMTP.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "runtime_common.rs"]
mod common;
use os_app::{
    pool::CrawlKey,
    runtime::{lifecycle_bus::CrawlEvent, sessions::SessionInfo},
    App, AppConfig,
};
use os_fetch::secrets::MemorySecretStore;
use os_model::{CrawlConfig, CrawlState};
use os_sched::mail::{self, Account, AccountStore, Auth, Completion, Tls};
use os_store::{CrawlFile, OpenMode};
use os_testsite::mocks::smtp::{Config, SmtpMock};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
fn populate(path: &std::path::Path, current: bool) {
    let f = CrawlFile::open(path, OpenMode::ReadWrite).unwrap();
    f.set_meta(
        "started_at",
        if current {
            "2026-10-21T00:00:00.000Z"
        } else {
            "2026-10-20T00:00:00.000Z"
        },
    )
    .unwrap();
    f.set_meta(
        "ended_at",
        if current {
            "2026-10-21T00:00:02.000Z"
        } else {
            "2026-10-20T00:00:02.000Z"
        },
    )
    .unwrap();
    f.set_meta("state", "Completed").unwrap();
    let names = if current {
        vec!["a", "i", "new1", "new2", "new3"]
    } else {
        vec!["a", "i", "old1", "old2"]
    };
    for (id, name) in names.iter().enumerate() {
        let url = format!("https://site.test/{name}");
        f.conn().execute("INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length,content_class,status_code,title_count,title_1,indexability,indexability_status) VALUES (?1,?2,?2,'https://site.test',1,'done','hyperlink',0,30,'HTML',200,1,?3,?4,?5)",rusqlite::params![id+1,url,if current && *name=="a"{"Changed"}else{"Original"},if current && *name=="i"{"Non-Indexable"}else{"Indexable"},if current && *name=="i"{"noindex"}else{""}]).unwrap();
    }
}
fn save_account(dir: &std::path::Path, mock: &SmtpMock, secrets: &MemorySecretStore) {
    AccountStore::new(dir)
        .save(
            &Account {
                id: "fixture".into(),
                label: "Mock".into(),
                host: "localhost".into(),
                port: mock.addr.port(),
                tls: Tls::None,
                username: "crawler".into(),
                from: "crawler@site.test".into(),
                auth: Auth::Password,
                allow_insecure: true,
                client_id: None,
            },
            Some("fixture-password"),
            secrets,
        )
        .unwrap();
}
fn populate_comparison(path: &std::path::Path, side: &str, fixture: &Value) {
    let file = CrawlFile::open(path, OpenMode::ReadWrite).unwrap();
    file.set_meta("state", "Completed").unwrap();
    let mut cfg = file.config().unwrap();
    cfg.compare.change_detection.elements = os_model::ChangeElement::ALL.to_vec();
    cfg.compare.change_detection.content_threshold_pct =
        fixture["content_threshold_pct"].as_u64().unwrap() as u8;
    cfg.extraction.structured_data.json_ld = true;
    file.set_config_with_clock(&cfg, &os_model::ManualClock::new(common::created_at()))
        .unwrap();
    for (index, row) in fixture["rows"].as_array().unwrap().iter().enumerate() {
        if row["only"].as_str().is_some_and(|only| only != side) {
            continue;
        }
        let mut values = json!({"title_1":"Original", "meta_desc_1":"Original", "h1_1":"Original",
            "word_count":10, "crawl_depth":1, "indexability":"Indexable", "indexability_status":"",
            "is_internal":true, "state":"done", "content_class":"HTML", "status_code":200});
        for overlay in [&row["both"], &row[side]] {
            if let Some(fields) = overlay.as_object() {
                for (key, value) in fields {
                    values[key] = value.clone();
                }
            }
        }
        let id = index as i64 + 1;
        let url = format!("https://site.test/{}", row["name"].as_str().unwrap());
        file.conn().execute("INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length,content_class,status_code,title_count,title_1,indexability,indexability_status,meta_desc_1,h1_1,word_count,crawl_depth) VALUES (?1,?2,?2,'https://site.test',?3,?4,'hyperlink',0,30,?5,?6,1,?7,?8,?9,?10,?11,?12,?13)", rusqlite::params![id,url,values["is_internal"].as_bool().unwrap(),values["state"].as_str(),values["content_class"].as_str(),values["status_code"].as_i64(),values["title_1"].as_str(),values["indexability"].as_str(),values["indexability_status"].as_str(),values["meta_desc_1"].as_str(),values["h1_1"].as_str(),values["word_count"].as_i64(),values["crawl_depth"].as_i64()]).unwrap();
        for key in [
            "inlinks",
            "unique_inlinks",
            "self_inlinks",
            "outlinks",
            "unique_outlinks",
            "self_outlinks",
            "external_outlinks",
            "unique_external_outlinks",
        ] {
            file.conn()
                .execute(
                    &format!("UPDATE url_counters SET {key}=?1 WHERE url_id=?2"),
                    rusqlite::params![values["counters"][key].as_i64().unwrap_or(0), id],
                )
                .unwrap();
        }
        if let Some(types) = values["types"].as_array() {
            for ty in types {
                // Store both representations, as the parser does: C02 reads the JSON types.
                file.conn().execute("INSERT INTO sd_items(url_id,format,type_path,json) VALUES (?1,'json-ld',?2,?3)", rusqlite::params![id,ty.as_str(),json!({"@type":ty}).to_string()]).unwrap();
            }
        }
        if let Some(changed) = values["signature"].as_u64() {
            let mut sig = vec![0u8; 512];
            sig[..changed as usize * 4].fill(1);
            file.conn()
                .execute(
                    "INSERT INTO minhash(url_id,sig) VALUES (?1,?2)",
                    rusqlite::params![id, sig],
                )
                .unwrap();
        }
    }
}

// spec: OUT-282
#[test]
fn overview_and_all_comparison_rows_match_manual_queries_and_fixture_golden() {
    let dir = tempfile::tempdir().unwrap();
    let previous = common::make_crawl(dir.path(), "previous.ospider");
    let current = common::make_crawl(dir.path(), "current.ospider");
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/out-m2-mail/compare.json"
    ))
    .unwrap();
    populate_comparison(&previous, "previous", &fixture);
    populate_comparison(&current, "current", &fixture);
    let data = os_analyze::mail_completion(&current, Some(&previous), true, false).unwrap();
    let completion: Completion = serde_json::from_value(data.clone()).unwrap();
    let f = CrawlFile::open(&current, OpenMode::ReadOnly).unwrap();
    assert_eq!(
        completion.overview,
        os_store::queries::app_support::overview_summary(f.conn()).unwrap()
    );
    let cancel = os_model::CancelToken::new();
    let mut cmp = os_analyze::compare::Comparison::open(
        os_analyze::compare::CrawlSide::open(&current, dir.path()).unwrap(),
        os_analyze::compare::CrawlSide::open(&previous, dir.path()).unwrap(),
        &[],
        &cancel,
    )
    .unwrap();
    // Every available manual overview row must be identical, not just New/Missing.
    let manual = cmp.overview(&cancel).unwrap().into_iter().filter_map(|row| Some(json!({
        "name":format!("{} / {}",row.tab,row.label), "previous":row.previous?, "current":row.current?}))).collect::<Vec<_>>();
    let changes = data["changes"].as_array().unwrap();
    assert_eq!(&changes[..manual.len()], manual);
    let p = cmp.pairing();
    assert_eq!(
        changes[manual.len()]["current"],
        (&p.current - &p.paired_current).len()
    );
    assert_eq!(
        changes[manual.len() + 1]["previous"],
        (&p.previous - &p.paired_previous).len()
    );
    // C02's manual Change Detection interface is still a stub on this branch.
    // Independently specified golden covers every element until that interface is available.
    let golden: Vec<Value> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/out-m2-mail/compare-counts.json"
    ))
    .unwrap();
    assert_eq!(&changes[manual.len()..], golden);
    let text = completion.render().0;
    let golden_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/out-m2-mail/compare.txt");
    let golden_text = format!(
        "{}\n",
        text.lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
    );
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&golden_path, &golden_text).unwrap();
    }
    assert_eq!(golden_text, std::fs::read_to_string(golden_path).unwrap());
    for row in golden.iter().skip(2) {
        assert!(
            text.contains(&format!("{} | 0 | 1 | +1", row["name"].as_str().unwrap())),
            "{row}"
        );
    }
    // Check actual displayed counters on both self-link-only changes in both crawls.
    for side in [cmp.current(), cmp.previous()] {
        for id in [4, 5] {
            let counters: (i64,i64,i64,i64) = side.file.conn().query_row(
                "SELECT c.inlinks-c.self_inlinks,c.unique_inlinks-(c.self_inlinks>0),c.outlinks-c.self_outlinks,c.unique_outlinks-(c.self_outlinks>0) FROM url_counters c WHERE url_id=?1", [id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
            assert_eq!(counters, (0, 0, 0, 0));
        }
    }
    assert!(os_analyze::mail_completion(&current, None, true, false).unwrap()["changes"].is_null());
    assert!(
        os_analyze::mail_completion(&current, Some(&previous), false, false).unwrap()["changes"]
            .is_null()
    );
    // Identical crawls: skip_empty suppresses all zero rows; disabling it preserves them.
    let same: Completion = serde_json::from_value(
        os_analyze::mail_completion(&current, Some(&current), true, true).unwrap(),
    )
    .unwrap();
    assert!(!same.render().0.contains("Page Titles |"));
    let same: Completion = serde_json::from_value(
        os_analyze::mail_completion(&current, Some(&current), true, false).unwrap(),
    )
    .unwrap();
    assert!(same.render().0.contains("Page Titles | 0 | 0 | +0"));
}
// spec: OUT-281
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ui_completion_sends_once_after_analysis_and_reopening_sends_nothing() {
    let mock = SmtpMock::start(Config::default()).await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecretStore::new());
    let app = App::new(AppConfig::new(dir.path()).with_secrets(secrets.clone())).unwrap();
    save_account(&app.config().paths.settings, &mock, &secrets);
    app.dispatch(common::ctx(),"settings.set",json!({"patch":{"accounts":{"notifyAccount":"fixture","notifyTo":["recipient@site.test"],"notifyOnSuccess":true},"network":{"proxy":{"mode":"none"}}}})).await.unwrap();
    let path = common::make_crawl(dir.path(), "ui.ospider");
    populate(&path, true);
    let key = CrawlKey("fixture-ui".into());
    app.read_pool().register(key.clone(), &path);
    app.sessions().set_current(
        common::ctx().window,
        SessionInfo {
            key: key.clone(),
            path,
            config: Arc::new(CrawlConfig::default()),
            state: CrawlState::Completed,
            writable: true,
            engine: None,
        },
    );
    app.crawl_events()
        .emit(&app, CrawlEvent::Opened(key.clone()));
    assert!(mock.messages().is_empty());
    app.crawl_events()
        .emit(&app, CrawlEvent::Completed(key.clone()));
    assert!(
        mock.messages().is_empty(),
        "mail waits for automatic analysis"
    );
    os_app::commands::mail::after_analysis(&app, &key);
    assert!(common::eventually(Duration::from_secs(5), || mock.messages().len() == 1).await);
    app.crawl_events()
        .emit(&app, CrawlEvent::Completed(key.clone()));
    os_app::commands::mail::after_analysis(&app, &key);
    app.crawl_events().emit(&app, CrawlEvent::Opened(key));
    assert_eq!(mock.messages().len(), 1);
    let messages = mock.messages();
    assert!(messages[0].data.contains("Total URLs Crawled: 5"));
}
// spec: OUT-281
#[tokio::test]
async fn failed_task_sends_and_removing_all_recipients_keeps_task_valid() {
    let mock = SmtpMock::start(Config::default()).await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let secrets = MemorySecretStore::new();
    save_account(dir.path(), &mock, &secrets);
    let mut settings = os_model::settings::AppSettings::default();
    settings.network.proxy.mode = os_model::ProxyMode::None;
    settings.save(dir.path()).unwrap();
    let store = os_sched::tasks::TaskStore::in_settings(dir.path());
    let path = store.path("nightly").unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        path,
        include_str!("../../../tests/fixtures/out-m2-tasks/nightly.ostask"),
    )
    .unwrap();
    let mut task = store.get("nightly").unwrap();
    task.notify.account = Some("fixture".into());
    let mut record =
        os_sched::history::RunRecord::start("run", "nightly", "Nightly", common::created_at());
    record.message = Some("fixture failure before crawl".into());
    record.finish(
        os_sched::history::RunResult::Failed,
        Some(3),
        common::created_at(),
    );
    assert!(mail::notify_task(
        dir.path(),
        dir.path(),
        &secrets,
        &task,
        &record,
        Completion::default()
    )
    .await
    .unwrap());
    assert!(mock.messages()[0].data.contains("[FAILED]"));
    assert!(mock.messages()[0]
        .data
        .contains("fixture failure before crawl"));
    task.notify.to.clear();
    task.notify.account = None;
    assert!(!mail::notify_task(
        dir.path(),
        dir.path(),
        &secrets,
        &task,
        &record,
        Completion::default()
    )
    .await
    .unwrap());
    let app = App::new(
        AppConfig::new(dir.path().join("app")).with_secrets(Arc::new(MemorySecretStore::new())),
    )
    .unwrap();
    let mut def: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/out-m2-tasks/task-def-basic.json"
    ))
    .unwrap();
    def["notify"] = json!({"onSuccess":true,"onFailure":true,"account":null,"to":[]});
    let issues = app
        .dispatch(common::ctx(), "tasks.validate", json!({"task":def}))
        .await
        .unwrap();
    assert!(
        !issues
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["field"].as_str().unwrap_or("").starts_with("notify")),
        "{issues}"
    );
}

// spec: OUT-281
// spec: OUT-282
#[tokio::test]
async fn retained_summary_can_be_mailed_after_a_temporary_crawl_is_discarded() {
    let mock = SmtpMock::start(Config::default()).await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let secrets = MemorySecretStore::new();
    save_account(dir.path(), &mock, &secrets);
    let path = common::make_crawl(dir.path(), "discarded.ospider");
    populate(&path, true);
    let snapshot = os_analyze::mail_completion(&path, None, false, false).unwrap();
    std::fs::remove_file(path).unwrap();
    let completion: Completion = serde_json::from_value(snapshot).unwrap();
    let mut network = os_model::settings::NetworkSettings::default();
    network.proxy.mode = os_model::ProxyMode::None;
    let policy = mail::Policy {
        account: Some("fixture".into()),
        on_success: true,
        to: vec!["recipient@site.test".into()],
        ..Default::default()
    };
    assert!(
        mail::notify(dir.path(), &secrets, &network, &policy, &completion)
            .await
            .unwrap()
    );
    assert!(mock.messages()[0].data.contains("Total URLs Crawled: 5"));
}
