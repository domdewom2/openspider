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

use crate::defs::{ColumnDef, ColumnKind, ColumnSql, FilterDef, FilterKind, SortKey, TabDef};

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
        ],
        default_sort: vec![SortKey {
            column: "Address".to_string(),
            descending: false,
        }],
        params: |cfg| {
            vec![
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
    }
}
