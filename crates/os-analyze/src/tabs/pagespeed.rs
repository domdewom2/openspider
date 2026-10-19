//! Tab „PageSpeed“: PageSpeed Insights und Lighthouse (INTEG-130, INTEG-133 … INTEG-139, INTEG-143; M2-I04).
//!
//! Die Werte stehen in `api_values` (Quelle `psi`, Metrik = Spaltenname); `os-integrations` (`psi::lighthouse`)
//! schreibt sie. Filterergebnisse stehen als Metrik `fail:<Filter>` (1 = nicht bestanden). Fehler einer URL kommen
//! entweder als Wert (`PSI Status = Error`, `PSI Error`) oder – bei Abruffehlern der Warteschlange – aus `api_errors`;
//! ein Eintrag in `api_errors` ist jünger als die gespeicherten Werte (ein Erfolg löscht ihn) und hat Vorrang: dann
//! sind alle Spalten leer und die URL steht in keinem Filter.
//!
//! Spalten der Metrikgruppen liefert [`column_source`] je nach Auswahl in `api.psi.metric_groups` (INTEG-133); der Tab
//! selbst trägt nur `Address`, `PSI Status` und `PSI Error`. Filter einer abgewählten Gruppe entfernt die Quelle aus
//! dem konfigurierten Tab ([`ColumnSource::keeps_filter`]).

use std::collections::BTreeSet;

use os_model::CrawlConfig;

use crate::defs::{
    intern, ColumnDef, ColumnKind, ColumnSource, FilterDef, FilterKind, Needs, Requirement, TabDef,
};

/// Grundmenge (INTEG-130): interne HTML-URLs mit Status 200 – genau die URLs, die abgefragt werden.
pub const POPULATION: &str =
    "u.is_internal = 1 AND u.in_tables = 1 AND u.content_class = 'HTML' AND u.status_code = 200";

/// Die URL hat einen jüngeren Abruffehler in `api_errors`.
const HAS_ERROR: &str =
    "EXISTS (SELECT 1 FROM api_errors e WHERE e.source = 'psi' AND e.url = u.url)";

/// Konfigurationsschlüssel der Anbindung.
const ENABLED: &str = "api.psi.enabled";

