//! Properties des Kontos (`sites.list`, INTEG-110) und Wahl der Property je URL (INTEG-122).

use url::Url;

/// Art einer Property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PropertyKind {
    /// Domain-Property `sc-domain:<host>` (alle Protokolle und Subdomains).
    Domain,
    /// URL-Präfix-Property (`https://host/pfad/`).
    UrlPrefix,
}

impl PropertyKind {
    /// Beschriftung im Property-Dropdown.
    pub fn label(self) -> &'static str {
        match self {
            PropertyKind::Domain => "Domain",
            PropertyKind::UrlPrefix => "URL-Präfix",
        }
    }
}

/// Berechtigungsstufe, mit der eine Property ausgegraut wird (INTEG-110).
pub const UNVERIFIED: &str = "siteUnverifiedUser";

/// Warnhinweis bei abweichender Crawl-Domain (INTEG-110).
pub const MISMATCH_WARNING: &str = "Property passt nicht zur Crawl-Domain";

/// Eine Property aus `sites.list`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    /// Kennung der API (`sc-domain:integ.test`, `https://integ.test/`).
    pub site_url: String,
    /// Berechtigungsstufe (`siteOwner`, `siteFullUser`, `siteRestrictedUser`, `siteUnverifiedUser`).
    pub permission: String,
    /// Art.
    pub kind: PropertyKind,
}

impl Property {
    /// Property aus Kennung und Berechtigung.
    pub fn new(site_url: &str, permission: &str) -> Property {
        let kind = if site_url.starts_with("sc-domain:") {
            PropertyKind::Domain
        } else {
            PropertyKind::UrlPrefix
        };
        Property {
            site_url: site_url.to_string(),
            permission: permission.to_string(),
            kind,
        }
    }

    /// Auswählbar (verifiziert)? Unverifizierte Properties werden ausgegraut.
    pub fn selectable(&self) -> bool {
        self.permission != UNVERIFIED
    }

    /// Beschriftung im Dropdown, z. B. `integ.test (Domain)` bzw. `https://integ.test/ (URL-Präfix)`.
    pub fn label(&self) -> String {
        let name = self
            .site_url
            .strip_prefix("sc-domain:")
            .unwrap_or(&self.site_url);
        format!("{name} ({})", self.kind.label())
    }

    /// Host einer Domain-Property.
    fn domain(&self) -> Option<&str> {
        self.site_url.strip_prefix("sc-domain:")
    }

    /// Gehört die URL zur Property?
    pub fn covers(&self, url: &str) -> bool {
        let Ok(u) = Url::parse(url) else {
            return false;
        };
        if !matches!(u.scheme(), "http" | "https") {
            return false;
        }
        match self.domain() {
            Some(domain) => {
                let host = u.host_str().unwrap_or_default().to_ascii_lowercase();
                let domain = domain.to_ascii_lowercase();
                host == domain || host.ends_with(&format!(".{domain}"))
            }
            None => Url::parse(&self.site_url).is_ok_and(|prefix| {
                let prefix = prefix.as_str();
                let url = u.as_str();
                url.starts_with(prefix) || format!("{url}/") == prefix
            }),
        }
    }

    /// Länge des passenden Präfixes für die Wahl der spezifischsten Property; Domain-Properties zählen 0.
    fn specificity(&self) -> usize {
        match self.kind {
            PropertyKind::Domain => 0,
            PropertyKind::UrlPrefix => self.site_url.len(),
        }
    }
}

/// Liest die Antwort von `sites.list` (`siteEntry[]`); Properties nach Kennung sortiert.
pub fn parse_sites(json: &serde_json::Value) -> Vec<Property> {
    let mut out: Vec<Property> = json["siteEntry"]
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter_map(|e| {
                    Some(Property::new(
                        e["siteUrl"].as_str()?,
                        e["permissionLevel"].as_str().unwrap_or_default(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| a.site_url.cmp(&b.site_url));
    out
}

/// Gefiltert nach Suchtext (Suchfeld des Dropdowns, ohne Beachtung der Groß-/Kleinschreibung).
pub fn search<'a>(properties: &'a [Property], text: &str) -> Vec<&'a Property> {
    let text = text.to_lowercase();
    properties
        .iter()
        .filter(|p| p.site_url.to_lowercase().contains(&text))
        .collect()
}

/// Passt die Property zur Crawl-Start-URL? Sonst erscheint [`MISMATCH_WARNING`].
pub fn matches_crawl(property: &Property, start_url: &str) -> bool {
    property.covers(start_url)
}

/// Warnhinweis zur gewählten Property, falls sie nicht zur Crawl-Domain passt.
pub fn crawl_warning(property: &Property, start_url: &str) -> Option<&'static str> {
    (!matches_crawl(property, start_url)).then_some(MISMATCH_WARNING)
}

/// Die spezifischste verifizierte Property für die URL: längster passender URL-Präfix, Domain-Property als Rückfall
/// (INTEG-122).
pub fn most_specific<'a>(url: &str, properties: &'a [Property]) -> Option<&'a Property> {
    properties
        .iter()
        .filter(|p| p.selectable() && p.covers(url))
        .max_by(|a, b| {
            a.specificity().cmp(&b.specificity()).then_with(|| {
                // Bei mehreren Domain-Properties gewinnt die längere (spezifischere) Domain.
                a.site_url.len().cmp(&b.site_url.len())
            })
        })
}

/// Status der Verbindung `Verbunden (<Konto> / <Property>)` (INTEG-110).
pub fn connected_label(account: &str, property: &str) -> String {
    format!("Verbunden ({account} / {property})")
}

/// Stabile Konto-Kennung und Beschriftung aus der Property-Liste (INTEG-003/110). Ohne Identitätsscope liefert Google
/// keine Konto-ID; die Kennung ist daher ein Hash über die sortierten Properties mit Berechtigung, die Beschriftung
/// nennt die erste auswählbare Property und die Zahl der weiteren. Ein erneutes Verbinden desselben Kontos ergibt
/// dieselbe Kennung und überschreibt den gespeicherten Eintrag.
pub fn account_identity(properties: &[Property]) -> (String, String) {
    let mut keys: Vec<String> = properties
        .iter()
        .map(|p| format!("{}\u{1f}{}", p.site_url, p.permission))
        .collect();
    keys.sort();
    // FNV-1a, 64 Bit: kein Geheimnis, nur eine stabile Kennung.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in keys.join("\u{1e}").bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    let selectable: Vec<&Property> = properties.iter().filter(|p| p.selectable()).collect();
    let label = match selectable.split_first() {
        None => "Search Console".to_string(),
        Some((first, [])) => first.label(),
        Some((first, rest)) => format!("{} (+{})", first.label(), rest.len()),
    };
    (format!("gsc-{hash:016x}"), label)
}
