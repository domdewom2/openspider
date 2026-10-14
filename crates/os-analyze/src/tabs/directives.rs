//! Tab „Directives“ (TECH-051 bis TECH-056, TECH-064).
//!
//! **Grundmenge (TECH-051):** alle internen Tabellenzeilen mit einer HTTP-Antwort (jeder Status), weil
//! `X-Robots-Tag` auch an Nicht-HTML- und Nicht-200-Antworten hängt. Die robots.txt hat keine
//! Tabellen-Sichtbarkeit und gehört nicht dazu (K-54, ersetzt K-45).
//!
//! **Filter** prüfen das Vorhandensein des Tokens in irgendeiner wirksamen Quelle (Meta oder Header, generisch
//! oder zum Agent-Token passend: `directives.applies = 1`), nicht die effektive Wirkung (TECH-066): `index` und
//! `noindex` zugleich stehen in beiden Filtern, `all` zählt nicht als `index`, `none` nicht als `noindex`. Die
//! Tokens stehen kommagetrennt in `directives.tokens`; verglichen wird mit Kommagrenzen.

use crate::defs::{ColumnDef, ColumnKind, FilterDef, FilterKind, TabDef};

use super::canonicals::{
    canonical_link_element_column, column, extension_column, http_canonical_column,
    meta_robots_column, multi_column, x_robots_tag_column,
};

/// Grundmenge: interne Zeilen mit Antwort.
const POPULATION: &str = "u.is_internal = 1 AND u.in_tables = 1 AND COALESCE(u.status_code, 0) > 0";

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
            // [OS-Erweiterung]: Wirkung der Angaben (TECH-066) und Indexierbarkeit.
            extension_column("Indexability", "u.indexability", ColumnKind::Text),
            extension_column(
                "Indexability Status",
                "u.indexability_status",
                ColumnKind::Text,
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
        ],
        default_sort: Vec::new(),
        params: |_| Vec::new(),
    }
}
