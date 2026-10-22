//! Geheimnisse (Passwörter) aus dem OS-Schlüsselbund; in Tests ein Speicher im Arbeitsspeicher
//! (OUT-225).
//!
//! Konfigurationsdateien enthalten nur die Kennung (`secret_ref`), nie das Passwort. Fehlt der Eintrag
//! (z. B. auf einem anderen Rechner), liefert [`SecretStore::get`] `None`; die Aufrufer melden das als
//! „nicht verbunden“ statt mit einem kryptischen Plattformfehler. Keiner der Typen zeigt Werte in
//! seiner `Debug`-Ausgabe.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};

/// Quelle für Geheimnisse, die nie in Konfigurationsdateien stehen (`secret_ref`).
pub trait SecretStore: Send + Sync {
    /// Optional writable view for services that receive the read-only contract (K2-61).
    fn as_mut_store(&self) -> Option<&dyn SecretStoreMut> {
        None
    }
    /// Liefert das Geheimnis zur Kennung.
    fn get(&self, id: &str) -> Option<String>;
    fn try_get(&self, id: &str) -> Result<Option<String>, SecretError> {
        Ok(self.get(id))
    }
}

/// Speicher im Arbeitsspeicher für Tests (Mock-Schlüsselbund).
#[derive(Default)]
pub struct MemorySecretStore {
    values: Mutex<HashMap<String, String>>,
}

impl std::fmt::Debug for MemorySecretStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let n = self.values.lock().unwrap_or_else(|e| e.into_inner()).len();
        f.debug_struct("MemorySecretStore")
            .field("entries", &n)
            .finish()
    }
}

impl MemorySecretStore {
    /// Leerer Speicher.
    pub fn new() -> Self {
        Self::default()
    }

    /// Legt ein Geheimnis ab.
    pub fn set(&self, id: &str, value: &str) {
        self.values
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id.to_string(), value.to_string());
    }
}

impl SecretStore for MemorySecretStore {
    fn as_mut_store(&self) -> Option<&dyn SecretStoreMut> {
        Some(self)
    }
    fn get(&self, id: &str) -> Option<String> {
        self.values
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .cloned()
    }
}

/// Betriebssystem-Schlüsselbund (macOS Keychain, Windows Credential Manager, Secret Service).
///
/// Die Kennung ist der Benutzername des Eintrags, `service` gruppiert die Einträge von OpenSpider.
#[derive(Debug, Clone)]
pub struct KeyringSecretStore {
    service: String,
}

impl KeyringSecretStore {
    /// Dienstname, unter dem OpenSpider seine Einträge ablegt.
    pub const DEFAULT_SERVICE: &'static str = "OpenSpider";

    /// Store für den Standard-Dienstnamen.
    pub fn new() -> Self {
        Self::with_service(Self::DEFAULT_SERVICE)
    }

    /// Store für einen eigenen Dienstnamen.
    pub fn with_service(service: &str) -> Self {
        KeyringSecretStore {
            service: service.to_string(),
        }
    }

    /// Legt ein Geheimnis im Schlüsselbund ab. Fehler enthalten nie den Wert.
    pub fn set(&self, id: &str, value: &str) -> Result<(), String> {
        keyring::Entry::new(&self.service, id)
            .and_then(|e| e.set_password(value))
            .map_err(|e| format!("Schlüsselbund nicht beschreibbar: {e}"))
    }

    /// Entfernt ein Geheimnis; ein fehlender Eintrag ist kein Fehler.
    pub fn delete(&self, id: &str) -> Result<(), String> {
        match keyring::Entry::new(&self.service, id).and_then(|e| e.delete_credential()) {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(format!("Schlüsselbund-Eintrag nicht löschbar: {e}")),
        }
    }
}

