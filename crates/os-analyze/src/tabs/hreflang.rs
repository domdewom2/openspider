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
//! Interne HTML-Seiten mit Status 2xx (SQ-03). Andere Ziele werden nur über TECH-074 bewertet. Der Filter „Non-200 Hreflang URLs“ behält seine wörtliche Bedingung. Ist „hreflang speichern“ aus (`spider.hreflang.store`), ist der Tab leer (OS-Verhalten:
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

use crate::defs::{intern, register_coverage_domain, Requirement};
use crate::details::{DetailReq, DetailRows};
use crate::ViewError;
use os_model::{AnalysisStep, CrawlConfig, HreflangOverLimit, Value};
use os_store::CrawlFile;
use std::sync::OnceLock;

use crate::defs::{
    ColumnDef, ColumnKind, ColumnSql, FilterDef, FilterKind, PairedPart, SortKey, TabDef,
};

/// Grundmenge: interne Seiten mit hreflang-Bezug; `:store` ist `spider.hreflang.store`.
const POPULATION: &str = "u.is_internal = 1 AND u.in_tables = 1 AND :store = 1 AND (:skip = 0 OR NOT EXISTS (SELECT 1 FROM url_ext e WHERE e.url_id = u.id AND e.key IN ('hreflang.truncated', 'hreflang.truncated_sitemap') AND e.v_int > 0)) AND u.status_code BETWEEN 200 AND 299 AND (u.content_class = 'HTML' OR EXISTS (SELECT 1 FROM hreflang h WHERE h.url_id = u.id AND h.source = 'http'))";

/// Die Seite hat mindestens eine Annotation (aus beliebiger Quelle).
const HAS_ANNOTATION: &str = "EXISTS (SELECT 1 FROM hreflang h WHERE h.url_id = u.id)";

/// Relationship predicates use `u` (source), `h` (annotation), `q` (target).
/// Reports and filters share these predicates (K2-46).
const PARSED: &str = "q.id <> u.id AND q.is_internal = 1 AND q.state = 'done' AND q.status_code = 200 \
    AND (q.content_class = 'HTML' OR EXISTS (SELECT 1 FROM hreflang qh WHERE qh.url_id = q.id AND qh.source = 'http'))";
const NON_200: &str = "q.id <> u.id AND q.state = 'done' AND COALESCE(q.status_code, 0) <> 200";
const MISSING_RETURN: &str =
    "NOT EXISTS (SELECT 1 FROM hreflang r WHERE r.url_id = q.id AND r.target_id = u.id)";
const INCONSISTENT: &str = "EXISTS (SELECT 1 FROM hreflang self JOIN hreflang r ON r.url_id = q.id AND r.target_id = u.id \
    WHERE self.url_id = u.id AND self.target_id = u.id AND self.code_norm <> 'x-default' AND r.code_norm <> 'x-default' AND self.code_norm <> r.code_norm)";
const NON_CANONICAL: &str =
    "q.id <> u.id AND q.state = 'done' AND q.status_code = 200 AND q.content_class = 'HTML' \
    AND q.canonical_count > 0 AND q.canonical_self = 0";
const NOINDEX: &str = "q.state = 'done' AND q.status_code = 200 AND q.effective_index = 0";
pub(crate) const FLAG_UNLINKED: &str = "hreflang_unlinked";
const UNLINKED: &str = "q.id <> u.id AND EXISTS (SELECT 1 FROM url_flags f WHERE f.url_id = q.id AND f.flag = 'hreflang_unlinked')";

fn relationship_predicate(filter: &str) -> String {
    match filter {
        "Non-200 Hreflang URLs" => NON_200.into(),
        "Unlinked Hreflang URLs" => UNLINKED.into(),
        "Missing Return Links" => format!("({PARSED}) AND ({MISSING_RETURN})"),
        "Inconsistent Language & Region Return Links" => format!("({PARSED}) AND ({INCONSISTENT})"),
        "Non-Canonical Return Links" => NON_CANONICAL.into(),
        "Noindex Return Links" => NOINDEX.into(),
        _ => "1".into(),
    }
}

