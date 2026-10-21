//! Projektbezogener Auto-Vergleich (OUT-152, TECH-308, CRAWL-036).
//! Ein separates SQLite-Artefakt enthält Konfiguration, Klassifikationen und Change-Detection-Ergebnisse.
use super::super::{CompareError, Comparison, CrawlSide};
use os_model::{CancelToken, CompareConfig, CrawlState, EndReason};
use os_store::{CrawlFile, OpenMode};
use std::path::{Path, PathBuf};

/// Kein Vorlauf ist ein reguläres Ergebnis, kein Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoResult {
    NoPrevious,
    Saved(PathBuf),
}

/// Vergleicht mit dem unmittelbar vorherigen, erfolgreichen und vollständigen Projektcrawl.
/// Wiederholung für dasselbe Paar und dieselbe Konfiguration erzeugt keinen zweiten Eintrag.
pub fn run(
    current: &Path,
    crawls: &Path,
    output: &Path,
    config: Option<&CompareConfig>,
    cancel: &CancelToken,
) -> Result<AutoResult, CompareError> {
    super::checkpoint(cancel)?;
    let current_file = CrawlFile::open(current, OpenMode::ReadWrite)?;
    let meta = current_file.meta()?;
    if meta.project.trim().is_empty() {
        return Err(CompareError::Invalid(
            "auto comparison requires a project".into(),
        ));
    }
    if meta.state != CrawlState::Completed || meta.end_reason != Some(EndReason::QueueEmpty) {
        return Err(CompareError::Invalid(
            "auto comparison requires a successfully completed crawl".into(),
        ));
    }
    let config = config.cloned().unwrap_or(current_file.config()?.compare);
    let mut candidates = os_store::registry::list_crawls(crawls)?;
    candidates.retain(|c| {
        c.meta.project == meta.project
            && c.meta.crawl_id != meta.crawl_id
            && c.meta.started_at < meta.started_at
            && c.meta.state == CrawlState::Completed
            && c.meta.end_reason == Some(EndReason::QueueEmpty)
            && c.queued == 0
    });
    candidates.sort_by_key(|c| (c.meta.started_at, c.meta.crawl_id.to_string()));
    let Some(previous) = candidates.pop() else {
        current_file.set_meta("compare.auto_state", "no_previous")?;
        tracing::info!(project=%meta.project,"kein vorheriger Crawl");
        return Ok(AutoResult::NoPrevious);
    };
    let cfg = current_file.config()?;
    let analysis = crate::analysis::ensure_analysis(&current_file, &cfg, cancel)
        .map_err(|e| CompareError::Invalid(e.to_string()))?;
    if !analysis.complete {
        return Err(CompareError::Cancelled);
    }
    drop(current_file);
    std::fs::create_dir_all(output).map_err(|e| CompareError::Invalid(e.to_string()))?;
    let config_json =
        serde_json::to_string(&config).map_err(|e| CompareError::Invalid(e.to_string()))?;
    // FNV-1a für eine plattformunabhängige Artefaktkennung; keine Zufalls-/Zeitabhängigkeit.
    let hash = config_json.bytes().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
    });
    let path = output.join(format!(
        "{}-{}-{hash:016x}.oscompare",
        meta.crawl_id, previous.meta.crawl_id
    ));
    if path.exists() {
        let db = rusqlite::Connection::open_with_flags(
            &path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        let stored: String =
            db.query_row("SELECT value FROM metadata WHERE key='config'", [], |r| {
                r.get(0)
            })?;
        if stored != config_json {
            return Err(CompareError::Invalid(
                "comparison identity collision".into(),
            ));
        }
        let file = CrawlFile::open(current, OpenMode::ReadWrite)?;
        file.set_meta("compare.latest", &path.to_string_lossy())?;
        file.set_meta("compare.auto_state", "available")?;
        return Ok(AutoResult::Saved(path));
    }
    let a = CrawlSide::open(current, output)?;
    let b = CrawlSide::open(&previous.path, output)?;
    let mut comparison = Comparison::open(a, b, &config.mapping, cancel)?;
    super::calculate(&comparison, &config.change_detection, cancel)?;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| CompareError::Invalid(e.to_string()))?;
    let mut cleanup = ArtifactCleanup {
        path: path.clone(),
        armed: true,
    };
    let db = rusqlite::Connection::open(&path)?;
    let tx = db.unchecked_transaction()?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL); CREATE TABLE IF NOT EXISTS classes(tab TEXT,filter TEXT,class TEXT,url_id INTEGER,PRIMARY KEY(tab,filter,class,url_id)); CREATE TABLE IF NOT EXISTS changes(url_id INTEGER,element TEXT,current,previous,delta REAL,pct REAL,changed INTEGER,PRIMARY KEY(url_id,element)); CREATE TABLE IF NOT EXISTS population(url_id INTEGER PRIMARY KEY,previous_id INTEGER,previous_address TEXT); DELETE FROM metadata;DELETE FROM classes;DELETE FROM changes;DELETE FROM population;")?;
    for (key, value) in [
        ("current", meta.crawl_id.to_string()),
        ("previous", previous.meta.crawl_id.to_string()),
        ("current_path", current.to_string_lossy().into_owned()),
        (
            "previous_path",
            previous.path.to_string_lossy().into_owned(),
        ),
        ("config", config_json),
        (
            "name",
            format!("{} – {} vs {}", meta.project, meta.name, previous.meta.name),
        ),
    ] {
        tx.execute(
            "INSERT INTO metadata VALUES(?1,?2)",
            rusqlite::params![key, value],
        )?;
    }
    let registry =
        crate::tabs::configured_for_file(&comparison.current().config, &comparison.current().file);
    for tab in &registry {
        if tab.id == super::super::CHANGE_DETECTION_TAB {
            continue;
        }
        for filter in &tab.filters {
            super::checkpoint(cancel)?;
            if filter.is_stub() {
                continue;
            }
            let Some(classes) = comparison.classes(tab.id, filter.id)? else {
                continue;
            };
            for (class, ids) in [
                ("Added", &classes.added),
                ("New", &classes.new),
                ("Removed", &classes.removed),
                ("Missing", &classes.missing),
            ] {
                let mut insert = tx.prepare_cached("INSERT INTO classes VALUES(?1,?2,?3,?4)")?;
                for id in ids.iter() {
                    insert.execute(rusqlite::params![tab.id, filter.id, class, id])?;
                }
            }
        }
    }
    let conn = comparison.current().file.conn();
    let mut rows=conn.prepare("SELECT url_id,element,current,previous,delta,pct,changed FROM temp.c02_cells ORDER BY url_id,element")?;
    for row in rows.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, rusqlite::types::Value>(2)?,
            r.get::<_, rusqlite::types::Value>(3)?,
            r.get::<_, Option<f64>>(4)?,
            r.get::<_, Option<f64>>(5)?,
            r.get::<_, bool>(6)?,
        ))
    })? {
        let (id, e, c, p, d, pct, changed) = row?;
        tx.execute(
            "INSERT INTO changes VALUES(?1,?2,?3,?4,?5,?6,?7)",
            rusqlite::params![id, e, c, p, d, pct, changed],
        )?;
    }
    let mut rows =
        conn.prepare("SELECT url_id,previous_id,previous_address FROM temp.c02_population")?;
    for row in rows.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, String>(2)?,
        ))
    })? {
        let (id, p, a) = row?;
        tx.execute(
            "INSERT INTO population VALUES(?1,?2,?3)",
            rusqlite::params![id, p, a],
        )?;
    }
    super::checkpoint(cancel)?;
    let counts = super::counts(&comparison, &config.change_detection, false)?;
    tx.execute(
        "INSERT INTO metadata VALUES('change_counts',?1)",
        [serde_json::to_string(&counts).map_err(|e| CompareError::Invalid(e.to_string()))?],
    )?;
    tx.commit()?;
    cleanup.armed = false;
    let file = CrawlFile::open(current, OpenMode::ReadWrite)?;
    file.set_meta("compare.latest", &path.to_string_lossy())?;
    file.set_meta("compare.auto_state", "available")?;
    Ok(AutoResult::Saved(path))
}

