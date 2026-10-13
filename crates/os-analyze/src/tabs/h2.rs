//! Tab „H2“ (ONPAGE-130 … 134; Filter-Kennungen aus `docs/plan/M1-filters.toml`).
//!
//! Wie [`super::h1`] für `<h2>`. „Non-sequential“ (ONPAGE-135) ist P1.

use os_model::Value;

use super::duplicates::{
    address_column, all_filter, column, duplicate_predicate, element_params, indexability_columns,
    missing_predicate, population, problem_filter,
};
use crate::defs::{ColumnKind, SortKey, TabDef};

/// Definition des Tabs „H2“.
pub fn tab() -> TabDef {
    let mut columns = vec![
        address_column(),
        column(
            "Occurrences",
            "COALESCE(u.h2_count, 0)",
            ColumnKind::Int,
            true,
        ),
        column("H2-1", "u.h2_1", ColumnKind::Text, true),
        column(
            "H2-1 Length",
            "COALESCE(u.h2_1_len, 0)",
            ColumnKind::Int,
            true,
        ),
        column("H2-2", "u.h2_2", ColumnKind::Text, true),
        column(
            "H2-2 Length",
            "COALESCE(u.h2_2_len, 0)",
            ColumnKind::Int,
            true,
        ),
    ];
    columns.extend(indexability_columns());
    TabDef {
        id: "H2",
        population: population!("u"),
        columns,
        filters: vec![
            all_filter(),
            // ONPAGE-131
            problem_filter("Missing", missing_predicate!("h2_1", "h2_2")),
            // ONPAGE-132 / ONPAGE-005
            problem_filter("Duplicate", duplicate_predicate!("h2_1")),
            // ONPAGE-133
            problem_filter(
                "Over {h2.max_chars} Characters",
                "u.h2_1_len > :h2_max_chars",
            ),
            // ONPAGE-134
            problem_filter("Multiple", "u.h2_count > 1"),
        ],
        default_sort: vec![SortKey {
            column: "H2-1".to_string(),
            descending: false,
        }],
        params: |cfg| {
            let mut p = element_params(cfg, cfg.extraction.h2);
            p.push(("h2_max_chars", Value::Int(i64::from(cfg.h2.max_chars))));
            p
        },
    }
}
