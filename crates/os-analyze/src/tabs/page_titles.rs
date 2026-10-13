//! Tab „Page Titles“ (ONPAGE-090 … 094, 098; Filter-Kennungen aus `docs/plan/M1-filters.toml`).
//!
//! Grundmenge: interne HTML-Seiten mit 2xx (ONPAGE-009, siehe [`super::duplicates`]). Die Werte (`Title 1/2`,
//! Länge in Codepoints nach N(x), Pixelbreite aus P05) stehen fertig in `urls`; die Filter rechnen nur mit
//! diesen Rohwerten und den Schwellen aus der Konfiguration (ONPAGE-002) – eine Änderung wirkt ohne Recrawl.
//! Die Pixel-Filter (ONPAGE-095/096) sind P1; die Pixelbreiten stehen nur als Spalten da.

use os_model::Value;

use super::duplicates::{
    address_column, all_filter, column, duplicate_predicate, element_params, indexability_columns,
    missing_predicate, population, problem_filter,
};
use crate::defs::{ColumnKind, SortKey, TabDef};

/// Definition des Tabs „Page Titles“.
pub fn tab() -> TabDef {
    let mut columns = vec![
        address_column(),
        column(
            "Occurrences",
            "MIN(COALESCE(u.title_count, 0), 2)",
            ColumnKind::Int,
            true,
        ),
        column("Title 1", "u.title_1", ColumnKind::Text, true),
        column(
            "Title 1 Length",
            "COALESCE(u.title_1_len, 0)",
            ColumnKind::Int,
            true,
        ),
        column(
            "Title 1 Pixel Width",
            "COALESCE(u.title_1_px, 0)",
            ColumnKind::Int,
            false,
        ),
        column("Title 2", "u.title_2", ColumnKind::Text, true),
        column(
            "Title 2 Length",
            "COALESCE(u.title_2_len, 0)",
            ColumnKind::Int,
            true,
        ),
        column(
            "Title 2 Pixel Width",
            "COALESCE(u.title_2_px, 0)",
            ColumnKind::Int,
            false,
        ),
    ];
    columns.extend(indexability_columns());
    TabDef {
        id: "Page Titles",
        population: population!("u"),
        columns,
        filters: vec![
            all_filter(),
            // ONPAGE-091: kein <title> oder alle Vorkommen nach N(x) leer.
            problem_filter("Missing", missing_predicate!("title_1", "title_2")),
            // ONPAGE-092 / ONPAGE-005.
            problem_filter("Duplicate", duplicate_predicate!("title_1")),
            // ONPAGE-093: Title 1 Length > title.max_chars.
            problem_filter(
                "Over {title.max_chars} Characters",
                "u.title_1_len > :title_max_chars",
            ),
            // ONPAGE-094: 0 < Title 1 Length < title.min_chars (fehlende Titel gehören nicht dazu).
            problem_filter(
                "Below {title.min_chars} Characters",
                "(u.title_1_len > 0 AND u.title_1_len < :title_min_chars)",
            ),
            // ONPAGE-098: mehr als ein <title> (Anzahl auf 2 gedeckelt gespeichert, > 1 genügt).
            problem_filter("Multiple", "u.title_count > 1"),
        ],
        // ONPAGE-092: Gleiche Titel stehen im Duplicate-Filter direkt untereinander.
        default_sort: vec![SortKey {
            column: "Title 1".to_string(),
            descending: false,
        }],
        params: |cfg| {
            let mut p = element_params(cfg, cfg.extraction.page_titles);
            p.push((
                "title_max_chars",
                Value::Int(i64::from(cfg.title.max_chars)),
            ));
            p.push((
                "title_min_chars",
                Value::Int(i64::from(cfg.title.min_chars)),
            ));
            p
        },
    }
}
