//! Tauri signatures are base64 encoded minisign files, not signatures of JSON metadata.
use base64::{engine::general_purpose::STANDARD, Engine};
use minisign_verify::{PublicKey, Signature};
#[derive(Debug, thiserror::Error)]
#[error("Update-Signatur ungültig")]
pub struct VerifyError;
pub fn public_key(encoded: &str) -> Result<PublicKey, VerifyError> {
    let bytes = STANDARD.decode(encoded.trim()).map_err(|_| VerifyError)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| VerifyError)?;
    PublicKey::decode(text).map_err(|_| VerifyError)
}
pub fn signature(encoded: &str) -> Result<Signature, VerifyError> {
    let bytes = STANDARD.decode(encoded.trim()).map_err(|_| VerifyError)?;
    Signature::decode(std::str::from_utf8(&bytes).map_err(|_| VerifyError)?)
        .map_err(|_| VerifyError)
}
pub fn verify(payload: &[u8], signature_b64: &str, pubkey_b64: &str) -> Result<(), VerifyError> {
    let result = public_key(pubkey_b64).and_then(|key| {
        key.verify(payload, &signature(signature_b64)?, false)
            .map_err(|_| VerifyError)
    });
    if result.is_err() {
        tracing::warn!("Update abgelehnt: Signaturprüfung fehlgeschlagen");
    }
    result
}

/// Verify both bytes and the release version covered by minisign's trusted comment.
/// Download/install callers should use this variant to reject a replay of an older signed artifact
/// paired with a forged newer manifest version. Newly built Tauri artifacts include this field.
pub fn verify_release(
    payload: &[u8],
    signature_b64: &str,
    pubkey_b64: &str,
    announced: &semver::Version,
) -> Result<(), VerifyError> {
    verify(payload, signature_b64, pubkey_b64)?;
    let signed = signature(signature_b64)?;
    let versions: Vec<_> = signed
        .trusted_comment()
        .split('\t')
        .filter_map(|field| field.strip_prefix("version:"))
        .collect();
    let version = match versions.as_slice() {
        [value] => {
            semver::Version::parse(value.trim().strip_prefix('v').unwrap_or(value.trim())).ok()
        }
        _ => None,
    };
    if version.as_ref() != Some(announced) {
        tracing::warn!("Update abgelehnt: Signierte Version stimmt nicht mit dem Manifest überein");
        return Err(VerifyError);
    }
    Ok(())
}
