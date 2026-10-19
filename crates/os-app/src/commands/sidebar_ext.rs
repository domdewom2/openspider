//! Sidebar-Aggregate und Konfigurationsinvalidierung (M2-K03).
use std::collections::BTreeMap;
use std::sync::Arc;

use futures_util::future::BoxFuture;
use os_analyze::aggregates::{self, Counts};
use os_model::CrawlConfig;
use serde_json::Value;

use crate::dto::{
    Bucket, CmdError, DepthRow, ErrorCode, PathNode, ResponseTimeScope, ResponseTimes,
    ResponseTimesArgs, SiteStructure, SiteStructureArgs, SiteStructureView,
};
use crate::pool::CrawlKey;
use crate::registry::{CmdResult, CommandMeta, Registry};
use crate::runtime::error::{no_crawl, parse_args, to_json};
use crate::{App, CallCtx};

fn classes(counts: &Counts, view: SiteStructureView) -> BTreeMap<String, u64> {
    match view {
        SiteStructureView::Indexability => BTreeMap::from([
            ("Indexable".into(), counts.indexable),
            ("Non-Indexable".into(), counts.non_indexable),
        ]),
        SiteStructureView::ResponseCodes => counts.response_codes.clone(),
        SiteStructureView::CrawlDepth => BTreeMap::new(),
    }
}

fn path_node(row: aggregates::PathTree) -> PathNode {
    PathNode {
        name: row.row.name,
        path: row.row.path,
        count: row.row.counts.count,
        by_class: classes(&row.row.counts, SiteStructureView::Indexability),
        children: row.children.into_iter().map(path_node).collect(),
        has_more_children: row.has_more_children,
    }
}

fn site_structure<'a>(app: &'a App, ctx: CallCtx, args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: SiteStructureArgs = parse_args(args)?;
        if !(1..=500).contains(&args.limit) {
            return Err(CmdError::new(ErrorCode::InvalidInput, "err-invalid-limit"));
        }
        if args.segment.is_some() {
            return Err(CmdError::unsupported(
                "sidebar.siteStructure.segment",
                "M2-K14",
            ));
        }
        let session = app
            .sessions()
            .current(&ctx.window)
            .ok_or_else(|| no_crawl(&ctx.window.0))?;
        let result = app
            .read_pool()
            .run_cached(session.key, "sidebar", ctx.cancel, move |file, cancel| {
                aggregates::live::refresh_with_cancel(file, &session.config, cancel)?;
                let paths = aggregates::path_tree(
                    file,
                    args.path.as_deref().unwrap_or(""),
                    args.offset,
                    args.limit,
                    if args.path.is_some() { 1 } else { 4 },
                )?
                .into_iter()
                .map(path_node)
                .collect();
                let depths = aggregates::depths(file)?
                    .into_iter()
                    .map(|(depth, c)| DepthRow {
                        depth,
                        count: c.count,
                        by_class: classes(&c, args.view),
                    })
                    .collect();
                Ok(SiteStructure { paths, depths })
            })
            .await?;
        to_json(&result)
    })
}

fn response_times<'a>(app: &'a App, ctx: CallCtx, args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: ResponseTimesArgs = parse_args(args)?;
        let session = app
            .sessions()
            .current(&ctx.window)
            .ok_or_else(|| no_crawl(&ctx.window.0))?;
        let result = app
            .read_pool()
            .run_cached(session.key, "sidebar", ctx.cancel, move |file, cancel| {
                aggregates::live::refresh_with_cancel(file, &session.config, cancel)?;
                let (counts, no_response) = aggregates::response_times(
                    file,
                    args.scope == ResponseTimeScope::InternalHtml,
                )?;
                Ok(ResponseTimes {
                    buckets: counts
                        .into_iter()
                        .enumerate()
                        .map(|(i, count)| Bucket {
                            from_ms: i as u32 * 1000,
                            to_ms: if i == 10 {
                                None
                            } else {
                                Some((i as u32 + 1) * 1000)
                            },
                            count,
                        })
                        .collect(),
                    no_response,
                })
            })
            .await?;
        to_json(&result)
    })
}

