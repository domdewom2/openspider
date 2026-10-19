//! Tab „Images“ (ONPAGE-170, ONPAGE-171, ONPAGE-173 … 177; Filter-Kennungen aus `docs/plan/M1-filters.toml`).
//!
//! # Grundmenge (ONPAGE-170)
//!
//! Alle Bild-URLs, intern **und** extern: Zeilen der Inhaltsklasse `Images` oder Zeilen, auf die eine
//! Bild-Referenz zeigt – eine Bildinstanz (`image_instances.image_id`, `<img src>`, `<a href>` auf ein Bild …)
//! oder ein Link vom Typ `Image`. Die Ziel-Adresse kommt immer aus `urls` (über `target_id`/`image_id`),
//! nie aus dem Rohwert des Attributs (K-37). Dadurch erscheint auch ein Bild, dessen Abruf mit 404 endet und
//! dessen Antwort deshalb keine Bild-Inhaltsklasse trägt. Srcset-Kandidaten haben keine URL-Zeile, solange
//! sie niemand sonst verlinkt (die Engine fragt sie in M1 nicht ab); sie stehen nur als Instanzen da. Ohne
//! „Check Images“ (`spider.images.store`) legt die Engine keine Bild-Zeilen an – der Tab ist dann leer.
//!
//! # Spalten (ONPAGE-171)
//!
//! `Address`, `Content` (Content-Type-Header), `Size` (Bytes; die UI zeigt KB), `IMG Inlinks` (verschiedene
//! interne Seiten mit mindestens einer Instanz; vom Writer gepflegter Zähler `url_counters.img_inlinks`),
//! `Indexability`, `Indexability Status`.
//!
//! # Größe (ONPAGE-173, Annahme A-6/A-45)
//!
//! `Size` = Bytes des dekomprimierten Antwortkörpers, wenn das Bild geladen wurde (`urls.size_bytes`), sonst
//! der `Content-Length`-Header, sonst 0. Nur Antworten mit 2xx haben eine Größe: bei 404 (und jeder anderen
//! Nicht-2xx-Antwort, auch Weiterleitungen) ist `Size` 0, egal wie groß die Fehlerseite ist. Dieselbe
//! Größe verwendet der Filter `Over {image.max_kb} KB`; ein 404-Bild erscheint deshalb in keinem
//! Größenfilter.
//!
//! # Filter (ONPAGE-174 … 177)
//!
//! Eine Bild-URL steht in einem Alt-Filter, wenn **mindestens eine** Instanz die Bedingung erfüllt. Gezählt
//! werden nur Instanzen eines `<img>` (Quelle `img`, sowie `srcset`-Kandidaten, die das Alt-Attribut ihres
//! `<img>` tragen); Bildlinks (`a`), `input type=image`, `video poster` und CSS haben keine Alt-Information
//! und zählen nie. `alt=""` und `alt` mit nur Leerraum (nach N(x) leer, `alt_len = 0`) ergeben *Missing Alt
//! Text*, ein fehlendes Attribut *Missing Alt Attribute*; ein Bild kann in beiden stehen (verschiedene
//! Instanzen). `role="presentation"` und `aria-hidden` befreien nicht (A-46). Die Bildfilter tragen kein
//! `ignore_non_indexable`: CRAWL-089 nennt nur die Element-Tabs und Low Content Pages (ONPAGE-009).
//!
//! # Benannte Parameter
//!
//! `:image_max_bytes` (`image.max_kb` × 1024), `:image_alt_max_chars`.

use os_model::Value;

use crate::defs::{
    ColumnDef, ColumnKind, ColumnSql, FilterDef, FilterKind, Requirement, SortKey, TabDef,
};

/// Grundmenge über dem Alias `u` (ONPAGE-170).
const POPULATION: &str = "u.in_tables = 1 AND (u.content_class = 'Images' \
     OR EXISTS (SELECT 1 FROM image_instances i WHERE i.image_id = u.id) \
     OR c.image_link_inlinks > 0)";

/// Größe in Bytes (ONPAGE-173): Körper, sonst `Content-Length`, sonst 0; nur bei 2xx.
macro_rules! size_expr {
    () => {
        concat!(
            "(CASE WHEN u.status_code BETWEEN 200 AND 299 THEN COALESCE(u.size_bytes, ",
            "(SELECT CAST(h.value AS INTEGER) FROM http_headers h WHERE h.url_id = u.id ",
            "AND h.direction = 'response' AND lower(h.name) = 'content-length' LIMIT 1), 0) ELSE 0 END)"
        )
    };
}

