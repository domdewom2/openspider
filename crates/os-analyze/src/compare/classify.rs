//! Klassifikation je Filter: Added / New / Removed / Missing (OUT-142, CRAWL-032).
//!
//! Für einen Filter F seien `C` die Current-URLs in F und `P` die Previous-URLs in F (jeweils aus dem Plan des Filters
//! mit der Konfiguration des eigenen Crawls, also dieselbe Menge wie beim Öffnen des Crawls allein). Mit den Paaren
//! aus [`Pairing`](super::Pairing):
//!
//! * **Added** = gepaarte `u ∈ C`, deren Partner nicht in `P` liegt (Current-Kennungen),
//! * **New** = `u ∈ C` ohne Partner (Current-Kennungen),
//! * **Removed** = gepaarte `p ∈ P`, deren Partner nicht in `C` liegt (Previous-Kennungen; die Current-Partner liefert
//!   [`Classes::removed_current`]),
//! * **Missing** = `p ∈ P` ohne Partner (Previous-Kennungen).
//!
//! Weil jede URL höchstens ein Paar bildet, gilt die Invariante `|C| − |P| = Added + New − Removed − Missing` für jeden
//! Filter ohne Sonderfall.

use os_model::{CrawlConfig, FilterState, Value};
use os_store::CrawlFile;
use roaring::RoaringBitmap;

use super::pairing::{url_index, Pairing};
use super::CompareError;
use crate::defs::TabDef;
use crate::views::{self, ViewQuery, URLS_FROM};

/// Unterfilter des Vergleichs (OUT-142).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CompareClass {
    /// In beiden Crawls, neu im Filter.
    Added,
    /// Nur im Current-Crawl, dort im Filter.
    New,
    /// In beiden Crawls, nicht mehr im Filter.
    Removed,
    /// Nur im Previous-Crawl, dort im Filter.
    Missing,
}

impl CompareClass {
    /// Alle vier Unterfilter in Anzeigereihenfolge.
    pub const ALL: [CompareClass; 4] = [
        CompareClass::Added,
        CompareClass::New,
        CompareClass::Removed,
        CompareClass::Missing,
    ];

    /// Kanonischer Name (`Added`, `New`, `Removed`, `Missing`).
    pub fn as_str(self) -> &'static str {
        match self {
            CompareClass::Added => "Added",
            CompareClass::New => "New",
            CompareClass::Removed => "Removed",
            CompareClass::Missing => "Missing",
        }
    }

    /// Name → Unterfilter (ohne Beachtung der Groß-/Kleinschreibung).
    pub fn parse(name: &str) -> Option<CompareClass> {
        CompareClass::ALL
            .into_iter()
            .find(|c| c.as_str().eq_ignore_ascii_case(name.trim()))
    }

    /// Crawl, aus dem die Zeilen dieses Unterfilters stammen: Removed zeigt die Current-Zeile des Partners, Missing
    /// die Previous-Zeile.
    pub fn side(self) -> Side {
        match self {
            CompareClass::Missing => Side::Previous,
            _ => Side::Current,
        }
    }
}

/// Einer der beiden Crawls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    /// Der aktuelle Crawl.
    Current,
    /// Der vorherige Crawl.
    Previous,
}

impl Side {
    /// Kanonischer Name (`Current`, `Previous`).
    pub fn as_str(self) -> &'static str {
        match self {
            Side::Current => "Current",
            Side::Previous => "Previous",
        }
    }
}

/// Die vier Mengen eines Filters.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Classes {
    /// Current-Kennungen.
    pub added: RoaringBitmap,
    /// Current-Kennungen.
    pub new: RoaringBitmap,
    /// Previous-Kennungen.
    pub removed: RoaringBitmap,
    /// Previous-Kennungen.
    pub missing: RoaringBitmap,
}

impl Classes {
    /// Klassifiziert `current` (Current-Kennungen in F) gegen `previous` (Previous-Kennungen in F).
    pub fn classify(pairing: &Pairing, current: &RoaringBitmap, previous: &RoaringBitmap) -> Classes {
        let previous_in_current = pairing.to_current(previous);
        let current_in_previous = {
            let mut out = RoaringBitmap::new();
            for id in current & &pairing.paired_current {
                if let Some(p) = pairing.previous_of(id) {
                    out.insert(p);
                }
            }
            out
        };
        Classes {
            added: &(current & &pairing.paired_current) - &previous_in_current,
            new: current - &pairing.paired_current,
            removed: &(previous & &pairing.paired_previous) - &current_in_previous,
            missing: previous - &pairing.paired_previous,
        }
    }

    /// Die Menge eines Unterfilters in den Kennungen des Crawls, aus dem seine Zeilen stammen
    /// ([`CompareClass::side`]).
    pub fn ids(&self, class: CompareClass, pairing: &Pairing) -> RoaringBitmap {
        match class {
            CompareClass::Added => self.added.clone(),
            CompareClass::New => self.new.clone(),
            CompareClass::Removed => self.removed_current(pairing),
            CompareClass::Missing => self.missing.clone(),
        }
    }

