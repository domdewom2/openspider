//! Detail services owned by M2-K09. Storage readers run exclusively in the app's pool.
use super::catalog::{session, view_error};
use crate::{
    assets,
    dto::*,
    registry::{CmdResult, CommandMeta, Registry},
    runtime::{
        error::{parse_args, store_error, to_json},
        App, CallCtx,
    },
    source,
};
use futures_util::future::BoxFuture;
use os_model::{UrlId, Value};
use os_store::queries::detail_ext as storage;

macro_rules! ensure_url {
    ($file:expr,$id:expr) => {
        source::ensure_url(
            os_store::queries::url_by_id($file.conn(), UrlId($id))
                .map_err(|e| store_error(&e))?
                .is_some(),
        )?
    };
}
macro_rules! load_source {
    ($file:expr,$id:expr,$which:expr) => {{
        ensure_url!($file, $id);
        let which = $which;
        let stored = match which {
            SourceWhich::Original => os_store::queries::stored_html($file, UrlId($id)),
            SourceWhich::Rendered => os_store::queries::stored_rendered_html($file, UrlId($id)),
        }
        .map_err(|e| store_error(&e))?;
        source::load(stored, which)?
    }};
}
fn source_gate(cfg: &os_model::CrawlConfig, which: SourceWhich) -> Result<(), CmdError> {
    match which {
        SourceWhich::Original if !cfg.extraction.store_html => {
            Err(source::unavailable("extraction.store_html"))
        }
        SourceWhich::Rendered if !cfg.extraction.store_rendered_html => {
            Err(source::unavailable("extraction.store_rendered_html"))
        }
        _ => Ok(()),
    }
}
fn dispatch<'a>(
    app: &'a App,
    ctx: CallCtx,
    args: serde_json::Value,
    action: &'static str,
) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        if action == "detail.lighthouse" {
            return Err(CmdError::unsupported(action, "P2"));
        }
        let s = session(app, &ctx)?;
        match action {
            "detail.serp" => {
                let a: SerpArgs = parse_args(args)?;
                app.read_pool()
                    .run_snapshot(s.key.clone(), move |file, _| {
                        Ok((|| {
                            let mut dto = source::serp_data(
                                storage::serp_data(file.conn(), a.url_id)
                                    .map_err(|e| store_error(&e))?,
                            )?;
                            dto.favicon = dto
                                .favicon
                                .and_then(|id| id.parse().ok())
                                .map(|id| assets::uri("img", &s.key, id));
                            to_json(&dto)
                        })())
                    })
                    .await?
            }
            "detail.serpMeasure" => {
                let a: SerpMeasureArgs = parse_args(args)?;
                tokio::task::spawn_blocking(move || to_json(&source::measure(&s.config, a)?))
                    .await
                    .map_err(|e| crate::runtime::error::internal("serpMeasure", e.to_string()))?
            }
            "detail.serpSaveEdit" | "detail.serpReset" => {
                let a: SerpEditArgs = parse_args(args)?;
                if !s.writable {
                    return Err(CmdError::new(ErrorCode::Permission, "err-db-read-only"));
                }
                if action == "detail.serpSaveEdit"
                    && a.title.as_ref().map_or(0, String::len)
                        + a.description.as_ref().map_or(0, String::len)
                        > 100_000
                {
                    return Err(CmdError::invalid_input("err-serp-too-large", "title"));
                }
                let edit =
                    (action == "detail.serpSaveEdit").then(|| os_store::queries::serp::SerpEdit {
                        device: "desktop".into(),
                        title: a.title,
                        description: a.description,
                        edited_at: app.config().clock.now().to_db(),
                    });
                let saved = app
                    .write_queue()
                    .submit(s.key, move |conn| {
                        storage::save_serp_edit(conn, a.url_id, edit.as_ref())
                    })
                    .await?;
                source::ensure_url(saved)?;
                Ok(serde_json::Value::Null)
            }
            "detail.sourceSave" => {
                let a: SourceSaveArgs = parse_args(args)?;
                source_gate(&s.config, a.which)?;
                app.read_pool()
                    .run_snapshot(s.key, move |file, _| {
                        Ok((|| {
                            let html = load_source!(file, a.url_id, a.which);
                            let text = match a.mode {
                                SourceMode::Html => html,
                                SourceMode::Text => source::visible_text(&html),
                            };
                            to_json(&source::save_file(&a.path, text.as_bytes())?)
                        })())
                    })
                    .await?
            }
            "detail.screenshotSave" => {
                let a: ScreenshotSaveArgs = parse_args(args)?;
                app.read_pool()
                    .run_snapshot(s.key.clone(), move |file, _| {
                        Ok((|| {
                            ensure_url!(file, a.url_id);
                            let summary = storage::screenshot_summary(file.conn(), a.url_id)
                                .map_err(|e| store_error(&e))?;
                            assets::screenshot(summary, &s.key, a.url_id, &s.config)?;
                            let bytes = storage::screenshot_tile(
                                file.conn(),
                                a.url_id,
                                a.tile.unwrap_or(0),
                            )
                            .map_err(|e| store_error(&e))?
                            .ok_or_else(|| {
                                CmdError::new(ErrorCode::NotFound, "err-screenshot-not-stored")
                            })?;
                            to_json(&source::save_file(&a.path, &bytes)?)
                        })())
                    })
                    .await?
            }
            "detail.sourceOpen" => {
                let a: SourceOpenArgs = parse_args(args)?;
                source_gate(&s.config, a.which)?;
                let docs = app.ext::<source::Documents>();
                let app = app.clone();
                app.clone()
                    .read_pool()
                    .run_snapshot(s.key.clone(), move |file, _| {
                        Ok((|| {
                            let html = load_source!(file, a.url_id, a.which);
                            let text = match a.mode {
                                SourceMode::Text => source::visible_text(&html),
                                SourceMode::Html if a.pretty => source::pretty(&html),
                                _ => html,
                            };
                            to_json(&docs.open(&app, ctx.window, s.key, text)?)
                        })())
                    })
                    .await?
            }
            "detail.sourceLines" => {
                let a: SourceLinesArgs = parse_args(args)?;
                let docs = app.ext::<source::Documents>();
                tokio::task::spawn_blocking(move || {
                    to_json(&docs.lines(&ctx.window, &s.key, &a)?)
                })
                .await
                .map_err(|e| crate::runtime::error::internal("sourceLines", e.to_string()))?
            }
            "detail.sourceDiff" => {
                let a: SourceDiffArgs = parse_args(args)?;
                source_gate(&s.config, SourceWhich::Original)?;
                source_gate(&s.config, SourceWhich::Rendered)?;
                let app = app.clone();
                let ops_app = app.clone();
                to_json(
                    &ops_app
                        .ops()
                        .spawn("op-source-diff", true, move |cancel, progress| {
                            Box::pin(async move {
                                app.read_pool()
                                    .run_snapshot(s.key, move |file, _| {
                                        Ok((|| {
                                            progress.report(0, Some(3));
                                            let old =
                                                load_source!(file, a.url_id, SourceWhich::Original);
                                            progress.report(1, Some(3));
                                            let new =
                                                load_source!(file, a.url_id, SourceWhich::Rendered);
                                            let (old, new) = match a.mode {
                                                DiffMode::Words => (
                                                    source::visible_text(&old),
                                                    source::visible_text(&new),
                                                ),
                                                _ => (old, new),
                                            };
                                            progress.report(2, Some(3));
                                            let result = source::diff(&old, &new, a.mode, &cancel)?;
                                            progress.report(3, Some(3));
                                            to_json(&result)
                                        })())
                                    })
                                    .await?
                            })
                        }),
                )
            }
            "detail.screenshot" => {
                let a: UrlIdArgs = parse_args(args)?;
                app.read_pool()
                    .run_snapshot(s.key.clone(), move |file, _| {
                        Ok((|| {
                            ensure_url!(file, a.url_id);
                            let summary = storage::screenshot_summary(file.conn(), a.url_id)
                                .map_err(|e| store_error(&e))?;
                            to_json(&assets::screenshot(summary, &s.key, a.url_id, &s.config)?)
                        })())
                    })
                    .await?
            }
            "detail.imagePreview" => {
                let a: UrlIdArgs = parse_args(args)?;
                let id = a.url_id;
                let mut dto = app
                    .read_pool()
                    .run_snapshot(s.key.clone(), move |file, _| {
                        Ok((|| {
                            ensure_url!(file, id);
                            let data = storage::image_data(file, id)
                                .map_err(|e| store_error(&e))?
                                .ok_or_else(|| CmdError::invalid_input("err-not-image", "urlId"))?;
                            let (width, height) = data
                                .stored
                                .as_ref()
                                .and_then(|b| os_parse::images::preview_dimensions(&b.bytes))
                                .unwrap_or((data.width, data.height));
                            Ok(ImagePreview {
                                url: String::new(),
                                width,
                                height,
                                stored: data.stored.is_some(),
                            })
                        })())
                    })
                    .await??;
                dto.url = assets::uri("img", &s.key, id);
                if !dto.stored && a.allow_live.unwrap_or(false) {
                    dto.url.push('/');
                    dto.url.push_str(&assets::confirm_live(app, s.path, id));
                }
                to_json(&dto)
            }
            "detail.structuredData" => {
                let a: UrlIdArgs = parse_args(args)?;
                let lang = match app.settings_snapshot().general.language {
                    os_model::UiLanguage::De => os_analyze::issues::Lang::De,
                    _ => os_analyze::issues::Lang::En,
                };
                app.read_pool()
                    .run_snapshot(s.key, move |file, _| {
                        Ok((|| {
                            ensure_url!(file, a.url_id);
                            let items = storage::sd_items(file.conn(), a.url_id)
                                .map_err(|e| store_error(&e))?;
                            to_json(&source::structured::tree(items, &s.config, lang)?)
                        })())
                    })
                    .await?
            }
            "detail.duplicates" => {
                let a: DuplicatesArgs = parse_args(args)?;
                app.read_pool()
                    .run_snapshot(s.key, move |file, cancel| {
                        Ok((|| {
                            ensure_url!(file, a.url_id);
                            if a.kind == DupKind::Near {
                                if !s.config.duplicates.near_enabled
                                    || !s.config.analysis.near_duplicates
                                {
                                    return Err(source::unavailable("duplicates.near_enabled"));
                                }
                                if os_analyze::views::state_of(
                                    file,
                                    &s.config,
                                    "Content",
                                    "Near Duplicates",
                                )
                                .map_err(view_error)?
                                    != os_model::FilterState::Ready
                                {
                                    return Err(CmdError::new(
                                        ErrorCode::Unsupported,
                                        "err-detail-unavailable",
                                    )
                                    .with_arg(
                                        "state",
                                        serde_json::json!({"code":"NEEDS_CRAWL_ANALYSIS"}),
                                    ));
                                }
                            } else if !s.config.extraction.hash
                                || !s.config.duplicates.exact_enabled
                            {
                                return Err(source::unavailable("duplicates.exact_enabled"));
                            }
                            let mode = match a.kind {
                                DupKind::Exact => "Exact",
                                DupKind::Near => "Near",
                            };
                            let params = serde_json::json!({"filter":mode});
                            let ids = [UrlId(a.url_id)];
                            let offset = a.offset.unwrap_or(0);
                            let limit = a.limit.unwrap_or(100).clamp(1, 500);
                            if cancel.is_cancelled() {
                                return Err(CmdError::new(ErrorCode::Cancelled, "err-cancelled"));
                            }
                            let rows = os_analyze::tabs::duplicates::detail_rows(
                                file,
                                &s.config,
                                &os_analyze::details::DetailReq {
                                    url_ids: &ids,
                                    params: &params,
                                    sort: &[],
                                    offset,
                                    limit: u64::from(limit),
                                },
                            )
                            .map_err(view_error)?;
                            let total = rows.total;
                            let mut partners = Vec::with_capacity(rows.rows.len());
                            for row in rows.rows {
                                let Value::Text(address) = &row[0] else {
                                    continue;
                                };
                                let id = storage::url_id_by_address(file.conn(), address)
                                    .map_err(|e| store_error(&e))?
                                    .ok_or_else(|| {
                                        CmdError::new(ErrorCode::NotFound, "err-url-not-found")
                                    })?;
                                partners.push(DupPartner {
                                    url_id: id,
                                    address: address.clone(),
                                    similarity: match row[1] {
                                        Value::Int(n) => Some(n as f64),
                                        _ => None,
                                    },
                                    match_type: Some(mode.into()),
                                    text: None,
                                    diff: None,
                                });
                            }
                            let focus = storage::content_text(file.conn(), a.url_id)
                                .map_err(|e| store_error(&e))?;
                            if let Some(id) = a.partner_id {
                                let partner = partners
                                    .iter_mut()
                                    .find(|p| p.url_id == id)
                                    .ok_or_else(|| {
                                        CmdError::invalid_input(
                                            "err-duplicate-partner",
                                            "partnerId",
                                        )
                                    })?;
                                partner.text = storage::content_text(file.conn(), id)
                                    .map_err(|e| store_error(&e))?;
                                partner.diff = match (&focus, &partner.text) {
                                    (Some(f), Some(o)) => {
                                        Some(source::diff(f, o, DiffMode::Words, cancel)?)
                                    }
                                    _ => None,
                                };
                            }
                            let end = offset.saturating_add(partners.len() as u64);
                            to_json(&DuplicatesResult {
                                total: Some(total),
                                next_offset: (end < total && !partners.is_empty()).then_some(end),
                                partners,
                                text: focus,
                            })
                        })())
                    })
                    .await?
            }
            "crawl.path" => {
                let a: UrlIdArgs = parse_args(args)?;
                app.read_pool()
                    .run_snapshot(s.key, move |file, _| {
                        Ok((|| {
                            ensure_url!(file, a.url_id);
                            let path = source::root_first_crawl_path(
                                storage::path_rows(file, a.url_id).map_err(|e| store_error(&e))?,
                            );
                            let mut result = vec![];
                            for row in path.rows {
                                let Value::Text(address) = &row[2] else {
                                    continue;
                                };
                                let id = storage::url_id_by_address(file.conn(), address)
                                    .map_err(|e| store_error(&e))?
                                    .unwrap_or(0);
                                let integer = |i: usize| match row[i] {
                                    Value::Int(n) => Some(n),
                                    _ => None,
                                };
                                let text = |i: usize| match &row[i] {
                                    Value::Text(t) => Some(t.clone()),
                                    _ => None,
                                };
                                let via = match text(4) {
                                    Some(v) => Some(v),
                                    None => storage::list_origin(file.conn(), id)
                                        .map_err(|e| store_error(&e))?,
                                };
                                result.push(CrawlPathDto {
                                    depth: integer(6).unwrap_or(result.len() as i64) as u32,
                                    url_id: id,
                                    address: address.clone(),
                                    status_code: integer(3).and_then(|n| n.try_into().ok()),
                                    via,
                                    anchor: text(5),
                                });
                            }
                            to_json(&result)
                        })())
                    })
                    .await?
            }
            _ => unreachable!("registered command"),
        }
    })
}
pub fn register(r: &mut Registry) {
    macro_rules! command {
        ($name:literal,$args:literal,$ret:literal,$long:literal) => {
            r.register(
                CommandMeta {
                    name: $name,
                    args: $args,
                    ret: $ret,
                    long_running: $long,
                },
                |app, ctx, args| dispatch(app, ctx, args, $name),
            );
        };
    }
    command!("detail.serp", "SerpArgs", "SerpData", false);
    command!(
        "detail.serpMeasure",
        "SerpMeasureArgs",
        "SerpMeasure",
        false
    );
    command!("detail.serpSaveEdit", "SerpEditArgs", "void", false);
    command!("detail.serpReset", "SerpEditArgs", "void", false);
    command!("detail.sourceSave", "SourceSaveArgs", "FileResult", false);
    command!(
        "detail.screenshotSave",
        "ScreenshotSaveArgs",
        "FileResult",
        false
    );
    command!("detail.sourceOpen", "SourceOpenArgs", "SourceDoc", false);
    command!("detail.sourceLines", "SourceLinesArgs", "string[]", false);
    command!("detail.sourceDiff", "SourceDiffArgs", "DiffHunks", true);
    command!("detail.screenshot", "UrlIdArgs", "ScreenshotInfo", false);
    command!("detail.imagePreview", "UrlIdArgs", "ImagePreview", false);
    command!("detail.structuredData", "UrlIdArgs", "SdTree", false);
    command!(
        "detail.duplicates",
        "DuplicatesArgs",
        "DuplicatesResult",
        false
    );
    command!(
        "detail.lighthouse",
        "LighthouseArgs",
        "LighthouseDto",
        false
    );
    command!("crawl.path", "UrlIdArgs", "CrawlPathDto[]", false);
}
