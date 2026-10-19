//! Inlinks/Outlinks-Vergleich im unteren Fenster (OUT-149, führend; CRAWL-144 verweist hierher).
//!
//! **Link-Identität (Entscheidung des Architekten):** das Tripel (Quelle, Ziel, `Type`), Quelle und Ziel jeweils nach
//! URL-Mapping (Previous-URLs werden mit denselben Regeln abgebildet wie bei der Paarbildung). Der Ankertext gehört
//! nicht zur Identität. Je Identität gibt es eine Zeile je Crawl; ihre Spalten stammen von der ersten Link-Instanz in
//! Dokumentreihenfolge. Hinzugefügt = `L_C \ L_P`, Entfernt = `L_P \ L_C`; für `L_C ∩ L_P` wird die Menge der
//! Ankertexte aller Instanzen verglichen (Whitespace normalisiert, case-sensitiv): verschieden → „Ankertext geändert“,
//! sonst „Unverändert“. Position, `Rel` und Pfad gehören nicht zur Identität (OS-Festlegung).
//!
//! Spalten: Standard-Linkformat (OUT-031, [`LINK_COLUMNS`]) plus `Link Change`, `Crawl`, `Previous Anchor`
//! ([`CHANGE_COLUMNS`], OS-Festlegung der Spaltennamen).

use std::collections::{BTreeSet, HashMap};

use os_model::Value;
use os_store::CrawlFile;

use super::classify::Side;
use super::mapping::UrlMapping;
use super::CompareError;

/// Standard-Linkformat (OUT-031) in Spaltenreihenfolge; identisch mit `os_export::links_export::LINK_COLUMNS`.
pub const LINK_COLUMNS: [&str; 15] = [
    "Type",
    "Source",
    "Destination",
    "Size (Bytes)",
    "Alt Text",
    "Anchor",
    "Status Code",
    "Status",
    "Follow",
    "Target",
    "Rel",
    "Path Type",
    "Link Path",
    "Link Position",
    "Link Origin",
];

/// Zusatzspalten im Vergleichsmodus (OUT-149, OUT-151).
pub const CHANGE_COLUMNS: [&str; 3] = ["Link Change", "Crawl", "Previous Anchor"];

const ANCHOR: usize = 5;

/// Richtung der Detailansicht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkDirection {
    /// Eingehende Links der URL.
    Inlinks,
    /// Ausgehende Links der URL.
    Outlinks,
}

/// Änderungsart eines Links.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LinkChange {
    /// Nur im Current-Crawl.
    Added,
    /// Nur im Previous-Crawl.
    Removed,
    /// In beiden, Ankertexte verschieden.
    AnchorChanged,
    /// In beiden, Ankertexte gleich.
    Unchanged,
}

impl LinkChange {
    /// Kanonischer Wert für Export und CLI.
    pub fn as_str(self) -> &'static str {
        match self {
            LinkChange::Added => "Added",
            LinkChange::Removed => "Removed",
            LinkChange::AnchorChanged => "Anchor Changed",
            LinkChange::Unchanged => "Unchanged",
        }
    }
}

/// Auswahl „Änderung“ (kanonische Werte `All`, `Added`, `Removed`, `Anchor Changed`, `Unchanged`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeFilter {
    /// Alle Links der gewählten Quelle (Standard).
    All,
    /// Nur diese Änderungsart.
    Only(LinkChange),
}

impl ChangeFilter {
    /// Kanonischer Wert → Auswahl (ohne Beachtung der Groß-/Kleinschreibung).
    pub fn parse(name: &str) -> Option<ChangeFilter> {
        let name = name.trim();
        if name.eq_ignore_ascii_case("All") {
            return Some(ChangeFilter::All);
        }
        [
            LinkChange::Added,
            LinkChange::Removed,
            LinkChange::AnchorChanged,
            LinkChange::Unchanged,
        ]
        .into_iter()
        .find(|c| c.as_str().eq_ignore_ascii_case(name))
        .map(ChangeFilter::Only)
    }
}

/// Auswahl „Quelle“: `Auto` folgt der Auswahl Current/Previous des oberen Fensters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkSource {
    /// Folgt dem oberen Fenster (Standard).
    Auto,
    /// Immer der Current-Crawl.
    Current,
    /// Immer der Previous-Crawl.
    Previous,
}

impl LinkSource {
    /// Wirksamer Crawl bei der Auswahl `upper` des oberen Fensters.
    pub fn resolve(self, upper: Side) -> Side {
        match self {
            LinkSource::Auto => upper,
            LinkSource::Current => Side::Current,
            LinkSource::Previous => Side::Previous,
        }
    }
}

