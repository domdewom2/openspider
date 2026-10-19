//! `os-fetch` – HTTP-Client, robots.txt, Rate-Limit, Authentifizierung, Proxy, Cookies, HTTP/2.
//!
//! Modulgerüst nach M1-Plan 3.6 (P00b); die Umsetzung liefern M1-P02 (Abruf) und M1-P03 (`robots`).

pub mod auth;
pub mod client;
pub mod content_type;
pub mod cookies;
pub mod error;
pub mod http;
pub mod net;
pub mod rate;
pub mod redact;
pub mod resolve;
pub mod robots;
pub mod secrets;
pub mod user_agents;

pub use client::{BasicAuthEntry, FetchConfig, HttpFetcher};
pub use content_type::content_class;
pub use cookies::CookieJar;
pub use error::FetchError;
pub use http::{
    BodyLimit, Fetch, FetchOutcome, FetchPurpose, FetchRequest, FetchResult, HttpResponse,
    SyntheticResponse,
};
pub use secrets::{
    secret_store_from_env, MemorySecretStore, SecretBackend, SecretError, SecretStore,
    SecretStoreMut,
};

// INTEG-150: gemeinsame PSL-Auswertung ohne weitere Abhängigkeit im Integrationscrate.
pub use os_url::registrable_domain;
