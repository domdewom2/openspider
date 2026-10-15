//! Zugriff auf die Tabelle `queue` für das RAM-Fenster der Warteschlange (K-78 b): Block lesen und der Aufbau beim
//! Fortsetzen.
//!
//! Alle Abfragen laufen auf einer **eigenen Leseverbindung** (`OpenMode::ReadOnly`, WAL: der Writer wird nicht
//! blockiert) und nie unter der Kern-Sperre. Zahlen stehen als Literale im SQL (nur Ganzzahlen aus dem eigenen
//! Zustand); Adressen kommen nie in ein SQL.
//!
//! **Reihenfolge:** Der Index `queue_order (priority, depth, seq)` sortiert Priorität 0 (mit Tiefe) und Priorität 1
//! (ohne Tiefe, `depth IS NULL`) genau wie der Speicher. Ausgelagert werden nur Einträge dieser Form
//! ([`crate::queue::Frontier`]); der Block wird deshalb je Priorität mit einer Zeilenwert-Bedingung hinter dem
//! Cursor gelesen (`(depth, seq, url_id) > (…)`), die der Index direkt bedient (kein Sortieren, keine Suche je Zeile
//! außer der Adresse aus `urls`).

use std::collections::HashSet;
use std::path::Path;

use os_model::UrlId;
use os_store::{CrawlFile, OpenMode};

use crate::frontier::{Core, Purpose, QueueItem, NO_DEPTH};
use crate::queue::{DiskRow, IdSet, Key};

/// Öffnet die Datei lesend (eigene Verbindung).
pub(crate) fn open_ro(path: &Path) -> Result<CrawlFile, String> {
    CrawlFile::open(path, OpenMode::ReadOnly).map_err(|e| e.to_string())
}

fn sql_err(e: impl std::fmt::Display) -> String {
    format!("Warteschlange lesen: {e}")
}

const SELECT_ROWS: &str = "SELECT q.url_id, q.seq, q.depth, q.priority, u.url, u.is_internal \
                           FROM queue q JOIN urls u ON u.id = q.url_id";

fn collect(file: &CrawlFile, sql: &str, out: &mut Vec<DiskRow>) -> Result<(), String> {
    let mut stmt = file.conn().prepare(sql).map_err(sql_err)?;
    let mut rows = stmt.query([]).map_err(sql_err)?;
    while let Some(row) = rows.next().map_err(sql_err)? {
        let id: i64 = row.get(0).map_err(sql_err)?;
        let seq: i64 = row.get(1).map_err(sql_err)?;
        let depth: Option<i64> = row.get(2).map_err(sql_err)?;
        let priority: i64 = row.get(3).map_err(sql_err)?;
        let url: String = row.get(4).map_err(sql_err)?;
        let internal: i64 = row.get(5).map_err(sql_err)?;
        out.push(DiskRow {
            id,
            seq: u64::try_from(seq).unwrap_or(0),
            depth: depth.and_then(|d| u32::try_from(d).ok()),
            priority: u8::try_from(priority).unwrap_or(u8::MAX),
            url,
            internal: internal != 0,
        });
    }
    Ok(())
}

/// Liest bis zu `limit` Zeilen mit Schlüssel hinter `after`, in Schlüsselreihenfolge.
pub(crate) fn read_chunk(
    file: &CrawlFile,
    after: &Key,
    limit: usize,
) -> Result<Vec<DiskRow>, String> {
    let mut out: Vec<DiskRow> = Vec::new();
    if after.priority == 0 && after.depth != NO_DEPTH {
        let sql = format!(
            "{SELECT_ROWS} WHERE q.priority = 0 AND q.depth IS NOT NULL \
             AND (q.depth, q.seq, q.url_id) > ({}, {}, {}) \
             ORDER BY q.depth, q.seq, q.url_id LIMIT {limit}",
            after.depth, after.seq as i64, after.id
        );
        collect(file, &sql, &mut out)?;
    }
    if out.len() < limit {
        let (seq, id) = if after.priority == 0 {
            (0, -1)
        } else {
            (after.seq as i64, after.id)
        };
        let rest = limit - out.len();
        let sql = format!(
            "{SELECT_ROWS} WHERE q.priority = 1 AND q.depth IS NULL \
             AND (q.seq, q.url_id) > ({seq}, {id}) \
             ORDER BY q.seq, q.url_id LIMIT {rest}"
        );
        collect(file, &sql, &mut out)?;
    }
    Ok(out)
}

/// Zahl der Zeilen in `queue`.
pub(crate) fn count_rows(file: &CrawlFile) -> Result<u64, String> {
    let n: i64 = file
        .conn()
        .query_row("SELECT COUNT(*) FROM queue", [], |r| r.get(0))
        .map_err(sql_err)?;
    Ok(u64::try_from(n).unwrap_or(0))
}

// -------------------------------------------------------------------------------------------------------
// Fortsetzen

