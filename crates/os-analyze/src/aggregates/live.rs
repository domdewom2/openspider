//! Inkrementelle Aggregate (UI-066, UI-069, UI-145).
//!
//! Je URL wird nur ihr alter Beitrag auf Platte vorgehalten. Ein Update zieht ihn
//! ab und addiert den neuen: Wiederholungen zählen nie doppelt. Temp-Tabellen sind
//! verbindungslokal und funktionieren auch bei einer schreibgeschützten Hauptdatei.
use os_model::{CrawlConfig, UrlId};
use os_store::{CrawlFile, StoreError};
use rusqlite::{params, OptionalExtension};

pub(crate) const SELECT: &str = "SELECT id, url, is_internal, content_class, indexability, crawl_depth,
    status_code, robots_blocked, response_time_ms, state, no_response_reason, in_tables FROM main.urls";

#[derive(Debug)]
pub(crate) struct Contribution {
    pub(crate) paths: Vec<(String, String, String)>, // path, parent, name
    pub(crate) depth: Option<u32>,
    pub(crate) indexable: bool,
    pub(crate) non_indexable: bool,
    pub(crate) response_class: usize,
    time: Option<i32>,
    internal_html: bool,
}

pub(crate) fn contribution(r: &rusqlite::Row<'_>) -> rusqlite::Result<(i64, Contribution)> {
    let url: String = r.get(1)?;
    let internal: bool = r.get(2)?;
    let visible: bool = r.get(11)?;
    let html = r.get::<_, String>(3)? == "HTML";
    let indexable = r.get::<_, Option<String>>(4)?.as_deref() == Some("Indexable");
    let depth = r
        .get::<_, Option<u32>>(5)?
        .filter(|d| *d > 0)
        .map(|d| d.min(10));
    let status: Option<u16> = r.get(6)?;
    let blocked: bool = r.get(7)?;
    let ms: Option<u32> = r.get(8)?;
    let state: String = r.get(9)?;
    let failed = r.get::<_, Option<String>>(10)?.is_some();
    let response_class = if blocked {
        4
    } else {
        match status.map(|s| s / 100) {
            Some(2) => 0,
            Some(3) => 1,
            Some(4) => 2,
            Some(5) => 3,
            _ => 5,
        }
    };
    // Nur abgeschlossene Abrufe, einschließlich Fehler. Weder noch nicht besuchte
    // URLs noch durch robots/Regeln übersprungene URLs sind Antwortzeit-Messungen.
    let attempted = status.is_some() || failed || state == "done";
    let time = if attempted && !blocked && visible {
        if status.is_none() {
            Some(-1)
        } else {
            ms.map(|ms| (ms / 1000).min(10) as i32)
        }
    } else {
        None
    };
    Ok((
        r.get(0)?,
        Contribution {
            paths: if internal && visible {
                directory_paths(&url)
            } else {
                Vec::new()
            },
            depth: if internal && visible { depth } else { None },
            indexable: html && indexable,
            non_indexable: html && !indexable,
            response_class,
            time,
            internal_html: internal && html && visible,
        },
    ))
}

fn directory_paths(url: &str) -> Vec<(String, String, String)> {
    let url = url.split(['?', '#']).next().unwrap_or(url);
    let Some((scheme, rest)) = url.split_once("://") else {
        return Vec::new();
    };
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let root = format!("{scheme}://{host}/");
    let mut out = vec![(root.clone(), String::new(), host.to_owned())];
    let mut parent = root;
    // Der letzte Bestandteil ist die Seite (auch ohne Dateiendung), außer bei
    // abschließendem Slash: dann gehört das Verzeichnis selbst zum Pfad.
    let mut parts: Vec<_> = path.split('/').collect();
    parts.pop();
    for part in parts {
        let next = format!("{parent}{part}/");
        out.push((next.clone(), parent, part.to_owned()));
        parent = next;
    }
    out
}

