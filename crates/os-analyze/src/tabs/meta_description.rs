//! Tab „Meta Description“ (ONPAGE-100 … 104, 107; Filter-Kennungen aus `docs/plan/M1-filters.toml`).
//!
//! Wie [`super::page_titles`]: Rohwerte aus `urls`, Schwellen aus der Konfiguration, Grundmenge und
//! Duplikat-Logik aus [`super::duplicates`]. Die Pixel-Filter (ONPAGE-105/106) und „Outside <head>“
//! (ONPAGE-108) sind P1.

use os_model::Value;

use super::duplicates::{
    address_column, all_filter, column, duplicate_predicate, element_params, indexability_columns,
    missing_predicate, population, problem_filter,
};
use crate::defs::{ColumnKind, SortKey, TabDef};

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
            false,
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
            false,
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
        ],
        default_sort: vec![SortKey {
            column: "Meta Description 1".to_string(),
            descending: false,
        }],
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
            p
        },
    }
}
