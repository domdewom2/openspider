//! CRAWL-134: one persisted, exclusive assignment shared by every consumer.
mod live;
mod rules;
mod stats;

use os_model::{CancelToken, CrawlConfig};
use os_store::CrawlFile;
use roaring::RoaringBitmap;

use crate::{error::ViewError, search::FilterSource, ColumnDef, ColumnKind};
pub use live::{observe, Observer};
pub use rules::{pending_conditions, validate, validate_file, Validation};
pub use stats::{statistics, SegmentStats};

#[derive(Debug, thiserror::Error)]
pub enum SegmentError {
    #[error("invalid segment rules: {0:?}")]
    Invalid(Vec<Validation>),
    #[error(transparent)]
    View(#[from] ViewError),
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error(transparent)]
    Store(#[from] os_store::StoreError),
    #[error("segment operation cancelled")]
    Cancelled,
    #[error("segment evaluation exceeded its time or regex step budget: {0}")]
    Evaluation(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
    pub segments: Vec<RoaringBitmap>,
    pub unassigned: RoaringBitmap,
    /// Conditions waiting for analysis or API values (segment/group/condition paths).
    pub pending: Vec<String>,
}

/// Preview does not write. References address earlier rule matches, before exclusive priority.
pub fn preview(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    cancel: &CancelToken,
) -> Result<Assignment, SegmentError> {
    rules::evaluate(file, cfg, cancel)
}

/// Atomic replacement; cancellation or invalid rules preserve the previous assignment.
pub fn reassign(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    cancel: &CancelToken,
) -> Result<Assignment, SegmentError> {
    assign(file, cfg, cancel, None)
}
/// Incremental live assignment for one bounded change-feed batch.
/// Configuration changes automatically fall back to a complete replacement.
pub fn reassign_urls(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    ids: &[os_model::UrlId],
    cancel: &CancelToken,
) -> Result<Assignment, SegmentError> {
    let stored = saved_config(file)?;
    if stored.segments != cfg.segments || file.get_meta("segments.definitions")?.is_none() {
        return reassign(file, cfg, cancel);
    }
    assign(file, cfg, cancel, Some(ids))
}
fn assign(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    cancel: &CancelToken,
    ids: Option<&[os_model::UrlId]>,
) -> Result<Assignment, SegmentError> {
    let tx = AssignmentTransaction::start(file)?;
    let assignment = rules::evaluate_subset(file, cfg, cancel, ids)?;
    if let Some(ids) = ids {
        for id in ids {
            file.conn()
                .execute("DELETE FROM url_segments WHERE url_id=?1", [id.0])?;
        }
    } else {
        file.conn().execute("DELETE FROM url_segments", [])?;
    }
    {
        let mut stmt = file
            .conn()
            .prepare("INSERT INTO url_segments(url_id,segment_id) VALUES(?1,?2)")?;
        for (index, bitmap) in assignment.segments.iter().enumerate() {
            for id in bitmap {
                if cancel.is_cancelled() {
                    return Err(SegmentError::Cancelled);
                }
                stmt.execute((id, index))?;
            }
        }
    }
    file.set_meta(
        "segments.definitions",
        &serde_json::to_string(&cfg.segments).expect("segment configuration"),
    )?;
    // Changing rules invalidates views even when the URL facts themselves did not change.
    let seq = os_store::queries::change_seq(file.conn())?
        .checked_add(1)
        .ok_or_else(|| SegmentError::Evaluation("change sequence exhausted".into()))?;
    file.set_meta("change_seq", &seq.to_string())?;
    file.set_meta("reset_seq", &seq.to_string())?;
    file.set_meta("segments.assignment_seq", &seq.to_string())?;
    if cancel.is_cancelled() {
        return Err(SegmentError::Cancelled);
    }
    tx.commit()?;
    Ok(assignment)
}

pub fn saved_config(file: &CrawlFile) -> Result<CrawlConfig, SegmentError> {
    let mut cfg = file.config()?;
    if let Some(json) = file.get_meta("segments.definitions")? {
        cfg.segments =
            serde_json::from_str(&json).map_err(|e| SegmentError::Evaluation(e.to_string()))?;
    }
    Ok(cfg)
}

/// `None` is the explicit "No segment" filter; absence of this source means all URLs.
#[derive(Debug, Clone)]
pub struct SegmentFilter {
    pub name: Option<String>,
}
impl SegmentFilter {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
        }
    }
    pub fn unassigned() -> Self {
        Self { name: None }
    }
    pub fn population_sql(&self) -> String {
        match &self.name {
            None => "NOT EXISTS(SELECT 1 FROM url_segments sg WHERE sg.url_id=u.id)".into(),
            Some(name) => format!("EXISTS(SELECT 1 FROM url_segments sg, json_each((SELECT value FROM crawl_meta WHERE key='segments.definitions'),'$.list') d WHERE sg.url_id=u.id AND sg.segment_id=CAST(d.key AS INTEGER) AND json_extract(d.value,'$.name')='{}')", name.replace('\'', "''")),
        }
    }
}
impl FilterSource for SegmentFilter {
    fn key(&self) -> String {
        format!(
            "segment:{}",
            serde_json::to_string(&self.name).expect("name")
        )
    }
    fn bitmap(&self, file: &CrawlFile) -> Result<RoaringBitmap, ViewError> {
        let mut stmt = file.conn().prepare(&format!(
            "SELECT u.id FROM urls u WHERE u.is_internal=1 AND ({})",
            self.population_sql()
        ))?;
        let mut bitmap = RoaringBitmap::new();
        for id in stmt.query_map([], |r| r.get::<_, u32>(0))? {
            bitmap.insert(id?);
        }
        Ok(bitmap)
    }
}

