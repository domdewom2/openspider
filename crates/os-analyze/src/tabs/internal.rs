//! Tab „Internal“ (ONPAGE-010 … ONPAGE-015, ONPAGE-018, ONPAGE-021, ONPAGE-022, ONPAGE-024).
//!
//! Grundmenge: alle internen URLs, die in den Tabellen erscheinen (`in_tables = 1`, K-40): auch
//! Weiterleitungen, Fehlerseiten, durch robots.txt gesperrte URLs und Ressourcen. Sitemap-Dateien und die
//! robots.txt jedes Hosts haben `in_tables = 0` und fehlen (K-54, CRAWL-098). Die Klassenfilter folgen
//! `urls.content_class` (ONPAGE-020).
//!
//! Spalten in der Reihenfolge der Spec; die zweiten Vorkommen von Titel, Beschreibung, Keywords, H1 und
//! H2 stehen am Ende. Die Spalten dieses Moduls dienen auch dem Tab „Security“ (`security.rs`) und dem Tab
//! „External“ (`external.rs`).

use crate::defs::{ColumnDef, ColumnKind, ColumnSql, FilterDef, FilterKind, TabDef};

/// Grundmenge der internen URLs.
pub(crate) const POPULATION: &str = "u.is_internal = 1 AND u.in_tables = 1";

/// `% of Total` (ONPAGE-015, K-56): verschiedene interne HTML-Quellseiten mit Status 2xx und mindestens einem
/// Hyperlink auf die URL (Selbstlinks zählen mit, A-9) ÷ Anzahl interner HTML-Seiten mit 2xx × 100. Ohne solche
/// Seiten leer. Der Zähler der URL steht in `url_counters.html_2xx_sources` (K-74/K-75): live, exakt und ohne den
/// Index `links_target`; die Gesamtzahl ist eine unkorrelierte Teilabfrage und wird einmal je Abfrage berechnet.
const PCT_OF_TOTAL: &str = "CASE WHEN (SELECT COUNT(*) FROM urls h WHERE h.is_internal = 1 AND \
     h.content_class = 'HTML' AND h.status_code BETWEEN 200 AND 299) = 0 THEN NULL ELSE \
     100.0 * c.html_2xx_sources / \
     (SELECT COUNT(*) FROM urls h WHERE h.is_internal = 1 AND h.content_class = 'HTML' AND \
     h.status_code BETWEEN 200 AND 299) END";

/// `Redirect Type` in der Bezeichnung der Tabs (ONPAGE-018, K-03).
pub(crate) const REDIRECT_TYPE: &str = "CASE u.redirect_type WHEN 'http' THEN 'HTTP Redirect' \
     WHEN 'hsts_policy' THEN 'HSTS Policy' WHEN 'javascript' THEN 'JavaScript Redirect' \
     WHEN 'meta_refresh' THEN 'Meta Refresh Redirect' WHEN 'http_refresh' THEN 'HTTP Refresh Redirect' \
     ELSE NULL END";

/// `Size (bytes)` (ONPAGE-012, A-6): HTML und Bilder = Länge des dekomprimierten Bodys; alle anderen
/// Ressourcen = Wert des `Content-Length`-Headers, ohne Header 0 (z. B. chunked PDF).
const SIZE: &str =
    "CASE WHEN u.size_bytes IS NULL OR u.content_class IN ('HTML', 'Images') THEN u.size_bytes \
     ELSE COALESCE((SELECT CAST(h.value AS INTEGER) FROM http_headers h WHERE h.url_id = u.id AND \
     h.direction = 'response' AND lower(h.name) = 'content-length' LIMIT 1), 0) END";

/// `HTTP Version` ohne Präfix: `1.1`, `2` (ONPAGE-018); gespeichert ist die Form `HTTP/1.1`.
const HTTP_VERSION: &str = "CASE WHEN u.http_version LIKE 'HTTP/%' THEN substr(u.http_version, 6) \
     ELSE u.http_version END";