/// Spalten der Metrikgruppen in Tab-Reihenfolge: `(Gruppe, Spalte, Darstellung)` (INTEG-134 … INTEG-138).
pub const COLUMNS: &[(&str, &str, ColumnKind)] = &[
    ("overview", "Total Size Savings (Bytes)", ColumnKind::Int),
    ("overview", "Total Time Savings (ms)", ColumnKind::Int),
    ("overview", "Total Requests", ColumnKind::Int),
    ("overview", "Total Page Size (Bytes)", ColumnKind::Int),
    ("overview", "HTML Size", ColumnKind::Int),
    ("overview", "HTML Count", ColumnKind::Int),
    ("overview", "Image Size", ColumnKind::Int),
    ("overview", "Image Count", ColumnKind::Int),
    ("overview", "CSS Size", ColumnKind::Int),
    ("overview", "CSS Count", ColumnKind::Int),
    ("overview", "JavaScript Size", ColumnKind::Int),
    ("overview", "JavaScript Count", ColumnKind::Int),
    ("overview", "Font Size", ColumnKind::Int),
    ("overview", "Font Count", ColumnKind::Int),
    ("overview", "Media Size", ColumnKind::Int),
    ("overview", "Media Count", ColumnKind::Int),
    ("overview", "Other Size", ColumnKind::Int),
    ("overview", "Other Count", ColumnKind::Int),
    ("overview", "Third Party Size", ColumnKind::Int),
    ("overview", "Third Party Count", ColumnKind::Int),
    ("crux", "Core Web Vitals Assessment", ColumnKind::Text),
    (
        "crux",
        "CrUX First Contentful Paint (sec)",
        ColumnKind::Float { decimals: 2 },
    ),
    (
        "crux",
        "CrUX First Contentful Paint Category",
        ColumnKind::Text,
    ),
    (
        "crux",
        "CrUX Largest Contentful Paint (sec)",
        ColumnKind::Float { decimals: 2 },
    ),
    (
        "crux",
        "CrUX Largest Contentful Paint Category",
        ColumnKind::Text,
    ),
    (
        "crux",
        "CrUX Cumulative Layout Shift",
        ColumnKind::Float { decimals: 2 },
    ),
    (
        "crux",
        "CrUX Cumulative Layout Shift Category",
        ColumnKind::Text,
    ),
    (
        "crux",
        "CrUX Interaction to Next Paint (ms)",
        ColumnKind::Int,
    ),
    (
        "crux",
        "CrUX Interaction to Next Paint Category",
        ColumnKind::Text,
    ),
    ("crux", "CrUX Time to First Byte (ms)", ColumnKind::Int),
    ("crux", "CrUX Time to First Byte Category", ColumnKind::Text),
    (
        "crux",
        "CrUX Origin Core Web Vitals Assessment",
        ColumnKind::Text,
    ),
    (
        "crux",
        "CrUX Origin First Contentful Paint (sec)",
        ColumnKind::Float { decimals: 2 },
    ),
    (
        "crux",
        "CrUX Origin First Contentful Paint Category",
        ColumnKind::Text,
    ),
    (
        "crux",
        "CrUX Origin Largest Contentful Paint (sec)",
        ColumnKind::Float { decimals: 2 },
    ),
    (
        "crux",
        "CrUX Origin Largest Contentful Paint Category",
        ColumnKind::Text,
    ),
    (
        "crux",
        "CrUX Origin Cumulative Layout Shift",
        ColumnKind::Float { decimals: 2 },
    ),
    (
        "crux",
        "CrUX Origin Cumulative Layout Shift Category",
        ColumnKind::Text,
    ),
    (
        "crux",
        "CrUX Origin Interaction to Next Paint (ms)",
        ColumnKind::Int,
    ),
    (
        "crux",
        "CrUX Origin Interaction to Next Paint Category",
        ColumnKind::Text,
    ),
    (
        "crux",
        "CrUX Origin Time to First Byte (ms)",
        ColumnKind::Int,
    ),
    (
        "crux",
        "CrUX Origin Time to First Byte Category",
        ColumnKind::Text,
    ),
    ("lighthouse", "Performance Score", ColumnKind::Int),
    (
        "lighthouse",
        "First Contentful Paint Time (sec)",
        ColumnKind::Float { decimals: 2 },
    ),
    (
        "lighthouse",
        "First Contentful Paint Score",
        ColumnKind::Int,
    ),
    (
        "lighthouse",
        "Speed Index Time (sec)",
        ColumnKind::Float { decimals: 2 },
    ),
    ("lighthouse", "Speed Index Score", ColumnKind::Int),
    (
        "lighthouse",
        "Largest Contentful Paint Time (sec)",
        ColumnKind::Float { decimals: 2 },
    ),
    (
        "lighthouse",
        "Largest Contentful Paint Score",
        ColumnKind::Int,
    ),
    (
        "lighthouse",
        "Time to Interactive (sec)",
        ColumnKind::Float { decimals: 2 },
    ),
    ("lighthouse", "Time to Interactive Score", ColumnKind::Int),
    (
        "lighthouse",
        "Max Potential First Input Delay (ms)",
        ColumnKind::Int,
    ),
    (
        "lighthouse",
        "Max Potential First Input Delay Score",
        ColumnKind::Int,
    ),
    ("lighthouse", "Total Blocking Time (ms)", ColumnKind::Int),
    ("lighthouse", "Total Blocking Time Score", ColumnKind::Int),
    (
        "lighthouse",
        "Cumulative Layout Shift",
        ColumnKind::Float { decimals: 3 },
    ),
    (
        "lighthouse",
        "Cumulative Layout Shift Score",
        ColumnKind::Int,
    ),
    ("lighthouse", "Lighthouse Version", ColumnKind::Text),
    ("insights", "Document Request Latency", ColumnKind::Text),
    ("insights", "Number of Redirects", ColumnKind::Int),
    ("insights", "Server Responds Quickly", ColumnKind::Text),
    ("insights", "Applies Text Compression", ColumnKind::Text),
    ("insights", "LCP Request Discovery", ColumnKind::Text),
    ("insights", "LCP Breakdown TTFB (ms)", ColumnKind::Int),
    ("insights", "LCP Breakdown Load Delay (ms)", ColumnKind::Int),
    (
        "insights",
        "LCP Breakdown Load Duration (ms)",
        ColumnKind::Int,
    ),
    (
        "insights",
        "LCP Breakdown Render Delay (ms)",
        ColumnKind::Int,
    ),
    (
        "insights",
        "Render Blocking Requests Savings (ms)",
        ColumnKind::Int,
    ),
    (
        "insights",
        "Preconnect Candidates Savings (ms)",
        ColumnKind::Int,
    ),
    (
        "insights",
        "Maximum Critical Path Latency (ms)",
        ColumnKind::Int,
    ),
    (
        "insights",
        "Use Efficient Cache Lifetimes Savings (Bytes)",
        ColumnKind::Int,
    ),
    ("insights", "Layout Shift Culprits", ColumnKind::Int),
    (
        "insights",
        "Improve Image Delivery Savings (Bytes)",
        ColumnKind::Int,
    ),
    ("insights", "Forced Reflow Savings (ms)", ColumnKind::Int),
    (
        "insights",
        "Legacy JavaScript Savings (Bytes)",
        ColumnKind::Int,
    ),
    (
        "insights",
        "Duplicated JavaScript Savings (Bytes)",
        ColumnKind::Int,
    ),
    ("insights", "3rd Parties", ColumnKind::Int),
    ("insights", "Network Payload Size (Bytes)", ColumnKind::Int),
    ("diagnostics", "Minify CSS Savings (ms)", ColumnKind::Int),
    ("diagnostics", "Minify CSS Savings (Bytes)", ColumnKind::Int),
    (
        "diagnostics",
        "Minify JavaScript Savings (ms)",
        ColumnKind::Int,
    ),
    (
        "diagnostics",
        "Minify JavaScript Savings (Bytes)",
        ColumnKind::Int,
    ),
    (
        "diagnostics",
        "Reduce Unused CSS Savings (ms)",
        ColumnKind::Int,
    ),
    (
        "diagnostics",
        "Reduce Unused CSS Savings (Bytes)",
        ColumnKind::Int,
    ),
    (
        "diagnostics",
        "Reduce Unused JavaScript Savings (ms)",
        ColumnKind::Int,
    ),
    (
        "diagnostics",
        "Reduce Unused JavaScript Savings (Bytes)",
        ColumnKind::Int,
    ),
    (
        "diagnostics",
        "JavaScript Execution Time (sec)",
        ColumnKind::Float { decimals: 2 },
    ),
    (
        "diagnostics",
        "JavaScript Execution Time Category",
        ColumnKind::Text,
    ),
    (
        "diagnostics",
        "Efficient Cache Policy Savings (Bytes)",
        ColumnKind::Int,
    ),
    (
        "diagnostics",
        "Minimize Main-Thread Work (sec)",
        ColumnKind::Float { decimals: 2 },
    ),
    (
        "diagnostics",
        "Minimize Main-Thread Work Category",
        ColumnKind::Text,
    ),
    ("diagnostics", "DOM Size", ColumnKind::Int),
    ("diagnostics", "Font Display Savings (ms)", ColumnKind::Int),
    (
        "diagnostics",
        "User Timing Marks and Measures",
        ColumnKind::Int,
    ),
];