/// Adds the persisted name column to URL tables, only when segments are configured.
pub fn columns(cfg: &CrawlConfig) -> Vec<ColumnDef> {
    if cfg.segments.list.is_empty() {
        return Vec::new();
    }
    vec![ColumnDef::new("Segment", "(SELECT json_extract(d.value,'$.name') FROM url_segments sg, json_each((SELECT value FROM crawl_meta WHERE key='segments.definitions'),'$.list') d WHERE sg.url_id=u.id AND sg.segment_id=CAST(d.key AS INTEGER) LIMIT 1)", ColumnKind::Text, true)]
}

/// A savepoint also protects callers that already own a transaction.
struct AssignmentTransaction<'a> {
    file: &'a CrawlFile,
    committed: bool,
}
impl<'a> AssignmentTransaction<'a> {
    fn start(file: &'a CrawlFile) -> Result<Self, SegmentError> {
        file.conn()
            .execute_batch("SAVEPOINT os_segment_assignment")?;
        Ok(Self {
            file,
            committed: false,
        })
    }
    fn commit(mut self) -> Result<(), SegmentError> {
        self.file
            .conn()
            .execute_batch("RELEASE os_segment_assignment")?;
        self.committed = true;
        Ok(())
    }
}
impl Drop for AssignmentTransaction<'_> {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self
                .file
                .conn()
                .execute_batch("ROLLBACK TO os_segment_assignment; RELEASE os_segment_assignment");
        }
    }
}

/// Portable configuration and the optional command representation (stable IDs and rule tree).
pub fn definition_snapshot(
    file: &CrawlFile,
) -> Result<(CrawlConfig, Option<String>), SegmentError> {
    Ok((saved_config(file)?, file.get_meta("segments.dto")?))
}
/// Store rules, command representation and assignment as one atomic change.
pub fn save(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    representation: &str,
    cancel: &CancelToken,
) -> Result<(), SegmentError> {
    let tx = AssignmentTransaction::start(file)?;
    reassign(file, cfg, cancel)?;
    file.set_config(cfg)?;
    file.set_meta("segments.dto", representation)?;
    tx.commit()
}
pub struct LiveSnapshot {
    pub fingerprint: String,
    pub sequence: u64,
    pub changed: Vec<os_model::UrlId>,
}
/// Cheap change detection; assignments themselves do not change this fingerprint.
pub fn live_snapshot(file: &CrawlFile, since: u64) -> Result<Option<LiveSnapshot>, SegmentError> {
    let cfg = file.config()?;
    if cfg.segments.list.is_empty() && file.get_meta("segments.definitions")?.is_none() {
        return Ok(None);
    }
    let mut states = file
        .conn()
        .prepare("SELECT step,state,finished_at,config_hash FROM analysis_steps ORDER BY step")?;
    let analysis = states
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut states = file
        .conn()
        .prepare("SELECT source,state,finished_at,urls_done FROM api_status ORDER BY source")?;
    let api = states
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, u64>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let fingerprint = serde_json::to_string(&(analysis, api, cfg.segments)).expect("fingerprint");
    let sequence = file
        .get_meta("change_seq")?
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let changed = os_store::queries::changed_since(file.conn(), since, 4097)?;
    Ok(Some(LiveSnapshot {
        fingerprint,
        sequence,
        changed,
    }))
}
