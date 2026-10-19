//! Deterministische, direkt über os-store angelegte UI-Lastfixture (UI-148, K-19).
//!
//! `cargo run --release -p os-loadtest --bin gen-ui-large -- --output target/ui.ospider`
//! erzeugt 5 Mio. interne HTML-Seiten mit je 20 echten Hyperlinks. `--rows 100000`
//! ist die CI-Variante. URL 1 hat min(N−1, 1 Mio.) verschiedene Inlink-Quellen;
//! URL 2 enthält genau 15 MiB rohes HTML. Alle Zeiten und IDs sind seedfest.
//! Sortierdaten enthalten NULL, Leertext, Duplikate, Unicode und lange Texte.
//! Die Tiefe (`crawl_depth`) ist ein Verteilungswert, nicht die Graph-Distanz:
//! Außerhalb des Hubs gilt 1 + r % 4, unabhängig von der Ringposition. Damit
//! eignet sich die Fixture für Tiefenfilter, nicht für Distanzprüfungen (UI-066).
//!
//! Speicher: 10 000 URLs/200 000 Links pro SQL-Transaktion, 64 MiB SQLite-Cache,
//! temporäre Sortierung auf Disk. Sekundärindizes entstehen nach dem Laden;
//! die fünf Textindizes entfallen bei großen Dateien gemäß Store K-83.
//! Die aktuelle Store-Migration (mindestens v3, derzeit v4) bleibt unverändert.
//! Eine private temporäre Datei wird erst nach Checkpoint und Schließen ohne
//! Überschreiben veröffentlicht. Ein Fehler hinterlässt keine halbe Zieldatei.
//! `synchronous=OFF` gilt nur im Staging-Ordner; bei Abbruch wird diese Datei
//! verworfen. Vor der Veröffentlichung wird wieder auf `NORMAL` gestellt.

use std::error::Error;
use std::path::Path;
use std::time::Instant;

use os_model::time::SeededIds;
use os_model::{CrawlConfig, CrawlMode, IdSource, Timestamp};
use os_store::{CrawlFile, CrawlInit, StorageProfile};
use serde::Serialize;

pub mod cli;

pub const DEFAULT_ROWS: u64 = 5_000_000;
pub const DEFAULT_SEED: u64 = 19;
pub const LINKS_PER_PAGE: u64 = 20;
pub const HTML_BYTES: usize = 15 * 1024 * 1024;
const BATCH_ROWS: u64 = 10_000;
type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Debug, Serialize)]
pub struct Report {
    pub rows: u64,
    pub links: u64,
    pub seed: u64,
    pub hub_inlinks: u64,
    pub html_bytes: usize,
    pub schema_version: u32,
    pub elapsed_ms: u128,
    pub file_bytes: u64,
}

