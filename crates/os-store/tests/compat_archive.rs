use os_store::{CrawlFile, OpenMode, StoreError, SCHEMA_VERSION};
use std::{fs, path::PathBuf};

fn archive() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/migrations")
}

// spec: OUT-226 [M3]
#[test]
fn every_schema_opens_and_migrates_with_an_unchanged_backup() {
    assert_eq!(SCHEMA_VERSION, 5, "Add a fixture when changing the schema");
    for version in 1..=5 {
        let source = archive().join(format!("schema-v{version}/crawl.ospider"));
        let bytes = fs::read(&source).unwrap();
        assert!(bytes.len() <= 1024 * 1024);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("crawl.ospider");
        fs::write(&path, &bytes).unwrap();
        let old = CrawlFile::open_with(
            &path,
            OpenMode::ReadOnly,
            &os_store::MIGRATIONS[..version as usize],
        )
        .unwrap();
        assert_eq!(old.meta().unwrap().schema_version, version);
        drop(old);
        let current = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
        assert_eq!(current.meta().unwrap().schema_version, SCHEMA_VERSION);
        assert_eq!(current.config().unwrap().schema_version, 2);
        let count: i64 = current
            .conn()
            .query_row("SELECT count(*) FROM urls", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
        if version >= 3 {
            let fact: i64 = current
                .conn()
                .query_row(
                    "SELECT v_int FROM url_ext WHERE url_id=1 AND key='head.title_outside'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(fact, 1);
        }
        if version < 5 {
            let backup = os_store::open::backup_path(&path, version);
            assert_eq!(fs::read(backup).unwrap(), bytes);
        }
        assert_eq!(fs::read(source).unwrap(), bytes);
    }
}

// spec: OUT-226 [M3]
#[test]
fn future_schema_is_rejected_without_changing_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("future.ospider");
    fs::copy(archive().join("schema-v5/crawl.ospider"), &path).unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.pragma_update(None, "user_version", 999).unwrap();
    drop(conn);
    let bytes = fs::read(&path).unwrap();
    let error = CrawlFile::open(&path, OpenMode::ReadWrite).err().unwrap();
    assert!(
        matches!(
            error,
            StoreError::NewerVersion {
                found: 999,
                supported: SCHEMA_VERSION,
                ..
            }
        ),
        "{error:?}"
    );
    assert_eq!(fs::read(path).unwrap(), bytes);
}

// spec: OUT-255
#[test]
fn archived_auth_profile_decrypts_with_test_key_only() {
    let bytes = fs::read(archive().join("app-v1/profile.osauth")).unwrap();
    let profile = os_store::auth_profiles::open(&bytes, None, |key_ref| {
        assert_eq!(key_ref, "compat-test");
        Some("11".repeat(32))
    })
    .unwrap();
    assert_eq!(profile["username"], "fixture");
    assert_eq!(profile["password"], "test-only");
    assert!(os_store::auth_profiles::open(&bytes, None, |_| Some("22".repeat(32))).is_err());
}
