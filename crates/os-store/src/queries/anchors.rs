//! Shared anchor normalization and aggregation (TECH-136/146).

use os_model::{UrlId, Value};
use rusqlite::params;

/// TECH-131/146: preserve case, collapse Unicode whitespace.
pub fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// TECH-136: exact phrase comparison, ignoring trailing punctuation.
pub fn phrase(text: &str) -> String {
    normalize(text)
        .to_lowercase()
        .trim_end_matches(['.', ',', ':', ';', '!', '?', '…', '»', '›', '→'])
        .trim_end()
        .to_owned()
}

fn authority(url: &str) -> String {
    let (scheme, rest) = url.split_once("://").unwrap_or(("", url));
    let auth = rest.split(['/', '?', '#']).next().unwrap_or("");
    let auth = auth.rsplit('@').next().unwrap_or(auth).to_ascii_lowercase();
    let has_port = if auth.starts_with('[') {
        auth.split_once(']')
            .is_some_and(|(_, rest)| rest.starts_with(':'))
    } else {
        auth.contains(':')
    };
    if has_port {
        auth
    } else {
        let port = if scheme.eq_ignore_ascii_case("https") {
            443
        } else {
            80
        };
        format!("{auth}:{port}")
    }
}

/// Loopback hosts only; private networks are deliberately excluded (TECH-141).
pub fn is_loopback(url: &str) -> bool {
    let auth = authority(url);
    let host = if auth.starts_with('[') {
        auth.split(']').next().unwrap_or("").trim_start_matches('[')
    } else {
        auth.split(':').next().unwrap_or("")
    };
    host == "localhost"
        || host.ends_with(".localhost")
        || host == "0.0.0.0"
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

/// Kollationen und skalare Funktionen auf jeder Store-Verbindung, auch nur lesend.
pub fn register_collations(conn: &rusqlite::Connection) -> Result<(), crate::StoreError> {
    conn.create_collation("OS_ANCHOR", |a, b| normalize(a).cmp(&normalize(b)))?;
    conn.create_collation("OS_ANCHOR_SORT", |a, b| {
        let a = normalize(a);
        let b = normalize(b);
        crate::collation::compare(&a, &b).then_with(|| a.cmp(&b))
    })?;
    conn.create_collation("OS_ANCHOR_PHRASE", |a, b| phrase(a).cmp(&phrase(b)))?;
    use rusqlite::functions::FunctionFlags;
    let flags = FunctionFlags::SQLITE_UTF8
        | FunctionFlags::SQLITE_DETERMINISTIC
        | FunctionFlags::SQLITE_INNOCUOUS;
    conn.create_scalar_function("os_authority", 1, flags, |ctx| {
        Ok(ctx.get::<Option<String>>(0)?.map(|url| authority(&url)))
    })?;
    conn.create_scalar_function("os_is_loopback", 1, flags, |ctx| {
        Ok(ctx
            .get::<Option<String>>(0)?
            .is_some_and(|url| is_loopback(&url)))
    })?;
    Ok(())
}

/// Options shared by aggregation and its drill-down. Defaults follow TECH-146.
#[derive(Debug, Clone, Copy)]
pub struct AnchorOptions {
    pub outlinks: bool,
    pub exclude_self: bool,
    pub exclude_nofollow: bool,
}
impl Default for AnchorOptions {
    fn default() -> Self {
        Self {
            outlinks: false,
            exclude_self: true,
            exclude_nofollow: false,
        }
    }
}

/// Effective text: anchor, otherwise the image/area alt text. Empty text is a separate group.
pub const TEXT_SQL: &str = "CASE WHEN COALESCE(l.anchor, '') COLLATE OS_ANCHOR = '' THEN COALESCE(l.alt_text, '') ELSE l.anchor END";

fn condition(ids: &[UrlId], options: AnchorOptions) -> String {
    let ids = ids
        .iter()
        .map(|id| id.0.to_string())
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "l.{} IN ({}) AND l.link_type = 'Hyperlink' AND l.crawlable = 1 {} {}",
        if options.outlinks {
            "source_id"
        } else {
            "target_id"
        },
        ids,
        if options.exclude_self {
            "AND l.source_id <> l.target_id"
        } else {
            ""
        },
        if options.exclude_nofollow {
            "AND l.follow = 1"
        } else {
            ""
        }
    )
}

pub fn detail_rows(
    file: &crate::CrawlFile,
    ids: &[UrlId],
    offset: u64,
    limit: u64,
) -> Result<super::DetailData, crate::StoreError> {
    detail_rows_with(file, ids, offset, limit, AnchorOptions::default())
}

/// Aggregate in SQLite, page groups only, and retain the denominator across pages.
pub fn detail_rows_with(
    file: &crate::CrawlFile,
    ids: &[UrlId],
    offset: u64,
    limit: u64,
    options: AnchorOptions,
) -> Result<super::DetailData, crate::StoreError> {
    aggregate(file, ids, offset, limit, options, &Extras::default(), &[])
}

/// Additional restrictions of the parameter-aware aggregation (position, one anchor group, free text).
#[derive(Default)]
struct Extras {
    position: Option<String>,
    anchor: Option<String>,
    search: Option<String>,
}

/// Sortable aggregate columns (display name, SQL expression).
const SORTABLE: [(&str, &str); 7] = [
    ("Anchor Text", "({TEXT}) COLLATE OS_ANCHOR_SORT"),
    ("Links", "COUNT(*)"),
    ("Unique Sources", "COUNT(DISTINCT l.source_id)"),
    ("Unique Targets", "COUNT(DISTINCT l.target_id)"),
    ("Follow Links", "SUM(l.follow <> 0)"),
    ("Nofollow Links", "SUM(l.follow = 0)"),
    ("% of Links", "COUNT(*)"),
];

