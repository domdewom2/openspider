//! `os-url` – URL-Normalisierung, Umschreibung (URL Rewriting), Include/Exclude und Segmente.
//!
//! Umsetzung von M1-P01 auf dem Modulgerüst nach M1-Plan 3.6.

pub mod compare;
pub mod metrics;
pub mod normalize;
pub mod rewrite;
pub mod rules;
pub mod scope;

pub use compare::{compare_key, eq_for_compare};
pub use metrics::{folder_depth, url_length};
pub use normalize::{normalize, parse_start_url, resolve, NormUrl, UrlError};
pub use rules::{IncludeHit, RuleDecision, RuleError, RuleKind, RuleSet, RuleWarning};
pub use scope::{registrable_domain, Scope, ScopeClass};
