//! Common live memberships, shared by tab sessions without retaining cells (UI-141, UI-147).
//!
//! One pass over `urls` prepares the Internal, External and Response Codes populations, the simple
//! content-class filters of Internal/External and the ONPAGE-015 denominator. Large crawls split the
//! pass by ID range over a few additional read connections of the same data version. A finished pass
//! is a derivable cache (K-1a): a writer may persist it in `view_cache`, so reopening a saved crawl
//! reads a few bitmaps instead of the URL table. Read-only hosts keep it in memory only.
use super::{build, session::BuildControl};
use crate::error::ViewError;
use os_store::{queries, view_cache, CrawlFile, OpenMode, StoreError};
use roaring::{RoaringBitmap, RoaringTreemap};
use rusqlite::OptionalExtension;
use std::{
    collections::HashMap,
    io::{Cursor, Read},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock, Weak,
    },
};

/// Populations prepared by the shared pass: External, Internal, Response Codes.
const POPULATIONS: [&str; 3] = [
    "(u.is_internal = 0 AND u.in_tables = 1)",
    "(u.is_internal = 1 AND u.in_tables = 1)",
    "(u.in_tables = 1 AND u.status_code IS NOT NULL)",
];
const FACTS: &str = "SELECT id, is_internal, in_tables, content_class, status_code FROM urls";
const CACHE_KEY: &str = "n21:membership";

/// Immutable facts from one data version. Views own their lifetime; the registry
/// holds only weak references, so idle/budget release also releases these facts.
pub(super) struct Membership {
    seq: u64,
    stamp: String,
    html_ids: RoaringTreemap,
    all: [RoaringBitmap; 3],
    classes: [HashMap<String, RoaringBitmap>; 2],
    pub html_2xx: i64,
    persisted: AtomicBool,
}

impl Membership {
    fn empty(seq: u64, stamp: &str) -> Self {
        Self {
            seq,
            stamp: stamp.to_owned(),
            html_ids: RoaringTreemap::new(),
            all: Default::default(),
            classes: Default::default(),
            html_2xx: 0,
            persisted: AtomicBool::new(false),
        }
    }

    /// A repaired copy for a newer data version; it has not been persisted yet.
    fn derived(&self, seq: u64) -> Self {
        Self {
            seq,
            stamp: self.stamp.clone(),
            html_ids: self.html_ids.clone(),
            all: self.all.clone(),
            classes: self.classes.clone(),
            html_2xx: self.html_2xx,
            persisted: AtomicBool::new(false),
        }
    }

    pub fn bytes(&self) -> u64 {
        self.html_ids.serialized_size() as u64
            + self
                .all
                .iter()
                .chain(self.classes.iter().flat_map(|c| c.values()))
                .map(|b| b.serialized_size() as u64)
                .sum::<u64>()
    }

    pub fn bitmap(&self, built: &build::Built<'_>) -> Option<RoaringBitmap> {
        let scope = scope(built)?;
        if built.filter.predicate == "1" {
            Some(self.all[scope].clone())
        } else {
            Some(
                self.classes[scope]
                    .get(class(built.filter.predicate)?)
                    .cloned()
                    .unwrap_or_default(),
            )
        }
    }

    fn insert(&mut self, row: &rusqlite::Row<'_>) -> Result<(), ViewError> {
        let raw_id: i64 = row.get(0)?;
        let internal: i64 = row.get(1)?;
        let visible: i64 = row.get(2)?;
        let content: Option<&str> = row.get_ref(3)?.as_str_or_null().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let status: Option<i64> = row.get(4)?;
        // ONPAGE-015 denominator: internal HTML 2xx, including hidden pages.
        if internal == 1
            && content == Some("HTML")
            && status.is_some_and(|s| (200..=299).contains(&s))
            && self.html_ids.insert(raw_id as u64)
        {
            self.html_2xx += 1;
        }
        if visible != 1 {
            return Ok(());
        }
        let id = super::session::url_index(raw_id)?;
        if status.is_some() {
            self.all[2].insert(id);
        }
        if !(0..=1).contains(&internal) {
            return Ok(());
        }
        let scope = internal as usize;
        self.all[scope].insert(id);
        if let Some(content) = content {
            if let Some(bitmap) = self.classes[scope].get_mut(content) {
                bitmap.insert(id);
            } else {
                let mut bitmap = RoaringBitmap::new();
                bitmap.insert(id);
                self.classes[scope].insert(content.to_owned(), bitmap);
            }
        }
        Ok(())
    }

