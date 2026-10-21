//! Bounded, selection-scoped source documents and background diffs (UI-041).
use crate::{
    dto::*,
    pool::CrawlKey,
    runtime::{App, WindowId},
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

mod serp;
mod spelling;
pub use spelling::register as register_spelling_actions;
pub mod structured;
pub use serp::{measure, serp_data};

/// Canonical app crawl path: root to target, with a zero-based step in that order.
pub fn root_first_crawl_path(
    mut data: os_store::queries::DetailData,
) -> os_store::queries::DetailData {
    data.rows.reverse();
    for (step, row) in data.rows.iter_mut().enumerate() {
        row[1] = os_model::Value::Int(step as i64);
    }
    data
}

pub const MAX_DOCUMENT_BYTES: usize = 20 * 1024 * 1024;
const CACHE_BYTES: usize = 128 * 1024 * 1024;

struct Document {
    id: String,
    owner: WindowId,
    crawl: CrawlKey,
    text: String,
    starts: Vec<u32>,
}
#[derive(Default)]
pub struct Documents(Mutex<VecDeque<Arc<Document>>>);
impl Documents {
    pub fn open(
        &self,
        app: &App,
        owner: WindowId,
        crawl: CrawlKey,
        text: String,
    ) -> Result<SourceDoc, CmdError> {
        if text.len() > MAX_DOCUMENT_BYTES {
            return Err(CmdError::invalid_input("err-source-too-large", "urlId"));
        }
        let id = app
            .config()
            .ids
            .new_crawl_id(app.config().clock.now())
            .to_string();
        let mut starts = vec![0];
        starts.extend(
            text.bytes()
                .enumerate()
                .filter_map(|(i, b)| (b == b'\n').then_some((i + 1) as u32)),
        );
        let dto = SourceDoc {
            doc_id: id.clone(),
            lines: starts.len() as u64,
            bytes: text.len() as u64,
        };
        let doc = Arc::new(Document {
            id,
            owner,
            crawl,
            text,
            starts,
        });
        let mut docs = self.0.lock().unwrap_or_else(|p| p.into_inner());
        let size = |d: &Document| d.text.len() + d.starts.len() * std::mem::size_of::<u32>();
        // One source plus its index always fits; eviction also bounds newline-heavy documents.
        while docs.iter().map(|d| size(d)).sum::<usize>() + size(&doc) > CACHE_BYTES {
            if docs.pop_front().is_none() {
                break;
            }
        }
        if size(&doc) > CACHE_BYTES {
            return Err(CmdError::invalid_input("err-source-too-large", "urlId"));
        }
        docs.push_back(doc);
        Ok(dto)
    }
    pub fn clear_crawl(&self, crawl: &CrawlKey) {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .retain(|d| &d.crawl != crawl);
    }
    pub fn clear_window(&self, owner: &WindowId) {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .retain(|d| &d.owner != owner);
    }
    pub fn lines(
        &self,
        owner: &WindowId,
        crawl: &CrawlKey,
        args: &SourceLinesArgs,
    ) -> Result<Vec<String>, CmdError> {
        if args.count > 10_000 {
            return Err(CmdError::invalid_input("err-source-range", "count"));
        }
        let doc = {
            let mut docs = self.0.lock().unwrap_or_else(|p| p.into_inner());
            let pos = docs
                .iter()
                .position(|d| d.id == args.doc_id && &d.owner == owner && &d.crawl == crawl)
                .ok_or_else(|| CmdError::new(ErrorCode::NotFound, "err-source-doc-not-found"))?;
            let doc = docs.remove(pos).expect("existing document");
            docs.push_back(doc.clone());
            doc
        };
        let start = usize::try_from(args.from)
            .unwrap_or(usize::MAX)
            .min(doc.starts.len());
        let end = start
            .saturating_add(args.count as usize)
            .min(doc.starts.len());
        let mut bytes = 0;
        let mut lines = Vec::new();
        for i in start..end {
            let end = doc
                .starts
                .get(i + 1)
                .copied()
                .map(|n| n as usize)
                .unwrap_or(doc.text.len());
            let line = doc.text[doc.starts[i] as usize..end]
                .trim_end_matches('\n')
                .strip_suffix('\r')
                .unwrap_or_else(|| doc.text[doc.starts[i] as usize..end].trim_end_matches('\n'));
            bytes += line.len();
            if bytes > MAX_DOCUMENT_BYTES {
                break;
            }
            lines.push(line.to_owned());
        }
        Ok(lines)
    }
}

pub fn unavailable(key: &str) -> CmdError {
    CmdError::new(ErrorCode::Unsupported, "err-detail-unavailable").with_arg(
        "state",
        serde_json::json!({"code":"FEATURE_DISABLED", "arg":key}),
    )
}
pub fn ensure_url(exists: bool) -> Result<(), CmdError> {
    if !exists {
        return Err(CmdError::new(ErrorCode::NotFound, "err-url-not-found"));
    }
    Ok(())
}
pub fn load(
    stored: Option<os_store::queries::StoredHtml>,
    which: SourceWhich,
) -> Result<String, CmdError> {
    let stored = stored.ok_or_else(|| {
        unavailable(match which {
            SourceWhich::Original => "extraction.store_html",
            SourceWhich::Rendered => "extraction.store_rendered_html",
        })
    })?;
    if stored.bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(CmdError::invalid_input("err-source-too-large", "urlId"));
    }
    let content_type = stored.charset.map(|c| format!("text/html; charset={c}"));
    Ok(os_parse::charset::decode(&stored.bytes, content_type.as_deref(), &[]).0)
}
/// Body text using the same walker as word counts, without configured content exclusions.
pub fn visible_text(html: &str) -> String {
    let area = os_model::ContentAreaConfig {
        exclude_elements: vec![],
        ..Default::default()
    };
    os_parse::content::area_text_flat(&os_parse::dom::Document::parse(html), &area)
}
/// Display-only formatting. Raw text elements and quoted attributes remain byte-for-byte intact.
pub fn pretty(html: &str) -> String {
    // Only display boundaries change; source bytes and raw-text contents are preserved.
    let serialized = html;
    let mut output = String::with_capacity(serialized.len());
    let mut quote = None;
    let mut in_tag = false;
    let mut raw: Option<String> = None;
    let mut tag_start = 0;
    for (i, c) in serialized.char_indices() {
        if c == '<' && !in_tag {
            let rest = &serialized[i..];
            if raw
                .as_ref()
                .is_some_and(|name| !rest.starts_with(&format!("</{name}")))
            {
                output.push(c);
                continue;
            }
            in_tag = true;
            tag_start = i;
            if raw.is_none()
                && !output.is_empty()
                && !output.ends_with('\n')
                && [
                    "<html", "<head", "<body", "<div", "<p>", "<section", "<main", "<article",
                    "<ul", "<li", "<table", "<tr",
                ]
                .iter()
                .any(|s| rest.starts_with(s))
            {
                output.push('\n');
            }
        } else if in_tag {
            if let Some(q) = quote {
                if c == q {
                    quote = None;
                }
            } else if c == '\'' || c == '"' {
                quote = Some(c);
            } else if c == '>' {
                in_tag = false;
                let name = serialized[tag_start + 1..i]
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_end_matches('/');
                if name.starts_with('/') {
                    raw = None;
                } else if matches!(name, "script" | "style" | "pre" | "textarea") {
                    raw = Some(name.to_owned());
                }
            }
        }
        output.push(c);
    }
    output
}

