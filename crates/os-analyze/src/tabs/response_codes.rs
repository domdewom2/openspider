//! Tab „Response Codes“ (ONPAGE-050 … 064, CRAWL-012).
//!
//! Grundmenge: alle URLs mit Antwort- oder Blockierungsstatus, die in den Tabellen stehen (intern und
//! extern; Sitemap-Dateien und die robots.txt jedes Hosts sind `in_tables = 0`, K-40/K-54). Jede Statusklasse
//! gibt es in drei Sichten (ONPAGE-051): ohne Präfix (intern + extern), `Internal …` und `External …`; die
//! beiden letzten zerlegen die erste disjunkt.
//!
//! Klassen (alle live per SQL, kein Nachlauf):
//! * **Blocked by Robots.txt** – `robots_blocked = 1` (Modus „Respektieren“ mit Status 0 und Modus
//!   „Ignorieren, aber melden“ mit echtem Status, A-17, CRAWL-099).
//! * **No Response** – Status 0 ohne robots.txt-Sperre.
//! * **Success (2XX)**, **Redirection (3XX)** (auch ohne `Location`), **Client Error (4XX)**,
//!   **Server Error (5XX)** – nach Statuscode.
//! * **Redirection (Meta Refresh)** – Weiterleitungsart `meta_refresh`, auch bei Selbst-Reload (K-57).
//! * **Redirection (HTTP Refresh)** – gültiges Header-Ziel verschieden von der eigenen URL, auch wenn
//!   eine höher priorisierte Weiterleitung vorhanden ist.
//! * **Redirect Chain** – interne URL, die auf eine URL weiterleitet, die selbst weiterleitet (≥ 2 Hops, jede
//!   Weiterleitungsart zählt). Ketten, die in einer Schleife enden, zählen mit (K-66, A-19 revidiert); eine
//!   Selbst-Weiterleitung oder ein Selbst-Refresh hat 1 Hop und zählt nicht. **Redirect Loop** enthält die
//!   Zyklus-Mitglieder (Flag `redirect_loop`) und die URLs davor, deren Kette in den Zyklus führt (A-20);
//!   eine URL kann also in beiden Filtern stehen. Beide Filter betreffen nur interne Start-URLs (ONPAGE-060:
//!   „Externe Ketten erscheinen nicht im Filter“); die `External …`-Sichten sind daher immer leer.
//! * **Other** – Codes außerhalb 100–599 (CRAWL-012, OS-Erweiterung); die Oberfläche zeigt den Filter nur
//!   bei Treffern.

use crate::defs::{ColumnDef, ColumnKind, ColumnSql, FilterDef, FilterKind, TabDef};

/// Grundmenge: Zeilen mit Antwort- oder Blockierungsstatus, sichtbar in den Tabellen.
const POPULATION: &str = "u.in_tables = 1 AND u.status_code IS NOT NULL";

macro_rules! blocked {
    () => {
        "u.robots_blocked = 1 AND NOT EXISTS (SELECT 1 FROM render_blocked b WHERE b.url=u.url AND b.reason IN ('robots','error'))"
    };
}
macro_rules! no_response {
    () => {
        "(u.status_code = 0 AND u.robots_blocked = 0)"
    };
}
macro_rules! success {
    () => {
        "u.status_code BETWEEN 200 AND 299"
    };
}
macro_rules! redirection {
    () => {
        "u.status_code BETWEEN 300 AND 399"
    };
}
macro_rules! meta_refresh {
    () => {
        "u.redirect_type = 'meta_refresh'"
    };
}
macro_rules! http_refresh {
    () => {
        "EXISTS (SELECT 1 FROM refresh r WHERE r.url_id = u.id AND r.source = 'header' AND r.target_url IS NOT NULL)"
    };
}
macro_rules! client_error {
    () => {
        "u.status_code BETWEEN 400 AND 499"
    };
}
macro_rules! server_error {
    () => {
        "u.status_code BETWEEN 500 AND 599"
    };
}
macro_rules! other {
    () => {
        "(u.status_code <> 0 AND (u.status_code < 100 OR u.status_code > 599))"
    };
}

