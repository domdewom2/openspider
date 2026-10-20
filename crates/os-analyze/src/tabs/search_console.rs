//! Tab „Search Console“: Search-Analytics- und URL-Inspection-Spalten mit elf Filtern (INTEG-115, 116, 123, 125).
//!
//! Die Werte schreibt `os-integrations` (`gsc`) in `api_values` mit Quelle `gsc` und Ebene `url`: Zahlen in `v_real`
//! (`clicks`, `impressions`, `ctr` als Anteil 0 – 1, `position` ungerundet, `canonical_not_selected` 0/1), Texte in
//! `v_text` (`summary`, `coverage`, …, `inspection_property`). Jede Spalte ist eine Suche über den Primärschlüssel von
//! `api_values` und damit auch live während des Crawls exakt.
//!
//! Darstellung: `CTR` hat vier Nachkommastellen als Anteil (= zwei Nachkommastellen in Prozent, die Prozentanzeige ist
//! Sache der Oberfläche), `Position` sechs, damit der Export praktisch ungerundet bleibt; die Oberfläche zeigt eine
//! Stelle (INTEG-115).
//!
//! Grundmenge: interne URLs der Tabellen (wie „Internal“). „No Search Analytics Data“ betrachtet nur gecrawlte HTML-Seiten.
//! „Orphan URLs“: URLs mit Search-Analytics-Daten ohne internen Verweis (Hyperlink oder Ressourcenlink) von einer
//! anderen Seite, die weder Start-URL noch Listeneintrag noch Weiterleitungsziel sind; sie entstehen, wenn API-URLs gecrawlt werden (INTEG-015). Der Filter beachtet
//! `issues.ignore_non_indexable` (CRAWL-089).

use crate::defs::{ColumnDef, ColumnKind, FilterDef, FilterKind, Needs, Requirement, TabDef};

/// Zahlenwert einer Metrik der Quelle `gsc`.
macro_rules! real {
    ($metric:literal) => {
        concat!(
            "(SELECT a.v_real FROM api_values a WHERE a.url_id = u.id AND a.source = 'gsc' AND a.metric = '",
            $metric,
            "' AND a.level = 'url')"
        )
    };
}

/// Textwert einer Metrik der Quelle `gsc`.
macro_rules! text {
    ($metric:literal) => {
        concat!(
            "(SELECT a.v_text FROM api_values a WHERE a.url_id = u.id AND a.source = 'gsc' AND a.metric = '",
            $metric,
            "' AND a.level = 'url')"
        )
    };
}

/// Grundmenge: interne URLs der Tabellen.
const POPULATION: &str = "u.is_internal = 1 AND u.in_tables = 1";

/// Search-Analytics-Spalten und ihre Metriken.
const ANALYTICS: [(&str, &str, ColumnKind); 4] = [
    ("Clicks", real!("clicks"), ColumnKind::Int),
    ("Impressions", real!("impressions"), ColumnKind::Int),
    ("CTR", real!("ctr"), ColumnKind::Float { decimals: 4 }),
    (
        "Position",
        real!("position"),
        ColumnKind::Float { decimals: 6 },
    ),
];

/// URL-Inspection-Spalten (INTEG-123) und `Inspection Property` (INTEG-122).
const INSPECTION: [(&str, &str, ColumnKind); 18] = [
    ("Summary", text!("summary"), ColumnKind::Text),
    ("Coverage", text!("coverage"), ColumnKind::Text),
    ("Last Crawl", text!("last_crawl"), ColumnKind::Timestamp),
    ("Crawled As", text!("crawled_as"), ColumnKind::Text),
    ("Crawl Allowed", text!("crawl_allowed"), ColumnKind::Text),
    ("Page Fetch", text!("page_fetch"), ColumnKind::Text),
    (
        "Indexing Allowed",
        text!("indexing_allowed"),
        ColumnKind::Text,
    ),
    (
        "User-Declared Canonical",
        text!("user_canonical"),
        ColumnKind::Url,
    ),
    (
        "Google-Selected Canonical",
        text!("google_canonical"),
        ColumnKind::Url,
    ),
    (
        "Mobile Usability",
        text!("mobile_usability"),
        ColumnKind::Text,
    ),
    (
        "Mobile Usability Issues",
        text!("mobile_usability_issues"),
        ColumnKind::Text,
    ),
    ("AMP Results", text!("amp_results"), ColumnKind::Text),
    ("AMP Issues", text!("amp_issues"), ColumnKind::Text),
    ("Rich Results", text!("rich_results"), ColumnKind::Text),
    (
        "Rich Results Types",
        text!("rich_results_types"),
        ColumnKind::Text,
    ),
    (
        "Rich Results Types Errors",
        text!("rich_results_types_errors"),
        ColumnKind::Text,
    ),
    (
        "Rich Results Warnings",
        text!("rich_results_warnings"),
        ColumnKind::Text,
    ),
    (
        "Inspection Property",
        text!("inspection_property"),
        ColumnKind::Text,
    ),
];

