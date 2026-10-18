//! M2-O02: CLI liest Verwaltungsnamen, Projekte, Sperren und verschobene Dateien.
mod common;
use common::{site, Env};
use os_model::{Clock, CrawlId, SystemClock};
use os_store::registry::Manager;
use serde_json::Value;

// spec: CRAWL-128
// spec: CRAWL-143
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn listing_and_load_share_stable_ids_after_rename_lock_and_location_move() {
    let env = Env::new();
    let site = site("out-basic-site").await;
    let run = env
        .run_async(vec![
            "--crawl".to_string(),
            "http://site.test/".into(),
            "--resolve".into(),
            common::resolve(&site),
            "--set".into(),
            "threads=1".into(),
        ])
        .await;
    assert_eq!(run.code, 0, "{}", run.dump());
    let listed = env.run(&["--list-crawls", "--json"]);
    let v: Value = serde_json::from_str(&listed.stdout).unwrap();
    let id = v[0]["id"].as_str().unwrap().parse::<CrawlId>().unwrap();
    let mut m = Manager::open(&env.crawls_dir()).unwrap();
    m.create_project("customer", "Kunde A").unwrap();
    m.rename(&id, "Umbenannt", SystemClock.now()).unwrap();
    m.move_project(&id, Some("customer"), SystemClock.now())
        .unwrap();
    m.lock_project("customer", true).unwrap();
    let dest = tempfile::tempdir().unwrap();
    m.move_file(&id, dest.path(), SystemClock.now(), &|_, _| true)
        .unwrap();
    drop(m);
    let listed = env.run(&["--list-crawls", "--json"]);
    assert_eq!(listed.code, 0, "{}", listed.dump());
    let v: Value = serde_json::from_str(&listed.stdout).unwrap();
    assert_eq!(v[0]["id"], id.to_string());
    assert_eq!(v[0]["name"], "Umbenannt");
    assert_eq!(v[0]["project"], "Kunde A");
    assert_eq!(v[0]["locked"], true);
    let table = env.run(&["--list-crawls"]);
    assert!(table.stdout.contains("yes"));
    let requests = site.total_requests();
    let loaded = env.run(&[
        "--load-crawl",
        &id.to_string(),
        "--export-tabs",
        "Internal:All",
        "--output-folder",
        "out",
    ]);
    assert_eq!(loaded.code, 0, "{}", loaded.dump());
    assert_eq!(site.total_requests(), requests);
    let resume = env.run(&["--load-crawl", &id.to_string(), "--resume"]);
    assert_eq!(resume.code, 1, "{}", resume.dump());
    assert!(resume.stderr.contains("Completed"));
}
