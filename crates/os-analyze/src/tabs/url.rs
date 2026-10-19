//! Tab „URL“ (ONPAGE-070 … 081): URL-Hygiene-Filter über der Anzeigeform der Adresse.
//!
//! Grundmenge: alle internen URLs der Tabellen mit Antwort- oder Blockierungsstatus. Die Prüfungen laufen
//! auf `urls.address` (Anzeigeform: Host als Unicode, kodierte Nicht-ASCII-Zeichen dekodiert, kodierte
//! ASCII-Zeichen wie `%20` bleiben kodiert; Fragmente stehen nicht in der Adresse, werden aber vorsorglich
//! ausgeblendet). Aus der Adresse werden Pfad und Query per SQL herausgelöst:
//!
//! * `pq` = alles ab dem ersten `/` nach dem `://` (Pfad, Query, Fragment),
//! * `pqnf` = `pq` ohne Fragment,
//! * `path` = `pqnf` ohne Query.
//!
//! Festlegungen (Annahmen A-22 … A-27):
//! * **Non ASCII Characters** (A-22): ein Zeichen > U+007F irgendwo in der Adresse (auch im IDN-Host) oder
//!   ein kodiertes Byte ≥ 0x80, das sich nicht dekodieren ließ (`%FF`, nicht darstellbare Zeichen).
//! * **Underscores** (ONPAGE-072, K-58 ersetzt A-23): `_` in Pfad oder Query, nicht im Host und nicht im Fragment.
//! * **Uppercase**: Unicode-Kategorie Lu in Pfad, Query oder Fragment – nicht in Schema, Host und nicht in
//!   den Hex-Ziffern von `%XX`.
//! * **Multiple Slashes**: `//` im Pfad (nicht das `//` nach dem Schema, nicht in Query/Fragment).
//! * **Contains A Space** (A-25): Leerzeichen oder `%20`; ein `+` zählt nicht.
//! * **Parameters** (A-27): `?` oder `&` außerhalb des Fragments (auch die leere Query `?`).
//! * **Over {url.max_chars} Characters**: `Length` (Codepoints der Adresse) größer als die Schwelle.

use std::sync::OnceLock;

use crate::defs::{ColumnDef, ColumnKind, ColumnSql, FilterDef, FilterKind, Requirement, TabDef};

/// Grundmenge: interne URLs mit Antwort- oder Blockierungsstatus.
const POPULATION: &str = "u.is_internal = 1 AND u.in_tables = 1 AND u.status_code IS NOT NULL";

/// Skalare Teilausdrücke über der Adresse von `u` (korreliert über `u.address`). Die Adresse enthält nach dem
/// `://` zuerst die Autorität (Host, ggf. Userinfo und Port, nie ein `/`), danach Pfad, Query und Fragment.
///
/// * `pq` = alles ab dem ersten `/` nach der Autorität (Pfad, Query, Fragment),
/// * `pqnf` = `pq` ohne Fragment,
/// * `path` = `pqnf` ohne Query.
macro_rules! pq_of_address {
    () => {
        "(SELECT substr(r, instr(r || '/', '/')) FROM (SELECT substr(u.address, instr(u.address, '://') + 3) AS r))"
    };
}

macro_rules! path_of_address {
    () => {
        "(SELECT substr(pqnf, 1, instr(pqnf || '?', '?') - 1) \
          FROM (SELECT substr(pq, 1, instr(pq || '#', '#') - 1) AS pqnf \
                FROM (SELECT substr(r, instr(r || '/', '/')) AS pq \
                      FROM (SELECT substr(u.address, instr(u.address, '://') + 3) AS r))))"
    };
}

macro_rules! pqnf_of_address {
    () => {
        "(SELECT substr(pq, 1, instr(pq || '#', '#') - 1) \
          FROM (SELECT substr(r, instr(r || '/', '/')) AS pq \
                FROM (SELECT substr(u.address, instr(u.address, '://') + 3) AS r)))"
    };
}

