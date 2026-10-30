use os_model::{settings::AppSettings, CrawlConfig};

// spec: OUT-250 [M3]
// spec: OUT-222 [M3]
// spec: OUT-224 [M3]
#[test]
fn updates_defaults_roundtrip_and_profile_separation() {
    let old = AppSettings::from_toml_str("").unwrap();
    assert!(old.updates.check);
    assert_eq!(old.updates.last_check, None);
    let loaded =
        AppSettings::from_toml_str(include_str!("../../../tests/fixtures/update/settings.toml"))
            .unwrap();
    assert!(!loaded.updates.check);
    assert_eq!(
        loaded.updates.last_check.as_deref(),
        Some("2026-10-30T12:00:00Z")
    );
    let text = loaded.to_toml_string().unwrap();
    assert_eq!(AppSettings::from_toml_str(&text).unwrap(), loaded);
    assert_eq!(
        AppSettings::from_toml_str(&text)
            .unwrap()
            .to_toml_string()
            .unwrap(),
        text
    );
    let profile = CrawlConfig::default().to_osconfig_string().unwrap();
    assert!(!profile.contains("[updates]"));
    assert!(!serde_json::to_value(CrawlConfig::default())
        .unwrap()
        .as_object()
        .unwrap()
        .contains_key("updates"));
    assert!(AppSettings::restart_required(["updates.check", "updates.last_check"]).is_empty());
}

// spec: OUT-222 [M3]
#[test]
fn settings_schema_golden() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/spec/schemas/settings.schema.json");
    let schema = schemars::schema_for!(AppSettings);
    let expected = format!("{}\n", serde_json::to_string_pretty(&schema).unwrap());
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, &expected).unwrap();
    }
    assert_eq!(std::fs::read_to_string(path).unwrap(), expected);
}
