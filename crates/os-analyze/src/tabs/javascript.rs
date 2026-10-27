//! Original and rendered page comparisons (TECH-101 … TECH-114).
use crate::defs::{intern, ColumnDef, ColumnKind, FilterDef, FilterKind, Requirement, TabDef};

pub const POPULATION: &str = "u.in_tables=1 AND u.is_internal=1 AND u.content_class='HTML' AND u.status_code=200 AND EXISTS (SELECT 1 FROM render_pages r WHERE r.url_id=u.id AND r.status='ok')";

fn fact(name: &str, numeric: bool) -> String {
    format!(
        "(SELECT {} FROM url_ext WHERE url_id=u.id AND key='render.{name}')",
        if numeric { "v_int" } else { "v_text" }
    )
}
fn text(name: &str) -> String {
    format!("COALESCE({}, '')", fact(name, false))
}
fn words(prefix: &str) -> String {
    format!(
        "COALESCE({}, 0)",
        fact(&format!("{prefix}_word_count"), true)
    )
}
fn token(prefix: &str, token: &str) -> String {
    let robots = text(&format!("{prefix}_robots"));
    format!("(instr(',' || lower({robots}) || ',', ',{token},')>0 OR instr(',' || lower({robots}) || ',', ',none,')>0)")
}
fn js_links(inlinks: bool, external: bool) -> String {
    let direction = if inlinks {
        "l.target_id=u.id AND s.is_internal=1"
    } else if external {
        "l.source_id=u.id AND t.is_internal=0"
    } else {
        "l.source_id=u.id AND t.is_internal=1"
    };
    let unique = if inlinks {
        "l.source_id"
    } else {
        "l.target_id"
    };
    format!("(SELECT COUNT(DISTINCT {unique}) FROM links l JOIN urls s ON s.id=l.source_id JOIN urls t ON t.id=l.target_id WHERE {direction} AND l.source_id<>l.target_id AND l.link_type='Hyperlink' AND l.link_origin='Rendered HTML')")
}
fn filter(id: &'static str, predicate: &str) -> FilterDef {
    FilterDef {
        id,
        predicate: intern(predicate),
        kind: FilterKind::Live,
        requires: vec![Requirement::Rendering],
        ignore_non_indexable: false,
        default_columns: &[],
    }
}
/// The comparisons remain available even if retaining page HTML is disabled.
pub fn tab() -> TabDef {
    use ColumnKind::{Int, Text, Url};
    let mut columns = vec![];
    for (key, sql, kind) in [
        ("Address", "u.address", Url),
        ("Status Code", "u.status_code", Int),
        ("Status", "u.status", Text),
        ("Indexability", "u.indexability", Text),
        ("Indexability Status", "u.indexability_status", Text),
    ] {
        columns.push(ColumnDef::new(key, sql, kind, true));
    }
    let raw = words("raw");
    let dom = words("rendered");
    for (key, sql, kind) in [
        ("HTML Word Count", raw.clone(), Int),
        ("Rendered HTML Word Count", dom.clone(), Int),
        ("Word Count Change", format!("({dom}-{raw})"), Int),
        (
            "JS Word Count %",
            format!("CASE WHEN {dom}>0 THEN ROUND(100.0*max({dom}-{raw},0)/{dom},2) ELSE 0.0 END"),
            ColumnKind::Float { decimals: 2 },
        ),
    ] {
        columns.push(ColumnDef::new(key, intern(&sql), kind, true));
    }
    for (label, key) in [
        ("Title", "title"),
        ("Meta Description", "description"),
        ("H1", "h1"),
        ("Canonical", "html_canonical"),
        ("Meta Robots", "robots"),
    ] {
        for (caption, prefix) in [("HTML", "raw"), ("Rendered HTML", "rendered")] {
            columns.push(ColumnDef::dynamic(
                &format!("{caption} {label}"),
                &fact(&format!("{prefix}_{key}"), false),
                Text,
                true,
            ));
        }
    }
    for (key, sql) in [
        (
            "Unique Inlinks",
            "(c.unique_inlinks-CASE WHEN c.self_inlinks>0 THEN 1 ELSE 0 END)".into(),
        ),
        ("Unique JS Inlinks", js_links(true, false)),
        (
            "Unique Outlinks",
            "(c.unique_outlinks-CASE WHEN c.self_outlinks>0 THEN 1 ELSE 0 END)".into(),
        ),
        ("Unique JS Outlinks", js_links(false, false)),
        (
            "Unique External Outlinks",
            "c.unique_external_outlinks".into(),
        ),
        ("Unique External JS Outlinks", js_links(false, true)),
    ] {
        columns.push(ColumnDef::new(key, intern(&sql), Int, true));
    }
    let mut filters = vec![
        filter("All", "1"),
        filter(
            "Pages With Blocked Resources",
            "EXISTS (SELECT 1 FROM render_blocked b WHERE b.url_id=u.id AND b.reason='robots')",
        ),
        filter(
            "Contains JavaScript Links",
            &format!("{}+{}>=1", js_links(false, false), js_links(false, true)),
        ),
        filter("Contains JavaScript Content", &format!("{dom}>{raw}")),
    ];
    for (id, t) in [
        ("Noindex Only In Original HTML", "noindex"),
        ("Nofollow Only In Original HTML", "nofollow"),
    ] {
        filters.push(filter(
            id,
            &format!("{} AND NOT {}", token("raw", t), token("rendered", t)),
        ));
    }
    let raw = text("raw_html_canonical");
    let dom = text("rendered_html_canonical");
    filters.push(filter(
        "Canonical Only In Rendered HTML",
        &format!("{raw}='' AND {dom}<>''"),
    ));
    filters.push(filter(
        "Canonical Mismatch",
        &format!("{raw}<>'' AND {dom}<>'' AND {raw} COLLATE BINARY <> {dom} COLLATE BINARY"),
    ));
    for (key, only, updated) in [
        (
            "title",
            "Page Title Only In Rendered HTML",
            "Page Title Updated By JavaScript",
        ),
        (
            "description",
            "Meta Description Only In Rendered HTML",
            "Meta Description Updated By JavaScript",
        ),
        ("h1", "H1 Only In Rendered HTML", "H1 Updated By JavaScript"),
    ] {
        let raw = text(&format!("raw_{key}"));
        let dom = text(&format!("rendered_{key}"));
        filters.push(filter(only, &format!("{raw}='' AND {dom}<>''")));
        filters.push(filter(
            updated,
            &format!("{raw}<>'' AND {dom}<>'' AND {raw} COLLATE BINARY <> {dom} COLLATE BINARY"),
        ));
    }
    let mut errors=filter("Pages With JavaScript Errors","EXISTS (SELECT 1 FROM render_console e WHERE e.url_id=u.id AND e.level IN ('error','exception'))");
    errors
        .requires
        .push(Requirement::Config("rendering.js_error_reporting"));
    filters.push(errors);
    TabDef {
        id: "JavaScript",
        population: POPULATION,
        columns,
        filters,
        default_sort: vec![],
        params: |_| vec![],
        hidden_population: None,
    }
}
