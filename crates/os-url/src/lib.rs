//! `os-url` – URL-Normalisierung, Umschreibung (URL Rewriting), Include/Exclude und Segmente.
//!
//! Modulgerüst nach M1-Plan 3.6 (P00b); die Umsetzung liefert M1-P01.

pub mod compare;
pub mod metrics;
pub mod normalize;
pub mod rules;
pub mod scope;

pub use compare::{compare_key, eq_for_compare};
pub use metrics::{folder_depth, url_length};
pub use normalize::{normalize, parse_start_url, resolve, NormUrl, UrlError};
pub use rules::{RuleDecision, RuleError, RuleSet};
pub use scope::{registrable_domain, Scope, ScopeClass};