fn init(file: &CrawlFile) -> Result<(), StoreError> {
    file.conn().execute_batch(&format!(
        "PRAGMA temp.cache_size=-4096;
         CREATE TEMP TABLE IF NOT EXISTS k03_rows AS {SELECT} WHERE 0;
         CREATE UNIQUE INDEX IF NOT EXISTS temp.k03_rows_id ON k03_rows(id);
         CREATE TEMP TABLE IF NOT EXISTS k03_meta(id INTEGER PRIMARY KEY CHECK(id=1), seq INTEGER NOT NULL, reset TEXT NOT NULL);
         CREATE TEMP TABLE IF NOT EXISTS k03_rowless(id INTEGER PRIMARY KEY CHECK(id=1), stored INTEGER NOT NULL);
         CREATE TEMP TABLE IF NOT EXISTS k03_counts(
           kind TEXT NOT NULL, key TEXT NOT NULL, parent TEXT NOT NULL, name TEXT NOT NULL,
           n INTEGER NOT NULL, idx INTEGER NOT NULL, nonidx INTEGER NOT NULL,
           s2 INTEGER NOT NULL, s3 INTEGER NOT NULL, s4 INTEGER NOT NULL, s5 INTEGER NOT NULL,
           blocked INTEGER NOT NULL, noresp INTEGER NOT NULL, PRIMARY KEY(kind,key)) WITHOUT ROWID;
         CREATE INDEX IF NOT EXISTS temp.k03_parent ON k03_counts(kind,parent,key);",
    ))?;
    Ok(())
}

// Bounded batches combine common paths/depth/time buckets before touching
// SQLite. Old raw facts are copied by SQLite, rather than serialized per URL.
// UI-145: keys are looked up without allocating; only a new bucket allocates.
#[derive(Default)]
struct Totals {
    // "kind\0key" → (parent, name, counts)
    map: std::collections::HashMap<String, (String, String, [i64; 9])>,
    buf: String,
}

impl Totals {
    fn put(
        &mut self,
        kind: &str,
        key: &str,
        parent: &str,
        name: &str,
        c: &Contribution,
        sign: i64,
    ) {
        self.buf.clear();
        self.buf.push_str(kind);
        self.buf.push('\0');
        self.buf.push_str(key);
        if !self.map.contains_key(self.buf.as_str()) {
            self.map.insert(
                self.buf.clone(),
                (parent.to_owned(), name.to_owned(), [0; 9]),
            );
        }
        let Some((_, _, counts)) = self.map.get_mut(self.buf.as_str()) else {
            return;
        };
        counts[0] += sign;
        counts[1] += sign * i64::from(c.indexable);
        counts[2] += sign * i64::from(c.non_indexable);
        counts[3 + c.response_class] += sign;
    }

    fn merge(&mut self, other: Totals) {
        for (key, (parent, name, counts)) in other.map {
            let entry = self.map.entry(key).or_insert((parent, name, [0; 9]));
            for (total, n) in entry.2.iter_mut().zip(counts) {
                *total += n;
            }
        }
    }
}

fn accumulate(totals: &mut Totals, c: &Contribution, sign: i64) {
    for (path, parent, name) in &c.paths {
        totals.put("path", path, parent, name, c, sign);
    }
    let mut number = itoa_buf();
    if let Some(depth) = c.depth {
        totals.put("depth", number.format(i64::from(depth)), "", "", c, sign);
    }
    if let Some(time) = c.time {
        totals.put("time_all", number.format(i64::from(time)), "", "", c, sign);
        if c.internal_html {
            totals.put("time_html", number.format(i64::from(time)), "", "", c, sign);
        }
    }
}

/// Small reusable decimal formatter (no allocation per bucket key).
struct Number([u8; 24], usize);
fn itoa_buf() -> Number {
    Number([0; 24], 0)
}
impl Number {
    fn format(&mut self, value: i64) -> &str {
        use std::io::Write;
        let mut cursor = std::io::Cursor::new(&mut self.0[..]);
        let _ = write!(cursor, "{value}");
        self.1 = cursor.position() as usize;
        std::str::from_utf8(&self.0[..self.1]).unwrap_or("")
    }
}

