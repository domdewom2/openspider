//! Persistent local SERP preview edits; original crawl values remain untouched.
use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerpEdit {
    /// Last selected device; edits belong to the URL and survive device switches.
    pub device: String,
    /// None uses the crawled title; Some("") deliberately clears the preview.
    pub title: Option<String>,
    /// None uses the crawled description, with no generated fallback.
    pub description: Option<String>,
    pub edited_at: String,
}

/// Loads the URL's local preview override, if it has one.
pub fn load(conn: &Connection, url_id: i64) -> rusqlite::Result<Option<SerpEdit>> {
    conn.query_row(
        "SELECT device, title, description, edited_at FROM serp_edits WHERE url_id = ?1",
        [url_id],
        |r| {
            Ok(SerpEdit {
                device: r.get(0)?,
                title: r.get(1)?,
                description: r.get(2)?,
                edited_at: r.get(3)?,
            })
        },
    )
    .optional()
}

/// Timestamp comes from the caller's clock, never the wall clock.
pub fn save(conn: &Connection, url_id: i64, edit: &SerpEdit) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO serp_edits (url_id, device, title, description, edited_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(url_id) DO UPDATE SET device=excluded.device, title=excluded.title,
             description=excluded.description, edited_at=excluded.edited_at",
        params![
            url_id,
            edit.device,
            edit.title,
            edit.description,
            edit.edited_at
        ],
    )?;
    Ok(())
}

/// Discards the override. Repeated resets are harmless.
pub fn reset(conn: &Connection, url_id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM serp_edits WHERE url_id = ?1", [url_id])?;
    Ok(())
}
