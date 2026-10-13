//! Crawl-Bereich: intern/extern (CRAWL-007/008/009).

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
#[derive(Debug, Clone)]
pub struct Scope {
    start_host: Option<String>,
    cfg: ScopeConfig,
}

impl Scope {
    /// Legt den Bereich fest.
    // STUB(M1-P01)
    pub fn new(start: &NormUrl, cfg: &ScopeConfig) -> Scope {
        Scope {
            start_host: start.url.host_str().map(str::to_string),
            cfg: cfg.clone(),
        }
    }

    /// Ordnet eine URL ein (Stub: gleicher Host = intern; mit `all_subdomains` gleiche Stammdomain).
    // STUB(M1-P01)
    pub fn classify(&self, url: &NormUrl) -> ScopeClass {
        let (Some(start), Some(host)) = (self.start_host.as_deref(), url.url.host_str()) else {
            return ScopeClass::External;
        };
        let same = host == start
            || (self.cfg.all_subdomains
                && registrable_domain(host).is_some()
                && registrable_domain(host) == registrable_domain(start));
        if same {
            ScopeClass::Internal
        } else {
            ScopeClass::External
        }
    }
}

/// Registrierbare Domain (eTLD+1) eines Hosts.
// STUB(M1-P01): `.test` als Suffix behandeln (Fixtures) ergänzt P01.
pub fn registrable_domain(host: &str) -> Option<String> {
    psl::domain_str(host).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalize::parse_start_url;

    #[test]
    fn same_host_is_internal() {
        let start = parse_start_url("https://site.example/").unwrap();
        let scope = Scope::new(&start, &ScopeConfig::default());
        let other = parse_start_url("https://other.example/x").unwrap();
        assert_eq!(scope.classify(&start), ScopeClass::Internal);
        assert_eq!(scope.classify(&other), ScopeClass::External);
    }
}
