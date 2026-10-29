//! URL properties and stored HTTP headers; no network access.
use super::catalog::{session, view_error};
use crate::{
    dto::*,
    registry::{CmdResult, CommandMeta, Registry},
    runtime::{
        error::{parse_args, to_json},
        App, CallCtx,
    },
};
use futures_util::future::BoxFuture;
use os_model::{UrlId, Value};
const FIELDS: [(&str, &str); 25] = [
    ("Address", "Address"),
    ("URL Encoded Address", "URL Encoded Address"),
    ("Content Type", "Content"),
    ("Status Code", "Status Code"),
    ("Status", "Status"),
    ("Indexability", "Indexability"),
    ("Indexability Status", "Indexability Status"),
    ("Size", "Size (bytes)"),
    ("Transferred", "Transferred (bytes)"),
    ("Response Time", "Response Time"),
    ("Crawl Depth", "Crawl Depth"),
    ("Link Score", "Link Score"),
    ("Inlinks", "Inlinks"),
    ("Unique Inlinks", "Unique Inlinks"),
    ("Outlinks", "Outlinks"),
    ("Unique Outlinks", "Unique Outlinks"),
    ("External Outlinks", "External Outlinks"),
    ("Redirect URL", "Redirect URL"),
    ("Redirect Type", "Redirect Type"),
    ("Canonical Link Element 1", "Canonical Link Element 1"),
    ("Meta Robots 1", "Meta Robots 1"),
    ("X-Robots-Tag 1", "X-Robots-Tag 1"),
    ("Last Modified", "Last-Modified"),
    ("Crawl Timestamp", "Crawl Timestamp"),
    ("Segment", "Segment"),
];
fn url_details<'a>(
    app: &'a App,
    ctx: CallCtx,
    args: serde_json::Value,
) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: UrlIdArgs = parse_args(args)?;
        let s = session(app, &ctx)?;
        app.read_pool()
            .run_cached(
                s.key,
                "detail-properties",
                crate::runtime::current_cancel(),
                move |file, _| {
                    let _snapshot = file.conn().unchecked_transaction()?;
                    Ok((|| {
                        let props = os_analyze::details::url_properties(
                            file,
                            &s.config,
                            UrlId(args.url_id),
                        )
                        .map_err(view_error)?;
                        if props.is_empty() {
                            return Err(CmdError::new(ErrorCode::NotFound, "err-url-not-found"));
                        }
                        let mut rows = FIELDS
                            .iter()
                            .map(|(name, key)| NameValue {
                                name: (*name).into(),
                                value: props.iter().find(|(k, _)| k == key).and_then(|(_, v)| {
                                    if matches!(v, Value::Null)
                                        || super::catalog::extraction_key(key).is_some_and(|k| {
                                            !super::catalog::config_on(&s.config, k)
                                        })
                                    {
                                        None
                                    } else {
                                        serde_json::to_value(v).ok()
                                    }
                                }),
                            })
                            .collect::<Vec<_>>();
                        for (name, value) in
                            os_analyze::details::url_details(file, &s.config, UrlId(args.url_id))
                                .map_err(view_error)?
                        {
                            // Leerer Text = kein Wert (z. B. Link Score vor dem Analyseschritt, M2-D08).
                            let value =
                                (!value.is_empty()).then_some(serde_json::Value::String(value));
                            if let Some(row) = rows.iter_mut().find(|row| row.name == name) {
                                row.value = value;
                            } else {
                                rows.push(NameValue { name, value });
                            }
                        }
                        to_json(&rows)
                    })())
                },
            )
            .await?
    })
}
fn headers<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: UrlIdArgs = parse_args(args)?;
        let s = session(app, &ctx)?;
        if let Some(gate) =
            super::view::detail_gate(os_analyze::details::DetailKind::Headers, &s.config)
        {
            return Err(
                CmdError::new(ErrorCode::Unsupported, "err-detail-unavailable")
                    .with_arg("state", serde_json::to_value(gate).unwrap_or_default()),
            );
        }
        app.read_pool()
            .run_cached(
                s.key,
                "detail-properties",
                crate::runtime::current_cancel(),
                move |file, _| {
                    let _snapshot = file.conn().unchecked_transaction()?;
                    if os_store::queries::url_by_id(file.conn(), UrlId(args.url_id))?.is_none() {
                        return Ok(Err(CmdError::new(ErrorCode::NotFound, "err-url-not-found")));
                    }
                    let mut dto = HeadersDto {
                        status_line: os_store::queries::app_support::http_status_line(
                            file.conn(),
                            UrlId(args.url_id),
                        )?,
                        request: vec![],
                        response: vec![],
                    };
                    if s.config.extraction.http_headers {
                        for h in os_store::queries::headers(file.conn(), UrlId(args.url_id))? {
                            let value = os_fetch::redact::header_value(&h.name, &h.value);
                            let row = Header {
                                name: h.name,
                                value,
                            };
                            if h.request {
                                dto.request.push(row);
                            } else {
                                dto.response.push(row);
                            }
                        }
                    }
                    Ok(to_json(&dto))
                },
            )
            .await?
    })
}
pub fn register(r: &mut Registry) {
    for (name, args, ret, h) in [
        (
            "detail.urlDetails",
            "UrlIdArgs",
            "NameValue[]",
            url_details as crate::registry::Handler,
        ),
        ("detail.headers", "UrlIdArgs", "HeadersDto", headers),
    ] {
        r.register(
            CommandMeta {
                name,
                args,
                ret,
                long_running: false,
            },
            h,
        );
    }
}
