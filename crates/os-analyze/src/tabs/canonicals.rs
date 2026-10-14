//! Tab „Canonicals“ (TECH-011 bis TECH-018).
//!
//! **Grundmenge (TECH-011):** interne Tabellenzeilen mit Status 2xx (K-56) und HTML-Inhalt, dazu interne
//! Nicht-HTML-2xx-URLs mit mindestens einer HTTP-Canonical. Ist `spider.canonicals.store` aus (CRAWL-046),
//! bleibt die Grundmenge leer: es gibt weder Detailzeilen noch Spaltenwerte, und es entsteht kein
//! `Canonicalised`.
//!
//! **Filter** (alle Live, außer „Non-Indexable Canonical“): gerechnet wird mit den Seitenfakten der Engine
//! (`canonical_count`, maßgebliche `canonical_url` nach TECH-017, `canonical_self` nach TECH-002) und den
//! Zeilen der Tabelle `canonicals`. „Multiple Conflicting“ zählt die verschiedenen aufgelösten Canonical-
//! URLs nach Normalisierung (Fragment entfernt, Prozent-Kodierung normalisiert); ungültige Rohwerte
//! (`resolved IS NULL`) zählen nicht mit. „Non-Indexable Canonical“ liest die Markierung
//! `non_indexable_canonical` aus `url_flags`, die der Nachlauf-Schritt „Kanonisierungen“ setzt.

use std::sync::OnceLock;

use os_model::{AnalysisStep, CrawlConfig, Value};

use crate::defs::{ColumnDef, ColumnKind, ColumnSql, FilterDef, FilterKind, TabDef};

/// Name der Markierung in `url_flags` (TECH-018), vom Schritt `analysis::steps::canonicals` gesetzt.
pub const FLAG_NON_INDEXABLE_CANONICAL: &str = "non_indexable_canonical";

/// Grundmenge: interne HTML-2xx-Zeilen und interne Nicht-HTML-2xx-URLs mit HTTP-Canonical (K-56).
const POPULATION: &str =
    "u.is_internal = 1 AND u.in_tables = 1 AND u.status_code BETWEEN 200 AND 299 \
     AND :canon_store = 1 \
     AND (u.content_class = 'HTML' \
          OR EXISTS (SELECT 1 FROM canonicals c WHERE c.url_id = u.id AND c.source = 'http'))";

/// Zählt die verschiedenen normalisierten Canonical-Ziele einer URL.
fn conflicting_predicate() -> &'static str {
    static SQL: OnceLock<String> = OnceLock::new();
    SQL.get_or_init(|| {
        format!(
            "(SELECT COUNT(DISTINCT {}) FROM canonicals c WHERE c.url_id = u.id AND c.resolved IS NOT NULL) >= 2",
            normalized_url_sql("c.resolved")
        )
    })
}

/// SQL-Ausdruck für den Vergleichsschlüssel einer aufgelösten URL (TECH-002), soweit er nicht schon durch die
/// URL-Auflösung entsteht (Schema, Host, Standardport): Fragment entfernen, Hexziffern von `%XX` groß
/// schreiben und unreservierte Zeichen (`A-Z a-z 0-9 - . _ ~`) dekodieren.
fn normalized_url_sql(col: &str) -> String {
    let mut e =
        format!("CASE WHEN instr({col}, '#') > 0 THEN substr({col}, 1, instr({col}, '#') - 1) ELSE {col} END");
    let wrap = |inner: String, from: &str, to: &str| format!("replace({inner}, '{from}', '{to}')");
    // erste Hexziffer groß, dann zweite (die erste ist danach schon groß)
    for first in 'a'..='f' {
        e = wrap(
            e,
            &format!("%{first}"),
            &format!("%{}", first.to_ascii_uppercase()),
        );
    }
    for first in "0123456789ABCDEF".chars() {
        for second in 'a'..='f' {
            e = wrap(
                e,
                &format!("%{first}{second}"),
                &format!("%{first}{}", second.to_ascii_uppercase()),
            );
        }
    }
    for byte in 0u8..=127 {
        let c = char::from(byte);
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~') {
            e = wrap(e, &format!("%{byte:02X}"), &c.to_string());
        }
    }
    e
}

pub(crate) fn column(
    key: &'static str,
    sql: &'static str,
    kind: ColumnKind,
    default_visible: bool,
) -> ColumnDef {
    ColumnDef {
        key,
        sql: ColumnSql::Expr(sql),
        kind,
        default_visible,
        extension: false,
    }
}

/// `[OS-Erweiterung]`-Spalte: im Standardexport ausgeblendet.
pub(crate) fn extension_column(
    key: &'static str,
    sql: &'static str,
    kind: ColumnKind,
) -> ColumnDef {
    ColumnDef {
        key,
        sql: ColumnSql::Expr(sql),
        kind,
        default_visible: false,
        extension: true,
    }
}

pub(crate) fn multi_column(
    base: &'static str,
    list_sql: &'static str,
    kind: ColumnKind,
) -> ColumnDef {
    ColumnDef {
        key: base,
        sql: ColumnSql::Multi { base, list_sql },
        kind,
        default_visible: true,
        extension: false,
    }
}