impl Default for KeyringSecretStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretStore for KeyringSecretStore {
    fn as_mut_store(&self) -> Option<&dyn SecretStoreMut> {
        Some(self)
    }
    fn get(&self, id: &str) -> Option<String> {
        self.try_get(id).ok().flatten()
    }
    fn try_get(&self, id: &str) -> Result<Option<String>, SecretError> {
        let entry = keyring::Entry::new(&self.service, id)
            .map_err(|_| SecretError::new("Schlüsselbund nicht erreichbar"))?;
        match entry.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) if self.service == Self::DEFAULT_SERVICE => {
                let migrated = migrate_legacy_once(id, || {
                    let old = keyring::Entry::new("openspider", id)
                        .map_err(|_| SecretError::new("Alter Schlüsselbund nicht erreichbar"))?;
                    match old.get_password() {
                        Ok(v) => {
                            entry.set_password(&v).map_err(|_| {
                                SecretError::new("Schlüsselbundmigration nicht schreibbar")
                            })?;
                            old.delete_credential().map_err(|_| {
                                SecretError::new("Alter Schlüsselbundeintrag nicht löschbar")
                            })?;
                            Ok(Some(v))
                        }
                        Err(keyring::Error::NoEntry) => Ok(None),
                        Err(_) => Err(SecretError::new("Alter Schlüsselbund nicht lesbar")),
                    }
                })?;
                if migrated.is_some() {
                    return Ok(migrated);
                }
                // Ein paralleler Leser kann nach unserem ersten get_password migriert haben.
                match entry.get_password() {
                    Ok(value) => Ok(Some(value)),
                    Err(keyring::Error::NoEntry) => Ok(None),
                    Err(_) => Err(SecretError::new("Schlüsselbund nicht lesbar")),
                }
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(SecretError::new("Schlüsselbund nicht lesbar")),
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Schreibende Schnittstelle (M2-F02, docs/plan/M2.md 3.6b)

/// Welche Ablage hinter einem [`SecretStoreMut`] steht (Diagnose und Hinweis in der Oberfläche).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecretBackend {
    /// Betriebssystem-Schlüsselbund (macOS Keychain, Windows Credential Manager, Secret Service).
    Keyring,
    /// Speicher im Arbeitsspeicher (Tests und Serverbetrieb ohne Schlüsselbund, `OPENSPIDER_SECRET_STORE=memory`).
    Memory,
}

/// Fehler beim Schreiben oder Löschen eines Geheimnisses. Die Meldung enthält nie den Wert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretError(String);

impl SecretError {
    /// Fehler mit Meldung (ohne Geheimnis).
    pub fn new(message: impl Into<String>) -> Self {
        SecretError(message.into())
    }

    /// Die Meldung.
    pub fn message(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SecretError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SecretError {}

/// Beschreibbare Quelle für Geheimnisse: API-Schlüssel, OAuth-Token, Profilschlüssel, SMTP-Passwörter, MCP-Token
/// (Regel C13). Kennungen haben die Form `openspider/<bereich>/<name>`, z. B. `api/gsc/refresh-token`.
///
/// Die bestehenden Typen behalten ihre Signaturen: `MemorySecretStore::set` (ohne Ergebnis) und
/// `KeyringSecretStore::set` (mit `Result<(), String>`) bleiben die inhärenten Methoden; dieses Trait ruft man über
/// `dyn SecretStoreMut` oder mit `SecretStoreMut::set(&store, …)`.
pub trait SecretStoreMut: SecretStore {
    /// Legt ein Geheimnis ab oder ersetzt es.
    fn set(&self, id: &str, value: &str) -> Result<(), SecretError>;

    /// Entfernt ein Geheimnis; ein fehlender Eintrag ist kein Fehler.
    fn delete(&self, id: &str) -> Result<(), SecretError>;

    /// Die Ablage hinter dem Store.
    fn backend(&self) -> SecretBackend;
}

impl SecretStoreMut for MemorySecretStore {
    fn set(&self, id: &str, value: &str) -> Result<(), SecretError> {
        MemorySecretStore::set(self, id, value);
        Ok(())
    }

    fn delete(&self, id: &str) -> Result<(), SecretError> {
        self.values
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id);
        Ok(())
    }

    fn backend(&self) -> SecretBackend {
        SecretBackend::Memory
    }
}

impl SecretStoreMut for KeyringSecretStore {
    fn set(&self, id: &str, value: &str) -> Result<(), SecretError> {
        KeyringSecretStore::set(self, id, value).map_err(SecretError)
    }

