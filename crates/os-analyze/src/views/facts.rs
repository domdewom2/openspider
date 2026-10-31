//! Overview/Issues facts of one data version (UI-145, UI-147): the summary rows and all filter counts.
//!
//! Computing them reads the whole crawl once per tab, minutes for five million URLs. The pieces (summary
//! and the counts of each tab) are kept in memory per crawl file and are a derivable cache (K-1a): a
//! writer persists every finished piece in `view_cache`, keyed by `change_seq`, the analysis/reset stamp
//! and a signature of configuration and tab definition. Work already done therefore survives closing the
//! crawl, and reopening a saved crawl answers Overview and Issues from a few kilobytes instead of a crawl
//! scan. Read-only and running crawls keep the pieces in memory only (`view_cache::save`).
use super::{filter_cache_stamp, filter_counts_parallel, FilterCount};
use crate::{defs::TabDef, error::ViewError};
use os_model::CrawlConfig;
use os_store::{queries, view_cache, CrawlFile};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
};

const SUMMARY_KEY: &str = "n21:summary";
const TAB_KEY: &str = "n21:counts:";
/// Files whose latest facts stay in memory (a few kilobytes each).
const MAX_FILES: usize = 8;

/// Summary rows (`Total URLs Encountered`, …) and the counts of all filters of all tabs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OverviewFacts {
    /// Overview summary rows (label, count).
    pub summary: Vec<(String, u64)>,
    /// Counts of all filters of all tabs, in tab and filter order.
    pub counts: Vec<FilterCount>,
}

struct Piece<T> {
    value: Arc<T>,
    persisted: bool,
}

#[derive(Default)]
struct Entry {
    crawl_id: String,
    seq: u64,
    stamp: String,
    summary: Option<Piece<Vec<(String, u64)>>>,
    /// Tab id → (signature, counts).
    tabs: HashMap<String, (String, Piece<Vec<FilterCount>>)>,
}

type Memory = Mutex<Vec<(PathBuf, Entry)>>;
fn memory() -> &'static Memory {
    static MEMORY: OnceLock<Memory> = OnceLock::new();
    MEMORY.get_or_init(Default::default)
}

fn tab_signature(tab: &TabDef, cfg: &str) -> String {
    let mut parts: Vec<&str> = vec![
        "counts-v1",
        env!("CARGO_PKG_VERSION"),
        cfg,
        tab.id,
        tab.population,
        tab.hidden_population.unwrap_or(""),
    ];
    for filter in &tab.filters {
        parts.extend([filter.id, filter.predicate]);
    }
    crate::cursor::query_hash(&parts)
}
fn summary_signature() -> String {
    crate::cursor::query_hash(&["summary-v1", env!("CARGO_PKG_VERSION")])
}

struct Version {
    crawl_id: String,
    seq: u64,
    stamp: String,
}
fn version(file: &CrawlFile) -> Result<Version, ViewError> {
    Ok(Version {
        crawl_id: file.get_meta("crawl_id")?.unwrap_or_default(),
        seq: queries::change_seq(file.conn())?,
        stamp: filter_cache_stamp(file)?,
    })
}
fn stored<T: serde::de::DeserializeOwned>(
    file: &CrawlFile,
    key: &str,
    signature: &str,
    v: &Version,
) -> Result<Option<T>, ViewError> {
    Ok(view_cache::load(file, key, signature)?
        .filter(|e| e.built_seq == v.seq && e.step_stamp == v.stamp)
        .and_then(|e| serde_json::from_slice(&e.data).ok()))
}

/// Runs `f` on the memory entry of this file and data version, replacing an outdated entry.
fn with_entry<R>(file: &CrawlFile, v: &Version, f: impl FnOnce(&mut Entry) -> R) -> R {
    let mut memory = memory().lock().unwrap_or_else(|e| e.into_inner());
    let at = memory.iter().position(|(path, _)| path == file.path());
    let current = at.filter(|i| {
        let e = &memory[*i].1;
        e.crawl_id == v.crawl_id && e.seq == v.seq && e.stamp == v.stamp
    });
    let index = match current {
        Some(i) => i,
        None => {
            if let Some(i) = at {
                memory.remove(i);
            }
            if memory.len() >= MAX_FILES {
                memory.remove(0);
            }
            memory.push((
                file.path().to_path_buf(),
                Entry {
                    crawl_id: v.crawl_id.clone(),
                    seq: v.seq,
                    stamp: v.stamp.clone(),
                    ..Entry::default()
                },
            ));
            memory.len() - 1
        }
    };
    f(&mut memory[index].1)
}

