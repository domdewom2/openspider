//! Cached permutations; only IDs and equal-value runs survive construction.
use super::super::{build::SortTerm, URLS_FROM};
use crate::{error::ViewError, search::SearchError};
use os_model::{CancelToken, UrlId};
use os_store::CrawlFile;
use rusqlite::{types::Value, OptionalExtension};
use std::cmp::Ordering;

pub(super) struct Permutation {
    ids: Vec<u32>,
    non_null: usize,
    ties: Vec<(u32, u32)>,
    terms: Vec<SortTerm>,
}
pub(super) fn signature(terms: &[SortTerm]) -> String {
    terms
        .iter()
        .map(|t| format!("{}:{}:{}", t.raw, t.collate, terms.len() > 1 && t.desc))
        .collect::<Vec<_>>()
        .join("|")
}
fn empty(v: &Value) -> bool {
    matches!(v, Value::Null) || matches!(v, Value::Text(s) if s.is_empty())
}
fn compare(a: &Value, b: &Value, collate: bool) -> Ordering {
    if empty(a) || empty(b) {
        return empty(a).cmp(&empty(b));
    }
    match (a, b) {
        (Value::Integer(a), Value::Integer(b)) => a.cmp(b),
        (Value::Real(a), Value::Real(b)) => a.total_cmp(b),
        (Value::Integer(a), Value::Real(b)) => (*a as f64).total_cmp(b),
        (Value::Real(a), Value::Integer(b)) => a.total_cmp(&(*b as f64)),
        (Value::Text(a), Value::Text(b)) if collate => os_store::collation::compare(a, b),
        (Value::Text(a), Value::Text(b)) => a.cmp(b),
        (Value::Blob(a), Value::Blob(b)) => a.cmp(b),
        _ => rank(a).cmp(&rank(b)),
    }
}
fn rank(v: &Value) -> u8 {
    match v {
        Value::Null => 0,
        Value::Integer(_) | Value::Real(_) => 1,
        Value::Text(_) => 2,
        Value::Blob(_) => 3,
    }
}
impl Permutation {
    pub fn build(
        file: &CrawlFile,
        terms: &[SortTerm],
        cancel: &CancelToken,
    ) -> Result<Self, ViewError> {
        if terms.len() == 1 && terms[0].collate {
            return Self::build_text(file, &terms[0], cancel);
        }
        let terms: Vec<_> = terms
            .iter()
            .cloned()
            .map(|mut t| {
                if terms.len() == 1 {
                    t.desc = false;
                }
                t
            })
            .collect();
        let select = terms
            .iter()
            .map(|t| t.raw.as_str())
            .collect::<Vec<_>>()
            .join(",");
        // Do not carry Address through SQLite's sorter. Fetch it only for actual ties.
        let order = if terms.len() == 1 {
            format!(
                "{}{} ASC",
                terms[0].raw,
                if terms[0].collate {
                    " COLLATE OS_TEXT"
                } else {
                    ""
                }
            )
        } else {
            terms
                .iter()
                .map(|t| {
                    format!(
                        "{} ASC, {} {}",
                        t.empty_expr(),
                        t.value_expr(),
                        if t.desc { "DESC" } else { "ASC" }
                    )
                })
                .collect::<Vec<_>>()
                .join(",")
        };
        let from = if select.contains("c.") {
            URLS_FROM
        } else {
            "urls u"
        };
        let mut stmt = file.conn().prepare(&format!(
            "SELECT u.id,{select} FROM {from} ORDER BY {order}"
        ))?;
        let mut rows = stmt.query([])?;
        let mut ids = Vec::new();
        let mut ties = Vec::new();
        let mut previous: Vec<Value> = Vec::new();
        let mut start = 0;
        let mut empty_ids = Vec::new();
        while let Some(row) = rows.next()? {
            cancel.checkpoint().map_err(|_| SearchError::Cancelled)?;
            let values = (0..terms.len())
                .map(|i| row.get::<_, Value>(i + 1))
                .collect::<Result<Vec<_>, _>>()?;
            let id = super::url_index(row.get(0)?)?;
            if terms.len() == 1 && empty(&values[0]) {
                empty_ids.push(id);
                continue;
            }
            if !previous.is_empty()
                && values
                    .iter()
                    .zip(&previous)
                    .zip(&terms)
                    .any(|((a, b), t)| compare(a, b, t.collate) != Ordering::Equal)
            {
                if ids.len() - start > 1 {
                    ties.push((start as u32, ids.len() as u32));
                }
                start = ids.len();
            }
            previous = values;
            ids.push(id);
        }
        if ids.len() - start > 1 {
            ties.push((start as u32, ids.len() as u32));
        }
        let non_null = ids.len();
        ids.extend(empty_ids);
        if ids.len() - non_null > 1 {
            ties.push((non_null as u32, ids.len() as u32));
        }
        drop(rows);
        order_addresses(file, &mut ids, &ties, cancel)?;
        ties.retain(|(_, end)| *end as usize <= non_null);
        ids.shrink_to_fit();
        ties.shrink_to_fit();
        Ok(Self {
            ids,
            non_null,
            ties,
            terms,
        })
    }

