//! Gemeinsame Lesefunktionen für Export, MCP und Engine.
//!
//! Alle Funktionen lesen nur (`&Connection`), materialisieren nie mehr als eine Seite und verwenden für
//! Sortierungen die Regel aus M1-Plan 3.9: Sortierschlüssel → Gleichstand über die Adresse (Bytevergleich
//! der Anzeigeform) → zuletzt die Link-Kennung; Textspalten mit `COLLATE OS_TEXT`, Adressen bytewise; leere
//! Werte (`NULL` und `''`) stehen bei auf- und absteigender Sortierung am Ende.
//!
//! Sortierschlüssel (`LinkSortKey.column`, ohne Beachtung der Groß-/Kleinschreibung) sind die Spaltennamen
//! des Standard-Linkformats (OUT-031): `Type`, `Source`, `Destination`, `Size (Bytes)`, `Alt Text`,
//! `Anchor`, `Status Code`, `Status`, `Follow`, `Target`, `Rel`, `Path Type`, `Link Path`, `Link Position`,
//! `Link Origin`.

use std::str::FromStr;

use os_model::{
    ContentClass, HeaderRow, Indexability, IndexabilityStatus, LinkType, ModelError,
    NotCrawledReason, RedirectType, UrlId, UrlState,
};
use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::error::StoreError;

// Abfragemodule der M2-Fachpakete (M2-F02: leere Stubs, `// STUB(M2-xx)`); Signaturen legt das jeweilige Paket fest.
pub mod anchors;
pub mod api;
pub mod crawl_path;
pub mod serp;
pub mod structured_data;

/// Sortierschlüssel einer Link-Abfrage (Standard-Linkformat, OUT-031/032).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkSortKey {
    /// Spaltenname des Standard-Linkformats.
    pub column: String,
    /// Absteigend sortieren.
    pub desc: bool,
}

/// Seitenanfrage für paginierte Abfragen.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageReq {
    /// Anzahl zu überspringender Zeilen.
    pub offset: u64,
    /// Höchstzahl Zeilen (1–500).
    pub limit: u32,
    /// Sortierung; leer = Standardreihenfolge (OUT-031/032).
    pub sort: Vec<LinkSortKey>,
}

impl PageReq {
    /// Seitenanfrage mit Standardsortierung.
    pub fn new(offset: u64, limit: u32) -> Self {
        PageReq {
            offset,
            limit,
            sort: Vec::new(),
        }
    }
}

impl Default for PageReq {
    fn default() -> Self {
        PageReq::new(0, 50)
    }
}

/// Eine Seite eines Abfrageergebnisses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T> {
    /// Zeilen dieser Seite.
    pub items: Vec<T>,
    /// Gesamtzahl der Zeilen.
    pub total: u64,
}

/// Eine URL-Zeile (Kerndaten).
#[derive(Debug, Clone, PartialEq)]
pub struct UrlRow {
    /// Kennung.
    pub id: UrlId,
    /// URL Encoded Address.
    pub url: String,
    /// Anzeigeform.
    pub address: String,
    /// Intern oder extern.
    pub is_internal: bool,
    /// Zustand.
    pub state: UrlState,
    /// Grund, falls nicht gecrawlt.
    pub not_crawled_reason: Option<NotCrawledReason>,
    /// Crawl-Tiefe.
    pub crawl_depth: Option<u32>,
    /// Statuscode (0 = keine Antwort).
    pub status_code: Option<u16>,
    /// Statustext.
    pub status: Option<String>,
    /// Inhaltsklasse.
    pub content_class: ContentClass,
    /// Indexierbarkeit.
    pub indexability: Option<Indexability>,
    /// Grund der Nicht-Indexierbarkeit.
    pub indexability_status: Option<IndexabilityStatus>,
}

/// Ein Link im Standard-Linkformat (Inlinks/Outlinks).
#[derive(Debug, Clone, PartialEq)]
pub struct LinkView {
    /// Linktyp.
    pub link_type: LinkType,
    /// Quell-URL.
    pub source_id: UrlId,
    /// Adresse der Quelle.
    pub source_address: String,
    /// Ziel-URL, falls bekannt.
    pub target_id: Option<UrlId>,
    /// Adresse des Ziels.
    pub target_address: String,
    /// Ankertext.
    pub anchor: Option<String>,
    /// Alt-Text.
    pub alt_text: Option<String>,
    /// Link wird verfolgt.
    pub follow: bool,
    /// Statuscode des Ziels.
    pub status_code: Option<u16>,
    /// Statustext des Ziels.
    pub status: Option<String>,
}