fn relationship_filter(id: &'static str) -> FilterDef {
    // K2-93/TECH-078: the tab lists the canonicalised alternate Q. Reports retain P -> Q.
    if id == "Non-Canonical Return Links" {
        return filter(id, "u.state = 'done' AND u.status_code = 200 AND u.content_class = 'HTML' \
            AND u.canonical_count > 0 AND u.canonical_self = 0 \
            AND EXISTS (SELECT 1 FROM hreflang h JOIN urls p ON p.id = h.url_id \
                WHERE h.target_id = u.id AND p.id <> u.id AND p.is_internal = 1 \
                AND p.in_tables = 1 AND p.status_code BETWEEN 200 AND 299 AND p.content_class = 'HTML')");
    }
    let mut def = filter(id, intern(&format!(
        "EXISTS (SELECT 1 FROM hreflang h JOIN urls q ON q.id = h.target_id WHERE h.url_id = u.id AND ({}))",
        relationship_predicate(id)
    )));
    if id == "Unlinked Hreflang URLs" {
        def.kind = FilterKind::PostCrawl(AnalysisStep::Hreflang);
    }
    def
}

/// Mindestens eine Annotation hat einen ungültigen Code (TECH-072); bei abgeschaltetem Script-Subtag gilt auch
/// `ll-Ssss` und `ll-Ssss-RR` (normalisiert kleingeschrieben) als ungültig.
const INCORRECT_CODES: &str = "EXISTS (SELECT 1 FROM hreflang h WHERE h.url_id = u.id AND \
    (h.code_valid = 0 OR (:allow_script = 0 AND \
        (h.code_norm GLOB '[a-z][a-z]-[a-z][a-z][a-z][a-z]' \
         OR h.code_norm GLOB '[a-z][a-z]-[a-z][a-z][a-z][a-z]-*'))))";

/// Interne HTML-Seite mit Status 2xx ohne Annotation (TECH-085, K-56).
const MISSING: &str =
    "u.is_internal = 1 AND u.content_class = 'HTML' AND u.status_code BETWEEN 200 AND 299 \
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

/// Teilspalten je Quelle in der Reihenfolge von TECH-071: je Index erst der Code, dann die URL.
macro_rules! paired_parts {
    ($source:literal) => {
        &[
            PairedPart {
                suffix: "",
                list_sql: codes_sql!($source),
                kind: ColumnKind::Text,
            },
            PairedPart {
                suffix: "URL",
                list_sql: urls_sql!($source),
                kind: ColumnKind::Url,
            },
        ]
    };
}