const CLICKS_ABOVE_0: &str = concat!(real!("clicks"), " >= 1");

const NO_DATA: &str = concat!(
    "u.content_class = 'HTML' AND COALESCE(u.status_code, 0) > 0 AND NOT EXISTS (SELECT 1 FROM api_values a ",
    "WHERE a.url_id = u.id AND a.source = 'gsc' AND a.metric = 'impressions' AND a.level = 'url')"
);

const NON_INDEXABLE_WITH_DATA: &str = concat!(
    "u.indexability = 'Non-Indexable' AND ",
    real!("impressions"),
    " >= 1"
);

const ORPHAN: &str = concat!(
    real!("impressions"),
    " IS NOT NULL AND u.discovered_via NOT IN ('start', 'list_input', 'redirect') ",
    "AND COALESCE(c.inlinks_all, 0) - COALESCE(c.self_inlinks, 0) <= 0"
);

const NOT_ON_GOOGLE: &str = concat!(text!("summary"), " = 'URL is not on Google'");

const INDEXABLE_NOT_INDEXED: &str = concat!(
    "u.indexability = 'Indexable' AND ",
    text!("summary"),
    " = 'URL is not on Google'"
);

const ON_GOOGLE_WITH_ISSUES: &str =
    concat!(text!("summary"), " = 'URL is on Google, but has issues'");

const CANONICAL_NOT_SELECTED: &str = concat!(real!("canonical_not_selected"), " = 1");

const NOT_MOBILE_FRIENDLY: &str = concat!(text!("mobile_usability"), " = 'Fail'");

const AMP_INVALID: &str = concat!(text!("amp_results"), " = 'Fail'");

const RICH_RESULT_INVALID: &str = concat!(
    "(",
    text!("rich_results"),
    " = 'Fail' OR COALESCE(",
    text!("rich_results_types_errors"),
    ", '') <> '')"
);

fn filter(id: &'static str, predicate: &'static str, config: &'static str) -> FilterDef {
    FilterDef {
        id,
        predicate,
        kind: FilterKind::Live,
        requires: vec![Requirement::Config(config)],
        ignore_non_indexable: false,
        default_columns: &[],
    }
}

/// Definition des Tabs „Search Console“.
pub fn tab() -> TabDef {
    let mut columns = vec![
        ColumnDef::new("Address", "u.address", ColumnKind::Url, true),
        ColumnDef::new("Status Code", "u.status_code", ColumnKind::Int, true),
        ColumnDef::new("Indexability", "u.indexability", ColumnKind::Text, true),
        ColumnDef::new(
            "Indexability Status",
            "u.indexability_status",
            ColumnKind::Text,
            true,
        ),
    ];
    columns.extend(ANALYTICS.iter().map(|(key, sql, kind)| {
        ColumnDef::new(key, sql, *kind, true).with_needs(Needs::Api("gsc"))
    }));
    columns.extend(INSPECTION.iter().map(|(key, sql, kind)| {
        ColumnDef::new(key, sql, *kind, true).with_needs(Needs::Config("api.gsc.url_inspection"))
    }));

    let mut orphan = filter("Orphan URLs", ORPHAN, "api.gsc.enabled");
    orphan.ignore_non_indexable = true;
    let filters = vec![
        FilterDef {
            id: "All",
            predicate: "1",
            kind: FilterKind::Live,
            requires: Vec::new(),
            ignore_non_indexable: false,
            default_columns: &[],
        },
        filter("Clicks Above 0", CLICKS_ABOVE_0, "api.gsc.enabled"),
        filter("No Search Analytics Data", NO_DATA, "api.gsc.enabled"),
        filter(
            "Non-Indexable with Search Analytics Data",
            NON_INDEXABLE_WITH_DATA,
            "api.gsc.enabled",
        ),
        orphan,
        filter(
            "URL Is Not on Google",
            NOT_ON_GOOGLE,
            "api.gsc.url_inspection",
        ),
        filter(
            "Indexable URL Not Indexed",
            INDEXABLE_NOT_INDEXED,
            "api.gsc.url_inspection",
        ),
        filter(
            "URL is on Google, But Has Issues",
            ON_GOOGLE_WITH_ISSUES,
            "api.gsc.url_inspection",
        ),
        filter(
            "User-Declared Canonical Not Selected",
            CANONICAL_NOT_SELECTED,
            "api.gsc.url_inspection",
        ),
        filter(
            "Page Is Not Mobile Friendly",
            NOT_MOBILE_FRIENDLY,
            "api.gsc.url_inspection",
        ),
        filter("AMP URL Is Invalid", AMP_INVALID, "api.gsc.url_inspection"),
        filter(
            "Rich Result Invalid",
            RICH_RESULT_INVALID,
            "api.gsc.url_inspection",
        ),
    ];
    TabDef {
        id: "Search Console",
        population: POPULATION,
        columns,
        filters,
        default_sort: Vec::new(),
        params: |_| Vec::new(),
        hidden_population: None,
    }
}