/// Ein Glied einer Weiterleitungskette.
#[derive(Debug, Clone, PartialEq)]
pub struct RedirectHop {
    /// URL.
    pub id: UrlId,
    /// Adresse.
    pub address: String,
    /// Statuscode.
    pub status_code: Option<u16>,
    /// Art der Weiterleitung zum nächsten Glied.
    pub redirect_type: Option<RedirectType>,
}

/// Weiterleitungskette ab einer URL.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RedirectChain {
    /// Glieder von der Start-URL bis zum Ende.
    pub hops: Vec<RedirectHop>,
    /// Die Kette läuft im Kreis.
    pub is_loop: bool,
    /// Abbruch durch `max_hops`.
    pub limit_reached: bool,
}

fn parse_enum<T: FromStr<Err = ModelError>>(value: &str) -> Result<T, StoreError> {
    value.parse::<T>().map_err(StoreError::from)
}

fn parse_opt<T: FromStr<Err = ModelError>>(value: Option<String>) -> Result<Option<T>, StoreError> {
    value.as_deref().map(parse_enum).transpose()
}

fn invalid_argument(message: String) -> StoreError {
    StoreError::InvalidInput(message)
}

const URL_ROW_COLUMNS: &str =
    "id, url, address, is_internal, state, not_crawled_reason, crawl_depth, \
     status_code, status, content_class, indexability, indexability_status";

fn url_row(row: &Row<'_>) -> Result<UrlRow, StoreError> {
    Ok(UrlRow {
        id: UrlId(row.get(0)?),
        url: row.get(1)?,
        address: row.get(2)?,
        is_internal: row.get::<_, i64>(3)? != 0,
        state: parse_enum(&row.get::<_, String>(4)?)?,
        not_crawled_reason: parse_opt(row.get(5)?)?,
        crawl_depth: row
            .get::<_, Option<i64>>(6)?
            .and_then(|d| u32::try_from(d).ok()),
        status_code: row
            .get::<_, Option<i64>>(7)?
            .and_then(|c| u16::try_from(c).ok()),
        status: row.get(8)?,
        content_class: parse_enum(&row.get::<_, String>(9)?)?,
        indexability: parse_opt(row.get(10)?)?,
        indexability_status: parse_opt(row.get(11)?)?,
    })
}

/// Sucht eine URL über ihre Adresse (URL Encoded Address, `urls.url`).
pub fn url_by_address(conn: &Connection, url: &str) -> Result<Option<UrlRow>, StoreError> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {URL_ROW_COLUMNS} FROM urls WHERE url = ?1"
    ))?;
    let mut rows = stmt.query([url])?;
    match rows.next()? {
        Some(row) => Ok(Some(url_row(row)?)),
        None => Ok(None),
    }
}

/// Sucht eine URL über ihre Kennung.
pub fn url_by_id(conn: &Connection, id: UrlId) -> Result<Option<UrlRow>, StoreError> {
    let mut stmt =
        conn.prepare_cached(&format!("SELECT {URL_ROW_COLUMNS} FROM urls WHERE id = ?1"))?;
    let mut rows = stmt.query([id.0])?;
    match rows.next()? {
        Some(row) => Ok(Some(url_row(row)?)),
        None => Ok(None),
    }
}

/// Art der Sortierung einer Spalte des Standard-Linkformats.
#[derive(Clone, Copy)]
enum SortKind {
    /// Text mit `COLLATE OS_TEXT`.
    Text,
    /// Adresse oder Pfad, bytewise.
    Bytes,
    /// Zahl oder Wahrheitswert.
    Number,
}

/// SQL-Ausdruck und Art einer Spalte des Standard-Linkformats. Aliase: `l` = links, `s` = Quelle,
/// `t` = Ziel (`LEFT JOIN`).
fn sort_column(name: &str) -> Option<(&'static str, SortKind)> {
    Some(match name.trim().to_ascii_lowercase().as_str() {
        "type" => ("l.link_type", SortKind::Text),
        "source" => ("s.address", SortKind::Bytes),
        "destination" => ("COALESCE(t.address, l.target_raw)", SortKind::Bytes),
        "size (bytes)" => ("t.size_bytes", SortKind::Number),
        "alt text" => ("l.alt_text", SortKind::Text),
        "anchor" => ("l.anchor", SortKind::Text),
        "status code" => ("t.status_code", SortKind::Number),
        "status" => (STATUS_EXPR, SortKind::Text),
        "follow" => ("l.follow", SortKind::Number),
        "target" => ("l.target_attr", SortKind::Text),
        "rel" => ("l.rel", SortKind::Text),
        "path type" => ("l.path_type", SortKind::Text),
        "link path" => ("l.link_path", SortKind::Bytes),
        "link position" => ("l.link_position", SortKind::Text),
        "link origin" => ("l.link_origin", SortKind::Text),
        _ => return None,
    })
}