fn flush(file: &CrawlFile, totals: &mut Totals) -> Result<(), StoreError> {
    let mut stmt = file.conn().prepare_cached(
        "INSERT INTO temp.k03_counts(kind,key,parent,name,n,idx,nonidx,s2,s3,s4,s5,blocked,noresp)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
         ON CONFLICT(kind,key) DO UPDATE SET n=n+excluded.n, idx=idx+excluded.idx, nonidx=nonidx+excluded.nonidx,
           s2=s2+excluded.s2,s3=s3+excluded.s3,s4=s4+excluded.s4,s5=s5+excluded.s5,
           blocked=blocked+excluded.blocked,noresp=noresp+excluded.noresp",
    )?;
    for (composite, (parent, name, n)) in std::mem::take(&mut totals.map) {
        let (kind, key) = composite
            .split_once('\0')
            .unwrap_or((composite.as_str(), ""));
        stmt.execute(params![
            kind, key, parent, name, n[0], n[1], n[2], n[3], n[4], n[5], n[6], n[7], n[8]
        ])?;
    }
    Ok(())
}

fn add(file: &CrawlFile, c: &Contribution, sign: i64) -> Result<(), StoreError> {
    let mut totals = Totals::default();
    accumulate(&mut totals, c, sign);
    flush(file, &mut totals)
}

fn remove(file: &CrawlFile, id: i64) -> Result<(), StoreError> {
    let sql = format!("{SELECT} WHERE id=?1").replace("main.urls", "temp.k03_rows");
    let old = file.conn().query_row(&sql, [id], contribution).optional()?;
    if let Some((_, old)) = old {
        add(file, &old, -1)?;
        file.conn()
            .execute("DELETE FROM temp.k03_rows WHERE id=?1", [id])?;
    }
    Ok(())
}

fn replace(file: &CrawlFile, id: i64, c: Contribution) -> Result<(), StoreError> {
    remove(file, id)?;
    add(file, &c, 1)?;
    file.conn().execute(
        &format!("INSERT INTO temp.k03_rows {SELECT} WHERE id=?1"),
        [id],
    )?;
    Ok(())
}

/// UI-145: the first build of a connection keeps only the aggregates. Per-URL raw facts are needed
/// solely to subtract old contributions; the first later change rebuilds once with them. Large
/// crawls are aggregated in disjoint ID ranges on further read connections of the same version.
fn cold_build(
    file: &CrawlFile,
    cancel: &os_model::CancelToken,
    with_rows: bool,
) -> Result<(), StoreError> {
    let interrupt = file.conn().get_interrupt_handle();
    let _registration = cancel.on_cancel(move || interrupt.interrupt());
    check_cancel(cancel)?;
    let mut totals = Totals::default();
    if with_rows {
        file.conn()
            .execute(&format!("INSERT INTO temp.k03_rows {SELECT}"), [])?;
        let sql = SELECT.replace("main.urls", "temp.k03_rows");
        scan_range(file.conn(), &sql, None, &mut totals, cancel)?;
    } else {
        file.conn()
            .execute("INSERT INTO temp.k03_rowless(id, stored) VALUES(1, 0)", [])?;
        match parallel_totals(file, cancel)? {
            Some(parallel) => {
                crate::views::parallel_pass_done();
                totals = parallel;
            }
            None => scan_range(file.conn(), SELECT, None, &mut totals, cancel)?,
        }
    }
    flush(file, &mut totals)
}

fn scan_range(
    conn: &rusqlite::Connection,
    select: &str,
    range: Option<(i64, i64)>,
    totals: &mut Totals,
    cancel: &os_model::CancelToken,
) -> Result<(), StoreError> {
    let mut stmt = match range {
        Some(_) => conn.prepare(&format!("{select} WHERE id > ?1 AND id <= ?2"))?,
        None => conn.prepare(select)?,
    };
    let mut rows = match range {
        Some((after, last)) => stmt.query([after, last])?,
        None => stmt.query([])?,
    };
    let mut seen = 0u32;
    while let Some(row) = rows.next()? {
        seen = seen.wrapping_add(1);
        if seen.is_multiple_of(1024) {
            check_cancel(cancel)?;
        }
        let (_, c) = contribution(row)?;
        accumulate(totals, &c, 1);
    }
    check_cancel(cancel)
}

