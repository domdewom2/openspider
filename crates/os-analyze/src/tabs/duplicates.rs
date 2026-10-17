//! Gemeinsame Bausteine der Element-Tabs Page Titles, Meta Description, H1 und H2: Grundmenge (ONPAGE-009),
//! Duplikat-Logik (ONPAGE-005) und `ignore_non_indexable` (CRAWL-089).
//!
//! # Grundmenge (ONPAGE-009)
//!
//! Interne HTML-Seiten mit Statuscode 2xx, die in den Tabellen stehen (`in_tables = 1`; die `robots.txt` hat
//! keine Tabellen-Sichtbarkeit, K-54). PDFs gehören nicht dazu (A-5b), ebenso
//! wenig Weiterleitungen, Fehlerseiten und blockierte URLs. Nicht indexierbare Seiten bleiben in „All“.
//!
//! # Duplikate (ONPAGE-005)
//!
//! Eine Seite ist Duplikat, wenn ihr erster Wert nicht leer ist und mindestens eine *andere* Seite der
//! Vergleichsmenge denselben Wert hat (die Groß-/Kleinschreibung zählt nicht – K-58, ersetzt A-4; die
//! Normalisierung N(x) ist beim Speichern geschehen). Verglichen wird mit der Kollation `OS_TEXT`
//! (os-store, ICU-Root, Stärke Secondary): Groß-/Kleinschreibung gleich, Akzente verschieden. Sie steht an
//! jeder Verbindung der Crawl-Datei zur Verfügung, auch dort, wo ein Aufrufer (os-mcp) den Plan selbst ausführt;
//! eine eigene Kollation `str::to_lowercase` bräuchte eine Registrierung an diesen Verbindungen. Die Vergleichsmenge ist die Grundmenge, eingeschränkt
//!
//! * auf indexierbare Seiten, solange `duplicates.only_indexable` gilt, und
//! * ohne Seiten einer Paginierungsserie, solange `duplicates.ignore_paginated` gilt: Das sind Seiten mit
//!   `rel="prev"` – als `<link>` im `<head>` mit nicht leerem Wert oder als `Link`-Header.
//!
//! Beide Schalter und die Gruppierung werden **live** in SQL ausgewertet (kein Zwischenspeicher): Das Prädikat
//! prüft, ob der Wert der Seite (Kollation `OS_TEXT`) in einer unkorrelierten
//! Gruppierung der Vergleichsmenge mehr als einmal vorkommt; SQLite wertet sie je Anweisung einmal aus. Die
//! Schalter kommen als benannte Parameter aus [`element_params`]. Eine Änderung der Schalter nach dem Crawl
//! wirkt deshalb sofort und ohne neue Abrufe; Duplikate erscheinen bereits während des Crawls, sobald die
//! zweite Seite geschrieben ist.
//!
//! # `ignore_non_indexable` (CRAWL-089)
//!
//! Alle Problemfilter der Element-Tabs tragen `FilterDef.ignore_non_indexable = true`; der Plan (`views`)
//! schließt dann bei `issues.ignore_non_indexable` (Standard an) nicht indexierbare Seiten aus. „All“ bleibt
//! unverändert. [`problem_filter`] und [`all_filter`] setzen das Kennzeichen an einer Stelle.
//!
//! # Benannte Parameter
//!
//! `:element_enabled` (Speicher-Schalter des Elements; aus → Tab leer, ONPAGE-090), `:dup_only_indexable`,
//! `:dup_ignore_paginated` sowie die Schwellen des jeweiligen Tabs (`:title_max_chars` …).

use os_model::{CrawlConfig, Value};

use crate::defs::{ColumnDef, ColumnKind, ColumnSql, FilterDef, FilterKind};

/// Grundmenge der Element-Tabs über dem Alias `$a` (ONPAGE-009), inklusive Speicher-Schalter. `$p` steht vor
/// jeder Spalte: leer für die Abfrage selbst und die Duplikat-Gruppierung; ein unäres `"+"` verbietet dem
/// Planer die Indizes auf Klasse und Status.
macro_rules! population {
    ($a:literal) => {
        $crate::tabs::duplicates::population!("", $a)
    };
    ($p:literal, $a:literal) => {
        concat!(
            $p,
            $a,
            ".is_internal = 1 AND ",
            $p,
            $a,
            ".in_tables = 1 AND ",
            $p,
            $a,
            ".content_class = 'HTML' AND ",
            $p,
            $a,
            ".status_code BETWEEN 200 AND 299 AND :element_enabled = 1"
        )
    };
}

