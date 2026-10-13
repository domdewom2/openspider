//! Indexierbarkeitsmodell (TECH-001). Reine Funktion über `os-model`-Typen, ohne Abhängigkeit auf `os-url`.
//!
//! # Prüfreihenfolge
//!
//! Der erste zutreffende Grund gewinnt (Reihenfolge = Variantenreihenfolge von [`IndexabilityStatus`]):
//!
//! 1. `Blocked by Robots.txt` – die URL ist von der robots.txt gesperrt (`robots_blocked`) oder wurde wegen
//!    einer nicht erreichbaren robots.txt gar nicht abgerufen,
//! 2. `No Response` – Statuscode 0 (Timeout, DNS-, TLS-, Verbindungsfehler),
//! 3. `Redirected` – 3xx, oder Refresh (Header bzw. Meta-Element) mit Ziel ≠ eigene URL,
//! 4. `Client Error` (4xx), 5. `Server Error` (5xx),
//! 6. `Self Referencing Meta Refresh` – Refresh ohne Ziel oder auf die eigene URL, nur mit
//!    `advanced.respect_self_ref_meta_refresh` (TECH-068),
//! 7. `noindex` – wirksames `noindex` aus Meta Robots, agent-spezifischem Meta-Tag oder `X-Robots-Tag`
//!    einschließlich `none` (`PageFacts.effective_index`, TECH-066),
//! 8. `Canonicalised` – maßgebliche Canonical-URL (TECH-017, `PageFacts.canonical_url`) ≠ eigene URL.
//!
//! Alle URL-Vergleiche laufen über `IndexabilityInput::same_url` (TECH-002). Eine ungültige URL ist nie
//! gleich einer anderen, auch nicht sich selbst; daran erkennt [`valid_url`] ungültige Canonical- und
//! Refresh-Ziele, die für die Bewertung ignoriert werden. Externe URLs tragen weder Seitenfakten noch
//! Refresh-Angaben und werden dadurch nur nach Status bewertet.
//!
//! Welcher Refresh maßgeblich ist, entscheidet dieselbe Regel wie beim Abruf: zuerst der `Refresh`-Header,
//! sonst das erste Meta-Element; weitere Angaben bleiben unberücksichtigt.

use crate::config::CrawlConfig;
use crate::enums::{Indexability, IndexabilityStatus, NoResponseReason, RefreshSource};
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

/// Wirkung der maßgeblichen Refresh-Angabe einer Seite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefreshEffect {
    /// Kein (auswertbarer) Refresh.
    None,
    /// Weiterleitung auf eine andere URL.
    Redirect,
    /// Refresh ohne Ziel oder auf die eigene URL.
    SelfReference,
}

/// Eine URL ist gültig, wenn sie sich mit sich selbst vergleichen lässt (TECH-002: ungültige URLs sind nie
/// gleich, auch nicht derselben Zeichenkette).
fn valid_url(same_url: fn(&str, &str) -> bool, url: &str) -> bool {
    same_url(url, url)
}

fn refresh_effect(input: &IndexabilityInput) -> RefreshEffect {
    let chosen = input
        .refresh
        .iter()
        .find(|r| r.source == RefreshSource::Header)
        .or_else(|| {
            input
                .refresh
                .iter()
                .find(|r| r.source == RefreshSource::Meta)
        });
    let Some(refresh) = chosen else {
        return RefreshEffect::None;
    };
    match refresh.target.as_deref() {
        // Kein Ziel angegeben (`content="30"`): Selbst-Refresh.
        None if refresh.target_raw.is_none() => RefreshEffect::SelfReference,
        // Ziel angegeben, aber nicht auflösbar: kein auswertbarer Refresh.
        None => RefreshEffect::None,
        Some(target) if !valid_url(input.same_url, target) => RefreshEffect::None,
        Some(target) if (input.same_url)(input.page_url, target) => RefreshEffect::SelfReference,
        Some(_) => RefreshEffect::Redirect,
    }
}

/// Bewertet die Indexierbarkeit einer URL.
pub fn evaluate(input: &IndexabilityInput) -> (Indexability, Option<IndexabilityStatus>) {
    match status_of(input) {
        Some(s) => (Indexability::NonIndexable, Some(s)),
        None => (Indexability::Indexable, None),
    }
}

fn status_of(input: &IndexabilityInput) -> Option<IndexabilityStatus> {
    let f = input.fetch;

    // 1. robots.txt
    let blocked = f.robots_blocked
        || matches!(
            f.no_response_reason,
            Some(NoResponseReason::BlockedByRobots | NoResponseReason::BlockedByRobotsUnreachable)
        );
    if blocked {
        return Some(IndexabilityStatus::BlockedByRobots);
    }

    // 2. keine Antwort
    if f.status_code == 0 {
        return Some(IndexabilityStatus::NoResponse);
    }

    // 3. Weiterleitung (3xx oder Refresh auf eine andere URL)
    let refresh = refresh_effect(input);
    if matches!(f.status_code, 300..=399) || refresh == RefreshEffect::Redirect {
        return Some(IndexabilityStatus::Redirected);
    }

    // 4./5. Fehlerantworten
    match f.status_code {
        400..=499 => return Some(IndexabilityStatus::ClientError),
        500..=599 => return Some(IndexabilityStatus::ServerError),
        _ => {}
    }

    // 6. Selbst-Refresh (TECH-068)
    if refresh == RefreshEffect::SelfReference && input.cfg.advanced.respect_self_ref_meta_refresh {
        return Some(IndexabilityStatus::SelfReferencingMetaRefresh);
    }

    let page = input.page?;

    // 7. noindex (Meta, agent-spezifisches Meta, X-Robots-Tag; `none` inklusive)
    if !page.effective_index {
        return Some(IndexabilityStatus::Noindex);
    }

    // 8. Canonical auf eine andere URL (nur mit gespeicherten Canonicals, CRAWL-046; ungültige ignoriert)
    if input.cfg.spider.canonicals.store {
        if let Some(canonical) = page.canonical_url.as_deref() {
            if valid_url(input.same_url, canonical) && !(input.same_url)(input.page_url, canonical)
            {
                return Some(IndexabilityStatus::Canonicalised);
            }
        }
    }
    None
}