/// `None` for small crawls or when another connection sees a different data version.
fn parallel_totals(
    file: &CrawlFile,
    cancel: &os_model::CancelToken,
) -> Result<Option<Totals>, StoreError> {
    let max_id: i64 =
        file.conn()
            .query_row("SELECT COALESCE(MAX(id), 0) FROM main.urls", [], |r| {
                r.get(0)
            })?;
    let workers = crate::views::parallel_workers(max_id, 4);
    if workers < 2 {
        return Ok(None);
    }
    let seq = os_store::queries::change_seq(file.conn())?;
    let reset = file.get_meta("reset_seq")?.unwrap_or_default();
    let step = max_id / workers as i64 + 1;
    let path = file.path();
    let parts: Vec<Result<Option<Totals>, StoreError>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers as i64)
            .map(|i| {
                let reset = &reset;
                scope.spawn(move || -> Result<Option<Totals>, StoreError> {
                    let after = if i == 0 { i64::MIN } else { i * step };
                    let last = if i + 1 == workers as i64 {
                        i64::MAX
                    } else {
                        (i + 1) * step
                    };
                    let worker = CrawlFile::open(path, os_store::OpenMode::ReadOnly)?;
                    let interrupt = worker.conn().get_interrupt_handle();
                    let _registration = cancel.on_cancel(move || interrupt.interrupt());
                    let _snapshot = worker.conn().unchecked_transaction()?;
                    if os_store::queries::change_seq(worker.conn())? != seq
                        || worker.get_meta("reset_seq")?.unwrap_or_default() != *reset
                    {
                        return Ok(None);
                    }
                    let mut totals = Totals::default();
                    scan_range(
                        worker.conn(),
                        SELECT,
                        Some((after, last)),
                        &mut totals,
                        cancel,
                    )?;
                    Ok(Some(totals))
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| {
                h.join().unwrap_or_else(|_| {
                    Err(StoreError::InvalidInput(
                        "Aggregatdurchlauf abgebrochen".into(),
                    ))
                })
            })
            .collect()
    });
    let mut merged = Totals::default();
    for part in parts {
        match part? {
            Some(part) => merged.merge(part),
            None => return Ok(None),
        }
    }
    Ok(Some(merged))
}

fn rowless(file: &CrawlFile) -> Result<bool, StoreError> {
    Ok(file
        .conn()
        .query_row("SELECT EXISTS(SELECT 1 FROM temp.k03_rowless)", [], |r| {
            r.get(0)
        })?)
}

/// Drops raw facts and aggregates; the caller rebuilds and writes the watermark.
fn clear(file: &CrawlFile) -> Result<(), StoreError> {
    file.conn().execute_batch(
        "DELETE FROM temp.k03_rows; DELETE FROM temp.k03_counts; DELETE FROM temp.k03_rowless;",
    )?;
    Ok(())
}

const CACHE_KEY: &str = "n21:sidebar";
type StoredRow = (String, String, String, String, [i64; 9]);
fn params_hash() -> String {
    crate::cursor::query_hash(&["sidebar-v1", env!("CARGO_PKG_VERSION"), SELECT])
}