/// Die 19 Filter (INTEG-139) in Spec-Reihenfolge mit ihrer Gruppe (INTEG-133).
pub const FILTERS: [(&str, &str); 19] = [
    ("Document Request Latency", "insights"),
    ("LCP Request Discovery", "insights"),
    ("Render Blocking Requests", "insights"),
    ("Network Dependency Tree", "insights"),
    ("Use Efficient Cache Lifetimes", "insights"),
    ("Layout Shift Culprits", "insights"),
    ("Improve Image Delivery", "insights"),
    ("Forced Reflow", "insights"),
    ("Legacy JavaScript", "insights"),
    ("Duplicated JavaScript", "insights"),
    ("Avoid Enormous Network Payloads", "insights"),
    ("Minify CSS", "diagnostics"),
    ("Minify JavaScript", "diagnostics"),
    ("Reduce Unused CSS", "diagnostics"),
    ("Reduce Unused JavaScript", "diagnostics"),
    ("Reduce JavaScript Execution Time", "diagnostics"),
    ("Minimize Main-Thread Work", "diagnostics"),
    ("Optimize DOM Size", "diagnostics"),
    ("Font Display", "diagnostics"),
];

/// Gruppenkennungen (INTEG-133), `mobile` gehört dem Mobile-Tab (M2-D23).
pub const GROUPS: [&str; 6] = [
    "overview",
    "crux",
    "lighthouse",
    "insights",
    "diagnostics",
    "mobile",
];