/// Verschränkte Mehrfachspalte einer Quelle: `<Basis> <n>`, `<Basis> <n> URL` für n = 1 … max.
fn paired(base: &'static str, parts: &'static [PairedPart]) -> ColumnDef {
    ColumnDef {
        key: base,
        sql: ColumnSql::MultiPaired { base, parts },
        kind: ColumnKind::Text,
        default_visible: true,
        extension: false,
        ..ColumnDef::META
    }
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

/// Parameter aus der Konfiguration: `store` (hreflang speichern) und `allow_script` (Script-Subtag erlaubt).
fn params(cfg: &CrawlConfig) -> Vec<(&'static str, Value)> {
    vec![
        ("store", Value::Int(i64::from(cfg.spider.hreflang.store))),
        (
            "skip",
            Value::Int(i64::from(
                cfg.hreflang.over_limit == HreflangOverLimit::Skip,
            )),
        ),
        (
            "allow_script",
            Value::Int(i64::from(cfg.hreflang.allow_script_subtag)),
        ),
    ]
}

/// Definition des Tabs „Hreflang“.
///
/// Die Spalten je Quelle sind verschränkte Mehrfachspalten (`ColumnSql::MultiPaired`, TECH-071): je Index erst
/// der Code, dann die URL – `HTML hreflang 1`, `HTML hreflang 1 URL`, `HTML hreflang 2`, … (ebenso `HTTP` und
/// `Sitemap`). Die Anzahl der Indizes ist das Maximum über die Ansicht.
pub fn tab() -> TabDef {
    // Der Prüfmarker der HTML-Validierung (M2-D10) existiert nur für geparste HTML-Seiten; Sitemap-/HTTP-
    // Annotationen anderer URLs zählen nicht als ungeprüft (TECH-008, K2-63).
    register_coverage_domain(
        "Hreflang",
        "validation.checked",
        "u.content_class = 'HTML' AND u.status_code BETWEEN 200 AND 299",
    );
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
                ..ColumnDef::META
            },
            ColumnDef {
                key: "Title 1",
                sql: ColumnSql::Expr("u.title_1"),
                kind: ColumnKind::Text,
                default_visible: true,
                extension: false,
                ..ColumnDef::META
            },
            ColumnDef {
                key: "Occurrences",
                sql: ColumnSql::Expr("(SELECT COUNT(*) FROM hreflang h WHERE h.url_id = u.id)"),
                kind: ColumnKind::Int,
                default_visible: true,
                extension: false,
                ..ColumnDef::META
            },
            paired("HTML hreflang", paired_parts!("html")),
            paired("HTTP hreflang", paired_parts!("http")),
            paired("Sitemap hreflang", paired_parts!("sitemap")),
            ColumnDef::new("Indexability", "u.indexability", ColumnKind::Text, true),
            ColumnDef::new("Indexability Status", "u.indexability_status", ColumnKind::Text, true),
        ],
        filters: vec![
            filter("All", "1"),
            filter("Contains Hreflang", HAS_ANNOTATION),
            relationship_filter("Non-200 Hreflang URLs"),
            relationship_filter("Missing Return Links"),
            filter("Incorrect Language & Region Codes", INCORRECT_CODES),
            filter("Missing", MISSING),
            relationship_filter("Unlinked Hreflang URLs"),
            relationship_filter("Inconsistent Language & Region Return Links"),
            relationship_filter("Non-Canonical Return Links"),
            relationship_filter("Noindex Return Links"),
            filter("Multiple Entries", "EXISTS (SELECT 1 FROM hreflang h WHERE h.url_id = u.id GROUP BY h.code_norm HAVING COUNT(*) >= 2)"),
            filter("Missing Self Reference", "EXISTS (SELECT 1 FROM hreflang h WHERE h.url_id = u.id) AND NOT EXISTS (SELECT 1 FROM hreflang h WHERE h.url_id = u.id AND h.target_id = u.id)"),
            filter("Not Using Canonical", "EXISTS (SELECT 1 FROM hreflang h WHERE h.url_id = u.id) AND u.canonical_count > 0 AND u.canonical_self = 0 AND NOT EXISTS (SELECT 1 FROM hreflang h WHERE h.url_id = u.id AND h.url = u.canonical_url)"),
            filter("Missing X-Default", "EXISTS (SELECT 1 FROM hreflang h WHERE h.url_id = u.id) AND NOT EXISTS (SELECT 1 FROM hreflang h WHERE h.url_id = u.id AND h.code_norm = 'x-default')"),
            outside_filter(),
        ],
        default_sort: vec![SortKey {
            column: "Address".to_string(),
            descending: false,
        }],
        params,
        hidden_population: None,
    }
}

fn outside_filter() -> FilterDef {
    let mut def = filter("Outside <head>", "EXISTS (SELECT 1 FROM url_ext e WHERE e.url_id = u.id AND e.key = 'hreflang.outside_head' AND e.v_int = 1) AND EXISTS (SELECT 1 FROM url_ext e WHERE e.url_id = u.id AND e.key = 'validation.checked' AND e.v_int = 1)");
    def.requires.push(Requirement::Coverage {
        config: "advanced.html_validation",
        marker: "validation.checked",
    });
    def
}

fn confirmation_sql() -> &'static str {
    static SQL: OnceLock<String> = OnceLock::new();
    SQL.get_or_init(|| {
        format!(
            "CASE
        WHEN h.target_id = u.id THEN 'Self'
        WHEN q.id IS NULL OR q.state <> 'done' THEN 'Not Crawled'
        WHEN COALESCE(q.status_code, 0) <> 200 THEN 'Non-200'
        WHEN ({NOINDEX}) THEN 'Noindex'
        WHEN ({NON_CANONICAL}) THEN 'Non-Canonical'
        WHEN NOT ({PARSED}) THEN 'Not Applicable'
        WHEN ({MISSING_RETURN}) THEN 'Missing'
        WHEN ({INCONSISTENT}) THEN 'Inconsistent'
        ELSE 'Confirmed' END"
        )
    })
}

const TARGET_URL: &str = "COALESCE(q.address, h.url, h.href_raw)";
const TARGET_STATUS: &str = "CASE WHEN q.state = 'done' THEN q.status_code END";
const TARGET_INDEX: &str = "CASE WHEN q.state = 'done' THEN q.indexability END";