/// Eine Zeile der Vergleichsansicht.
#[derive(Debug, Clone, PartialEq)]
pub struct LinkRow {
    /// Werte des Standard-Linkformats ([`LINK_COLUMNS`]).
    pub values: Vec<Value>,
    /// Änderungsart; `None`, wenn die URL nur in einem Crawl existiert (Änderungsfilter ausgegraut).
    pub change: Option<LinkChange>,
    /// Crawl, aus dem die Zeile stammt (`Removed` stets `Previous`).
    pub crawl: Side,
    /// Ankertext des Previous-Crawls, genau bei `Anchor Changed` gefüllt.
    pub previous_anchor: Option<String>,
}

impl LinkRow {
    /// Werte samt Zusatzspalten ([`LINK_COLUMNS`] + [`CHANGE_COLUMNS`]).
    pub fn export_values(&self) -> Vec<Value> {
        let mut v = self.values.clone();
        v.push(
            self.change
                .map_or(Value::Null, |c| Value::Text(c.as_str().into())),
        );
        v.push(Value::Text(self.crawl.as_str().into()));
        v.push(
            self.previous_anchor
                .clone()
                .map_or(Value::Null, Value::Text),
        );
        v
    }
}

/// In welchen Crawls die markierte URL existiert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    /// In beiden (nach URL-Mapping).
    Both,
    /// Nur im Current-Crawl.
    CurrentOnly,
    /// Nur im Previous-Crawl.
    PreviousOnly,
}

/// Ergebnis für eine markierte URL.
#[derive(Debug, Clone, PartialEq)]
pub struct LinkComparison {
    /// Existenz der URL.
    pub presence: Presence,
    /// Zeilen des Current-Crawls (Hinzugefügt, Ankertext geändert, Unverändert).
    pub current: Vec<LinkRow>,
    /// Zeilen des Previous-Crawls (Entfernt, Ankertext geändert, Unverändert).
    pub previous: Vec<LinkRow>,
}

impl LinkComparison {
    /// Änderungsfilter verfügbar (nicht ausgegraut): nur, wenn die URL in beiden Crawls existiert.
    pub fn filters_enabled(&self) -> bool {
        self.presence == Presence::Both
    }

    /// Zeilen der Ansicht für Quelle und Änderung. Existiert die URL nur in einem Crawl, gelten die Änderungsfilter
    /// nicht und die Ansicht zeigt die Links dieses Crawls.
    pub fn view(&self, source: LinkSource, upper: Side, change: ChangeFilter) -> Vec<&LinkRow> {
        match self.presence {
            Presence::CurrentOnly => return self.current.iter().collect(),
            Presence::PreviousOnly => return self.previous.iter().collect(),
            Presence::Both => {}
        }
        let side = source.resolve(upper);
        let rows = |s: Side| match s {
            Side::Current => &self.current,
            Side::Previous => &self.previous,
        };
        match change {
            ChangeFilter::All => rows(side).iter().collect(),
            ChangeFilter::Only(LinkChange::Added) => self
                .current
                .iter()
                .filter(|r| r.change == Some(LinkChange::Added))
                .collect(),
            ChangeFilter::Only(LinkChange::Removed) => self
                .previous
                .iter()
                .filter(|r| r.change == Some(LinkChange::Removed))
                .collect(),
            ChangeFilter::Only(c) => rows(side).iter().filter(|r| r.change == Some(c)).collect(),
        }
    }
}

struct Instance {
    key: Identity,
    values: Vec<Value>,
    anchor: String,
}

fn cell(v: rusqlite::types::ValueRef<'_>) -> Value {
    use rusqlite::types::ValueRef;
    match v {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) => Value::Int(i),
        ValueRef::Real(f) => Value::Float(f),
        ValueRef::Text(t) | ValueRef::Blob(t) => {
            Value::Text(String::from_utf8_lossy(t).into_owned())
        }
    }
}

fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Liest die Link-Instanzen einer URL in Dokumentreihenfolge; `map` bildet Quelle und Ziel auf die Identität ab.
fn instances(
    file: &CrawlFile,
    url_id: i64,
    direction: LinkDirection,
    map: &dyn Fn(&str) -> String,
) -> Result<Vec<Instance>, CompareError> {
    let (filter, order) = match direction {
        LinkDirection::Outlinks => ("l.source_id = ?1", "l.position ASC, l.id ASC"),
        LinkDirection::Inlinks => (
            "l.target_id = ?1",
            "s.address ASC, l.position ASC, l.id ASC",
        ),
    };
    let sql = format!(
        "SELECT l.link_type, s.address, COALESCE(t.address, l.target_raw), t.size_bytes, l.alt_text, l.anchor, \
                CASE WHEN t.state = 'done' AND t.status_code > 0 THEN t.status_code END, \
                CASE WHEN t.id IS NULL THEN NULL \
                     WHEN t.state <> 'done' OR t.status_code IS NULL THEN 'Not Crawled' ELSE t.status END, \
                CASE WHEN l.follow <> 0 THEN 'True' ELSE 'False' END, \
                l.target_attr, l.rel, l.path_type, l.link_path, l.link_position, l.link_origin, \
                s.url, COALESCE(t.url, l.target_raw) \
         FROM links l JOIN urls s ON s.id = l.source_id LEFT JOIN urls t ON t.id = l.target_id \
         WHERE {filter} ORDER BY {order}"
    );
    let mut stmt = file.conn().prepare(&sql)?;
    let mut rows = stmt.query([url_id])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let values: Vec<Value> = (0..LINK_COLUMNS.len())
            .map(|i| row.get_ref(i).map(cell))
            .collect::<Result<_, _>>()?;
        let link_type = match &values[0] {
            Value::Text(t) => t.clone(),
            _ => String::new(),
        };
        let source: String = row.get(15)?;
        let target: String = row.get(16)?;
        let anchor = match &values[ANCHOR] {
            Value::Text(t) => normalize_ws(t),
            _ => String::new(),
        };
        out.push(Instance {
            key: (map(&source), map(&target), link_type),
            values,
            anchor,
        });
    }
    Ok(out)
}

