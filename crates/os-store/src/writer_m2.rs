//! Writer für die M2-Teile eines `WriteBatch` (Schema v3, `docs/plan/M2.md` 3.4): `ext`, `structured_data`/`sd_issues`,
//! `render`, `a11y`, `spelling`, `custom_search`, `custom_extraction`, `custom_js`, `forms`, `minhash`, `api_values`
//! sowie der Quelltext `PageWrite.html` (`page_html`).
//!
//! Alles läuft in **derselben Transaktion** wie die Seite. **Änderungsfeed:** `url_ext`-Zeilen tragen `change_seq = n`
//! (`queries::changed_since` liest die Tabelle mit); jeder andere Teil stempelt `urls.change_seq = n` der betroffenen
//! URL. Eine unbekannte URL-Kennung ist ein Fehler des Batches (`StoreError::InvalidInput`), die Datei bleibt dann
//! unverändert.
//!
//! **Ersetzungsregeln** (ein erneuter Abruf liefert den vollständigen Stand, deshalb gilt je Teil):
//!
//! | Teil | Regel |
//! |---|---|
//! | `ext` | Upsert je `(URL, Schlüssel)`; Schlüssel verschiedener Pakete und Zeitpunkte (Parsen, Analyse) ergänzen sich |
//! | `structured_data` | die Elemente einer im Batch genannten URL ersetzen deren bisherige Elemente **und** alle ihre `sd_issues` |
//! | `sd_issues` | ersetzen je `(URL, validation)` die bisherigen Befunde dieser Art (`parse`, `schema_org`, `rich_results`); `item` zählt in den Elementen der URL aus diesem Batch, sonst in den gespeicherten (nach `id`) |
//! | `render` | ersetzt je URL Seite, Konsole, blockierte Ressourcen, Netzwerk und Screenshot-Kacheln |
//! | `a11y` | ersetzt je URL alle Regelergebnisse und Knoten |
//! | `spelling` | ersetzt je `(URL, kind)` |
//! | `custom_search`, `custom_extraction`, `custom_js`, `forms` | ersetzen je URL die Zeilen der Tabelle |
//! | `minhash` | Upsert je URL |
//! | `api_values` | Upsert je `(URL, Quelle, Metrik, Ebene)`; Werte ohne bekannte URL (`url_id = None`) landen in `api_urls` |
//!
//! Ein `PageWrite` mit `replace` löscht außerdem die Zeilen dieser Tabellen der URL (außer `url_ext`, `api_values`,
//! `serp_edits`, `url_segments`, `near_duplicates`), damit ein erneuter Abruf keine Reste des alten Abrufs behält.

use os_model::{ExtKey, SdIssueWrite, UrlId, Value as ModelValue, WriteBatch};
use rusqlite::{params, OptionalExtension, Transaction};

use crate::error::StoreError;

fn bad(message: String) -> StoreError {
    StoreError::InvalidInput(message)
}

/// Ob der Batch keinen der M2-Teile trägt (für die No-op-Prüfung von `write_batch`).
pub(crate) fn parts_empty(batch: &WriteBatch) -> bool {
    batch.cookies.is_empty()
        && batch.discoveries.is_empty()
        && batch.ext.is_empty()
        && batch.structured_data.is_empty()
        && batch.sd_issues.is_empty()
        && batch.render.is_empty()
        && batch.a11y.is_empty()
        && batch.spelling.is_empty()
        && batch.custom_search.is_empty()
        && batch.custom_extraction.is_empty()
        && batch.custom_js.is_empty()
        && batch.forms.is_empty()
        && batch.minhash.is_empty()
        && batch.api_values.is_empty()
}

