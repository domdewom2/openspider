//! Mobile (TECH-230–237): Lighthouse-Scores und unabhängige HTML-Alternativen.
use crate::defs::{
    intern, ColumnDef, ColumnKind, ColumnSource, FilterDef, FilterKind, Requirement, TabDef,
};
use os_model::{CrawlConfig, Value};

const ELIGIBLE: &str = super::pagespeed::POPULATION;
const HAS_ERROR: &str = "EXISTS (SELECT 1 FROM api_errors e WHERE e.source='psi' AND e.url=u.url)";
const ALTERNATE: &str = "(SELECT v_text FROM url_ext WHERE url_id=u.id AND key='mobile.alternate')";

/// Zugehörige Score-Spalte je kanonischem Filter (TECH-232–236).
pub const FILTERS: [(&str, &str); 5] = [
    ("Viewport Not Set", "Viewport"),
    ("Target Size", "Tap Targets"),
    ("Content Not Sized Correctly", "Content Width"),
    ("Illegible Font Size", "Font Display"),
    ("Unsupported Plugins", "Plugins"),
];
/// Hinweise werden als Voraussetzung statt einer irreführenden Trefferzahl transportiert.
pub const NOT_CONFIGURED: &str = "Mobile-Prüfung nicht konfiguriert";
pub const LOCAL_ONLY: &str = "nur mit lokaler Quelle";
pub const MISSING_AUDIT: &str = "Audit von der Quelle nicht geliefert";

fn stored(name: &str, text: bool) -> String {
    format!("(SELECT a.{} FROM api_values a WHERE a.url_id=u.id AND a.source='psi' AND a.metric='{}' AND a.level='url')",
        if text { "v_text" } else { "v_real" }, name.replace('\'', "''"))
}
fn status_sql() -> String {
    format!(
        "CASE WHEN {HAS_ERROR} THEN 'Error' ELSE {} END",
        stored("PSI Status", true)
    )
}
fn error_sql() -> String {
    format!("CASE WHEN {HAS_ERROR} THEN (SELECT TRIM(COALESCE(e.code,'') || ' ' || e.message) FROM api_errors e WHERE e.source='psi' AND e.url=u.url ORDER BY e.id DESC LIMIT 1) ELSE {} END", stored("PSI Error", true))
}
fn score_sql(column: &str) -> String {
    let value = stored(column, false);
    let value = if column == "Plugins" {
        format!("CASE WHEN COALESCE({},0)=1 THEN {value} ELSE COALESCE({value}, (SELECT v_int FROM url_ext WHERE url_id=u.id AND key='mobile.plugins')) END", stored("mobile.plugins_audit", false))
    } else {
        value
    };
    format!("CASE WHEN ({})='Success' THEN {value} END", status_sql())
}
fn checked_sql(column: &str) -> String {
    format!(
        "CASE WHEN :mobile_enabled AND {} THEN {} END",
        if column == "Content Width" {
            ":mobile_local"
        } else {
            "1"
        },
        score_sql(column)
    )
}
fn filter(id: &'static str, predicate: &'static str) -> FilterDef {
    FilterDef {
        id,
        predicate,
        kind: FilterKind::Live,
        requires: Vec::new(),
        ignore_non_indexable: false,
        default_columns: &[],
    }
}
fn score_filter(id: &'static str, column: &str) -> FilterDef {
    filter(id, intern(&format!("({}) < 100", checked_sql(column))))
}
fn params(cfg: &CrawlConfig) -> Vec<(&'static str, Value)> {
    vec![
        (":mobile_enabled", Value::Bool(selected(cfg))),
        (":mobile_local", Value::Bool(cfg.api.psi.local)),
        (
            ":mobile_store",
            Value::Bool(cfg.spider.mobile_alternate.store),
        ),
    ]
}
fn selected(cfg: &CrawlConfig) -> bool {
    cfg.api.psi.enabled && super::pagespeed::selection(cfg).0.contains("mobile")
}

