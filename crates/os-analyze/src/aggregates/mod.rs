//! Vorberechnete Sidebar-Aggregate (K-5). Der Cache liegt auf der Leseverbindung in
//! SQLite-Temp-Tabellen: keine Änderung der Crawl-Datei, kein crawlweiter RAM-Baum.
//! Ein erster Durchlauf baut ihn auf; danach werden nur URL-Differenzen angewandt.

pub mod live;
pub mod status;

use std::collections::BTreeMap;

use os_store::CrawlFile;
use rusqlite::params;

/// Vorberechnete Aufschlüsselung, unabhängig vom gewählten View.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Counts {
    pub count: u64,
    pub indexable: u64,
    pub non_indexable: u64,
    pub response_codes: BTreeMap<String, u64>,
}

/// Eine Verzeichniszeile. `path` ist eine vollständige URL ohne Query/Fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathCount {
    pub name: String,
    pub path: String,
    pub counts: Counts,
    pub has_children: bool,
}

/// Seitenweise direkte Kinder, deterministisch nach vollständigem Pfad sortiert.
pub fn paths(
    file: &CrawlFile,
    parent: &str,
    offset: u64,
    limit: u32,
) -> Result<Vec<PathCount>, os_store::StoreError> {
    let mut stmt = file.conn().prepare_cached(
        "SELECT a.key, a.name, a.n, a.idx, a.nonidx, a.s2, a.s3, a.s4, a.s5, a.blocked, a.noresp,
         EXISTS(SELECT 1 FROM temp.k03_counts c WHERE c.kind='path' AND c.parent=a.key AND c.n>0)
         FROM temp.k03_counts a WHERE a.kind='path' AND a.parent=?1 AND a.n>0
         ORDER BY a.key LIMIT ?2 OFFSET ?3",
    )?;
    let rows = stmt
        .query_map(
            params![parent, limit, i64::try_from(offset).unwrap_or(i64::MAX)],
            |r| {
                Ok(PathCount {
                    path: r.get(0)?,
                    name: r.get(1)?,
                    counts: counts(r, 2)?,
                    has_children: r.get(11)?,
                })
            },
        )?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Tiefen 1–9 und 10 (= 10+); Tiefe 0 und unbekannte Tiefen fehlen bewusst.
pub fn depths(file: &CrawlFile) -> Result<Vec<(u32, Counts)>, os_store::StoreError> {
    let mut stmt = file.conn().prepare_cached(
        "SELECT CAST(key AS INTEGER), n, idx, nonidx, s2, s3, s4, s5, blocked, noresp
         FROM temp.k03_counts WHERE kind='depth' ORDER BY CAST(key AS INTEGER)",
    )?;
    let found: BTreeMap<u32, Counts> = stmt
        .query_map([], |r| Ok((r.get(0)?, counts(r, 1)?)))?
        .collect::<Result<_, _>>()?;
    Ok((1..=10)
        .map(|d| (d, found.get(&d).cloned().unwrap_or_default()))
        .collect())
}

/// Antwortzeiten 0–1 s bis 10+ s, getrennt nach allen URLs und internem HTML.
/// Der zweite Wert zählt abgerufene URLs ohne Antwort, nicht die Warteschlange.
pub fn response_times(
    file: &CrawlFile,
    internal_html: bool,
) -> Result<([u64; 11], u64), os_store::StoreError> {
    let kind = if internal_html {
        "time_html"
    } else {
        "time_all"
    };
    let mut stmt = file
        .conn()
        .prepare_cached("SELECT CAST(key AS INTEGER), n FROM temp.k03_counts WHERE kind=?1")?;
    let mut buckets = [0; 11];
    let mut no_response = 0;
    for row in stmt.query_map([kind], |r| Ok((r.get::<_, i32>(0)?, r.get::<_, u64>(1)?)))? {
        let (bucket, count) = row?;
        if bucket == -1 {
            no_response = count;
        } else if (0..=10).contains(&bucket) {
            buckets[bucket as usize] = count;
        }
    }
    Ok((buckets, no_response))
}

fn counts(r: &rusqlite::Row<'_>, start: usize) -> rusqlite::Result<Counts> {
    let names = [
        "2xx",
        "3xx",
        "4xx",
        "5xx",
        "Blocked by robots.txt",
        "No Response",
    ];
    Ok(Counts {
        count: r.get(start)?,
        indexable: r.get(start + 1)?,
        non_indexable: r.get(start + 2)?,
        response_codes: names
            .into_iter()
            .enumerate()
            .map(|(i, name)| Ok((name.to_owned(), r.get(start + 3 + i)?)))
            .collect::<rusqlite::Result<_>>()?,
    })
}

/// Aufgeklappter, in der Nutzlast begrenzter Verzeichnisbaum.
#[derive(Debug, Clone)]
pub struct PathTree {
    pub row: PathCount,
    pub children: Vec<PathTree>,
    pub has_more_children: bool,
}

/// Lazy Loading: maximal 500 Knoten pro Antwort, übrige Kinder bleiben ladbar.
pub fn path_tree(
    file: &CrawlFile,
    parent: &str,
    offset: u64,
    limit: u32,
    levels: u8,
) -> Result<Vec<PathTree>, os_store::StoreError> {
    fn build(
        file: &CrawlFile,
        parent: &str,
        offset: u64,
        limit: u32,
        levels: u8,
        budget: &mut usize,
    ) -> Result<Vec<PathTree>, os_store::StoreError> {
        let rows = paths(file, parent, offset, limit.min(*budget as u32))?;
        *budget -= rows.len();
        rows.into_iter()
            .map(|row| {
                let children = if levels > 1 && row.has_children && *budget > 0 {
                    build(file, &row.path, 0, limit, levels - 1, budget)?
                } else {
                    Vec::new()
                };
                let more = row.has_children
                    && (children.is_empty()
                        || !paths(file, &row.path, children.len() as u64, 1)?.is_empty());
                Ok(PathTree {
                    row,
                    children,
                    has_more_children: more,
                })
            })
            .collect()
    }
    build(file, parent, offset, limit, levels, &mut 500)
}