/// Zeichen > U+007F: mehr Bytes als Zeichen.
macro_rules! has_non_ascii_char {
    ($text:literal) => {
        concat!("length(CAST(", $text, " AS BLOB)) <> length(", $text, ")")
    };
}

const NON_ASCII_SQL: &str = concat!(
    "(",
    has_non_ascii_char!("u.address"),
    " OR u.address GLOB '*%[89A-F][0-9A-F]*')"
);

/// Unterstrich in Pfad oder Query, nicht im Host und nicht im Fragment (ONPAGE-072, K-58 ersetzt A-23). Der
/// Vorab-Test (`instr`) spart die Zerlegung für die meisten Zeilen.
const UNDERSCORES_SQL: &str = concat!(
    "(instr(u.address, '_') > 0 AND instr(",
    pqnf_of_address!(),
    ", '_') > 0)"
);

const MULTIPLE_SLASHES_SQL: &str = concat!(
    "(instr(u.address, '//') > 0 AND instr(",
    path_of_address!(),
    ", '//') > 0)"
);

const SPACE_SQL: &str = "(instr(u.address, ' ') > 0 OR instr(u.address, '%20') > 0)";

/// Eine Adresse ohne `#` (die Anzeigeform enthält keine Fragmente) hat Parameter, sobald sie ein `?` enthält;
/// sonst entscheidet die Zerlegung (`&` im Pfad, `?`/`&` vor einem Fragment).
const PARAMETERS_SQL: &str = concat!(
    "((instr(u.address, '#') = 0 AND instr(u.address, '?') > 0) \
      OR ((instr(u.address, '?') > 0 OR instr(u.address, '&') > 0) AND (instr(",
    pqnf_of_address!(),
    ", '?') > 0 OR instr(",
    pqnf_of_address!(),
    ", '&') > 0)))"
);

const OVER_SQL: &str = "length(u.address) > :max";

/// Zeichenklasse aller Großbuchstaben (Lu) oberhalb von U+007F als Inhalt einer GLOB-Klasse `[…]`.
/// Std liefert die Eigenschaft Uppercase (Lu + Other_Uppercase); Other_Uppercase wird ausgenommen.
fn non_ascii_uppercase_class() -> String {
    fn other_uppercase(c: u32) -> bool {
        matches!(c, 0x2160..=0x216F | 0x24B6..=0x24CF | 0x1F130..=0x1F149 | 0x1F150..=0x1F169 | 0x1F170..=0x1F189)
    }
    let mut ranges: Vec<(u32, u32)> = Vec::new();
    for code in 0x80u32..=0x10_FFFF {
        let Some(c) = char::from_u32(code) else {
            continue;
        };
        if !c.is_uppercase() || other_uppercase(code) {
            continue;
        }
        match ranges.last_mut() {
            Some((_, end)) if *end + 1 == code => *end = code,
            _ => ranges.push((code, code)),
        }
    }
    let mut class = String::new();
    for (start, end) in ranges {
        let (Some(a), Some(b)) = (char::from_u32(start), char::from_u32(end)) else {
            continue;
        };
        class.push(a);
        match end - start {
            0 => {}
            1 => class.push(b),
            _ => {
                class.push('-');
                class.push(b);
            }
        }
    }
    class
}

/// SQL-Ausdruck, der gültige `%XX`-Escapes mit Hex-Buchstaben A–F in `expr` unschädlich macht (sie werden zu
/// `%xx`), damit die Hex-Ziffern nicht als Großbuchstaben zählen. Die Adressen sind normalisiert, ihre
/// Hex-Ziffern also immer groß geschrieben.
fn hex_neutralized(expr: &str) -> String {
    const HEX: &str = "0123456789ABCDEF";
    let mut sql = expr.to_string();
    for a in HEX.chars() {
        for b in HEX.chars() {
            if a.is_ascii_digit() && b.is_ascii_digit() {
                continue;
            }
            sql = format!("replace({sql}, '%{a}{b}', '%xx')");
        }
    }
    sql
}