/// Seite einer Paginierungsserie über dem Alias `$a` (ONPAGE-005): `rel="prev"` als `<link>` im `<head>` mit
/// Wert oder als HTTP-`Link`-Header.
macro_rules! paginated {
    ($a:literal) => {
        concat!(
            "(",
            $a,
            ".has_rel_prev = 1 AND EXISTS (SELECT 1 FROM pagination g WHERE g.url_id = ",
            $a,
            ".id AND g.rel = 'prev' AND g.raw <> '' AND (g.in_head = 1 OR g.source <> 'html')))"
        )
    };
}

/// Zugehörigkeit zur Vergleichsmenge (ohne Grundmenge) über dem Alias `$a`.
macro_rules! comparison_member {
    ($a:literal) => {
        concat!(
            "((:dup_only_indexable = 0 OR COALESCE(",
            $a,
            ".indexability, '') <> 'Non-Indexable') AND (:dup_ignore_paginated = 0 OR NOT ",
            $crate::tabs::duplicates::paginated!($a),
            "))"
        )
    };
}

/// Prädikat „Duplicate“ für die Wertspalte `$col` von `urls` (Alias `u`): Der Wert der Seite kommt in der
/// Vergleichsmenge mindestens zweimal vor, verglichen ohne Beachtung der Groß-/Kleinschreibung (Kollation
/// `OS_TEXT`: ICU-Root, Stärke Secondary; ONPAGE-005, K-58). Die Gruppierung ist eine unkorrelierte
/// Unterabfrage, die SQLite je Anweisung einmal auswertet (O(n log n) statt eines Index-Nachschlags je Zeile).
macro_rules! duplicate_predicate {
    ($col:literal) => {
        concat!(
            "(u.",
            $col,
            " IS NOT NULL AND u.",
            $col,
            " <> '' AND ",
            $crate::tabs::duplicates::comparison_member!("u"),
            " AND (u.",
            $col,
            " COLLATE OS_TEXT) IN (SELECT d.",
            $col,
            " FROM urls d WHERE ",
            $crate::tabs::duplicates::population!("", "d"),
            " AND d.",
            $col,
            " IS NOT NULL AND d.",
            $col,
            " <> '' AND ",
            $crate::tabs::duplicates::comparison_member!("d"),
            " GROUP BY d.",
            $col,
            " COLLATE OS_TEXT HAVING COUNT(*) > 1))"
        )
    };
}

pub(crate) use {comparison_member, duplicate_predicate, paginated, population};

/// Prädikat „Missing“ (ONPAGE-091/101/121/131): beide gespeicherten Vorkommen leer oder nicht vorhanden.
macro_rules! missing_predicate {
    ($a:literal, $b:literal) => {
        concat!(
            "(COALESCE(u.",
            $a,
            ", '') = '' AND COALESCE(u.",
            $b,
            ", '') = '')"
        )
    };
}

pub(crate) use missing_predicate;

/// Die Filter „All“ aller Element-Tabs: die Grundmenge, von `ignore_non_indexable` nie betroffen.
pub(crate) fn all_filter() -> FilterDef {
    FilterDef {
        id: "All",
        predicate: "1",
        kind: FilterKind::Live,
        requires: Vec::new(),
        ignore_non_indexable: false,
        default_columns: &[],
    }
}

/// Ein Problemfilter: live, ohne Voraussetzungen, `issues.ignore_non_indexable` wirkt (CRAWL-089).
pub(crate) fn problem_filter(id: &'static str, predicate: &'static str) -> FilterDef {
    FilterDef {
        id,
        predicate,
        kind: FilterKind::Live,
        requires: Vec::new(),
        ignore_non_indexable: true,
        default_columns: &[],
    }
}

/// Eine Referenzspalte; Pixelbreiten sind standardmäßig ausgeblendet (ONPAGE-191, OUT-020).
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
        ..ColumnDef::META
    }
}

/// `Address`, vorn in jedem Element-Tab (URL-Spalte: bytewise sortiert).
pub(crate) fn address_column() -> ColumnDef {
    column("Address", "u.address", ColumnKind::Url, true)
}

/// `Indexability` und `Indexability Status`, am Ende jedes Element-Tabs.
pub(crate) fn indexability_columns() -> [ColumnDef; 2] {
    [
        column("Indexability", "u.indexability", ColumnKind::Text, true),
        column(
            "Indexability Status",
            "u.indexability_status",
            ColumnKind::Text,
            true,
        ),
    ]
}