fn quoted(s: &str) -> String {
    s.replace('\'', "''")
}

/// SQL eines gespeicherten Werts (`v_real` bzw. `v_text`) der Quelle `psi`.
fn stored(metric: &str, text: bool) -> String {
    format!(
        "(SELECT a.{} FROM api_values a WHERE a.url_id = u.id AND a.source = 'psi' AND a.metric = '{}' AND a.level = 'url')",
        if text { "v_text" } else { "v_real" },
        quoted(metric)
    )
}

/// SQL einer Gruppenspalte: leer bei einem jüngeren Abruffehler.
fn value_sql(metric: &str, text: bool) -> String {
    format!(
        "CASE WHEN {HAS_ERROR} THEN NULL ELSE {} END",
        stored(metric, text)
    )
}

/// `PSI Status` (genau `Success` | `Error`, leer vor der Abfrage; TECH-230, INTEG-143).
fn status_sql() -> String {
    format!(
        "CASE WHEN {HAS_ERROR} THEN 'Error' ELSE {} END",
        stored("PSI Status", true)
    )
}

/// `PSI Error`: Code und Meldung; ein erschöpftes Kontingent aus der Warteschlange erhält den Präfix `Quota exceeded:`.
fn error_sql() -> String {
    format!(
        "CASE WHEN {HAS_ERROR} THEN (SELECT CASE WHEN e.message LIKE 'Quota exceeded%' AND e.message NOT LIKE 'Quota exceeded:%' \
         THEN 'Quota exceeded: Tageskontingent erschöpft' ELSE TRIM(COALESCE(e.code, '') || ' ' || e.message) END \
         FROM api_errors e WHERE e.source = 'psi' AND e.url = u.url ORDER BY e.id DESC LIMIT 1) ELSE {} END",
        stored("PSI Error", true)
    )
}

/// Prädikat eines der 19 Filter (INTEG-139): Ergebnis `fail:<Filter>` = 1, ohne jüngeren Fehler.
pub fn filter_predicate(filter: &str) -> &'static str {
    intern(&format!(
        "NOT {HAS_ERROR} AND EXISTS (SELECT 1 FROM api_values a WHERE a.url_id = u.id AND a.source = 'psi' \
         AND a.metric = 'fail:{}' AND a.level = 'url' AND a.v_real = 1)",
        quoted(filter)
    ))
}

/// Prädikat „PSI Errors“ (INTEG-143): genau die URLs mit `PSI Status = Error`, einschließlich erschöpftem Kontingent.
pub fn psi_errors_predicate() -> &'static str {
    intern(&format!(
        "({HAS_ERROR} OR {} = 'Error')",
        stored("PSI Status", true)
    ))
}

/// Filter „PSI Errors“ (INTEG-143). Nicht registriert: die Kennung fehlt in `docs/plan/M2-filters.toml` (Regel 10);
/// die Registrierung ist eine offene Frage an den Planer (Bericht M2-I04).
pub fn psi_errors_filter() -> FilterDef {
    FilterDef {
        id: "PSI Errors",
        predicate: psi_errors_predicate(),
        kind: FilterKind::Live,
        requires: vec![Requirement::Config(ENABLED)],
        ignore_non_indexable: false,
        default_columns: &[],
    }
}

