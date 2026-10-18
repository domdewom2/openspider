use os_store::auth_profiles::{self, ProfileError, Protection};
use serde_json::json;

fn fixture() -> serde_json::Value {
    json!({"profile":{"name":"fixture","kind":"form","host":"fixture.test","username":"fixture-user","secret":"session=COOKIE-FIXTURE"},"createdAt":"2026-10-18T12:00:00Z"})
}
// spec: OUT-223
// spec: CRAWL-121
#[test]
fn portable_profile_preserves_all_credentials_without_plaintext() {
    let fixture = fixture();
    let a = auth_profiles::seal(&fixture, Protection::Passphrase("PASS-FIXTURE")).unwrap();
    let b = auth_profiles::seal(&fixture, Protection::Passphrase("PASS-FIXTURE")).unwrap();
    assert_ne!(a, b, "salt and nonce must be fresh");
    for secret in ["COOKIE-FIXTURE", "fixture-user", "PASS-FIXTURE"] {
        assert!(!String::from_utf8_lossy(&a).contains(secret));
    }
    assert_eq!(
        auth_profiles::open(&a, Some("PASS-FIXTURE"), |_| panic!(
            "portable profile needs no keyring"
        ))
        .unwrap(),
        fixture
    );
    assert!(matches!(
        auth_profiles::open(&a, Some("wrong"), |_| None),
        Err(ProfileError::Decryption)
    ));
    assert!(matches!(
        auth_profiles::open(&a, None, |_| None),
        Err(ProfileError::Passphrase)
    ));
}
// spec: OUT-223
#[test]
fn machine_binding_and_authenticated_header_reject_foreign_keys_and_tampering() {
    let key = auth_profiles::new_key();
    let bytes = auth_profiles::seal(
        &fixture(),
        Protection::Machine {
            key_ref: "auth/fixture",
            key: &key,
        },
    )
    .unwrap();
    assert_eq!(
        auth_profiles::open(&bytes, None, |id| {
            assert_eq!(id, "auth/fixture");
            Some(key.clone())
        })
        .unwrap(),
        fixture()
    );
    assert!(matches!(
        auth_profiles::open(&bytes, None, |_| None),
        Err(ProfileError::MachineKey)
    ));
    assert!(matches!(
        auth_profiles::open(&bytes, None, |_| Some(auth_profiles::new_key())),
        Err(ProfileError::Decryption)
    ));
    let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    v["header"]["keyRef"] = json!("auth/forged");
    assert!(matches!(
        auth_profiles::open(&serde_json::to_vec(&v).unwrap(), None, |_| Some(
            key.clone()
        )),
        Err(ProfileError::Decryption)
    ));
    v["header"]["nonce"] = json!("00");
    assert!(matches!(
        auth_profiles::open(&serde_json::to_vec(&v).unwrap(), None, |_| Some(
            key.clone()
        )),
        Err(ProfileError::Invalid)
    ));
    assert!(auth_profiles::open(b"not a profile", None, |_| None).is_err());
}