/// K-1a: aggregates persisted for exactly this data version replace the cold build
/// of a reopened saved crawl. Raw facts follow with the first change.
fn restore(file: &CrawlFile, seq: u64, reset: &str) -> Result<bool, StoreError> {
    let Some(rows) = os_store::view_cache::load(file, CACHE_KEY, &params_hash())?
        .filter(|e| e.built_seq == seq && e.step_stamp == reset)
        .and_then(|e| serde_json::from_slice::<Vec<StoredRow>>(&e.data).ok())
    else {
        return Ok(false);
    };
    let mut stmt = file.conn().prepare_cached(
        "INSERT INTO temp.k03_counts(kind,key,parent,name,n,idx,nonidx,s2,s3,s4,s5,blocked,noresp)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
    )?;
    for (kind, key, parent, name, n) in rows {
        stmt.execute(params![
            kind, key, parent, name, n[0], n[1], n[2], n[3], n[4], n[5], n[6], n[7], n[8]
        ])?;
    }
    file.conn()
        .execute("INSERT INTO temp.k03_rowless(id, stored) VALUES(1, 1)", [])?;
    file.conn().execute(
        "INSERT OR REPLACE INTO temp.k03_meta(id,seq,reset) VALUES(1,?1,?2)",
        params![seq, reset],
    )?;
    Ok(true)
}

/// Aggregates of a cold build, ready for a writer (see [`persist`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Export {
    pub seq: u64,
    pub reset: String,
    pub data: Vec<u8>,
}

/// Takes the aggregates of a cold build on this connection once; `None` if they came from
/// `view_cache`, were taken already or follow live changes.
pub fn take_export(file: &CrawlFile) -> Result<Option<Export>, StoreError> {
    init(file)?;
    let pending: bool = file.conn().query_row(
        "SELECT EXISTS(SELECT 1 FROM temp.k03_rowless WHERE stored=0)",
        [],
        |r| r.get(0),
    )?;
    let Some((seq, reset)) = file
        .conn()
        .query_row("SELECT seq,reset FROM temp.k03_meta WHERE id=1", [], |r| {
            Ok((r.get::<_, u64>(0)?, r.get::<_, String>(1)?))
        })
        .optional()?
        .filter(|_| pending)
    else {
        return Ok(None);
    };
    let mut stmt = file.conn().prepare(
        "SELECT kind,key,parent,name,n,idx,nonidx,s2,s3,s4,s5,blocked,noresp FROM temp.k03_counts ORDER BY kind,key",
    )?;
    let rows = stmt
        .query_map([], |r| {
            let mut n = [0i64; 9];
            for (i, v) in n.iter_mut().enumerate() {
                *v = r.get(4 + i)?;
            }
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, n))
        })?
        .collect::<Result<Vec<StoredRow>, _>>()?;
    let data = serde_json::to_vec(&rows).map_err(|e| StoreError::InvalidInput(e.to_string()))?;
    file.conn()
        .execute("UPDATE temp.k03_rowless SET stored=1", [])?;
    Ok(Some(Export { seq, reset, data }))
}

/// Stores an [`Export`] with a writable connection; read-only and running crawls store nothing.
pub fn persist(file: &CrawlFile, export: &Export) -> Result<bool, StoreError> {
    let saved = os_store::view_cache::save(
        file,
        CACHE_KEY,
        &params_hash(),
        &os_store::view_cache::Entry {
            step_stamp: export.reset.clone(),
            built_seq: export.seq,
            data: export.data.clone(),
        },
    )?;
    if saved {
        crate::views::note_cache_write(file.path());
    }
    Ok(saved)
}

fn check_cancel(cancel: &os_model::CancelToken) -> Result<(), StoreError> {
    if cancel.is_cancelled() {
        return Err(StoreError::Sqlite(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_INTERRUPT),
            None,
        )));
    }
    Ok(())
}

/// Bringt den Cache innerhalb eines konsistenten Snapshots auf den Feed-Stand.
/// Der Erstaufbau streamt Zeilen; laufende Aktualisierungen lesen nur den Index
/// `urls_change_seq`. Löschungen werden bei geändertem `reset_seq` abgeglichen.
pub fn refresh(file: &CrawlFile, cfg: &CrawlConfig) -> Result<(), StoreError> {
    refresh_with_cancel(file, cfg, &os_model::CancelToken::new())
}

