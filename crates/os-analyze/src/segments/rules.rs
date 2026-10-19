use super::{Assignment, SegmentError};
use crate::{ColumnDef, ColumnKind, ColumnSql, Needs};
use fancy_regex::{Regex, RegexBuilder};
use os_model::{CancelToken, CrawlConfig, SegmentCondition, SegmentOp};
use os_store::CrawlFile;
use roaring::RoaringBitmap;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Validation {
    pub path: String,
    pub message: String,
    pub line: u32,
    pub position: u32,
}
fn error(path: String, message: impl Into<String>) -> Validation {
    Validation {
        path,
        message: message.into(),
        line: 1,
        position: 0,
    }
}
fn regex(pattern: &str) -> Result<Regex, SegmentError> {
    if pattern.len() > 65536 {
        return Err(SegmentError::Evaluation("pattern-too-long".into()));
    }
    RegexBuilder::new(pattern)
        .backtrack_limit(1_000_000)
        .build()
        .map_err(|e| SegmentError::Evaluation(e.to_string()))
}
fn columns(cfg: &CrawlConfig, file: Option<&CrawlFile>) -> Vec<ColumnDef> {
    let tabs = match file {
        Some(f) => crate::tabs::configured_for_file(cfg, f),
        None => crate::tabs::configured(cfg),
    };
    let mut seen = HashSet::new();
    tabs.into_iter()
        .flat_map(|t| t.columns)
        .filter(|c| seen.insert(c.key))
        .collect()
}
fn numeric(op: SegmentOp) -> bool {
    matches!(
        op,
        SegmentOp::GreaterThan
            | SegmentOp::LessThan
            | SegmentOp::GreaterOrEqual
            | SegmentOp::LessOrEqual
    )
}
struct ResolvedColumn<'a> {
    definition: &'a ColumnDef,
    sql: String,
    list: bool,
    kind: ColumnKind,
}
fn find<'a>(columns: &'a [ColumnDef], field: &str) -> Option<ResolvedColumn<'a>> {
    for col in columns {
        let whole = col.key.eq_ignore_ascii_case(field);
        let lists: Vec<(&str, &str, &str, ColumnKind)> = match &col.sql {
            ColumnSql::Expr(sql) if whole => {
                return Some(ResolvedColumn {
                    definition: col,
                    sql: sql.to_string(),
                    list: false,
                    kind: col.kind,
                })
            }
            ColumnSql::Expr(_) => continue,
            ColumnSql::Multi { base, list_sql } | ColumnSql::MultiCrawlWide { base, list_sql } => {
                vec![(base, "", list_sql, col.kind)]
            }
            ColumnSql::MultiPaired { base, parts } => parts
                .iter()
                .map(|p| (*base, p.suffix, p.list_sql, p.kind))
                .collect(),
        };
        for (base, suffix, sql, kind) in lists {
            let numbered = field
                .get(..base.len())
                .filter(|prefix| prefix.eq_ignore_ascii_case(base))
                .and_then(|_| field[base.len()..].strip_prefix(' '))
                .and_then(|tail| {
                    let (n, rest) = tail.split_once(' ').unwrap_or((tail, ""));
                    rest.eq_ignore_ascii_case(suffix)
                        .then(|| n.parse::<usize>().ok().filter(|n| *n > 0))
                        .flatten()
                });
            if !whole && numbered.is_none() {
                continue;
            }
            let sql = sql.replace("?1", "u.id");
            let sql = if whole {
                format!("(WITH vals(value) AS ({sql}) SELECT json_group_array(value) FROM vals)")
            } else {
                format!(
                    "(WITH vals(value) AS ({sql}) SELECT value FROM vals LIMIT 1 OFFSET {})",
                    numbered.expect("numbered") - 1
                )
            };
            return Some(ResolvedColumn {
                definition: col,
                sql,
                list: whole,
                kind,
            });
        }
    }
    None
}
pub fn validate(cfg: &CrawlConfig) -> Vec<Validation> {
    validate_in(cfg, &columns(cfg, None))
}
fn validate_in(cfg: &CrawlConfig, columns: &[ColumnDef]) -> Vec<Validation> {
    let mut errors = Vec::new();
    let mut names = HashSet::new();
    for (s, segment) in cfg.segments.list.iter().enumerate() {
        let path = format!("segments.{s}");
        if !(1..=64).contains(&segment.name.chars().count())
            || segment.name.trim().is_empty()
            || !names.insert(segment.name.to_lowercase())
        {
            errors.push(error(format!("{path}.name"), "name-length-or-duplicate"));
        }
        if segment.color.len() != 7
            || !segment.color.starts_with('#')
            || !segment.color[1..].bytes().all(|b| b.is_ascii_hexdigit())
        {
            errors.push(error(format!("{path}.color"), "expected-rrggbb"));
        }
        for (g, group) in segment.groups.iter().enumerate() {
            for (c, cond) in group.conditions.iter().enumerate() {
                let path = format!("{path}.groups.{g}.conditions.{c}");
                match cond.op {
                    SegmentOp::InSegment => {
                        if !cfg.segments.list[..s].iter().any(|p| p.name == cond.value) {
                            errors.push(error(path.clone(), "segment-reference-must-be-above"));
                        }
                    }
                    SegmentOp::InNoSegment => {}
                    SegmentOp::HasIssue | SegmentOp::HasNoIssue => {
                        if !crate::issues::catalog().iter().any(|i| i.id == cond.value) {
                            errors.push(error(path.clone(), "unknown-issue"));
                        }
                    }
                    _ => match find(columns, &cond.field) {
                        None => errors.push(error(path.clone(), "unknown-column")),
                        Some(col)
                            if numeric(cond.op)
                                && (!matches!(
                                    col.kind,
                                    ColumnKind::Int
                                        | ColumnKind::Float { .. }
                                        | ColumnKind::Seconds3
                                ) || !cond.value.parse::<f64>().is_ok_and(f64::is_finite)) =>
                        {
                            errors.push(error(
                                path.clone(),
                                "expected-numeric-column-and-finite-value",
                            ));
                        }
                        _ => {}
                    },
                }
                if matches!(
                    cond.op,
                    SegmentOp::RegexMatches | SegmentOp::RegexNotMatches
                ) {
                    if let Err(e) = regex(&cond.value) {
                        let position = match Regex::new(&cond.value).err() {
                            Some(fancy_regex::Error::ParseError(pos, _)) => pos as u32,
                            _ => 0,
                        };
                        let mut issue = error(path, e.to_string());
                        issue.position = position;
                        errors.push(issue);
                    }
                }
            }
        }
    }
    errors
}
fn check(cancel: &CancelToken, deadline: Instant) -> Result<(), SegmentError> {
    if cancel.is_cancelled() {
        return Err(SegmentError::Cancelled);
    }
    if Instant::now() > deadline {
        return Err(SegmentError::Evaluation("deadline".into()));
    }
    Ok(())
}
fn text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        _ => v.to_string(),
    }
}
fn matches(
    v: &serde_json::Value,
    cond: &SegmentCondition,
    re: Option<&Regex>,
) -> Result<bool, SegmentError> {
    let s = text(v);
    let empty = v.is_null() || s.is_empty();
    let value = &cond.value;
    Ok(match cond.op {
        SegmentOp::Contains => s.contains(value),
        SegmentOp::NotContains => !s.contains(value),
        SegmentOp::Equals => s == *value,
        SegmentOp::NotEquals => s != *value,
        SegmentOp::StartsWith => s.starts_with(value),
        SegmentOp::EndsWith => s.ends_with(value),
        SegmentOp::RegexMatches | SegmentOp::RegexNotMatches => {
            let yes = re
                .expect("compiled regex")
                .is_match(&s)
                .map_err(|e| SegmentError::Evaluation(e.to_string()))?;
            if cond.op == SegmentOp::RegexMatches {
                yes
            } else {
                !yes
            }
        }
        SegmentOp::IsEmpty => empty,
        SegmentOp::IsNotEmpty => !empty,
        op if numeric(op) => {
            if empty {
                false
            } else {
                let a = s
                    .parse::<f64>()
                    .map_err(|e| SegmentError::Evaluation(e.to_string()))?;
                let b = value.parse::<f64>().expect("validated number");
                match op {
                    SegmentOp::GreaterThan => a > b,
                    SegmentOp::LessThan => a < b,
                    SegmentOp::GreaterOrEqual => a >= b,
                    _ => a <= b,
                }
            }
        }
        _ => false,
    })
}
fn column_pending(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    col: &ColumnDef,
) -> Result<bool, SegmentError> {
    let needs = col.needs.or_else(|| {
        (col.key == "Link Score").then_some(Needs::CrawlAnalysis(os_model::AnalysisStep::LinkScore))
    });
    Ok(match needs {
        Some(Needs::CrawlAnalysis(step)) => {
            let status = crate::aggregates::status::analysis_status(file, cfg);
            status.error.is_some()
                || status.stale.contains(&step)
                || !status
                    .steps
                    .iter()
                    .any(|(s, state)| *s == step && *state == os_model::StepState::Done)
        }
        Some(Needs::Api(source)) => !file
            .conn()
            .query_row(
                "SELECT state IN ('completed','done') OR (state='idle' AND urls_total>0 AND urls_done+urls_failed>=urls_total) FROM api_status WHERE source=?1",
                [source],
                |r| r.get::<_, bool>(0),
            )
            .optional()?
            .unwrap_or(false),
        _ => false,
    })
}
pub fn pending_conditions(
    file: &CrawlFile,
    cfg: &CrawlConfig,
) -> Result<Vec<String>, SegmentError> {
    let columns = columns(cfg, Some(file));
    let mut pending = Vec::new();
    for (s, segment) in cfg.segments.list.iter().enumerate() {
        for (g, group) in segment.groups.iter().enumerate() {
            for (c, cond) in group.conditions.iter().enumerate() {
                if let Some(col) = find(&columns, &cond.field) {
                    if column_pending(file, cfg, col.definition)? {
                        pending.push(format!("segments.{s}.groups.{g}.conditions.{c}"));
                    }
                }
            }
        }
    }
    Ok(pending)
}
pub fn validate_file(file: &CrawlFile, cfg: &CrawlConfig) -> Vec<Validation> {
    validate_in(cfg, &columns(cfg, Some(file)))
}

