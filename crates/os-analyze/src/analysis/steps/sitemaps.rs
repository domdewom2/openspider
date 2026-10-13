//! Analyse-Teilschritt „Sitemaps“: Sitemap-Abgleich und Orphan-URLs (TECH-212 bis TECH-215, TECH-005).
//!
//! Der Schritt schreibt zwei Marken in `url_flags`; die Nachlauf-Filter des Tabs „Sitemaps“ lesen sie:
//!
//! * `in_sitemap` – die URL steht (exakte URL nach TECH-002, also gleiche `urls`-Zeile) in mindestens einem
//!   `<loc>` einer verarbeiteten `urlset`-Sitemap (`sitemap_entries`). Wert leer.
//! * `orphan` – die URL ist ein Orphan (TECH-214). Wert: Kennung der Wurzel-Orphan-URL, über die ein
//!   Folge-Orphan entdeckt wurde (`Found Via`); bei Wurzel-Orphans leer.
//!
//! # Orphan-Algorithmus (TECH-214)
//!
//! 1. `R` = alle URLs, die von den Start-URLs (`discovered_via = 'start'`) über Kanten des Crawls erreichbar
//!    sind – **ohne** Sitemap-Kanten. Kanten sind gespeicherte Links der Typen Hyperlink, PDF-Link,
//!    HTTP-Weiterleitung, Meta-/HTTP-Refresh, JavaScript-Weiterleitung, Canonical (Element und Header),
//!    hreflang (Element und Header) und `rel=next`/`prev`. hreflang aus Sitemaps ist keine Kante (die
//!    Angabe stammt aus der Sitemap), ebenso wenig die Einträge der Sitemaps selbst. Ressourcen (Bilder, CSS,
//!    JavaScript) sind keine Kanten.
//! 2. Wurzel-Orphans = URLs in Sitemaps, die nicht in `R` liegen.
//! 3. Folge-Orphans = alles, was von den Wurzel-Orphans aus über dieselben Kanten erreichbar ist und nicht
//!    in `R` liegt (Breitensuche, Wurzeln aufsteigend nach Kennung: gleiche Eingabe → gleiche Marken,
//!    unabhängig von der Abrufreihenfolge).
//! 4. Gemeldet werden nur interne HTML-Seiten und PDF-Dokumente der Tabellen; Ressourcen nie.
//!
//! Im Listenmodus (ohne Spider-Crawl) gibt es keine Orphans. Die Marken werden in **einer** Transaktion
//! ersetzt (nie Teilergebnisse); geänderte URL-Zeilen erhalten eine neue Sequenz im Änderungsfeed
//! (`urls.change_seq`, `crawl_meta.change_seq`).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use os_model::{CrawlConfig, LinkType, StepState};
use os_store::{queries, CrawlFile};
use rusqlite::Connection;

use crate::analysis::CancelToken;
use crate::error::AnalyzeError;

/// Marke „URL steht in einer Sitemap“.
pub const FLAG_IN_SITEMAP: &str = "in_sitemap";
/// Marke „URL ist ein Orphan“ (Wert: Wurzel-Orphan-URL oder leer).
pub const FLAG_ORPHAN: &str = "orphan";

/// Linktypen, die für die Erreichbarkeit als Kante zählen (TECH-214).
const EDGE_TYPES: [LinkType; 12] = [
    LinkType::Hyperlink,
    LinkType::PdfLink,
    LinkType::HttpRedirect,
    LinkType::MetaRefresh,
    LinkType::HttpRefresh,
    LinkType::JavaScriptRedirect,
    LinkType::Canonical,
    LinkType::HttpCanonical,
    LinkType::Hreflang,
    LinkType::HttpHreflang,
    LinkType::RelNext,
    LinkType::RelPrev,
];

/// Nach wie vielen Knoten die Abbruchmarke geprüft wird.
const CANCEL_CHECK_EVERY: usize = 1024;

