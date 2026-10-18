//! Versioned `.osauth` envelopes (OUT-223): XChaCha20-Poly1305 with Argon2id or a keyring key.
//! The caller supplies keyring access; this module never opens the OS keychain.
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    aead::{Aead, Generate, KeyInit, Payload},
    XChaCha20Poly1305, XNonce,
};
use serde_json::{json, Value};

#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("Ungültiges oder beschädigtes .osauth-Profil")]
    Invalid,
    #[error("Falsche Passphrase oder beschädigtes Profil")]
    Decryption,
    #[error("Rechnergebundenes Profil: Schlüssel auf diesem Rechner nicht verfügbar")]
    MachineKey,
    #[error("Eine nicht leere Passphrase ist erforderlich")]
    Passphrase,
}

/// Cryptographically random key, encoded for SecretStoreMut.
pub fn new_key() -> String {
    let key = <[u8; 32]>::generate();
    hex::encode(key)
}

pub enum Protection<'a> {
    Passphrase(&'a str),
    Machine { key_ref: &'a str, key: &'a str },
}

fn derive(passphrase: &str, salt: &[u8]) -> Result<[u8; 32], ProfileError> {
    if passphrase.is_empty() {
        return Err(ProfileError::Passphrase);
    }
    let mut key = [0; 32];
    Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(19456, 2, 1, Some(32)).map_err(|_| ProfileError::Invalid)?,
    )
    .hash_password_into(passphrase.as_bytes(), salt, &mut key)
    .map_err(|_| ProfileError::Invalid)?;
    Ok(key)
}

fn machine_key(key: &str) -> Result<[u8; 32], ProfileError> {
    hex::decode(key)
        .ok()
        .and_then(|v| v.try_into().ok())
        .ok_or(ProfileError::MachineKey)
}

/// Encrypts the entire profile, including username and cookie/storage data. Header is authenticated.
pub fn seal(profile: &Value, protection: Protection<'_>) -> Result<Vec<u8>, ProfileError> {
    let salt = <[u8; 16]>::generate();
    let nonce = <[u8; 24]>::generate();
    let (mode, key_ref, key) = match protection {
        Protection::Passphrase(pass) => ("argon2id", "", derive(pass, &salt)?),
        Protection::Machine { key_ref, key } => ("keyring", key_ref, machine_key(key)?),
    };
    let header = json!({"format":"osauth", "version":1, "mode":mode, "keyRef":key_ref,
        "salt":hex::encode(salt), "nonce":hex::encode(nonce)});
    let aad = serde_json::to_vec(&header).map_err(|_| ProfileError::Invalid)?;
    let plaintext = serde_json::to_vec(profile).map_err(|_| ProfileError::Invalid)?;
    let encrypted = XChaCha20Poly1305::new((&key).into())
        .encrypt(
            &XNonce::from(nonce),
            Payload {
                msg: &plaintext,
                aad: &aad,
            },
        )
        .map_err(|_| ProfileError::Invalid)?;
    serde_json::to_vec(&json!({"header":header,"ciphertext":hex::encode(encrypted)}))
        .map_err(|_| ProfileError::Invalid)
}

/// Decrypts portable or machine-bound files; resolves keys solely through the supplied secret store.
pub fn open(
    data: &[u8],
    passphrase: Option<&str>,
    lookup: impl FnOnce(&str) -> Option<String>,
) -> Result<Value, ProfileError> {
    if data.len() > 16 * 1024 * 1024 {
        return Err(ProfileError::Invalid);
    }
    let envelope: Value = serde_json::from_slice(data).map_err(|_| ProfileError::Invalid)?;
    let h = &envelope["header"];
    if h["format"] != "osauth" || h["version"] != 1 {
        return Err(ProfileError::Invalid);
    }
    let decode = |v: &Value| {
        hex::decode(v.as_str().ok_or(ProfileError::Invalid)?).map_err(|_| ProfileError::Invalid)
    };
    let salt = decode(&h["salt"])?;
    let nonce = decode(&h["nonce"])?;
    if salt.len() != 16 || nonce.len() != 24 {
        return Err(ProfileError::Invalid);
    }
    let nonce: [u8; 24] = nonce.try_into().map_err(|_| ProfileError::Invalid)?;
    let key = match h["mode"].as_str() {
        Some("argon2id") => derive(passphrase.ok_or(ProfileError::Passphrase)?, &salt)?,
        Some("keyring") => machine_key(
            &lookup(h["keyRef"].as_str().ok_or(ProfileError::Invalid)?)
                .ok_or(ProfileError::MachineKey)?,
        )?,
        _ => return Err(ProfileError::Invalid),
    };
    let aad = serde_json::to_vec(h).map_err(|_| ProfileError::Invalid)?;
    let ciphertext = decode(&envelope["ciphertext"])?;
    let plaintext = XChaCha20Poly1305::new((&key).into())
        .decrypt(
            &XNonce::from(nonce),
            Payload {
                msg: &ciphertext,
                aad: &aad,
            },
        )
        .map_err(|_| ProfileError::Decryption)?;
    serde_json::from_slice(&plaintext).map_err(|_| ProfileError::Invalid)
}
