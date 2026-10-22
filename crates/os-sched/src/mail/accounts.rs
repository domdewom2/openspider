use super::MailError;
use os_fetch::secrets::SecretStoreMut;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Tls {
    Starttls,
    Tls,
    None,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Auth {
    #[default]
    Password,
    Gmail,
    Microsoft365,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub label: String,
    pub host: String,
    pub port: u16,
    pub tls: Tls,
    pub username: String,
    pub from: String,
    #[serde(default)]
    pub auth: Auth,
    #[serde(default)]
    pub allow_insecure: bool,
    #[serde(default)]
    pub client_id: Option<String>,
}
pub struct AccountStore {
    dir: PathBuf,
}
impl AccountStore {
    pub fn new(settings: &Path) -> Self {
        Self {
            dir: settings.join("mail"),
        }
    }
    fn path(&self, id: &str) -> Result<PathBuf, MailError> {
        if id.is_empty()
            || id.len() > 100
            || !id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        {
            return Err(MailError::new("Ungültige Mail-Kontokennung"));
        }
        Ok(self.dir.join(format!("{id}.json")))
    }
    pub fn get(&self, id: &str) -> Result<Account, MailError> {
        serde_json::from_slice(&std::fs::read(self.path(id)?)?)
            .map_err(|_| MailError::new("Mail-Konto nicht lesbar"))
    }
    pub fn list(&self) -> Result<Vec<Account>, MailError> {
        if !self.dir.exists() {
            return Ok(vec![]);
        }
        let mut accounts = vec![];
        for entry in std::fs::read_dir(&self.dir)? {
            let p = entry?.path();
            if p.extension().is_some_and(|e| e == "json") {
                if let Some(id) = p.file_stem().and_then(|s| s.to_str()) {
                    accounts.push(self.get(id)?);
                }
            }
        }
        accounts.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(accounts)
    }
    pub fn save(
        &self,
        account: &Account,
        password: Option<&str>,
        secrets: &dyn SecretStoreMut,
    ) -> Result<(), MailError> {
        let dest = self.path(&account.id)?;
        account.validate()?;
        let bytes = serde_json::to_vec_pretty(account)
            .map_err(|_| MailError::new("Mail-Konto nicht speicherbar"))?;
        std::fs::create_dir_all(&self.dir)?;
        let temp = self.dir.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
        std::fs::write(&temp, bytes)?;
        let key = format!("mail/{}", account.id);
        let previous = secrets
            .try_get(&key)
            .map_err(|_| MailError::new("Schlüsselbund nicht lesbar"))?;
        let result = (|| {
            if let Some(password) = password {
                secrets
                    .set(&format!("mail/{}", account.id), password)
                    .map_err(|_| MailError::new("Schlüsselbund nicht beschreibbar"))?;
            }
            if std::fs::rename(&temp, dest).is_err() {
                if password.is_some() {
                    match previous.as_deref() { Some(value) => secrets.set(&key,value), None => secrets.delete(&key) }.map_err(|_|MailError::new("Kontodatei nicht speicherbar; Schlüsselbund konnte nicht wiederhergestellt werden"))?;
                }
                return Err(MailError::new("Kontodatei nicht speicherbar"));
            }
            Ok(())
        })();
        let _ = std::fs::remove_file(temp);
        result
    }
    pub fn delete(&self, id: &str, secrets: &dyn SecretStoreMut) -> Result<(), MailError> {
        let path = self.path(id)?;
        secrets
            .delete(&format!("mail/{id}"))
            .map_err(|_| MailError::new("Schlüsselbund nicht beschreibbar"))?;
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}
impl Account {
    pub fn validate(&self) -> Result<(), MailError> {
        if self.port == 0
            || self.host.is_empty()
            || self.host.chars().any(char::is_control)
            || self.host.contains(['/', '@', '\r', '\n', ' '])
            || self.label.chars().any(char::is_control)
            || self.username.chars().any(char::is_control)
        {
            return Err(MailError::new(
                "SMTP-Host, Port oder Kontobezeichnung ungültig",
            ));
        }
        self.from
            .parse::<lettre::message::Mailbox>()
            .map_err(|_| MailError::new("Ungültige Absenderadresse"))?;
        if self.tls == Tls::None && !self.allow_insecure {
            return Err(MailError::new(
                "Unverschlüsseltes SMTP ausdrücklich bestätigen",
            ));
        }
        if self.auth != Auth::Password && self.tls == Tls::None {
            return Err(MailError::new("OAuth erfordert TLS"));
        }
        Ok(())
    }
}
