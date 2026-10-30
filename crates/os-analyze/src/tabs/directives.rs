//! Tab „Directives“ (TECH-051 bis TECH-068).
//!
//! **Grundmenge (TECH-051):** alle internen Tabellenzeilen mit einer HTTP-2xx-Antwort, weil
//! `X-Robots-Tag` auch an Nicht-HTML- und Nicht-200-Antworten hängt. Die robots.txt hat keine
//! Tabellen-Sichtbarkeit und gehört nicht dazu (K-54, ersetzt K-45).
//!
//! **Filter** prüfen das Vorhandensein des Tokens in irgendeiner wirksamen Quelle (Meta oder Header, generisch
//! oder zum Agent-Token passend: `directives.applies = 1`), nicht die effektive Wirkung (TECH-066): `index` und
//! `noindex` zugleich stehen in beiden Filtern, `all` zählt nicht als `index`, `none` nicht als `noindex`. Die
//! Tokens stehen kommagetrennt in `directives.tokens`; verglichen wird mit Kommagrenzen.

use crate::defs::{
    register_coverage_domain, ColumnDef, ColumnKind, FilterDef, FilterKind, Requirement, TabDef,
};

use super::canonicals::{
    canonical_link_element_column, column, extension_column, http_canonical_column,
    meta_robots_column, multi_column, x_robots_tag_column,
};

/// Grundmenge: interne Zeilen mit Antwort.
const POPULATION: &str =
    "u.is_internal = 1 AND u.in_tables = 1 AND u.status_code BETWEEN 200 AND 299";

/// Prädikat „Token vorhanden“ (Kommagrenzen, nur wirksame Direktiven).
macro_rules! has_token {
    ($token:literal) => {
        concat!(
            "EXISTS (SELECT 1 FROM directives d WHERE d.url_id = u.id AND d.applies = 1 \
             AND (',' || d.tokens || ',') LIKE '%,",
            $token,
            ",%')"
        )
    };
}

fn filter(id: &'static str, predicate: &'static str) -> FilterDef {
    FilterDef {
        id,
        predicate,
        kind: FilterKind::Live,
        requires: Vec::new(),
        // CRAWL-089: Die Directives-Filter schränkt die Option nie ein (TECH-053 u. a.).
        ignore_non_indexable: false,
        default_columns: &[],
    }
}

/// `Meta Refresh n`: Rohwert; Meta-Elemente zuerst (Quelltextreihenfolge), dann `Refresh`-Header.
fn meta_refresh_column() -> ColumnDef {
    multi_column(
        "Meta Refresh",
        "SELECT r.raw FROM refresh r WHERE r.url_id = ?1 \
         ORDER BY CASE r.source WHEN 'meta' THEN 0 ELSE 1 END, r.position",
        ColumnKind::Text,
    )
}

/// Definition des Tabs „Directives“.
pub fn tab() -> TabDef {
    // TECH-008/TECH-065: only parsed HTML responses can carry the validation marker; PDFs and images in the
    // population (X-Robots-Tag) must not count as unchecked (N01-C01).
    register_coverage_domain(
        "Directives",
        "validation.checked",
        "u.content_class = 'HTML'",
    );
    TabDef {
        id: "Directives",
        population: POPULATION,
        columns: vec![
            column("Address", "u.address", ColumnKind::Url, true),
            column(
                "Occurrences",
                "(SELECT COUNT(*) FROM directives d WHERE d.url_id = u.id)",
                ColumnKind::Int,
                true,
            ),
            meta_robots_column(),
            x_robots_tag_column(),
            meta_refresh_column(),
            canonical_link_element_column(),
            http_canonical_column(),
            // M2-S01: Indexierbarkeit gehört zu den Standardspalten.
            column("Indexability", "u.indexability", ColumnKind::Text, true),
            column(
                "Indexability Status",
                "u.indexability_status",
                ColumnKind::Text,
                true,
            ),
            extension_column("Effective Index", "u.effective_index", ColumnKind::Bool),
            extension_column("Effective Follow", "u.effective_follow", ColumnKind::Bool),
        ],
        filters: vec![
            filter("All", "1"),
            filter("Index", has_token!("index")),
            filter("Noindex", has_token!("noindex")),
            filter("Follow", has_token!("follow")),
            filter("Nofollow", has_token!("nofollow")),
            filter("None", has_token!("none")),
            filter(
                "Refresh",
                "EXISTS (SELECT 1 FROM refresh r WHERE r.url_id = u.id)",
            ),
            filter("NoArchive", has_token!("noarchive")),
            filter("NoSnippet", has_token!("nosnippet")),
            filter("Max-Snippet", has_token!("max-snippet:%")),
            filter("Max-Image-Preview", has_token!("max-image-preview:%")),
            filter("Max-Video-Preview", has_token!("max-video-preview:%")),
            filter("NoImageIndex", has_token!("noimageindex")),
            filter("NoTranslate", concat!("EXISTS (SELECT 1 FROM directives d WHERE d.url_id = u.id ",
                "AND (d.applies = 1 OR (d.source = 'meta_robots' AND d.agent = 'google')) ",
                "AND (',' || d.tokens || ',') LIKE '%,notranslate,%')")),
            // LIKE behandelt auch `_` als Platzhalter (kein ESCAPE). Das Muster erfasst
            // die normalisierten unavailable_after-Tokens; ein strikt literaler
            // Unterstrich-Abgleich müsste `_` mit einer ESCAPE-Klausel maskieren.
            filter("Unavailable_After", has_token!("unavailable_after:%")),
            FilterDef {
                requires: vec![Requirement::Coverage {
                    config: "advanced.html_validation",
                    marker: "validation.checked",
                }],
                ..filter("Outside <head>", "EXISTS (SELECT 1 FROM directives d WHERE d.url_id = u.id AND d.source = 'meta_robots' AND d.in_head = 0)")
            },
        ],
        default_sort: Vec::new(),
        params: |_| Vec::new(),
        hidden_population: Some("u.is_internal = 1 AND u.status_code BETWEEN 200 AND 299 AND u.indexability_status = 'noindex' AND EXISTS (SELECT 1 FROM url_flags f WHERE f.url_id = u.id AND f.flag = 'respect_hidden')"),
    }
}

/// URL-Beiträge des Fachpakets M2-D06.
pub fn url_details(
    file: &os_store::CrawlFile,
    _: &os_model::CrawlConfig,
    id: os_model::UrlId,
) -> Result<Vec<(String, String)>, crate::ViewError> {
    let mut stmt = file
        .conn()
        .prepare("SELECT tokens FROM directives WHERE url_id = ?1 AND applies = 1")?;
    let mut rows = stmt.query([id.0])?;
    let mut smallest: Option<u64> = None;
    while let Some(row) = rows.next()? {
        let tokens: String = row.get(0)?;
        for token in tokens.split(',') {
            if let Some(value) = token.strip_prefix("max-snippet:") {
                if let Ok(n) = value.parse::<u64>() {
                    smallest = Some(smallest.map_or(n, |old| old.min(n)));
                }
            }
        }
    }
    Ok(smallest
        .map(|n| vec![("Max-Snippet".into(), n.to_string())])
        .unwrap_or_default())
}