/// Statustext des Ziels; ein Ziel, das nicht gecrawlt wurde, heißt `Not Crawled` (OUT-031, OS-Festlegung).
const STATUS_EXPR: &str =
    "CASE WHEN t.id IS NOT NULL AND t.state <> 'done' THEN 'Not Crawled' ELSE t.status END";

/// Richtung einer Link-Abfrage.
#[derive(Clone, Copy)]
enum Direction {
    In,
    Out,
}

/// `ORDER BY`-Klausel; das zweite Ergebnis sagt, ob sie Spalten des Ziels (`t`) braucht.
fn order_by(direction: Direction, sort: &[LinkSortKey]) -> Result<(String, bool), StoreError> {
    let mut parts: Vec<String> = Vec::new();
    let mut needs_target = matches!(direction, Direction::Out) && !sort.is_empty();
    for key in sort {
        let (expr, kind) = sort_column(&key.column).ok_or_else(|| {
            invalid_argument(format!("unbekannte Sortierspalte {:?}", key.column))
        })?;
        let dir = if key.desc { "DESC" } else { "ASC" };
        needs_target |= expr.contains("t.");
        // Leere Werte stehen bei jeder Richtung am Ende.
        parts.push(format!("({expr} IS NULL OR {expr} = '') ASC"));
        parts.push(match kind {
            SortKind::Text => format!("{expr} COLLATE OS_TEXT {dir}, {expr} {dir}"),
            SortKind::Bytes | SortKind::Number => format!("{expr} {dir}"),
        });
    }
    // Standardreihenfolge und Gleichstand: Inlinks nach Quelladresse, Outlinks nach Dokumentposition.
    match direction {
        Direction::In => {
            if sort.is_empty() {
                parts.push("s.address ASC, l.position ASC".to_string());
            } else {
                parts.push("s.address ASC".to_string());
            }
        }
        Direction::Out => {
            if sort.is_empty() {
                parts.push("l.position ASC".to_string());
            } else {
                parts.push("COALESCE(t.address, l.target_raw) ASC".to_string());
            }
        }
    }
    parts.push("l.id ASC".to_string());
    Ok((parts.join(", "), needs_target))
}

fn link_view(row: &Row<'_>) -> Result<LinkView, StoreError> {
    Ok(LinkView {
        link_type: parse_enum::<LinkType>(&row.get::<_, String>(0)?)?,
        source_id: UrlId(row.get(1)?),
        source_address: row.get(2)?,
        target_id: row.get::<_, Option<i64>>(3)?.map(UrlId),
        target_address: row.get(4)?,
        anchor: row.get(5)?,
        alt_text: row.get(6)?,
        follow: row.get::<_, i64>(7)? != 0,
        status_code: row
            .get::<_, Option<i64>>(8)?
            .and_then(|c| u16::try_from(c).ok()),
        status: row.get(9)?,
    })
}

/// Schritt 2 einer Link-Abfrage: die Zeilen zu den Kennungen der Seite, in die Reihenfolge von Schritt 1 gebracht.
fn link_views_by_ids(conn: &Connection, ids: &[i64]) -> Result<Vec<LinkView>, StoreError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = vec!["?"; ids.len()].join(",");
    let rows_sql = format!(
        "SELECT l.link_type, l.source_id, s.address, l.target_id, COALESCE(t.address, l.target_raw), \
                l.anchor, l.alt_text, l.follow, t.status_code, {STATUS_EXPR}, l.id \
         FROM links l JOIN urls s ON s.id = l.source_id LEFT JOIN urls t ON t.id = l.target_id \
         WHERE l.id IN ({placeholders})"
    );
    let mut stmt = conn.prepare_cached(&rows_sql)?;
    let mut rows = stmt.query(rusqlite::params_from_iter(ids.iter()))?;
    let mut by_id: std::collections::HashMap<i64, LinkView> =
        std::collections::HashMap::with_capacity(ids.len());
    while let Some(row) = rows.next()? {
        by_id.insert(row.get(10)?, link_view(row)?);
    }
    Ok(ids.iter().filter_map(|i| by_id.remove(i)).collect())
}

