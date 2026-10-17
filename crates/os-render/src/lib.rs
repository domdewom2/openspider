//! `os-render` – JS-Rendering über Chromium (CDP), Screenshots und gerendertes DOM (CRAWL-078 ff.).
//!
//! Gerüst von M2-F02 (`docs/plan/M2.md` 3.5): Die öffentliche Oberfläche steht in [`api`] ([`Renderer::launch`],
//! [`Renderer::render`], [`RenderedPage`]); die Module `axe/` (Barrierefreiheit, M2-D21), `snippets/` (Custom JavaScript,
//! M2-D22) und `login` (formularbasierte Anmeldung, M2-U17) sind Stubs der Fachpakete. Seitenlauf, Screenshots, Konsole
//! und blockierte Ressourcen setzt M2-R01 um.

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
