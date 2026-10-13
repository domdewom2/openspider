//! Indexierbarkeitsmodell (TECH-001). Reine Funktion über `os-model`-Typen.
//!
//! STUB(M1-P24): P00a liefert nur die Statuscode-Regeln; P24 ergänzt noindex, Canonical und Meta Refresh.

use crate::config::CrawlConfig;
use crate::enums::{Indexability, IndexabilityStatus, NoResponseReason};
use crate::parse::RefreshRef;
use crate::write::{FetchFacts, PageFacts};

/// Eingabe der Bewertung.
pub struct IndexabilityInput<'a> {
    /// Abrufergebnis.
    pub fetch: &'a FetchFacts,
    /// Seitenfakten (nur HTML).
    pub page: Option<&'a PageFacts>,
    /// Adresse der Seite.
    pub page_url: &'a str,
    /// Refresh-Angaben der Seite.
    pub refresh: &'a [RefreshRef],
    /// Konfiguration.
    pub cfg: &'a CrawlConfig,
    /// URL-Gleichheit nach TECH-002; die Engine übergibt `os_url::compare::eq_for_compare`.
    pub same_url: fn(&str, &str) -> bool,
}

/// Bewertet die Indexierbarkeit einer URL.
// STUB(M1-P24)
pub fn evaluate(input: &IndexabilityInput) -> (Indexability, Option<IndexabilityStatus>) {
    let f = input.fetch;
    let blocked = f.robots_blocked
        || matches!(
            f.no_response_reason,
            Some(NoResponseReason::BlockedByRobots | NoResponseReason::BlockedByRobotsUnreachable)
        );
    let status = if blocked {
        Some(IndexabilityStatus::BlockedByRobots)
    } else {
        match f.status_code {
            0 => Some(IndexabilityStatus::NoResponse),
            300..=399 => Some(IndexabilityStatus::Redirected),
            400..=499 => Some(IndexabilityStatus::ClientError),
            500..=599 => Some(IndexabilityStatus::ServerError),
            _ => None,
        }
    };
    match status {
        Some(s) => (Indexability::NonIndexable, Some(s)),
        None => (Indexability::Indexable, None),
    }
}