/// Die Weiterleitungskette ab dieser URL erreicht eine Schleife (Flag `redirect_loop`, gesetzt von der Engine
/// bei den Mitgliedern des Zyklus): die URL selbst oder eine URL, auf die sie (über beliebig viele Hops)
/// weiterleitet. `UNION` (ohne `ALL`) beendet den Lauf auch im Zyklus. Der erste Teil (unkorreliert, nur einmal
/// ausgewertet) spart die Kettenläufe, solange der Crawl überhaupt keine Schleife kennt.
macro_rules! reaches_loop {
    () => {
        "(EXISTS (SELECT 1 FROM url_flags WHERE flag = 'redirect_loop') \
          AND EXISTS (WITH RECURSIVE walk(id) AS (SELECT u.id UNION \
                   SELECT t.redirect_target_id FROM urls t JOIN walk w ON t.id = w.id \
                   WHERE t.redirect_target_id IS NOT NULL) \
                 SELECT 1 FROM url_flags f JOIN walk w ON f.url_id = w.id \
                 WHERE f.flag = 'redirect_loop'))"
    };
}

/// Die URL leitet weiter und ihr Ziel leitet ebenfalls weiter (≥ 2 Hops). Ketten, die in einer Schleife enden,
/// zählen mit (K-66); eine Selbst-Weiterleitung oder ein Selbst-Refresh hat 1 Hop (`redirect_target_id = id`)
/// und zählt nicht.
macro_rules! redirect_chain {
    () => {
        "(u.is_internal = 1 AND u.redirect_url IS NOT NULL AND u.redirect_target_id IS NOT NULL \
          AND u.redirect_target_id <> u.id \
          AND EXISTS (SELECT 1 FROM urls t WHERE t.id = u.redirect_target_id \
                      AND t.redirect_url IS NOT NULL))"
    };
}

/// Die URL leitet weiter und ihre Kette erreicht eine Schleife (Mitglieder des Zyklus und URLs davor).
macro_rules! redirect_loop {
    () => {
        concat!(
            "(u.is_internal = 1 AND u.redirect_target_id IS NOT NULL AND ",
            reaches_loop!(),
            ")"
        )
    };
}

/// Anzahl der weiterleitenden URLs ab dieser URL (Hops einer Kette; Schleifen zählen jede URL einmal). Eine
/// URL leitet weiter, wenn sie eine Redirect URL hat; das Ziel muss dafür nicht gespeichert sein.
const NUMBER_OF_REDIRECTS_SQL: &str = "(SELECT COUNT(*) FROM (WITH RECURSIVE w(id) AS ( \
     SELECT u.id WHERE u.redirect_url IS NOT NULL \
     UNION \
     SELECT t.redirect_target_id FROM urls t JOIN w ON t.id = w.id \
       JOIN urls b ON b.id = t.redirect_target_id \
       WHERE b.redirect_url IS NOT NULL) \
     SELECT id FROM w))";

/// Anzeigename der Weiterleitungsart (ONPAGE-018, `RedirectType::tab_label`).
const REDIRECT_TYPE_SQL: &str = "CASE u.redirect_type \
     WHEN 'http' THEN 'HTTP Redirect' \
     WHEN 'hsts_policy' THEN 'HSTS Policy' \
     WHEN 'javascript' THEN 'JavaScript Redirect' \
     WHEN 'meta_refresh' THEN 'Meta Refresh Redirect' \
     WHEN 'http_refresh' THEN 'HTTP Refresh Redirect' \
     ELSE NULL END";