    /// Disjoint ID ranges of the same data version.
    fn merge(&mut self, other: Membership) {
        self.html_ids |= other.html_ids;
        self.html_2xx += other.html_2xx;
        for (mine, theirs) in self.all.iter_mut().zip(other.all) {
            *mine |= theirs;
        }
        for (mine, theirs) in self.classes.iter_mut().zip(other.classes) {
            for (class, bitmap) in theirs {
                *mine.entry(class).or_default() |= bitmap;
            }
        }
    }

    fn encode(&self) -> Result<Vec<u8>, ViewError> {
        let fail = |_| invalid("Mitgliedschaften nicht serialisierbar");
        let mut out = b"OSM1".to_vec();
        self.html_ids.serialize_into(&mut out).map_err(fail)?;
        for bitmap in &self.all {
            bitmap.serialize_into(&mut out).map_err(fail)?;
        }
        for classes in &self.classes {
            let mut names: Vec<_> = classes.keys().collect();
            names.sort();
            out.extend_from_slice(&(names.len() as u32).to_le_bytes());
            for name in names {
                out.extend_from_slice(&(name.len() as u32).to_le_bytes());
                out.extend_from_slice(name.as_bytes());
                classes[name].serialize_into(&mut out).map_err(fail)?;
            }
        }
        Ok(out)
    }

    /// `None` for any foreign or truncated blob: the cache is then rebuilt.
    fn decode(data: &[u8], seq: u64, stamp: &str) -> Option<Self> {
        fn u32_le(r: &mut Cursor<&[u8]>) -> Option<u32> {
            let mut b = [0; 4];
            r.read_exact(&mut b).ok()?;
            Some(u32::from_le_bytes(b))
        }
        let mut r = Cursor::new(data.strip_prefix(b"OSM1")?);
        let mut facts = Self::empty(seq, stamp);
        facts.html_ids = RoaringTreemap::deserialize_from(&mut r).ok()?;
        facts.html_2xx = i64::try_from(facts.html_ids.len()).ok()?;
        for bitmap in &mut facts.all {
            *bitmap = RoaringBitmap::deserialize_from(&mut r).ok()?;
        }
        for classes in &mut facts.classes {
            for _ in 0..u32_le(&mut r)? {
                let mut name = vec![0; u32_le(&mut r)? as usize];
                r.read_exact(&mut name).ok()?;
                let bitmap = RoaringBitmap::deserialize_from(&mut r).ok()?;
                classes.insert(String::from_utf8(name).ok()?, bitmap);
            }
        }
        (r.position() as usize == r.get_ref().len()).then_some(facts)
    }
}

fn invalid(message: &str) -> ViewError {
    StoreError::InvalidInput(message.into()).into()
}

fn class(predicate: &str) -> Option<&str> {
    predicate
        .strip_prefix("u.content_class = '")?
        .strip_suffix('\'')
        .filter(|s| !s.contains('\''))
}

fn scope(built: &build::Built<'_>) -> Option<usize> {
    if !built.filter.requires.is_empty()
        || built.filter.ignore_non_indexable
        || !matches!(built.filter.kind, crate::defs::FilterKind::Live)
        || (built.filter.predicate != "1" && class(built.filter.predicate).is_none())
    {
        return None;
    }
    let scope = POPULATIONS
        .iter()
        .position(|p| *p == built.plan.population_sql)?;
    // Response Codes filters are status rules, not content classes.
    (scope < 2 || built.filter.predicate == "1").then_some(scope)
}

