//! OUT-015: correct standalone uploaded addresses while preserving their spelling and position.
use os_engine::list_input;
use os_store::{CrawlFile, OpenMode};
use std::path::Path;

pub fn prepare(items: &mut [String]) -> Vec<(u64, String)> {
    let mut originals = Vec::new();
    let mut position = 1;
    for item in items {
        let raw = item.trim();
        if raw.is_empty() {
            continue;
        }
        let parsed = list_input::parse_text(raw);
        if parsed.urls.is_empty() && !raw.chars().any(char::is_whitespace) {
            let candidate = os_url::parse_start_url(raw).ok().filter(|url| {
                !raw.contains("://") && url.url.host_str().is_some_and(|host| host.contains('.'))
            });
            let corrected = candidate
                .map(|n| n.encoded)
                .or_else(|| raw.contains("://").then(|| "http://[".to_string()));
            if let Some(corrected) = corrected {
                originals.push((position, raw.to_owned()));
                *item = corrected;
            }
        }
        position += parsed.urls.len().max(1) as u64;
    }
    originals
}
pub fn restore(path: &Path, originals: &[(u64, String)]) -> Result<(), String> {
    if originals.is_empty() {
        return Ok(());
    }
    let file = CrawlFile::open(path, OpenMode::ReadWrite).map_err(|e| e.to_string())?;
    let tx = file
        .conn()
        .unchecked_transaction()
        .map_err(|e| e.to_string())?;
    for (position, original) in originals {
        tx.execute(
            "UPDATE list_input SET original=?1 WHERE position=?2",
            (original, position),
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())
}
