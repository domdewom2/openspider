//! Wertänderungen zwischen zugeordneten, beantworteten internen HTML-Seiten (TECH-300–307).
//! Ergebnisse liegen in SQLite-Temptabellen; es lebt immer nur ein Seitenpaar im Arbeitsspeicher.
pub mod auto;
mod page;

use super::{checkpoint, CompareError, Comparison, RoaringBitmap, Side};
use crate::defs::{intern, ColumnDef, ColumnKind, FilterDef, FilterKind, TabDef};
use os_model::{CancelToken, ChangeDetectionConfig, ChangeElement};
use rusqlite::params;

/// Kanonische Filterreihenfolge, unabhängig von der Reihenfolge einer Eingabekonfiguration.
pub const ELEMENTS: [(ChangeElement, &str, &str); 14] = [
    (ChangeElement::Indexability, "Indexability", "Indexability"),
    (ChangeElement::PageTitles, "Page Titles", "Title 1"),
    (
        ChangeElement::MetaDescription,
        "Meta Description",
        "Meta Description 1",
    ),
    (ChangeElement::H1, "H1", "H1-1"),
    (ChangeElement::WordCount, "Word Count", "Word Count"),
    (ChangeElement::CrawlDepth, "Crawl Depth", "Crawl Depth"),
    (ChangeElement::Inlinks, "Inlinks", "Inlinks"),
    (
        ChangeElement::UniqueInlinks,
        "Unique Inlinks",
        "Unique Inlinks",
    ),
    (
        ChangeElement::InternalOutlinks,
        "Internal Outlinks",
        "Outlinks",
    ),
    (
        ChangeElement::UniqueInternalOutlinks,
        "Unique Internal Outlinks",
        "Unique Outlinks",
    ),
    (
        ChangeElement::ExternalOutlinks,
        "External Outlinks",
        "External Outlinks",
    ),
    (
        ChangeElement::UniqueExternalOutlinks,
        "Unique External Outlinks",
        "Unique External Outlinks",
    ),
    (
        ChangeElement::StructuredDataUniqueTypes,
        "Structured Data Unique Types",
        "Unique Types",
    ),
    (ChangeElement::Content, "Content", "Content Change %"),
];

/// Angezeigter Prozentwert; Klassifikation erfolgt erst nach dieser Rundung.
pub fn content_change_pct(similarity: f64) -> f64 {
    ((1.0 - similarity.clamp(0.0, 1.0)) * 10_000.0).round() / 100.0
}

/// MinHash-Ähnlichkeit: genau 128 Little-Endian-u32-Werte, fester Seed aus M2-D02.
pub fn signature_similarity(a: &[u8], b: &[u8]) -> Option<f64> {
    if a.is_empty() && b.is_empty() {
        return Some(1.0);
    }
    if (a.is_empty() && b.len() == 512) || (b.is_empty() && a.len() == 512) {
        return Some(0.0);
    }
    if a.len() != 512 || b.len() != 512 {
        return None;
    }
    Some(
        a.as_chunks::<4>()
            .0
            .iter()
            .zip(b.as_chunks::<4>().0.iter())
            .filter(|(a, b)| a == b)
            .count() as f64
            / 128.0,
    )
}

/// Verfügbarkeit beruht auf gespeichertem Inhaltsbereich oder MinHash, nicht auf einem Leistungsprofil.
pub fn content_available(c: &Comparison, side: Side) -> Result<bool, CompareError> {
    Ok(c.side(side).file.conn().query_row("SELECT EXISTS(SELECT 1 FROM minhash) OR EXISTS(SELECT 1 FROM url_ext WHERE key='content.text' AND v_text IS NOT NULL)", [], |r| r.get(0))?)
}

