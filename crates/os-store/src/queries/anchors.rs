//! Abfragen: Ankertexte und Link-Abfragen der Links-Tabs (Non-Descriptive Anchor Text, Detailansicht).
//!
//! Die Funktionen legt das Fachpaket an; M2-F02 stellt nur das Modul bereit, damit `queries/mod.rs` nach Welle 0
//! eingefroren ist.

// STUB(M2-D08)

/// Erweiterungspunkt für die Ankertext-Kollation (M2-D08).
pub fn register_collations(_conn: &rusqlite::Connection) -> Result<(), crate::StoreError> {
    Ok(())
}

// STUB(M2-D08)
pub fn detail_rows(
    _: &crate::CrawlFile,
    _: &[os_model::UrlId],
    _: u64,
    _: u64,
) -> Result<super::DetailData, crate::StoreError> {
    Err(crate::StoreError::NotImplemented("M2-D08"))
}

/// Aggregated immutable input for anchor consumers (UI-099/TECH-146).
/// Keeping follow/self flags separate allows changing options without rereading live data.
#[derive(Debug, Clone)]
pub struct SnapshotGroup {
    pub target_id: i64,
    pub text: String,
    pub follow: bool,
    pub self_link: bool,
    pub count: u64,
}

/// Internal hyperlink phrases, case-sensitive; image hyperlink fallback uses its alt text.
/// This additive entrypoint also lets the Anchors detail provider share normalization.
pub fn snapshot_groups(file: &crate::CrawlFile) -> Result<Vec<SnapshotGroup>, crate::StoreError> {
    let mut stmt = file.conn().prepare(
        "SELECT l.target_id, l.anchor, l.alt_text, l.follow, l.source_id = l.target_id, COUNT(*)
         FROM links l JOIN urls s ON s.id = l.source_id
         WHERE s.is_internal = 1 AND l.link_type = 'Hyperlink' AND l.target_id IS NOT NULL
         GROUP BY l.target_id, l.anchor, l.alt_text, l.follow, l.source_id = l.target_id",
    )?;
    let rows = stmt.query_map([], |r| {
        let anchor: Option<String> = r.get(1)?;
        let alt: Option<String> = r.get(2)?;
        let text = anchor
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .or(alt.as_deref())
            .unwrap_or("")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        Ok(SnapshotGroup {
            target_id: r.get(0)?,
            text: if text.is_empty() {
                "(leer)".into()
            } else {
                text
            },
            follow: r.get(3)?,
            self_link: r.get(4)?,
            count: r.get(5)?,
        })
    })?;
    rows.collect::<Result<_, _>>().map_err(Into::into)
}
