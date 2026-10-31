//! Overview/Issues facts of one data version (UI-145, UI-147): the summary rows and all filter counts.
//!
//! Computing them reads the whole crawl once per tab. The result is kept in memory per crawl file and
//! is a derivable cache (K-1a): a writer persists it in `view_cache`, keyed by `change_seq`, the
//! analysis/reset stamp and a signature of configuration and filter definitions. Reopening a saved
//! crawl then answers Overview and Issues from a few kilobytes instead of a crawl scan. Read-only and
//! running crawls keep the facts in memory only (`view_cache::save`).
use super::{filter_cache_stamp, filter_counts_parallel, FilterCount};
use crate::{defs::TabDef, error::ViewError};
use os_model::CrawlConfig;
use os_store::{queries, view_cache, CrawlFile};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
};

const CACHE_KEY: &str = "n21:overview";
/// Files whose latest facts stay in memory (a few kilobytes each).
const MAX_FILES: usize = 8;

/// Summary rows (`Total URLs Encountered`, …) and the counts of all filters of all tabs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OverviewFacts {
    pub summary: Vec<(String, u64)>,
    pub counts: Vec<FilterCount>,
}

struct Entry {
    crawl_id: String,
    seq: u64,
    stamp: String,
    signature: String,
    facts: Arc<OverviewFacts>,
    persisted: AtomicBool,
}

type Memory = Mutex<Vec<(PathBuf, Arc<Entry>)>>;
fn memory() -> &'static Memory {
    static MEMORY: OnceLock<Memory> = OnceLock::new();
    MEMORY.get_or_init(Default::default)
}

fn signature(tabs: &[TabDef], cfg: &CrawlConfig) -> String {
    let cfg = serde_json::to_string(cfg).unwrap_or_default();
    let mut parts: Vec<&str> = vec!["overview-v1", env!("CARGO_PKG_VERSION"), &cfg];
    for tab in tabs {
        parts.extend([tab.id, tab.population, tab.hidden_population.unwrap_or("")]);
        for filter in &tab.filters {
            parts.extend([filter.id, filter.predicate]);
        }
    }
    crate::cursor::query_hash(&parts)
}

/// Facts of the file's current data version: from memory, from `view_cache`, or computed once.
/// The second value is `true` while a computed result still awaits [`persist`].
pub fn overview_facts(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    cancel: &os_model::CancelToken,
) -> Result<(Arc<OverviewFacts>, bool), ViewError> {
    let conn = file.conn();
    let _snapshot = conn
        .is_autocommit()
        .then(|| conn.unchecked_transaction())
        .transpose()?;
    let tabs = crate::tabs::configured_for_file(cfg, file);
    let crawl_id = file.get_meta("crawl_id")?.unwrap_or_default();
    let seq = queries::change_seq(conn)?;
    let stamp = filter_cache_stamp(file)?;
    let signature = signature(&tabs, cfg);
    let current = |e: &Entry| {
        e.crawl_id == crawl_id && e.seq == seq && e.stamp == stamp && e.signature == signature
    };
    if let Some(entry) = memory()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|(path, e)| path == file.path() && current(e))
    {
        return Ok((
            entry.1.facts.clone(),
            !entry.1.persisted.load(Ordering::Relaxed),
        ));
    }
    let stored = view_cache::load(file, CACHE_KEY, &signature)?
        .filter(|e| e.built_seq == seq && e.step_stamp == stamp)
        .and_then(|e| serde_json::from_slice::<OverviewFacts>(&e.data).ok());
    let persisted = stored.is_some();
    let facts = match stored {
        Some(facts) => facts,
        None => OverviewFacts {
            summary: os_store::queries::app_support::overview_summary(conn)?,
            counts: filter_counts_parallel(&tabs, file, cfg, cancel)?,
        },
    };
    let facts = Arc::new(facts);
    let mut memory = memory().lock().unwrap_or_else(|e| e.into_inner());
    memory.retain(|(path, _)| path != file.path());
    memory.push((
        file.path().to_path_buf(),
        Arc::new(Entry {
            crawl_id,
            seq,
            stamp,
            signature,
            facts: facts.clone(),
            persisted: AtomicBool::new(persisted),
        }),
    ));
    if memory.len() > MAX_FILES {
        memory.remove(0);
    }
    Ok((facts, !persisted))
}

/// Writes the computed facts of the file's current data version to `view_cache`. Call with a writable
/// connection outside a read transaction; returns whether an entry was written.
pub fn persist(file: &CrawlFile) -> Result<bool, ViewError> {
    if file.conn().is_readonly("main")? {
        return Ok(false);
    }
    let crawl_id = file.get_meta("crawl_id")?.unwrap_or_default();
    let seq = queries::change_seq(file.conn())?;
    let stamp = filter_cache_stamp(file)?;
    let entry = memory()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|(path, e)| {
            path == file.path()
                && e.crawl_id == crawl_id
                && e.seq == seq
                && e.stamp == stamp
                && !e.persisted.load(Ordering::Relaxed)
        })
        .map(|(_, e)| e.clone());
    let Some(entry) = entry else {
        return Ok(false);
    };
    let data = serde_json::to_vec(entry.facts.as_ref())
        .map_err(|e| os_store::StoreError::InvalidInput(e.to_string()))?;
    let saved = view_cache::save(
        file,
        CACHE_KEY,
        &entry.signature,
        &view_cache::Entry {
            step_stamp: entry.stamp.clone(),
            built_seq: entry.seq,
            data,
        },
    )?;
    if saved {
        entry.persisted.store(true, Ordering::Relaxed);
    }
    Ok(saved)
}

/// Persists every derivable view fact of the current data version (memberships and Overview/Issues).
pub fn persist_all(file: &CrawlFile) -> Result<bool, ViewError> {
    let shared = super::session::persist_shared(file)?;
    Ok(persist(file)? | shared)
}

#[cfg(test)]
pub(super) fn forget(file: &CrawlFile) {
    memory()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|(path, _)| path != file.path());
}

#[cfg(test)]
#[path = "facts_n21_tests.rs"]
mod n21_tests;