/// Computes at most `max_tabs` missing tab pieces (and the summary) of the file's current data
/// version, preferring pieces already stored in `view_cache`. Returns `true` once all are present.
pub fn overview_progress(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    cancel: &os_model::CancelToken,
    max_tabs: usize,
) -> Result<bool, ViewError> {
    let conn = file.conn();
    let _snapshot = conn
        .is_autocommit()
        .then(|| conn.unchecked_transaction())
        .transpose()?;
    let v = version(file)?;
    let tabs = crate::tabs::configured_for_file(cfg, file);
    let cfg_json = serde_json::to_string(cfg).unwrap_or_default();
    let signatures: Vec<String> = tabs.iter().map(|t| tab_signature(t, &cfg_json)).collect();
    let (has_summary, missing) = with_entry(file, &v, |e| {
        let missing: Vec<usize> = (0..tabs.len())
            .filter(|i| {
                e.tabs
                    .get(tabs[*i].id)
                    .is_none_or(|(sig, _)| *sig != signatures[*i])
            })
            .collect();
        (e.summary.is_some(), missing)
    });
    if !has_summary {
        let (value, persisted) = match stored(file, SUMMARY_KEY, &summary_signature(), &v)? {
            Some(value) => (value, true),
            None => (
                os_store::queries::app_support::overview_summary(conn)?,
                false,
            ),
        };
        with_entry(file, &v, |e| {
            e.summary = Some(Piece {
                value: Arc::new(value),
                persisted,
            })
        });
    }
    let mut todo = Vec::new();
    for i in missing.iter().copied() {
        let key = format!("{TAB_KEY}{}", tabs[i].id);
        match stored::<Vec<FilterCount>>(file, &key, &signatures[i], &v)? {
            Some(value) => with_entry(file, &v, |e| {
                e.tabs.insert(
                    tabs[i].id.to_owned(),
                    (
                        signatures[i].clone(),
                        Piece {
                            value: Arc::new(value),
                            persisted: true,
                        },
                    ),
                );
            }),
            None => todo.push(i),
        }
    }
    let more = todo.len() > max_tabs;
    todo.truncate(max_tabs);
    if !todo.is_empty() {
        let counts = filter_counts_parallel(&tabs, &todo, file, cfg, cancel)?;
        with_entry(file, &v, |e| {
            for i in &todo {
                let tab = &tabs[*i];
                let value: Vec<FilterCount> =
                    counts.iter().filter(|c| c.tab == tab.id).cloned().collect();
                e.tabs.insert(
                    tab.id.to_owned(),
                    (
                        signatures[*i].clone(),
                        Piece {
                            value: Arc::new(value),
                            persisted: false,
                        },
                    ),
                );
            }
        });
    }
    Ok(!more)
}

/// Facts of the file's current data version: from memory, from `view_cache`, or computed now.
/// The second value is `true` while computed pieces still await [`persist`].
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
    while !overview_progress(file, cfg, cancel, usize::MAX)? {}
    let v = version(file)?;
    let tabs = crate::tabs::configured_for_file(cfg, file);
    Ok(with_entry(file, &v, |e| {
        let mut pending = !e.summary.as_ref().is_some_and(|s| s.persisted);
        let mut counts = Vec::new();
        for tab in &tabs {
            if let Some((_, piece)) = e.tabs.get(tab.id) {
                pending |= !piece.persisted;
                counts.extend(piece.value.iter().cloned());
            }
        }
        let summary = e
            .summary
            .as_ref()
            .map(|s| s.value.as_ref().clone())
            .unwrap_or_default();
        (Arc::new(OverviewFacts { summary, counts }), pending)
    }))
}

/// Writes all computed, not yet stored pieces of the file's current data version to `view_cache`.
/// Call with a writable connection outside a read transaction; returns whether anything was written.
pub fn persist(file: &CrawlFile) -> Result<bool, ViewError> {
    if file.conn().is_readonly("main")? {
        return Ok(false);
    }
    let v = version(file)?;
    // Collect under the lock, write without it.
    type Pending = Vec<(String, String, Vec<u8>)>;
    let pending: Pending = with_entry(file, &v, |e| {
        let mut out = Vec::new();
        if let Some(s) = e.summary.as_ref().filter(|s| !s.persisted) {
            if let Ok(data) = serde_json::to_vec(s.value.as_ref()) {
                out.push((SUMMARY_KEY.to_owned(), summary_signature(), data));
            }
        }
        for (tab, (signature, piece)) in &e.tabs {
            if !piece.persisted {
                if let Ok(data) = serde_json::to_vec(piece.value.as_ref()) {
                    out.push((format!("{TAB_KEY}{tab}"), signature.clone(), data));
                }
            }
        }
        out
    });
    let mut saved = Vec::new();
    for (key, signature, data) in pending {
        let entry = view_cache::Entry {
            step_stamp: v.stamp.clone(),
            built_seq: v.seq,
            data,
        };
        if view_cache::save(file, &key, &signature, &entry)? {
            saved.push(key);
        } else {
            break;
        }
    }
    let any = !saved.is_empty();
    if any {
        super::membership::note_cache_write(file.path());
    }
    with_entry(file, &v, |e| {
        for key in saved {
            if key == SUMMARY_KEY {
                if let Some(s) = e.summary.as_mut() {
                    s.persisted = true;
                }
            } else if let Some((_, piece)) = key
                .strip_prefix(TAB_KEY)
                .and_then(|tab| e.tabs.get_mut(tab))
            {
                piece.persisted = true;
            }
        }
    });
    Ok(any)
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
