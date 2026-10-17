//! Crawl-Datei anlegen und öffnen (OUT-230, CRAWL-127): Kennung, Versionsprüfung, Migration mit Sicherung.

use std::io::Read;
use std::path::{Path, PathBuf};

use os_model::time::SystemClock;
use os_model::{Clock, CrawlConfig, CrawlId, CrawlMode, CrawlState, Timestamp};
use rusqlite::{Connection, ErrorCode, OpenFlags};

use crate::collation;
use crate::error::StoreError;
use crate::profile::{self, StorageProfile};
use crate::schema::{self, Migration, APPLICATION_ID, MIGRATIONS};

/// Öffnungsmodus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenMode {
    /// Nur lesen (kein Schreiben, keine Migration).
    ReadOnly,
    /// Lesen und schreiben (migriert ältere Dateien).
    ReadWrite,
}

/// Angaben zum Anlegen einer neuen Crawl-Datei.
#[derive(Debug, Clone)]
pub struct CrawlInit {
    /// Kennung des Crawls.
    pub crawl_id: CrawlId,
    /// Modus.
    pub mode: CrawlMode,
    /// Start-URL (Spider-Modus).
    pub start_url: Option<String>,
    /// Quelle der URL-Liste (Listenmodus).
    pub list_source: Option<String>,
    /// Projektname.
    pub project: String,
    /// Crawl-Name.
    pub name: String,
    /// Wirksame Konfiguration (wird eingebettet).
    pub config: CrawlConfig,
    /// Anlegezeitpunkt.
    pub created_at: Timestamp,
}

/// Eine geöffnete Crawl-Datei (SQLite).
pub struct CrawlFile {
    path: PathBuf,
    conn: Connection,
}

const SQLITE_MAGIC: &[u8; 16] = b"SQLite format 3\0";

fn classify(path: &Path, e: rusqlite::Error) -> StoreError {
    if let rusqlite::Error::SqliteFailure(f, msg) = &e {
        match f.code {
            ErrorCode::NotADatabase => {
                return StoreError::NotACrawlFile {
                    path: path.to_path_buf(),
                }
            }
            ErrorCode::DatabaseCorrupt => {
                return StoreError::Corrupt {
                    path: path.to_path_buf(),
                    detail: msg.clone().unwrap_or_else(|| e.to_string()),
                }
            }
            _ => {}
        }
    }
    StoreError::Sqlite(e)
}

/// Ob neben `path` eine nicht leere `<datei>-wal` liegt (Frames, die beim Öffnen wiederhergestellt werden).
fn wal_has_frames(path: &Path) -> bool {
    let mut name = path.as_os_str().to_owned();
    name.push("-wal");
    std::fs::metadata(std::path::PathBuf::from(name)).is_ok_and(|m| m.len() > 0)
}