pub fn register(r: &mut Registry) {
    r.register(
        CommandMeta {
            name: "sidebar.siteStructure",
            args: "SiteStructureArgs",
            ret: "SiteStructure",
            long_running: false,
        },
        site_structure,
    );
    r.register(
        CommandMeta {
            name: "sidebar.responseTimes",
            args: "ResponseTimesArgs",
            ret: "ResponseTimes",
            long_running: false,
        },
        response_times,
    );
}

/// Der F05b-Bus ruft diesen Hörer nach `config.set` auf. Keine Datenbankarbeit auf
/// dem aufrufenden Thread; die serialisierte Warteschlange bewahrt die Altwerte.
pub fn on_config_changed(
    app: &App,
    key: &CrawlKey,
    old: &Arc<CrawlConfig>,
    new: &Arc<CrawlConfig>,
) {
    let (app, key, old, new) = (app.clone(), key.clone(), old.clone(), new.clone());
    app.handle().spawn(async move {
        let result = app
            .write_queue()
            .submit_file(&key, move |file| {
                aggregates::status::on_config_changed(file, &old, &new).map_err(|e| {
                    CmdError::new(ErrorCode::Store, "err-store").with_detail(e.to_string())
                })
            })
            .await;
        if let Err(error) = result {
            tracing::warn!(?error, "Analyse-Invalidierung fehlgeschlagen");
        }
    });
}

/// Bereitet den Cache beim Öffnen vor; das Lesen einer Sidebar muss danach nur
/// noch den Feed nachführen. Fremde, laufende Dateien bleiben schreibgeschützt.
pub fn on_startup(app: &App) {
    app.crawl_events().subscribe(Box::new(|app, event| {
        use crate::runtime::lifecycle_bus::CrawlEvent;
        let key = match event {
            CrawlEvent::Opened(key) | CrawlEvent::Started(key) | CrawlEvent::Completed(key) => key,
            _ => return,
        };
        let Some(session) = app.sessions().all().into_iter().find(|s| &s.key == key) else {
            return;
        };
        let app = app.clone();
        app.handle().spawn(async move {
            if let Err(error) = app
                .read_pool()
                .run_cached(
                    session.key,
                    "sidebar",
                    Default::default(),
                    move |file, cancel| {
                        aggregates::live::refresh_with_cancel(file, &session.config, cancel)
                    },
                )
                .await
            {
                tracing::warn!(?error, "Vorberechnung der Sidebar fehlgeschlagen");
            }
        });
    }));
}

/// Abbildung für `analysis.status` (Command-Besitzer K04). Der Kernvertrag bleibt
/// bei `StepState`; der Hinweiszustand `NeedsRecrawl` gehört ausschließlich ins DTO.
pub fn step_statuses(
    data: &aggregates::status::AnalysisStatusData,
) -> Result<Vec<crate::dto::AnalysisStepStatus>, CmdError> {
    use crate::dto::{AnalysisStepStatus, StepStateDto};
    use os_model::StepState;
    if let Some(error) = &data.error {
        return Err(CmdError::new(ErrorCode::Store, "err-store").with_detail(error));
    }
    Ok(data
        .steps
        .iter()
        .map(|(step, state)| AnalysisStepStatus {
            step: step.as_str().into(),
            stale: data.stale.contains(step),
            state: if data.needs_recrawl.contains(step) {
                StepStateDto::NeedsRecrawl
            } else {
                match state {
                    StepState::Pending => StepStateDto::Pending,
                    StepState::Done => StepStateDto::Done,
                    StepState::Skipped => StepStateDto::Skipped,
                    StepState::NotApplicable => StepStateDto::NotApplicable,
                }
            },
        })
        .collect())
}
