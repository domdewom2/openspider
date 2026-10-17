//! View-Sitzungen mit wahlfreiem Zugriff (K-1): Filter-Bitmaps als ableitbarer Cache (`view_cache`), Lazy-Aufbau,
//! Invalidierung, Sortierpermutationen, Positionsliste samt Umkehrabbildung, `rows`/`locate`, Speicherbudget.
//!
//! M2-F02 stellt nur die Signaturen bereit, an die `os-app` (K05) und die Benchmarks (`benches/views.rs`) anschließen;
//! die Umsetzung gehört M2-K01. Bis dahin liefern alle Funktionen `ViewError::NotImplemented`.

use std::time::Duration;

use os_model::{CrawlConfig, UrlId, Value};
use os_store::CrawlFile;

use crate::defs::SortKey;
use crate::error::ViewError;
use crate::search::FilterSource;
use crate::views::{ColumnSelection, SearchSpec};

/// Was eine Sitzung zeigt: Tab, Filter, Suche, Sortierung, Spalten und optionale Einschränkungen.
pub struct SessionSpec {
    /// Tab, z. B. `"Internal"`.
    pub tab: String,
    /// Filter, z. B. `"HTML"`.
    pub filter: String,
    /// Suche.
    pub search: Option<SearchSpec>,
    /// Sortierung (leer = Standard des Tabs).
    pub sort: Vec<SortKey>,
    /// Spalten.
    pub columns: ColumnSelection,
    /// Weitere Einschränkungen (Segment, Vergleichsklasse), ODER-frei verknüpft mit UND.
    pub restrict: Vec<std::sync::Arc<dyn FilterSource>>,
}

/// Speicherbudget einer Sitzung bzw. aller Sitzungen (K-1g).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    /// Höchstzahl Bytes für Positionslisten, Umkehrabbildungen und Permutationen (LRU-begrenzt).
    pub max_bytes: u64,
    /// Nach dieser Leerlaufzeit gibt die Sitzung ihren Speicher frei.
    pub idle_release: Duration,
}

impl Default for Budget {
    fn default() -> Self {
        Budget {
            max_bytes: 512 * 1024 * 1024,
            idle_release: Duration::from_secs(60),
        }
    }
}

/// Anfrage eines Zeilenfensters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowsReq {
    /// Version, auf die sich der Aufrufer bezieht (`change_seq`); eine andere Version wird verworfen.
    pub version: u64,
    /// Erste Zeile.
    pub offset: u64,
    /// Zeilenzahl (höchstens 500).
    pub limit: u32,
}

/// Ein Zeilenfenster.
#[derive(Debug, Clone, PartialEq)]
pub struct Rows {
    /// Spaltennamen.
    pub columns: Vec<String>,
    /// Zeilen.
    pub rows: Vec<Vec<Value>>,
    /// Zeilenschlüssel (`UrlId`) in Zeilenreihenfolge.
    pub row_keys: Vec<UrlId>,
    /// Gesamtzahl der Zeilen der Ansicht.
    pub total: u64,
    /// Version der Ansicht.
    pub version: u64,
}

/// Eine offene Ansicht mit wahlfreiem Zugriff.
pub struct ViewSession {
    spec: SessionSpec,
    budget: Budget,
}

// STUB(M2-K01)
impl ViewSession {
    /// Öffnet eine Sitzung (baut Positionsliste und Umkehrabbildung lazy beim ersten Zugriff auf).
    pub fn open(
        _file: &CrawlFile,
        _cfg: &CrawlConfig,
        spec: SessionSpec,
        budget: Budget,
    ) -> Result<ViewSession, ViewError> {
        Ok(ViewSession { spec, budget })
    }

    /// Ersetzt Filter, Suche oder Sortierung; Zählung und Positionsliste werden bei Bedarf neu aufgebaut.
    pub fn update(
        &mut self,
        _file: &CrawlFile,
        _cfg: &CrawlConfig,
        spec: SessionSpec,
    ) -> Result<(), ViewError> {
        self.spec = spec;
        Err(ViewError::NotImplemented("ViewSession::update (M2-K01)"))
    }

    /// Liefert ein Zeilenfenster.
    pub fn rows(&self, _file: &CrawlFile, _req: RowsReq) -> Result<Rows, ViewError> {
        Err(ViewError::NotImplemented("ViewSession::rows (M2-K01)"))
    }

    /// Zeilenindex einer URL in der Ansicht, falls sie darin liegt (Anker, Tail-Modus, UI-004).
    pub fn locate(&self, _file: &CrawlFile, _id: UrlId) -> Result<Option<u64>, ViewError> {
        Err(ViewError::NotImplemented("ViewSession::locate (M2-K01)"))
    }

    /// Prüft geänderte URLs (aus `queries::changed_since`) gegen die Ansicht und führt Positionsliste und Version nach (K-1c).
    pub fn apply_changes(&mut self, _file: &CrawlFile, _ids: &[UrlId]) -> Result<(), ViewError> {
        Err(ViewError::NotImplemented(
            "ViewSession::apply_changes (M2-K01)",
        ))
    }

    /// Das Speicherbudget dieser Sitzung.
    pub fn budget(&self) -> Budget {
        self.budget
    }

    /// Die aktuelle Beschreibung.
    pub fn spec(&self) -> &SessionSpec {
        &self.spec
    }

    /// Gibt die Sitzung und ihren Speicher frei.
    pub fn close(self) {}
}
