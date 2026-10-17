//! Öffentliche Leseschnittstelle der Crawl-Datei (OUT-233 Manifest, OUT-234 `q_`-Sichten; M2-F02, Etappe E0).
//!
//! Fremde Leser (Skripte, BI-Werkzeuge, die Cloud-Variante) greifen nur über die nur-lesenden Sichten `q_manifest`,
//! `q_config`, `q_urls`, `q_links`, `q_sitemap_urls` und `q_columns` auf eine `.ospider`-Datei zu, nie auf interne
//! Tabellen. Die Sichten liegen **in der Datei**: Sie sind Teil der Migration `0003_m2.sql` (Entscheidung und Gründe im
//! Kopf dieser Datei), brauchen weder OpenSpider-Code noch eigene Kollationen oder Funktionen und laufen mit
//! Standard-SQLite (`file:<pfad>?mode=ro`, bei schreibgeschütztem Verzeichnis `immutable=1`).
//!
//! **Warum in der Migration und nicht in einer Installationsroutine beim Öffnen?** Eine neu angelegte und eine aus
//! Schema v1/v2 migrierte Datei erhalten so byte-gleiche Sichtdefinitionen (Golden `tests/golden/out-m2-read-api/`); ein
//! Öffnen verändert die Datei nicht (kein DDL, keine Sperre gegen einen laufenden Crawl); die Definition ist mit der
//! Schemaversion eingefroren, spätere Ergänzungen der Version 1 (neue Spalten am Ende, neue Sichten) kommen als neue
//! Migration über ein N-Paket. Was SQLite nicht berechnen kann (SHA-256 der Konfiguration, Programmversionen), pflegt
//! [`crate::meta`] als Schlüssel in `crawl_meta`.
//!
//! Dieses Modul liefert die Lesehilfen für OpenSpider-eigenen Code (Tests, CLI, MCP): Manifest lesen, Version prüfen,
//! Selbstbeschreibung prüfen.

use std::collections::BTreeSet;

use rusqlite::Connection;

use crate::error::StoreError;
use crate::open::CrawlFile;
pub use crate::schema::READ_API_VERSION;

/// Wert von `q_manifest.format`.
pub const FORMAT: &str = "ospider";

/// Version des Manifests (`q_manifest.manifest_version`).
pub const MANIFEST_VERSION: u32 = 1;

/// Die Sichten der Version 1 der Leseschnittstelle.
pub const VIEWS: &[&str] = &[
    "q_manifest",
    "q_config",
    "q_urls",
    "q_links",
    "q_sitemap_urls",
    "q_columns",
];

/// Schlüssel des Manifests, Version 1 (OUT-233); jeder ist in `q_manifest` immer vorhanden.
pub const MANIFEST_KEYS: &[&str] = &[
    "format",
    "manifest_version",
    "read_api_version",
    "schema_version",
    "openspider_version",
    "last_writer_version",
    "writer_kind",
    "crawl_id",
    "name",
    "project",
    "mode",
    "start_url",
    "list_source",
    "started_at",
    "ended_at",
    "state",
    "end_reason",
    "completeness",
    "user_agent",
    "config_sha256",
    "urls_total",
    "urls_crawled",
    "analysis_state",
];

/// Eine Zeile der Selbstbeschreibung `q_columns`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnDoc {
    /// Name der Sicht.
    pub view: String,
    /// Name der Spalte.
    pub column: String,
    /// Typ: `INTEGER`, `REAL` oder `TEXT`.
    pub sql_type: String,
    /// Bedeutung in einem Satz.
    pub description: String,
}

/// Liest `q_manifest` (Schlüssel aufsteigend). Ein unbekannter Wert ist `None`.
pub fn manifest(conn: &Connection) -> Result<Vec<(String, Option<String>)>, StoreError> {
    let mut stmt = conn.prepare("SELECT key, value FROM q_manifest ORDER BY key")?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Prüft `format` und `read_api_version` wie ein Leser es tun soll: unbekanntes Format oder unbekannte Hauptversion
/// ergibt eine klare Meldung.
pub fn check(conn: &Connection) -> Result<(), StoreError> {
    let get = |key: &str| -> Result<Option<String>, StoreError> {
        Ok(conn.query_row(
            "SELECT value FROM q_manifest WHERE key = ?1",
            [key],
            |r| r.get::<_, Option<String>>(0),
        )?)
    };
    match get("format")? {
        Some(f) if f == FORMAT => {}
        other => {
            return Err(StoreError::InvalidInput(format!(
                "keine OpenSpider-Crawl-Datei (format = {})",
                other.as_deref().unwrap_or("NULL")
            )))
        }
    }
    let version = get("read_api_version")?;
    match version.as_deref().and_then(|v| v.parse::<u32>().ok()) {
        Some(v) if v == READ_API_VERSION => Ok(()),
        _ => Err(StoreError::InvalidInput(format!(
            "unbekannte Version der Leseschnittstelle: {}",
            version.as_deref().unwrap_or("NULL")
        ))),
    }
}

/// Alle Zeilen von `q_columns` (nach Sicht und Spalte in der Reihenfolge der Sichten).
pub fn columns(conn: &Connection) -> Result<Vec<ColumnDoc>, StoreError> {
    let mut stmt = conn.prepare("SELECT \"view\", \"column\", type, description FROM q_columns")?;
    let rows = stmt
        .query_map([], |r| {
            Ok(ColumnDoc {
                view: r.get(0)?,
                column: r.get(1)?,
                sql_type: r.get(2)?,
                description: r.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Spalten der Sichten ohne passende Zeile in `q_columns` (oder mit leerer Beschreibung bzw. unbekanntem Typ) sowie
/// Beschreibungen ohne Spalte: `Vec<(Sicht, Spalte, Befund)>`. Leer = die Selbstbeschreibung ist lückenlos.
pub fn describe_gaps(conn: &Connection) -> Result<Vec<(String, String, String)>, StoreError> {
    let docs = columns(conn)?;
    let mut problems = Vec::new();
    let mut real: BTreeSet<(String, String)> = BTreeSet::new();
    for view in VIEWS {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({view})"))?;
        let cols = stmt
            .query_map([], |r| r.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?;
        if cols.is_empty() {
            problems.push(((*view).to_string(), String::new(), "Sicht fehlt".to_string()));
        }
        for c in cols {
            real.insert(((*view).to_string(), c));
        }
    }
    for (view, column) in &real {
        match docs.iter().find(|d| &d.view == view && &d.column == column) {
            None => problems.push((view.clone(), column.clone(), "nicht beschrieben".into())),
            Some(d) => {
                if d.description.trim().is_empty() {
                    problems.push((view.clone(), column.clone(), "Beschreibung leer".into()));
                }
                if !matches!(d.sql_type.as_str(), "INTEGER" | "REAL" | "TEXT") {
                    problems.push((view.clone(), column.clone(), "Typ unbekannt".into()));
                }
            }
        }
    }
    for d in &docs {
        if !real.contains(&(d.view.clone(), d.column.clone())) {
            problems.push((d.view.clone(), d.column.clone(), "Spalte existiert nicht".into()));
        }
    }
    Ok(problems)
}

impl CrawlFile {
    /// Das Manifest der Datei (`q_manifest`), Schlüssel aufsteigend.
    pub fn manifest(&self) -> Result<Vec<(String, Option<String>)>, StoreError> {
        manifest(self.conn())
    }

    /// Prüft Format und Version der Leseschnittstelle (siehe [`check`]).
    pub fn check_read_api(&self) -> Result<(), StoreError> {
        check(self.conn())
    }
}
