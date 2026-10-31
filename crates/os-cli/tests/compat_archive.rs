mod common;
use common::Env;
use std::{fs, path::PathBuf};

fn archive() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/migrations")
}

// spec: OUT-255
#[test]
fn every_archived_schema_and_release_exports_internal_all() {
    let root = archive();
    let mut stands: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.join("crawl.ospider").is_file())
        .collect();
    let releases = root.join("releases");
    if releases.is_dir() {
        stands.extend(fs::read_dir(releases).unwrap().map(|e| e.unwrap().path()));
    }
    stands.sort();
    assert!(stands.len() >= 5);
    for stand in stands {
        let env = Env::new();
        let path = env.work.path().join("archived.ospider");
        let before = fs::read(stand.join("crawl.ospider")).unwrap();
        fs::write(&path, &before).unwrap();
        let run = env.run(&[
            "--load-crawl",
            path.to_str().unwrap(),
            "--export-tabs",
            "Internal:All",
            "--output-folder",
            "out",
        ]);
        assert_eq!(run.code, 0, "{}: {}", stand.display(), run.dump());
        let actual = fs::read_to_string(env.out().join("internal_all.csv")).unwrap();
        let normalized = os_testsite::golden::normalize(&actual, &[]);
        os_testsite::golden::assert_golden(stand.join("internal_all.csv"), &normalized);
        assert_eq!(fs::read(stand.join("crawl.ospider")).unwrap(), before);
    }
}

// spec: OUT-255
// spec: OUT-226 [M3]
#[test]
fn archived_profiles_presets_and_tasks_load_and_config_migration_keeps_backup() {
    let preset =
        os_export::types::ExportPreset::load(&archive().join("app-v1/basic.ospreset")).unwrap();
    assert_eq!(preset.name, "Basic");
    assert_eq!(preset.items.len(), 3);
    let tasks = os_sched::tasks::bundle::import_text(
        &fs::read_to_string(archive().join("app-v1/nightly.ostask")).unwrap(),
    )
    .unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, "nightly");
    for version in 1..=2 {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("profile.osconfig");
        let bytes =
            fs::read(archive().join(format!("config-v{version}/profile.osconfig"))).unwrap();
        fs::write(&path, &bytes).unwrap();
        let report = os_model::CrawlConfig::migrate_file(&path).unwrap();
        if version == 1 {
            assert_eq!(fs::read(report.unwrap().backup.unwrap()).unwrap(), bytes);
        } else {
            assert!(report.is_none());
        }
    }
}

// spec: OUT-255
#[test]
fn both_archived_config_versions_open_and_export_with_current_cli() {
    for version in 1..=2 {
        let env = Env::new();
        let path = env.work.path().join("current.ospider");
        fs::copy(archive().join("schema-v5/crawl.ospider"), &path).unwrap();
        let profile = archive().join(format!("config-v{version}/profile.osconfig"));
        let run = env.run(&[
            "--load-crawl",
            path.to_str().unwrap(),
            "--config",
            profile.to_str().unwrap(),
            "--export-tabs",
            "Internal:All",
            "--output-folder",
            "out",
        ]);
        assert_eq!(run.code, 0, "{}", run.dump());
        let actual = fs::read_to_string(env.out().join("internal_all.csv")).unwrap();
        os_testsite::golden::assert_golden(
            archive().join("schema-v5/internal_all.csv"),
            &os_testsite::golden::normalize(&actual, &[]),
        );
    }
}
