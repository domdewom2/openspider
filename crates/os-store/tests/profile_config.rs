//! Dateifixtures für Konfigurationsprofile; die Leistungsprofile bleiben unabhängig.
use os_model::CrawlConfig;
use os_store::profile::ConfigProfiles;

// spec: OUT-220 [M2]
// spec: CRAWL-132
// spec: CRAWL-102
#[test]
fn profiles_preserve_full_toml_and_default_across_restart() {
    let dir = tempfile::tempdir().unwrap();
    let store = ConfigProfiles::new(dir.path());
    let mut cfg = CrawlConfig::load_str(include_str!(
        "../../../tests/fixtures/config/commented.osconfig"
    ))
    .unwrap()
    .config;
    cfg.generator = CrawlConfig::default().generator;
    cfg.robots.custom_files.insert(
        "www.example.test".into(),
        "User-agent: *\nDisallow: /private".into(),
    );
    cfg.ui.columns.insert(
        "internal".into(),
        vec!["address".into(), "status_code".into()],
    );
    store.save("SEO", &cfg, false).unwrap();
    assert!(store.save("SEO", &cfg, false).is_err());
    assert_eq!(store.load("SEO").unwrap().config, cfg);
    store.set_default(Some("SEO")).unwrap();
    let reopened = ConfigProfiles::new(dir.path());
    assert_eq!(reopened.default_name().unwrap().as_deref(), Some("SEO"));
    assert_eq!(CrawlConfig::load_default(dir.path()).unwrap().config, cfg);
    let path = reopened.path("SEO").unwrap();
    let bytes = std::fs::read(&path).unwrap();
    reopened.save("SEO", &cfg, true).unwrap();
    assert_eq!(bytes, std::fs::read(path).unwrap());
    reopened.delete("SEO").unwrap();
    assert_eq!(reopened.default_name().unwrap(), None);
    assert!(!CrawlConfig::default_profile_path(dir.path()).exists());
    assert!(reopened.names().unwrap().is_empty());
}

// spec: OUT-220
#[test]
fn invalid_names_do_not_escape_profile_directory_and_recent_is_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let store = ConfigProfiles::new(dir.path());
    for name in [
        "",
        "..",
        "../escape",
        "a/b",
        "a\\b",
        " leading",
        "trailing ",
        "a\n",
        "a:",
    ] {
        assert!(store.path(name).is_err(), "{name:?}");
    }
    for n in 0..12 {
        store.remember_loaded(&format!("{n}")).unwrap();
    }
    store.remember_loaded("7").unwrap();
    let recent = store.recent().unwrap();
    assert_eq!(recent.len(), 10);
    assert_eq!(&recent[..3], &["7", "11", "10"]);
    assert_eq!(recent.iter().filter(|n| n.as_str() == "7").count(), 1);
}

// spec: OUT-220 [M2]
#[test]
fn managed_export_destinations_require_absolute_paths() {
    let dir = tempfile::tempdir().unwrap();
    let store = ConfigProfiles::new(dir.path());
    assert!(matches!(
        store.is_managed_path(std::path::Path::new("relative.osconfig")),
        Err(os_store::StoreError::InvalidInput(_))
    ));
    store.save("SEO", &CrawlConfig::default(), false).unwrap();
    assert!(store.is_managed_path(&store.path("SEO").unwrap()).unwrap());
    assert!(store
        .is_managed_path(&CrawlConfig::default_profile_path(dir.path()))
        .unwrap());
    assert!(!store
        .is_managed_path(&dir.path().join("export.osconfig"))
        .unwrap());
}