/// Prüft die SQLite-Kopfzeile: `Ok(false)`, wenn die Datei nicht mit dem SQLite-Magic beginnt (auch bei
/// kürzeren Dateien); `Err(Corrupt)`, wenn die Datei kürzer ist als der Kopf angibt (abgeschnitten) und keine nicht leere `-wal`-Datei daneben liegt.
fn check_sqlite_header(path: &Path) -> Result<bool, StoreError> {
    let mut f = std::fs::File::open(path).map_err(|e| StoreError::io(path, e))?;
    let mut head = [0u8; 100];
    let mut read = 0;
    while read < head.len() {
        match f.read(&mut head[read..]) {
            Ok(0) => break,
            Ok(n) => read += n,
            Err(e) => return Err(StoreError::io(path, e)),
        }
    }
    if read < SQLITE_MAGIC.len() || &head[..16] != SQLITE_MAGIC {
        return Ok(false);
    }
    if read < head.len() {
        return Err(StoreError::Corrupt {
            path: path.to_path_buf(),
            detail: "Datei abgeschnitten (Kopfbereich unvollständig)".to_string(),
        });
    }
    // Größe laut Kopf (Seitenzahl × Seitengröße) ist nur gültig, wenn „version-valid-for“ dem
    // Änderungszähler entspricht (SQLite-Dateiformat, Abschnitt 1.3).
    let page_size = match u16::from_be_bytes([head[16], head[17]]) {
        1 => 65_536u64,
        n => u64::from(n),
    };
    let page_count = u64::from(u32::from_be_bytes([head[28], head[29], head[30], head[31]]));
    let change_counter = u32::from_be_bytes([head[24], head[25], head[26], head[27]]);
    let valid_for = u32::from_be_bytes([head[92], head[93], head[94], head[95]]);
    // Neben einer nicht leeren `-wal`-Datei gilt der Kopf der Hauptdatei nicht: Ein harter Abbruch mitten im
    // WAL-Checkpoint hinterlässt eine Hauptdatei, deren Kopf schon die neue Seitenzahl nennt, während die Seiten
    // noch zum Teil in der `-wal`-Datei stehen. SQLite nimmt die Größe dann aus dem letzten Commit der
    // `-wal`-Datei und stellt die Datei beim Öffnen wieder her (PERF-005, CRAWL-125). Ohne `-wal`-Datei ist die
    // Hauptdatei allein maßgeblich und eine zu kurze Datei abgeschnitten.
    if page_count > 0 && change_counter == valid_for && !wal_has_frames(path) {
        let actual = f.metadata().map_err(|e| StoreError::io(path, e))?.len();
        if actual < page_count * page_size {
            return Err(StoreError::Corrupt {
                path: path.to_path_buf(),
                detail: format!(
                    "Datei abgeschnitten ({actual} statt {} Bytes)",
                    page_count * page_size
                ),
            });
        }
    }
    Ok(true)
}

/// Erzeuger-Angabe aus der Datei (für die Meldung bei zu neuer Version), falls lesbar.
fn read_generator(conn: &Connection) -> String {
    conn.query_row(
        "SELECT value FROM crawl_meta WHERE key = 'generator'",
        [],
        |r| r.get::<_, String>(0),
    )
    .unwrap_or_else(|_| "einer neueren OpenSpider-Version".to_string())
}

