//! Meta Keywords: fixed first/second values and live filters (ONPAGE-110 … 113).

use super::duplicates::{
    address_column, all_filter, column, duplicate_predicate, element_params, indexability_columns,
    missing_predicate, population, problem_filter,
};
use crate::defs::{ColumnKind, TabDef};

/// Definition des Tabs „Meta Keywords“.
pub fn tab() -> TabDef {
    let mut columns = vec![
        address_column(),
        column(
            "Occurrences",
            "MIN(COALESCE(u.meta_kw_count, 0), 2)",
            ColumnKind::Int,
            true,
        ),
        column("Meta Keywords 1", "u.meta_kw_1", ColumnKind::Text, true),
        column(
            "Meta Keywords 1 Length",
            "COALESCE(u.meta_kw_1_len, 0)",
            ColumnKind::Int,
            true,
        ),
        column("Meta Keywords 2", "u.meta_kw_2", ColumnKind::Text, true),
        column(
            "Meta Keywords 2 Length",
            "COALESCE(u.meta_kw_2_len, 0)",
            ColumnKind::Int,
            true,
        ),
    ];
    columns.extend(indexability_columns());
    TabDef {
        id: "Meta Keywords",
        population: population!("u"),
        columns,
        filters: vec![
            all_filter(),
            // ONPAGE-111
            problem_filter("Missing", concat!(
                missing_predicate!("meta_kw_1", "meta_kw_2"),
                " AND NOT EXISTS (SELECT 1 FROM url_ext e WHERE e.url_id = u.id AND e.key = 'head.meta_keywords_nonempty' AND e.v_int = 1)",
            )),
            // ONPAGE-112 / ONPAGE-005
            problem_filter("Duplicate", duplicate_predicate!("meta_kw_1")),
            // ONPAGE-113: all occurrences count, including empty values.
            problem_filter("Multiple", "u.meta_kw_count > 1"),
        ],
        // Keine Tab-Sortierung: Entdeckungsreihenfolge; nur `Duplicate` gruppiert (tabs::filter_default_sort).
        default_sort: Vec::new(),
        params: |cfg| element_params(cfg, cfg.extraction.meta_keywords),
        hidden_population: None,
    }
}