    fn build_text(
        file: &CrawlFile,
        term: &SortTerm,
        cancel: &CancelToken,
    ) -> Result<Self, ViewError> {
        let from = if term.raw.contains("c.") {
            URLS_FROM
        } else {
            "urls u"
        };
        let mut stmt = file
            .conn()
            .prepare(&format!("SELECT u.id,{} FROM {from}", term.raw))?;
        let mut rows = stmt.query([])?;
        let mut values = Vec::new();
        let mut empty_ids = Vec::new();
        while let Some(row) = rows.next()? {
            cancel.checkpoint().map_err(|_| SearchError::Cancelled)?;
            let id = super::url_index(row.get(0)?)?;
            let value: Value = row.get(1)?;
            if empty(&value) {
                empty_ids.push(id);
            } else {
                values.push((id, value));
            }
        }
        drop(rows);
        values.shrink_to_fit();
        let order = sort_text(&mut values, cancel)?;
        let mut ids = Vec::with_capacity(values.len() + empty_ids.len());
        let mut ties = Vec::new();
        let mut previous: Option<usize> = None;
        let mut start = 0;
        for index in order {
            if ids.len().is_multiple_of(1024) {
                cancel.checkpoint().map_err(|_| SearchError::Cancelled)?;
            }
            let index = index as usize;
            if previous
                .is_some_and(|p| compare(&values[p].1, &values[index].1, true) != Ordering::Equal)
            {
                if ids.len() - start > 1 {
                    ties.push((start as u32, ids.len() as u32));
                }
                start = ids.len();
            }
            previous = Some(index);
            ids.push(values[index].0);
        }
        if ids.len() - start > 1 {
            ties.push((start as u32, ids.len() as u32));
        }
        drop(values);
        let non_null = ids.len();
        ids.extend(empty_ids);
        if ids.len() - non_null > 1 {
            ties.push((non_null as u32, ids.len() as u32));
        }
        order_addresses(file, &mut ids, &ties, cancel)?;
        ties.retain(|(_, end)| *end as usize <= non_null);
        ties.shrink_to_fit();
        let mut term = term.clone();
        term.desc = false;
        Ok(Self {
            ids,
            non_null,
            ties,
            terms: vec![term],
        })
    }

