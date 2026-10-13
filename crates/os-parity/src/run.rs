//! `os-parity run`: startet die Fixture-Site, erzeugt die Exporte laut Manifest und vergleicht alle
//! (Abschnitt 5.3/5.4).
//!
//! STUB(M1-P11)

use std::path::PathBuf;

use crate::error::ParityError;

/// Argumente von `os-parity run`.
#[derive(Debug, Clone, clap::Args)]
pub struct RunArgs {
    /// Name der Site (z. B. `out-basic-site`).
    #[arg(long, conflicts_with = "all")]
    pub site: Option<String>,
    /// Alle Sites mit Manifest.
    #[arg(long)]
    pub all: bool,
    /// Pfad des `openspider`-Binärs (sonst Umgebungsvariable `OPENSPIDER_BIN`).
    #[arg(long)]
    pub openspider: Option<PathBuf>,
    /// Ausgabeordner der Berichte.
    #[arg(long, default_value = "target/parity")]
    pub out: PathBuf,
}

/// Führt den Lauf aus; `Ok(true)` = alle Schwellen erfüllt.
// STUB(M1-P11)
pub fn run(_args: &RunArgs) -> Result<bool, ParityError> {
    Err(ParityError::NotImplemented("os-parity run (M1-P11)"))
}
