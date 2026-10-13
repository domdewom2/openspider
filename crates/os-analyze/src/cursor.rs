//! Keyset-Cursor für das Paging der Ansichten (base64url, an Abfrage und Crawl gebunden).
//!
//! Inhalt (JSON, dann base64url ohne Padding): `{v, crawl_id, query_hash, last_key, last_address, last_id}`.
//! `last_key` enthält die Rohwerte der Sortierschlüssel der letzten Zeile, `last_address` und `last_id` die
//! beiden festen Schlüssel am Ende des Sortiertupels (3.9). Ein Cursor gehört zu genau einem Crawl und einer
//! Abfrage (Tab, Filter, Sortierung, Suche); jeder andere Gebrauch ergibt [`ViewError::ForeignCursor`].

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

use crate::error::ViewError;

/// Aktuelle Cursor-Version.
pub const CURSOR_VERSION: u32 = 1;

/// Inhalt eines Cursors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cursor {
    /// Format-Version.
    pub v: u32,
    /// Crawl, aus dem der Cursor stammt.
    pub crawl_id: String,
    /// Hash der Abfrage (Tab, Filter, Sortierung, Suche).
    pub query_hash: String,
    /// Rohwerte der Sortierschlüssel der letzten Zeile (`null`, Zahl oder Text).
    pub last_key: Vec<Json>,
    /// `Address` der letzten Zeile.
    pub last_address: String,
    /// `UrlId` der letzten Zeile.
    pub last_id: i64,
}

impl Cursor {
    /// Kodiert den Cursor als undurchsichtige Zeichenkette.
    pub fn encode(&self) -> String {
        // Serialisierung eines Cursors aus JSON-Werten kann nicht scheitern.
        let json = serde_json::to_vec(self).unwrap_or_default();
        URL_SAFE_NO_PAD.encode(json)
    }

    /// Dekodiert einen Cursor. Unlesbare oder fremdformatige Cursor gelten als fremd.
    pub fn decode(s: &str) -> Result<Cursor, ViewError> {
        let bytes = URL_SAFE_NO_PAD
            .decode(s.trim())
            .map_err(|_| ViewError::ForeignCursor)?;
        let cursor: Cursor =
            serde_json::from_slice(&bytes).map_err(|_| ViewError::ForeignCursor)?;
        if cursor.v != CURSOR_VERSION {
            return Err(ViewError::ForeignCursor);
        }
        Ok(cursor)
    }

    /// Prüft, dass der Cursor zu diesem Crawl und dieser Abfrage gehört.
    pub fn check(&self, crawl_id: &str, query_hash: &str) -> Result<(), ViewError> {
        if self.crawl_id == crawl_id && self.query_hash == query_hash {
            Ok(())
        } else {
            Err(ViewError::ForeignCursor)
        }
    }
}

/// Stabiler 64-Bit-Hash (FNV-1a) über die Teile, als 16 Hexziffern. Bewusst nicht `DefaultHasher`:
/// der Wert muss über Programmläufe und Versionen gleich bleiben.
pub fn query_hash(parts: &[&str]) -> String {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    for part in parts {
        for byte in part.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(PRIME);
        }
        // Trenner, damit ["ab", "c"] und ["a", "bc"] verschieden hashen.
        hash ^= 0xff;
        hash = hash.wrapping_mul(PRIME);
    }
    format!("{hash:016x}")
}