/// Generate a closed, independently readable fixture, without replacing existing files.
/// `progress` receives the phase and number of URLs already written.
pub fn generate(
    output: &Path,
    rows: u64,
    seed: u64,
    mut progress: impl FnMut(&str, u64),
) -> Result<Report> {
    // At least 22 rows keep the 20 ring targets distinct and exclude self-links.
    // The upper bound also keeps every SQL integer product below i64::MAX.
    if !(22..=DEFAULT_ROWS).contains(&rows) {
        return Err("--rows muss zwischen 22 und 5000000 liegen".into());
    }
    if output.try_exists()? {
        return Err(format!("Zieldatei existiert bereits: {}", output.display()).into());
    }
    let started = Instant::now();
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let staging = tempfile::Builder::new()
        .prefix("gen-ui-large-")
        .tempdir_in(parent)?;
    let path = staging.path().join("crawl.ospider");
    let at = Timestamp::from_unix_millis(1_760_227_200_000);
    let mut config = CrawlConfig {
        generator: "gen-ui-large v1".into(),
        ..CrawlConfig::default()
    };
    config.extraction.store_html = true;
    let init = CrawlInit {
        crawl_id: SeededIds::new(seed).new_crawl_id(at),
        mode: CrawlMode::Spider,
        start_url: Some("https://ui.test/".into()),
        list_source: None,
        project: "UI benchmarks".into(),
        name: format!("gen-ui-large n={rows} seed={seed}"),
        config,
        created_at: at,
    };
    let file = CrawlFile::create(&path, &init, StorageProfile::MinMemory)?;
    let conn = file.conn();
    conn.execute_batch("PRAGMA synchronous=OFF; PRAGMA temp_store=FILE; PRAGMA threads=2;")?;
    let indexes = conn
        .prepare(
            "SELECT name, sql FROM sqlite_master WHERE type='index' AND sql IS NOT NULL
         AND tbl_name IN ('urls','links','url_counters') ORDER BY name",
        )?
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for (name, _) in &indexes {
        conn.execute_batch(&format!("DROP INDEX \"{name}\""))?;
    }
    // Retain the exact schema definition, rather than duplicating the Store trigger.
    let trigger: String = conn.query_row(
        "SELECT sql FROM sqlite_master WHERE name='urls_counters_insert'",
        [],
        |r| r.get(0),
    )?;
    conn.execute_batch(
        "DROP TRIGGER urls_counters_insert;
         CREATE TEMP TABLE numbers(n INTEGER PRIMARY KEY);
         WITH RECURSIVE n(x) AS (VALUES(0) UNION ALL SELECT x+1 FROM n WHERE x<9999)
         INSERT INTO numbers SELECT x FROM n;
         CREATE TEMP TABLE slots(n INTEGER PRIMARY KEY);
         INSERT INTO slots SELECT n FROM numbers WHERE n<20;",
    )?;
    let hub = (rows - 1).min(1_000_000);
    // The whole u64 seed is preserved in the identity and metadata. Fold both halves
    // for the bounded integer PRNG used by SQLite's column distributions.
    let sql_seed = ((seed ^ (seed >> 32)) & 0x7fff_ffff) as i64;
    for first in (1..=rows).step_by(BATCH_ROWS as usize) {
        let last = (first + BATCH_ROWS - 1).min(rows);
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            include_str!("urls.sql"),
            [first as i64, last as i64, sql_seed],
        )?;
        tx.execute(
            include_str!("links.sql"),
            [first as i64, last as i64, rows as i64, hub as i64],
        )?;
        tx.execute(
            include_str!("counters.sql"),
            [first as i64, last as i64, rows as i64, hub as i64],
        )?;
        tx.commit()?;
        progress("rows", last);
    }
    // Raw HTML is intentionally not compressed: this stresses the detail loader
    // and remains readable through stored_html's legacy/raw branch.
    let mut html = format!("<!doctype html><html><head><title>UI seed {seed}</title></head><body>");
    html.extend(std::iter::repeat_n(
        'x',
        HTML_BYTES - html.len() - "</body></html>".len(),
    ));
    html.push_str("</body></html>");
    conn.execute(
        "INSERT INTO page_html(url_id,encoding,body) VALUES(2,'UTF-8',?1)",
        [html.as_bytes()],
    )?;
    conn.execute_batch(&trigger)?;
    for (name, sql) in &indexes {
        if rows >= os_store::url_index::DROP_FROM_URLS as u64
            && os_store::url_index::WRITE_PHASE_DROPPED
                .iter()
                .any(|i| i.name == name)
        {
            continue;
        }
        progress(name, rows);
        conn.execute_batch(sql)?;
    }
    let tx = conn.unchecked_transaction()?;
    for (key, value) in [
        ("state", "Completed".into()),
        ("end_reason", "queue_empty".into()),
        ("started_at", at.to_db()),
        ("ended_at", at.to_db()),
        ("next_url_id", (rows + 1).to_string()),
        ("next_seq", (rows * LINKS_PER_PAGE + 1).to_string()),
        ("change_seq", "1".into()),
        ("owner_kind", "cli".into()),
        ("gen_ui.seed", seed.to_string()),
        ("gen_ui.rows", rows.to_string()),
        ("gen_ui.version", "1".into()),
    ] {
        tx.execute(
            "INSERT OR REPLACE INTO crawl_meta(key,value) VALUES(?1,?2)",
            [key, &value],
        )?;
    }
    tx.commit()?;
    progress("checkpoint", rows);
    conn.execute_batch("PRAGMA synchronous=NORMAL; PRAGMA wal_checkpoint(TRUNCATE);")?;
    drop(file);
    let file_bytes = std::fs::metadata(&path)?.len();
    // Same directory/filesystem; persist_noclobber also handles an output created
    // by another process after the initial existence check.
    tempfile::TempPath::try_from_path(path)?.persist_noclobber(output)?;
    Ok(Report {
        rows,
        links: rows * LINKS_PER_PAGE,
        seed,
        hub_inlinks: hub,
        html_bytes: HTML_BYTES,
        schema_version: os_store::SCHEMA_VERSION,
        elapsed_ms: started.elapsed().as_millis(),
        file_bytes,
    })
}