/// Bedingung „`x` enthält einen Großbuchstaben (Lu)“. ASCII per Klasse; oberhalb von U+007F zuerst ein
/// billiger Test auf Zeichen außerhalb von ASCII, Latin-1-Zeichensetzung und den Kleinbuchstaben ß–ÿ (dort
/// gibt es keine Lu), erst dann die große Klasse aller Lu-Zeichen.
fn has_uppercase(x: &str, class: &str) -> String {
    /// Zeichen außerhalb von ASCII (Leerzeichen bis `~`), Latin-1-Zeichensetzung (U+00A0–U+00BF) und ß–ÿ.
    const OUTSIDE_LOWER_LATIN1: &str = "*[^ -~\u{A0}-\u{BF}\u{DF}-\u{FF}]*";
    format!(
        "({x} GLOB '*[A-Z]*' OR ({x} GLOB '{OUTSIDE_LOWER_LATIN1}' AND {x} GLOB '*[{class}]*'))"
    )
}

/// Prädikat „Uppercase“: Großbuchstabe (Lu) in Pfad, Query oder Fragment, ohne Hex-Ziffern von `%XX`.
/// Adressen ohne `%` und ohne Userinfo (`@`) brauchen keine Zerlegung: Schema und Host sind kleingeschrieben,
/// jeder Großbuchstabe liegt also in Pfad oder Query. Die übrigen Zeilen werden zerlegt und von den
/// Hex-Ziffern bereinigt.
fn uppercase_sql() -> &'static str {
    static SQL: OnceLock<String> = OnceLock::new();
    SQL.get_or_init(|| {
        let class = non_ascii_uppercase_class();
        let simple = has_uppercase("u.address", &class);
        let cleaned = has_uppercase("h", &class);
        let h = hex_neutralized("pq");
        // Bewusst als OR/AND statt als CASE: SQLite wertet die Variante mit CASE um ein Vielfaches langsamer aus.
        format!(
            "((instr(u.address, '%') = 0 AND instr(u.address, '@') = 0 AND {simple}) \
              OR ((instr(u.address, '%') > 0 OR instr(u.address, '@') > 0) \
                  AND EXISTS (SELECT 1 FROM (SELECT {h} AS h FROM (SELECT {pq} AS pq)) WHERE {cleaned})))",
            pq = pq_of_address!()
        )
    })
}

/// Zerlegt einen URL-Bestandteil, ohne leere Segmente als Treffer zu zählen.
fn split_sql(expr: &str, separator: char) -> String {
    format!(
        "(WITH RECURSIVE parts(rest, part) AS (VALUES (({expr}) || '{separator}', '') \
        UNION ALL SELECT substr(rest, instr(rest, '{separator}') + 1), \
        substr(rest, 1, instr(rest, '{separator}') - 1) FROM parts WHERE rest <> '') \
        SELECT part FROM parts WHERE part <> '')"
    )
}

/// Dekodiert Prozent-Bytes genau einmal. Hex wird als UTF-8-Blob zusammengesetzt,
/// damit auch mehrbyteige Fragment-IDs korrekt verglichen werden.
fn decoded_sql(expr: &str) -> String {
    format!("(CASE WHEN instr(({expr}), '%') = 0 THEN ({expr}) ELSE \
        (WITH RECURSIVE percent_bytes(rest, bytes) AS (VALUES (({expr}), '') UNION ALL \
         SELECT CASE WHEN substr(rest,1,1) = '%' AND substr(rest,2,2) GLOB '[0-9A-Fa-f][0-9A-Fa-f]' \
                     THEN substr(rest,4) ELSE substr(rest,2) END, \
                bytes || CASE WHEN substr(rest,1,1) = '%' AND substr(rest,2,2) GLOB '[0-9A-Fa-f][0-9A-Fa-f]' \
                     THEN substr(rest,2,2) ELSE hex(substr(rest,1,1)) END \
         FROM percent_bytes WHERE rest <> '') SELECT CAST(unhex(bytes) AS TEXT) FROM percent_bytes WHERE rest = '') END)")
}

