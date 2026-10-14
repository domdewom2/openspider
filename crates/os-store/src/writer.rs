//! `SqliteSink`: schreibt `WriteBatch`es in die Crawl-Datei (eine Transaktion je Batch).
//!
//! Reihenfolge innerhalb einer Transaktion: `new_urls` → `url_updates` → `pages` (Kindzeilen und Zähler) →
//! `queue` → `sitemaps`/`robots` → `meta`/`events`/`counters`. Alle Anweisungen sind vorbereitet und werden
//! im Statement-Cache der Verbindung gehalten.
//!
//! **Änderungsfeed** (M1-Plan 3.9): Je Batch ist `n = crawl_meta.change_seq + 1`. Jede eingefügte oder
//! geänderte `urls`-Zeile – neue URLs, `UrlUpdate`s, Seiten (auch `replace`) und alle Zielzeilen von
//! Zähleränderungen – erhält `change_seq = n`; am Ende der Transaktion wird `crawl_meta.change_seq = n`
//! geschrieben. Ein vollständig leerer Batch ist ein No-op und erhöht die Sequenz nicht.
//!
//! **Kennungen:** Nach dem Schreiben gelten `next_url_id > max(NewUrl.id)` und `next_seq > max(Push.seq)`
//! in `crawl_meta` (unabhängig davon, was die Engine in `meta` übergibt), damit ein Fortsetzen nie
//! Kennungen doppelt vergibt.
//!
//! **Kindzeilen:** `http_headers.direction` ist `request` oder `response`; `pagination.rel` ist `next` oder
//! `prev`; Quellen (`source`) sind die `as_str()`-Werte der Modell-Enums; `directives.tokens` ist
//! kommagetrennt; `position` der Hinweis-Tabellen ist `order` des Parsers.

use std::path::Path;
use std::sync::{Arc, OnceLock};

use os_model::time::{Clock, SystemClock};
use os_model::{
    CanonicalRow, DirectiveRow, HeaderRow, HintSource, HreflangRow, ImageRow, LinkRow, PageFacts,
    PageWrite, PaginationRow, QueueOp, RefreshRow, RobotsWrite, SitemapWrite, UrlId, UrlUpdate,
    WriteBatch,
};
use os_model::{ElementPair, NewUrl};
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, Transaction, TransactionBehavior};

use crate::collation;
use crate::counters::{BatchCounters, Contribution};
use crate::error::StoreError;
use crate::open::{CrawlFile, OpenMode};
use crate::profile::{self, StorageProfile};
use crate::sink::CrawlSink;

/// Anzahl vorbereiteter Anweisungen, die die Verbindung vorhält.
const STATEMENT_CACHE: usize = 128;

/// Standard-Senke der Engine.
pub struct SqliteSink {
    file: CrawlFile,
    profile: StorageProfile,
    clock: Arc<dyn Clock>,
}

impl SqliteSink {
    /// Öffnet die Crawl-Datei zum Schreiben: registriert die Kollation `OS_TEXT`, setzt die PRAGMAs des
    /// Profils (CRAWL-124) und dimensioniert den Statement-Cache.
    pub fn open(path: &Path, profile: StorageProfile) -> Result<Self, StoreError> {
        let file = CrawlFile::open(path, OpenMode::ReadWrite)?;
        collation::ensure(file.conn())?;
        profile::apply(file.conn(), profile)?;
        file.conn()
            .set_prepared_statement_cache_capacity(STATEMENT_CACHE);
        Ok(SqliteSink {
            file,
            profile,
            clock: Arc::new(SystemClock),
        })
    }

    /// Ersetzt die Zeitquelle (Regel 2.8): sie liefert `robots_files.fetched_at`.
    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    /// Die geöffnete Crawl-Datei (Lesezugriff, z. B. für Diagnose und Tests).
    pub fn file(&self) -> &CrawlFile {
        &self.file
    }

    /// Das Leistungsprofil dieser Senke.
    pub fn profile(&self) -> StorageProfile {
        self.profile
    }

