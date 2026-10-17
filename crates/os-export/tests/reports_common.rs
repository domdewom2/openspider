//! Gemeinsame Hilfen der Report-, Issues- und Sitemap-Tests (M1-P17); als `#[path]`-Modul eingebunden.
//! Eigenes Testziel ohne Tests.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use os_analyze::{ColumnDef, ColumnKind, ColumnSql, FilterDef, FilterKind, SortKey, TabDef};
use os_model::{CrawlConfig, Value};

/// Gelesene CSV-Datei: Kopfzeile und Datenzeilen (BOM wird verworfen).
pub struct Table {
    pub header: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl Table {
    pub fn read(path: &Path) -> Table {
        let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_reader(bytes);
        let mut all: Vec<Vec<String>> = rdr
            .records()
            .map(|r| r.unwrap().iter().map(str::to_string).collect())
            .collect();
        let header = all.remove(0);
        Table { header, rows: all }
    }

    /// Index der Spalte (muss genau einmal vorkommen).
    pub fn col(&self, name: &str) -> usize {
        let hits: Vec<usize> = self
            .header
            .iter()
            .enumerate()
            .filter(|(_, h)| h.as_str() == name)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(hits.len(), 1, "Spalte {name:?} in {:?}", self.header);
        hits[0]
    }

    /// Wert einer Zelle der Zeile `row`.
    pub fn get<'a>(&self, row: &'a [String], name: &str) -> &'a str {
        row.get(self.col(name)).map_or("", String::as_str)
    }

    /// Erste Zeile, deren Spalte `name` auf `suffix` endet.
    pub fn find(&self, name: &str, suffix: &str) -> Option<&Vec<String>> {
        let c = self.col(name);
        self.rows.iter().find(|r| r[c].ends_with(suffix))
    }

    /// Wie [`find`](Self::find), mit Panik bei fehlender Zeile.
    pub fn row(&self, name: &str, suffix: &str) -> &Vec<String> {
        self.find(name, suffix)
            .unwrap_or_else(|| panic!("keine Zeile mit {name} auf {suffix:?}"))
    }

    /// Alle Werte einer Spalte.
    pub fn column(&self, name: &str) -> Vec<&str> {
        let c = self.col(name);
        self.rows.iter().map(|r| r[c].as_str()).collect()
    }
}

/// Standard-Konfiguration für die Fixture-Crawls (unbegrenzte Tiefe, Subdomains aus).
pub fn base_config() -> CrawlConfig {
    CrawlConfig::default()
}

/// Pfad einer Datei im Ausgabeordner.
pub fn out(dir: &Path, name: &str) -> PathBuf {
    dir.join(name)
}

fn col(key: &'static str, sql: &'static str, kind: ColumnKind, visible: bool) -> ColumnDef {
    ColumnDef {
        key,
        sql: ColumnSql::Expr(sql),
        kind,
        default_visible: visible,
        extension: false,
        ..ColumnDef::META
    }
}

fn filter(id: &'static str, predicate: &'static str) -> FilterDef {
    FilterDef {
        id,
        predicate,
        kind: FilterKind::Live,
        requires: Vec::new(),
        ignore_non_indexable: false,
    }
}

fn sort_address() -> Vec<SortKey> {
    vec![SortKey {
        column: "Address".to_string(),
        descending: false,
    }]
}

/// Echte Prädikate für die Katalog-Tabs „Page Titles“ (Missing, Duplicate, Over …), „H1“ (Missing) und
/// „Response Codes“ (Internal Client Error (4XX)) – Platz für die Pakete P20–P25, die diese Tabs sonst
/// liefern. Namen und Filterkennungen stimmen mit `core/issues/catalog.toml` überein.
pub fn issue_tabs() -> Vec<TabDef> {
    vec![
        TabDef {
            id: "Page Titles",
            population: "u.is_internal = 1 AND u.in_tables = 1 AND u.content_class = 'HTML' AND u.status_code BETWEEN 200 AND 299",
            columns: vec![
                col("Address", "u.address", ColumnKind::Url, true),
                col("Title 1", "u.title_1", ColumnKind::Text, true),
                col("Title 1 Length", "u.title_1_len", ColumnKind::Int, true),
            ],
            filters: vec![
                filter("All", "1"),
                filter("Missing", "(u.title_1 IS NULL OR u.title_1 = '')"),
                filter(
                    "Duplicate",
                    "(u.title_1 <> '' AND (SELECT COUNT(*) FROM urls d WHERE d.title_1 = u.title_1 AND d.is_internal = 1 AND d.content_class = 'HTML' AND d.status_code BETWEEN 200 AND 299) > 1)",
                ),
                filter(
                    "Over {title.max_chars} Characters",
                    "u.title_1_len > :title_max_chars",
                ),
            ],
            default_sort: sort_address(),
            params: |cfg| {
                vec![(
                    "title_max_chars",
                    Value::Int(i64::from(cfg.title.max_chars)),
                )]
            },
        },
        TabDef {
            id: "H1",
            population: "u.is_internal = 1 AND u.in_tables = 1 AND u.content_class = 'HTML' AND u.status_code BETWEEN 200 AND 299",
            columns: vec![
                col("Address", "u.address", ColumnKind::Url, true),
                col("H1-1", "u.h1_1", ColumnKind::Text, true),
            ],
            filters: vec![
                filter("All", "1"),
                filter("Missing", "(u.h1_1 IS NULL OR u.h1_1 = '')"),
            ],
            default_sort: sort_address(),
            params: |_| Vec::new(),
        },
        TabDef {
            id: "Response Codes",
            population: "u.in_tables = 1 AND u.state = 'done'",
            columns: vec![
                col("Address", "u.address", ColumnKind::Url, true),
                col("Status Code", "u.status_code", ColumnKind::Int, true),
                col("Status", "u.status", ColumnKind::Text, true),
            ],
            filters: vec![
                filter("All", "1"),
                filter(
                    "Internal Client Error (4XX)",
                    "(u.is_internal = 1 AND u.status_code BETWEEN 400 AND 499)",
                ),
            ],
            default_sort: sort_address(),
            params: |_| Vec::new(),
        },
    ]
}
