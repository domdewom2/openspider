//! `os-parity diff`: vergleicht einen Referenz-Export (Screaming Frog Free) mit dem OpenSpider-Export
//! anhand eines Spaltenmappings (Abschnitt 5.2/5.4).
//!
//! STUB(M1-P11)

use std::path::PathBuf;

use crate::error::ParityError;

/// Argumente von `os-parity diff`.
#[derive(Debug, Clone, clap::Args)]
pub struct DiffArgs {
    /// CSV der Referenz.
    #[arg(long)]
    pub sf: PathBuf,
    /// CSV von OpenSpider.
    #[arg(long)]
    pub os: PathBuf,
    /// Spaltenmapping (TOML).
    #[arg(long)]
    pub mapping: PathBuf,
    /// Basis-URL der Referenz-Site (wird zu `{BASE}` normalisiert).
    #[arg(long)]
    pub sf_base: String,
    /// Basis-URL der OpenSpider-Site.
    #[arg(long)]
    pub os_base: String,
    /// Ausgabeordner für `report.json` und `report.md`.
    #[arg(long)]
    pub out: Option<PathBuf>,
}

/// Führt den Vergleich aus; `Ok(true)` = alle Schwellen erfüllt.
// STUB(M1-P11)
pub fn run(_args: &DiffArgs) -> Result<bool, ParityError> {
    Err(ParityError::NotImplemented("os-parity diff (M1-P11)"))
}