fn aggregate(
    file: &crate::CrawlFile,
    ids: &[UrlId],
    offset: u64,
    limit: u64,
    options: AnchorOptions,
    extras: &Extras,
    sort: &[super::LinkSortKey],
) -> Result<super::DetailData, crate::StoreError> {
    // The free-text clause needs the source/target addresses, hence the joins exist only with a search.
    let (joins, search) = if extras.search.is_some() {
        (
            "JOIN urls s ON s.id = l.source_id LEFT JOIN urls t ON t.id = l.target_id",
            "AND instr(lower(s.address || ' ' || COALESCE(t.address, l.target_raw) || ' ' || \
             COALESCE(l.anchor, '') || ' ' || COALESCE(l.alt_text, '')), lower(?5)) > 0",
        )
    } else {
        ("", "AND ?5 IS NULL")
    };
    let base = format!(
        "FROM links l {joins} WHERE {} AND (?3 IS NULL OR l.link_position = ?3) \
         AND (?4 IS NULL OR ({TEXT_SQL}) COLLATE OS_ANCHOR = ?4) {search} \
         GROUP BY ({TEXT_SQL}) COLLATE OS_ANCHOR",
        condition(ids, options)
    );
    let bind = params![
        limit.min(i64::MAX as u64),
        offset.min(i64::MAX as u64),
        extras.position,
        extras.anchor,
        extras.search
    ];
    let total = file.conn().query_row(
        &format!("SELECT COUNT(*) FROM (SELECT 1 {base})"),
        params![0, 0, extras.position, extras.anchor, extras.search],
        |r| r.get(0),
    )?;
    let mut order = Vec::new();
    for key in sort {
        if let Some((_, expr)) = SORTABLE
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(&key.column))
        {
            order.push(format!(
                "{} {}",
                expr.replace("{TEXT}", TEXT_SQL),
                if key.desc { "DESC" } else { "ASC" }
            ));
        }
    }
    order.push("COUNT(*) DESC".into());
    order.push(format!("({TEXT_SQL}) COLLATE OS_ANCHOR_SORT ASC"));
    let mut stmt = file.conn().prepare(&format!(
        "SELECT {TEXT_SQL}, COUNT(*), COUNT(DISTINCT l.source_id),
        COUNT(DISTINCT l.target_id), SUM(l.follow <> 0), SUM(l.follow = 0),
        100.0 * COUNT(*) / SUM(COUNT(*)) OVER () {base}
        ORDER BY {} LIMIT ?1 OFFSET ?2",
        order.join(", ")
    ))?;
    let rows = stmt
        .query_map(bind, |r| {
            let anchor = normalize(&r.get::<_, String>(0)?);
            let mut values = vec![Value::Text(if anchor.is_empty() {
                "(leer)".into()
            } else {
                anchor
            })];
            for i in 1..6 {
                values.push(Value::Int(r.get(i)?));
            }
            values.push(Value::Float(r.get(6)?));
            Ok(values)
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(super::DetailData {
        columns: [
            "Anchor Text",
            "Links",
            "Unique Sources",
            "Unique Targets",
            "Follow Links",
            "Nofollow Links",
            "% of Links",
        ]
        .map(str::to_owned)
        .to_vec(),
        rows,
        total,
    })
}

/// IDs for opening an aggregate group as individual links, using exactly the same comparison.
/// Pass the raw group key (empty string for the empty group), not its display label.
pub fn matching_link_ids(
    file: &crate::CrawlFile,
    ids: &[UrlId],
    anchor: &str,
    options: AnchorOptions,
) -> Result<Vec<i64>, crate::StoreError> {
    let mut stmt = file.conn().prepare(&format!(
        "SELECT l.id FROM links l WHERE {} AND ({TEXT_SQL}) COLLATE OS_ANCHOR = ?1 ORDER BY l.id",
        condition(ids, options)
    ))?;
    let ids = stmt
        .query_map([anchor], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}

/// Parameter-aware extension missing from the original F05 anchor contract (K2-61).
///
/// Reads `incoming` (default true; alternatively `direction`/`outlinks`), `excludeSelf` (default true, TECH-146),
/// `excludeNofollow`, `position`, `anchor` (restricts to one group) and `search`; sorts by the aggregate columns.
pub fn selected_detail_rows(
    file: &crate::CrawlFile,
    urls: &[os_model::UrlId],
    params: &serde_json::Value,
    sort: &[super::LinkSortKey],
    offset: u64,
    limit: u64,
) -> Result<super::DetailData, crate::StoreError> {
    let flag = |camel: &str, default: bool| {
        params
            .get(camel)
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(default)
    };
    let text = |key: &str| {
        params
            .get(key)
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    let outlinks = match params.get("incoming").and_then(serde_json::Value::as_bool) {
        Some(incoming) => !incoming,
        None => {
            flag("outlinks", false)
                || params.get("direction").and_then(serde_json::Value::as_str) == Some("outlinks")
        }
    };
    let options = AnchorOptions {
        outlinks,
        exclude_self: flag("excludeSelf", true),
        exclude_nofollow: flag("excludeNofollow", false),
    };
    let extras = Extras {
        position: text("position"),
        anchor: text("anchor"),
        search: text("search"),
    };
    aggregate(file, urls, offset, limit, options, &extras, sort)
}
