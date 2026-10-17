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

use crate::defs::{ColumnDef, ColumnKind, ColumnSql, FilterDef, FilterKind, TabDef};

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
        ],
        default_sort: Vec::new(),
        params: |cfg| vec![("max", os_model::Value::Int(i64::from(cfg.url.max_chars)))],
        hidden_population: None,
    }
}
