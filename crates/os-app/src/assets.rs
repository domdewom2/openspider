//! Crawl-scoped screenshot tiles and confirmed image previews over `ospider://`.
use crate::{
    dto::*,
    runtime::{
        hooks::{AssetResolver, AssetResponse},
        App,
    },
    source::Documents,
};
use futures_util::future::BoxFuture;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Default)]
struct LiveGrants(Mutex<HashMap<String, (std::path::PathBuf, i64)>>);
struct Resolver;
impl AssetResolver for Resolver {
    fn resolve(&self, app: App, uri: String) -> BoxFuture<'static, AssetResponse> {
        Box::pin(async move { resolve(&app, &uri).await })
    }
}
pub fn init(app: &App) {
    app.set_asset_resolver(Arc::new(Resolver));
    let docs = app.ext::<Documents>();
    app.on_window_closed(Box::new(move |_, window| docs.clear_window(window)));
    app.crawl_events().subscribe(Box::new(|app, event| {
        use crate::runtime::lifecycle_bus::CrawlEvent;
        if let CrawlEvent::Closed(key) | CrawlEvent::Cleared(key) = event {
            app.ext::<Documents>().clear_crawl(key);
            app.ext::<LiveGrants>()
                .0
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .clear();
        }
    }));
}
pub fn crawl_segment(key: &crate::pool::CrawlKey) -> String {
    key.0
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub fn uri(kind: &str, key: &crate::pool::CrawlKey, id: i64) -> String {
    format!("ospider://{kind}/{}/{id}", crawl_segment(key))
}
pub fn screenshot(
    summary: (u32, u32, u32),
    key: &crate::pool::CrawlKey,
    id: i64,
    cfg: &os_model::CrawlConfig,
) -> Result<ScreenshotInfo, CmdError> {
    if cfg.rendering.mode != os_model::RenderingMode::JavaScript {
        return Err(
            CmdError::new(ErrorCode::Unsupported, "err-detail-unavailable")
                .with_arg("state", serde_json::json!({"code":"NEEDS_JS_RENDERING"})),
        );
    }
    let (width, height, tiles) = summary;
    if tiles == 0 {
        return Err(CmdError::new(
            ErrorCode::NotFound,
            "err-screenshot-not-stored",
        ));
    }
    Ok(ScreenshotInfo {
        url: uri("shot", key, id),
        width,
        height,
        tiles,
    })
}
pub fn confirm_live(app: &App, path: std::path::PathBuf, id: i64) -> String {
    let token = app
        .config()
        .ids
        .new_crawl_id(app.config().clock.now())
        .to_string();
    let grants = app.ext::<LiveGrants>();
    let mut grants = grants.0.lock().unwrap_or_else(|p| p.into_inner());
    if grants.len() >= 256 {
        grants.clear();
    }
    grants.insert(token.clone(), (path, id));
    token
}
fn image_response(mime: &str, body: Vec<u8>) -> AssetResponse {
    let mime = mime
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if mime == "image/svg+xml" {
        return match os_parse::images::safe_svg_preview(&body) {
            Some(body) => AssetResponse {
                status: 200,
                mime,
                body,
            },
            None => AssetResponse::not_found(),
        };
    }
    if !matches!(
        mime.as_str(),
        "image/png"
            | "image/jpeg"
            | "image/gif"
            | "image/webp"
            | "image/avif"
            | "image/x-icon"
            | "image/vnd.microsoft.icon"
    ) {
        return AssetResponse::not_found();
    }
    AssetResponse {
        status: 200,
        mime,
        body,
    }
}
/// No path from the URI is ever interpreted as a filesystem path. Only registered, open crawls are accessible.
pub async fn resolve(app: &App, uri: &str) -> AssetResponse {
    let Some(rest) = uri.strip_prefix("ospider://") else {
        return AssetResponse::not_found();
    };
    let parts = rest.split('/').collect::<Vec<_>>();
    if !(3..=4).contains(&parts.len()) || !matches!(parts[0], "shot" | "img") {
        return AssetResponse::not_found();
    }
    let Some(s) = app
        .sessions()
        .all()
        .into_iter()
        .find(|s| crawl_segment(&s.key) == parts[1])
    else {
        return AssetResponse::not_found();
    };
    let Ok(id) = parts[2].parse::<i64>() else {
        return AssetResponse::not_found();
    };
    if id <= 0 {
        return AssetResponse::not_found();
    }
    if parts[0] == "shot" {
        let tile = if parts.len() == 3 {
            0
        } else {
            match parts[3].parse::<u32>() {
                Ok(n) => n,
                Err(_) => return AssetResponse::not_found(),
            }
        };
        return app
            .read_pool()
            .run_snapshot(s.key, move |file, _| {
                let bytes = os_store::queries::detail_ext::screenshot_tile(file.conn(), id, tile)?;
                Ok(bytes
                    .map(|body| AssetResponse {
                        status: 200,
                        mime: "image/png".into(),
                        body,
                    })
                    .unwrap_or_else(AssetResponse::not_found))
            })
            .await
            .unwrap_or_else(|_| AssetResponse::not_found());
    }
    let stored = app
        .read_pool()
        .run_snapshot(s.key.clone(), move |file, _| {
            os_store::queries::detail_ext::stored_image(file, id)
        })
        .await;
    if let Ok(Some((mime, body))) = stored {
        return image_response(&mime, body);
    }
    let Some(token) = parts.get(3) else {
        return AssetResponse::not_found();
    };
    if !app
        .ext::<LiveGrants>()
        .0
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(*token)
        .is_some_and(|(path, url_id)| path == &s.path && *url_id == id)
    {
        return AssetResponse::not_found();
    }
    let address = app
        .read_pool()
        .run_snapshot(s.key, move |file, _| {
            os_store::queries::detail_ext::image_address(file.conn(), id)
        })
        .await;
    let Ok(Some(address)) = address else {
        return AssetResponse::not_found();
    };
    let Ok(url) = os_url::parse_start_url(&address) else {
        return AssetResponse::not_found();
    };
    let Ok(mut cfg) = os_fetch::FetchConfig::from_crawl_config(
        &s.config,
        &*app.config().secrets,
        app.config().clock.clone(),
    ) else {
        return AssetResponse::not_found();
    };
    cfg = cfg.with_network(&app.settings_snapshot().network);
    cfg.max_body_bytes = 20 * 1024 * 1024;
    let Ok(fetcher) = os_fetch::HttpFetcher::new(cfg) else {
        return AssetResponse::not_found();
    };
    use os_fetch::Fetch;
    let response = fetcher
        .fetch(os_fetch::FetchRequest {
            url: url.url,
            purpose: os_fetch::FetchPurpose::Resource,
            cookies: None,
            max_body_bytes: 20 * 1024 * 1024,
            body_limit: os_fetch::BodyLimit {
                max_decoded_bytes: 20 * 1024 * 1024,
                abort_media_after: None,
            },
            attempt: 0,
        })
        .await;
    match response.outcome {
        os_fetch::FetchOutcome::Response(r)
            if r.status == 200 && !r.truncated && r.decode_error.is_none() =>
        {
            image_response(
                r.headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
                    .map(|(_, v)| v.as_str())
                    .unwrap_or(""),
                r.body.to_vec(),
            )
        }
        _ => AssetResponse::not_found(),
    }
}
