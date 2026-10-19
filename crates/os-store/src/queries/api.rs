//! API-Fakten, letzte Fehler je URL und Änderungsfeed; keine Zugangsdaten.
use crate::StoreError;
use os_model::{ApiValueWrite, Timestamp, UrlId};
use rusqlite::{params, Connection};

#[derive(Debug, Clone)]
pub struct Status {
    pub source: String,
    pub state: String,
    pub total: u64,
    pub done: u64,
    pub failed: u64,
    pub detail: Option<String>,
}
#[derive(Debug, Clone)]
pub struct ErrorRow {
    pub at: String,
    pub url: Option<String>,
    pub code: String,
    pub message: String,
}
pub fn statuses(conn: &Connection) -> Result<Vec<Status>, StoreError> {
    Ok(conn.prepare("SELECT source,state,urls_total,urls_done,urls_failed,detail FROM api_status ORDER BY source")?.query_map([],|r|Ok(Status {source:r.get(0)?,state:r.get(1)?,total:r.get(2)?,done:r.get(3)?,failed:r.get(4)?,detail:r.get(5)?}))?.collect::<Result<_,_>>()?)
}
pub fn set_status(conn: &Connection, status: &Status) -> Result<(), StoreError> {
    conn.execute("INSERT INTO api_status(source,state,urls_total,urls_done,urls_failed,detail) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(source) DO UPDATE SET state=excluded.state,urls_total=excluded.urls_total,urls_done=excluded.urls_done,urls_failed=excluded.urls_failed,detail=excluded.detail",params![status.source,status.state,status.total,status.done,status.failed,status.detail])?;
    Ok(())
}
pub fn errors(
    conn: &Connection,
    source: &str,
    offset: u64,
    limit: u32,
) -> Result<Vec<ErrorRow>, StoreError> {
    Ok(conn.prepare("SELECT at,url,code,message FROM api_errors WHERE source=?1 ORDER BY id DESC LIMIT ?2 OFFSET ?3")?.query_map(params![source,limit.min(10000),offset.min(i64::MAX as u64)],|r|Ok(ErrorRow {at:r.get(0)?,url:r.get(1)?,code:r.get(2)?,message:r.get(3)?}))?.collect::<Result<_,_>>()?)
}
pub fn record_error(
    conn: &Connection,
    source: &str,
    url: &str,
    code: &str,
    message: &str,
    at: Timestamp,
) -> Result<(), StoreError> {
    let tx = conn.unchecked_transaction()?;
    clear_error(&tx, source, url)?;
    tx.execute(
        "INSERT INTO api_errors(source,url,code,message,at) VALUES(?1,?2,?3,?4,?5)",
        params![source, url, code, message, at.to_db()],
    )?;
    let seq = crate::queries::change_seq(&tx)? + 1;
    tx.execute(
        "UPDATE urls SET change_seq=?1 WHERE url=?2",
        params![seq, url],
    )?;
    tx.execute(
        "UPDATE crawl_meta SET value=?1 WHERE key='change_seq'",
        [seq.to_string()],
    )?;
    tx.commit()?;
    Ok(())
}
pub fn clear_error(conn: &Connection, source: &str, url: &str) -> Result<(), StoreError> {
    conn.execute(
        "DELETE FROM api_errors WHERE source=?1 AND url=?2",
        params![source, url],
    )?;
    Ok(())
}
pub fn error_count(conn: &Connection, source: &str) -> Result<u64, StoreError> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM api_errors WHERE source=?1",
        [source],
        |r| r.get(0),
    )?)
}
pub fn urls(
    conn: &Connection,
    source: &str,
    missing: bool,
) -> Result<Vec<(UrlId, String)>, StoreError> {
    let sql = if missing {
        "SELECT id,url FROM urls u WHERE NOT EXISTS(SELECT 1 FROM api_values a WHERE a.url_id=u.id AND a.source=?1) ORDER BY id"
    } else {
        "SELECT id,url FROM urls WHERE ?1 IS NOT NULL ORDER BY id"
    };
    Ok(conn
        .prepare(sql)?
        .query_map([source], |r| Ok((UrlId(r.get(0)?), r.get(1)?)))?
        .collect::<Result<_, _>>()?)
}
pub fn invalidate(conn: &Connection, source: &str, remove: bool) -> Result<bool, StoreError> {
    let tx = conn.unchecked_transaction()?;
    let conn = &tx;
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM api_values WHERE source=?1)",
        [source],
        |r| r.get(0),
    )?;
    if exists {
        if remove {
            let seq = crate::queries::change_seq(conn)? + 1;
            conn.execute("UPDATE urls SET change_seq=?1 WHERE id IN(SELECT url_id FROM api_values WHERE source=?2)",params![seq,source])?;
            conn.execute(
                "UPDATE crawl_meta SET value=?1 WHERE key='change_seq'",
                [seq.to_string()],
            )?;
            conn.execute("DELETE FROM api_values WHERE source=?1", [source])?;
            conn.execute("DELETE FROM api_errors WHERE source=?1", [source])?;
        }
        conn.execute("INSERT INTO api_status(source,state,detail) VALUES(?1,'idle',?2) ON CONFLICT(source) DO UPDATE SET detail=excluded.detail",params![source,if remove {None}else{Some("Daten stammen aus einer früheren Konfiguration")}])?;
    }
    tx.commit()?;
    Ok(exists)
}
/// Ersetzt alle Metriken nur der erfolgreich neu angeforderten URL/Quelle (INTEG-009).
pub fn replace_values(
    conn: &mut Connection,
    source: &str,
    successful: &[(UrlId, String)],
    values: &[ApiValueWrite],
) -> Result<(), StoreError> {
    let tx = conn.transaction()?;
    for (id, url) in successful {
        tx.execute(
            "DELETE FROM api_values WHERE url_id=?1 AND source=?2",
            params![id.0, source],
        )?;
        clear_error(&tx, source, url)?;
    }
    for v in values.iter().filter(|v| v.source == source) {
        if let Some(id) = v.url_id {
            tx.execute("INSERT OR REPLACE INTO api_values(url_id,source,metric,level,v_real,v_text,fetched_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![id.0,v.source,v.metric,v.level,v.v_real,v.v_text,v.fetched_at.to_db()])?;
        } else {
            tx.execute(
                "INSERT OR IGNORE INTO api_urls(url,source,first_seen) VALUES(?1,?2,?3)",
                params![v.url, v.source, v.fetched_at.to_db()],
            )?;
        }
    }
    // API-Spalten nehmen am normalen Änderungsfeed teil.
    let seq: u64 = tx
        .query_row(
            "SELECT value FROM crawl_meta WHERE key='change_seq'",
            [],
            |r| r.get::<_, String>(0),
        )?
        .parse()
        .unwrap_or(0)
        + 1;
    for (id, _) in successful {
        tx.execute(
            "UPDATE urls SET change_seq=?1 WHERE id=?2",
            params![seq, id.0],
        )?;
    }
    tx.execute(
        "UPDATE crawl_meta SET value=?1 WHERE key='change_seq'",
        [seq.to_string()],
    )?;
    tx.commit()?;
    Ok(())
}
pub fn has_values(conn: &Connection, source: &str) -> Result<bool, StoreError> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM api_values WHERE source=?1)",
        [source],
        |r| r.get(0),
    )?)
}

/// Reine Argon2id-Schlüsselableitung für API-Geheimnisexporte; kein Datenbankzugriff.
/// Die Kryptobibliothek ist bereits Teil des eingefrorenen Store-Manifests.
/// TODO(M2-N01): argon2 in os-integrations/Cargo.toml aufnehmen und diese Funktion nach
/// os_integrations::secret_export verschieben (fachlicher Besitzer; Manifest bis dahin eingefroren).
pub fn derive_export_key(password: &str, salt: &[u8; 16]) -> Result<[u8; 32], StoreError> {
    if password.is_empty() {
        return Err(StoreError::InvalidInput(
            "Exportpasswort darf nicht leer sein".into(),
        ));
    }
    let mut key = [0; 32];
    let params = argon2::Params::new(19_456, 2, 1, Some(32))
        .map_err(|_| StoreError::InvalidInput("Export-KDF nicht verfügbar".into()))?;
    argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params)
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|_| StoreError::InvalidInput("Exportpasswort nicht verwendbar".into()))?;
    Ok(key)
}