/// Führt den Schritt aus. Bei Abbruch bleibt der Schritt ausstehend (`Pending`) und nichts wird geschrieben.
pub fn run(
    file: &CrawlFile,
    _cfg: &CrawlConfig,
    cancel: &CancelToken,
) -> Result<StepState, AnalyzeError> {
    let conn = file.conn();
    let in_sitemap = load_in_sitemap(conn)?;
    if cancel.is_cancelled() {
        return Ok(StepState::Pending);
    }
    let spider = file.get_meta("mode")?.as_deref() != Some("list");
    let orphans = if spider {
        match find_orphans(conn, &in_sitemap, cancel)? {
            Some(orphans) => orphans,
            None => return Ok(StepState::Pending),
        }
    } else {
        BTreeMap::new()
    };
    if cancel.is_cancelled() {
        return Ok(StepState::Pending);
    }
    apply(file, &in_sitemap, &orphans)?;
    Ok(StepState::Done)
}

/// Kennungen aller URLs, die in einer verarbeiteten `urlset`-Sitemap stehen.
fn load_in_sitemap(conn: &Connection) -> Result<BTreeSet<i64>, AnalyzeError> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT e.url_id FROM sitemap_entries e JOIN sitemaps s ON s.id = e.sitemap_id \
         WHERE s.kind = 'urlset' ORDER BY e.url_id",
    )?;
    let ids = stmt
        .query_map([], |r| r.get::<_, i64>(0))?
        .collect::<Result<BTreeSet<_>, _>>()?;
    Ok(ids)
}