impl CrawlFile {
    /// Legt eine neue Crawl-Datei an (Schema, Kennung, Metadaten, Konfiguration). Eine vorhandene Datei wird
    /// nie überschrieben.
    pub fn create(
        path: &Path,
        init: &CrawlInit,
        profile: StorageProfile,
    ) -> Result<Self, StoreError> {
        if path.exists() {
            return Err(StoreError::AlreadyExists(path.to_path_buf()));
        }
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|e| StoreError::io(parent, e))?;
        }
        match Self::create_inner(path, init, profile) {
            Ok(f) => Ok(f),
            Err(e) => {
                // Keine halb angelegte Datei zurücklassen.
                for suffix in ["", "-wal", "-shm"] {
                    let mut p = path.as_os_str().to_owned();
                    p.push(suffix);
                    let _ = std::fs::remove_file(PathBuf::from(p));
                }
                Err(e)
            }
        }
    }

    fn create_inner(
        path: &Path,
        init: &CrawlInit,
        profile: StorageProfile,
    ) -> Result<Self, StoreError> {
        let mut conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;
        collation::ensure(&conn)?;
        profile::apply(&conn, profile)?;
        conn.pragma_update(None, "application_id", APPLICATION_ID)?;
        schema::migrate(&mut conn, 0, MIGRATIONS)?;

        let tx = conn.transaction()?;
        {
            let mut put = tx.prepare("INSERT INTO crawl_meta (key, value) VALUES (?1, ?2)")?;
            let mut rows: Vec<(&str, String)> = vec![
                ("crawl_id", init.crawl_id.to_string()),
                (
                    "schema_version",
                    schema::latest_version(MIGRATIONS).to_string(),
                ),
                ("generator", init.config.generator.clone()),
                ("created_at", init.created_at.to_db()),
                ("mode", init.mode.as_str().to_string()),
                ("project", init.project.clone()),
                ("name", init.name.clone()),
                ("state", CrawlState::Running.as_str().to_string()),
                ("next_url_id", "1".to_string()),
                ("next_seq", "1".to_string()),
                ("analysis_stale", "0".to_string()),
                // Änderungsfeed (P00c, 3.9): der Writer erhöht den Wert je Batch.
                ("change_seq", "0".to_string()),
                // Manifest (OUT-233, M2-F02): anlegendes und zuletzt schreibendes Programm.
                ("openspider_version", crate::meta::OPENSPIDER_VERSION.to_string()),
                ("last_writer_version", crate::meta::OPENSPIDER_VERSION.to_string()),
            ];
            if let Some(u) = &init.start_url {
                rows.push(("start_url", u.clone()));
            }
            if let Some(l) = &init.list_source {
                rows.push(("list_source", l.clone()));
            }
            for (k, v) in rows {
                put.execute([k, v.as_str()])?;
            }
        }
        let osconfig = init.config.to_toml_string()?;
        tx.execute(
            "INSERT INTO crawl_config (id, osconfig, updated_at) VALUES (1, ?1, ?2)",
            [&osconfig, &init.created_at.to_db()],
        )?;
        crate::meta::stamp_config(&tx, &osconfig)?;
        tx.commit()?;
        // Kennung und Schema stehen damit auch ohne WAL-Datei im Dateikopf (z. B. nach einem Absturz).
        conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
        Ok(CrawlFile {
            path: path.to_path_buf(),
            conn,
        })
    }

    /// Öffnet eine vorhandene Crawl-Datei. Fremde Dateien und Dateien mit neuerer Schemaversion werden
    /// abgewiesen, ohne sie zu verändern; ältere werden (im Modus `ReadWrite`) nach einer Sicherungskopie
    /// `<name>.v<alt>.bak.ospider` migriert.
    pub fn open(path: &Path, mode: OpenMode) -> Result<Self, StoreError> {
        Self::open_with(path, mode, MIGRATIONS)
    }

    /// Wie [`open`](Self::open) mit eigener Migrationsliste (für Tests des Migrationsrahmens).
    pub fn open_with(
        path: &Path,
        mode: OpenMode,
        migrations: &[Migration],
    ) -> Result<Self, StoreError> {
        Self::open_inner(path, mode, migrations, true)
    }

    /// Migriert eine ältere Crawl-Datei **als Kopie**: `src` bleibt Byte für Byte unverändert (auch ihre `-wal`-Datei
    /// wird nicht angefasst), `dst` ist eine eigenständige Datei im aktuellen Schema (OUT-184, OS-Festlegung:
    /// `--load-crawl` einer Datei älterer Schemaversion). Eine Sicherungskopie `<name>.v<alt>.bak.ospider` entsteht nicht,
    /// weil das Original selbst die Sicherung ist. Ist `src` bereits aktuell, ist `dst` eine einfache Kopie.
    /// `dst` darf nicht existieren.
    pub fn migrate_copy(src: &Path, dst: &Path) -> Result<Self, StoreError> {
        if dst.exists() {
            return Err(StoreError::AlreadyExists(dst.to_path_buf()));
        }
        if !src.exists() {
            return Err(StoreError::NotFound(src.to_path_buf()));
        }
        if !check_sqlite_header(src)? {
            return Err(StoreError::NotACrawlFile {
                path: src.to_path_buf(),
            });
        }
        if let Some(parent) = dst.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|e| StoreError::io(parent, e))?;
        }
        let copy = (|| -> Result<(), StoreError> {
            let probe = Connection::open_with_flags(
                src,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )
            .map_err(|e| classify(src, e))?;
            probe.busy_timeout(std::time::Duration::from_secs(5))?;
            let app_id = schema::application_id(&probe).map_err(|e| match e {
                StoreError::Sqlite(e) => classify(src, e),
                other => other,
            })?;
            if app_id != APPLICATION_ID {
                return Err(StoreError::NotACrawlFile {
                    path: src.to_path_buf(),
                });
            }
            let found = schema::user_version(&probe)?;
            let latest = schema::latest_version(MIGRATIONS);
            if found > latest {
                return Err(StoreError::NewerVersion {
                    path: src.to_path_buf(),
                    found,
                    supported: latest,
                    generator: read_generator(&probe),
                });
            }
            // `VACUUM INTO` liest einen konsistenten Schnappschuss (auch aus dem WAL) und schreibt nur `dst`.
            let target = dst.to_string_lossy().into_owned();
            probe.execute("VACUUM INTO ?1", [target.as_str()])?;
            Ok(())
        })();
        if let Err(e) = copy {
            let _ = std::fs::remove_file(dst);
            return Err(e);
        }
        match Self::open_inner(dst, OpenMode::ReadWrite, MIGRATIONS, false) {
            Ok(f) => Ok(f),
            Err(e) => {
                for suffix in ["", "-wal", "-shm"] {
                    let mut p = dst.as_os_str().to_owned();
                    p.push(suffix);
                    let _ = std::fs::remove_file(PathBuf::from(p));
                }
                Err(e)
            }
        }
    }

    fn open_inner(
        path: &Path,
        mode: OpenMode,
        migrations: &[Migration],
        backup: bool,
    ) -> Result<Self, StoreError> {
        if !path.exists() {
            return Err(StoreError::NotFound(path.to_path_buf()));
        }
        if !check_sqlite_header(path)? {
            return Err(StoreError::NotACrawlFile {
                path: path.to_path_buf(),
            });
        }
        let latest = schema::latest_version(migrations);
        // Die Manifest-Werte (OUT-233) pflegt nur die Produktionsliste; Tests mit eigenen Listen haben kein `crawl_meta`.
        let production = std::ptr::eq(migrations, MIGRATIONS);

        // Prüfen, ohne die Datei zu verändern.
        let (found, generator) = {
            let probe = Connection::open_with_flags(
                path,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )
            .map_err(|e| classify(path, e))?;
            let app_id = schema::application_id(&probe).map_err(|e| match e {
                StoreError::Sqlite(e) => classify(path, e),
                other => other,
            })?;
            if app_id != APPLICATION_ID {
                return Err(StoreError::NotACrawlFile {
                    path: path.to_path_buf(),
                });
            }
            let version = schema::user_version(&probe)?;
            if version == 0 {
                return Err(StoreError::Corrupt {
                    path: path.to_path_buf(),
                    detail: "Schema unvollständig angelegt".to_string(),
                });
            }
            (version, read_generator(&probe))
        };
        if found > latest {
            return Err(StoreError::NewerVersion {
                path: path.to_path_buf(),
                found,
                supported: latest,
                generator,
            });
        }
        if found < latest && mode == OpenMode::ReadOnly {
            return Err(StoreError::MigrationRequired {
                path: path.to_path_buf(),
                found,
                supported: latest,
            });
        }

        // Die Sicherungskopie entsteht vor der ersten Änderung der Datei (auch vor `PRAGMA journal_mode`), damit sie
        // dem Original Byte für Byte entspricht (OUT-184). Liegen noch Frames in einer `-wal`-Datei, ist die Hauptdatei
        // allein unvollständig; dann entsteht die Kopie nach dem Zusammenführen (weiter unten).
        let mut backed_up = false;
        if found < latest && mode == OpenMode::ReadWrite && backup && !wal_has_frames(path) {
            let target = backup_path(path, found);
            std::fs::copy(path, &target).map_err(|e| StoreError::io(&target, e))?;
            backed_up = true;
        }

        let flags = match mode {
            OpenMode::ReadOnly => OpenFlags::SQLITE_OPEN_READ_ONLY,
            OpenMode::ReadWrite => OpenFlags::SQLITE_OPEN_READ_WRITE,
        };
        let mut conn = Connection::open_with_flags(path, flags).map_err(|e| classify(path, e))?;
        collation::ensure(&conn)?;
        if mode == OpenMode::ReadWrite {
            profile::apply(&conn, StorageProfile::default())?;
        } else {
            conn.busy_timeout(std::time::Duration::from_secs(5))?;
        }

        if found < latest {
            // Sicherungskopie als eigenständige Datei (WAL zusammenführen), dann migrieren.
            conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
            if backup && !backed_up {
                let target = backup_path(path, found);
                std::fs::copy(path, &target).map_err(|e| StoreError::io(&target, e))?;
            }
            schema::migrate(&mut conn, found, migrations)?;
            conn.execute(
                "INSERT INTO crawl_meta (key, value) VALUES ('schema_version', ?1) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [latest.to_string()],
            )?;
            // Manifest (OUT-233): was die alte Datei nicht kennt (`openspider_version`), bleibt unbekannt (NULL);
            // abgeleitet werden nur Werte aus der eingebetteten Konfiguration.
            if production {
                if let Some(osconfig) = crate::meta::raw_config(&conn)? {
                    crate::meta::stamp_config(&conn, &osconfig)?;
                }
            }
        }
        if mode == OpenMode::ReadWrite && production {
            crate::meta::stamp_writer(&conn)?;
        }
        Ok(CrawlFile {
            path: path.to_path_buf(),
            conn,
        })
    }

    /// Pfad der Datei.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Die SQLite-Verbindung (für `os-analyze`, `os-export`, `os-mcp`).
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Liest die eingebettete Konfiguration (CRAWL-127: die damalige Konfiguration).
    pub fn config(&self) -> Result<CrawlConfig, StoreError> {
        let text: Option<String> = self
            .conn
            .query_row("SELECT osconfig FROM crawl_config WHERE id = 1", [], |r| {
                r.get(0)
            })
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            })?;
        let text = text.ok_or(StoreError::MissingConfig)?;
        Ok(CrawlConfig::from_toml_str(&text)?)
    }

    /// Ersetzt die eingebettete Konfiguration („Konfiguration übernehmen“).
    ///
    /// Der Zeitstempel kommt von der Systemuhr; für deterministische Läufe siehe
    /// [`CrawlFile::set_config_with_clock`].
    pub fn set_config(&self, c: &CrawlConfig) -> Result<(), StoreError> {
        self.set_config_with_clock(c, &SystemClock)
    }

    /// Wie [`CrawlFile::set_config`], aber mit dem Zeitstempel aus `clock` (Regel 2.8).
    pub fn set_config_with_clock(
        &self,
        c: &CrawlConfig,
        clock: &dyn Clock,
    ) -> Result<(), StoreError> {
        let osconfig = c.to_toml_string()?;
        self.conn.execute(
            "INSERT INTO crawl_config (id, osconfig, updated_at) VALUES (1, ?1, ?2) \
             ON CONFLICT(id) DO UPDATE SET osconfig = excluded.osconfig, updated_at = excluded.updated_at",
            [&osconfig, &clock.now().to_db()],
        )?;
        crate::meta::stamp_config(&self.conn, &osconfig)?;
        Ok(())
    }
}

/// `<verzeichnis>/<name>.v<alt>.bak.ospider` (CRAWL-127).
pub fn backup_path(path: &Path, old_version: u32) -> PathBuf {
    let stem = path
        .file_stem()
        .map_or_else(|| "crawl".into(), |s| s.to_string_lossy().into_owned());
    path.with_file_name(format!("{stem}.v{old_version}.bak.ospider"))
}
