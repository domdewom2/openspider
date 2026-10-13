//! `os-fetch` – HTTP-Client, robots.txt, Rate-Limit, Authentifizierung, Proxy, Cookies, HTTP/2.
//!
//! Modulgerüst nach M1-Plan 3.6 (P00b); die Umsetzung liefern M1-P02 (Abruf) und M1-P03 (`robots`).

pub mod auth;
pub mod client;
pub mod content_type;
pub mod cookies;
pub mod error;
pub mod http;
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
pub use http::{Fetch, FetchOutcome, FetchPurpose, FetchRequest, FetchResult, HttpResponse};
pub use secrets::{MemorySecretStore, SecretStore};