/// Berechnet die Orphans: Kennung → Wurzel-Orphan (`None` bei Wurzel-Orphans). `None` im Ergebnis des
/// Aufrufs = abgebrochen.
fn find_orphans(
    conn: &Connection,
    in_sitemap: &BTreeSet<i64>,
    cancel: &CancelToken,
) -> Result<Option<BTreeMap<i64, Option<i64>>>, AnalyzeError> {
    let mut seeds_stmt = conn.prepare(
        "SELECT id FROM urls WHERE discovered_via = 'start' AND in_tables = 1 ORDER BY id",
    )?;
    let seeds = seeds_stmt
        .query_map([], |r| r.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    if seeds.is_empty() || in_sitemap.is_empty() {
        return Ok(Some(BTreeMap::new()));
    }
    let max_id: i64 = conn.query_row("SELECT COALESCE(MAX(id), 0) FROM urls", [], |r| r.get(0))?;
    let size = usize::try_from(max_id).unwrap_or(0) + 1;
    let mut seen = vec![false; size];

    let types = EDGE_TYPES
        .iter()
        .map(|t| format!("'{}'", t.as_str()))
        .collect::<Vec<_>>()
        .join(", ");
    let mut edges = conn.prepare(&format!(
        "SELECT DISTINCT target_id FROM links WHERE source_id = ?1 AND target_id IS NOT NULL \
         AND link_type IN ({types}) ORDER BY target_id"
    ))?;

    // 1. R: erreichbar von den Start-URLs ohne Sitemap-Kanten.
    let mut queue: VecDeque<i64> = VecDeque::new();
    for &s in &seeds {
        if mark(&mut seen, s) {
            queue.push_back(s);
        }
    }
    let mut steps = 0usize;
    while let Some(id) = queue.pop_front() {
        steps += 1;
        if steps.is_multiple_of(CANCEL_CHECK_EVERY) && cancel.is_cancelled() {
            return Ok(None);
        }
        for target in targets(&mut edges, id)? {
            if mark(&mut seen, target) {
                queue.push_back(target);
            }
        }
    }

    // 2. Wurzel-Orphans: in Sitemaps, nicht in R.
    let mut found: BTreeMap<i64, Option<i64>> = BTreeMap::new();
    for &root in in_sitemap {
        if mark(&mut seen, root) {
            found.insert(root, None);
            queue.push_back(root);
        }
    }

    // 3. Folge-Orphans: von den Wurzeln aus erreichbar, nicht in R.
    let mut origin: BTreeMap<i64, i64> = found.keys().map(|&r| (r, r)).collect();
    while let Some(id) = queue.pop_front() {
        steps += 1;
        if steps.is_multiple_of(CANCEL_CHECK_EVERY) && cancel.is_cancelled() {
            return Ok(None);
        }
        let root = origin.get(&id).copied().unwrap_or(id);
        for target in targets(&mut edges, id)? {
            if mark(&mut seen, target) {
                found.insert(target, Some(root));
                origin.insert(target, root);
                queue.push_back(target);
            }
        }
    }

    // 4. Nur interne HTML-Seiten und PDF-Dokumente der Tabellen werden gemeldet.
    let mut eligible = conn.prepare(
        "SELECT 1 FROM urls WHERE id = ?1 AND is_internal = 1 AND in_tables = 1 \
         AND content_class IN ('HTML', 'PDF')",
    )?;
    let mut out = BTreeMap::new();
    for (id, via) in found {
        if eligible.exists([id])? {
            out.insert(id, via);
        }
    }
    Ok(Some(out))
}

/// Markiert `id` als besucht; `true`, wenn sie neu ist (und im Bereich liegt).
fn mark(seen: &mut [bool], id: i64) -> bool {
    match usize::try_from(id).ok().and_then(|i| seen.get_mut(i)) {
        Some(slot) if !*slot => {
            *slot = true;
            true
        }
        _ => false,
    }
}

/// Ziele der Kanten, die von `id` ausgehen (aufsteigend, ohne Doppelte).
fn targets(stmt: &mut rusqlite::Statement<'_>, id: i64) -> Result<Vec<i64>, AnalyzeError> {
    let rows = stmt.query_map([id], |r| r.get::<_, i64>(0))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// Ersetzt die Marken in einer Transaktion und stempelt geänderte URL-Zeilen im Änderungsfeed.
fn apply(
    file: &CrawlFile,
    in_sitemap: &BTreeSet<i64>,
    orphans: &BTreeMap<i64, Option<i64>>,
) -> Result<(), AnalyzeError> {
    let conn = file.conn();
    conn.execute_batch("BEGIN IMMEDIATE")?;
    match apply_flags(file, in_sitemap, orphans) {
        Ok(()) => {
            conn.execute_batch("COMMIT")?;
            Ok(())
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

type FlagKey = (i64, &'static str);

fn apply_flags(
    file: &CrawlFile,
    in_sitemap: &BTreeSet<i64>,
    orphans: &BTreeMap<i64, Option<i64>>,
) -> Result<(), AnalyzeError> {
    let conn = file.conn();

    let mut desired: BTreeMap<FlagKey, Option<String>> = BTreeMap::new();
    for &id in in_sitemap {
        desired.insert((id, FLAG_IN_SITEMAP), None);
    }
    for (&id, via) in orphans {
        desired.insert((id, FLAG_ORPHAN), via.map(|v| v.to_string()));
    }

    let mut existing: BTreeMap<FlagKey, Option<String>> = BTreeMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT url_id, flag, value FROM url_flags WHERE flag IN ('in_sitemap', 'orphan')",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })?;
        for row in rows {
            let (id, flag, value) = row?;
            let flag = if flag == FLAG_IN_SITEMAP {
                FLAG_IN_SITEMAP
            } else {
                FLAG_ORPHAN
            };
            existing.insert((id, flag), value);
        }
    }

    let mut changed: BTreeSet<i64> = BTreeSet::new();
    {
        let mut delete = conn.prepare("DELETE FROM url_flags WHERE url_id = ?1 AND flag = ?2")?;
        for key in existing.keys() {
            if !desired.contains_key(key) {
                delete.execute(rusqlite::params![key.0, key.1])?;
                changed.insert(key.0);
            }
        }
    }
    {
        let mut insert = conn.prepare(
            "INSERT OR REPLACE INTO url_flags (url_id, flag, value) VALUES (?1, ?2, ?3)",
        )?;
        for (key, value) in &desired {
            if existing.get(key) != Some(value) {
                insert.execute(rusqlite::params![key.0, key.1, value])?;
                changed.insert(key.0);
            }
        }
    }

    if !changed.is_empty() {
        let seq = i64::try_from(queries::change_seq(conn)?).unwrap_or(i64::MAX - 1) + 1;
        let mut stamp = conn.prepare("UPDATE urls SET change_seq = ?1 WHERE id = ?2")?;
        for id in &changed {
            stamp.execute(rusqlite::params![seq, id])?;
        }
        file.set_meta("change_seq", &seq.to_string())?;
    }
    Ok(())
}
