//! Tab „Security“ (ONPAGE-030, ONPAGE-031, ONPAGE-032).
//!
//! Grundmenge („All“): alle internen URLs, die in den Tabellen erscheinen (alle Inhaltstypen). `HTTP URLs`
//! und `HTTPS URLs` unterscheiden nur das Schema der URL (F-TLS); da Schema-Wechsel desselben Hosts intern
//! bleiben, ist die Vereinigung beider Filter die Grundmenge. Externe URLs erscheinen nie.
//!
//! Die übrigen Security-Filter (ONPAGE-033 … ONPAGE-043) sind P1 und nicht Teil von M1.

use crate::defs::{ColumnKind, FilterDef, FilterKind, TabDef};
use crate::tabs::internal::{
    base_columns, multi, CANONICAL_LIST, META_ROBOTS_LIST, POPULATION, X_ROBOTS_LIST,
};

fn filter(id: &'static str, predicate: &'static str) -> FilterDef {
    FilterDef {
        id,
        predicate,
        kind: FilterKind::Live,
        requires: Vec::new(),
        ignore_non_indexable: false,
        default_columns: &[],
    }
}

/// Definition des Tabs „Security“.
pub fn tab() -> TabDef {
    let mut columns = base_columns();
    columns.extend([
        multi("Canonical Link Element", CANONICAL_LIST, ColumnKind::Url),
        multi("Meta Robots", META_ROBOTS_LIST, ColumnKind::Text),
        multi("X-Robots-Tag", X_ROBOTS_LIST, ColumnKind::Text),
    ]);
    TabDef {
        id: "Security",
        population: POPULATION,
        columns,
        filters: vec![
            filter("All", "1"),
            filter("HTTP URLs", "u.url LIKE 'http://%'"),
            filter("HTTPS URLs", "u.url LIKE 'https://%'"),
        ],
        default_sort: Vec::new(),
        params: |_| Vec::new(),
        hidden_population: None,
    }
}
