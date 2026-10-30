//! STUB(M3-R02): reserved installer cleanup hook. No destructive action yet.
use crate::exit::CliError;
pub fn cleanup() -> Result<(), CliError> {
    Err(CliError::usage(
        "Deinstallationsbereinigung noch nicht verfügbar (STUB M3-R02)",
    ))
}