pub(super) fn applicable(built: &build::Built<'_>) -> bool {
    scope(built).is_some()
}

type Registry = Mutex<HashMap<String, Weak<Membership>>>;
fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(Default::default)
}
fn cache_key(file: &CrawlFile, seq: u64, stamp: &str) -> Result<String, ViewError> {
    Ok(format!(
        "{:?}:{}:{seq}:{stamp}",
        file.path(),
        file.get_meta("crawl_id")?.unwrap_or_default()
    ))
}
fn params_hash() -> String {
    crate::cursor::query_hash(&[
        "membership-v1",
        env!("CARGO_PKG_VERSION"),
        FACTS,
        &POPULATIONS.join("|"),
    ])
}
fn remember(key: String, facts: &Arc<Membership>) {
    let mut entries = registry().lock().unwrap_or_else(|e| e.into_inner());
    entries.retain(|_, v| v.strong_count() > 0);
    entries.insert(key, Arc::downgrade(facts));
}

pub(super) fn load(
    file: &CrawlFile,
    seq: u64,
    stamp: &str,
    control: &BuildControl,
) -> Result<Arc<Membership>, ViewError> {
    let key = cache_key(file, seq, stamp)?;
    {
        let mut entries = registry().lock().unwrap_or_else(|e| e.into_inner());
        entries.retain(|_, v| v.strong_count() > 0);
        if let Some(facts) = entries.get(&key).and_then(Weak::upgrade) {
            return Ok(facts);
        }
    }
    // K-1a: a persisted pass of exactly this data version replaces the scan.
    let stored = view_cache::load(file, CACHE_KEY, &params_hash())?
        .filter(|e| e.built_seq == seq && e.step_stamp == stamp)
        .and_then(|e| Membership::decode(&e.data, seq, stamp));
    let facts = match stored {
        Some(facts) => {
            facts.persisted.store(true, Ordering::Relaxed);
            facts
        }
        None => scan(file, seq, stamp, control)?,
    };
    let facts = Arc::new(facts);
    remember(key, &facts);
    Ok(facts)
}

fn scan(
    file: &CrawlFile,
    seq: u64,
    stamp: &str,
    control: &BuildControl,
) -> Result<Membership, ViewError> {
    let max_id: i64 = file
        .conn()
        .query_row("SELECT COALESCE(MAX(id), 0) FROM urls", [], |r| r.get(0))?;
    // The user waits for this pass: all cores, at most four connections.
    let workers = super::parallel_workers(max_id, 1);
    if workers > 1 {
        if let Some(facts) = parallel_scan(file, seq, stamp, max_id, workers, control)? {
            super::parallel_pass_done();
            return Ok(facts);
        }
    }
    let mut facts = Membership::empty(seq, stamp);
    scan_range(file.conn(), i64::MIN, i64::MAX, &mut facts, control)?;
    Ok(facts)
}

fn scan_range(
    conn: &rusqlite::Connection,
    after: i64,
    last: i64,
    facts: &mut Membership,
    control: &BuildControl,
) -> Result<(), ViewError> {
    let mut stmt = conn.prepare(&format!("{FACTS} WHERE id > ?1 AND id <= ?2"))?;
    let mut rows = stmt.query([after, last])?;
    let mut seen = 0u32;
    while let Some(row) = rows.next()? {
        seen = seen.wrapping_add(1);
        if seen.is_multiple_of(1024) {
            control.check()?;
        }
        facts.insert(row)?;
    }
    control.check()
}