/// Baut die Warteschlange beim Fortsetzen (CRAWL-126): Bei einer großen Warteschlange nur das Fenster (die `window`
/// Einträge mit den kleinsten Schlüsseln) plus alle nicht auslagerbaren Einträge im RAM, vom Rest nur die Kennungen
/// in einer Bitmenge (K-78 b). Eine kleine Warteschlange liegt ganz im RAM wie bisher.
///
/// `redirect_targets` = Kennungen der Weiterleitungsziele (ihre `hops` liefert `hops_of`); sie werden nie
/// ausgelagert.
///
/// Die Ressourcen-Erkennung braucht keinen Index auf `links.target_id` (K-74): ein einziger sequenzieller Lauf über
/// `links`, der nur Zeilen mit Ressourcen-Linktyp betrachtet und deren Ziele in einer Bitmenge merkt.
pub(crate) fn load_queue(
    file: &CrawlFile,
    core: &mut Core,
    redirect_targets: &HashSet<i64>,
    hops_of: &dyn Fn(i64) -> u32,
    window: usize,
) -> Result<(), String> {
    let conn = file.conn();
    let total = count_rows(file)?;
    if total == 0 {
        return Ok(());
    }
    // 1. Ressourcen (ein sequenzieller Lauf über `links`)
    let mut resources = IdSet::default();
    {
        let mut stmt = conn
            .prepare(
                "SELECT target_id FROM links \
                 WHERE link_type IN ('Image', 'CSS', 'JavaScript', 'Media', 'Iframe', 'Other') \
                 AND target_id IS NOT NULL",
            )
            .map_err(sql_err)?;
        let mut rows = stmt.query([]).map_err(sql_err)?;
        while let Some(row) = rows.next().map_err(sql_err)? {
            resources.set(row.get::<_, i64>(0).map_err(sql_err)?);
        }
    }
    let purpose_of = |row: &DiskRow| {
        if !row.internal {
            Purpose::External
        } else if resources.get(row.id) {
            Purpose::Resource
        } else {
            Purpose::Page
        }
    };
    let item_of = |row: DiskRow, purpose: Purpose| QueueItem {
        id: UrlId(row.id),
        depth: row.depth,
        priority: row.priority,
        seq: row.seq,
        purpose,
        hops: if redirect_targets.contains(&row.id) {
            hops_of(row.id)
        } else {
            0
        },
        session: 0,
        replace: false,
        url: row.url.into_boxed_str(),
    };

    // 2. Form der Schlüssel: nur „Priorität 0 mit Tiefe“ und „Priorität 1 ohne Tiefe“ lassen sich auslagern
    let unusual: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM queue WHERE NOT ((priority = 0 AND depth IS NOT NULL) \
             OR (priority = 1 AND depth IS NULL))",
            [],
            |r| r.get(0),
        )
        .map_err(sql_err)?;
    let spill = core.frontier.spill_enabled() && unusual == 0 && total > 2 * window as u64;

    if !spill {
        // ganz ins RAM, wie bisher
        let mut rows = Vec::with_capacity(usize::try_from(total).unwrap_or(0));
        collect(
            file,
            &format!("{SELECT_ROWS} ORDER BY q.priority, q.depth, q.seq"),
            &mut rows,
        )?;
        for row in rows {
            let purpose = purpose_of(&row);
            if purpose == Purpose::Resource {
                core.frontier.mark_resource(row.id);
            }
            core.frontier.push(item_of(row, purpose));
        }
        return Ok(());
    }

    // 3. Fenster laden, Rest nur als Kennungen merken
    let mut block = Vec::new();
    let start = Key {
        priority: 0,
        depth: 0,
        seq: 0,
        id: -1,
    };
    let mut disk = IdSet::default();
    {
        let mut stmt = conn.prepare("SELECT url_id FROM queue").map_err(sql_err)?;
        let mut rows = stmt.query([]).map_err(sql_err)?;
        while let Some(row) = rows.next().map_err(sql_err)? {
            let id: i64 = row.get(0).map_err(sql_err)?;
            disk.set(id);
            if resources.get(id) {
                core.frontier.mark_resource(id);
            }
        }
    }
    let mut chunk = read_chunk(file, &start, window)?;
    let floor = chunk.last().map(Key::of_row).unwrap_or(start);
    block.append(&mut chunk);
    // Weiterleitungsziele bleiben im RAM (ihre Hops stammen aus dem Weiterleitungsgraphen)
    let loaded: HashSet<i64> = block.iter().map(|r| r.id).collect();
    let mut pinned = Vec::new();
    for id in redirect_targets {
        if loaded.contains(id) || !disk.get(*id) {
            continue;
        }
        collect(
            file,
            &format!("{SELECT_ROWS} WHERE q.url_id = {id}"),
            &mut pinned,
        )?;
    }
    for row in block.into_iter().chain(pinned) {
        disk.clear(row.id);
        let purpose = purpose_of(&row);
        core.frontier.push(item_of(row, purpose));
    }
    core.frontier.adopt_disk(disk, floor);
    Ok(())
}
