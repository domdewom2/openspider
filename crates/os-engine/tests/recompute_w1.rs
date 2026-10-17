//! Infrastruktur (M2-F05a) für Neuberechnungen ohne Recrawl.
use os_engine::recompute;
use os_model::{CancelToken, CrawlConfig};
use os_store::{CrawlFile, StorageProfile};

// spec: INTEG-082
// Infrastruktur (M2-F05a) für INTEG-082; Fachprüfung M2-D16.
#[test]
fn only_changed_trigger_paths_dispatch_and_cancel_prevents_dispatch() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("recompute.ospider");
    let cfg: CrawlConfig = CrawlConfig::from_toml_str(include_str!(
        "../../../tests/fixtures/config/default.osconfig"
    ))
    .unwrap();
    use os_model::{CrawlMode, IdSource, SeededIds, Timestamp};
    let created_at = Timestamp::from_db("2026-10-17T00:00:00.000Z").unwrap();
    let init = os_store::CrawlInit {
        config: cfg.clone(),
        crawl_id: SeededIds::new(42).new_crawl_id(created_at),
        mode: CrawlMode::Spider,
        start_url: Some("https://site.test/".into()),
        list_source: None,
        project: "fixture".into(),
        name: "recompute".into(),
        created_at,
    };
    let mut file = CrawlFile::create(&path, &init, StorageProfile::Balanced).unwrap();
    let cancel = CancelToken::default();
    assert!(recompute::run_triggered(&mut file, &cfg, &cfg, &cancel)
        .unwrap()
        .ran
        .is_empty());
    for (path, value, owners) in [
        ("/extraction/hash", serde_json::json!(false), vec!["M2-E01"]),
        (
            "/content/area/include_elements",
            serde_json::json!(["main"]),
            vec!["M2-E01", "M2-D02"],
        ),
        (
            "/advanced/respect_noindex",
            serde_json::json!(true),
            vec!["M2-D06"],
        ),
        (
            "/advanced/respect_canonical",
            serde_json::json!(true),
            vec!["M2-D06"],
        ),
        (
            "/advanced/respect_self_ref_meta_refresh",
            serde_json::json!(true),
            vec!["M2-D06"],
        ),
    ] {
        let mut json = serde_json::to_value(&cfg).unwrap();
        assert_ne!(
            json.pointer(path).unwrap(),
            &value,
            "Fixture muss den Auslöser ändern: {path}"
        );
        *json.pointer_mut(path).unwrap() = value;
        let changed = serde_json::from_value(json).unwrap();
        assert_eq!(
            recompute::run_triggered(&mut file, &cfg, &changed, &cancel)
                .unwrap()
                .ran,
            owners,
            "{path}"
        );
    }
    let mut next = cfg.clone();
    next.link_positions.rules.pop();
    assert_eq!(
        recompute::run_triggered(&mut file, &cfg, &next, &cancel)
            .unwrap()
            .ran,
        vec!["M2-D16"]
    );
    cancel.cancel();
    assert!(matches!(
        recompute::run_triggered(&mut file, &cfg, &next, &cancel),
        Err(os_engine::EngineError::Cancelled)
    ));
}
