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
        let changes=super::ui_change::page(&mut a.comparison,&a.elements,super::ui_change::PageRequest{filter:"All",change:Some("changed"),offset:0,limit:1,content_threshold_pct:a.content_threshold_pct,sort:None},&ctx.cancel)?;
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
    sort: Option<SortKey>,
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
            let mut page = super::ui_change::page(c, &a.elements, super::ui_change::PageRequest { filter: &q.filter, change: q.class.as_deref(), offset: args.offset, limit: args.limit, content_threshold_pct: a.content_threshold_pct, sort: q.sort.as_ref() }, &ctx.cancel)?;
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
            let columns: Vec<_> = os_analyze::compare::links::LINK_COLUMNS.iter().chain(os_analyze::compare::links::CHANGE_COLUMNS.iter()).copied().collect();
            // Keys identify the original row, independently of filtering, sorting and paging.
            let mut rows: Vec<_> = all.into_iter().map(|row| {
                let key = links.current.iter().position(|r| std::ptr::eq(r, row)).map(|i| i as i64 + 1)
                    .or_else(|| links.previous.iter().position(|r| std::ptr::eq(r, row)).map(|i| -(i as i64) - 1)).expect("link belongs to comparison");
                (key, row.export_values())
            }).collect();
            if let Some(sort) = &q.sort {
                let index = columns.iter().position(|column| *column == sort.column).ok_or_else(|| CmdError::invalid_input("err-compare-input", "sort"))?;
                sort_link_rows(&mut rows, index, sort.dir);
            }
            let total = rows.len();
            let page: Vec<_> = rows.into_iter().skip(args.offset as usize).take(args.limit as usize).collect();
            let row_keys: Vec<_> = page.iter().map(|(key, _)| *key).collect();
            let values: Vec<_> = page.into_iter().map(|(_, values)| values).collect();
            return Ok(json!({"viewId":args.view_id,"version":args.version,"offset":args.offset,"columns":columns,"cells":values,
                "rowKeys":row_keys,"total":total,"side":q.side,"filtersEnabled":links.filters_enabled()}));
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
        if let Some(sort) = &q.sort { query.sort = super::super::view::sorts(std::slice::from_ref(sort)); }
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

fn sort_link_rows(rows: &mut [(i64, Vec<Value>)], index: usize, dir: SortDir) {
    rows.sort_by(|a, b| {
        let cmp = match (&a.1[index], &b.1[index]) {
            (Value::Null, Value::Null) => std::cmp::Ordering::Equal,
            (Value::Null, _) => std::cmp::Ordering::Less,
            (_, Value::Null) => std::cmp::Ordering::Greater,
            (Value::Int(x), Value::Int(y)) => x.cmp(y),
            (Value::Float(x), Value::Float(y)) => x.total_cmp(y),
            (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
            (Value::Text(x), Value::Text(y)) => x.cmp(y),
            (x, y) => format!("{x:?}").cmp(&format!("{y:?}")),
        };
        (if dir == SortDir::Desc { cmp.reverse() } else { cmp }).then_with(|| a.0.cmp(&b.0))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    // spec: TECH-301
    #[test]
    fn link_sort_precedes_paging_and_preserves_keys() {
        let mut rows: Vec<_> = (0..205).map(|i| (i + 1, vec![Value::Int(204 - i)])).collect();
        sort_link_rows(&mut rows, 0, SortDir::Asc);
        assert_eq!(rows[99], (106, vec![Value::Int(99)]));
        assert_eq!(rows[100], (105, vec![Value::Int(100)]));
        sort_link_rows(&mut rows, 0, SortDir::Desc);
        assert_eq!(rows[105], (106, vec![Value::Int(99)]));
    }
}
