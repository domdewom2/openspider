use os_app::update::{manifest::https_url, platform_key, verify, UpdateManifest};
const MANIFEST: &str = include_str!("../../../tests/fixtures/update/manifest.json");
const PUBKEY: &str = include_str!("../../../tests/fixtures/update/test.key.pub");
const SIGNATURE: &str = include_str!("../../../tests/fixtures/update/artifact.sig");
const ARTIFACT: &[u8] = include_bytes!("../../../tests/fixtures/update/artifact");

// spec: OUT-252
#[test]
fn tauri_signature_rejects_one_modified_byte_and_wrong_keys() {
    verify(ARTIFACT, SIGNATURE, PUBKEY).unwrap();
    os_app::update::verify_release(ARTIFACT, SIGNATURE, PUBKEY, &"1.0.1".parse().unwrap()).unwrap();
    assert!(
        os_app::update::verify_release(ARTIFACT, SIGNATURE, PUBKEY, &"9.0.0".parse().unwrap())
            .is_err()
    );
    let mut changed = ARTIFACT.to_vec();
    changed[0] ^= 1;
    assert!(verify(&changed, SIGNATURE, PUBKEY).is_err());
    assert!(verify(ARTIFACT, "invalid", PUBKEY).is_err());
    assert!(verify(ARTIFACT, SIGNATURE, "").is_err());
}

// spec: OUT-252
#[test]
fn manifest_enforces_https_semver_no_downgrade_and_beta_policy() {
    let mut manifest: UpdateManifest = serde_json::from_str(MANIFEST).unwrap();
    manifest.validate(false).unwrap();
    for url in [
        "http://updates.test/latest.json",
        "https://user:pass@updates.test/file",
        "file:///tmp/latest.json",
    ] {
        assert!(https_url(url).is_err());
    }
    assert!(manifest.is_newer_than(&"1.0.0".parse().unwrap()));
    for current in ["1.0.1", "1.0.1+other", "2.0.0"] {
        assert!(!manifest.is_newer_than(&current.parse().unwrap()));
    }
    manifest.version = "1.1.0-beta.1".parse().unwrap();
    assert!(manifest.validate(false).is_err());
    manifest.validate(true).unwrap();
    manifest.pub_date = Some("invalid".into());
    assert!(manifest.validate(true).is_err());
    assert!(serde_json::from_str::<UpdateManifest>(&MANIFEST.replace("1.0.1", "banana")).is_err());
    assert_eq!(
        platform_key("windows", "x86_64", Some("msi")),
        "windows-x86_64-msi"
    );
    assert_eq!(platform_key("darwin", "aarch64", None), "darwin-aarch64");
}

// spec: OUT-252
#[test]
fn rejection_emits_a_warning_without_payload_or_key_material() {
    #[derive(Clone)]
    struct Log(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
    impl std::io::Write for Log {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let bytes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let capture = bytes.clone();
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(move || Log(capture.clone()))
        .finish();
    // Global within this isolated test binary: parallel security tests use the same callsites.
    tracing::subscriber::set_global_default(subscriber).unwrap();
    assert!(verify(b"changed-payload", SIGNATURE, PUBKEY).is_err());
    assert!(https_url("http://updates.test/latest.json").is_err());
    let logged = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
    assert!(logged.contains("WARN"));
    assert!(logged.contains("Signaturprüfung fehlgeschlagen"));
    assert!(logged.contains("HTTPS"));
    assert!(!logged.contains("changed-payload"));
    assert!(!logged.contains(PUBKEY.trim()));
}

// spec: OUT-252
#[test]
fn build_beta_variable_selects_endpoint_and_manifest_policy_together() {
    let context = os_app::update::UpdateContext::default();
    let beta = option_env!("OPENSPIDER_UPDATE_BETA") == Some("1");
    assert_eq!(context.beta, beta);
    assert_eq!(
        context.endpoint,
        if beta {
            "https://github.com/domdewom2/openspider/releases/latest/download/latest-beta.json"
        } else {
            "https://github.com/domdewom2/openspider/releases/latest/download/latest.json"
        }
    );
    let mut manifest: UpdateManifest = serde_json::from_str(MANIFEST).unwrap();
    manifest.version = "1.1.0-beta.1".parse().unwrap();
    assert_eq!(manifest.validate(context.beta).is_ok(), beta);
}