/// Löscht die seitenbezogenen M2-Zeilen einer URL (`PageWrite.replace`).
pub(crate) fn clear_page_children(tx: &Transaction<'_>, id: i64) -> Result<(), StoreError> {
    for (namespace, _) in os_model::ext::NAMESPACES {
        if os_model::ext::is_fetch_scoped(namespace) {
            tx.execute("DELETE FROM url_ext WHERE url_id = ?1 AND substr(key, 1, instr(key, '.') - 1) = ?2", params![id, namespace])?;
        }
    }
    for sql in [
        "DELETE FROM cookies WHERE url_id = ?1",
        "DELETE FROM sd_items WHERE url_id = ?1",
        "DELETE FROM sd_issues WHERE url_id = ?1",
        "DELETE FROM forms WHERE url_id = ?1",
        "DELETE FROM spelling_findings WHERE url_id = ?1",
        "DELETE FROM custom_search_hits WHERE url_id = ?1",
        "DELETE FROM custom_extraction_values WHERE url_id = ?1",
        "DELETE FROM custom_js_values WHERE url_id = ?1",
        "DELETE FROM a11y_rules WHERE url_id = ?1",
        "DELETE FROM a11y_nodes WHERE url_id = ?1",
        "DELETE FROM render_pages WHERE url_id = ?1",
        "DELETE FROM render_console WHERE url_id = ?1",
        "DELETE FROM render_blocked WHERE url_id = ?1",
        "DELETE FROM render_network WHERE url_id = ?1",
        "DELETE FROM screenshots WHERE url_id = ?1",
        "DELETE FROM minhash WHERE url_id = ?1",
        "DELETE FROM page_html WHERE url_id = ?1",
    ] {
        tx.prepare_cached(sql)?.execute([id])?;
    }
    Ok(())
}

/// Schreibt den Quelltext einer Seite (`page_html`, CRAWL-063).
pub(crate) fn write_page_html(
    tx: &Transaction<'_>,
    id: i64,
    html: &os_model::PageHtml,
) -> Result<(), StoreError> {
    tx.prepare_cached(
        "INSERT OR REPLACE INTO page_html (url_id, encoding, body, compression) VALUES (?1, ?2, ?3, 'zstd')",
    )?
    .execute(params![id, html.encoding, compress(&html.body)?])?;
    Ok(())
}

