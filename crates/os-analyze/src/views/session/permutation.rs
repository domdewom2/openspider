//! Ascending permutations with a separate empty tail and duplicate-value runs.
use std::cmp::Ordering;

use os_model::CancelToken;
use os_store::CrawlFile;
use rusqlite::types::Value;

use super::super::{build::SortTerm, URLS_FROM};
use crate::{error::ViewError, search::SearchError};

pub(super) struct Permutation {
    ids: Vec<u32>,
    non_null: usize,
    // Only runs with >1 member need storage. Singleton runs cost no extra memory.
    ties: Vec<(u32, u32)>,
}

fn equal(a: &Value, b: &Value, collate: bool) -> bool {
    match (a, b) {
        (Value::Text(a), Value::Text(b)) if collate => {
            os_store::collation::compare(a, b) == Ordering::Equal
        }
        (Value::Integer(a), Value::Real(b)) => *a as f64 == *b,
        (Value::Real(a), Value::Integer(b)) => *a == *b as f64,
        _ => a == b,
    }
}

impl Permutation {
    pub fn build(
        file: &CrawlFile,
        term: &SortTerm,
        cancel: &CancelToken,
    ) -> Result<Self, ViewError> {
        let sql = format!(
            "SELECT u.id, {} FROM {URLS_FROM} ORDER BY {} ASC, {} ASC, u.address ASC, u.id ASC",
            term.raw,
            term.empty_expr(),
            term.value_expr()
        );
        let mut stmt = file.conn().prepare(&sql)?;
        let mut rows = stmt.query([])?;
        let mut ids = Vec::new();
        let mut ties = Vec::new();
        let mut previous = Value::Null;
        let mut start = 0;
        let mut non_null = 0;
        while let Some(row) = rows.next()? {
            cancel.checkpoint().map_err(|_| SearchError::Cancelled)?;
            let id = super::url_index(row.get(0)?)?;
            let value: Value = row.get(1)?;
            let empty =
                matches!(&value, Value::Null) || matches!(&value, Value::Text(s) if s.is_empty());
            if !empty {
                if !equal(&value, &previous, term.collate) {
                    if ids.len() - start > 1 {
                        ties.push((start as u32, ids.len() as u32));
                    }
                    start = ids.len();
                }
                previous = value;
                non_null += 1;
            }
            ids.push(id);
        }
        if non_null - start > 1 {
            ties.push((start as u32, non_null as u32));
        }
        ids.shrink_to_fit();
        ties.shrink_to_fit();
        Ok(Self {
            ids,
            non_null,
            ties,
        })
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
