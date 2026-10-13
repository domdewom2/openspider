//! Tab „Hreflang“ (TECH-071, TECH-073, TECH-074, TECH-076, TECH-080, TECH-085).
//!
//! # Datengrundlage
//!
//! Die Tabelle `hreflang` hält alle Annotationen einer URL aus drei Quellen: `html`, `http` und `sitemap`
//! (`sitemap_id` verweist auf die Sitemap-Datei; die URL der Zeile ist die `<loc>`-URL des `<url>`-Blocks).
//! Die **Annotationsmenge** einer Seite vereinigt alle drei Quellen (Begriffe vor TECH-070).
//!
//! # Grundmenge (TECH-071)
//!
//! Interne HTML-200-Seiten, interne URLs mit HTTP- oder Sitemap-hreflang und gecrawlte interne
//! hreflang-Ziele. Ist „hreflang speichern“ aus (`spider.hreflang.store`), ist der Tab leer (OS-Verhalten:
//! „Store an, Crawl aus“ befüllt ihn bereits; „Store aus“ lässt ihn und alle Filter bei 0).
//!
//! # Live-Filter (TECH-005)
//!
//! Alle Filter sind reine SQL-Prädikate und werden mit jedem Zugriff neu bewertet; sobald ein Alternate
//! abgerufen ist, wird die Quelle neu bewertet, und ein später gefundener Rücklink nimmt sie wieder heraus.
//!
//! # Codeprüfung (TECH-072)
//!
//! `hreflang.code_valid` wird beim Speichern berechnet; Sitemap-hreflang ist mit erlaubtem Script-Subtag
//! geprüft (K-35). Ist `hreflang.allow_script_subtag` aus, gilt jeder Code in der Form `ll-Ssss[-RR]` als
//! ungültig – die Entscheidung fällt hier im Prädikat und wirkt nach einer Konfigurationsänderung ohne Recrawl.

use os_model::{CrawlConfig, Value};

use crate::defs::{ColumnDef, ColumnKind, ColumnSql, FilterDef, FilterKind, SortKey, TabDef};

/// Grundmenge: interne Seiten mit hreflang-Bezug; `:store` ist `spider.hreflang.store`.
const POPULATION: &str = "u.is_internal = 1 AND u.in_tables = 1 AND :store = 1 AND (\
    (u.content_class = 'HTML' AND u.status_code = 200) \
    OR EXISTS (SELECT 1 FROM hreflang h WHERE h.url_id = u.id AND h.source <> 'html') \
    OR (u.state = 'done' AND EXISTS \
        (SELECT 1 FROM hreflang h WHERE h.target_id = u.id AND h.url_id <> u.id)))";

/// Die Seite hat mindestens eine Annotation (aus beliebiger Quelle).
const HAS_ANNOTATION: &str = "EXISTS (SELECT 1 FROM hreflang h WHERE h.url_id = u.id)";

/// Mindestens ein Alternate (≠ Seite) antwortet nicht mit 200: robots-gesperrt, ohne Antwort, 3xx, 4xx, 5xx.
/// Nicht abgerufene Ziele (`state <> 'done'`) zählen nicht (TECH-074).
const NON_200: &str = "EXISTS (SELECT 1 FROM hreflang h JOIN urls t ON t.id = h.target_id \
    WHERE h.url_id = u.id AND t.id <> u.id AND t.state = 'done' AND COALESCE(t.status_code, 0) <> 200)";

/// Mindestens ein Alternate Q (intern, mit 200 als HTML bzw. mit HTTP-hreflang geparst) verweist nicht auf die
/// Seite zurück; der Rücklink darf aus jeder Quelle stammen, verglichen wird die URL-Kennung (TECH-002).
const MISSING_RETURN: &str = "EXISTS (SELECT 1 FROM hreflang h JOIN urls q ON q.id = h.target_id \
    WHERE h.url_id = u.id AND q.id <> u.id AND q.is_internal = 1 AND q.state = 'done' AND q.status_code = 200 \
    AND (q.content_class = 'HTML' OR EXISTS \
        (SELECT 1 FROM hreflang qh WHERE qh.url_id = q.id AND qh.source = 'http')) \
    AND NOT EXISTS (SELECT 1 FROM hreflang r WHERE r.url_id = q.id AND r.target_id = u.id))";

/// Mindestens eine Annotation hat einen ungültigen Code (TECH-072); bei abgeschaltetem Script-Subtag gilt auch
/// `ll-Ssss` und `ll-Ssss-RR` (normalisiert kleingeschrieben) als ungültig.
const INCORRECT_CODES: &str = "EXISTS (SELECT 1 FROM hreflang h WHERE h.url_id = u.id AND \
    (h.code_valid = 0 OR (:allow_script = 0 AND \
        (h.code_norm GLOB '[a-z][a-z]-[a-z][a-z][a-z][a-z]' \
         OR h.code_norm GLOB '[a-z][a-z]-[a-z][a-z][a-z][a-z]-*'))))";