/// Sortierte, doppelfreie URL-Kennungen.
fn distinct(ids: impl Iterator<Item = i64>) -> Vec<i64> {
    let mut v: Vec<i64> = ids.collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Stempelt `urls.change_seq` der URLs; eine unbekannte URL ist ein Fehler.
fn stamp_urls(tx: &Transaction<'_>, part: &str, ids: &[i64], seq: i64) -> Result<(), StoreError> {
    if ids.is_empty() {
        return Ok(());
    }
    let mut stamp = tx.prepare_cached("UPDATE urls SET change_seq = ?1 WHERE id = ?2")?;
    for id in ids {
        if stamp.execute(params![seq, id])? == 0 {
            return Err(bad(format!("{part}: URL {id} existiert nicht")));
        }
    }
    Ok(())
}

/// Schreibt alle M2-Teile des Batches (nach den Seiten, damit die URL-Zeilen existieren).
pub(crate) fn write(tx: &Transaction<'_>, batch: &WriteBatch, seq: i64) -> Result<(), StoreError> {
    write_w1(tx, batch, seq)?;
    write_ext(tx, batch, seq)?;
    write_structured_data(tx, batch, seq)?;
    write_render(tx, batch, seq)?;
    write_a11y(tx, batch, seq)?;
    write_spelling(tx, batch, seq)?;
    write_custom(tx, batch, seq)?;
    write_forms(tx, batch, seq)?;
    write_minhash(tx, batch, seq)?;
    write_api_values(tx, batch, seq)?;
    Ok(())
}

fn write_ext(tx: &Transaction<'_>, batch: &WriteBatch, seq: i64) -> Result<(), StoreError> {
    if batch.ext.is_empty() {
        return Ok(());
    }
    let mut exists = tx.prepare_cached("SELECT 1 FROM urls WHERE id = ?1")?;
    let mut upsert = tx.prepare_cached(
        "INSERT INTO url_ext (url_id, key, v_int, v_real, v_text, change_seq) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
         ON CONFLICT(url_id, key) DO UPDATE SET v_int = excluded.v_int, v_real = excluded.v_real, \
           v_text = excluded.v_text, change_seq = excluded.change_seq",
    )?;
    let mut known: std::collections::HashSet<i64> = std::collections::HashSet::new();
    for e in &batch.ext {
        ExtKey::parse(e.key.as_str())
            .map_err(|_| bad(format!("ext: ungültiger Schlüssel {:?}", e.key.as_str())))?;
        let id = e.url.0;
        if !known.contains(&id) {
            if exists.query_row([id], |_| Ok(())).optional()?.is_none() {
                return Err(bad(format!("ext: URL {id} existiert nicht")));
            }
            known.insert(id);
        }
        let (v_int, v_real, v_text): (Option<i64>, Option<f64>, Option<&str>) = match &e.value {
            ModelValue::Null => (None, None, None),
            ModelValue::Bool(b) => (Some(i64::from(*b)), None, None),
            ModelValue::Int(i) => (Some(*i), None, None),
            ModelValue::Float(f) => (None, Some(*f), None),
            ModelValue::Text(t) => (None, None, Some(t.as_str())),
        };
        upsert.execute(params![id, e.key.as_str(), v_int, v_real, v_text, seq])?;
    }
    Ok(())
}

fn write_structured_data(
    tx: &Transaction<'_>,
    batch: &WriteBatch,
    seq: i64,
) -> Result<(), StoreError> {
    if batch.structured_data.is_empty() && batch.sd_issues.is_empty() {
        return Ok(());
    }
    let item_urls = distinct(batch.structured_data.iter().map(|i| i.url_id.0));
    let issue_urls = distinct(batch.sd_issues.iter().map(|i| i.url_id.0));
    stamp_urls(
        tx,
        "structured_data",
        &distinct(item_urls.iter().chain(issue_urls.iter()).copied()),
        seq,
    )?;

    // Elemente ersetzen (samt ihren Befunden, die sich auf die alten Elemente beziehen).
    {
        let mut del_items = tx.prepare_cached("DELETE FROM sd_items WHERE url_id = ?1")?;
        let mut del_issues = tx.prepare_cached("DELETE FROM sd_issues WHERE url_id = ?1")?;
        for id in &item_urls {
            del_items.execute([id])?;
            del_issues.execute([id])?;
        }
    }
    // Befunde ersetzen je (URL, Art).
    {
        let mut del =
            tx.prepare_cached("DELETE FROM sd_issues WHERE url_id = ?1 AND validation = ?2")?;
        let mut seen: std::collections::HashSet<(i64, &str)> = std::collections::HashSet::new();
        for i in &batch.sd_issues {
            if seen.insert((i.url_id.0, i.validation.as_str())) {
                del.execute(params![i.url_id.0, i.validation])?;
            }
        }
    }

    let mut insert_item = tx.prepare_cached(
        "INSERT INTO sd_items (url_id, format, type_path, json) VALUES (?1, ?2, ?3, ?4)",
    )?;
    let mut ids_by_url: std::collections::HashMap<i64, Vec<i64>> = std::collections::HashMap::new();
    for item in &batch.structured_data {
        insert_item.execute(params![
            item.url_id.0,
            item.format,
            item.type_path,
            item.json
        ])?;
        ids_by_url
            .entry(item.url_id.0)
            .or_default()
            .push(tx.last_insert_rowid());
    }
    let mut insert_issue = tx.prepare_cached(
        "INSERT INTO sd_issues (url_id, item_id, validation, feature, severity, property, code) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    for issue in &batch.sd_issues {
        let item_id = resolve_item(tx, &ids_by_url, issue)?;
        insert_issue.execute(params![
            issue.url_id.0,
            item_id,
            issue.validation,
            issue.feature,
            issue.severity,
            issue.property,
            issue.code
        ])?;
    }
    Ok(())
}

/// `SdIssueWrite.item` → `sd_items.id`: Index in den Elementen der URL aus diesem Batch, sonst in den gespeicherten.
fn resolve_item(
    tx: &Transaction<'_>,
    batch_ids: &std::collections::HashMap<i64, Vec<i64>>,
    issue: &SdIssueWrite,
) -> Result<Option<i64>, StoreError> {
    let Some(index) = issue.item else {
        return Ok(None);
    };
    let index = usize::try_from(index).unwrap_or(usize::MAX);
    if let Some(ids) = batch_ids.get(&issue.url_id.0) {
        return ids.get(index).copied().map(Some).ok_or_else(|| {
            bad(format!(
                "sd_issues: URL {} hat kein Element mit Index {index}",
                issue.url_id
            ))
        });
    }
    let found: Option<i64> = tx
        .prepare_cached("SELECT id FROM sd_items WHERE url_id = ?1 ORDER BY id LIMIT 1 OFFSET ?2")?
        .query_row(params![issue.url_id.0, index as i64], |r| r.get(0))
        .optional()?;
    found.map(Some).ok_or_else(|| {
        bad(format!(
            "sd_issues: URL {} hat kein Element mit Index {index}",
            issue.url_id
        ))
    })
}

fn write_render(tx: &Transaction<'_>, batch: &WriteBatch, seq: i64) -> Result<(), StoreError> {
    if batch.render.is_empty() {
        return Ok(());
    }
    stamp_urls(
        tx,
        "render",
        &distinct(batch.render.iter().map(|r| r.url_id.0)),
        seq,
    )?;
    for r in &batch.render {
        let id = r.url_id.0;
        for sql in [
            "DELETE FROM render_console WHERE url_id = ?1",
            "DELETE FROM render_blocked WHERE url_id = ?1",
            "DELETE FROM render_network WHERE url_id = ?1",
            "DELETE FROM screenshots WHERE url_id = ?1",
        ] {
            tx.prepare_cached(sql)?.execute([id])?;
        }
        tx.prepare_cached(
            "INSERT OR REPLACE INTO render_pages (url_id, status, html, word_count, load_ms, wait_ms, total_ms, compression, console_dropped) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'zstd', ?8)",
        )?
        .execute(params![
            id,
            r.status,
            r.html.as_deref().map(compress).transpose()?,
            r.word_count,
            r.timings.load_ms,
            r.timings.wait_ms,
            r.timings.total_ms,
            r.console_dropped
        ])?;
        let mut console = tx.prepare_cached(
            "INSERT INTO render_console (url_id, position, level, text, source, line, col, timestamp_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?;
        for (i, c) in r.console.iter().enumerate() {
            console.execute(params![
                id,
                i as i64,
                c.level,
                c.text,
                c.source,
                c.line,
                c.column,
                c.timestamp_ms
            ])?;
        }
        let mut blocked = tx.prepare_cached(
            "INSERT INTO render_blocked (url_id, position, url, reason, status) VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for (i, b) in r.blocked.iter().enumerate() {
            blocked.execute(params![id, i as i64, b.url, b.reason, b.status])?;
        }
        let mut network = tx.prepare_cached(
            "INSERT INTO render_network (url_id, position, url, status, resource_type, bytes, duration_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;
        for (i, n) in r.network.iter().enumerate() {
            network.execute(params![
                id,
                i as i64,
                n.url,
                n.status,
                n.resource_type,
                i64::try_from(n.bytes).unwrap_or(i64::MAX),
                n.duration_ms
            ])?;
        }
        let mut shots = tx.prepare_cached(
            "INSERT OR REPLACE INTO screenshots (url_id, tile, png, width, height) VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for s in &r.screenshots {
            shots.execute(params![id, s.tile, s.data, s.width, s.height])?;
        }
    }
    Ok(())
}

fn write_a11y(tx: &Transaction<'_>, batch: &WriteBatch, seq: i64) -> Result<(), StoreError> {
    if batch.a11y.is_empty() {
        return Ok(());
    }
    stamp_urls(
        tx,
        "a11y",
        &distinct(batch.a11y.iter().map(|a| a.url_id.0)),
        seq,
    )?;
    for a in &batch.a11y {
        let id = a.url_id.0;
        tx.prepare_cached("DELETE FROM a11y_rules WHERE url_id = ?1")?
            .execute([id])?;
        tx.prepare_cached("DELETE FROM a11y_nodes WHERE url_id = ?1")?
            .execute([id])?;
        let mut rule = tx.prepare_cached(
            "INSERT OR REPLACE INTO a11y_rules (url_id, rule_id, status, impact) VALUES (?1, ?2, ?3, ?4)",
        )?;
        for r in &a.rules {
            rule.execute(params![id, r.rule_id, r.status, r.impact])?;
        }
        let mut node = tx.prepare_cached(
            "INSERT INTO a11y_nodes (url_id, rule_id, target, snippet, summary) VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for n in &a.nodes {
            node.execute(params![id, n.rule_id, n.target, n.snippet, n.summary])?;
        }
    }
    Ok(())
}

fn write_spelling(tx: &Transaction<'_>, batch: &WriteBatch, seq: i64) -> Result<(), StoreError> {
    if batch.spelling.is_empty() {
        return Ok(());
    }
    stamp_urls(
        tx,
        "spelling",
        &distinct(batch.spelling.iter().map(|s| s.url_id.0)),
        seq,
    )?;
    let mut del =
        tx.prepare_cached("DELETE FROM spelling_findings WHERE url_id = ?1 AND kind = ?2")?;
    let mut seen: std::collections::HashSet<(i64, &str)> = std::collections::HashSet::new();
    for s in &batch.spelling {
        if seen.insert((s.url_id.0, s.kind.as_str())) {
            del.execute(params![s.url_id.0, s.kind])?;
        }
    }
    let mut insert = tx.prepare_cached(
        "INSERT INTO spelling_findings (url_id, kind, language, section, text, detail, suggestion, rule_id) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?;
    for s in &batch.spelling {
        insert.execute(params![
            s.url_id.0,
            s.kind,
            s.language,
            s.section,
            s.text,
            s.detail,
            s.suggestion,
            s.rule_id
        ])?;
    }
    Ok(())
}

fn write_custom(tx: &Transaction<'_>, batch: &WriteBatch, seq: i64) -> Result<(), StoreError> {
    if !batch.custom_search.is_empty() {
        let ids = distinct(batch.custom_search.iter().map(|c| c.url_id.0));
        stamp_urls(tx, "custom_search", &ids, seq)?;
        let mut del = tx.prepare_cached("DELETE FROM custom_search_hits WHERE url_id = ?1")?;
        for id in &ids {
            del.execute([id])?;
        }
        let mut insert = tx.prepare_cached(
            "INSERT OR REPLACE INTO custom_search_hits (url_id, def_index, count, error) VALUES (?1, ?2, ?3, ?4)",
        )?;
        for c in &batch.custom_search {
            insert.execute(params![c.url_id.0, c.def_index, c.count, c.error])?;
        }
    }
    if !batch.custom_extraction.is_empty() {
        let ids = distinct(batch.custom_extraction.iter().map(|c| c.url_id.0));
        stamp_urls(tx, "custom_extraction", &ids, seq)?;
        let mut del =
            tx.prepare_cached("DELETE FROM custom_extraction_values WHERE url_id = ?1")?;
        for id in &ids {
            del.execute([id])?;
        }
        let mut insert = tx.prepare_cached(
            "INSERT OR REPLACE INTO custom_extraction_values (url_id, def_index, number, value, error) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for c in &batch.custom_extraction {
            insert.execute(params![c.url_id.0, c.def_index, c.number, c.value, c.error])?;
        }
    }
    if !batch.custom_js.is_empty() {
        let ids = distinct(batch.custom_js.iter().map(|c| c.url_id.0));
        stamp_urls(tx, "custom_js", &ids, seq)?;
        let mut del = tx.prepare_cached("DELETE FROM custom_js_values WHERE url_id = ?1")?;
        for id in &ids {
            del.execute([id])?;
        }
        let mut insert = tx.prepare_cached(
            "INSERT OR REPLACE INTO custom_js_values (url_id, def_index, number, value, error) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for c in &batch.custom_js {
            insert.execute(params![c.url_id.0, c.def_index, c.number, c.value, c.error])?;
        }
    }
    Ok(())
}

fn write_forms(tx: &Transaction<'_>, batch: &WriteBatch, seq: i64) -> Result<(), StoreError> {
    if batch.forms.is_empty() {
        return Ok(());
    }
    let ids = distinct(batch.forms.iter().map(|f| f.url_id.0));
    stamp_urls(tx, "forms", &ids, seq)?;
    let mut del = tx.prepare_cached("DELETE FROM forms WHERE url_id = ?1")?;
    for id in &ids {
        del.execute([id])?;
    }
    let mut insert = tx.prepare_cached(
        "INSERT OR REPLACE INTO forms (url_id, idx, action, method) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for f in &batch.forms {
        insert.execute(params![f.url_id.0, f.idx, f.action, f.method])?;
    }
    Ok(())
}

/// `minhash.sig`: die `u32`-Werte der Signatur, little-endian, lückenlos.
pub fn encode_minhash(sig: &[u32]) -> Vec<u8> {
    sig.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// Umkehrung von [`encode_minhash`]; `None` bei einer Länge, die kein Vielfaches von 4 ist.
pub fn decode_minhash(bytes: &[u8]) -> Option<Vec<u32>> {
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    Some(
        bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| u32::from_le_bytes(*c))
            .collect(),
    )
}

fn write_minhash(tx: &Transaction<'_>, batch: &WriteBatch, seq: i64) -> Result<(), StoreError> {
    if batch.minhash.is_empty() {
        return Ok(());
    }
    stamp_urls(
        tx,
        "minhash",
        &distinct(batch.minhash.iter().map(|(u, _)| u.0)),
        seq,
    )?;
    let mut insert =
        tx.prepare_cached("INSERT OR REPLACE INTO minhash (url_id, sig) VALUES (?1, ?2)")?;
    for (UrlId(id), sig) in &batch.minhash {
        insert.execute(params![id, encode_minhash(sig)])?;
    }
    Ok(())
}

fn write_api_values(tx: &Transaction<'_>, batch: &WriteBatch, seq: i64) -> Result<(), StoreError> {
    if batch.api_values.is_empty() {
        return Ok(());
    }
    stamp_urls(
        tx,
        "api_values",
        &distinct(
            batch
                .api_values
                .iter()
                .filter_map(|v| v.url_id.map(|u| u.0)),
        ),
        seq,
    )?;
    let mut upsert = tx.prepare_cached(
        "INSERT INTO api_values (url_id, source, metric, level, v_real, v_text, fetched_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
         ON CONFLICT(url_id, source, metric, level) DO UPDATE SET v_real = excluded.v_real, \
           v_text = excluded.v_text, fetched_at = excluded.fetched_at",
    )?;
    let mut unknown = tx.prepare_cached(
        "INSERT OR IGNORE INTO api_urls (url, source, first_seen) VALUES (?1, ?2, ?3)",
    )?;
    for v in &batch.api_values {
        match v.url_id {
            Some(UrlId(id)) => {
                upsert.execute(params![
                    id,
                    v.source,
                    v.metric,
                    v.level,
                    v.v_real,
                    v.v_text,
                    v.fetched_at.to_db()
                ])?;
            }
            None => {
                unknown.execute(params![v.url, v.source, v.fetched_at.to_db()])?;
            }
        }
    }
    Ok(())
}

pub(crate) fn compress(bytes: &[u8]) -> Result<Vec<u8>, StoreError> {
    zstd::stream::encode_all(bytes, 3)
        .map_err(|e| StoreError::InvalidInput(format!("HTML-Kompression: {e}")))
}
fn write_w1(tx: &Transaction<'_>, batch: &WriteBatch, seq: i64) -> Result<(), StoreError> {
    let mut stamp = tx.prepare_cached("UPDATE urls SET change_seq = ?1 WHERE id = ?2")?;
    for id in distinct(batch.cookies.iter().map(|c| c.url_id.0)) {
        if stamp.execute(params![seq, id])? == 0 {
            return Err(bad(format!("cookies: URL {id} existiert nicht")));
        }
        tx.execute("DELETE FROM cookies WHERE url_id = ?1", [id])?;
    }
    let mut cookie = tx.prepare_cached("INSERT INTO cookies (url_id, position, origin, name, value, domain, path, expires, max_age, secure, http_only, same_site, raw) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)")?;
    for c in &batch.cookies {
        cookie.execute(params![
            c.url_id.0,
            c.position,
            c.origin.as_str(),
            c.name,
            c.value,
            c.domain,
            c.path,
            c.expires,
            c.max_age,
            c.secure,
            c.http_only,
            c.same_site,
            c.raw
        ])?;
    }
    let mut discovery = tx.prepare_cached("INSERT INTO discovery_edges (seq, target_id, source_id, kind, link_type, anchor, depth) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)")?;
    for d in &batch.discoveries {
        if stamp.execute(params![seq, d.target.0])? == 0 {
            return Err(bad(format!("discovery: URL {} existiert nicht", d.target)));
        }
        discovery.execute(params![
            d.seq,
            d.target.0,
            d.source.map(|s| s.0),
            d.kind.as_str(),
            d.link_type.map(|t| t.as_str()),
            d.anchor,
            d.depth
        ])?;
    }
    Ok(())
}