    /// Removed als Current-Kennungen der Partner.
    pub fn removed_current(&self, pairing: &Pairing) -> RoaringBitmap {
        pairing.to_current(&self.removed)
    }

    /// Anzahl eines Unterfilters.
    pub fn count(&self, class: CompareClass) -> u64 {
        match class {
            CompareClass::Added => self.added.len(),
            CompareClass::New => self.new.len(),
            CompareClass::Removed => self.removed.len(),
            CompareClass::Missing => self.missing.len(),
        }
    }
}

/// Vergleichszeile eines Filters (Overview-Spalten `Current`, `Previous`, `Change`, `Added`, `New`, `Removed`,
/// `Missing`).
#[derive(Debug, Clone, PartialEq)]
pub struct FilterComparison {
    /// Tab.
    pub tab: String,
    /// Kennung des Filters (`FilterDef.id`, Platzhalter unaufgelöst).
    pub filter: String,
    /// Anzeigename mit den Schwellen des Current-Crawls.
    pub label: String,
    /// Zustand im Current-Crawl.
    pub current_state: FilterState,
    /// Zustand im Previous-Crawl; `None`, wenn der Filter dort nicht existiert (z. B. eigener Custom-Filter).
    pub previous_state: Option<FilterState>,
    /// Treffer im Current-Crawl (nur bei `Ready`).
    pub current: Option<u64>,
    /// Treffer im Previous-Crawl (nur bei `Ready`).
    pub previous: Option<u64>,
    /// Unterfilter; nur, wenn beide Seiten Zahlen haben.
    pub classes: Option<[u64; 4]>,
}

impl FilterComparison {
    /// `Current − Previous`, wenn beide Seiten Zahlen haben.
    pub fn change(&self) -> Option<i64> {
        Some(self.current? as i64 - self.previous? as i64)
    }

    /// `Change` in Prozent von `Previous` (zwei Nachkommastellen); leer bei `Previous` = 0.
    pub fn change_pct(&self) -> Option<f64> {
        let previous = self.previous?;
        let change = self.change()?;
        (previous > 0).then(|| ((change as f64 * 100.0 / previous as f64) * 100.0).round() / 100.0)
    }

    /// Anzahl eines Unterfilters.
    pub fn class(&self, class: CompareClass) -> Option<u64> {
        let i = CompareClass::ALL.iter().position(|c| *c == class)?;
        self.classes.map(|c| c[i])
    }
}

/// Bitmap eines Filters in einer Crawl-Datei: Zustand und – nur bei `Ready` – die Treffer.
#[derive(Debug, Clone, PartialEq)]
pub struct FilterSet {
    /// Zustand des Filters.
    pub state: FilterState,
    /// Treffer (`None`, solange der Zustand keine Zahl erlaubt).
    pub ids: Option<RoaringBitmap>,
}

fn bind(stmt: &mut rusqlite::Statement<'_>, params: &[(String, Value)]) -> rusqlite::Result<()> {
    for (name, value) in params {
        if let Some(idx) = stmt.parameter_index(name)? {
            match value {
                Value::Null => stmt.raw_bind_parameter(idx, rusqlite::types::Null)?,
                Value::Bool(b) => stmt.raw_bind_parameter(idx, i64::from(*b))?,
                Value::Int(i) => stmt.raw_bind_parameter(idx, *i)?,
                Value::Float(f) => stmt.raw_bind_parameter(idx, *f)?,
                Value::Text(s) => stmt.raw_bind_parameter(idx, s.as_str())?,
            }
        }
    }
    Ok(())
}

/// Berechnet die Treffer eines Filters (gleicher Plan wie Tabelle, Zähler und View-Sitzung).
pub fn filter_set(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
    tab: &str,
    filter: &str,
) -> Result<FilterSet, CompareError> {
    let state = views::state_of_in(tabs, file, cfg, tab, filter)?;
    if !state.yields_rows() {
        return Ok(FilterSet { state, ids: None });
    }
    let mut plan = views::plan_in(tabs, &ViewQuery::new(tab, filter), cfg)?;
    crate::tabs::content::bind_analysis_params(&mut plan.params, file, cfg)?;
    let sql = format!(
        "SELECT u.id FROM {URLS_FROM} WHERE ({}) AND ({})",
        plan.population_sql, plan.filter_sql
    );
    let mut stmt = file.conn().prepare(&sql)?;
    bind(&mut stmt, &plan.params)?;
    let mut ids = RoaringBitmap::new();
    let mut rows = stmt.raw_query();
    while let Some(row) = rows.next()? {
        ids.insert(url_index(row.get(0)?)?);
    }
    Ok(FilterSet {
        state,
        ids: Some(ids),
    })
}
