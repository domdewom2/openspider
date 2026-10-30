//! STUB(M3-U01): manual-only CLI update check, no automatic network requests.
use crate::exit::CliError;
/// M3-U01 will return 0 for current and 10 for available; never fake success.
pub fn check() -> Result<i32, CliError> {
    Err(CliError::usage(
        "Update-Prüfung noch nicht verfügbar (STUB M3-U01)",
    ))
}