    /// Small live batches use primary-key probes and binary insertion, never a crawl-wide sort.
    pub fn repair(&mut self, file: &CrawlFile, changed: &[UrlId]) -> Result<bool, ViewError> {
        let changed: std::collections::BTreeSet<u32> = changed
            .iter()
            .map(|id| super::url_index(id.0))
            .collect::<Result<_, _>>()?;
        if changed.len().saturating_mul(100) > self.ids.len() {
            return Ok(false);
        }
        let select = self
            .terms
            .iter()
            .map(|t| t.raw.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let mut stmt = file.conn().prepare(&format!(
            "SELECT {select},u.address FROM {URLS_FROM} WHERE u.id=?1"
        ))?;
        let mut fetch = |id: u32| -> Result<Option<(Vec<Value>, String)>, ViewError> {
            Ok(stmt
                .query_row([id], |r| {
                    Ok((
                        (0..self.terms.len())
                            .map(|i| r.get(i))
                            .collect::<Result<Vec<Value>, _>>()?,
                        r.get(self.terms.len())?,
                    ))
                })
                .optional()?)
        };
        // Compact IDs and translate the existing equal-value intervals in one linear pass.
        let mut removed = 0u32;
        let mut tie = 0;
        let mut ties = Vec::new();
        let mut run_start = 0;
        let mut index = 0u32;
        self.ids.retain(|id| {
            if tie < self.ties.len() && index == self.ties[tie].0 {
                run_start = index - removed;
            }
            let keep = !changed.contains(id);
            if !keep {
                removed += 1;
            }
            index += 1;
            if tie < self.ties.len() && index == self.ties[tie].1 {
                let end = index - removed;
                if end - run_start > 1 {
                    ties.push((run_start, end));
                }
                tie += 1;
            }
            keep
        });
        self.ties = ties;
        // Recompute the boundary from the old tail length, including removals from either segment.
        self.non_null = if self.terms.len() > 1 {
            self.ids.len()
        } else {
            let mut lo = 0;
            let mut hi = self.ids.len();
            while lo < hi {
                let mid = (lo + hi) / 2;
                let value = fetch(self.ids[mid])?
                    .ok_or_else(|| super::invalid("Unvollständiger Änderungsfeed"))?;
                if empty(&value.0[0]) {
                    hi = mid;
                } else {
                    lo = mid + 1;
                }
            }
            lo
        };
        for id in changed {
            let Some(value) = fetch(id)? else {
                continue;
            };
            let cmp_values = |other: &[Value]| {
                for ((a, b), t) in other.iter().zip(&value.0).zip(&self.terms) {
                    let mut order = compare(a, b, t.collate);
                    if t.desc && !empty(a) && !empty(b) {
                        order = order.reverse();
                    }
                    if order != Ordering::Equal {
                        return order;
                    }
                }
                Ordering::Equal
            };
            let mut lo = 0;
            let mut hi = self.ids.len();
            while lo < hi {
                let mid = (lo + hi) / 2;
                let other = fetch(self.ids[mid])?
                    .ok_or_else(|| super::invalid("Unvollständiger Änderungsfeed"))?;
                let order = cmp_values(&other.0)
                    .then_with(|| other.1.cmp(&value.1))
                    .then(self.ids[mid].cmp(&id));
                if order == Ordering::Less {
                    lo = mid + 1;
                } else {
                    hi = mid;
                }
            }
            let is_non_null = self.terms.len() > 1 || !empty(&value.0[0]);
            let left_equal = lo > 0
                && is_non_null
                && cmp_values(&fetch(self.ids[lo - 1])?.unwrap().0) == Ordering::Equal;
            let right_equal = lo < self.ids.len()
                && is_non_null
                && cmp_values(&fetch(self.ids[lo])?.unwrap().0) == Ordering::Equal;
            let pos = lo as u32;
            for (start, end) in &mut self.ties {
                if *start >= pos {
                    *start += 1;
                }
                if *end > pos {
                    *end += 1;
                }
            }
            self.ids.insert(lo, id);
            if is_non_null {
                self.non_null += 1;
            }
            if left_equal || right_equal {
                let mut start = pos - u32::from(left_equal);
                let mut end = pos + 1 + u32::from(right_equal);
                self.ties.retain(|&(a, b)| {
                    if a < end && b > start {
                        start = start.min(a);
                        end = end.max(b);
                        false
                    } else {
                        true
                    }
                });
                self.ties.push((start, end));
                self.ties.sort_unstable();
            }
        }
        Ok(true)
    }
    pub fn visit(&self, descending: bool, mut f: impl FnMut(u32)) {
        if descending {
            let mut end = self.non_null;
            let mut tie = self.ties.len();
            while end > 0 {
                if tie > 0 && self.ties[tie - 1].1 as usize == end {
                    let start = self.ties[tie - 1].0 as usize;
                    for id in &self.ids[start..end] {
                        f(*id);
                    }
                    end = start;
                    tie -= 1;
                } else {
                    end -= 1;
                    f(self.ids[end]);
                }
            }
        } else {
            for id in &self.ids[..self.non_null] {
                f(*id);
            }
        }
        for id in &self.ids[self.non_null..] {
            f(*id);
        }
    }
    pub fn bytes(&self) -> u64 {
        (self.ids.capacity() * 4 + self.ties.capacity() * 8) as u64
    }
}

/// Sort bounded blocks, then merge adjacent runs. At most four workers and no retained workers.
fn sort_text(values: &mut [(u32, Value)], cancel: &CancelToken) -> Result<Vec<u32>, ViewError> {
    const CHUNK: usize = 32_768;
    let len = u32::try_from(values.len()).map_err(|_| super::invalid("Zu viele Sortwerte"))?;
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get().min(4));
    if values.len() <= CHUNK {
        cancel.checkpoint().map_err(|_| SearchError::Cancelled)?;
        values.sort_unstable_by(|a, b| compare(&a.1, &b.1, true));
        return Ok((0..len).collect());
    }
    let batch = CHUNK * values.len().div_ceil(CHUNK).div_ceil(workers);
    std::thread::scope(|scope| {
        let jobs: Vec<_> = values
            .chunks_mut(batch)
            .map(|batch| {
                scope.spawn(move || {
                    for chunk in batch.chunks_mut(CHUNK) {
                        cancel.checkpoint().map_err(|_| SearchError::Cancelled)?;
                        chunk.sort_unstable_by(|a, b| compare(&a.1, &b.1, true));
                    }
                    Ok::<_, ViewError>(())
                })
            })
            .collect();
        for job in jobs {
            job.join()
                .map_err(|_| super::invalid("Sortierthread abgebrochen"))??;
        }
        Ok::<_, ViewError>(())
    })?;
    let mut indices: Vec<u32> = (0..len).collect();
    let mut scratch = vec![0; values.len()];
    let mut width = CHUNK;
    while width < values.len() {
        let pair = width * 2;
        let batch = pair * values.len().div_ceil(pair).div_ceil(workers);
        let values = &*values;
        std::thread::scope(|scope| {
            let jobs: Vec<_> = scratch
                .chunks_mut(batch)
                .enumerate()
                .map(|(i, out)| {
                    let input = &indices[i * batch..i * batch + out.len()];
                    scope.spawn(move || {
                        for (out, input) in out.chunks_mut(pair).zip(input.chunks(pair)) {
                            let (left, right) = input.split_at(width.min(input.len()));
                            let (mut a, mut b) = (0, 0);
                            for (i, slot) in out.iter_mut().enumerate() {
                                if i.is_multiple_of(1024) {
                                    cancel.checkpoint().map_err(|_| SearchError::Cancelled)?;
                                }
                                if a < left.len()
                                    && (b == right.len()
                                        || compare(
                                            &values[left[a] as usize].1,
                                            &values[right[b] as usize].1,
                                            true,
                                        ) != Ordering::Greater)
                                {
                                    *slot = left[a];
                                    a += 1;
                                } else {
                                    *slot = right[b];
                                    b += 1;
                                }
                            }
                        }
                        Ok::<_, ViewError>(())
                    })
                })
                .collect();
            for job in jobs {
                job.join()
                    .map_err(|_| super::invalid("Sortierthread abgebrochen"))??;
            }
            Ok::<_, ViewError>(())
        })?;
        std::mem::swap(&mut indices, &mut scratch);
        width *= 2;
    }
    Ok(indices)
}