/// `Canonical Link Element n`: aufgelöste URL (sonst Rohwert) der HTML-Elemente in Quelltextreihenfolge.
pub(crate) fn canonical_link_element_column() -> ColumnDef {
    multi_column(
        "Canonical Link Element",
        "SELECT COALESCE(c.resolved, c.raw) FROM canonicals c \
         WHERE c.url_id = ?1 AND c.source = 'html' ORDER BY c.position",
        ColumnKind::Url,
    )
}

/// `HTTP Canonical n`: Einträge der `Link`-Header in Header-Reihenfolge.
pub(crate) fn http_canonical_column() -> ColumnDef {
    multi_column(
        "HTTP Canonical",
        "SELECT COALESCE(c.resolved, c.raw) FROM canonicals c \
         WHERE c.url_id = ?1 AND c.source = 'http' ORDER BY c.position",
        ColumnKind::Url,
    )
}

/// `Meta Robots n`: Rohinhalt von `content`, bei agent-spezifischem Tag mit Präfix (`googlebot: noindex`).
pub(crate) fn meta_robots_column() -> ColumnDef {
    multi_column(
        "Meta Robots",
        "SELECT CASE WHEN d.agent IS NULL THEN d.raw ELSE d.agent || ': ' || d.raw END \
         FROM directives d WHERE d.url_id = ?1 AND d.source = 'meta_robots' ORDER BY d.position",
        ColumnKind::Text,
    )
}

/// `X-Robots-Tag n`: Rohwerte je Header-Abschnitt (mit Agent-Präfix, wie geliefert).
pub(crate) fn x_robots_tag_column() -> ColumnDef {
    multi_column(
        "X-Robots-Tag",
        "SELECT d.raw FROM directives d \
         WHERE d.url_id = ?1 AND d.source = 'x_robots_tag' ORDER BY d.position",
        ColumnKind::Text,
    )
}

fn params(cfg: &CrawlConfig) -> Vec<(&'static str, Value)> {
    vec![(
        "canon_store",
        Value::Int(i64::from(cfg.spider.canonicals.store)),
    )]
}

fn filter(id: &'static str, predicate: &'static str, ignore_non_indexable: bool) -> FilterDef {
    FilterDef {
        id,
        predicate,
        kind: FilterKind::Live,
        requires: Vec::new(),
        // CRAWL-089/TECH-015 (K-56): Nur „Missing“ schließt bei der Option nicht indexierbare Seiten aus.
        ignore_non_indexable,
    }
}

/// Definition des Tabs „Canonicals“.
pub fn tab() -> TabDef {
    TabDef {
        id: "Canonicals",
        population: POPULATION,
        columns: vec![
            column("Address", "u.address", ColumnKind::Url, true),
            column("Occurrences", "u.canonical_count", ColumnKind::Int, true),
            column("Indexability", "u.indexability", ColumnKind::Text, true),
            column(
                "Indexability Status",
                "u.indexability_status",
                ColumnKind::Text,
                true,
            ),
            canonical_link_element_column(),
            http_canonical_column(),
            meta_robots_column(),
            x_robots_tag_column(),
            column(
                "rel=\"next\" 1",
                "(SELECT COALESCE(p.url, p.raw) FROM pagination p \
                  WHERE p.url_id = u.id AND p.rel = 'next' ORDER BY p.position LIMIT 1)",
                ColumnKind::Url,
                true,
            ),
            column(
                "rel=\"prev\" 1",
                "(SELECT COALESCE(p.url, p.raw) FROM pagination p \
                  WHERE p.url_id = u.id AND p.rel = 'prev' ORDER BY p.position LIMIT 1)",
                ColumnKind::Url,
                true,
            ),
            // [OS-Erweiterung]: die maßgebliche Canonical-URL nach TECH-017.
            extension_column("Canonical URL", "u.canonical_url", ColumnKind::Url),
        ],
        filters: vec![
            filter("All", "1", false),
            filter("Contains Canonical", "u.canonical_count >= 1", false),
            filter(
                "Self Referencing",
                "u.canonical_count >= 1 AND u.canonical_self = 1",
                false,
            ),
            filter(
                "Canonicalised",
                "u.canonical_count >= 1 AND u.canonical_self = 0",
                false,
            ),
            filter(
                "Missing",
                "u.content_class = 'HTML' AND u.canonical_count = 0",
                true,
            ),
            filter("Multiple", "u.canonical_count >= 2", false),
            filter("Multiple Conflicting", conflicting_predicate(), false),
            FilterDef {
                id: "Non-Indexable Canonical",
                predicate: "EXISTS (SELECT 1 FROM url_flags f \
                            WHERE f.url_id = u.id AND f.flag = 'non_indexable_canonical')",
                kind: FilterKind::PostCrawl(AnalysisStep::Canonicals),
                requires: Vec::new(),
                ignore_non_indexable: false,
            },
        ],
        default_sort: Vec::new(),
        params,
    }
}
