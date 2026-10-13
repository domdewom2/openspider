//! `os-parity coverage`: Status je Spec-ID aus Tests mit `// spec: …`-Tag und Testergebnissen (Abschnitt 5.4).
//!
//! STUB(M1-P11)

use std::path::PathBuf;

use crate::error::ParityError;

/// Argumente von `os-parity coverage`.
#[derive(Debug, Clone, clap::Args)]
pub struct CoverageArgs {
    /// Paketliste (`docs/plan/M1-packages.json`).
    #[arg(long)]
    pub packages: PathBuf,
    /// Ausgabe von `cargo test --workspace --exclude openspider-desktop --no-fail-fast`.
    #[arg(long)]
    pub test_output: Option<PathBuf>,
    /// JUnit-XML aus `cargo nextest` (Alternative zu `--test-output`).
    #[arg(long)]
    pub junit: Option<PathBuf>,
    /// Statusdatei (`docs/plan/M1-status.json`).
    #[arg(long)]
    pub write: Option<PathBuf>,
}

/// Ermittelt den Status; `Ok(true)` = keine ID mit Status `open`.
// STUB(M1-P11)
pub fn run(_args: &CoverageArgs) -> Result<bool, ParityError> {
    Err(ParityError::NotImplemented("os-parity coverage (M1-P11)"))
}
