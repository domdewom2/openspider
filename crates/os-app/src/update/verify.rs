/// Verification fails closed until M3-U01 implements minisign verification.
#[derive(Debug, thiserror::Error)]
#[error("STUB(M3-U01): update signature verification unavailable")]
pub struct VerifyError;
/// STUB(M3-U01): never accepts an unverified payload.
pub fn verify(_payload: &[u8], _signature_b64: &str, _pubkey_b64: &str) -> Result<(), VerifyError> {
    Err(VerifyError)
}
