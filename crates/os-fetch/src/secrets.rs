//! Geheimnisse (Passwörter) aus dem OS-Schlüsselbund; in Tests ein Speicher im Arbeitsspeicher
//! (OUT-225).
//!
//! Konfigurationsdateien enthalten nur die Kennung (`secret_ref`), nie das Passwort. Fehlt der Eintrag
//! (z. B. auf einem anderen Rechner), liefert [`SecretStore::get`] `None`; die Aufrufer melden das als
//! „nicht verbunden“ statt mit einem kryptischen Plattformfehler. Keiner der Typen zeigt Werte in
//! seiner `Debug`-Ausgabe.

use std::collections::HashMap;
use std::sync::Mutex;

/// Quelle für Geheimnisse, die nie in Konfigurationsdateien stehen (`secret_ref`).
pub trait SecretStore: Send + Sync {
    /// Liefert das Geheimnis zur Kennung.
    fn get(&self, id: &str) -> Option<String>;
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
    pub const DEFAULT_SERVICE: &'static str = "openspider";

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
    fn get(&self, id: &str) -> Option<String> {
        match keyring::Entry::new(&self.service, id).and_then(|e| e.get_password()) {
            Ok(v) => Some(v),
            Err(keyring::Error::NoEntry) => None,
            Err(e) => {
                tracing::warn!(secret_ref = id, error = %e, "Schlüsselbund nicht lesbar");
                None
            }
        }
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