/// A bounded SQL page of annotations, shared by details and reports. Sorting is whitelist-based,
/// uses the store collation and always has a deterministic relationship tie-breaker.
fn query_relationships(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    req: &DetailReq<'_>,
    fields: &[(&str, ColumnKind, &str)],
    predicate: &str,
    selected_only: bool,
) -> Result<DetailRows, ViewError> {
    let ids =
        serde_json::to_string(&req.url_ids.iter().map(|id| id.0).collect::<Vec<_>>()).expect("ids");
    let where_sql = format!(
        "({POPULATION}) AND ({predicate}) {}",
        if selected_only {
            "AND u.id IN (SELECT value FROM json_each(:ids))"
        } else {
            ""
        }
    );
    let from = format!("FROM hreflang h JOIN urls u ON u.id = h.url_id LEFT JOIN urls q ON q.id = h.target_id WHERE {where_sql}");
    let bind = |stmt: &mut rusqlite::Statement<'_>| -> Result<(), rusqlite::Error> {
        for (name, value) in [
            (":store", i64::from(cfg.spider.hreflang.store)),
            (
                ":skip",
                i64::from(cfg.hreflang.over_limit == HreflangOverLimit::Skip),
            ),
        ] {
            if let Some(i) = stmt.parameter_index(name)? {
                stmt.raw_bind_parameter(i, value)?;
            }
        }
        if let Some(i) = stmt.parameter_index(":ids")? {
            stmt.raw_bind_parameter(i, &ids)?;
        }
        Ok(())
    };
    let mut count = file.conn().prepare(&format!("SELECT COUNT(*) {from}"))?;
    bind(&mut count)?;
    let total = count.raw_query().next()?.expect("count").get(0)?;
    let mut order = Vec::new();
    for key in req.sort {
        let Some((_, kind, expr)) = fields.iter().find(|(name, _, _)| *name == key.column) else {
            return Err(ViewError::Name(crate::NameError::Unknown {
                input: key.column.clone(),
                suggestions: fields.iter().map(|(n, _, _)| n.to_string()).collect(),
            }));
        };
        order.push(format!("({expr}) IS NULL ASC"));
        order.push(format!(
            "({expr}) {} {}",
            if matches!(kind, ColumnKind::Text) {
                "COLLATE OS_TEXT"
            } else {
                ""
            },
            if key.descending { "DESC" } else { "ASC" }
        ));
    }
    order.push("u.address, u.id, h.source, h.position".into());
    let sql = format!(
        "SELECT {} {from} ORDER BY {} LIMIT :limit OFFSET :offset",
        fields
            .iter()
            .map(|(_, _, expr)| *expr)
            .collect::<Vec<_>>()
            .join(","),
        order.join(",")
    );
    let mut stmt = file.conn().prepare(&sql)?;
    bind(&mut stmt)?;
    stmt.raw_bind_parameter(
        stmt.parameter_index(":limit")?.expect("limit"),
        req.limit.min(500),
    )?;
    stmt.raw_bind_parameter(
        stmt.parameter_index(":offset")?.expect("offset"),
        req.offset.min(i64::MAX as u64),
    )?;
    let mut cursor = stmt.raw_query();
    let mut rows = Vec::new();
    while let Some(row) = cursor.next()? {
        rows.push(
            (0..fields.len())
                .map(|i| {
                    Ok(match row.get_ref(i)? {
                        rusqlite::types::ValueRef::Null => Value::Null,
                        rusqlite::types::ValueRef::Integer(v) => Value::Int(v),
                        rusqlite::types::ValueRef::Real(v) => Value::Float(v),
                        rusqlite::types::ValueRef::Text(v) => {
                            Value::Text(String::from_utf8_lossy(v).into_owned())
                        }
                        rusqlite::types::ValueRef::Blob(_) => Value::Null,
                    })
                })
                .collect::<Result<Vec<_>, rusqlite::Error>>()?,
        );
    }
    Ok(DetailRows {
        columns: fields.iter().map(|(n, k, _)| (n.to_string(), *k)).collect(),
        rows,
        total,
        notes: Vec::new(),
    })
}

/// Selected annotations with confirmation precedence from TECH-089 and K2-46.
pub fn detail_rows(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    req: &DetailReq<'_>,
) -> Result<DetailRows, ViewError> {
    let mut fields = Vec::new();
    if req.url_ids.len() > 1 {
        fields.push(("Address", ColumnKind::Url, "u.address"));
    }
    fields.extend([
        ("Source", ColumnKind::Text, "h.source"),
        ("Language Code", ColumnKind::Text, "h.code_raw"),
        ("Hreflang URL", ColumnKind::Url, TARGET_URL),
        ("Status Code", ColumnKind::Int, TARGET_STATUS),
        ("Indexability", ColumnKind::Text, TARGET_INDEX),
        ("Confirmation", ColumnKind::Text, confirmation_sql()),
    ]);
    let mut data = query_relationships(file, cfg, req, &fields, "1", true)?;
    for id in req
        .url_ids
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
    {
        data.notes.extend(url_details(file, cfg, *id)?);
    }
    Ok(data)
}

