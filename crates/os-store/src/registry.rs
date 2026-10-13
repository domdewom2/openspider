//! Verwaltung der Crawl-Dateien im Crawl-Ordner (`<crawls>/<crawl_id>.ospider`, OUT-228, CRAWL-127).
//!
//! Das Verzeichnis wird nach Dateien `<crawl_id>.ospider` durchsucht (Sicherungen `….v<alt>.bak.ospider`
//! und Reparatur-Kopien `….repaired.ospider` zählen nicht). Metadaten werden **ohne Migration** und
//! schreibgeschützt gelesen; nicht lesbare Dateien (fremd, beschädigt) fehlen in der Liste und stehen in
//! [`ListingReport::skipped`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use os_model::time::{Clock, IdSource};
use os_model::{CrawlId, CrawlMode, CrawlState, EndReason, Timestamp};
use rusqlite::Connection;

use crate::error::StoreError;
use crate::files::{classify_open, open_raw_metadata};
use crate::meta::CrawlMeta;
use crate::open::{CrawlFile, OpenMode};
use crate::schema;

/// Dateiendung einer Crawl-Datei.
pub const EXTENSION: &str = "ospider";

/// Eintrag der Crawl-Liste.
#[derive(Debug, Clone)]
pub struct CrawlListing {
    /// Pfad der Datei.
    pub path: PathBuf,
    /// Metadaten (ohne Migration gelesen).
    pub meta: CrawlMeta,
    /// Anzahl der URLs in der Datei.
    pub url_count: u64,
}

/// Liste samt der Dateien, die nicht gelesen werden konnten.
#[derive(Debug, Default)]
pub struct ListingReport {
    /// Lesbare Crawls, aufsteigend nach Dateiname (UUID v7 = Anlegereihenfolge).
    pub crawls: Vec<CrawlListing>,
    /// Übersprungene Dateien mit Ursache.
    pub skipped: Vec<(PathBuf, StoreError)>,
}

/// Standarddateiname einer Crawl-Datei im Crawl-Ordner.
pub fn file_name(id: &CrawlId) -> String {
    format!("{id}.{EXTENSION}")
}

/// Pfad der Crawl-Datei zu einer Kennung im Crawl-Ordner (ohne zu prüfen, ob sie existiert).
pub fn path_for(dir: &Path, id: &CrawlId) -> PathBuf {
    dir.join(file_name(id))
}

fn is_crawl_file_name(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(&format!(".{EXTENSION}")) else {
        return false;
    };
    CrawlId::from_str(stem).is_ok()
}

fn candidate_files(dir: &Path) -> Result<Vec<PathBuf>, StoreError> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(StoreError::io(dir, e)),
    };
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| StoreError::io(dir, e))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_crawl_file_name(&name) && entry.path().is_file() {
            files.push(entry.path());
        }
    }
    files.sort();
    Ok(files)
}

/// Listet alle Crawl-Dateien im Ordner (nicht lesbare Dateien fehlen; Ursachen: [`list_crawls_report`]).
pub fn list_crawls(dir: &Path) -> Result<Vec<CrawlListing>, StoreError> {
    Ok(list_crawls_report(dir)?.crawls)
}

/// Wie [`list_crawls`], aber mit den übersprungenen Dateien und ihrer Ursache.
pub fn list_crawls_report(dir: &Path) -> Result<ListingReport, StoreError> {
    let mut report = ListingReport::default();
    for path in candidate_files(dir)? {
        match read_listing(&path) {
            Ok(listing) => report.crawls.push(listing),
            Err(e) => report.skipped.push((path, e)),
        }
    }
    Ok(report)
}

/// Liest Metadaten und URL-Anzahl einer Datei, ohne sie zu migrieren oder zu verändern.
pub fn read_listing(path: &Path) -> Result<CrawlListing, StoreError> {
    let conn = open_raw_metadata(path)?;
    let read = || -> Result<(CrawlMeta, i64), StoreError> {
        let meta = read_meta(&conn)?;
        let url_count: i64 = conn.query_row("SELECT COUNT(*) FROM urls", [], |r| r.get(0))?;
        Ok((meta, url_count))
    };
    let (meta, url_count) = read().map_err(|e| classify_error(path, e))?;
    Ok(CrawlListing {
        path: path.to_path_buf(),
        meta,
        url_count: u64::try_from(url_count).unwrap_or(0),
    })
}

/// Sucht die Datei zu einer Crawl-Kennung: zuerst `<dir>/<id>.ospider`, danach über die Kennung in den
/// Metadaten anderer Crawl-Dateien (z. B. nach manuellem Umbenennen).
pub fn find(dir: &Path, id: &CrawlId) -> Option<PathBuf> {
    let direct = path_for(dir, id);
    if direct.is_file() {
        return Some(direct);
    }
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let is_plain = path.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
            n.ends_with(".ospider")
                && !n.ends_with(".bak.ospider")
                && !n.ends_with(".repaired.ospider")
        });
        if !is_plain || !path.is_file() {
            continue;
        }
        if let Ok(conn) = open_raw_metadata(&path) {
            let stored: rusqlite::Result<String> = conn.query_row(
                "SELECT value FROM crawl_meta WHERE key = 'crawl_id'",
                [],
                |r| r.get(0),
            );
            if stored.ok().and_then(|s| CrawlId::from_str(&s).ok()) == Some(*id) {
                return Some(path);
            }
        }
    }
    None
}