/// Abbrechbarer Erstaufbau und Feed-Nachlauf; auch die Rust-Schleife prüft die
/// Marke, damit sehr viele kurze SQLite-Anweisungen den Interrupt nicht umgehen.
pub fn refresh_with_cancel(
    file: &CrawlFile,
    _cfg: &CrawlConfig,
    cancel: &os_model::CancelToken,
) -> Result<(), StoreError> {
    check_cancel(cancel)?;
    init(file)?;
    let tx = file.conn().unchecked_transaction()?;
    let seq = os_store::queries::change_seq(file.conn())?;
    let reset = file.get_meta("reset_seq")?.unwrap_or_default();
    let previous: Option<(u64, String)> = file
        .conn()
        .query_row("SELECT seq,reset FROM temp.k03_meta WHERE id=1", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()?;
    if previous
        .as_ref()
        .is_some_and(|(old, old_reset)| *old == seq && *old_reset == reset)
    {
        return Ok(());
    }
    let rowless = rowless(file)?;
    let mut previous = previous;
    if previous.as_ref().is_some_and(|(old, _)| *old > seq) || rowless {
        clear(file)?;
        previous = None;
    }
    if previous.is_none() && !rowless && restore(file, seq, &reset)? {
        tx.commit()?;
        return Ok(());
    }
    let since = previous
        .as_ref()
        .filter(|(old, _)| *old <= seq)
        .map(|(old, _)| *old);
    if previous.as_ref().is_some_and(|(_, old)| *old != reset) {
        let mut after = i64::MIN;
        loop {
            // Kein Löschen aus einer Tabelle mit noch offenem Zeilencursor.
            let mut stmt = file.conn().prepare("SELECT id FROM temp.k03_rows c WHERE id>?1 AND NOT EXISTS(SELECT 1 FROM main.urls u WHERE u.id=c.id) ORDER BY id LIMIT 4096")?;
            let ids = stmt
                .query_map([after], |r| r.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            if ids.is_empty() {
                break;
            }
            after = *ids.last().expect("nicht leer");
            for id in ids {
                check_cancel(cancel)?;
                remove(file, id)?;
            }
        }
    }
    if let Some(since) = since {
        let mut stmt = file
            .conn()
            .prepare(&format!("{SELECT} WHERE change_seq > ?1"))?;
        let mut rows = stmt.query([since])?;
        while let Some(row) = rows.next()? {
            check_cancel(cancel)?;
            let (id, c) = contribution(row)?;
            replace(file, id, c)?;
        }
    } else {
        cold_build(file, cancel, rowless)?;
    }
    file.conn().execute(
        "INSERT OR REPLACE INTO temp.k03_meta(id,seq,reset) VALUES(1,?1,?2)",
        params![seq, reset],
    )?;
    tx.commit()?;
    Ok(())
}

/// F05-Vertrag: führt genau die genannten Änderungen einschließlich Löschungen
/// aus. Wiederholungen sind idempotent. Der Feed-Wasserstand bleibt unverändert,
/// damit nicht genannte Änderungen beim nächsten `refresh` weiter eintreffen.
pub fn apply_changes(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    ids: &[UrlId],
) -> Result<(), crate::AnalyzeError> {
    init(file)?;
    let initialized: bool =
        file.conn()
            .query_row("SELECT EXISTS(SELECT 1 FROM temp.k03_meta)", [], |r| {
                r.get(0)
            })?;
    if !initialized {
        refresh(file, cfg)?;
        return Ok(());
    }
    if rowless(file)? {
        // Without raw facts the named changes cannot be subtracted: rebuild the
        // current state once and keep raw facts from now on. The feed watermark
        // stays; replaying later changes is idempotent.
        let tx = file.conn().unchecked_transaction()?;
        clear(file)?;
        cold_build(file, &os_model::CancelToken::new(), true)?;
        tx.commit().map_err(StoreError::from)?;
        return Ok(());
    }
    let tx = file.conn().unchecked_transaction()?;
    let mut stmt = file
        .conn()
        .prepare_cached(&format!("{SELECT} WHERE id=?1"))?;
    for id in ids {
        match stmt.query_row([id.0], contribution).optional()? {
            Some((id, c)) => replace(file, id, c)?,
            None => remove(file, id.0)?,
        }
    }
    tx.commit().map_err(StoreError::from)?;
    Ok(())
}