    /// Schließt den Crawl ab: stellt `synchronous` auf den Wert nach dem Crawl und führt das WAL vollständig
    /// in die Hauptdatei zurück (CRAWL-124).
    pub fn finish(&mut self) -> Result<(), StoreError> {
        profile::settle(self.file.conn(), self.profile)?;
        wal_checkpoint(self.file.conn())
    }
}

impl Drop for SqliteSink {
    fn drop(&mut self) {
        // Der Abschluss-Checkpoint beim Schließen der Verbindung soll mit `synchronous = NORMAL` laufen.
        let _ = profile::settle(self.file.conn(), self.profile);
    }
}

fn wal_checkpoint(conn: &Connection) -> Result<(), StoreError> {
    let t = std::time::Instant::now();
    let result = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()));
    crate::sink::diag::record_checkpoint(t);
    result?;
    Ok(())
}

impl CrawlSink for SqliteSink {
    fn write(&mut self, batch: WriteBatch) -> Result<(), StoreError> {
        write_batch(self.file.conn(), &batch, self.clock.as_ref())
    }

    /// Dauerhafter Zeitpunkt: WAL in die Hauptdatei zurückführen. Unter „Maximale Geschwindigkeit“
    /// (`synchronous = OFF`) wird dafür kurz auf `NORMAL` geschaltet, damit der Checkpoint wirklich auf den
    /// Datenträger gelangt.
    fn checkpoint(&mut self) -> Result<(), StoreError> {
        let conn = self.file.conn();
        let relaxed =
            self.profile.synchronous_during_crawl() != self.profile.synchronous_after_crawl();
        if relaxed {
            profile::settle(conn, self.profile)?;
        }
        let result = wal_checkpoint(conn);
        if relaxed {
            conn.pragma_update(None, "synchronous", self.profile.synchronous_during_crawl())?;
        }
        result
    }
}

/// Fehlerhafter Batch (unbekannte URL-ID, doppelte Adresse): `StoreError::InvalidInput`.
fn batch_error(message: String) -> StoreError {
    StoreError::InvalidInput(message)
}

fn is_empty(batch: &WriteBatch) -> bool {
    batch.new_urls.is_empty()
        && batch.url_updates.is_empty()
        && batch.pages.is_empty()
        && batch.queue.is_empty()
        && batch.sitemaps.is_empty()
        && batch.robots.is_empty()
        && batch.meta.is_empty()
        && batch.events.is_empty()
        && batch.counters.is_empty()
        && batch.url_flags.is_empty()
}

