//! Lastszenarien (PERF-001 bis PERF-008) und ihre synthetischen Sites.

use clap::ValueEnum;
use os_testsite::SyntheticConfig;
use serde::Serialize;

/// Größe eines Laufs: `small` läuft in jeder CI, `large` vor Meilenstein-Abnahmen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Size {
    /// 10.000 URLs.
    Small,
    /// 1.000.000 URLs (bzw. szenariospezifisch).
    Large,
}

impl Size {
    /// Name wie auf der Kommandozeile.
    pub fn as_str(self) -> &'static str {
        match self {
            Size::Small => "small",
            Size::Large => "large",
        }
    }
}

/// Beschreibung eines Szenarios.
#[derive(Debug, Clone, Copy)]
pub struct ScenarioDef {
    /// Name auf der Kommandozeile.
    pub name: &'static str,
    /// Spec-ID in `docs/spec/07-performance.md`.
    pub perf_id: &'static str,
    /// Kurzbeschreibung.
    pub description: &'static str,
    /// Seitenanzahl der synthetischen Site (small, large).
    pub pages: (u64, u64),
    /// Anpassung der Standardkonfiguration (Anteile, Verzögerungen).
    pub tune: fn(&mut SyntheticConfig),
}

fn plain(_: &mut SyntheticConfig) {}

fn with_errors(c: &mut SyntheticConfig) {
    c.status_301_share = 0.02;
    c.status_404_share = 0.01;
    c.status_5xx_share = 0.005;
}

fn with_slow_pages(c: &mut SyntheticConfig) {
    c.delay_ms = 250;
    c.slow_share = 0.01;
}

fn with_duplicates(c: &mut SyntheticConfig) {
    c.duplicate_title_share = 0.2;
    c.noindex_share = 0.05;
    c.canonical_other_share = 0.05;
}

/// Alle Szenarien. PERF-004 (UI) wird im Frontend-Test geprüft und hat hier keinen Eintrag.
pub const SCENARIOS: &[ScenarioDef] = &[
    ScenarioDef {
        name: "perf-001",
        perf_id: "PERF-001",
        description: "Speicherobergrenze im Textmodus (30 Links je Seite)",
        pages: (10_000, 1_000_000),
        tune: plain,
    },
    ScenarioDef {
        name: "perf-002",
        perf_id: "PERF-002",
        description: "Durchsatz (URLs/s), Stabilität über den gesamten Crawl",
        pages: (10_000, 1_000_000),
        tune: plain,
    },
    ScenarioDef {
        name: "perf-003",
        perf_id: "PERF-003",
        description: "Gegendruck bei künstlich verlangsamtem Store",
        pages: (10_000, 1_000_000),
        tune: with_errors,
    },
    ScenarioDef {
        name: "perf-005",
        perf_id: "PERF-005",
        description: "Absturzsicherheit und Fortsetzen",
        pages: (10_000, 100_000),
        tune: with_errors,
    },
    ScenarioDef {
        name: "perf-006",
        perf_id: "PERF-006",
        description: "JS-Rendering unter Kontrolle (langsame Seiten)",
        pages: (1_000, 10_000),
        tune: with_slow_pages,
    },
    ScenarioDef {
        name: "perf-007",
        perf_id: "PERF-007",
        description: "Nachlauf-Analysen (Duplikate, Link Score)",
        pages: (10_000, 1_000_000),
        tune: with_duplicates,
    },
    ScenarioDef {
        name: "perf-008",
        perf_id: "PERF-008",
        description: "Große Exporte",
        pages: (10_000, 1_000_000),
        tune: with_errors,
    },
];

/// Sucht ein Szenario anhand des Namens (`perf-001`) oder der Spec-ID (`PERF-001`).
pub fn find(name: &str) -> Option<&'static ScenarioDef> {
    SCENARIOS
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(name) || s.perf_id.eq_ignore_ascii_case(name))
}

impl ScenarioDef {
    /// Konfiguration der synthetischen Site für eine Größe.
    pub fn config(&self, size: Size) -> SyntheticConfig {
        let mut cfg = SyntheticConfig {
            pages: match size {
                Size::Small => self.pages.0,
                Size::Large => self.pages.1,
            },
            ..Default::default()
        };
        (self.tune)(&mut cfg);
        cfg
    }
}
