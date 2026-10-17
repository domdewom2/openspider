//! `os-render` – JS-Rendering über Chromium (CDP), Screenshots und gerendertes DOM (CRAWL-078 ff.).
//!
//! Chromium/CDP-Seitenlauf, Ressourcenrichtlinien, Screenshots und begrenzte
//! Konsolenerfassung stehen in [`api`]. Die Engine übergibt Netzwerkidentität,
//! robots-Freigaben und bereits abgerufene Hauptdokumente. Die Module `axe/`,
//! `snippets/` und `login` bleiben Erweiterungspunkte ihrer Fachpakete.

pub mod api;
pub mod axe;
pub mod login;
pub mod snippets;

pub use api::{LaunchOpts, RenderError, RenderOpts, RenderedPage, Renderer};

/// Name dieses Crates.
pub const NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches() {
        assert_eq!(NAME, "os-render");
    }
}