/// Spaltenausdruck `Size`.
const SIZE: &str = size_expr!();

/// Prädikat „Over X KB“ (ONPAGE-174): Größe > `image.max_kb` × 1024 Byte.
const OVER_KB: &str = concat!("(", size_expr!(), " > :image_max_bytes)");

/// Instanz eines `<img>` (Quelle `img` oder `srcset`) der Bild-URL `u`, mit zusätzlicher Bedingung `$cond`.
macro_rules! img_instance {
    ($cond:literal) => {
        concat!(
            "EXISTS (SELECT 1 FROM image_instances i WHERE i.image_id = u.id ",
            "AND i.source IN ('img', 'srcset') AND ",
            $cond,
            ")"
        )
    };
}

fn column(key: &'static str, sql: &'static str, kind: ColumnKind) -> ColumnDef {
    ColumnDef {
        key,
        sql: ColumnSql::Expr(sql),
        kind,
        default_visible: true,
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
        default_columns: &[],
    }
}

/// Definition des Tabs „Images“.
pub fn tab() -> TabDef {
    TabDef {
        id: "Images",
        population: POPULATION,
        columns: vec![
            column("Address", "u.address", ColumnKind::Url),
            column("Content", "u.content_type", ColumnKind::Text),
            column("Size", SIZE, ColumnKind::Int),
            column("IMG Inlinks", "c.img_inlinks", ColumnKind::Int),
            column("Indexability", "u.indexability", ColumnKind::Text),
            column(
                "Indexability Status",
                "u.indexability_status",
                ColumnKind::Text,
            ),
        ],
        filters: vec![
            filter("All", "1"),
            // ONPAGE-174: Größe > image.max_kb × 1024, intern und extern.
            filter("Over {image.max_kb} KB", OVER_KB),
            // ONPAGE-175: alt vorhanden, aber nach N(x) leer.
            filter(
                "Missing Alt Text",
                img_instance!("i.alt_present = 1 AND COALESCE(i.alt_len, 0) = 0"),
            ),
            // ONPAGE-176: kein alt-Attribut.
            filter("Missing Alt Attribute", img_instance!("i.alt_present = 0")),
            // ONPAGE-177: Alt-Text-Länge (Codepoints) > image.alt_max_chars.
            filter(
                "Alt Text Over {image.alt_max_chars} Characters",
                img_instance!("COALESCE(i.alt_len, 0) > :image_alt_max_chars"),
            ),
            post_filter("Background Images", "EXISTS (SELECT 1 FROM url_flags f WHERE f.url_id=u.id AND f.flag='image_background')", vec![Requirement::RenderingOrConfig("images.parse_inline_css_backgrounds")]),
            post_filter("Missing Size Attributes", img_instance!("(i.width_attr IS NULL OR trim(i.width_attr) = '' OR i.height_attr IS NULL OR trim(i.height_attr) = '')"), vec![Requirement::RenderingOrConfig("images.parse_inline_css_backgrounds")]),
            post_filter("Incorrectly Sized Images", INCORRECT, vec![Requirement::Rendering]),
        ],
        default_sort: vec![SortKey {
            column: "Address".to_string(),
            descending: false,
        }],
        params: |cfg| {
            vec![
                ("image_min_saving", Value::Int(i64::from(cfg.images.incorrect_size_min_saving_kb) * 1024)),
                ("rendering", Value::Int(i64::from(cfg.rendering.mode == os_model::RenderingMode::JavaScript))),
                ("header_dims", Value::Int(i64::from(cfg.images.real_dimensions_from_header))),
                (
                    "image_max_bytes",
                    Value::Int(i64::from(cfg.image.max_kb) * 1024),
                ),
                (
                    "image_alt_max_chars",
                    Value::Int(i64::from(cfg.image.alt_max_chars)),
                ),
            ]
        },
        hidden_population: None,
    }
}

fn post_filter(id: &'static str, predicate: &'static str, requires: Vec<Requirement>) -> FilterDef {
    FilterDef {
        kind: FilterKind::PostCrawl(os_model::AnalysisStep::Images),
        requires,
        ..filter(id, predicate)
    }
}