/// Eingehende Links ohne den Index `links_target` (K-74): **ein** sequenzieller Lauf über `links` liefert Treffer,
/// Sortierung und Gesamtzahl zugleich; die Seite wird aus dem sortierten Strom geschnitten. Nur die Kennungen
/// (8 Byte je Treffer) laufen durch Rust, nie die breiten Zeilen. Mit Index gilt der Pfad in [`links_page`].
fn inlinks_without_index(
    conn: &Connection,
    id: UrlId,
    page: PageReq,
) -> Result<Page<LinkView>, StoreError> {
    let (order, needs_target) = order_by(Direction::In, &page.sort)?;
    let join_target = if needs_target {
        "LEFT JOIN urls t ON t.id = l.target_id"
    } else {
        ""
    };
    let limit = u64::from(page.limit.clamp(1, MAX_PAGE_LIMIT));
    let sql = format!(
        "SELECT l.id FROM links l JOIN urls s ON s.id = l.source_id {join_target} \
         WHERE l.target_id = ?1 ORDER BY {order}"
    );
    let mut stmt = conn.prepare_cached(&sql)?;
    let mut rows = stmt.query([id.0])?;
    let mut ids: Vec<i64> = Vec::new();
    let mut total = 0u64;
    while let Some(row) = rows.next()? {
        if total >= page.offset && total < page.offset.saturating_add(limit) {
            ids.push(row.get(0)?);
        }
        total += 1;
    }
    drop(rows);
    drop(stmt);
    Ok(Page {
        items: link_views_by_ids(conn, &ids)?,
        total,
    })
}