fn int(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

/// Schreibt einen Batch in einer Transaktion.
fn write_batch(conn: &Connection, batch: &WriteBatch, clock: &dyn Clock) -> Result<(), StoreError> {
    if is_empty(batch) {
        return Ok(());
    }
    let built = std::time::Instant::now();
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    let seq = read_change_seq(&tx)? + 1;

    insert_new_urls(&tx, &batch.new_urls, seq)?;
    apply_url_updates(&tx, &batch.url_updates, seq)?;
    let mut batch_counters = BatchCounters::default();
    for page in &batch.pages {
        write_page(&tx, page, seq, &mut batch_counters)?;
    }
    batch_counters.flush(&tx, seq)?;
    write_url_flags(&tx, &batch.url_flags, seq)?;
    apply_queue(&tx, &batch.queue)?;
    for sitemap in &batch.sitemaps {
        write_sitemap(&tx, sitemap)?;
    }
    for robots in &batch.robots {
        write_robots(&tx, robots, clock)?;
    }
    write_meta(&tx, batch, seq)?;
    write_events_and_counters(&tx, batch)?;
    crate::sink::diag::record_build(built);

    let committing = std::time::Instant::now();
    tx.commit()?;
    crate::sink::diag::record_commit(committing);
    Ok(())
}

fn read_change_seq(tx: &Transaction<'_>) -> Result<i64, StoreError> {
    let value: Option<String> = {
        let mut stmt =
            tx.prepare_cached("SELECT value FROM crawl_meta WHERE key = 'change_seq'")?;
        let mut rows = stmt.query([])?;
        match rows.next()? {
            Some(row) => Some(row.get(0)?),
            None => None,
        }
    };
    Ok(value.and_then(|v| v.parse().ok()).unwrap_or(0))
}

fn insert_new_urls(tx: &Transaction<'_>, urls: &[NewUrl], seq: i64) -> Result<(), StoreError> {
    if urls.is_empty() {
        return Ok(());
    }
    let mut stmt = tx.prepare_cached(
        "INSERT INTO urls (id, url, address, origin, is_internal, in_tables, state, not_crawled_reason, \
         discovered_via, crawl_depth, folder_depth, url_length, change_seq) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
    )?;
    for u in urls {
        let inserted = stmt.execute(params![
            u.id.0,
            u.url,
            u.address,
            u.origin,
            u.is_internal,
            u.in_tables,
            u.state.as_str(),
            u.not_crawled_reason.map(|r| r.as_str()),
            u.discovered_via.as_str(),
            u.crawl_depth,
            u.folder_depth,
            u.url_length,
            seq
        ]);
        match inserted {
            Ok(_) => {}
            Err(rusqlite::Error::SqliteFailure(f, _))
                if f.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                return Err(batch_error(format!(
                    "NewUrl: URL-ID {} oder Adresse {:?} ist bereits vorhanden",
                    u.id, u.url
                )));
            }
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

fn apply_url_updates(
    tx: &Transaction<'_>,
    updates: &[UrlUpdate],
    seq: i64,
) -> Result<(), StoreError> {
    if updates.is_empty() {
        return Ok(());
    }
    let mut depth =
        tx.prepare_cached("UPDATE urls SET crawl_depth = ?1, change_seq = ?2 WHERE id = ?3")?;
    let mut state = tx.prepare_cached(
        "UPDATE urls SET state = ?1, not_crawled_reason = ?2, change_seq = ?3 WHERE id = ?4",
    )?;
    let mut in_tables =
        tx.prepare_cached("UPDATE urls SET in_tables = ?1, change_seq = ?2 WHERE id = ?3")?;
    for update in updates {
        let (changed, id) = match update {
            UrlUpdate::Depth { id, depth: d } => (depth.execute(params![d, seq, id.0])?, id),
            UrlUpdate::State {
                id,
                state: s,
                reason,
            } => (
                state.execute(params![s.as_str(), reason.map(|r| r.as_str()), seq, id.0])?,
                id,
            ),
            UrlUpdate::InTables { id, value } => {
                (in_tables.execute(params![value, seq, id.0])?, id)
            }
        };
        if changed == 0 {
            return Err(batch_error(format!("UrlUpdate: URL {id} existiert nicht")));
        }
    }
    Ok(())
}

/// Spalten der Zeile `urls`, die ein `PageWrite` setzt (ohne `state`, `not_crawled_reason`, `change_seq`).
const PAGE_COLUMNS: &[&str] = &[
    "crawl_timestamp",
    "status_code",
    "status",
    "no_response_reason",
    "content_type",
    "content_class",
    "http_version",
    "size_bytes",
    "transferred_bytes",
    "total_transferred_bytes",
    "response_time_ms",
    "last_modified_raw",
    "last_modified",
    "redirect_url",
    "redirect_target_id",
    "redirect_type",
    "meta_refresh_delay",
    "hash_md5",
    "charset",
    "robots_blocked",
    "robots_matched_line",
    "robots_txt_url",
    "title_count",
    "title_1",
    "title_1_len",
    "title_1_px",
    "title_1_in_head",
    "title_2",
    "title_2_len",
    "title_2_px",
    "title_2_in_head",
    "meta_desc_count",
    "meta_desc_1",
    "meta_desc_1_len",
    "meta_desc_1_px",
    "meta_desc_1_in_head",
    "meta_desc_2",
    "meta_desc_2_len",
    "meta_desc_2_px",
    "meta_desc_2_in_head",
    "meta_kw_count",
    "meta_kw_1",
    "meta_kw_1_len",
    "meta_kw_2",
    "meta_kw_2_len",
    "h1_count",
    "h1_1",
    "h1_1_len",
    "h1_2",
    "h1_2_len",
    "h2_count",
    "h2_1",
    "h2_1_len",
    "h2_2",
    "h2_2_len",
    "word_count",
    "text_ratio",
    "canonical_count",
    "canonical_url",
    "canonical_target_id",
    "canonical_self",
    "effective_index",
    "effective_follow",
    "has_rel_next",
    "has_rel_prev",
    "hreflang_count",
    "indexability",
    "indexability_status",
];

fn page_update_sql() -> &'static str {
    static SQL: OnceLock<String> = OnceLock::new();
    SQL.get_or_init(|| {
        let mut sql = String::from("UPDATE urls SET state = 'done', not_crawled_reason = NULL, ");
        for (i, column) in PAGE_COLUMNS.iter().enumerate() {
            sql.push_str(&format!("{column} = ?{}, ", i + 1));
        }
        let n = PAGE_COLUMNS.len();
        sql.push_str(&format!("change_seq = ?{} WHERE id = ?{}", n + 1, n + 2));
        sql
    })
}

fn push_text(values: &mut Vec<Value>, e: &Option<os_model::ExtractedText>, with_px: bool) {
    match e {
        Some(t) => {
            values.push(Value::from(t.text.clone()));
            values.push(Value::from(i64::from(t.len)));
            if with_px {
                values.push(Value::from(t.px.map(i64::from)));
                values.push(Value::from(i64::from(t.in_head)));
            }
        }
        None => {
            values.push(Value::Null);
            values.push(Value::Null);
            if with_px {
                values.push(Value::Null);
                values.push(Value::Null);
            }
        }
    }
}

/// `count`, danach erstes und zweites Vorkommen. `with_px`: Titel und Description haben Pixelbreite und
/// `in_head`; Keywords und Überschriften nur Text und Länge.
fn push_pair(values: &mut Vec<Value>, pair: Option<&ElementPair>, with_px: bool) {
    match pair {
        Some(p) => {
            values.push(Value::from(i64::from(p.count)));
            push_text(values, &p.first, with_px);
            push_text(values, &p.second, with_px);
        }
        None => {
            values.push(Value::Null);
            let width = if with_px { 4 } else { 2 };
            for _ in 0..2 * width {
                values.push(Value::Null);
            }
        }
    }
}

fn page_values(page: &PageWrite, seq: i64) -> Vec<Value> {
    let f = &page.fetch;
    let mut v: Vec<Value> = Vec::with_capacity(PAGE_COLUMNS.len() + 2);
    v.push(Value::from(f.crawl_timestamp.to_db()));
    v.push(Value::from(i64::from(f.status_code)));
    v.push(Value::from(f.status.clone()));
    v.push(Value::from(
        f.no_response_reason.map(|r| r.as_str().to_string()),
    ));
    v.push(Value::from(f.content_type.clone()));
    v.push(Value::from(f.content_class.as_str().to_string()));
    v.push(Value::from(f.http_version.clone()));
    v.push(Value::from(int(f.size_bytes)));
    v.push(Value::from(int(f.transferred_bytes)));
    v.push(Value::from(int(f.total_transferred_bytes)));
    v.push(Value::from(f.response_time_ms.map(i64::from)));
    v.push(Value::from(f.last_modified_raw.clone()));
    v.push(Value::from(f.last_modified.map(|t| t.to_db())));
    v.push(Value::from(f.redirect_url.clone()));
    v.push(Value::from(f.redirect_target.map(|t| t.0)));
    v.push(Value::from(f.redirect_type.map(|t| t.as_str().to_string())));
    v.push(Value::from(f.meta_refresh_delay.map(i64::from)));
    v.push(Value::from(f.hash_md5.clone()));
    v.push(Value::from(f.charset.clone()));
    v.push(Value::from(f.robots_blocked));
    v.push(Value::from(f.robots_matched_line.clone()));
    v.push(Value::from(f.robots_txt_url.clone()));

    let p: Option<&PageFacts> = page.page.as_ref();
    push_pair(&mut v, p.map(|p| &p.title), true);
    push_pair(&mut v, p.map(|p| &p.meta_description), true);
    push_pair(&mut v, p.map(|p| &p.meta_keywords), false);
    push_pair(&mut v, p.map(|p| &p.h1), false);
    push_pair(&mut v, p.map(|p| &p.h2), false);
    v.push(Value::from(p.and_then(|p| p.word_count).map(i64::from)));
    v.push(Value::from(p.and_then(|p| p.text_ratio).map(f64::from)));
    v.push(Value::from(p.map_or(0, |p| i64::from(p.canonical_count))));
    v.push(Value::from(p.and_then(|p| p.canonical_url.clone())));
    v.push(Value::from(p.and_then(|p| p.canonical_target).map(|t| t.0)));
    v.push(Value::from(p.and_then(|p| p.canonical_self)));
    v.push(Value::from(p.map(|p| p.effective_index)));
    v.push(Value::from(p.map(|p| p.effective_follow)));
    v.push(Value::from(p.map(|p| p.has_rel_next)));
    v.push(Value::from(p.map(|p| p.has_rel_prev)));
    v.push(Value::from(p.map_or(0, |p| i64::from(p.hreflang_count))));
    v.push(Value::from(page.indexability.as_str().to_string()));
    v.push(Value::from(
        page.indexability_status.map(|s| s.as_str().to_string()),
    ));
    debug_assert_eq!(v.len(), PAGE_COLUMNS.len());
    v.push(Value::from(seq));
    v.push(Value::from(page.url_id.0));
    v
}

fn source_is_internal(tx: &Transaction<'_>, id: i64) -> Result<bool, StoreError> {
    let mut stmt = tx.prepare_cached("SELECT is_internal FROM urls WHERE id = ?1")?;
    let mut rows = stmt.query([id])?;
    Ok(match rows.next()? {
        Some(row) => row.get::<_, i64>(0)? != 0,
        None => false,
    })
}

fn delete_children(tx: &Transaction<'_>, id: i64) -> Result<(), StoreError> {
    for sql in [
        "DELETE FROM links WHERE source_id = ?1",
        "DELETE FROM image_instances WHERE page_id = ?1",
        "DELETE FROM canonicals WHERE url_id = ?1",
        "DELETE FROM directives WHERE url_id = ?1",
        "DELETE FROM refresh WHERE url_id = ?1",
        // hreflang aus Sitemaps gehören der Sitemap, nicht der Seite (M1.md 3.11)
        "DELETE FROM hreflang WHERE url_id = ?1 AND source <> 'sitemap'",
        // Weiterleitungs-Flags werden beim erneuten Abruf neu bestimmt
        "DELETE FROM url_flags WHERE url_id = ?1 AND flag IN ('redirect_loop', 'redirect_limit_reached')",
        "DELETE FROM pagination WHERE url_id = ?1",
        "DELETE FROM http_headers WHERE url_id = ?1",
    ] {
        tx.prepare_cached(sql)?.execute([id])?;
    }
    Ok(())
}

fn write_page(
    tx: &Transaction<'_>,
    page: &PageWrite,
    seq: i64,
    batch_counters: &mut BatchCounters,
) -> Result<(), StoreError> {
    let id = page.url_id.0;
    let changed = tx
        .prepare_cached(page_update_sql())?
        .execute(params_from_iter(page_values(page, seq).iter()))?;
    if changed == 0 {
        return Err(batch_error(format!("PageWrite: URL {id} existiert nicht")));
    }

    let internal = source_is_internal(tx, id)?;
    let old = if page.replace {
        let old = if internal {
            Contribution::load(tx, id)?
        } else {
            Contribution::default()
        };
        delete_children(tx, id)?;
        old
    } else {
        Contribution::default()
    };

    insert_links(tx, id, &page.links)?;
    insert_images(tx, id, &page.images)?;
    insert_canonicals(tx, id, &page.canonicals)?;
    insert_directives(tx, id, &page.directives)?;
    insert_refresh(tx, id, &page.refresh)?;
    insert_hreflang(tx, id, &page.hreflang)?;
    insert_pagination(tx, id, &page.pagination)?;
    insert_headers(tx, id, &page.headers)?;

    if internal {
        let new = Contribution::from_rows(&page.links, &page.images);
        batch_counters.add_page(tx, id, &old, &new)?;
    }
    Ok(())
}

fn insert_links(tx: &Transaction<'_>, id: i64, links: &[LinkRow]) -> Result<(), StoreError> {
    if links.is_empty() {
        return Ok(());
    }
    let mut stmt = tx.prepare_cached(
        "INSERT INTO links (source_id, target_id, target_raw, link_type, position, anchor, alt_text, follow, \
         rel, target_attr, path_type, link_path, link_position, link_origin, in_head) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
    )?;
    for l in links {
        stmt.execute(params![
            id,
            l.target_id.map(|t| t.0),
            l.target_raw,
            l.link_type.as_str(),
            l.position,
            l.anchor,
            l.alt_text,
            l.follow,
            l.rel,
            l.target_attr,
            l.path_type.map(|p| p.as_str()),
            l.link_path,
            l.link_position.map(|p| p.as_str()),
            l.link_origin.as_str(),
            l.in_head
        ])?;
    }
    Ok(())
}

fn insert_images(tx: &Transaction<'_>, id: i64, images: &[ImageRow]) -> Result<(), StoreError> {
    if images.is_empty() {
        return Ok(());
    }
    let mut stmt = tx.prepare_cached(
        "INSERT INTO image_instances (page_id, image_id, src_raw, source, alt_present, alt, alt_len, \
         width_attr, height_attr, link_path, position) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
    )?;
    for i in images {
        let m = &i.instance;
        stmt.execute(params![
            id,
            i.image_id.map(|t| t.0),
            m.src_raw,
            m.source.as_str(),
            m.alt_present,
            m.alt,
            m.alt_len,
            m.width_attr,
            m.height_attr,
            m.xpath,
            m.doc_order
        ])?;
    }
    Ok(())
}

fn insert_canonicals(
    tx: &Transaction<'_>,
    id: i64,
    rows: &[CanonicalRow],
) -> Result<(), StoreError> {
    if rows.is_empty() {
        return Ok(());
    }
    let mut stmt = tx.prepare_cached(
        "INSERT OR REPLACE INTO canonicals (url_id, source, position, raw, resolved, target_id, in_head) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    for c in rows {
        stmt.execute(params![
            id,
            c.r.source.as_str(),
            c.r.order,
            c.r.raw,
            c.r.resolved,
            c.target_id.map(|t| t.0),
            c.r.in_head
        ])?;
    }
    Ok(())
}

fn insert_directives(
    tx: &Transaction<'_>,
    id: i64,
    rows: &[DirectiveRow],
) -> Result<(), StoreError> {
    if rows.is_empty() {
        return Ok(());
    }
    let mut stmt = tx.prepare_cached(
        "INSERT OR REPLACE INTO directives (url_id, source, position, agent, raw, tokens, applies, in_head) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?;
    for DirectiveRow(d) in rows {
        stmt.execute(params![
            id,
            d.source.as_str(),
            d.order,
            d.agent,
            d.raw,
            d.tokens.join(","),
            d.applies,
            d.in_head
        ])?;
    }
    Ok(())
}

fn insert_refresh(tx: &Transaction<'_>, id: i64, rows: &[RefreshRow]) -> Result<(), StoreError> {
    if rows.is_empty() {
        return Ok(());
    }
    let mut stmt = tx.prepare_cached(
        "INSERT OR REPLACE INTO refresh (url_id, source, position, raw, delay, target_raw, target_url, \
         target_id, in_head) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?;
    for r in rows {
        stmt.execute(params![
            id,
            r.r.source.as_str(),
            r.r.order,
            r.r.raw,
            r.r.delay,
            r.r.target_raw,
            r.r.target,
            r.target_id.map(|t| t.0),
            r.r.in_head
        ])?;
    }
    Ok(())
}

fn insert_hreflang(tx: &Transaction<'_>, id: i64, rows: &[HreflangRow]) -> Result<(), StoreError> {
    if rows.is_empty() {
        return Ok(());
    }
    let mut stmt = tx.prepare_cached(
        "INSERT OR REPLACE INTO hreflang (url_id, source, position, sitemap_id, code_raw, code_norm, \
         code_valid, invalid_reason, href_raw, url, target_id, in_head) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
    )?;
    for h in rows {
        stmt.execute(params![
            id,
            h.r.source.as_str(),
            h.r.order,
            h.sitemap_id.map(|s| s.0),
            h.r.code_raw,
            h.r.code_norm,
            h.r.code_valid,
            h.r.invalid_reason,
            h.r.href_raw,
            h.r.resolved,
            h.target_id.map(|t| t.0),
            h.r.in_head
        ])?;
    }
    Ok(())
}

fn insert_pagination(
    tx: &Transaction<'_>,
    id: i64,
    rows: &[PaginationRow],
) -> Result<(), StoreError> {
    if rows.is_empty() {
        return Ok(());
    }
    let mut stmt = tx.prepare_cached(
        "INSERT OR REPLACE INTO pagination (url_id, rel, source, position, raw, url, target_id, in_head) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?;
    for p in rows {
        stmt.execute(params![
            id,
            if p.r.rel_next { "next" } else { "prev" },
            p.r.source.as_str(),
            p.r.order,
            p.r.raw,
            p.r.resolved,
            p.target_id.map(|t| t.0),
            p.r.in_head
        ])?;
    }
    Ok(())
}

fn insert_headers(tx: &Transaction<'_>, id: i64, rows: &[HeaderRow]) -> Result<(), StoreError> {
    if rows.is_empty() {
        return Ok(());
    }
    let mut stmt = tx.prepare_cached(
        "INSERT OR REPLACE INTO http_headers (url_id, direction, position, name, value) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for h in rows {
        stmt.execute(params![
            id,
            if h.request { "request" } else { "response" },
            h.position,
            h.name,
            h.value
        ])?;
    }
    Ok(())
}

fn apply_queue(tx: &Transaction<'_>, ops: &[QueueOp]) -> Result<(), StoreError> {
    if ops.is_empty() {
        return Ok(());
    }
    let mut push = tx.prepare_cached(
        "INSERT OR REPLACE INTO queue (url_id, seq, depth, priority) VALUES (?1, ?2, ?3, ?4)",
    )?;
    let mut pop = tx.prepare_cached("DELETE FROM queue WHERE url_id = ?1")?;
    let mut max_seq: Option<i64> = None;
    for op in ops {
        match op {
            QueueOp::Push {
                id,
                seq,
                depth,
                priority,
            } => {
                push.execute(params![id.0, int(*seq), depth, priority])?;
                max_seq = Some(max_seq.map_or(int(*seq), |m| m.max(int(*seq))));
            }
            QueueOp::Pop { id } => {
                pop.execute([id.0])?;
            }
        }
    }
    if let Some(max) = max_seq {
        raise_meta_counter(tx, "next_seq", max.saturating_add(1))?;
    }
    Ok(())
}

/// Hebt einen numerischen `crawl_meta`-Wert auf mindestens `value` an.
fn raise_meta_counter(tx: &Transaction<'_>, key: &str, value: i64) -> Result<(), StoreError> {
    tx.prepare_cached(
        "INSERT INTO crawl_meta (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value WHERE CAST(value AS INTEGER) < ?2",
    )?
    .execute(params![key, value])?;
    Ok(())
}

/// Schreibt `url_flags` (`INSERT OR REPLACE`) und stempelt `change_seq` der betroffenen URLs (K-20).
fn write_url_flags(
    tx: &Transaction<'_>,
    flags: &[(UrlId, String, Option<String>)],
    seq: i64,
) -> Result<(), StoreError> {
    if flags.is_empty() {
        return Ok(());
    }
    let mut insert = tx.prepare_cached(
        "INSERT OR REPLACE INTO url_flags (url_id, flag, value) VALUES (?1, ?2, ?3)",
    )?;
    let mut stamp = tx.prepare_cached("UPDATE urls SET change_seq = ?1 WHERE id = ?2")?;
    for (id, flag, value) in flags {
        if stamp.execute(params![seq, id.0])? == 0 {
            return Err(batch_error(format!("url_flags: URL {id} existiert nicht")));
        }
        insert.execute(params![id.0, flag, value])?;
    }
    Ok(())
}

fn write_sitemap(tx: &Transaction<'_>, s: &SitemapWrite) -> Result<(), StoreError> {
    tx.prepare_cached(
        "INSERT OR REPLACE INTO sitemaps (id, url, url_id, parent_id, kind, entries, invalid_entries, \
         size_bytes, status_code, depth, found_via) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
    )?
    .execute(params![
        s.id.0,
        s.url,
        s.url_id.map(|u| u.0),
        s.parent.map(|p| p.0),
        s.kind.as_str(),
        s.entries,
        s.invalid_entries,
        int(s.size_bytes),
        s.status_code,
        s.depth,
        s.found_via
    ])?;
    tx.prepare_cached("DELETE FROM sitemap_entries WHERE sitemap_id = ?1")?
        .execute([s.id.0])?;
    if !s.entry_urls.is_empty() {
        let mut stmt = tx.prepare_cached(
            "INSERT OR REPLACE INTO sitemap_entries (sitemap_id, url_id, loc_raw, lastmod) \
             VALUES (?1, ?2, ?3, ?4)",
        )?;
        for (url_id, loc_raw, lastmod) in &s.entry_urls {
            stmt.execute(params![s.id.0, url_id.0, loc_raw, lastmod])?;
        }
    }
    // Eine erneute Übergabe derselben Sitemap ersetzt auch deren hreflang-Angaben.
    tx.prepare_cached("DELETE FROM hreflang WHERE source = 'sitemap' AND sitemap_id = ?1")?
        .execute([s.id.0])?;
    if !s.hreflang.is_empty() {
        let mut stmt = tx.prepare_cached(
            "INSERT OR REPLACE INTO hreflang (url_id, source, position, sitemap_id, code_raw, code_norm, \
             code_valid, invalid_reason, href_raw, url, target_id, in_head) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        )?;
        for (url_id, h) in &s.hreflang {
            stmt.execute(params![
                url_id.0,
                HintSource::Sitemap.as_str(),
                h.r.order,
                s.id.0,
                h.r.code_raw,
                h.r.code_norm,
                h.r.code_valid,
                h.r.invalid_reason,
                h.r.href_raw,
                h.r.resolved,
                h.target_id.map(|t| t.0),
                h.r.in_head
            ])?;
        }
    }
    Ok(())
}

fn write_robots(
    tx: &Transaction<'_>,
    r: &RobotsWrite,
    clock: &dyn Clock,
) -> Result<(), StoreError> {
    tx.prepare_cached(
        "INSERT OR REPLACE INTO robots_files (origin, robots_url, fetch_status, status_code, attempts, body, \
         note, fetched_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?
    .execute(params![
        r.origin,
        r.robots_url,
        r.fetch_status,
        r.status_code,
        r.attempts,
        r.body,
        r.note,
        clock.now().to_db()
    ])?;
    Ok(())
}

fn write_meta(tx: &Transaction<'_>, batch: &WriteBatch, seq: i64) -> Result<(), StoreError> {
    if !batch.meta.is_empty() {
        let mut stmt = tx.prepare_cached(
            "INSERT INTO crawl_meta (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )?;
        for (key, value) in &batch.meta {
            stmt.execute([key, value])?;
        }
    }
    if let Some(max_id) = batch.new_urls.iter().map(|u| u.id.0).max() {
        raise_meta_counter(tx, "next_url_id", max_id.saturating_add(1))?;
    }
    // Der Änderungsfeed gehört dem Writer – zuletzt, damit `meta` ihn nicht überschreiben kann.
    tx.prepare_cached(
        "INSERT INTO crawl_meta (key, value) VALUES ('change_seq', ?1) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )?
    .execute([seq.to_string()])?;
    Ok(())
}

fn write_events_and_counters(tx: &Transaction<'_>, batch: &WriteBatch) -> Result<(), StoreError> {
    if !batch.events.is_empty() {
        let mut stmt = tx.prepare_cached(
            "INSERT INTO crawl_events (at, kind, code, message) VALUES (?1, ?2, ?3, ?4)",
        )?;
        for e in &batch.events {
            stmt.execute(params![e.at.to_db(), e.kind, e.code, e.message])?;
        }
    }
    if !batch.counters.is_empty() {
        let mut stmt = tx.prepare_cached(
            "INSERT INTO crawl_counters (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = value + excluded.value",
        )?;
        for (key, delta) in &batch.counters {
            stmt.execute(params![key, delta])?;
        }
    }
    Ok(())
}
