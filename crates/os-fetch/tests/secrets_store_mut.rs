//! `SecretStoreMut` und `secret_store_from_env` (M2.md 3.6b, Regel C13, OUT-225): Schreiben, Ersetzen, Löschen und die
//! Wahl des Speichers über `OPENSPIDER_SECRET_STORE`. Kein Test öffnet den Betriebssystem-Schlüsselbund.

use std::sync::Arc;

use os_fetch::secrets::{
    secret_store_from_env, KeyringSecretStore, MemorySecretStore, SecretBackend, SecretStore,
    SecretStoreMut, SECRET_STORE_ENV,
};

// spec: OUT-225
#[test]
fn memory_store_sets_replaces_and_deletes_through_the_trait() {
    let store: Arc<dyn SecretStoreMut> = Arc::new(MemorySecretStore::new());
    assert_eq!(store.backend(), SecretBackend::Memory);
    assert_eq!(store.get("api/gsc/refresh-token"), None);
    store.set("api/gsc/refresh-token", "erster-wert").unwrap();
    assert_eq!(
        store.get("api/gsc/refresh-token").as_deref(),
        Some("erster-wert")
    );
    store.set("api/gsc/refresh-token", "zweiter-wert").unwrap();
    assert_eq!(
        store.get("api/gsc/refresh-token").as_deref(),
        Some("zweiter-wert")
    );
    store.delete("api/gsc/refresh-token").unwrap();
    assert_eq!(store.get("api/gsc/refresh-token"), None);
    // Ein fehlender Eintrag ist kein Fehler.
    store.delete("api/gsc/refresh-token").unwrap();
    store.delete("gibt-es-nicht").unwrap();
    // Andere Einträge bleiben unberührt.
    store.set("mail/1", "a").unwrap();
    store.set("mail/2", "b").unwrap();
    store.delete("mail/1").unwrap();
    assert_eq!(store.get("mail/2").as_deref(), Some("b"));
}

// spec: OUT-225
#[test]
fn existing_signatures_are_unchanged() {
    // Die inhärenten Methoden gelten weiter (Aufrufer aus M1 kompilieren unverändert).
    let memory = MemorySecretStore::new();
    let _: () = memory.set("a", "b");
    assert_eq!(memory.get("a").as_deref(), Some("b"));
    let keyring = KeyringSecretStore::with_service("openspider-test-nicht-benutzt");
    // Nur die Signatur wird geprüft (Funktionszeiger); der Schlüsselbund wird nie geöffnet.
    let _set: fn(&KeyringSecretStore, &str, &str) -> Result<(), String> = KeyringSecretStore::set;
    let _delete: fn(&KeyringSecretStore, &str) -> Result<(), String> = KeyringSecretStore::delete;
    assert_eq!(SecretStoreMut::backend(&keyring), SecretBackend::Keyring);
}

// spec: OUT-225
#[test]
fn store_from_env_is_a_process_wide_memory_store_or_the_keyring() {
    // Eine Funktion, weil die Umgebungsvariable prozessweit gilt.
    std::env::remove_var(SECRET_STORE_ENV);
    assert_eq!(secret_store_from_env().backend(), SecretBackend::Keyring);

    std::env::set_var(SECRET_STORE_ENV, "memory");
    let a = secret_store_from_env();
    let b = secret_store_from_env();
    assert_eq!(a.backend(), SecretBackend::Memory);
    a.set("auth-profile/shop/key", "geheim").unwrap();
    assert_eq!(
        b.get("auth-profile/shop/key").as_deref(),
        Some("geheim"),
        "prozessweit: alle Aufrufe teilen den Speicher"
    );
    b.delete("auth-profile/shop/key").unwrap();
    assert_eq!(a.get("auth-profile/shop/key"), None);

    // Schreibweise egal, umgebende Leerzeichen auch.
    std::env::set_var(SECRET_STORE_ENV, " Memory ");
    assert_eq!(secret_store_from_env().backend(), SecretBackend::Memory);

    // Jeder andere Wert wählt den Schlüsselbund (ohne ihn zu öffnen).
    std::env::set_var(SECRET_STORE_ENV, "keyring");
    assert_eq!(secret_store_from_env().backend(), SecretBackend::Keyring);
    std::env::remove_var(SECRET_STORE_ENV);
}

// spec: OUT-225
#[test]
fn errors_and_debug_output_never_contain_values() {
    let err = os_fetch::secrets::SecretError::new("Schlüsselbund nicht beschreibbar");
    assert_eq!(err.message(), "Schlüsselbund nicht beschreibbar");
    assert!(!err.to_string().contains("TOP-SECRET"));
    let store = MemorySecretStore::new();
    SecretStoreMut::set(&store, "a", "TOP-SECRET").unwrap();
    assert!(!format!("{store:?}").contains("TOP-SECRET"));
}