/// Gewählte Gruppen und Einzelspalten (INTEG-133). Leere Auswahl = alle Gruppen (OS-Festlegung); bei Quelle Lokal
/// nie CrUX (INTEG-130).
pub fn selection(cfg: &CrawlConfig) -> (BTreeSet<&'static str>, BTreeSet<String>) {
    let psi = &cfg.api.psi;
    let mut groups = BTreeSet::new();
    let mut columns = BTreeSet::new();
    for entry in &psi.metric_groups {
        match GROUPS.iter().find(|g| g.eq_ignore_ascii_case(entry.trim())) {
            Some(g) => {
                groups.insert(*g);
            }
            None => {
                columns.insert(entry.trim().to_string());
            }
        }
    }
    if psi.metric_groups.is_empty() {
        groups.extend(GROUPS);
    }
    if psi.local {
        groups.remove("crux");
        columns.retain(|c| group_of(c) != Some("crux"));
    }
    (groups, columns)
}

fn group_of(column: &str) -> Option<&'static str> {
    COLUMNS
        .iter()
        .find(|(_, name, _)| *name == column)
        .map(|(g, _, _)| *g)
}

/// Erscheint die Spalte bei dieser Konfiguration?
pub fn column_visible(cfg: &CrawlConfig, column: &str) -> bool {
    let (groups, columns) = selection(cfg);
    match group_of(column) {
        Some(g) => groups.contains(g) || columns.contains(column),
        None => true,
    }
}

/// Erscheint der Filter bei dieser Konfiguration? Filter gehören zu ihrer Gruppe (INTEG-133).
pub fn filter_visible(cfg: &CrawlConfig, filter: &str) -> bool {
    let (groups, _) = selection(cfg);
    FILTERS
        .iter()
        .find(|(f, _)| *f == filter)
        .is_none_or(|(_, g)| groups.contains(g))
}

/// Definition des Tabs „PageSpeed“: Status- und Fehlerspalte, 19 Filter in der Reihenfolge des Filterregisters.
pub fn tab() -> TabDef {
    let mut filters = vec![FilterDef {
        id: "All",
        predicate: "1",
        kind: FilterKind::Live,
        requires: Vec::new(),
        ignore_non_indexable: false,
        default_columns: &[],
    }];
    let mut ids: Vec<&'static str> = FILTERS.iter().map(|(f, _)| *f).collect();
    // Das Register führt die Filter alphabetisch (docs/plan/M2-filters.toml); die Spec-Reihenfolge steht in FILTERS.
    ids.sort_unstable();
    for id in ids {
        filters.push(FilterDef {
            id,
            predicate: filter_predicate(id),
            kind: FilterKind::Live,
            requires: vec![Requirement::Config(ENABLED)],
            ignore_non_indexable: false,
            default_columns: &[],
        });
    }
    TabDef {
        id: "PageSpeed",
        population: POPULATION,
        columns: vec![
            super::duplicates::address_column(),
            ColumnDef::dynamic("PSI Status", &status_sql(), ColumnKind::Text, true)
                .with_needs(Needs::Api("psi")),
            ColumnDef::dynamic("PSI Error", &error_sql(), ColumnKind::Text, true)
                .with_needs(Needs::Api("psi")),
        ],
        filters,
        default_sort: Vec::new(),
        params: |_| Vec::new(),
        hidden_population: None,
    }
}

/// Spalten der gewählten Metrikgruppen; entfernt Filter abgewählter Gruppen aus dem konfigurierten Tab.
struct PageSpeedColumns;

impl ColumnSource for PageSpeedColumns {
    fn columns(&self, cfg: &CrawlConfig) -> Vec<ColumnDef> {
        COLUMNS
            .iter()
            .filter(|(_, name, _)| column_visible(cfg, name))
            .map(|(_, name, kind)| {
                ColumnDef::dynamic(
                    name,
                    &value_sql(name, *kind == ColumnKind::Text),
                    *kind,
                    true,
                )
                .with_needs(Needs::Api("psi"))
            })
            .collect()
    }

    fn keeps_filter(&self, cfg: &CrawlConfig, filter: &str) -> bool {
        filter_visible(cfg, filter)
    }
}

/// Quelle der Gruppenspalten des Tabs „PageSpeed“ (INTEG-133).
pub fn column_source() -> Option<Box<dyn ColumnSource>> {
    Some(Box::new(PageSpeedColumns))
}