fn column(key: &'static str, sql: &'static str, kind: ColumnKind, visible: bool) -> ColumnDef {
    ColumnDef {
        key,
        sql: ColumnSql::Expr(sql),
        kind,
        default_visible: visible,
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

/// Die drei Sichten eines Prädikats: alle, intern, extern.
macro_rules! views {
    ($p:expr) => {
        (
            $p,
            concat!("u.is_internal = 1 AND (", $p, ")"),
            concat!("u.is_internal = 0 AND (", $p, ")"),
        )
    };
}

/// Spalten nach ONPAGE-050; `Matched Robots.txt Line` (ONPAGE-052) und `Number of Redirects`
/// (ONPAGE-060, OS-Erweiterung) sind standardmäßig ausgeblendet.
fn columns() -> Vec<ColumnDef> {
    vec![
        column("Address", "u.address", ColumnKind::Url, true),
        column("Content", "u.content_type", ColumnKind::Text, true),
        column("Status Code", "u.status_code", ColumnKind::Int, true),
        column("Status", "u.status", ColumnKind::Text, true),
        column("Indexability", "u.indexability", ColumnKind::Text, true),
        column(
            "Indexability Status",
            "u.indexability_status",
            ColumnKind::Text,
            true,
        ),
        column("Inlinks", "c.inlinks", ColumnKind::Int, true),
        column(
            "Response Time",
            "u.response_time_ms",
            ColumnKind::Seconds3,
            true,
        ),
        column("Redirect URL", "u.redirect_url", ColumnKind::Url, true),
        column("Redirect Type", REDIRECT_TYPE_SQL, ColumnKind::Text, true),
        column(
            "Matched Robots.txt Line",
            "u.robots_matched_line",
            ColumnKind::Text,
            false,
        ),
        ColumnDef {
            extension: true,
            ..column(
                "Number of Redirects",
                NUMBER_OF_REDIRECTS_SQL,
                ColumnKind::Int,
                false,
            )
        },
    ]
}

/// Definition des Tabs „Response Codes“.
pub fn tab() -> TabDef {
    let blocked = views!(blocked!());
    let no_response = views!(no_response!());
    let success = views!(success!());
    let redirection = views!(redirection!());
    let meta_refresh = views!(meta_refresh!());
    let http_refresh = views!(http_refresh!());
    let chain = views!(redirect_chain!());
    let looped = views!(redirect_loop!());
    let client_error = views!(client_error!());
    let server_error = views!(server_error!());
    let other = views!(other!());

    TabDef {
        id: "Response Codes",
        population: POPULATION,
        columns: columns(),
        filters: vec![
            filter("All", "1"),
            filter("Blocked by Robots.txt", blocked.0),
            filter("No Response", no_response.0),
            filter("Success (2XX)", success.0),
            filter("Redirection (3XX)", redirection.0),
            filter("Redirection (Meta Refresh)", meta_refresh.0),
            filter("Redirection (HTTP Refresh)", http_refresh.0),
            filter("Redirect Chain", chain.0),
            filter("Redirect Loop", looped.0),
            filter("Client Error (4XX)", client_error.0),
            filter("Server Error (5XX)", server_error.0),
            filter("Other", other.0),
            filter("Internal All", "u.is_internal = 1"),
            filter("Internal Blocked by Robots.txt", blocked.1),
            filter("Internal No Response", no_response.1),
            filter("Internal Success (2XX)", success.1),
            filter("Internal Redirection (3XX)", redirection.1),
            filter("Internal Redirection (Meta Refresh)", meta_refresh.1),
            filter("Internal Redirection (HTTP Refresh)", http_refresh.1),
            filter("Internal Redirect Chain", chain.1),
            filter("Internal Redirect Loop", looped.1),
            filter("Internal Client Error (4XX)", client_error.1),
            filter("Internal Server Error (5XX)", server_error.1),
            filter("Internal Other", other.1),
            filter("External All", "u.is_internal = 0"),
            filter("External Blocked by Robots.txt", blocked.2),
            filter("External No Response", no_response.2),
            filter("External Success (2XX)", success.2),
            filter("External Redirection (3XX)", redirection.2),
            filter("External Redirection (Meta Refresh)", meta_refresh.2),
            filter("External Redirection (HTTP Refresh)", http_refresh.2),
            filter("External Redirection (JavaScript)", "0 = 1"),
            filter("External Redirect Chain", chain.2),
            filter("External Redirect Loop", looped.2),
            filter("External Client Error (4XX)", client_error.2),
            filter("External Server Error (5XX)", server_error.2),
            filter("External Other", other.2),
        ],
        default_sort: Vec::new(),
        params: |_| Vec::new(),
        hidden_population: None,
    }
}

/// Kettenglieder für die ausgewählten Start-URLs, in Auswahl- und Sprungreihenfolge.
pub fn redirect_details(
    file: &os_store::CrawlFile,
    _: &os_model::CrawlConfig,
    req: &crate::details::DetailReq<'_>,
) -> Result<crate::details::DetailRows, crate::ViewError> {
    use os_model::Value;
    let mut rows = Vec::new();
    let mut notes = Vec::new();
    for id in req.url_ids {
        let chain = os_store::queries::redirect_chain(file.conn(), *id, 1_000)?;
        if chain.is_loop {
            notes.push((id.0.to_string(), "Redirect Loop".into()));
        }
        if chain.limit_reached {
            notes.push((id.0.to_string(), "Max Redirects Reached".into()));
        }
        for (index, hop) in chain.hops.iter().enumerate() {
            rows.push(vec![
                Value::Int(id.0),
                Value::Int(index as i64),
                Value::Text(hop.address.clone()),
                hop.status_code
                    .map_or(Value::Null, |s| Value::Int(i64::from(s))),
                hop.redirect_type
                    .map_or(Value::Null, |t| Value::Text(t.tab_label().into())),
            ]);
        }
    }
    let total = rows.len() as u64;
    let rows = rows
        .into_iter()
        .skip(req.offset.min(usize::MAX as u64) as usize)
        .take(req.limit.min(500) as usize)
        .collect();
    Ok(crate::details::DetailRows {
        columns: vec![
            ("URL ID".into(), ColumnKind::Int),
            ("Hop".into(), ColumnKind::Int),
            ("Address".into(), ColumnKind::Url),
            ("Status Code".into(), ColumnKind::Int),
            ("Redirect Type".into(), ColumnKind::Text),
        ],
        rows,
        total,
        notes,
    })
}

/// Header mit gültigem, von der eigenen URL verschiedenem Ziel. Auch Header, die neben einer
/// höher priorisierten Weiterleitung vorkommen, bleiben für diesen Filter sichtbar.
pub fn http_refresh_filter() -> &'static str {
    http_refresh!()
}

/// Hält den F05-Registrierungsaufruf auch für ergänzte Tab-Definitionen idempotent.
pub fn configure_filters(tab: &mut TabDef) {
    // M1-Filter zuerst, dann die M2-Ergänzungen in Registry-Reihenfolge. Die zentrale
    // Registrierung ergänzt dazwischen noch die Rendering-Filter von M2-R02.
    tab.filters.sort_by_key(|filter| match filter.id {
        "External Redirection (JavaScript)" => 1,
        "Redirection (HTTP Refresh)"
        | "Internal Redirection (HTTP Refresh)"
        | "External Redirection (HTTP Refresh)" => 2,
        _ => 0,
    });
    let predicates = views!(http_refresh!());
    for filter in &mut tab.filters {
        filter.predicate = match filter.id {
            "Redirection (HTTP Refresh)" => predicates.0,
            "Internal Redirection (HTTP Refresh)" => predicates.1,
            "External Redirection (HTTP Refresh)" => predicates.2,
            "Blocked Resource" | "Internal Blocked Resource" | "External Blocked Resource" => {
                filter.requires = vec![crate::defs::Requirement::Rendering];
                match filter.id {
                    "Internal Blocked Resource" => "u.is_internal=1 AND u.id IN (SELECT t.id FROM render_blocked b JOIN urls t ON t.url=b.url WHERE b.reason IN ('robots','error'))",
                    "External Blocked Resource" => "u.is_internal=0 AND u.id IN (SELECT t.id FROM render_blocked b JOIN urls t ON t.url=b.url WHERE b.reason IN ('robots','error'))",
                    _ => "u.id IN (SELECT t.id FROM render_blocked b JOIN urls t ON t.url=b.url WHERE b.reason IN ('robots','error'))",
                }
            }
            "Redirection (JavaScript)"
            | "Internal Redirection (JavaScript)"
            | "External Redirection (JavaScript)" => {
                filter.requires = vec![crate::defs::Requirement::Rendering];
                if filter.id.starts_with("External") {
                    // External pages are never rendered; keep a real zero-count filter.
                    "0 = 1"
                } else {
                    "u.is_internal=1 AND u.content_class='HTML' AND u.status_code BETWEEN 200 AND 299 AND u.redirect_type='javascript'"
                }
            }
            _ => continue,
        };
    }
}