/// Parameter, die alle Element-Tabs brauchen: Speicher-Schalter des Elements und die beiden
/// Duplikat-Schalter (ONPAGE-005, ONPAGE-090).
pub(crate) fn element_params(
    cfg: &CrawlConfig,
    element_stored: bool,
) -> Vec<(&'static str, Value)> {
    vec![
        ("element_enabled", Value::Int(i64::from(element_stored))),
        (
            "dup_only_indexable",
            Value::Int(i64::from(cfg.duplicates.only_indexable)),
        ),
        (
            "dup_ignore_paginated",
            Value::Int(i64::from(cfg.duplicates.ignore_paginated)),
        ),
    ]
}

/// Duplikate der gewählten URL, vor dem Paging nach Ähnlichkeit/Adresse geordnet.
pub fn detail_rows(
    file: &os_store::CrawlFile,
    cfg: &CrawlConfig,
    req: &crate::details::DetailReq<'_>,
) -> Result<crate::details::DetailRows, crate::ViewError> {
    use os_model::Value;
    let mut out = crate::details::DetailRows {
        columns: vec![
            ("Address".into(), ColumnKind::Url),
            ("Similarity".into(), ColumnKind::Int),
            ("Match Type".into(), ColumnKind::Text),
        ],
        rows: Vec::new(),
        total: 0,
        notes: Vec::new(),
    };
    let Some(id) = req.url_ids.first() else {
        return Ok(out);
    };
    let mode = req
        .params
        .get("filter")
        .and_then(|v| v.as_str())
        .unwrap_or("All");
    let exact = mode != "Near" && cfg.extraction.hash && cfg.duplicates.exact_enabled;
    let state = crate::views::state_of(file, cfg, "Content", "Near Duplicates")?;
    let near = mode != "Exact" && state == os_model::FilterState::Ready;
    if mode == "Near" && !near {
        out.notes.push((
            "needs_crawl_analysis".into(),
            "Nahezu-Duplikate benötigen eine aktuelle Crawl-Analyse".into(),
        ));
    }
    let mut stmt=file.conn().prepare("SELECT address,score,kind FROM (SELECT d.address,100 AS score,'Exact' AS kind FROM urls u JOIN urls d ON d.hash_md5=u.hash_md5 WHERE u.id=?1 AND d.id<>u.id AND u.hash_md5<>'' AND d.is_internal=1 AND d.in_tables=1 AND d.content_class='HTML' AND d.status_code BETWEEN 200 AND 299 AND (?2=0 OR (COALESCE(d.indexability,'')<>'Non-Indexable' AND COALESCE(u.indexability,'')<>'Non-Indexable')) AND ?3 UNION ALL SELECT d.address,CAST(ROUND(n.similarity*100) AS INTEGER),'Near' FROM near_duplicates n JOIN urls d ON d.id=n.other_id WHERE n.url_id=?1 AND ?4) ORDER BY score DESC,address COLLATE BINARY,kind")?;
    let rows = stmt
        .query_map(
            rusqlite::params![id.0, cfg.duplicates.only_indexable, exact, near],
            |r| {
                Ok(vec![
                    Value::Text(r.get(0)?),
                    Value::Int(r.get(1)?),
                    Value::Text(r.get(2)?),
                ])
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    out.total = rows.len() as u64;
    out.rows = rows
        .into_iter()
        .skip(req.offset.min(usize::MAX as u64) as usize)
        .take(req.limit.min(500) as usize)
        .collect();
    if out.total == 0 {
        out.notes
            .push(("empty".into(), "Keine Duplikate gefunden".into()));
    }
    Ok(out)
}

/// Häufigkeit der normalisierten Dreier-Shingles der ausgewählten URLs.
pub fn ngram_details(
    file: &os_store::CrawlFile,
    _: &CrawlConfig,
    req: &crate::details::DetailReq<'_>,
) -> Result<crate::details::DetailRows, crate::ViewError> {
    let mut counts = std::collections::BTreeMap::<String, i64>::new();
    for id in req.url_ids {
        let text = file
            .conn()
            .query_row(
                "SELECT v_text FROM url_ext WHERE url_id=?1 AND key='content.text'",
                [id.0],
                |r| r.get::<_, String>(0),
            )
            .unwrap_or_default();
        for shingle in crate::minhash::shingles(&text) {
            *counts.entry(shingle).or_default() += 1;
        }
    }
    let total = counts.len() as u64;
    Ok(crate::details::DetailRows {
        columns: vec![
            ("N-Gram".into(), ColumnKind::Text),
            ("Count".into(), ColumnKind::Int),
        ],
        rows: counts
            .into_iter()
            .skip(req.offset.min(usize::MAX as u64) as usize)
            .take(req.limit.min(500) as usize)
            .map(|(s, n)| vec![Value::Text(s), Value::Int(n)])
            .collect(),
        total,
        notes: Vec::new(),
    })
}