type Identity = (String, String, String);

struct Group {
    values: Vec<Value>,
    anchors: BTreeSet<String>,
    first_anchor: String,
}

fn group(instances: Vec<Instance>) -> (Vec<Identity>, HashMap<Identity, Group>) {
    let mut order = Vec::new();
    let mut groups: HashMap<Identity, Group> = HashMap::new();
    for i in instances {
        match groups.get_mut(&i.key) {
            Some(g) => {
                g.anchors.insert(i.anchor);
            }
            None => {
                order.push(i.key.clone());
                groups.insert(
                    i.key,
                    Group {
                        values: i.values,
                        anchors: BTreeSet::from([i.anchor.clone()]),
                        first_anchor: i.anchor,
                    },
                );
            }
        }
    }
    (order, groups)
}

fn joined(anchors: &BTreeSet<String>) -> String {
    anchors.iter().cloned().collect::<Vec<_>>().join(" | ")
}

/// Vergleicht die Links einer URL: `current_id`/`previous_id` sind die Kennungen der URL in beiden Crawls (`None`,
/// wenn sie dort nicht existiert).
pub fn compare_links(
    current: &CrawlFile,
    previous: &CrawlFile,
    mapping: &UrlMapping,
    current_id: Option<i64>,
    previous_id: Option<i64>,
    direction: LinkDirection,
) -> Result<LinkComparison, CompareError> {
    let identity = |u: &str| u.to_string();
    let mapped = |u: &str| {
        if mapping.is_empty() {
            u.to_string()
        } else {
            mapping.apply(u).url
        }
    };
    let cur = match current_id {
        Some(id) => instances(current, id, direction, &identity)?,
        None => Vec::new(),
    };
    let prev = match previous_id {
        Some(id) => instances(previous, id, direction, &mapped)?,
        None => Vec::new(),
    };
    let presence = match (current_id, previous_id) {
        (Some(_), Some(_)) => Presence::Both,
        (Some(_), None) => Presence::CurrentOnly,
        (None, Some(_)) => Presence::PreviousOnly,
        (None, None) => {
            return Err(CompareError::Invalid(
                "URL existiert in keinem der beiden Crawls".into(),
            ))
        }
    };
    let (cur_order, cur_groups) = group(cur);
    let (prev_order, prev_groups) = group(prev);
    let both = presence == Presence::Both;
    let mut current_rows = Vec::new();
    for key in &cur_order {
        let g = &cur_groups[key];
        let (change, previous_anchor) = match prev_groups.get(key) {
            _ if !both => (None, None),
            None => (Some(LinkChange::Added), None),
            Some(p) if p.anchors != g.anchors => (
                Some(LinkChange::AnchorChanged),
                Some(if p.anchors.len() == 1 {
                    p.first_anchor.clone()
                } else {
                    joined(&p.anchors)
                }),
            ),
            Some(_) => (Some(LinkChange::Unchanged), None),
        };
        current_rows.push(LinkRow {
            values: g.values.clone(),
            change,
            crawl: Side::Current,
            previous_anchor,
        });
    }
    let mut previous_rows = Vec::new();
    for key in &prev_order {
        let g = &prev_groups[key];
        let (change, previous_anchor) = match cur_groups.get(key) {
            _ if !both => (None, None),
            None => (Some(LinkChange::Removed), None),
            Some(c) if c.anchors != g.anchors => (
                Some(LinkChange::AnchorChanged),
                Some(if g.anchors.len() == 1 {
                    g.first_anchor.clone()
                } else {
                    joined(&g.anchors)
                }),
            ),
            Some(_) => (Some(LinkChange::Unchanged), None),
        };
        previous_rows.push(LinkRow {
            values: g.values.clone(),
            change,
            crawl: Side::Previous,
            previous_anchor,
        });
    }
    Ok(LinkComparison {
        presence,
        current: current_rows,
        previous: previous_rows,
    })
}