// Die Rechnung bleibt auf Instanzebene. Fehlende Messung ist NULL, keine Einsparung ist 0.
macro_rules! savings {
    () => { concat!(
        "CASE WHEN :rendering=1 AND i.in_viewport=1 AND i.real_w>0 AND i.real_h>0 ",
        "AND i.display_w>0 AND i.display_h>0 ",
        "AND lower(COALESCE(u.content_type,'')) NOT LIKE 'image/svg+xml%' ",
        "AND lower(rtrim(substr(COALESCE(u.address,i.resolved,i.src_raw),1,instr(COALESCE(u.address,i.resolved,i.src_raw)||'?', '?')-1),'#')) NOT LIKE '%.svg' ",
        "THEN CASE WHEN i.real_w>i.display_w OR i.real_h>i.display_h ",
        "THEN CAST(COALESCE(i.bytes,", size_expr!(), ") * ",
        "(1.0 - (1.0*i.display_w*i.display_h)/(1.0*i.real_w*i.real_h)) AS INTEGER) ELSE 0 END END"
    ) };
}
const INCORRECT: &str = concat!(
    "EXISTS (SELECT 1 FROM image_instances i WHERE i.image_id=u.id AND (",
    savings!(),
    ") >= :image_min_saving AND (i.real_w>i.display_w OR i.real_h>i.display_h))"
);

/// Auswahl auf Instanzebene für Details und sämtliche Bild-Bulk-Exporte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFilter {
    All,
    MissingAltText,
    MissingAltAttribute,
    LongAlt,
    OverSize,
    MissingSize,
    IncorrectSize,
    Background,
}
impl ImageFilter {
    pub fn tab_filter(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::MissingAltText => "Missing Alt Text",
            Self::MissingAltAttribute => "Missing Alt Attribute",
            Self::LongAlt => "Alt Text Over {image.alt_max_chars} Characters",
            Self::OverSize => "Over {image.max_kb} KB",
            Self::MissingSize => "Missing Size Attributes",
            Self::IncorrectSize => "Incorrectly Sized Images",
            Self::Background => "Background Images",
        }
    }
    fn predicate(self) -> &'static str {
        match self {
            Self::All => "1",
            Self::MissingAltText => "source IN ('img','srcset') AND alt_present=1 AND COALESCE(alt_len,0)=0",
            Self::MissingAltAttribute => "source IN ('img','srcset') AND alt_present=0",
            Self::LongAlt => "source IN ('img','srcset') AND alt_len>:alt_max",
            Self::OverSize => "size>:max_bytes",
            Self::MissingSize => "source IN ('img','srcset') AND (width_attr IS NULL OR trim(width_attr)='' OR height_attr IS NULL OR trim(height_attr)='')",
            Self::IncorrectSize => "savings>=:min_saving AND (real_w>display_w OR real_h>display_h)",
            Self::Background => "EXISTS (SELECT 1 FROM url_flags f WHERE f.url_id=image_id AND f.flag='image_background')",
        }
    }
}

/// Bulk-Spalten; der Detail-Tab blendet Size aus (ONPAGE-172), Erweiterungen auf Wunsch ein.
pub const INSTANCE_COLUMNS: &[(&str, ColumnKind)] = &[
    ("From", ColumnKind::Url),
    ("To", ColumnKind::Url),
    ("Alt Text", ColumnKind::Text),
    ("Size", ColumnKind::Int),
    ("Real Dimensions (WxH)", ColumnKind::Text),
    ("Dimensions in Attributes (WxH)", ColumnKind::Text),
    ("Display Dimensions (WxH)", ColumnKind::Text),
    ("Potential Savings", ColumnKind::Int),
    ("Path Type", ColumnKind::Text),
    ("Link Path", ColumnKind::Text),
    ("Link Position", ColumnKind::Text),
    ("Link Origin", ColumnKind::Text),
    ("Alt Text Length", ColumnKind::Int),
    ("Source", ColumnKind::Text),
];