pub fn diff(
    old: &str,
    new: &str,
    mode: DiffMode,
    cancel: &os_analyze::analysis::CancelToken,
) -> Result<DiffHunks, CmdError> {
    if cancel.is_cancelled() {
        return Err(CmdError::new(ErrorCode::Cancelled, "err-cancelled"));
    }
    // A deterministic complexity gate rejects divergent large inputs before exact Myers work.
    // Common prefixes/suffixes do not contribute, so large identical documents remain cheap.
    fn units(text: &str, mode: DiffMode) -> Vec<&str> {
        match mode {
            DiffMode::Words => text.split_whitespace().collect(),
            DiffMode::Lines => text.split_inclusive('\n').collect(),
        }
    }
    let (old_units, new_units) = (units(old, mode), units(new, mode));
    let prefix = old_units
        .iter()
        .zip(&new_units)
        .take_while(|(a, b)| a == b)
        .count();
    let suffix = old_units[prefix..]
        .iter()
        .rev()
        .zip(new_units[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let old_work = old_units.len() - prefix - suffix;
    let new_work = new_units.len() - prefix - suffix;
    if old_work.saturating_mul(new_work) > 4_000_000 {
        return Err(CmdError::invalid_input(
            "err-source-diff-too-complex",
            "text",
        ));
    }
    drop((old_units, new_units));
    let mut config = similar::TextDiff::configure();
    // No wall-clock deadline: Myers must produce the same exact result on every machine.
    config.algorithm(similar::Algorithm::Myers);
    let old_words: Vec<_> = if mode == DiffMode::Words {
        old.split_whitespace().collect()
    } else {
        vec![]
    };
    let new_words: Vec<_> = if mode == DiffMode::Words {
        new.split_whitespace().collect()
    } else {
        vec![]
    };
    let diff = match mode {
        DiffMode::Lines => config.diff_lines(old, new),
        DiffMode::Words => config.diff_slices(&old_words, &new_words),
    };
    let mut result = DiffHunks {
        added: 0,
        removed: 0,
        hunks: vec![],
    };
    for ops in diff.grouped_ops(3) {
        if cancel.is_cancelled() {
            return Err(CmdError::new(ErrorCode::Cancelled, "err-cancelled"));
        }
        let first = ops.first().expect("nonempty diff group");
        let last = ops.last().expect("nonempty diff group");
        let mut hunk = DiffHunk {
            old_start: first.old_range().start as u64 + 1,
            old_lines: (last.old_range().end - first.old_range().start) as u64,
            new_start: first.new_range().start as u64 + 1,
            new_lines: (last.new_range().end - first.new_range().start) as u64,
            lines: vec![],
        };
        for op in ops {
            for change in diff.iter_changes(&op) {
                let units = match mode {
                    DiffMode::Lines => 1,
                    DiffMode::Words => change.value().split_whitespace().count() as u64,
                };
                let kind = match change.tag() {
                    similar::ChangeTag::Equal => DiffKind::Context,
                    similar::ChangeTag::Insert => {
                        result.added += units;
                        DiffKind::Added
                    }
                    similar::ChangeTag::Delete => {
                        result.removed += units;
                        DiffKind::Removed
                    }
                };
                hunk.lines.push(DiffLine {
                    kind,
                    text: change.value().into(),
                });
            }
        }
        result.hunks.push(hunk);
    }
    Ok(result)
}

/// Publish a complete file atomically in the destination directory chosen by dialog.save.
pub fn save_file(path: &str, bytes: &[u8]) -> Result<FileResult, CmdError> {
    use std::io::Write;
    let target = std::path::Path::new(path);
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    let fail = |e: std::io::Error| {
        CmdError::new(ErrorCode::Io, "err-detail-save").with_detail(e.to_string())
    };
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(fail)?;
    temporary.write_all(bytes).map_err(fail)?;
    temporary.flush().map_err(fail)?;
    temporary.persist(target).map_err(|e| fail(e.error))?;
    Ok(FileResult {
        path: path.into(),
        bytes: bytes.len() as u64,
    })
}