/// Interne HTML-200-Seite ohne Annotation (TECH-085).
const MISSING: &str = "u.is_internal = 1 AND u.content_class = 'HTML' AND u.status_code = 200 \
    AND NOT EXISTS (SELECT 1 FROM hreflang h WHERE h.url_id = u.id)";

/// Liste der Codes einer Quelle (`?1` = URL-Kennung), in Quelltext- bzw. Header-Reihenfolge (TECH-004).
macro_rules! codes_sql {
    ($source:literal) => {
        concat!(
            "SELECT h.code_raw FROM hreflang h WHERE h.url_id = ?1 AND h.source = '",
            $source,
            "' ORDER BY h.position"
        )
    };
}

/// Liste der Ziel-URLs einer Quelle: Adresse der Zielzeile, sonst aufgelöste URL, sonst Rohwert.
macro_rules! urls_sql {
    ($source:literal) => {
        concat!(
            "SELECT COALESCE((SELECT t.address FROM urls t WHERE t.id = h.target_id), h.url, h.href_raw) \
             FROM hreflang h WHERE h.url_id = ?1 AND h.source = '",
            $source,
            "' ORDER BY h.position"
        )
    };
}

fn multi(
    key: &'static str,
    base: &'static str,
    list_sql: &'static str,
    kind: ColumnKind,
) -> ColumnDef {
    ColumnDef {
        key,
        sql: ColumnSql::Multi { base, list_sql },
        kind,
        default_visible: true,
        extension: false,
    }
}

fn filter(id: &'static str, predicate: &'static str) -> FilterDef {
    FilterDef {
        id,
        predicate,
        kind: FilterKind::Live,
        requires: Vec::new(),
        ignore_non_indexable: false,
    }
}

/// Parameter aus der Konfiguration: `store` (hreflang speichern) und `allow_script` (Script-Subtag erlaubt).
fn params(cfg: &CrawlConfig) -> Vec<(&'static str, Value)> {
    vec![
        ("store", Value::Int(i64::from(cfg.spider.hreflang.store))),
        (
            "allow_script",
            Value::Int(i64::from(cfg.hreflang.allow_script_subtag)),
        ),
    ]
}

/// Definition des Tabs „Hreflang“.
///
/// Die Spalten je Quelle erscheinen als zwei Mehrfachspalten (Codes, dann URLs): `HTML hreflang 1 … n` und
/// `HTML hreflang URL 1 … n`. Die Referenz verschränkt Code und URL je Index; das lässt `ColumnSql::Multi`
/// (Name `<Basis> <n>`) nicht zu (siehe Bericht, Schnittstellenbedarf).
pub fn tab() -> TabDef {
    TabDef {
        id: "Hreflang",
        population: POPULATION,
        columns: vec![
            ColumnDef {
                key: "Address",
                sql: ColumnSql::Expr("u.address"),
                kind: ColumnKind::Url,
                default_visible: true,
                extension: false,
            },
            ColumnDef {
                key: "Title 1",
                sql: ColumnSql::Expr("u.title_1"),
                kind: ColumnKind::Text,
                default_visible: true,
                extension: false,
            },
            ColumnDef {
                key: "Occurrences",
                sql: ColumnSql::Expr("(SELECT COUNT(*) FROM hreflang h WHERE h.url_id = u.id)"),
                kind: ColumnKind::Int,
                default_visible: true,
                extension: false,
            },
            multi(
                "HTML hreflang",
                "HTML hreflang",
                codes_sql!("html"),
                ColumnKind::Text,
            ),
            multi(
                "HTML hreflang URL",
                "HTML hreflang URL",
                urls_sql!("html"),
                ColumnKind::Url,
            ),
            multi(
                "HTTP hreflang",
                "HTTP hreflang",
                codes_sql!("http"),
                ColumnKind::Text,
            ),
            multi(
                "HTTP hreflang URL",
                "HTTP hreflang URL",
                urls_sql!("http"),
                ColumnKind::Url,
            ),
            multi(
                "Sitemap hreflang",
                "Sitemap hreflang",
                codes_sql!("sitemap"),
                ColumnKind::Text,
            ),
            multi(
                "Sitemap hreflang URL",
                "Sitemap hreflang URL",
                urls_sql!("sitemap"),
                ColumnKind::Url,
            ),
        ],
        filters: vec![
            filter("All", "1"),
            filter("Contains Hreflang", HAS_ANNOTATION),
            filter("Non-200 Hreflang URLs", NON_200),
            filter("Missing Return Links", MISSING_RETURN),
            filter("Incorrect Language & Region Codes", INCORRECT_CODES),
            filter("Missing", MISSING),
        ],
        default_sort: vec![SortKey {
            column: "Address".to_string(),
            descending: false,
        }],
        params,
    }
}
