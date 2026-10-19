//! Element tab h1: stored values and live filters.

use os_model::Value;

use super::duplicates::{
    address_column, all_filter, column, duplicate_predicate, element_params, indexability_columns,
    missing_predicate, population, problem_filter,
};
use crate::defs::{ColumnKind, TabDef};

/// Definition des Tabs „H1“.
pub fn tab() -> TabDef {
    let mut columns = vec![
        address_column(),
        column(
            "Occurrences",
            "COALESCE(u.h1_count, 0)",
            ColumnKind::Int,
            true,
        ),
        column("H1-1", "u.h1_1", ColumnKind::Text, true),
        column(
            "H1-1 Length",
            "COALESCE(u.h1_1_len, 0)",
            ColumnKind::Int,
            true,
        ),
        column("H1-2", "u.h1_2", ColumnKind::Text, true),
        column(
            "H1-2 Length",
            "COALESCE(u.h1_2_len, 0)",
            ColumnKind::Int,
            true,
        ),
    ];
    columns.extend(indexability_columns());
    TabDef {
        id: "H1",
        population: population!("u"),
        columns,
        filters: vec![
            all_filter(),
            // ONPAGE-121
            problem_filter("Missing", missing_predicate!("h1_1", "h1_2")),
            // ONPAGE-122 / ONPAGE-005
            problem_filter("Duplicate", duplicate_predicate!("h1_1")),
            // ONPAGE-123: nur H1-1 zählt (A-32).
            problem_filter(
                "Over {h1.max_chars} Characters",
                "u.h1_1_len > :h1_max_chars",
            ),
            // ONPAGE-124: Occurrences > 1, auch leere H1 zählen.
            problem_filter("Multiple", "u.h1_count > 1"),
            problem_filter("Alt Text in h1", "EXISTS (SELECT 1 FROM url_ext e WHERE e.url_id = u.id AND e.key = 'head.h1_alt' AND e.v_int = 1)"),
            problem_filter("Non-sequential", "EXISTS (SELECT 1 FROM url_ext e WHERE e.url_id = u.id AND e.key = 'head.h1_non_sequential' AND e.v_int = 1)"),
        ],
        // Keine Tab-Sortierung: Entdeckungsreihenfolge; nur `Duplicate` gruppiert (tabs::filter_default_sort).
        default_sort: Vec::new(),
        params: |cfg| {
            let mut p = element_params(cfg, cfg.extraction.h1);
            p.push(("h1_max_chars", Value::Int(i64::from(cfg.h1.max_chars))));
            p
        },
        hidden_population: None,
    }
}