/// Per-URL truncation notice, including HTTP/HTML and sitemap counters transported by F05a.
pub fn url_details(
    file: &CrawlFile,
    _: &CrawlConfig,
    url: os_model::UrlId,
) -> Result<Vec<(String, String)>, ViewError> {
    let n: i64 = file.conn().query_row("SELECT COALESCE(SUM(v_int), 0) FROM url_ext WHERE url_id = ?1 AND key IN ('hreflang.truncated', 'hreflang.truncated_sitemap')", [url.0], |r| r.get(0))?;
    Ok(if n > 0 {
        vec![("Hreflang Truncated".into(), n.to_string())]
    } else {
        Vec::new()
    })
}

/// Bounded report page; relationship selection exactly matches the corresponding tab filter (K2-46).
/// `All` selects every annotation of the Hreflang population.
pub fn report_rows(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    filter: &str,
    offset: u64,
) -> Result<DetailRows, ViewError> {
    let text = ColumnKind::Text;
    let url = ColumnKind::Url;
    let mut fields = vec![
        ("Address", url, "u.address"),
        ("Hreflang Language", text, "h.code_raw"),
        ("Hreflang URL", url, TARGET_URL),
    ];
    match filter {
        "All" | "Non-200 Hreflang URLs" | "Unlinked Hreflang URLs" => {
            fields.extend([
                ("Annotation Source", text, "CASE h.source WHEN 'html' THEN 'HTML' WHEN 'http' THEN 'HTTP Header' ELSE 'XML Sitemap' END"),
                ("Status Code", ColumnKind::Int, TARGET_STATUS),
                ("Indexability", text, TARGET_INDEX),
                ("Is Self Reference", text, "CASE WHEN h.target_id = u.id THEN 'True' ELSE 'False' END"),
            ]);
            if filter == "Non-200 Hreflang URLs" {
                fields.push(("Status", text, "q.status"));
            }
        }
        "Missing Return Links" => fields.extend([
            ("Hreflang URL Status Code", ColumnKind::Int, TARGET_STATUS),
            ("Hreflang URL Indexability", text, TARGET_INDEX),
            ("Missing Return Link To", url, "u.address"),
        ]),
        "Inconsistent Language & Region Return Links" => {
            // The same mismatch pair supplies both output codes. K2-46 overrides the older OUT-094 direction.
            fields = vec![("Address", url, "u.address"), ("Hreflang URL", url, TARGET_URL),
                ("Expected Language", text, "(SELECT s.code_raw FROM hreflang s JOIN hreflang r ON r.url_id = q.id AND r.target_id = u.id WHERE s.url_id = u.id AND s.target_id = u.id AND s.code_norm <> r.code_norm ORDER BY s.source, s.position, r.source, r.position LIMIT 1)"),
                ("Actual Language", text, "(SELECT r.code_raw FROM hreflang s JOIN hreflang r ON r.url_id = q.id AND r.target_id = u.id WHERE s.url_id = u.id AND s.target_id = u.id AND s.code_norm <> r.code_norm ORDER BY s.source, s.position, r.source, r.position LIMIT 1)")];
        }
        "Non-Canonical Return Links" => {
            fields.push(("Canonical Of Hreflang URL", url, "q.canonical_url"))
        }
        "Noindex Return Links" => {
            fields.push(("Indexability Status", text, "q.indexability_status"))
        }
        _ => {
            return Err(ViewError::Name(crate::NameError::Unknown {
                input: filter.into(),
                suggestions: Vec::new(),
            }))
        }
    }
    if filter != "All" && !crate::views::state_of(file, cfg, "Hreflang", filter)?.yields_rows() {
        return Ok(DetailRows {
            columns: fields.iter().map(|(n, k, _)| (n.to_string(), *k)).collect(),
            rows: Vec::new(),
            total: 0,
            notes: Vec::new(),
        });
    }
    let req = DetailReq {
        url_ids: &[],
        params: &serde_json::Value::Null,
        sort: &[],
        offset,
        limit: 500,
    };
    query_relationships(
        file,
        cfg,
        &req,
        &fields,
        &relationship_predicate(filter),
        false,
    )
}