fn instance_cte() -> String {
    format!(
        r#"WITH facts AS (
      SELECT i.*, p.address AS from_address, COALESCE(u.address,i.resolved,i.src_raw) AS to_address,
        CASE WHEN u.status_code BETWEEN 200 AND 299 THEN COALESCE(i.bytes,{size}) END AS size,
        CASE WHEN lower(COALESCE(u.content_type,'')) LIKE 'image/svg+xml%' THEN NULL WHEN :rendering=1 AND i.in_viewport=1 THEN i.real_w WHEN :header_dims=1 THEN
          (SELECT v_int FROM url_ext WHERE url_id=u.id AND key='image.real_w') END AS rw,
        CASE WHEN lower(COALESCE(u.content_type,'')) LIKE 'image/svg+xml%' THEN NULL WHEN :rendering=1 AND i.in_viewport=1 THEN i.real_h WHEN :header_dims=1 THEN
          (SELECT v_int FROM url_ext WHERE url_id=u.id AND key='image.real_h') END AS rh,
        CASE WHEN :rendering=1 AND i.in_viewport=1 THEN i.display_w END AS dw,
        CASE WHEN :rendering=1 AND i.in_viewport=1 THEN i.display_h END AS dh,
        {savings} AS savings
      FROM image_instances i JOIN urls p ON p.id=i.page_id LEFT JOIN urls u ON u.id=i.image_id
    ), selected AS (SELECT * FROM facts WHERE (page_id IN (SELECT value FROM json_each(:ids))
        OR image_id IN (SELECT value FROM json_each(:ids)) OR :all_rows=1) AND ({{predicate}})),
    cells AS (SELECT id, from_address AS c0, to_address AS c1, alt AS c2, size AS c3,
      CASE WHEN rw>0 AND rh>0 THEN rw||'x'||rh END AS c4,
      CASE WHEN trim(width_attr)<>'' AND trim(height_attr)<>'' THEN width_attr||'x'||height_attr END AS c5,
      CASE WHEN dw IS NOT NULL AND dh IS NOT NULL THEN dw||'x'||dh END AS c6,
      savings AS c7,
      CASE WHEN lower(src_raw) LIKE 'http://%' OR lower(src_raw) LIKE 'https://%' THEN 'Absolute'
        WHEN src_raw LIKE '//%' THEN 'Protocol-Relative' WHEN src_raw LIKE '/%' THEN 'Root-Relative' ELSE 'Path-Relative' END AS c8,
      link_path AS c9, COALESCE(link_position,'Content') AS c10, origin AS c11, alt_len AS c12, source AS c13 FROM selected) "#,
        size = SIZE,
        savings = savings!()
    )
}

/// Gemeinsame paginierte Abfrage; eine leere URL-Auswahl bedeutet hier alle Instanzen (Bulk).
pub fn instance_rows(
    file: &os_store::CrawlFile,
    cfg: &os_model::CrawlConfig,
    req: &crate::details::DetailReq<'_>,
    filter: ImageFilter,
) -> Result<crate::details::DetailRows, crate::ViewError> {
    let cte = instance_cte().replace("{predicate}", filter.predicate());
    let mut order = Vec::new();
    for key in req.sort {
        if let Some((index, (_, kind))) = INSTANCE_COLUMNS
            .iter()
            .enumerate()
            .find(|(_, (name, _))| *name == key.column)
        {
            let direction = if key.descending { "DESC" } else { "ASC" };
            order.push(format!("c{index} IS NULL ASC"));
            order.push(format!(
                "c{index} {} {direction}",
                if matches!(kind, ColumnKind::Text | ColumnKind::Url) {
                    "COLLATE OS_TEXT"
                } else {
                    ""
                }
            ));
        }
    }
    order.extend([
        "c0 COLLATE OS_TEXT".to_string(),
        "c1 COLLATE OS_TEXT".to_string(),
        "id".to_string(),
    ]);
    let sql = format!(
        "{cte} SELECT {}, COUNT(*) OVER () FROM cells ORDER BY {} LIMIT :limit OFFSET :offset",
        (0..INSTANCE_COLUMNS.len())
            .map(|i| format!("c{i}"))
            .collect::<Vec<_>>()
            .join(","),
        order.join(",")
    );
    let mut stmt = file.conn().prepare(&sql)?;
    bind_instance_params(&mut stmt, cfg, req)?;
    let mut cursor = stmt.raw_query();
    let mut rows = Vec::new();
    let mut total = None;
    while let Some(row) = cursor.next()? {
        total = Some(row.get::<_, u64>(INSTANCE_COLUMNS.len())?);
        rows.push(instance_values(row)?);
    }
    let total = match total {
        Some(n) => n,
        None => {
            let mut count = file
                .conn()
                .prepare(&format!("{cte} SELECT COUNT(*) FROM cells"))?;
            bind_instance_params(&mut count, cfg, req)?;
            let n = count.raw_query().next()?.expect("count").get(0)?;
            n
        }
    };
    Ok(crate::details::DetailRows {
        columns: INSTANCE_COLUMNS
            .iter()
            .map(|(n, k)| ((*n).into(), *k))
            .collect(),
        rows,
        total,
        notes: Vec::new(),
    })
}

