use os_model::{settings::AppSettings, CrawlConfig};
use std::{fs, path::PathBuf};

fn archive() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/migrations")
}

// spec: OUT-255
// spec: OUT-226 [M3]
#[test]
fn archived_configs_and_settings_remain_readable() {
    for version in 1..=2 {
        let bytes =
            fs::read_to_string(archive().join(format!("config-v{version}/profile.osconfig")))
                .unwrap();
        let loaded = CrawlConfig::load_str(&bytes).unwrap();
        let (migrated, report) = os_model::config::migrate_toml(&bytes).unwrap();
        assert_eq!(report.from_version, version);
        assert_eq!(report.to_version, 2);
        assert_eq!(
            CrawlConfig::load_str(&migrated).unwrap().config.speed,
            loaded.config.speed
        );
        let encoded = loaded.config.to_osconfig_string().unwrap();
        assert_eq!(
            CrawlConfig::load_str(&encoded)
                .unwrap()
                .config
                .to_osconfig_string()
                .unwrap(),
            encoded
        );
    }
    let settings = AppSettings::from_toml_str(
        &fs::read_to_string(archive().join("app-v1/settings.toml")).unwrap(),
    )
    .unwrap();
    assert_eq!(settings.general.language, os_model::UiLanguage::En);
    assert!(settings.updates.check);
    assert_eq!(
        AppSettings::from_toml_str(&settings.to_toml_string().unwrap()).unwrap(),
        settings
    );
}