fn order_addresses(
    file: &CrawlFile,
    ids: &mut [u32],
    ties: &[(u32, u32)],
    cancel: &CancelToken,
) -> Result<(), ViewError> {
    let count: usize = ties.iter().map(|(start, end)| (end - start) as usize).sum();
    if count == ids.len() && count > 0 {
        // If every ID belongs to a tie (e.g. Status Code), read addresses once instead of once per value.
        let inverse = super::Inverse::build(ids);
        let mut groups: Vec<Vec<(u32, String)>> = ties
            .iter()
            .map(|(start, end)| Vec::with_capacity((end - start) as usize))
            .collect();
        let mut stmt = file.conn().prepare("SELECT id,address FROM urls")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            cancel.checkpoint().map_err(|_| SearchError::Cancelled)?;
            let id = super::url_index(row.get(0)?)?;
            let Some(pos) = inverse.locate(id).map(|p| p as u32) else {
                continue;
            };
            let group = ties.partition_point(|(_, end)| *end <= pos);
            if group < ties.len() && ties[group].0 <= pos {
                groups[group].push((id, row.get(1)?));
            }
        }
        for (&(start, end), mut addresses) in ties.iter().zip(groups) {
            cancel.checkpoint().map_err(|_| SearchError::Cancelled)?;
            addresses.sort_unstable_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
            for (id, (sorted, _)) in ids[start as usize..end as usize].iter_mut().zip(addresses) {
                *id = sorted;
            }
        }
    } else {
        for &(start, end) in ties {
            let group = &mut ids[start as usize..end as usize];
            let mut addresses = Vec::with_capacity(group.len());
            for chunk in group.chunks(500) {
                cancel.checkpoint().map_err(|_| SearchError::Cancelled)?;
                let requested = chunk
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                let mut stmt = file.conn().prepare(&format!(
                    "SELECT id,address FROM urls WHERE id IN ({requested})"
                ))?;
                let rows =
                    stmt.query_map([], |r| Ok((r.get::<_, u32>(0)?, r.get::<_, String>(1)?)))?;
                addresses.extend(rows.collect::<Result<Vec<_>, _>>()?);
            }
            addresses.sort_unstable_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
            for (id, (sorted, _)) in group.iter_mut().zip(addresses) {
                *id = sorted;
            }
        }
    }
    Ok(())
}