/// Definition einschließlich der neun Standardspalten (TECH-231).
pub fn tab() -> TabDef {
    let mut columns = vec![
        super::duplicates::address_column(),
        ColumnDef::dynamic("PSI Status", &status_sql(), ColumnKind::Text, true),
        ColumnDef::dynamic("PSI Error", &error_sql(), ColumnKind::Text, true),
    ];
    for column in [
        "Viewport",
        "Tap Targets",
        "Content Width",
        "Font Display",
        "Plugins",
    ] {
        columns.push(ColumnDef::dynamic(
            column,
            &checked_sql(column),
            ColumnKind::Int,
            true,
        ));
    }
    columns.push(ColumnDef::dynamic(
        "Mobile Alternate Link",
        &format!("CASE WHEN :mobile_store THEN {ALTERNATE} END"),
        ColumnKind::Url,
        true,
    ));
    let mut filters = vec![filter("All", "1")];
    filters.extend(FILTERS.iter().map(|(id, col)| score_filter(id, col)));
    filters.push(filter(
        "Mobile Alternate Link",
        intern(&format!(":mobile_store AND {ALTERNATE} IS NOT NULL")),
    ));
    TabDef {
        id: "Mobile",
        population: intern(&format!(
            "{ELIGIBLE} AND (({}) IS NOT NULL OR (:mobile_store AND {ALTERNATE} IS NOT NULL))",
            status_sql()
        )),
        columns,
        filters,
        default_sort: Vec::new(),
        params,
        hidden_population: None,
    }
}

struct MobileColumns;
impl ColumnSource for MobileColumns {
    fn columns(&self, _: &CrawlConfig) -> Vec<ColumnDef> {
        Vec::new()
    }
    fn filters(&self, cfg: &CrawlConfig) -> Vec<FilterDef> {
        FILTERS
            .iter()
            .map(|(id, col)| {
                let mut def = score_filter(id, col);
                let hint = if !selected(cfg) {
                    Some(NOT_CONFIGURED)
                } else if *col == "Content Width" && !cfg.api.psi.local {
                    Some(LOCAL_ONLY)
                } else {
                    None
                };
                if let Some(hint) = hint {
                    def.requires.push(Requirement::Config(hint));
                }
                def
            })
            .collect()
    }
    fn file_filters(&self, cfg: &CrawlConfig, file: &os_store::CrawlFile) -> Vec<FilterDef> {
        if !selected(cfg) {
            return Vec::new();
        }
        // Drive the checks from api_values_source, never scan the crawl's entire URL population for missing audits.
        let success_sql = format!("SELECT EXISTS(SELECT 1 FROM api_values a JOIN urls u ON u.id=a.url_id WHERE a.source='psi' AND a.metric='PSI Status' AND a.level='url' AND a.v_text='Success' AND {ELIGIBLE} AND NOT {HAS_ERROR})");
        if file
            .conn()
            .query_row(&success_sql, [], |r| r.get::<_, bool>(0))
            != Ok(true)
        {
            return Vec::new();
        }
        let mut overrides = Vec::new();
        for mut def in self.filters(cfg) {
            if !def.requires.is_empty() {
                continue;
            }
            let column = FILTERS
                .iter()
                .find(|(id, _)| *id == def.id)
                .expect("score filter")
                .1;
            // The plugin fallback is relevant only for URLs with a successful PSI response.
            let metric = if column == "Plugins" {
                "PSI Status"
            } else {
                column
            };
            let sql = format!(
                "SELECT EXISTS(SELECT 1 FROM api_values a JOIN urls u ON u.id=a.url_id \
                 WHERE a.source='psi' AND a.metric='{metric}' AND a.level='url' AND {ELIGIBLE} \
                 AND ({}) IS NOT NULL)",
                score_sql(column)
            );
            let Ok(delivered) = file.conn().query_row(&sql, [], |r| r.get::<_, bool>(0)) else {
                continue;
            };
            if !delivered {
                def.requires.push(Requirement::Config(MISSING_AUDIT));
                overrides.push(def);
            }
        }
        overrides
    }
}
/// Konfigurations- und dateiabhängige Voraussetzungen ohne Entfernen der Filter.
pub fn column_source() -> Option<Box<dyn ColumnSource>> {
    Some(Box::new(MobileColumns))
}

/// Zusatzinformationen für Fachansichten/Tooltips, ohne zusätzliche Exportspalten.
pub fn alternate_count(
    file: &os_store::CrawlFile,
    url: os_model::UrlId,
) -> Result<u64, crate::ViewError> {
    Ok(file.conn().query_row("SELECT COALESCE((SELECT v_int FROM url_ext WHERE url_id=?1 AND key='mobile.alternate_count'),0)", [url.0], |r| r.get(0))?)
}
/// Die kompatible Spaltenbezeichnung erklärt die Schriftgrößenprüfung.
pub fn column_tooltip(column: &str) -> Option<&'static str> {
    (column == "Font Display").then_some("Lesbare Schriftgrößen")
}
