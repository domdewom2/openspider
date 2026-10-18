//! Ein JSON-Eintrag je Integration; flüchtige Schlüssel verlassen den Prozess nie.
use crate::ApiError;
use os_fetch::SecretStoreMut;
use os_model::{ApiKind, Timestamp};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Credential {
    pub account_id: String,
    pub label: String,
    pub api_key: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_at: Option<Timestamp>,
    pub client_secret: Option<String>,
    pub scopes: Vec<String>,
}
impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Credential([redacted])")
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
struct Accounts {
    accounts: BTreeMap<String, Credential>,
}

pub struct Credentials {
    store: Arc<dyn SecretStoreMut>,
    persistent: Mutex<BTreeMap<String, Accounts>>,
    operation: Mutex<()>,
    session: Mutex<BTreeMap<String, BTreeMap<String, Credential>>>,
}
impl Credentials {
    pub fn new(store: Arc<dyn SecretStoreMut>) -> Self {
        Self {
            store,
            persistent: Mutex::default(),
            operation: Mutex::default(),
            session: Mutex::default(),
        }
    }
    fn saved(&self, kind: ApiKind) -> Result<Accounts, ApiError> {
        if let Some(saved) = self
            .persistent
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&kind.to_string())
            .cloned()
        {
            return Ok(saved);
        }
        let raw = self.store.try_get(&format!("api/{kind}")).map_err(|_| {
            ApiError::Transport("Schlüsselbund nicht lesbar; Sitzungsschlüssel möglich".into())
        })?;
        let saved: Accounts = raw
            .map(|s| {
                serde_json::from_str(&s)
                    .map_err(|_| ApiError::Transport("Zugangsdaten nicht lesbar".into()))
            })
            .unwrap_or_else(|| Ok(Accounts::default()))?;
        self.persistent
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(kind.to_string(), saved.clone());
        Ok(saved)
    }
    pub fn list(&self, kind: ApiKind) -> Result<Vec<Credential>, ApiError> {
        let _operation = self.operation.lock().unwrap_or_else(|p| p.into_inner());
        let saved_result = self.saved(kind);
        let has_session = self
            .session
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains_key(&kind.to_string());
        let mut saved = match saved_result {
            Ok(saved) => saved.accounts,
            Err(_) if has_session => BTreeMap::new(),
            Err(e) => return Err(e),
        };
        if let Some(session) = self
            .session
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&kind.to_string())
        {
            saved.extend(session.clone());
        }
        Ok(saved.into_values().collect())
    }
    /// Liefert false, wenn nur die Sitzung gespeichert werden konnte (gesperrter Schlüsselbund).
    pub fn put(
        &self,
        kind: ApiKind,
        credential: Credential,
        remember: bool,
    ) -> Result<bool, ApiError> {
        let _operation = self.operation.lock().unwrap_or_else(|p| p.into_inner());
        let mut persisted = false;
        if remember {
            if let Ok(mut saved) = self.saved(kind) {
                saved
                    .accounts
                    .insert(credential.account_id.clone(), credential.clone());
                persisted = self.write_saved(kind, &saved).is_ok();
            }
        } else {
            // Ein früher gemerkter Schlüssel darf nach einem Neustart nicht wieder auftauchen.
            let mut saved = self.saved(kind).unwrap_or_default();
            if saved.accounts.remove(&credential.account_id).is_some() {
                self.write_saved(kind, &saved)?;
            }
        }
        self.session
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .entry(kind.to_string())
            .or_default()
            .insert(credential.account_id.clone(), credential);
        Ok(persisted)
    }
    fn write_saved(&self, kind: ApiKind, saved: &Accounts) -> Result<(), ApiError> {
        let id = format!("api/{kind}");
        let result = if saved.accounts.is_empty() {
            self.store.delete(&id)
        } else {
            self.store.set(
                &id,
                &serde_json::to_string(saved)
                    .map_err(|_| ApiError::Transport("Zugangsdaten nicht serialisierbar".into()))?,
            )
        };
        result.map_err(|_| ApiError::Transport("Schlüsselbund nicht beschreibbar".into()))?;
        self.persistent
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(kind.to_string(), saved.clone());
        Ok(())
    }
    pub fn remove(&self, kind: ApiKind, id: &str) -> Result<(), ApiError> {
        let _operation = self.operation.lock().unwrap_or_else(|p| p.into_inner());
        let mut saved = self.saved(kind)?;
        saved.accounts.remove(id);
        self.write_saved(kind, &saved)?;
        if let Some(session) = self
            .session
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get_mut(&kind.to_string())
        {
            session.remove(id);
        }
        Ok(())
    }
    pub fn is_persistent(&self, kind: ApiKind, id: &str) -> bool {
        self.saved(kind).is_ok_and(|a| a.accounts.contains_key(id))
    }
}
pub fn key_suffix(key: &str) -> String {
    key.chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

impl os_fetch::SecretStore for Credentials {
    fn get(&self, id: &str) -> Option<String> {
        if let Some(kind) = id
            .strip_prefix("api/")
            .and_then(|k| k.parse::<ApiKind>().ok())
        {
            let accounts = self
                .list(kind)
                .ok()?
                .into_iter()
                .map(|c| (c.account_id.clone(), c))
                .collect();
            return serde_json::to_string(&Accounts { accounts }).ok();
        }
        self.store.get(id)
    }
}

/// Quellen lesen ihr ausgewähltes Konto aus der gemeinsamen, auch flüchtigen Secret-Sicht.
pub fn read_account(
    store: &dyn os_fetch::SecretStore,
    kind: ApiKind,
    id: Option<&str>,
) -> Result<Credential, ApiError> {
    let raw = store
        .try_get(&format!("api/{kind}"))
        .map_err(|_| ApiError::NotConnected(kind.to_string()))?
        .ok_or_else(|| ApiError::NotConnected(kind.to_string()))?;
    let mut accounts: Accounts =
        serde_json::from_str(&raw).map_err(|_| ApiError::NotConnected(kind.to_string()))?;
    let id = id
        .map(str::to_string)
        .or_else(|| {
            (accounts.accounts.len() == 1).then(|| {
                accounts
                    .accounts
                    .keys()
                    .next()
                    .expect("one account")
                    .clone()
            })
        })
        .ok_or_else(|| ApiError::NotConnected(kind.to_string()))?;
    accounts
        .accounts
        .remove(&id)
        .ok_or_else(|| ApiError::NotConnected(kind.to_string()))
}
