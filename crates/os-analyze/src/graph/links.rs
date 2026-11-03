//! Links eines Knotens einblenden (UI-097): tatsächliche Hyperlinks statt der Entdeckungskanten.
//!
//! Zählweise wie die Spalten `Unique Inlinks`/`Unique Outlinks` (`os_store::counters`): interne Quellseite,
//! Linktyp `Hyperlink`, crawlbar; je Paar aus Quelle und Ziel genau eine Kante (Selbstlinks zählen mit). Die Optionen
//! schränken die gezählten Instanzen ein: ohne nofollow nur Links mit `follow = 1`, ohne Navigation/Footer nur Links
//! außerhalb der Linkpositionen `Navigation` und `Footer`. Ein Paar zählt, sobald eine passende Instanz existiert.
//!
//! Die Links werden beim Einblenden gelesen (mit Index `links_target` bzw. `links_source` je Knoten eine Suche);
//! Knoten außerhalb des Schnappschusses fallen weg, Kanten zu nicht dargestellten Knoten zählen nur in `total`.
use std::collections::{BTreeSet, HashSet};

use os_store::{CrawlFile, StoreError};

use super::{invalid, Snapshot};

/// Richtung der eingeblendeten Links (Kontextmenü UI-097).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkDirection {
    /// „Inlinks zeigen“: Links auf den Knoten.
    Inlinks,
    /// „Inlinks auf Kinder zeigen“: Links auf die direkten Kinder des Knotens.
    ChildInlinks,
    /// „Outlinks zeigen“: Links vom Knoten.
    Outlinks,
    /// „Outlinks von Kindern zeigen“: Links von den direkten Kindern des Knotens.
    ChildOutlinks,
}

impl LinkDirection {
    /// Kennung im IPC-Vertrag (`VizGraphArgs.linksMode`).
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "inlinks" => Self::Inlinks,
            "childInlinks" => Self::ChildInlinks,
            "outlinks" => Self::Outlinks,
            "childOutlinks" => Self::ChildOutlinks,
            _ => return None,
        })
    }

    pub fn inbound(self) -> bool {
        matches!(self, Self::Inlinks | Self::ChildInlinks)
    }

    fn children(self) -> bool {
        matches!(self, Self::ChildInlinks | Self::ChildOutlinks)
    }
}

/// Optionen der Einblendung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkOverlayOptions {
    pub node: i64,
    pub direction: LinkDirection,
    /// Verzeichnisbaum statt Entdeckungsbaum (für „Kinder“).
    pub directory: bool,
    /// Links mit nofollow einbeziehen (Standard an, wie die Spalte Unique Inlinks).
    pub include_nofollow: bool,
    /// Links in Navigation und Footer (Linkposition) ausschließen (Standard aus).
    pub exclude_nav_footer: bool,
}

/// Ergebnis: Kanten (Quelle, Ziel) zwischen dargestellten Knoten und die Gesamtzahl gemäß Optionen.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LinkOverlay {
    pub edges: Vec<(i64, i64)>,
    /// Eindeutige Paare gemäß Optionen (= Unique Inlinks bzw. Unique Outlinks bei „zeigen“ ohne Kinder).
    pub total: u64,
}

impl Snapshot {
    /// Links eines Knotens bzw. seiner direkten Kinder; `shown` = aktuell dargestellte Knoten.
    pub fn link_overlay(
        &self,
        file: &CrawlFile,
        options: &LinkOverlayOptions,
        shown: &HashSet<i64>,
    ) -> Result<LinkOverlay, StoreError> {
        if !self.contains_node(options.node, options.directory) {
            return Err(invalid("unknown link overlay node"));
        }
        let focus: Vec<i64> = if options.direction.children() {
            self.children_of(options.node, options.directory)
        } else {
            vec![options.node]
        };
        // Pfadknoten des Verzeichnisbaums haben keine eigene URL und damit keine Links.
        let focus: Vec<i64> = focus
            .into_iter()
            .filter(|id| self.address_of(*id, options.directory).is_some())
            .collect();
        let mut filter = String::new();
        if !options.include_nofollow {
            filter.push_str(" AND l.follow = 1");
        }
        if options.exclude_nav_footer {
            filter.push_str(" AND COALESCE(l.link_position, '') NOT IN ('Navigation', 'Footer')");
        }
        let sql = if options.direction.inbound() {
            format!(
                "SELECT DISTINCT l.source_id FROM links l JOIN urls s ON s.id = l.source_id \
                 WHERE l.target_id = ?1 AND l.link_type = 'Hyperlink' AND l.crawlable = 1 AND s.is_internal = 1{filter} \
                 ORDER BY l.source_id"
            )
        } else {
            format!(
                "SELECT DISTINCT l.target_id FROM links l JOIN urls s ON s.id = l.source_id JOIN urls t ON t.id = l.target_id \
                 WHERE l.source_id = ?1 AND l.link_type = 'Hyperlink' AND l.crawlable = 1 AND s.is_internal = 1 \
                 AND t.is_internal = 1{filter} ORDER BY l.target_id"
            )
        };
        let mut stmt = file.conn().prepare(&sql)?;
        let mut pairs = BTreeSet::new();
        for id in &focus {
            let rows = stmt.query_map([id], |r| r.get::<_, i64>(0))?;
            for other in rows {
                let other = other?;
                pairs.insert(if options.direction.inbound() {
                    (other, *id)
                } else {
                    (*id, other)
                });
            }
        }
        let total = pairs.len() as u64;
        let edges = pairs
            .into_iter()
            .filter(|(s, t)| shown.contains(s) && shown.contains(t))
            .collect();
        Ok(LinkOverlay { edges, total })
    }
}
