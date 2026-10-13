//! Geheimnisse (Passwörter) aus dem OS-Schlüsselbund; in Tests ein Speicher im Arbeitsspeicher.
//!
//! STUB(M1-P02): P02 ergänzt die Schlüsselbund-Anbindung (`keyring`).

use std::collections::HashMap;
use std::sync::Mutex;

/// Quelle für Geheimnisse, die nie in Konfigurationsdateien stehen (`secret_ref`).
pub trait SecretStore: Send + Sync {
    /// Liefert das Geheimnis zur Kennung.
    fn get(&self, id: &str) -> Option<String>;
}

/// Speicher im Arbeitsspeicher für Tests.
#[derive(Debug, Default)]
pub struct MemorySecretStore {
    values: Mutex<HashMap<String, String>>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_roundtrip() {
        let store = MemorySecretStore::new();
        assert_eq!(store.get("a"), None);
        store.set("a", "geheim");
        assert_eq!(store.get("a").as_deref(), Some("geheim"));
    }
}
