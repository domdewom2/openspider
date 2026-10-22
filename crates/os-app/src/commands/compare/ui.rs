//! Additive U23 read adapters for the existing sidebar/view commands. No new IPC command names.
use os_analyze::compare::{
    ChangeFilter, CompareClass, FilterComparison, LinkDirection, LinkSource, Side,
};
use os_analyze::views::{self, ColumnSelection, PageStart, ViewQuery};
use os_model::{UrlId, Value};
use serde::Deserialize;
use serde_json::{json, Value as Json};

use super::{compare_error, with_active};
use crate::{
    dto::*,
    runtime::{error::parse_args, App, CallCtx},
};

fn filter_json(f: &FilterComparison) -> Json {
    json!({"tab":f.tab,"filter":f.filter,"label":f.label,"current":f.current,"previous":f.previous,
        "change":f.change(),"classes":f.classes})
}

/// Explicit opt-in keeps ordinary single-crawl sidebar consumers unchanged.
pub(crate) async fn overview(app: &App, ctx: CallCtx) -> Result<Json, CmdError> {
    with_active(app, &ctx.window, move |a| {
        let rows = a.comparison.overview(&ctx.cancel).map_err(compare_error)?;
        let mut overview:Vec<_>=rows.iter().map(filter_json).collect();
        let changes=super::ui_change::page(&mut a.comparison,&a.elements,super::ui_change::PageRequest{filter:"All",change:Some("changed"),offset:0,limit:1,content_threshold_pct:a.content_threshold_pct},&ctx.cancel)?;
        for element in std::iter::once("All").chain(a.elements.iter().map(|e|super::ui_change::filter_name(e))) {
            let n=if element=="All" {changes["changedTotal"].as_u64().unwrap_or(0)} else {changes["changedCounts"][element].as_u64().unwrap_or(0)};
            overview.push(json!({"tab":"Change Detection","filter":element,"label":element,"current":n,"previous":n,"change":n,"classes":null}));
        }
        let issues = a.comparison.issues(&ctx.cancel).map_err(compare_error)?;
        let notes: Vec<_> = a.comparison.notes().iter().map(|n| match n {
            os_analyze::compare::CompareNote::DifferentModes { .. } => json!({"kind":"modes"}),
            os_analyze::compare::CompareNote::Migrated { .. } => json!({"kind":"migration"}),
            os_analyze::compare::CompareNote::Collision(c) => json!({"kind":"collision","address":c.current_url,"winner":c.winner}),
            os_analyze::compare::CompareNote::MappingWarning(w) => json!({"kind":"mapping","rule":w.rule+1}),
        }).collect();
        Ok(json!({"summary":[],"tabs":[],"comparison":{
            "overview":overview,
            "issues":issues.iter().map(|i| filter_json(&i.filter)).collect::<Vec<_>>(),"notes":notes}}))
    }).await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Query {
    current: String,
    previous: String,
    #[serde(default = "default_side")]
    side: String,
    tab: String,
    filter: String,
    class: Option<String>,
    url_id: Option<i64>,
    source: Option<String>,
    change: Option<String>,
    #[serde(default)]
    search: String,
}
fn default_side() -> String {
    "Current".into()
}
fn side(s: &str) -> Result<Side, CmdError> {
    match s {
        "Current" => Ok(Side::Current),
        "Previous" => Ok(Side::Previous),
        _ => Err(CmdError::invalid_input("err-compare-input", "side")),
    }
}

/// Stateless pages identified by both crawl IDs and comparison version; bounded IPC payloads.
pub(crate) async fn rows(app: &App, ctx: CallCtx, raw: Json) -> Result<Json, CmdError> {
    let q: Query = parse_args(raw["comparison"].clone())?;
    let args: RowsArgs = parse_args(raw)?;
    if !(1..=500).contains(&args.limit) {
        return Err(CmdError::invalid_input("err-invalid-limit", "limit"));
    }
    with_active(app, &ctx.window, move |a| {
        let c = &mut a.comparison;
        if c.version() != args.version || c.current().meta.crawl_id.to_string() != q.current || c.previous().meta.crawl_id.to_string() != q.previous {
            return Err(CmdError::new(ErrorCode::Stale,"err-view-stale"));
        }
        ctx.cancel.checkpoint().map_err(|_| CmdError::new(ErrorCode::Cancelled,"err-cancelled"))?;
        if q.tab == "Change Detection" {
            let mut page = super::ui_change::page(c, &a.elements, super::ui_change::PageRequest { filter: &q.filter, change: q.class.as_deref(), offset: args.offset, limit: args.limit, content_threshold_pct: a.content_threshold_pct }, &ctx.cancel)?;
            page["viewId"] = json!(args.view_id); page["version"] = json!(args.version); page["offset"] = json!(args.offset);
            return Ok(page);
        }
        let upper = side(&q.side)?;
        if let Some(id) = q.url_id {
            let direction = match q.tab.as_str() { "inlinks" => LinkDirection::Inlinks, "outlinks" => LinkDirection::Outlinks,
                _ => return Err(CmdError::invalid_input("err-compare-input","tab")) };
            if !c.exists(upper,id) { return Err(CmdError::new(ErrorCode::NotFound,"err-url-not-found")); }
            let links = c.links(upper,id,direction).map_err(compare_error)?;
            let source = match q.source.as_deref().unwrap_or("Auto") { "Auto" => LinkSource::Auto, "Current" => LinkSource::Current, "Previous" => LinkSource::Previous,
                _ => return Err(CmdError::invalid_input("err-compare-input","source")) };
            let change = ChangeFilter::parse(q.change.as_deref().unwrap_or("All")).ok_or_else(|| CmdError::invalid_input("err-compare-input","change"))?;
            let all = links.view(source,upper,change);
            let total = all.len();
            let values: Vec<_> = all.into_iter().skip(args.offset as usize).take(args.limit as usize).map(|r| r.export_values()).collect();
            let columns: Vec<_> = os_analyze::compare::links::LINK_COLUMNS.iter().chain(os_analyze::compare::links::CHANGE_COLUMNS.iter()).copied().collect();
            return Ok(json!({"viewId":args.view_id,"version":args.version,"offset":args.offset,"columns":columns,"cells":values,
                "rowKeys":[],"total":total,"side":q.side,"filtersEnabled":links.filters_enabled()}));
        }
        let class = q.class.as_deref().map(|s| CompareClass::parse(s).ok_or_else(|| CmdError::invalid_input("err-compare-input","class"))).transpose()?;
        let selected_side = class.map_or(upper, |cl| if cl == CompareClass::Removed { Side::Current } else { cl.side() });
        let ids = if let Some(cl) = class {
            let classes = c.classes(&q.tab,&q.filter).map_err(compare_error)?.ok_or_else(|| CmdError::invalid_input("err-compare-input","filter"))?;
            if cl == CompareClass::Removed { classes.removed_current(c.pairing()) } else { classes.ids(cl,c.pairing()) }
        } else {
            c.filter_set(selected_side,&q.tab,&q.filter).map_err(compare_error)?.and_then(|f| f.ids.clone()).ok_or_else(|| CmdError::invalid_input("err-compare-input","filter"))?
        };
        let s = c.side(selected_side);
        let mut registry = os_analyze::tabs::configured_for_file(&s.config,&s.file);
        for tab in &mut registry {
            if tab.id == q.tab { for f in &mut tab.filters { f.predicate = "1"; f.ignore_non_indexable = false; } }
        }
        // Selection bypasses the original filter: Removed rows no longer satisfy it in Current.
        let mut query = ViewQuery::new(&q.tab,&q.filter);
        query.columns = if args.columns.is_empty() { ColumnSelection::Default } else { ColumnSelection::Named(args.columns) };
        query.start = PageStart::Offset(args.offset); query.limit = args.limit;
        if !q.search.is_empty() { query.search = Some(views::SearchSpec::Simple(q.search)); }
        let keys: Vec<_> = ids.iter().map(|id| UrlId(i64::from(id))).collect();
        let p = views::query_url_ids_in(&registry,&s.file,&s.config,&query,&keys,true).map_err(super::super::catalog::view_error)?;
        let mut columns = p.columns;
        columns.push("Comparison Status".into());
        if c.has_mapping() { columns.push("Previous Address".into()); }
        let classes = c.classes(&q.tab,&q.filter).map_err(compare_error)?;
        let mut cells = Vec::new();
        for (key,mut row) in p.row_keys.iter().zip(p.rows) {
            let id = key.0 as u32;
            let status = class.or_else(|| classes.as_ref().and_then(|k| CompareClass::ALL.into_iter().find(|cl| {
                cl.side() == selected_side && k.ids(*cl,c.pairing()).contains(id)
            })));
            row.push(Value::Text(status.map_or("Unchanged", |cl| cl.as_str()).into()));
            if c.has_mapping() {
                let previous = if selected_side == Side::Previous { Some(key.0) } else { c.partner(Side::Current,key.0) };
                let address: Option<String> = previous.and_then(|id| os_store::queries::url_by_id(c.previous().file.conn(), UrlId(id)).ok().flatten().map(|r|r.address));
                row.push(address.map_or(Value::Null,Value::Text));
            }
            cells.push(row);
        }
        Ok(json!({"viewId":args.view_id,"version":args.version,"offset":args.offset,"columns":columns,"cells":cells,
            "rowKeys":p.row_keys.iter().map(|id|id.0).collect::<Vec<_>>(),"partnerKeys":p.row_keys.iter().map(|id|c.partner(selected_side,id.0)).collect::<Vec<_>>(),"total":p.total,"side":selected_side.as_str(),"filtersEnabled":true}))
    }).await
}

#[cfg(test)]
#[allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "../../../../os-analyze/tests/compare_common.rs"]
mod fixture;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::AppConfig;
    use fixture::*;
    async fn open_pair(app: &App, old: &std::path::Path, new: &std::path::Path) -> CompareSession {
        serde_json::from_value(
            app.dispatch(
                CallCtx::for_window("main"),
                "compare.open",
                json!({"current":old,"previous":new,"mapping":[],"elements":[]}),
            )
            .await
            .unwrap(),
        )
        .unwrap()
    }
    fn args(s: &CompareSession, query: Json) -> Json {
        let mut query = query;
        query["current"] = json!(s.current.crawl_id);
        query["previous"] = json!(s.previous.crawl_id);
        json!({"viewId":"comparison-test","version":s.version,"offset":0,"limit":100,"columns":[],"comparison":query})
    }
    async fn read(app: &App, args: Json) -> Json {
        app.dispatch(CallCtx::for_window("main"), "view.rows", args)
            .await
            .unwrap()
    }

    // spec: OUT-142
    #[tokio::test(flavor = "multi_thread")]
    async fn overview_counts_and_every_class_page_use_the_same_fixture_population() {
        let old = v1().await;
        let new = v2().await;
        let home = tempfile::tempdir().unwrap();
        let app = App::new(AppConfig::new(home.path())).unwrap();
        let s = open_pair(&app, old.file.path(), new.file.path()).await;
        let overview = app
            .dispatch(
                CallCtx::for_window("main"),
                "sidebar.overview",
                json!({"comparison":true}),
            )
            .await
            .unwrap();
        for row in overview["comparison"]["overview"].as_array().unwrap() {
            if let (Some(c), Some(p), Some(k)) = (
                row["current"].as_i64(),
                row["previous"].as_i64(),
                row["classes"].as_array(),
            ) {
                assert_eq!(
                    c - p,
                    k[0].as_i64().unwrap() + k[1].as_i64().unwrap()
                        - k[2].as_i64().unwrap()
                        - k[3].as_i64().unwrap()
                );
            }
        }
        let html = overview["comparison"]["overview"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["tab"] == "Internal" && r["filter"] == "HTML")
            .unwrap();
        for (i, class) in ["Added", "New", "Removed", "Missing"].iter().enumerate() {
            let page = read(
                &app,
                args(&s, json!({"tab":"Internal","filter":"HTML","class":class})),
            )
            .await;
            assert_eq!(page["total"], html["classes"][i]);
            assert_eq!(
                page["cells"].as_array().unwrap().len() as u64,
                page["total"].as_u64().unwrap()
            );
        }
        // Removed rows must bypass the old 4xx predicate and show the repaired Current row.
        let page = read(
            &app,
            args(
                &s,
                json!({"tab":"Response Codes","filter":"Client Error (4xx)","class":"Removed"}),
            ),
        )
        .await;
        assert!(page["total"].as_u64().unwrap() > 0);
        assert!(page["cells"].to_string().contains("missing.html"));
        let mut stale = args(&s, json!({"tab":"Internal","filter":"HTML"}));
        stale["comparison"]["current"] = json!("other-crawl");
        assert_eq!(
            app.dispatch(CallCtx::for_window("main"), "view.rows", stale)
                .await
                .unwrap_err()
                .code,
            ErrorCode::Stale
        );
        assert_eq!(
            app.dispatch(
                CallCtx::for_window("other"),
                "view.rows",
                args(&s, json!({"tab":"Internal","filter":"HTML"}))
            )
            .await
            .unwrap_err()
            .code,
            ErrorCode::NotFound
        );
    }
    // spec: CRAWL-144
    #[tokio::test(flavor = "multi_thread")]
    async fn link_pages_preserve_anchor_changes_sources_and_one_sided_urls() {
        let old = v1().await;
        let new = v2().await;
        let home = tempfile::tempdir().unwrap();
        let app = App::new(AppConfig::new(home.path())).unwrap();
        let s = open_pair(&app, old.file.path(), new.file.path()).await;
        let id = id_of(&new.file, &www("/c.html"));
        let page=read(&app,args(&s,json!({"tab":"inlinks","filter":"All","urlId":id,"source":"Auto","change":"Anchor Changed"}))).await;
        assert_eq!(page["total"], 1);
        let row = &page["cells"][0];
        assert_eq!(row[5], "Neu");
        assert_eq!(row[15], "Anchor Changed");
        assert_eq!(row[16], "Current");
        assert_eq!(row[17], "Alt");
        let id = id_of(&new.file, &www("/start.html"));
        let removed=read(&app,args(&s,json!({"tab":"outlinks","filter":"All","urlId":id,"source":"Current","change":"Removed"}))).await;
        assert!(removed["cells"].to_string().contains("b.html"));
        let before=read(&app,args(&s,json!({"tab":"outlinks","filter":"All","urlId":id,"source":"Previous","change":"All"}))).await;
        let after=read(&app,args(&s,json!({"tab":"outlinks","filter":"All","urlId":id,"source":"Current","change":"All"}))).await;
        assert!(before["cells"].to_string().contains("b.html"));
        assert!(!after["cells"].to_string().contains("b.html"));
        let id = id_of(&new.file, &www("/new1.html"));
        let one=read(&app,args(&s,json!({"tab":"outlinks","filter":"All","urlId":id,"source":"Previous","change":"Removed"}))).await;
        assert_eq!(one["filtersEnabled"], false);
    }
    // spec: TECH-301
    #[tokio::test(flavor = "multi_thread")]
    async fn change_projection_has_only_paired_html_and_numeric_deltas() {
        let old = v1().await;
        let new = v2().await;
        let home = tempfile::tempdir().unwrap();
        let app = App::new(AppConfig::new(home.path())).unwrap();
        let s = open_pair(&app, old.file.path(), new.file.path()).await;
        let page = read(
            &app,
            args(&s, json!({"tab":"Change Detection","filter":"All"})),
        )
        .await;
        let cols = page["columns"].as_array().unwrap();
        assert_eq!(cols[0], "Address");
        assert!(cols.contains(&json!("Title 1 (Previous)")));
        assert!(cols.contains(&json!("Word Count Change %")));
        assert!(page["total"].as_u64().unwrap() > 0);
        for row in page["cells"].as_array().unwrap() {
            assert!(!row[0].as_str().unwrap().contains("new1.html"));
            assert_eq!(row.as_array().unwrap().len(), cols.len());
        }
        let cur = cols
            .iter()
            .position(|c| c == "Word Count (Current)")
            .unwrap();
        let prev = cols
            .iter()
            .position(|c| c == "Word Count (Previous)")
            .unwrap();
        let delta = cols.iter().position(|c| c == "Word Count Change").unwrap();
        let pct = cols
            .iter()
            .position(|c| c == "Word Count Change %")
            .unwrap();
        for row in page["cells"].as_array().unwrap() {
            if let (Some(c), Some(p)) = (row[cur].as_f64(), row[prev].as_f64()) {
                assert_eq!(row[delta].as_f64(), Some(c - p));
                if p == 0. {
                    assert!(row[pct].is_null());
                }
            }
        }
    }
}