/// Berechnet den Vergleich atomar. Abbruch lässt den vorherigen Stand stehen.
pub fn calculate(
    c: &Comparison,
    cfg: &ChangeDetectionConfig,
    cancel: &CancelToken,
) -> Result<(), CompareError> {
    if cfg.content_threshold_pct > 100 {
        return Err(CompareError::Invalid(
            "content threshold must be 0..100".into(),
        ));
    }
    checkpoint(cancel)?;
    let conn = c.current().file.conn();
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch("CREATE TEMP TABLE IF NOT EXISTS c02_population(url_id INTEGER PRIMARY KEY, previous_id INTEGER, previous_address TEXT);
        CREATE TEMP TABLE IF NOT EXISTS c02_cells(url_id INTEGER, element TEXT, current, previous, delta REAL, pct REAL, changed INTEGER NOT NULL, PRIMARY KEY(url_id,element)) WITHOUT ROWID;
        DELETE FROM c02_cells; DELETE FROM c02_population;")?;
    let mut current = page::Reader::new(c.current())?;
    let mut previous = page::Reader::new(c.previous())?;
    let mut population = tx.prepare("INSERT INTO c02_population VALUES (?1,?2,?3)")?;
    let mut cell = tx.prepare("INSERT INTO c02_cells VALUES (?1,?2,?3,?4,?5,?6,?7)")?;
    for id in c.pairing().paired_current.iter() {
        checkpoint(cancel)?;
        let previous_id = c.partner(Side::Current, i64::from(id)).expect("paired URL");
        let (Some(a), Some(b)) = (
            current.read(i64::from(id), cfg)?,
            previous.read(previous_id, cfg)?,
        ) else {
            continue;
        };
        population.execute(params![id, previous_id, b.address])?;
        for &(e, _, _) in &ELEMENTS {
            if !cfg.elements.contains(&e) {
                continue;
            }
            let (a, b) = (a.value(e), b.value(e));
            let (Some(a), Some(b)) = (a, b) else {
                continue;
            };
            let (delta, pct, changed) = if e == ChangeElement::Content {
                let (rusqlite::types::Value::Blob(a), rusqlite::types::Value::Blob(b)) = (a, b)
                else {
                    continue;
                };
                let similarity = signature_similarity(a, b);
                let Some(similarity) = similarity else {
                    continue;
                };
                let pct = content_change_pct(similarity);
                cell.execute(params![
                    id,
                    e.as_str(),
                    pct,
                    rusqlite::types::Null,
                    rusqlite::types::Null,
                    pct,
                    pct > f64::from(cfg.content_threshold_pct)
                ])?;
                continue;
            } else if let (rusqlite::types::Value::Integer(a), rusqlite::types::Value::Integer(b)) =
                (a, b)
            {
                let delta = a - b;
                (
                    Some(delta as f64),
                    (*b != 0).then(|| delta as f64 / *b as f64 * 100.0),
                    a != b
                        || (e == ChangeElement::StructuredDataUniqueTypes
                            && current.types != previous.types),
                )
            } else {
                (
                    None,
                    None,
                    a != b
                        || (e == ChangeElement::Indexability
                            && current.reasons != previous.reasons),
                )
            };
            cell.execute(params![id, e.as_str(), a, b, delta, pct, changed])?;
        }
        // Zusatzspalten erklären Indexability-Grundwechsel und Typmengen bei gleicher Anzahl.
        for (e, extra_a, extra_b) in [
            ("indexability_status", &current.reasons, &previous.reasons),
            ("unique_type_names", &current.types, &previous.types),
        ] {
            let selected = if e == "indexability_status" {
                ChangeElement::Indexability
            } else {
                ChangeElement::StructuredDataUniqueTypes
            };
            if cfg.elements.contains(&selected)
                && a.value(selected).is_some()
                && b.value(selected).is_some()
            {
                cell.execute(params![
                    id,
                    e,
                    extra_a,
                    extra_b,
                    rusqlite::types::Null,
                    rusqlite::types::Null,
                    extra_a != extra_b
                ])?;
            }
        }
    }
    drop((cell, population, current, previous));
    checkpoint(cancel)?;
    tx.commit()?;
    Ok(())
}

/// Ergebnismenge in Current-Kennungen; Not Changed ist exakt das Komplement innerhalb All.
pub fn ids(c: &Comparison, filter: &str, changed: bool) -> Result<RoaringBitmap, CompareError> {
    let element = element(filter)?;
    let predicate = match element {
        None => "1".to_string(),
        Some(e) => format!("EXISTS(SELECT 1 FROM temp.c02_cells d WHERE d.url_id=p.url_id AND d.element='{}' AND d.changed=1)",e.as_str()),
    };
    let sql = if element.is_none() || changed {
        predicate
    } else {
        format!("NOT ({predicate})")
    };
    let mut stmt = c.current().file.conn().prepare(&format!(
        "SELECT p.url_id FROM temp.c02_population p WHERE {sql} ORDER BY p.url_id"
    ))?;
    let mut ids = RoaringBitmap::new();
    for id in stmt.query_map([], |r| r.get::<_, u32>(0))? {
        ids.insert(id?);
    }
    Ok(ids)
}

fn element(filter: &str) -> Result<Option<ChangeElement>, CompareError> {
    if filter.eq_ignore_ascii_case("All") {
        return Ok(None);
    }
    ELEMENTS
        .iter()
        .find(|(e, label, _)| label.eq_ignore_ascii_case(filter) || e.as_str() == filter)
        .map(|(e, _, _)| Some(*e))
        .ok_or_else(|| CompareError::Invalid(format!("unknown change element: {filter}")))
}

fn expression(element: &str, field: &str) -> String {
    format!(
        "(SELECT d.{field} FROM temp.c02_cells d WHERE d.url_id=u.id AND d.element='{element}')"
    )
}

/// Vergleichsspezifischer SQL-Tab. Derselbe Plan dient Tabellen, Sortierung, Suche und Export.
pub fn tab(cfg: &ChangeDetectionConfig, changed: bool) -> TabDef {
    let mut tab = crate::defs::placeholder_tab(super::CHANGE_DETECTION_TAB, vec![]);
    tab.population = "u.id IN (SELECT url_id FROM temp.c02_population)";
    let mut all = FilterDef::placeholder("All", FilterKind::Live, &[]);
    all.predicate = "1";
    tab.filters.push(all);
    for &(e, label, name) in &ELEMENTS {
        if !cfg.elements.contains(&e) {
            continue;
        }
        let numeric = matches!(
            e,
            ChangeElement::WordCount
                | ChangeElement::CrawlDepth
                | ChangeElement::Inlinks
                | ChangeElement::UniqueInlinks
                | ChangeElement::InternalOutlinks
                | ChangeElement::UniqueInternalOutlinks
                | ChangeElement::ExternalOutlinks
                | ChangeElement::UniqueExternalOutlinks
                | ChangeElement::StructuredDataUniqueTypes
        );
        let kind = if numeric {
            ColumnKind::Int
        } else {
            ColumnKind::Text
        };
        let mut filter = FilterDef::placeholder(label, FilterKind::Live, &[]);
        let pred = format!("EXISTS(SELECT 1 FROM temp.c02_cells d WHERE d.url_id=u.id AND d.element='{}' AND d.changed=1)",e.as_str());
        filter.predicate = intern(&if changed {
            pred
        } else {
            format!("NOT ({pred})")
        });
        tab.filters.push(filter);
        if e == ChangeElement::Content {
            tab.columns.push(ColumnDef::dynamic(
                name,
                &expression(e.as_str(), "current"),
                ColumnKind::Float { decimals: 2 },
                true,
            ));
            continue;
        }
        for (suffix, field) in [("Current", "current"), ("Previous", "previous")] {
            tab.columns.push(ColumnDef::dynamic(
                &format!("{name} ({suffix})"),
                &expression(e.as_str(), field),
                kind,
                true,
            ));
        }
        if numeric {
            for (suffix, field, kind) in [
                ("Change", "delta", ColumnKind::Int),
                ("Change %", "pct", ColumnKind::Float { decimals: 2 }),
            ] {
                tab.columns.push(ColumnDef::dynamic(
                    &format!("{name} {suffix}"),
                    &expression(e.as_str(), field),
                    kind,
                    true,
                ));
            }
        }
        let extra = match e {
            ChangeElement::Indexability => Some(("Indexability Status", "indexability_status")),
            ChangeElement::StructuredDataUniqueTypes => {
                Some(("Unique Type Names", "unique_type_names"))
            }
            _ => None,
        };
        if let Some((name, key)) = extra {
            for (suffix, field) in [("Current", "current"), ("Previous", "previous")] {
                tab.columns.push(ColumnDef::dynamic(
                    &format!("{name} ({suffix})"),
                    &expression(key, field),
                    ColumnKind::Text,
                    true,
                ));
            }
        }
    }
    crate::defs::apply_meta_defaults(&mut tab.columns);
    tab
}

/// Einheitliche Zähler für Overview und Abschlussmail (TECH-301, OUT-152).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChangeCount {
    pub element: String,
    pub current: u64,
    pub previous: u64,
    pub change: u64,
}
pub fn counts(
    c: &Comparison,
    cfg: &ChangeDetectionConfig,
    skip_empty: bool,
) -> Result<Vec<ChangeCount>, CompareError> {
    let mut out = Vec::new();
    for (e, label, _) in ELEMENTS {
        if !cfg.elements.contains(&e) {
            continue;
        }
        let n = ids(c, label, true)?.len();
        if n == 0 && skip_empty {
            continue;
        }
        out.push(ChangeCount {
            element: label.to_string(),
            current: n,
            previous: n,
            change: n,
        });
    }
    Ok(out)
}

/// Prozentänderung sortiert leer wie +∞. Hilfsschlüssel bleiben aus der Ausgabe ausgeschlossen.
pub fn tab_for_query(
    cfg: &ChangeDetectionConfig,
    changed: bool,
    q: &mut crate::views::ViewQuery,
) -> TabDef {
    use crate::views::ColumnSelection;
    let mut tab = tab(cfg, changed);
    let original: Vec<String> = tab.columns.iter().map(|c| c.key.to_string()).collect();
    let mut added = false;
    for key in &mut q.sort {
        if !key.column.ends_with(" Change %") {
            continue;
        }
        let Some(column) = tab.columns.iter().find(|c| c.key == key.column) else {
            continue;
        };
        let crate::defs::ColumnSql::Expr(sql) = column.sql else {
            continue;
        };
        let name = format!("__sort_{}", key.column);
        tab.columns.push(ColumnDef::dynamic(
            &name,
            &format!("COALESCE(({sql}),1e999)"),
            ColumnKind::Float { decimals: 2 },
            false,
        ));
        key.column = name;
        added = true;
    }
    if added && matches!(q.columns, ColumnSelection::All | ColumnSelection::Default) {
        q.columns = ColumnSelection::Named(original);
    }
    tab
}