fn repetitive_sql() -> &'static str {
    static SQL: OnceLock<String> = OnceLock::new();
    SQL.get_or_init(|| {
        format!(
            "EXISTS (SELECT 1 FROM {} GROUP BY part COLLATE BINARY HAVING count(*) > 1)",
            split_sql(path_of_address!(), '/')
        )
    })
}

fn query_sql() -> String {
    format!(
        "(SELECT CASE WHEN instr(pq, '?') > 0 THEN substr(pq, instr(pq, '?') + 1) ELSE '' END \
        FROM (SELECT {} AS pq))",
        pqnf_of_address!()
    )
}

fn search_sql() -> &'static str {
    static SQL: OnceLock<String> = OnceLock::new();
    SQL.get_or_init(|| {
        let segments = split_sql(path_of_address!(), '/');
        let query = split_sql(&query_sql(), '&');
        let segment = decoded_sql("part");
        let name = decoded_sql("replace(substr(part, 1, instr(part, '=') - 1), '+', ' ')");
        // SearchAction is read from persisted SD items, including nested potentialAction objects.
        // Glob metacharacters in literals are escaped; only {...} placeholders become wildcards.
        format!("(EXISTS (SELECT 1 FROM {segments}, json_each(:search_segments) config \
                    WHERE lower({segment}) = lower(config.value)) \
             OR EXISTS (SELECT 1 FROM {query}, json_each(:search_params) config \
                    WHERE instr(part, '=') > 0 AND length(substr(part, instr(part, '=') + 1)) > 0 \
                    AND lower({name}) = lower(config.value)) \
             OR EXISTS (SELECT 1 FROM sd_items sd, json_tree(CASE WHEN json_valid(sd.json) THEN sd.json ELSE '{{}}' END) action \
                  WHERE sd.url_id = u.id AND action.type = 'object' \
                  AND EXISTS (SELECT 1 FROM json_each(action.value) typ \
                       WHERE typ.key = '@type' AND (lower(typ.value) IN ('searchaction', 'https://schema.org/searchaction', 'http://schema.org/searchaction') \
                           OR (typ.type = 'array' AND EXISTS (SELECT 1 FROM json_each(typ.value) t WHERE lower(t.value) = 'searchaction')))) \
                  AND EXISTS (SELECT 1 FROM json_tree(action.value) template \
                       WHERE template.type = 'text' AND template.key IN ('urlTemplate', 'target') \
                       AND lower(u.url) GLOB (WITH RECURSIVE pattern(rest, glob) AS (VALUES (lower(template.value), '') \
                           UNION ALL SELECT \
                               CASE WHEN substr(rest,1,1) = '{{' AND instr(rest,'}}') > 1 THEN substr(rest,instr(rest,'}}')+1) ELSE substr(rest,2) END, \
                               glob || CASE WHEN substr(rest,1,1) = '{{' AND instr(rest,'}}') > 1 THEN '*' \
                                   WHEN substr(rest,1,1) = '[' THEN '[[]' WHEN substr(rest,1,1) = '*' THEN '[*]' \
                                   WHEN substr(rest,1,1) = '?' THEN '[?]' ELSE substr(rest,1,1) END \
                           FROM pattern WHERE rest <> '') SELECT glob FROM pattern WHERE rest = ''))))")
    })
}

fn ga_sql() -> &'static str {
    static SQL: OnceLock<String> = OnceLock::new();
    SQL.get_or_init(|| {
        let name = decoded_sql("replace(substr(part, 1, instr(part || '=', '=') - 1), '+', ' ')");
        format!("EXISTS (SELECT 1 FROM {} WHERE lower({name}) GLOB 'utm_*' OR lower({name}) IN ('_ga','_gl'))",
            split_sql(&query_sql(), '&'))
    })
}