fn links_page(
    conn: &Connection,
    direction: Direction,
    id: UrlId,
    page: PageReq,
) -> Result<Page<LinkView>, StoreError> {
    if matches!(direction, Direction::In) && !crate::link_index::present(conn)? {
        return inlinks_without_index(conn, id, page);
    }
    let (filter, count_sql) = match direction {
        Direction::In => (
            "l.target_id = ?1",
            "SELECT COUNT(*) FROM links WHERE target_id = ?1",
        ),
        Direction::Out => (
            "l.source_id = ?1",
            "SELECT COUNT(*) FROM links WHERE source_id = ?1",
        ),
    };
    let (order, needs_target) = order_by(direction, &page.sort)?;
    let total: i64 = conn.query_row(count_sql, [id.0], |r| r.get(0))?;
    let limit = page.limit.clamp(1, MAX_PAGE_LIMIT);
    let offset = i64::try_from(page.offset).unwrap_or(i64::MAX);

    // Schritt 1: nur die Kennungen der Seite in Sortierreihenfolge. Der Sortierer hält so nur die
    // Sortierschlüssel und die Link-Kennung, nicht die breite Ergebniszeile (tiefe Offsets bei 1 Mio. Links).
    let join_target = if needs_target {
        "LEFT JOIN urls t ON t.id = l.target_id"
    } else {
        ""
    };
    let ids_sql = format!(
        "SELECT l.id FROM links l JOIN urls s ON s.id = l.source_id {join_target} \
         WHERE {filter} ORDER BY {order} LIMIT ?2 OFFSET ?3"
    );
    let mut stmt = conn.prepare_cached(&ids_sql)?;
    let ids: Vec<i64> = stmt
        .query_map(params![id.0, limit, offset], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    drop(stmt);
    if ids.is_empty() {
        return Ok(Page {
            items: Vec::new(),
            total: u64::try_from(total).unwrap_or(0),
        });
    }

    // Schritt 2: die Zeilen zu diesen Kennungen, danach in die Reihenfolge von Schritt 1 gebracht.
    Ok(Page {
        items: link_views_by_ids(conn, &ids)?,
        total: u64::try_from(total).unwrap_or(0),
    })
}

/// Größte Seitengröße einer Link-Abfrage.
pub const MAX_PAGE_LIMIT: u32 = 500;

/// Eingehende Links einer URL (alle Linktypen, Standard-Linkformat). Standardreihenfolge (OUT-031): nach
/// Quelladresse, dann Dokumentposition. Blättert per Offset, ohne mehr als eine Seite zu materialisieren.
pub fn inlinks(conn: &Connection, id: UrlId, page: PageReq) -> Result<Page<LinkView>, StoreError> {
    links_page(conn, Direction::In, id, page)
}

/// Ausgehende Links einer URL (alle Linktypen, Standard-Linkformat). Standardreihenfolge (OUT-032): nach
/// Dokumentposition. Blättert per Offset, ohne mehr als eine Seite zu materialisieren.
pub fn outlinks(conn: &Connection, id: UrlId, page: PageReq) -> Result<Page<LinkView>, StoreError> {
    links_page(conn, Direction::Out, id, page)
}

/// Gespeicherte HTTP-Header einer URL (Anfrage vor Antwort, jeweils in Reihenfolge).
pub fn headers(conn: &Connection, id: UrlId) -> Result<Vec<HeaderRow>, StoreError> {
    let mut stmt = conn.prepare_cached(
        "SELECT direction, position, name, value FROM http_headers WHERE url_id = ?1 \
         ORDER BY direction, position",
    )?;
    let rows = stmt
        .query_map([id.0], |r| {
            Ok(HeaderRow {
                request: r.get::<_, String>(0)? == "request",
                position: u32::try_from(r.get::<_, i64>(1)?).unwrap_or(0),
                name: r.get(2)?,
                value: r.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Weiterleitungskette ab `start` (höchstens `max_hops` Glieder). Das erste Glied ist `start` selbst; jedes
/// Glied trägt die Art der Weiterleitung zum nächsten. `is_loop`: die Kette kehrt zu einem bekannten Glied
/// zurück (das Glied wird nicht doppelt aufgeführt); `limit_reached`: es gäbe ein weiteres Glied, aber
/// `max_hops` ist erreicht.
pub fn redirect_chain(
    conn: &Connection,
    start: UrlId,
    max_hops: u32,
) -> Result<RedirectChain, StoreError> {
    let mut chain = RedirectChain::default();
    let mut stmt = conn.prepare_cached(
        "SELECT address, status_code, redirect_type, redirect_target_id FROM urls WHERE id = ?1",
    )?;
    let mut seen = std::collections::HashSet::new();
    let mut current = Some(start);
    while let Some(id) = current {
        let mut rows = stmt.query([id.0])?;
        let Some(row) = rows.next()? else {
            break;
        };
        if seen.contains(&id) {
            chain.is_loop = true;
            break;
        }
        if chain.hops.len() >= max_hops as usize {
            chain.limit_reached = true;
            break;
        }
        seen.insert(id);
        chain.hops.push(RedirectHop {
            id,
            address: row.get(0)?,
            status_code: row
                .get::<_, Option<i64>>(1)?
                .and_then(|c| u16::try_from(c).ok()),
            redirect_type: parse_opt(row.get(2)?)?,
        });
        current = row.get::<_, Option<i64>>(3)?.map(UrlId);
    }
    Ok(chain)
}

/// Aktuelle Sequenz des Änderungsfeeds (`crawl_meta.change_seq`); `0`, wenn der Schlüssel fehlt.
/// Der Writer (P07) erhöht sie je `WriteBatch`; Löschungen setzen zusätzlich `reset_seq` (3.9).
pub fn change_seq(conn: &Connection) -> Result<u64, StoreError> {
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM crawl_meta WHERE key = 'change_seq'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    Ok(value.and_then(|v| v.parse().ok()).unwrap_or(0))
}

/// URLs mit `urls.change_seq > seq`, `url_counters.change_seq > seq` oder `url_ext.change_seq > seq` (Schema v2: Zähleränderungen
/// stempeln die Zählertabelle; Schema v3: Zusatzfakten die Tabelle `url_ext`), aufsteigend nach `(change_seq, id)` mit dem
/// jeweils größten der Stempel, ohne Dubletten, höchstens `limit`. Die übrigen M2-Tabellen stempeln `urls.change_seq`.
pub fn changed_since(conn: &Connection, seq: u64, limit: u32) -> Result<Vec<UrlId>, StoreError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id FROM ( \
           SELECT id, MAX(s) AS s FROM ( \
             SELECT id, change_seq AS s FROM urls WHERE change_seq > ?1 \
             UNION ALL \
             SELECT url_id, change_seq FROM url_counters WHERE change_seq > ?1 \
             UNION ALL \
             SELECT url_id, change_seq FROM url_ext WHERE change_seq > ?1) \
           GROUP BY id) \
         ORDER BY s, id LIMIT ?2",
    )?;
    let ids = stmt
        .query_map(
            params![i64::try_from(seq).unwrap_or(i64::MAX), limit],
            |r| Ok(UrlId(r.get(0)?)),
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ids)
}