/// Bedingung für die Element-Spalten des Internal-Tabs (ONPAGE-011, K-60): nur interne HTML-Seiten mit
/// Status 2xx. Weiterleitungen, Fehlerseiten und URLs ohne Antwort haben leere Zellen, auch wenn ihr Körper
/// `<title>` oder `<h1>` enthält (A-5: der Inhalt wird nicht ausgewertet).
macro_rules! html_2xx {
    ($e:expr) => {
        concat!(
            "CASE WHEN u.content_class = 'HTML' AND u.status_code BETWEEN 200 AND 299 THEN ",
            $e,
            " END"
        )
    };
}

/// Wie `html_2xx!`, zusätzlich PDFs mit Status 2xx (Titel und Keywords aus den Dokumenteigenschaften,
/// ONPAGE-009/-166).
macro_rules! html_or_pdf_2xx {
    ($e:expr) => {
        concat!(
            "CASE WHEN u.content_class IN ('HTML', 'PDF') AND u.status_code BETWEEN 200 AND 299 THEN ",
            $e,
            " END"
        )
    };
}

/// Bausteine der Spaltenlisten (auch für `security.rs` und `external.rs`).
pub(crate) fn col(
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

/// Spalte mit `extension = true`: standardmäßig ausgeblendet (`[OS-Erweiterung]`).
pub(crate) fn ext_col(key: &'static str, sql: &'static str, kind: ColumnKind) -> ColumnDef {
    ColumnDef {
        key,
        sql: ColumnSql::Expr(sql),
        kind,
        default_visible: false,
        extension: true,
    }
}

/// Mehrfachspalte `<Basis> 1 … n` (TECH-004), sichtbar.
pub(crate) fn multi(base: &'static str, list_sql: &'static str, kind: ColumnKind) -> ColumnDef {
    ColumnDef {
        key: base,
        sql: ColumnSql::Multi { base, list_sql },
        kind,
        default_visible: true,
        extension: false,
    }
}

/// Alle `Meta Robots`-Angaben der URL, die für den eigenen Crawler gelten, in Quelltextreihenfolge.
pub(crate) const META_ROBOTS_LIST: &str = "SELECT raw FROM directives WHERE url_id = ?1 AND \
     source = 'meta_robots' AND applies = 1 ORDER BY position";

/// Alle `X-Robots-Tag`-Werte der URL in Header-Reihenfolge.
pub(crate) const X_ROBOTS_LIST: &str = "SELECT raw FROM directives WHERE url_id = ?1 AND \
     source = 'x_robots_tag' AND applies = 1 ORDER BY position";

/// Alle `<link rel=\"canonical\">` im HTML (absolut aufgelöst), in Quelltextreihenfolge.
pub(crate) const CANONICAL_LIST: &str = "SELECT COALESCE(resolved, raw) FROM canonicals WHERE \
     url_id = ?1 AND source = 'html' ORDER BY position";

/// Die Basisspalten `Address … Indexability Status` (ONPAGE-010), gemeinsam für Internal und Security.
pub(crate) fn base_columns() -> Vec<ColumnDef> {
    vec![
        col("Address", "u.address", ColumnKind::Url, true),
        col("Content", "u.content_type", ColumnKind::Text, true),
        col("Status Code", "u.status_code", ColumnKind::Int, true),
        col("Status", "u.status", ColumnKind::Text, true),
        col("Indexability", "u.indexability", ColumnKind::Text, true),
        col(
            "Indexability Status",
            "u.indexability_status",
            ColumnKind::Text,
            true,
        ),
    ]
}

/// Filter eines Inhaltstyps (`urls.content_class`).
fn class_filter(id: &'static str, predicate: &'static str) -> FilterDef {
    FilterDef {
        id,
        predicate,
        kind: FilterKind::Live,
        requires: Vec::new(),
        ignore_non_indexable: false,
    }
}

/// Die Klassenfilter `HTML`, `JavaScript`, `CSS`, `Images`, `PDF` (ONPAGE-021/022).
pub(crate) fn class_filters() -> Vec<FilterDef> {
    vec![
        class_filter("HTML", "u.content_class = 'HTML'"),
        class_filter("JavaScript", "u.content_class = 'JavaScript'"),
        class_filter("CSS", "u.content_class = 'CSS'"),
        class_filter("Images", "u.content_class = 'Images'"),
        class_filter("PDF", "u.content_class = 'PDF'"),
    ]
}

/// Der Filter `Unknown` (ONPAGE-024).
pub(crate) fn unknown_filter() -> FilterDef {
    class_filter("Unknown", "u.content_class = 'Unknown'")
}

/// Definition des Tabs „Internal“.
pub fn tab() -> TabDef {
    let mut columns = base_columns();
    columns.extend([
        // ONPAGE-011: Kernwerte der Element-Tabs und Direktiven
        col("Title 1", html_or_pdf_2xx!("u.title_1"), ColumnKind::Text, true),
        col("Title 1 Length", html_or_pdf_2xx!("u.title_1_len"), ColumnKind::Int, true),
        col(
            "Title 1 Pixel Width",
            html_or_pdf_2xx!("u.title_1_px"),
            ColumnKind::Int,
            true,
        ),
        col(
            "Meta Description 1",
            html_2xx!("u.meta_desc_1"),
            ColumnKind::Text,
            true,
        ),
        col(
            "Meta Description 1 Length",
            html_2xx!("u.meta_desc_1_len"),
            ColumnKind::Int,
            true,
        ),
        col(
            "Meta Description 1 Pixel Width",
            html_2xx!("u.meta_desc_1_px"),
            ColumnKind::Int,
            true,
        ),
        col("Meta Keywords 1", html_or_pdf_2xx!("u.meta_kw_1"), ColumnKind::Text, true),
        col(
            "Meta Keywords 1 Length",
            html_or_pdf_2xx!("u.meta_kw_1_len"),
            ColumnKind::Int,
            true,
        ),
        col("H1-1", html_2xx!("u.h1_1"), ColumnKind::Text, true),
        col("H1-1 Length", html_2xx!("u.h1_1_len"), ColumnKind::Int, true),
        col("H2-1", html_2xx!("u.h2_1"), ColumnKind::Text, true),
        col("H2-1 Length", html_2xx!("u.h2_1_len"), ColumnKind::Int, true),
        multi("Meta Robots", META_ROBOTS_LIST, ColumnKind::Text),
        multi("X-Robots-Tag", X_ROBOTS_LIST, ColumnKind::Text),
        col(
            "Meta Refresh 1",
            "(SELECT raw FROM refresh WHERE url_id = u.id AND source = 'meta' ORDER BY position LIMIT 1)",
            ColumnKind::Text,
            true,
        ),
        multi("Canonical Link Element", CANONICAL_LIST, ColumnKind::Url),
        col(
            "rel=\"next\" 1",
            "(SELECT COALESCE(url, raw) FROM pagination WHERE url_id = u.id AND rel = 'next' AND source = 'html' ORDER BY position LIMIT 1)",
            ColumnKind::Url,
            true,
        ),
        col(
            "rel=\"prev\" 1",
            "(SELECT COALESCE(url, raw) FROM pagination WHERE url_id = u.id AND rel = 'prev' AND source = 'html' ORDER BY position LIMIT 1)",
            ColumnKind::Url,
            true,
        ),
        col(
            "HTTP rel=\"next\" 1",
            "(SELECT COALESCE(url, raw) FROM pagination WHERE url_id = u.id AND rel = 'next' AND source = 'http' ORDER BY position LIMIT 1)",
            ColumnKind::Url,
            true,
        ),
        col(
            "HTTP rel=\"prev\" 1",
            "(SELECT COALESCE(url, raw) FROM pagination WHERE url_id = u.id AND rel = 'prev' AND source = 'http' ORDER BY position LIMIT 1)",
            ColumnKind::Url,
            true,
        ),
        // ONPAGE-012: Größen
        col("Size (bytes)", SIZE, ColumnKind::Int, true),
        col(
            "Transferred (bytes)",
            "u.transferred_bytes",
            ColumnKind::Int,
            true,
        ),
        col(
            "Total Transferred (bytes)",
            "u.total_transferred_bytes",
            ColumnKind::Int,
            true,
        ),
        // ONPAGE-013
        col("Word Count", html_2xx!("u.word_count"), ColumnKind::Int, true),
        col(
            "Text Ratio",
            html_2xx!("u.text_ratio"),
            ColumnKind::Float { decimals: 1 },
            true,
        ),
        // ONPAGE-014
        col("Crawl Depth", "u.crawl_depth", ColumnKind::Int, true),
        col("Folder Depth", "u.folder_depth", ColumnKind::Int, true),
        // ONPAGE-015: Selbstlinks zählen mit (A-9, K-07)
        col("Inlinks", "c.inlinks", ColumnKind::Int, true),
        col("Unique Inlinks", "c.unique_inlinks", ColumnKind::Int, true),
        col(
            "% of Total",
            PCT_OF_TOTAL,
            ColumnKind::Float { decimals: 2 },
            true,
        ),
        col("Outlinks", "c.outlinks", ColumnKind::Int, true),
        col("Unique Outlinks", "c.unique_outlinks", ColumnKind::Int, true),
        col(
            "External Outlinks",
            "c.external_outlinks",
            ColumnKind::Int,
            true,
        ),
        col(
            "Unique External Outlinks",
            "c.unique_external_outlinks",
            ColumnKind::Int,
            true,
        ),
        // ONPAGE-018
        col("Hash", "u.hash_md5", ColumnKind::Hash, true),
        col(
            "Response Time",
            "u.response_time_ms",
            ColumnKind::Seconds3,
            true,
        ),
        col("Last-Modified", "u.last_modified_raw", ColumnKind::Text, true),
        col("Redirect URL", "u.redirect_url", ColumnKind::Url, true),
        col("Redirect Type", REDIRECT_TYPE, ColumnKind::Text, true),
        col("HTTP Version", HTTP_VERSION, ColumnKind::Text, true),
        col("URL Encoded Address", "u.url", ColumnKind::Url, true),
        col(
            "Crawl Timestamp",
            "u.crawl_timestamp",
            ColumnKind::Timestamp,
            true,
        ),
        // zweite Vorkommen am Ende der Tabelle (ONPAGE-011)
        col("Title 2", html_or_pdf_2xx!("u.title_2"), ColumnKind::Text, true),
        col("Title 2 Length", html_or_pdf_2xx!("u.title_2_len"), ColumnKind::Int, true),
        col(
            "Title 2 Pixel Width",
            html_or_pdf_2xx!("u.title_2_px"),
            ColumnKind::Int,
            true,
        ),
        col(
            "Meta Description 2",
            html_2xx!("u.meta_desc_2"),
            ColumnKind::Text,
            true,
        ),
        col(
            "Meta Description 2 Length",
            html_2xx!("u.meta_desc_2_len"),
            ColumnKind::Int,
            true,
        ),
        col(
            "Meta Description 2 Pixel Width",
            html_2xx!("u.meta_desc_2_px"),
            ColumnKind::Int,
            true,
        ),
        col("Meta Keywords 2", html_or_pdf_2xx!("u.meta_kw_2"), ColumnKind::Text, true),
        col(
            "Meta Keywords 2 Length",
            html_or_pdf_2xx!("u.meta_kw_2_len"),
            ColumnKind::Int,
            true,
        ),
        col("H1-2", html_2xx!("u.h1_2"), ColumnKind::Text, true),
        col("H1-2 Length", html_2xx!("u.h1_2_len"), ColumnKind::Int, true),
        col("H2-2", html_2xx!("u.h2_2"), ColumnKind::Text, true),
        col("H2-2 Length", html_2xx!("u.h2_2_len"), ColumnKind::Int, true),
    ]);

    let mut filters = vec![class_filter("All", "1")];
    filters.extend(class_filters());
    filters.push(unknown_filter());

    TabDef {
        id: "Internal",
        population: POPULATION,
        columns,
        filters,
        default_sort: Vec::new(),
        params: |_| Vec::new(),
    }
}
