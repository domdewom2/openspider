//! Passwortgeschützter API-Geheimnisexport mit Argon2id und AES-256-GCM (INTEG-002).
use crate::ApiError;
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng, Payload},
    Aes256Gcm, Nonce,
};
const MAGIC: &[u8] = b"OSAPI1";
pub fn encrypt(plaintext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, ApiError> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| failure())?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let encrypted = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext,
                aad: MAGIC,
            },
        )
        .map_err(|_| failure())?;
    Ok([MAGIC, nonce.as_slice(), &encrypted].concat())
}
pub fn decrypt(envelope: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, ApiError> {
    if envelope.len() < MAGIC.len() + 12 + 16 || !envelope.starts_with(MAGIC) {
        return Err(failure());
    }
    Aes256Gcm::new_from_slice(key)
        .map_err(|_| failure())?
        .decrypt(
            Nonce::from_slice(&envelope[6..18]),
            Payload {
                msg: &envelope[18..],
                aad: MAGIC,
            },
        )
        .map_err(|_| failure())
}
fn failure() -> ApiError {
    ApiError::Transport("Geheimnisexport: Passwort oder Datei ungültig".into())
}

/// Vollständiger passwortgeschützter Export für K07 (Argon2id + AES-256-GCM).
pub fn encrypt_with_password(plaintext: &[u8], password: &str) -> Result<Vec<u8>, ApiError> {
    use aes_gcm::aead::rand_core::RngCore;
    let mut salt = [0; 16];
    OsRng.fill_bytes(&mut salt);
    let mut key =
        os_store::queries::api::derive_export_key(password, &salt).map_err(|_| failure())?;
    let result = encrypt(plaintext, &key);
    key.fill(0);
    Ok([b"OSAPIP1".as_slice(), &salt, &result?].concat())
}
pub fn decrypt_with_password(envelope: &[u8], password: &str) -> Result<Vec<u8>, ApiError> {
    if envelope.len() < 23 || !envelope.starts_with(b"OSAPIP1") {
        return Err(failure());
    }
    let salt: [u8; 16] = envelope[7..23].try_into().map_err(|_| failure())?;
    let mut key =
        os_store::queries::api::derive_export_key(password, &salt).map_err(|_| failure())?;
    let result = decrypt(&envelope[23..], &key);
    key.fill(0);
    result
}
