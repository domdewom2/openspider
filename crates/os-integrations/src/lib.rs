//! `os-integrations` – Anbindungen an GSC, GA4, PSI, Majestic/Ahrefs/Moz und OAuth (04 A, E – J). KI-Anbieter und
//! DataForSEO gehören `os-ai` (M4/M5) und sind nicht Teil von M2.
//!
//! Gerüst von M2-F02 (`docs/plan/M2.md` 3.5): Der Rahmen steht in [`core`] (Trait [`ApiSource`], [`UrlRef`], [`ApiCtx`],
//! [`ApiStatus`]); die Module `oauth`, `queue` (M2-I01), `ga4` (I02), `gsc` (I03), `psi` (I04), `linkmetrics` (I05),
//! `drive` (X03) und `crawl_glue` (I06) sind Stubs der Fachpakete. Geheimnisse (Tokens, Schlüssel) laufen nur über
//! `os_fetch::secrets` (Regel C13); alle HTTP-Aufrufe über das Workspace-`reqwest`, in Tests gegen die Mocks von
//! `os-testsite` (Basis-URL je Quelle, INTEG-019).

pub mod core;
pub mod crawl_glue;
pub mod drive;
pub mod ga4;
pub mod gsc;
pub mod linkmetrics;
pub mod oauth;
pub mod psi;
pub mod queue;

pub use core::ApiError as IntegError;
pub use core::{ApiCtx, ApiError, ApiSource, ApiState, ApiStatus, BoxFuture, UrlRef};

/// Name dieses Crates.
pub const NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches() {
        assert_eq!(NAME, "os-integrations");
    }
}