/// Each worker reads on its own connection and verifies the data version first. Any
/// difference (a concurrent writer batch) falls back to the caller's snapshot.
fn parallel_scan(
    file: &CrawlFile,
    seq: u64,
    stamp: &str,
    max_id: i64,
    workers: usize,
    control: &BuildControl,
) -> Result<Option<Membership>, ViewError> {
    let path = file.path();
    let step = max_id / workers as i64 + 1;
    let parts: Vec<Result<Option<Membership>, ViewError>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|i| {
                let after = if i == 0 { i64::MIN } else { i as i64 * step };
                let last = if i + 1 == workers {
                    i64::MAX
                } else {
                    (i as i64 + 1) * step
                };
                scope.spawn(move || -> Result<Option<Membership>, ViewError> {
                    let worker = CrawlFile::open(path, OpenMode::ReadOnly)?;
                    let _snapshot = worker.conn().unchecked_transaction()?;
                    if queries::change_seq(worker.conn())? != seq
                        || super::session::stamp(&worker)? != stamp
                    {
                        return Ok(None);
                    }
                    let mut facts = Membership::empty(seq, stamp);
                    scan_range(worker.conn(), after, last, &mut facts, control)?;
                    Ok(Some(facts))
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|_| Err(invalid("Mitgliedschaftsdurchlauf abgebrochen")))
            })
            .collect()
    });
    let mut merged = Membership::empty(seq, stamp);
    for part in parts {
        match part? {
            Some(part) => merged.merge(part),
            None => return Ok(None),
        }
    }
    Ok(Some(merged))
}

/// Persists the shared facts of the file's current data version, if a view prepared them and
/// they are not stored yet. Read-only and running crawls store nothing (`view_cache::save`).
pub(super) fn persist(file: &CrawlFile) -> Result<bool, ViewError> {
    let seq = queries::change_seq(file.conn())?;
    let stamp = super::session::stamp(file)?;
    let key = cache_key(file, seq, &stamp)?;
    let facts = registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&key)
        .and_then(Weak::upgrade);
    match facts {
        Some(facts) => save(file, &facts),
        None => Ok(false),
    }
}

pub(super) fn save(file: &CrawlFile, facts: &Membership) -> Result<bool, ViewError> {
    if facts.persisted.load(Ordering::Relaxed) || file.conn().is_readonly("main")? {
        return Ok(false);
    }
    let saved = view_cache::save(
        file,
        CACHE_KEY,
        &params_hash(),
        &view_cache::Entry {
            step_stamp: facts.stamp.clone(),
            built_seq: facts.seq,
            data: facts.encode()?,
        },
    )?;
    if saved {
        facts.persisted.store(true, Ordering::Relaxed);
    }
    Ok(saved)
}

/// The feed names every changed/deleted ID. Repair common facts before the next
/// materialization, preserving the incremental path under live crawl updates.
pub(super) fn repair(
    file: &CrawlFile,
    previous: &Membership,
    ids: &[os_model::UrlId],
    stamp: &str,
) -> Result<Option<Arc<Membership>>, ViewError> {
    let seq = queries::change_seq(file.conn())?;
    if previous.stamp != stamp || previous.seq > seq {
        return Ok(None);
    }
    let mut facts = previous.derived(seq);
    let mut stmt = file.conn().prepare(&format!("{FACTS} WHERE id=?1"))?;
    for id in ids {
        if facts.html_ids.remove(id.0 as u64) {
            facts.html_2xx -= 1;
        }
        if let Ok(id) = u32::try_from(id.0) {
            for bitmap in facts
                .all
                .iter_mut()
                .chain(facts.classes.iter_mut().flat_map(|c| c.values_mut()))
            {
                bitmap.remove(id);
            }
        }
        // query_row's closure preserves SQL errors; insertion can also reject a
        // visible ID outside the bitmap range, so keep its Result nested.
        if let Some(inserted) = stmt
            .query_row([id.0], |row| Ok(facts.insert(row)))
            .optional()?
        {
            inserted?;
        }
    }
    let facts = Arc::new(facts);
    remember(cache_key(file, seq, stamp)?, &facts);
    Ok(Some(facts))
}

pub(super) fn pending(facts: &Membership) -> bool {
    !facts.persisted.load(Ordering::Relaxed)
}
