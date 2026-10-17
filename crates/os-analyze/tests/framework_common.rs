//! Gemeinsame Hilfen der Rahmen-Tests (`framework_*.rs`, M1-P14); als `#[path]`-Modul eingebunden.
//! Eigenes Testziel ohne Tests.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use os_analyze::{
    ColumnDef, ColumnKind, ColumnSql, FilterDef, FilterKind, Requirement, SortKey, TabDef,
};
use os_model::time::{IdSource, SeededIds};
use os_model::{AnalysisStep, CrawlConfig, CrawlMode, Timestamp, Value};
use os_store::{CrawlFile, CrawlInit, StorageProfile};

pub fn created_at() -> Timestamp {
    Timestamp::from_db("2026-10-13T07:35:00.000Z").unwrap()
}

pub fn init(seed: u64, config: CrawlConfig) -> CrawlInit {
    CrawlInit {
        crawl_id: SeededIds::new(seed).new_crawl_id(created_at()),
        mode: CrawlMode::Spider,
        start_url: Some("https://site.test/".into()),
        list_source: None,
        project: "Projekt".into(),
        name: "Rahmentest".into(),
        config,
        created_at: created_at(),
    }
}

/// Legt eine leere Crawl-Datei an.
pub fn create(dir: &Path, name: &str, seed: u64) -> (PathBuf, CrawlFile) {
    let path = dir.join(name);
    let file = CrawlFile::create(
        &path,
        &init(seed, CrawlConfig::default()),
        StorageProfile::Balanced,
    )
    .unwrap();
    (path, file)
}

/// Eine URL-Zeile; weitere Spalten über `set`.
pub struct Row {
    pub id: i64,
    pub address: String,
    pub internal: bool,
    pub extra: Vec<(&'static str, rusqlite::types::Value)>,
}

pub fn row(id: i64, address: &str) -> Row {
    Row {
        id,
        address: address.to_string(),
        internal: true,
        extra: Vec::new(),
    }
}

impl Row {
    pub fn external(mut self) -> Row {
        self.internal = false;
        self
    }

    fn set(mut self, col: &'static str, v: rusqlite::types::Value) -> Row {
        self.extra.retain(|(c, _)| *c != col);
        self.extra.push((col, v));
        self
    }

    pub fn int(self, col: &'static str, v: i64) -> Row {
        self.set(col, rusqlite::types::Value::Integer(v))
    }

    pub fn text(self, col: &'static str, v: &str) -> Row {
        self.set(col, rusqlite::types::Value::Text(v.to_string()))
    }

    pub fn real(self, col: &'static str, v: f64) -> Row {
        self.set(col, rusqlite::types::Value::Real(v))
    }

    pub fn null(self, col: &'static str) -> Row {
        self.set(col, rusqlite::types::Value::Null)
    }
}

/// Fügt URL-Zeilen direkt per SQL ein (ohne Engine und Sink).
pub fn insert(file: &CrawlFile, rows: &[Row]) {
    for r in rows {
        let mut cols = vec![
            "id",
            "url",
            "address",
            "origin",
            "is_internal",
            "state",
            "discovered_via",
            "folder_depth",
            "url_length",
        ];
        let mut vals: Vec<rusqlite::types::Value> = vec![
            r.id.into(),
            format!("https://site.test/{}", r.address).into(),
            r.address.clone().into(),
            "https://site.test".to_string().into(),
            i64::from(r.internal).into(),
            "Done".to_string().into(),
            "Link".to_string().into(),
            0i64.into(),
            (r.address.chars().count() as i64).into(),
        ];
        for (c, v) in &r.extra {
            cols.push(c);
            vals.push(v.clone());
        }
        let placeholders = (1..=cols.len())
            .map(|i| format!("?{i}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "INSERT INTO urls ({}) VALUES ({placeholders})",
            cols.join(", ")
        );
        file.conn()
            .execute(&sql, rusqlite::params_from_iter(vals))
            .unwrap();
    }
}

pub fn set_change_seq(file: &CrawlFile, n: u64) {
    file.set_meta("change_seq", &n.to_string()).unwrap();
}

fn col(key: &'static str, sql: &'static str, kind: ColumnKind, default_visible: bool) -> ColumnDef {
    ColumnDef {
        key,
        sql: ColumnSql::Expr(sql),
        kind,
        default_visible,
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

/// Test-Tab „Test“: interne URLs, Titel, Statuscode, Mehrfachspalte „Canonical“ (aus `canonicals`).
pub fn test_tab() -> TabDef {
    TabDef {
        id: "Test",
        population: "u.is_internal = 1",
        columns: vec![
            // Address ist hier absichtlich als Text deklariert: die Sortierregel gilt trotzdem bytewise.
            col("Address", "u.address", ColumnKind::Text, true),
            col("Title 1", "u.title_1", ColumnKind::Text, true),
            col("Title 1 Length", "u.title_1_len", ColumnKind::Int, true),
            col("Status Code", "u.status_code", ColumnKind::Int, true),
            col("Indexable", "u.effective_index", ColumnKind::Bool, true),
            col(
                "Text Ratio",
                "u.text_ratio",
                ColumnKind::Float { decimals: 2 },
                false,
            ),
            col(
                "Response Time",
                "u.response_time_ms",
                ColumnKind::Seconds3,
                false,
            ),
            col("Hash", "u.hash_md5", ColumnKind::Hash, false),
            col("Path", "u.address", ColumnKind::Url, false),
            ColumnDef {
                key: "Canonical",
                sql: ColumnSql::Multi {
                    base: "Canonical",
                    list_sql: "SELECT raw FROM canonicals WHERE url_id = ?1 ORDER BY position",
                },
                kind: ColumnKind::Text,
                default_visible: true,
                extension: false,
                ..ColumnDef::META
            },
            ColumnDef {
                key: "Ext Spalte",
                sql: ColumnSql::Expr("u.crawl_depth"),
                kind: ColumnKind::Int,
                default_visible: false,
                extension: true,
                ..ColumnDef::META
            },
        ],
        filters: vec![
            filter("All", "1"),
            filter("Missing Title", "(u.title_1 IS NULL OR u.title_1 = '')"),
            filter("Over {title.max_chars} Characters", "u.title_1_len > :max"),
            FilterDef {
                ignore_non_indexable: true,
                ..filter(
                    "Missing Title (Indexable)",
                    "(u.title_1 IS NULL OR u.title_1 = '')",
                )
            },
            FilterDef {
                kind: FilterKind::PostCrawl(AnalysisStep::Canonicals),
                ..filter("Needs Analysis", "u.canonical_count > 0")
            },
            FilterDef {
                requires: vec![Requirement::Config("duplicates.near_enabled")],
                kind: FilterKind::PostCrawl(AnalysisStep::Content),
                ..filter("Needs Config", "1")
            },
            FilterDef {
                requires: vec![Requirement::Rendering],
                ..filter("Needs Rendering", "1")
            },
            filter("Errors", "u.status_code >= 400"),
        ],
        default_sort: Vec::new(),
        params: |cfg| vec![("max", Value::Int(i64::from(cfg.title.max_chars)))],
    }
}

pub fn sort(column: &str, descending: bool) -> SortKey {
    SortKey {
        column: column.to_string(),
        descending,
    }
}

pub fn addresses(page: &os_analyze::views::ViewPage) -> Vec<String> {
    page.rows
        .iter()
        .map(|r| match &r[0] {
            Value::Text(s) => s.clone(),
            other => panic!("Address ist kein Text: {other:?}"),
        })
        .collect()
}