/// Importiert eine Crawl-Datei in den Crawl-Ordner (CRAWL-127, OUT-228): Die Quelle wird als eigenständige
/// Kopie nach `<dir>/<crawl_id>.ospider` geschrieben (ohne `-wal`/`-shm`). Existiert die Kennung dort
/// bereits, erhält die Kopie eine neue Kennung aus `ids`. Eine Datei, die keine OpenSpider-Crawl-Datei ist,
/// wird abgewiesen, ohne dass etwas kopiert wird. Liefert den Pfad der importierten Datei.
pub fn import_file(
    src: &Path,
    dir: &Path,
    ids: &dyn IdSource,
    clock: &dyn Clock,
) -> Result<PathBuf, StoreError> {
    let source = open_raw_metadata(src)?;
    let meta = read_meta(&source).map_err(|e| classify_error(src, e))?;

    let mut id = meta.crawl_id;
    let renamed = find(dir, &id).is_some();
    if renamed {
        loop {
            id = ids.new_crawl_id(clock.now());
            if find(dir, &id).is_none() {
                break;
            }
        }
    }
    let dest = path_for(dir, &id);

    // Die Kopie entsteht über die Verbindung auf der Quelle; ältere Schemaversionen bleiben ältere
    // Schemaversionen (sie werden erst beim Öffnen migriert).
    std::fs::create_dir_all(dir).map_err(|e| StoreError::io(dir, e))?;
    if dest.exists() {
        return Err(StoreError::AlreadyExists(dest));
    }
    let tmp = {
        let mut name = dest.as_os_str().to_owned();
        name.push(".importing");
        PathBuf::from(name)
    };
    let _ = std::fs::remove_file(&tmp);
    let copied = source
        .execute("VACUUM INTO ?1", [tmp.to_string_lossy().as_ref()])
        .map_err(StoreError::from)
        .and_then(|_| {
            if renamed {
                let copy = Connection::open(&tmp)?;
                copy.execute(
                    "UPDATE crawl_meta SET value = ?1 WHERE key = 'crawl_id'",
                    [id.to_string()],
                )?;
            }
            std::fs::rename(&tmp, &dest).map_err(|e| StoreError::io(&dest, e))
        });
    if let Err(e) = copied {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(dest)
}

/// Ordnet SQLite-Fehler beim Lesen einer beschädigten Datei dem Fehlerbild „beschädigt“ zu.
fn classify_error(path: &Path, e: StoreError) -> StoreError {
    match e {
        StoreError::Sqlite(e) => classify_open(path, e),
        other => other,
    }
}

fn parse<T: FromStr<Err = os_model::ModelError>>(
    map: &HashMap<String, String>,
    key: &str,
) -> Result<T, StoreError> {
    let value = map
        .get(key)
        .ok_or_else(|| StoreError::MissingMeta(key.to_string()))?;
    value.parse::<T>().map_err(|_| StoreError::InvalidMeta {
        key: key.to_string(),
        value: value.clone(),
    })
}

fn parse_opt<T: FromStr<Err = os_model::ModelError>>(
    map: &HashMap<String, String>,
    key: &str,
) -> Result<Option<T>, StoreError> {
    match map.get(key) {
        None => Ok(None),
        Some(v) if v.is_empty() => Ok(None),
        Some(_) => parse(map, key).map(Some),
    }
}

fn parse_ts(map: &HashMap<String, String>, key: &str) -> Result<Option<Timestamp>, StoreError> {
    match map.get(key) {
        None => Ok(None),
        Some(v) if v.is_empty() => Ok(None),
        Some(v) => Timestamp::from_db(v)
            .map(Some)
            .map_err(|_| StoreError::InvalidMeta {
                key: key.to_string(),
                value: v.clone(),
            }),
    }
}

/// Liest `crawl_meta` einer beliebigen Verbindung (ohne Migration), wie [`CrawlFile::meta`].
fn read_meta(conn: &Connection) -> Result<CrawlMeta, StoreError> {
    let mut stmt = conn.prepare("SELECT key, value FROM crawl_meta")?;
    let map: HashMap<String, String> = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<Result<_, _>>()?;
    let get = |key: &str| -> Result<String, StoreError> {
        map.get(key)
            .cloned()
            .ok_or_else(|| StoreError::MissingMeta(key.to_string()))
    };
    Ok(CrawlMeta {
        crawl_id: parse::<CrawlId>(&map, "crawl_id")?,
        mode: parse::<CrawlMode>(&map, "mode")?,
        start_url: map.get("start_url").filter(|v| !v.is_empty()).cloned(),
        project: get("project")?,
        name: get("name")?,
        state: parse::<CrawlState>(&map, "state")?,
        end_reason: parse_opt::<EndReason>(&map, "end_reason")?,
        started_at: parse_ts(&map, "started_at")?,
        ended_at: parse_ts(&map, "ended_at")?,
        heartbeat_at: parse_ts(&map, "heartbeat_at")?,
        schema_version: schema::user_version(conn)?,
    })
}

impl CrawlFile {
    /// Öffnet die Datei zu einer Kennung im Crawl-Ordner (siehe [`find`]).
    pub fn open_by_id(dir: &Path, id: &CrawlId, mode: OpenMode) -> Result<CrawlFile, StoreError> {
        let path = find(dir, id).ok_or_else(|| StoreError::NotFound(path_for(dir, id)))?;
        CrawlFile::open(&path, mode)
    }
}
