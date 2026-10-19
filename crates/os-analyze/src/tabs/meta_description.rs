//! Element tab meta_description: stored values and live filters.

use os_model::Value;

use super::duplicates::{
    address_column, all_filter, column, duplicate_predicate, element_params, indexability_columns,
    missing_predicate, population, problem_filter,
};
use crate::defs::{ColumnKind, TabDef};

/// Definition des Tabs „Meta Description“.
pub fn tab() -> TabDef {
    let mut columns = vec![
        address_column(),
        column(
            "Occurrences",
            "MIN(COALESCE(u.meta_desc_count, 0), 2)",
            ColumnKind::Int,
            true,
        ),
        column(
            "Meta Description 1",
            "u.meta_desc_1",
            ColumnKind::Text,
            true,
        ),
        column(
            "Meta Description 1 Length",
            "COALESCE(u.meta_desc_1_len, 0)",
            ColumnKind::Int,
            true,
        ),
        column(
            "Meta Description 1 Pixel Width",
            "COALESCE(u.meta_desc_1_px, 0)",
            ColumnKind::Int,
            true,
        ),
        column(
            "Meta Description 2",
            "u.meta_desc_2",
            ColumnKind::Text,
            true,
        ),
        column(
            "Meta Description 2 Length",
            "COALESCE(u.meta_desc_2_len, 0)",
            ColumnKind::Int,
            true,
        ),
        column(
            "Meta Description 2 Pixel Width",
            "COALESCE(u.meta_desc_2_px, 0)",
            ColumnKind::Int,
            true,
        ),
    ];
    columns.extend(indexability_columns());
    TabDef {
        id: "Meta Description",
        population: population!("u"),
        columns,
        filters: vec![
            all_filter(),
            // ONPAGE-101
            problem_filter("Missing", missing_predicate!("meta_desc_1", "meta_desc_2")),
            // ONPAGE-102 / ONPAGE-005
            problem_filter("Duplicate", duplicate_predicate!("meta_desc_1")),
            // ONPAGE-103
            problem_filter(
                "Over {meta_description.max_chars} Characters",
                "u.meta_desc_1_len > :md_max_chars",
            ),
            // ONPAGE-104
            problem_filter(
                "Below {meta_description.min_chars} Characters",
                "(u.meta_desc_1_len > 0 AND u.meta_desc_1_len < :md_min_chars)",
            ),
            // ONPAGE-107
            problem_filter("Multiple", "u.meta_desc_count > 1"),
            problem_filter(
                "Over {meta_description.max_px} Pixels",
                "u.meta_desc_1_px > :md_max_px",
            ),
            problem_filter(
                "Below {meta_description.min_px} Pixels",
                "u.meta_desc_1_px > 0 AND u.meta_desc_1_px < :md_min_px",
            ),
            {
                let mut f = problem_filter("Outside <head>", "EXISTS (SELECT 1 FROM url_ext e WHERE e.url_id = u.id AND e.key = 'head.description_outside' AND e.v_int = 1)");
                f.requires.push(crate::defs::Requirement::Coverage {
                    config: "advanced.html_validation",
                    marker: "validation.checked",
                });
                f
            },
        ],
        // Keine Tab-Sortierung: Entdeckungsreihenfolge; nur `Duplicate` gruppiert (tabs::filter_default_sort).
        default_sort: Vec::new(),
        params: |cfg| {
            let mut p = element_params(cfg, cfg.extraction.meta_description);
            p.push((
                "md_max_chars",
                Value::Int(i64::from(cfg.meta_description.max_chars)),
            ));
            p.push((
                "md_min_chars",
                Value::Int(i64::from(cfg.meta_description.min_chars)),
            ));
            p.push((
                "md_max_px",
                Value::Int(i64::from(cfg.meta_description.max_px)),
            ));
            p.push((
                "md_min_px",
                Value::Int(i64::from(cfg.meta_description.min_px)),
            ));
            p
        },
        hidden_population: None,
    }
}
