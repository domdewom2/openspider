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
}
