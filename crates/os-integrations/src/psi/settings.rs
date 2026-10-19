//! Quelle, Gerätetyp, Metrikgruppen und Grenzen der PSI-Anbindung (INTEG-130 … INTEG-133, INTEG-142).

use std::collections::BTreeSet;

use os_model::config::{ApiLimitsConfig, PsiConfig};
use os_model::PsiDevice;

use super::columns::{Group, COLUMNS, FILTERS};

/// Anfragen je Fenster der Remote-API (INTEG-142).
pub const REQUESTS_PER_WINDOW: u32 = 60;
/// Fensterlänge in Sekunden (INTEG-142).
pub const WINDOW_S: u32 = 100;
/// Gleichzeitige Remote-Anfragen (INTEG-142).
pub const CONCURRENCY: u32 = 5;
/// Tageskontingent je Schlüssel (INTEG-142, Tageswechsel America/Los_Angeles).
pub const DAILY_QUOTA: u64 = 25_000;
/// Pause nach gehäuften 500ern (INTEG-143).
pub const BURST_PAUSE_S: u64 = 600;
/// Mehr als so viele aufeinanderfolgende 500er lösen die Pause aus (INTEG-143).
pub const BURST_LIMIT: u32 = 10;

/// Quelle der Lighthouse-Daten (INTEG-130). Standard: Remote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Source {
    /// PSI-API mit Schlüssel; liefert zusätzlich CrUX.
    #[default]
    Remote,
    /// Lighthouse auf dem eigenen Rechner (INTEG-132).
    Local,
}

impl Source {
    /// Quelle laut Konfiguration (`api.psi.local`).
    pub fn of(cfg: &PsiConfig) -> Source {
        if cfg.local {
            Source::Local
        } else {
            Source::Remote
        }
    }
}

/// Wert des Request-Parameters `strategy` (INTEG-131).
pub fn strategy(device: PsiDevice) -> &'static str {
    match device {
        PsiDevice::Desktop => "desktop",
        _ => "mobile",
    }
}

/// Auswahl der Metrikgruppen bzw. Einzelmetriken (INTEG-133).
///
/// `api.psi.metric_groups` enthält Gruppenkennungen (`overview`, `crux`, `lighthouse`, `insights`, `diagnostics`,
/// `mobile`) und/oder einzelne Spaltennamen. **OS-Festlegung:** eine leere Liste wählt alle Gruppen (Standard). Bei
/// Quelle Lokal ist CrUX nie gewählt (INTEG-130).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    groups: BTreeSet<Group>,
    columns: BTreeSet<String>,
}

impl Selection {
    /// Auswahl aus der Konfiguration.
    pub fn of(cfg: &PsiConfig) -> Selection {
        let mut groups = BTreeSet::new();
        let mut columns = BTreeSet::new();
        for entry in &cfg.metric_groups {
            match Group::parse(entry) {
                Some(g) => {
                    groups.insert(g);
                }
                None => {
                    columns.insert(entry.trim().to_string());
                }
            }
        }
        if cfg.metric_groups.is_empty() {
            groups.extend(Group::ALL);
        }
        if Source::of(cfg) == Source::Local {
            groups.remove(&Group::Crux);
            columns.retain(|c| super::columns::group_of(c) != Some(Group::Crux));
        }
        Selection { groups, columns }
    }

    /// Ist die ganze Gruppe gewählt?
    pub fn group(&self, group: Group) -> bool {
        self.groups.contains(&group)
    }

    /// Erscheint die Spalte? `PSI Status`/`PSI Error` immer.
    pub fn column(&self, column: &str) -> bool {
        match super::columns::group_of(column) {
            Some(g) => self.group(g) || self.columns.contains(column),
            None => true,
        }
    }

    /// Sichtbare Spalten in Tab-Reihenfolge (ohne Status/Fehler).
    pub fn columns(&self) -> Vec<&'static str> {
        COLUMNS
            .iter()
            .filter(|(_, name, _)| self.column(name))
            .map(|(_, name, _)| *name)
            .collect()
    }

    /// Erscheint der Filter? Ein Filter gehört zu seiner Gruppe (INTEG-133).
    pub fn filter(&self, filter: &str) -> bool {
        FILTERS
            .iter()
            .find(|(f, _)| *f == filter)
            .is_none_or(|(_, g)| self.group(*g))
    }
}

/// Gruppen des Metrik-Dialogs mit „wählbar“ (CrUX ist bei Lokal deaktiviert, INTEG-130).
pub fn dialog_groups(source: Source) -> Vec<(Group, bool)> {
    Group::ALL
        .into_iter()
        .map(|g| (g, !(source == Source::Local && g == Group::Crux)))
        .collect()
}

/// Wirksame Grenzen der Remote-API: ohne eigene Rate gelten 60 Anfragen je 100 s (INTEG-142).
pub fn effective_limits(cfg: &PsiConfig) -> ApiLimitsConfig {
    let mut limits = cfg.limits.clone();
    if limits.requests_per_window.is_none() {
        limits.requests_per_window = Some(REQUESTS_PER_WINDOW);
        limits.window_s = WINDOW_S;
    }
    limits
}

/// Tageskontingent der Remote-Quelle (eigenes Limit aus `limits.quota_limit`, sonst 25.000).
pub fn daily_quota(cfg: &PsiConfig) -> u64 {
    cfg.limits.quota_limit.unwrap_or(DAILY_QUOTA)
}