fn bind_instance_params(
    stmt: &mut rusqlite::Statement<'_>,
    cfg: &os_model::CrawlConfig,
    req: &crate::details::DetailReq<'_>,
) -> rusqlite::Result<()> {
    let ids = serde_json::to_string(&req.url_ids.iter().map(|id| id.0).collect::<Vec<_>>())
        .expect("integer IDs");
    let rendering = cfg.rendering.mode == os_model::RenderingMode::JavaScript;
    let params = rusqlite::named_params! { ":ids":ids, ":all_rows":req.url_ids.is_empty(), ":rendering":rendering,
    ":header_dims":cfg.images.real_dimensions_from_header,
    ":limit":req.limit.min(500), ":offset":req.offset.min(i64::MAX as u64),
    ":max_bytes":i64::from(cfg.image.max_kb)*1024, ":alt_max":cfg.image.alt_max_chars,
    ":min_saving":i64::from(cfg.images.incorrect_size_min_saving_kb)*1024 };
    for (name, value) in params {
        if let Some(index) = stmt.parameter_index(name)? {
            stmt.raw_bind_parameter(index, *value)?;
        }
    }
    Ok(())
}
fn instance_values(row: &rusqlite::Row<'_>) -> rusqlite::Result<Vec<Value>> {
    use rusqlite::types::ValueRef;
    (0..INSTANCE_COLUMNS.len())
        .map(|i| {
            Ok(match row.get_ref(i)? {
                ValueRef::Null | ValueRef::Blob(_) => Value::Null,
                ValueRef::Integer(v) => Value::Int(v),
                ValueRef::Real(v) => Value::Float(v),
                ValueRef::Text(v) => Value::Text(String::from_utf8_lossy(v).into_owned()),
            })
        })
        .collect()
}

/// Ein SQL-Lauf für einen vollständigen Export; im Rust-Speicher lebt nur eine Instanzzeile.
pub fn stream_instances<E: From<crate::ViewError>>(
    file: &os_store::CrawlFile,
    cfg: &os_model::CrawlConfig,
    filter: ImageFilter,
    mut emit: impl FnMut(&[Value]) -> Result<(), E>,
) -> Result<u64, E> {
    let run = || -> Result<rusqlite::Statement<'_>, crate::ViewError> {
        let cte = instance_cte().replace("{predicate}", filter.predicate());
        let sql = format!(
            "{cte} SELECT {} FROM cells ORDER BY c0 COLLATE OS_TEXT,c1 COLLATE OS_TEXT,id",
            (0..INSTANCE_COLUMNS.len())
                .map(|i| format!("c{i}"))
                .collect::<Vec<_>>()
                .join(",")
        );
        let mut stmt = file.conn().prepare(&sql)?;
        bind_instance_params(
            &mut stmt,
            cfg,
            &crate::details::DetailReq {
                url_ids: &[],
                params: &serde_json::json!({}),
                sort: &[],
                offset: 0,
                limit: 500,
            },
        )?;
        Ok(stmt)
    };
    let mut stmt = run()?;
    let mut cursor = stmt.raw_query();
    let mut count = 0;
    while let Some(row) = cursor.next().map_err(crate::ViewError::from)? {
        let values = instance_values(row).map_err(crate::ViewError::from)?;
        emit(&values)?;
        count += 1;
    }
    Ok(count)
}

/// Paritätsspalten ohne Size; Source und Alt Text Length sind nur mit extensions=true sichtbar.
pub fn detail_rows(
    file: &os_store::CrawlFile,
    cfg: &os_model::CrawlConfig,
    req: &crate::details::DetailReq<'_>,
) -> Result<crate::details::DetailRows, crate::ViewError> {
    let mut data = if req.url_ids.is_empty() {
        crate::details::DetailRows {
            columns: INSTANCE_COLUMNS
                .iter()
                .map(|(n, k)| ((*n).into(), *k))
                .collect(),
            rows: Vec::new(),
            total: 0,
            notes: Vec::new(),
        }
    } else {
        instance_rows(file, cfg, req, ImageFilter::All)?
    };
    if req.params.get("extensions").and_then(|v| v.as_bool()) != Some(true) {
        data.columns.truncate(12);
        for row in &mut data.rows {
            row.truncate(12);
        }
    }
    data.columns.remove(3);
    for row in &mut data.rows {
        row.remove(3);
    }
    Ok(data)
}