fn broken_bookmark_sql() -> &'static str {
    static SQL: OnceLock<String> = OnceLock::new();
    SQL.get_or_init(|| {
        let fragment = decoded_sql("substr(u.url, instr(u.url, '#') + 1)");
        format!("(:crawl_fragments = 1 AND instr(u.url, '#') > 0 AND \
            EXISTS (SELECT 1 FROM urls base JOIN url_ext targets ON targets.url_id = base.id \
                AND targets.key = 'urlfilter.bookmark_targets' \
                WHERE base.url = substr(u.url, 1, instr(u.url, '#') - 1) \
                AND base.status_code BETWEEN 200 AND 299 AND base.content_class = 'HTML' \
                AND json_valid(targets.v_text) \
                AND {fragment} <> '' AND {fragment} <> 'top' \
                AND substr({fragment},1,1) <> '!' AND substr({fragment},1,8) <> ':~:text=' \
                AND NOT EXISTS (SELECT 1 FROM json_each(targets.v_text) target WHERE target.value = {fragment} COLLATE BINARY)))")
    })
}

fn url_params(cfg: &os_model::CrawlConfig) -> Vec<(&'static str, os_model::Value)> {
    vec![
        ("max", os_model::Value::Int(i64::from(cfg.url.max_chars))),
        (
            "search_segments",
            os_model::Value::Text(
                serde_json::to_string(&cfg.url.internal_search_segments).expect("string list"),
            ),
        ),
        (
            "search_params",
            os_model::Value::Text(
                serde_json::to_string(&cfg.url.internal_search_params).expect("string list"),
            ),
        ),
        (
            "crawl_fragments",
            os_model::Value::Bool(cfg.advanced.crawl_fragments),
        ),
    ]
}

fn column(key: &'static str, sql: &'static str, kind: ColumnKind, visible: bool) -> ColumnDef {
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
        default_columns: &[],
    }
}

/// Definition des Tabs „URL“.
pub fn tab() -> TabDef {
    TabDef {
        id: "URL",
        population: POPULATION,
        columns: vec![
            column("Address", "u.address", ColumnKind::Url, true),
            column("Content", "u.content_type", ColumnKind::Text, true),
            column("Status Code", "u.status_code", ColumnKind::Int, true),
            column("Status", "u.status", ColumnKind::Text, true),
            column("Indexability", "u.indexability", ColumnKind::Text, true),
            column(
                "Indexability Status",
                "u.indexability_status",
                ColumnKind::Text,
                true,
            ),
            column("Hash", "u.hash_md5", ColumnKind::Hash, true),
            column("Length", "length(u.address)", ColumnKind::Int, true),
            column(
                "Canonical Link Element 1",
                "(SELECT COALESCE(c.resolved, c.raw) FROM canonicals c \
                  WHERE c.url_id = u.id AND c.source = 'html' ORDER BY c.position LIMIT 1)",
                ColumnKind::Url,
                true,
            ),
            column("URL Encoded Address", "u.url", ColumnKind::Url, true),
        ],
        filters: vec![
            filter("All", "1"),
            filter("Non ASCII Characters", NON_ASCII_SQL),
            filter("Underscores", UNDERSCORES_SQL),
            filter("Uppercase", uppercase_sql()),
            filter("Multiple Slashes", MULTIPLE_SLASHES_SQL),
            filter("Contains A Space", SPACE_SQL),
            filter("Parameters", PARAMETERS_SQL),
            filter("Over {url.max_chars} Characters", OVER_SQL),
            filter("Repetitive Path", repetitive_sql()),
            filter("Internal Search", search_sql()),
            FilterDef {
                requires: vec![Requirement::Config("advanced.crawl_fragments")],
                ..filter("Broken Bookmark", broken_bookmark_sql())
            },
            filter("GA Tracking Parameters", ga_sql()),
        ],
        default_sort: Vec::new(),
        params: url_params,
        hidden_population: None,
    }
}