    fn delete(&self, id: &str) -> Result<(), SecretError> {
        KeyringSecretStore::delete(self, id).map_err(SecretError)
    }

    fn backend(&self) -> SecretBackend {
        SecretBackend::Keyring
    }
}

/// Umgebungsvariable, die den Speicher-Store wählt (`memory`).
pub const SECRET_STORE_ENV: &str = "OPENSPIDER_SECRET_STORE";

/// Der Store für diesen Prozess: mit `OPENSPIDER_SECRET_STORE=memory` ein **prozessweiter** [`MemorySecretStore`]
/// (alle Aufrufe teilen ihn; kein Schlüsselbund, kein Secret Service, kein D-Bus – Server und Tests), sonst der
/// [`KeyringSecretStore`] mit dem Standard-Dienstnamen. Das Erzeugen berührt den Schlüsselbund nicht.
pub fn secret_store_from_env() -> Arc<dyn SecretStoreMut> {
    static MEMORY: OnceLock<Arc<MemorySecretStore>> = OnceLock::new();
    let memory =
        std::env::var(SECRET_STORE_ENV).is_ok_and(|v| v.trim().eq_ignore_ascii_case("memory"));
    if memory {
        MEMORY
            .get_or_init(|| Arc::new(MemorySecretStore::new()))
            .clone()
    } else {
        Arc::new(KeyringSecretStore::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: OUT-225
    #[test]
    fn memory_store_roundtrip() {
        let store = MemorySecretStore::new();
        assert_eq!(store.get("a"), None);
        store.set("a", "geheim");
        assert_eq!(store.get("a").as_deref(), Some("geheim"));
    }

    // spec: OUT-225
    #[test]
    fn debug_output_never_contains_values() {
        let store = MemorySecretStore::new();
        store.set("a", "TOP-SECRET");
        assert!(!format!("{store:?}").contains("TOP-SECRET"));
    }

    // spec: OUT-225
    #[test]
    fn missing_entry_is_none_not_an_error() {
        let store: &dyn SecretStore = &MemorySecretStore::new();
        assert_eq!(store.get("gibt-es-nicht"), None);
    }
}

/// K2-52: je Kennung höchstens ein Zugriff auf den alten Dienst, auch bei parallelen Lesern.
/// Der Mutex bleibt während des Keyring-I/O in `migrate` gesperrt. Die Kennung wird vor
/// dem Aufruf als versucht markiert; auch ein Fehlschlag wird im Prozess nicht erneut versucht.
fn migrate_legacy_once(
    id: &str,
    migrate: impl FnOnce() -> Result<Option<String>, SecretError>,
) -> Result<Option<String>, SecretError> {
    static ATTEMPTED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let mut attempted = ATTEMPTED
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if !attempted.insert(id.to_owned()) {
        return Ok(None);
    }
    migrate()
}

#[cfg(test)]
mod migration_tests {
    use super::*;

    // spec: OUT-225
    #[test]
    fn legacy_migration_runs_once_per_secret_and_process_without_keyring_access() {
        let fixture = "fixture/legacy-migration";
        assert_eq!(
            migrate_legacy_once(fixture, || Ok(Some("value".into())))
                .unwrap()
                .as_deref(),
            Some("value")
        );
        assert_eq!(
            migrate_legacy_once(fixture, || panic!("must not access legacy service twice"))
                .unwrap(),
            None
        );
        assert!(migrate_legacy_once("fixture/another-secret", || Ok(None))
            .unwrap()
            .is_none());
    }
}
