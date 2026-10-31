//! Disk-first LSH: Signaturen im Crawl-Speicher, Kandidaten je Seite, exakte Jaccard-Prüfung.
use super::{Scratch, Transaction};
use crate::{analysis::CancelToken, error::AnalyzeError, minhash};
use os_model::{CrawlConfig, StepState};
use os_store::CrawlFile;
use std::collections::BTreeSet;

pub fn run(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    cancel: &CancelToken,
) -> Result<StepState, AnalyzeError> {
    if !cfg.duplicates.near_enabled {
        return Ok(StepState::NotApplicable);
    }
    cancel.set_progress(0.0);
    if cancel.checkpoint().is_err() {
        return Ok(StepState::Pending);
    }
    let profile = std::env::var_os("OPENSPIDER_NEAR_DUP_PROFILE").is_some();
    let started = std::time::Instant::now();
    let mut phases = [std::time::Duration::ZERO; 3];
    let conn = file.conn();
    // Temporäre Tabellen liegen auf Disk; Ergebniswechsel erst nach vollständigem Lauf.
    conn.execute_batch("DROP TABLE IF EXISTS temp.d02_bands; DROP TABLE IF EXISTS temp.d02_pairs; CREATE TEMP TABLE d02_bands(band INTEGER, sig BLOB, hash TEXT, url_id INTEGER); CREATE INDEX temp.d02_band_lookup ON d02_bands(band,sig,hash,url_id); CREATE TEMP TABLE d02_pairs(url_id INTEGER,other_id INTEGER,similarity REAL,PRIMARY KEY(url_id,other_id)) WITHOUT ROWID;")?;
    let mut scratch = Scratch::new(conn);
    scratch.begin(Transaction::Collect)?;
    let mut stmt=conn.prepare("SELECT u.id,m.sig,u.hash_md5 FROM urls u JOIN minhash m ON m.url_id=u.id WHERE u.is_internal=1 AND u.in_tables=1 AND u.status_code BETWEEN 200 AND 299 AND u.content_class='HTML' AND (?1=0 OR COALESCE(u.indexability,'') <> 'Non-Indexable') ORDER BY u.id")?;
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM minhash", [], |r| r.get(0))?;
    let mut rows = stmt.query([cfg.duplicates.only_indexable])?;
    let height = minhash::band_rows(cfg.duplicates.near_threshold_pct) * 4;
    let band_values = (0..512 / height)
        .map(|band| {
            let parameter = band * 2 + 3;
            format!("(?{parameter},?{},?1,?2)", parameter + 1)
        })
        .collect::<Vec<_>>()
        .join(",");
    // Eine vorbereitete Sammelanweisung je Signatur statt einer je Band.
    let mut insert_bands =
        conn.prepare(&format!("INSERT INTO temp.d02_bands VALUES {band_values}"))?;
    let mut processed = 0;
    while let Some(row) = rows.next()? {
        if cancel.checkpoint().is_err() {
            return Ok(StepState::Pending);
        }
        let id: i64 = row.get(0)?;
        let sig: Vec<u8> = row.get(1)?;
        let hash: Option<String> = row.get(2)?;
        if sig.len() != 512 {
            continue;
        }
        let phase = std::time::Instant::now();
        let mut candidates = BTreeSet::new();
        // Gleiche, nichtleere HTML-Hashes sind keine Near-Paare. Zwei Indexbereiche
        // überspringen sie bereits bei der Kandidatensuche: auch Millionen exakter
        // Kopien erzeugen so keine quadratische Anzahl verworfener Kandidaten.
        let candidate_sql = if hash.as_ref().is_some_and(|h| !h.is_empty()) {
            "SELECT url_id FROM temp.d02_bands WHERE band=?1 AND sig=?2 AND hash<?3 UNION ALL SELECT url_id FROM temp.d02_bands WHERE band=?1 AND sig=?2 AND hash>?3"
        } else {
            "SELECT url_id FROM temp.d02_bands WHERE band=?1 AND sig=?2 AND ?3=''"
        };
        let mut find = conn.prepare_cached(candidate_sql)?;
        for (band, bytes) in sig.chunks(height).enumerate() {
            for candidate in find.query_map(
                rusqlite::params![band as i64, bytes, hash.as_deref().unwrap_or("")],
                |r| r.get::<_, i64>(0),
            )? {
                if cancel.checkpoint().is_err() {
                    return Ok(StepState::Pending);
                }
                candidates.insert(candidate?);
            }
        }
        phases[0] += phase.elapsed();
        let phase = std::time::Instant::now();
        let mut band_params = vec![
            rusqlite::types::Value::Text(hash.clone().unwrap_or_default()),
            rusqlite::types::Value::Integer(id),
        ];
        for (band, bytes) in sig.chunks(height).enumerate() {
            band_params.push(rusqlite::types::Value::Integer(band as i64));
            band_params.push(rusqlite::types::Value::Blob(bytes.to_vec()));
        }
        insert_bands.execute(rusqlite::params_from_iter(band_params))?;
        phases[1] += phase.elapsed();
        let phase = std::time::Instant::now();
        if !candidates.is_empty() {
            let text: String = conn
                .query_row(
                    "SELECT v_text FROM url_ext WHERE url_id=?1 AND key='content.text'",
                    [id],
                    |r| r.get(0),
                )
                .unwrap_or_default();
            let own = minhash::shingles(&text);
            for candidate in candidates {
                if cancel.checkpoint().is_err() {
                    return Ok(StepState::Pending);
                }
                let (other_hash,text):(Option<String>,String)=conn.query_row("SELECT u.hash_md5,COALESCE(e.v_text,'') FROM urls u LEFT JOIN url_ext e ON e.url_id=u.id AND e.key='content.text' WHERE u.id=?1",[candidate],|r|Ok((r.get(0)?,r.get(1)?)))?;
                if hash
                    .as_ref()
                    .is_some_and(|h| !h.is_empty() && Some(h) == other_hash.as_ref())
                {
                    continue;
                }
                let similarity = minhash::jaccard(&own, &minhash::shingles(&text));
                if similarity * 100.0 + 1e-10 >= f64::from(cfg.duplicates.near_threshold_pct) {
                    for (a, b) in [(id, candidate), (candidate, id)] {
                        conn.execute(
                            "INSERT INTO temp.d02_pairs VALUES (?1,?2,?3)",
                            rusqlite::params![a, b, similarity],
                        )?;
                    }
                }
            }
        }
        phases[2] += phase.elapsed();
        processed += 1;
        cancel.set_progress(processed as f32 / total.max(1) as f32);
    }
    if cancel.checkpoint().is_err() {
        return Ok(StepState::Pending);
    }
    drop(rows);
    drop(stmt);
    scratch.release()?;
    if cancel.checkpoint().is_err() {
        return Ok(StepState::Pending);
    }
    scratch.begin(Transaction::Publish)?;
    conn.execute_batch(
        "DELETE FROM near_duplicates; INSERT INTO near_duplicates SELECT * FROM temp.d02_pairs;",
    )?;
    scratch.release()?;
    if profile {
        eprintln!(
            "n16 baseline total={:?} band_lookup={:?} band_write={:?} compare_write={:?}",
            started.elapsed(),
            phases[0],
            phases[1],
            phases[2]
        );
    }
    cancel.set_progress(1.0);
    Ok(StepState::Done)
}