/// Lädt ein gespeichertes Ergebnis einschließlich wirksamer Konfiguration ohne neue Änderungserkennung.
pub fn load(
    path: &Path,
    cancel: &CancelToken,
) -> Result<(Comparison, CompareConfig), CompareError> {
    let db =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let get = |key: &str| {
        db.query_row("SELECT value FROM metadata WHERE key=?1", [key], |r| {
            r.get::<_, String>(0)
        })
    };
    let config: CompareConfig =
        serde_json::from_str(&get("config")?).map_err(|e| CompareError::Invalid(e.to_string()))?;
    let work = path
        .parent()
        .ok_or_else(|| CompareError::Invalid("comparison directory missing".into()))?;
    let c = Comparison::open(
        CrawlSide::open(Path::new(&get("current_path")?), work)?,
        CrawlSide::open(Path::new(&get("previous_path")?), work)?,
        &config.mapping,
        cancel,
    )?;
    let conn = c.current().file.conn();
    conn.execute(
        "ATTACH DATABASE ?1 AS c02_saved",
        [path.to_string_lossy().as_ref()],
    )?;
    let result=conn.execute_batch("CREATE TEMP TABLE c02_population AS SELECT * FROM c02_saved.population; CREATE UNIQUE INDEX temp.c02_population_id ON c02_population(url_id); CREATE TEMP TABLE c02_cells AS SELECT * FROM c02_saved.changes; CREATE UNIQUE INDEX temp.c02_cells_id ON c02_cells(url_id,element);");
    let detached = conn.execute_batch("DETACH DATABASE c02_saved");
    result?;
    detached?;
    Ok((c, config))
}

/// Mail-Inhalt aus demselben gespeicherten Ergebnis wie der manuelle Vergleich.
pub fn saved_counts(
    path: &Path,
    skip_empty: bool,
) -> Result<Vec<super::ChangeCount>, CompareError> {
    let db =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let json: String = db.query_row(
        "SELECT value FROM metadata WHERE key='change_counts'",
        [],
        |r| r.get(0),
    )?;
    let mut counts: Vec<super::ChangeCount> =
        serde_json::from_str(&json).map_err(|e| CompareError::Invalid(e.to_string()))?;
    if skip_empty {
        counts.retain(|c| c.change != 0);
    }
    Ok(counts)
}

/// Abschlussmail: None bedeutet „kein vorheriger Crawl“, sonst die gespeicherte Änderungstabelle.
pub fn notification_counts(
    file: &CrawlFile,
    skip_empty: bool,
) -> Result<Option<Vec<super::ChangeCount>>, CompareError> {
    match file.get_meta("compare.latest")? {
        Some(path) => saved_counts(Path::new(&path), skip_empty).map(Some),
        None => Ok(None),
    }
}

struct ArtifactCleanup {
    path: PathBuf,
    armed: bool,
}
impl Drop for ArtifactCleanup {
    fn drop(&mut self) {
        if self.armed {
            for suffix in ["", "-journal", "-wal", "-shm"] {
                let mut path = self.path.as_os_str().to_owned();
                path.push(suffix);
                let _ = std::fs::remove_file(PathBuf::from(path));
            }
        }
    }
}
