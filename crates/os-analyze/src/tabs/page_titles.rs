//! Element tab page_titles: stored values and live filters.

use os_model::Value;

use super::duplicates::{
    address_column, all_filter, column, duplicate_predicate, element_params, indexability_columns,
    missing_predicate, population, problem_filter,
};
use crate::defs::{ColumnKind, TabDef};

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
            true,
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
            true,
        ),
    ];
    columns.push(column("H1-1", "u.h1_1", ColumnKind::Text, false));
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
            problem_filter("Over {title.max_px} Pixels", "u.title_1_px > :title_max_px"),
            problem_filter(
                "Below {title.min_px} Pixels",
                "u.title_1_px > 0 AND u.title_1_px < :title_min_px",
            ),
            {
                let mut f = problem_filter(
                    "Same as H1",
                    "u.title_1 <> '' AND u.title_1 = u.h1_1 COLLATE BINARY",
                );
                f.default_columns = &[
                    "Address",
                    "Occurrences",
                    "Title 1",
                    "Title 1 Length",
                    "Title 1 Pixel Width",
                    "Title 2",
                    "Title 2 Length",
                    "Title 2 Pixel Width",
                    "H1-1",
                    "Indexability",
                    "Indexability Status",
                ];
                f
            },
            {
                let mut f = problem_filter("Outside <head>", "EXISTS (SELECT 1 FROM url_ext e WHERE e.url_id = u.id AND e.key = 'head.title_outside' AND e.v_int = 1)");
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
            let mut p = element_params(cfg, cfg.extraction.page_titles);
            p.push((
                "title_max_chars",
                Value::Int(i64::from(cfg.title.max_chars)),
            ));
            p.push((
                "title_min_chars",
                Value::Int(i64::from(cfg.title.min_chars)),
            ));
            p.push(("title_max_px", Value::Int(i64::from(cfg.title.max_px))));
            p.push(("title_min_px", Value::Int(i64::from(cfg.title.min_px))));
            p
        },
        hidden_population: None,
    }
}