#[allow(clippy::too_many_arguments)]
fn atom(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    columns: &[ColumnDef],
    cond: &SegmentCondition,
    universe: &RoaringBitmap,
    prior: &[RoaringBitmap],
    cancel: &CancelToken,
    deadline: Instant,
) -> Result<(RoaringBitmap, bool), SegmentError> {
    match cond.op {
        SegmentOp::InSegment => {
            return Ok((
                prior[cfg
                    .segments
                    .list
                    .iter()
                    .position(|s| s.name == cond.value)
                    .expect("validated reference")]
                .clone(),
                false,
            ))
        }
        SegmentOp::InNoSegment => {
            let mut rest = universe.clone();
            for p in prior {
                rest -= p;
            }
            return Ok((rest, false));
        }
        SegmentOp::HasIssue | SegmentOp::HasNoIssue => {
            let entry = crate::issues::catalog()
                .iter()
                .find(|i| i.id == cond.value)
                .expect("validated issue");
            let tabs = crate::tabs::configured_for_file(cfg, file);
            let tab = tabs.iter().find(|t| t.id == entry.tab).expect("issue tab");
            let label = crate::issues::filter_label(entry, &(tab.params)(cfg));
            if crate::views::state_of_in(&tabs, file, cfg, &entry.tab, &label)?
                != os_model::FilterState::Ready
            {
                return Ok((RoaringBitmap::new(), true));
            }
            let mut query = crate::views::ViewQuery::new(&entry.tab, label);
            query.limit = 500;
            let mut bitmap = RoaringBitmap::new();
            loop {
                check(cancel, deadline)?;
                let page = crate::views::query_view(file, cfg, &query)?;
                for id in page.row_keys {
                    bitmap.insert(
                        u32::try_from(id.0).map_err(|e| SegmentError::Evaluation(e.to_string()))?,
                    );
                }
                if page.next_cursor.is_none() {
                    break;
                }
                query.start = crate::views::PageStart::Cursor(page.next_cursor.expect("cursor"));
            }
            if cond.op == SegmentOp::HasNoIssue {
                bitmap = universe - &bitmap;
            }
            return Ok((bitmap, false));
        }
        _ => {}
    }
    let col = find(columns, &cond.field).expect("validated column");
    let pending = column_pending(file, cfg, col.definition)?;
    if pending {
        return Ok((RoaringBitmap::new(), true));
    }
    let sql = &col.sql;
    let list = col.list;
    let restriction = if universe.len() <= 8192 {
        format!(
            "u.id IN ({})",
            universe
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",")
        )
    } else {
        "1".into()
    };
    let re = matches!(
        cond.op,
        SegmentOp::RegexMatches | SegmentOp::RegexNotMatches
    )
    .then(|| regex(&cond.value))
    .transpose()?;
    let mut stmt = file.conn().prepare(&format!(
        "SELECT u.id,({sql}) FROM {} WHERE u.is_internal=1 AND ({restriction})",
        crate::views::URLS_FROM
    ))?;
    let mut rows = stmt.query([])?;
    let mut bitmap = RoaringBitmap::new();
    while let Some(row) = rows.next()? {
        check(cancel, deadline)?;
        let v = match row.get_ref(1)? {
            rusqlite::types::ValueRef::Null => serde_json::Value::Null,
            rusqlite::types::ValueRef::Integer(v) => v.into(),
            rusqlite::types::ValueRef::Real(v) => serde_json::json!(v),
            rusqlite::types::ValueRef::Text(v) => String::from_utf8_lossy(v).into_owned().into(),
            _ => serde_json::Value::Null,
        };
        let values = if list {
            serde_json::from_str::<Vec<serde_json::Value>>(&text(&v))
                .map_err(|e| SegmentError::Evaluation(e.to_string()))?
        } else {
            vec![v]
        };
        let mut yes = false;
        for value in &values {
            let adjusted = if col.kind == ColumnKind::Seconds3 {
                value
                    .as_f64()
                    .map(|n| serde_json::json!((n / 1000.0 * 1000.0).round() / 1000.0))
            } else {
                None
            };
            let value = adjusted.as_ref().unwrap_or(value);
            let number_equal = matches!(
                col.kind,
                ColumnKind::Int | ColumnKind::Float { .. } | ColumnKind::Seconds3
            ) && matches!(cond.op, SegmentOp::Equals | SegmentOp::NotEquals);
            let scalar_matches = if number_equal {
                value
                    .as_f64()
                    .zip(cond.value.parse::<f64>().ok())
                    .is_some_and(|(a, b)| {
                        if cond.op == SegmentOp::Equals {
                            a == b
                        } else {
                            a != b
                        }
                    })
            } else {
                matches(value, cond, re.as_ref())?
            };
            if scalar_matches {
                yes = true;
                break;
            }
        }
        if values.is_empty() {
            yes = matches(&serde_json::Value::Null, cond, re.as_ref())?;
        }
        if yes {
            bitmap.insert(row.get(0)?);
        }
    }
    Ok((bitmap, false))
}
pub(super) fn evaluate(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    cancel: &CancelToken,
) -> Result<Assignment, SegmentError> {
    evaluate_subset(file, cfg, cancel, None)
}
pub(super) fn evaluate_subset(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    cancel: &CancelToken,
    ids: Option<&[os_model::UrlId]>,
) -> Result<Assignment, SegmentError> {
    let columns = columns(cfg, Some(file));
    let errors = validate_in(cfg, &columns);
    if !errors.is_empty() {
        return Err(SegmentError::Invalid(errors));
    }
    let deadline = Instant::now() + Duration::from_secs(60);
    check(cancel, deadline)?;
    let predicate = ids
        .map(|ids| {
            format!(
                " AND id IN ({})",
                ids.iter()
                    .map(|id| id.0.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .unwrap_or_default();
    let mut stmt = file.conn().prepare(&format!(
        "SELECT id FROM urls WHERE is_internal=1{predicate}"
    ))?;
    let mut universe = RoaringBitmap::new();
    for id in stmt.query_map([], |r| r.get::<_, u32>(0))? {
        universe.insert(id?);
    }
    let mut assigned = RoaringBitmap::new();
    let mut prior = Vec::new();
    let mut segments = Vec::new();
    let mut pending = Vec::new();
    for (s, segment) in cfg.segments.list.iter().enumerate() {
        let mut matched = RoaringBitmap::new();
        for (g, group) in segment.groups.iter().enumerate() {
            let mut matches = universe.clone();
            for (c, cond) in group.conditions.iter().enumerate() {
                let (bitmap, waiting) = atom(
                    file, cfg, &columns, cond, &universe, &prior, cancel, deadline,
                )?;
                if waiting {
                    pending.push(format!("segments.{s}.groups.{g}.conditions.{c}"));
                }
                matches &= bitmap;
            }
            matched |= matches;
        }
        let exclusive = &matched - &assigned;
        assigned |= &exclusive;
        prior.push(matched);
        segments.push(exclusive);
    }
    Ok(Assignment {
        segments,
        unassigned: universe - assigned,
        pending,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_model::{IdSource, SeededIds, Timestamp};
    // spec: CRAWL-134
    #[test]
    fn analysis_and_api_conditions_wait_for_completed_fresh_results() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = CrawlConfig::default();
        let file = CrawlFile::create(
            &dir.path().join("pending.ospider"),
            &os_store::CrawlInit {
                config: cfg.clone(),
                crawl_id: SeededIds::new(14).new_crawl_id(Timestamp::UNIX_EPOCH),
                mode: os_model::CrawlMode::Spider,
                start_url: None,
                list_source: None,
                project: "fixture".into(),
                name: "pending".into(),
                created_at: Timestamp::UNIX_EPOCH,
            },
            os_store::StorageProfile::Balanced,
        )
        .unwrap();
        let api = ColumnDef::new(
            "API Metric",
            "NULL",
            ColumnKind::Float { decimals: 2 },
            true,
        )
        .with_needs(Needs::Api("gsc"));
        assert!(column_pending(&file, &cfg, &api).unwrap());
        file.conn()
            .execute_batch("INSERT INTO api_status(source,state) VALUES('gsc','running')")
            .unwrap();
        assert!(column_pending(&file, &cfg, &api).unwrap());
        file.conn()
            .execute_batch("UPDATE api_status SET state='completed'")
            .unwrap();
        assert!(!column_pending(&file, &cfg, &api).unwrap());
        let score = ColumnDef::new("Link Score", "NULL", ColumnKind::Int, true);
        assert!(column_pending(&file, &cfg, &score).unwrap());
        file.conn()
            .execute(
                "INSERT INTO analysis_steps(step,state,config_hash) VALUES(?1,'Done',?2)",
                (
                    os_model::AnalysisStep::LinkScore.as_str(),
                    crate::state::step_config_hash(os_model::AnalysisStep::LinkScore, &cfg),
                ),
            )
            .unwrap();
        // Stored StepState uses the enum's canonical wire spelling.
        file.conn()
            .execute(
                "UPDATE analysis_steps SET state=?1",
                [os_model::StepState::Done.as_str()],
            )
            .unwrap();
        assert!(!column_pending(&file, &cfg, &score).unwrap());
    }
}
