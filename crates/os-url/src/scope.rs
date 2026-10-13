//! Crawl-Bereich: intern/extern (CRAWL-006/007/008/009).

use std::net::IpAddr;

use os_model::ScopeConfig;

use crate::normalize::NormUrl;

/// Einordnung einer URL relativ zum Crawl-Bereich.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeClass {
    /// Innerhalb des Crawl-Bereichs.
    Internal,
    /// Außerhalb des Crawl-Bereichs.
    External,
}

/// Crawl-Bereich, abgeleitet aus Start-URL und Konfiguration.
///
/// Host-Regeln (CRAWL-006 bis 008): Der exakte Host der Start-URL ist intern, ebenso jeder Host
/// derselben registrierbaren Domain, wenn `all_subdomains` gesetzt ist oder die Start-URL auf der
/// Stammdomain ohne Subdomain liegt (Ausnahme aus CRAWL-007). Schema und Port spielen keine Rolle.
/// Ordner-Regel (CRAWL-009): Bei `subfolder_only` ohne `crawl_outside_start_folder` muss der Pfad
/// mit dem Startordner beginnen (Pfad der Start-URL bis einschließlich des letzten `/`); die
/// Beschränkung gilt auf jedem internen Host.
#[derive(Debug, Clone)]
pub struct Scope {
    start_host: Option<String>,
    start_domain: Option<String>,
    all_subdomains: bool,
    folder: Option<String>,
}

impl Scope {
    /// Legt den Bereich fest.
    pub fn new(start: &NormUrl, cfg: &ScopeConfig) -> Scope {
        let start_host = start.url.host_str().map(|h| h.to_ascii_lowercase());
        let start_domain = start_host.as_deref().and_then(registrable_domain);
        // Stammdomain ohne Subdomain: alle Subdomains gelten im Standardbereich als intern.
        let root_start = start_domain.is_some() && start_domain == start_host;
        let folder_only = cfg.subfolder_only && !cfg.crawl_outside_start_folder;
        Scope {
            all_subdomains: cfg.all_subdomains || root_start,
            folder: folder_only.then(|| start_folder(start.url.path())),
            start_host,
            start_domain,
        }
    }

    /// Startordner, falls die Ordnerbeschränkung aktiv ist (`/blog/` für `/blog/seite.html`).
    pub fn start_folder(&self) -> Option<&str> {
        self.folder.as_deref()
    }

    /// Ordnet eine URL ein.
    pub fn classify(&self, url: &NormUrl) -> ScopeClass {
        let Some(host) = url.url.host_str() else {
            return ScopeClass::External;
        };
        if !self.host_is_internal(host) {
            return ScopeClass::External;
        }
        match &self.folder {
            Some(folder) if !url.url.path().starts_with(folder.as_str()) => ScopeClass::External,
            _ => ScopeClass::Internal,
        }
    }

    fn host_is_internal(&self, host: &str) -> bool {
        let host = host.trim_end_matches('.');
        let Some(start) = self.start_host.as_deref() else {
            return false;
        };
        if host.eq_ignore_ascii_case(start) {
            return true;
        }
        if !self.all_subdomains {
            return false;
        }
        match (&self.start_domain, registrable_domain(host)) {
            (Some(start_domain), Some(domain)) => *start_domain == domain,
            _ => false,
        }
    }
}

/// Pfad bis einschließlich des letzten `/` (`/blog/a.html` → `/blog/`, `/blog` → `/`).
fn start_folder(path: &str) -> String {
    match path.rfind('/') {
        Some(idx) => path[..=idx].to_string(),
        None => "/".to_string(),
    }
}

/// Registrierbare Domain (eTLD+1) eines Hosts nach der Public Suffix List.
///
/// `.test` wird wie ein öffentliches Suffix behandelt (`a.b.site.test` → `site.test`), damit die
/// Fixture-Hosts wie echte Domains funktionieren. IP-Adressen, einzelne Labels und reine Suffixe
/// (`co.uk`) haben keine registrierbare Domain (`None`).
pub fn registrable_domain(host: &str) -> Option<String> {
    let trimmed = host.trim().trim_end_matches('.');
    if trimmed.is_empty() {
        return None;
    }
    let bare = trimmed.trim_start_matches('[').trim_end_matches(']');
    if bare.parse::<IpAddr>().is_ok() {
        return None;
    }
    let ascii = if trimmed.is_ascii() {
        trimmed.to_ascii_lowercase()
    } else {
        idna::domain_to_ascii(trimmed).ok()?.to_ascii_lowercase()
    };
    let labels: Vec<&str> = ascii.split('.').collect();
    if labels.iter().any(|l| l.is_empty()) {
        return None;
    }
    // Numerisches letztes Label (`1.2.3`, `0x7f.1`): keine Domain.
    if labels
        .last()
        .is_some_and(|l| l.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    if labels.last() == Some(&"test") {
        return (labels.len() >= 2).then(|| labels[labels.len() - 2..].join("."));
    }
    psl::domain_str(&ascii).map(str::to_string)
}
