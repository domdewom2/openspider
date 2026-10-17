use super::{
    capture::Capture, intercept::Intercept, protocol, Device, RenderError, RenderOpts,
    RenderedPage, ScreenshotFormat,
};
use base64::{prelude::BASE64_STANDARD, Engine};
use chromiumoxide::{
    cdp::{
        browser_protocol::{
            browser::{
                PermissionDescriptor, PermissionSetting, SetDownloadBehaviorBehavior,
                SetDownloadBehaviorParams, SetPermissionParams,
            },
            emulation::{SetDeviceMetricsOverrideParams, SetTouchEmulationEnabledParams},
            inspector::EventTargetCrashed,
            network::{
                Headers, SetBypassServiceWorkerParams, SetCookiesParams, SetExtraHttpHeadersParams,
            },
            page::{
                CaptureScreenshotFormat, CaptureScreenshotParams, EventLoadEventFired,
                NavigateParams, StopLoadingParams,
            },
            target::{CreateBrowserContextParams, CreateTargetParams},
        },
        js_protocol::runtime::TerminateExecutionParams,
    },
    Browser, Page,
};
use futures_util::StreamExt;
use os_model::{ImageRenderFacts, RenderTimings, ScreenshotTile};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

// A caller can cancel the render future without rendering another page. Stop
// JavaScript and close the target even then; generation retirement alone would
// leave an infinite script running until the next render or explicit close.
struct PageCleanup(Option<Page>);
impl Drop for PageCleanup {
    fn drop(&mut self) {
        if let Some(page) = self.0.take() {
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn(async move {
                    let _ = tokio::time::timeout(
                        Duration::from_millis(500),
                        page.execute(TerminateExecutionParams::default()),
                    )
                    .await;
                    let _ = tokio::time::timeout(Duration::from_secs(1), page.close()).await;
                });
            }
        }
    }
}

pub async fn render(
    browser: &Browser,
    url: &str,
    opts: &RenderOpts,
    launch: &super::LaunchOpts,
) -> Result<RenderedPage, RenderError> {
    // Per-render contexts isolate storage/cookies and prevent concurrent pages from
    // changing each other's identity. They are disposed even after hard timeouts.
    let context = tokio::time::timeout(
        Duration::from_secs(5),
        browser.create_browser_context(
            CreateBrowserContextParams::builder()
                .dispose_on_detach(true)
                .build(),
        ),
    )
    .await
    .map_err(|_| RenderError::Crashed("Browser antwortet nicht".into()))?
    .map_err(|e| RenderError::Crashed(e.to_string()))?;
    let budget = if opts.page_timeout_ms > 0 {
        opts.page_timeout_ms as u64
    } else {
        opts.response_timeout_ms as u64
            + opts.ajax_timeout_ms as u64
            + opts.screenshot.resize_wait_ms as u64
            + 5_000
    };
    let work = async {
        browser
            .execute(
                SetDownloadBehaviorParams::builder()
                    .behavior(SetDownloadBehaviorBehavior::Deny)
                    .browser_context_id(context.clone())
                    .events_enabled(true)
                    .build()
                    .map_err(protocol)?,
            )
            .await
            .map_err(protocol)?;
        for name in ["notifications", "geolocation"] {
            browser
                .execute(
                    SetPermissionParams::builder()
                        .permission(PermissionDescriptor::new(name))
                        .setting(PermissionSetting::Denied)
                        .browser_context_id(context.clone())
                        .build()
                        .map_err(protocol)?,
                )
                .await
                .map_err(protocol)?;
        }
        let page = browser
            .new_page(
                CreateTargetParams::builder()
                    .url("about:blank")
                    .browser_context_id(context.clone())
                    .build()
                    .map_err(protocol)?,
            )
            .await
            .map_err(|e| RenderError::Crashed(e.to_string()))?;
        let mut cleanup = PageCleanup(Some(page.clone()));
        let mut crashed = page
            .event_listener::<EventTargetCrashed>()
            .await
            .map_err(protocol)?;
        let result = tokio::select! {
            result = run(&page, browser.websocket_address(), url, opts, launch) => result,
            _ = crashed.next() => Err(RenderError::Crashed(url.into())),
        };
        if matches!(result, Err(RenderError::Timeout(_))) {
            let _ = page.execute(TerminateExecutionParams::default()).await;
        }
        if matches!(
            tokio::time::timeout(Duration::from_millis(300), page.close()).await,
            Ok(Ok(()))
        ) {
            cleanup.0 = None;
        }
        result
    };
    let result = tokio::time::timeout(Duration::from_millis(budget), work)
        .await
        .unwrap_or_else(|_| Err(RenderError::Timeout(url.into())));
    let _ = tokio::time::timeout(
        Duration::from_secs(1),
        browser.dispose_browser_context(context),
    )
    .await;
    result
}

async fn metrics(page: &Page, opts: &RenderOpts, size: (u32, u32)) -> Result<(), RenderError> {
    page.execute(
        SetDeviceMetricsOverrideParams::builder()
            .width(size.0)
            .height(size.1)
            .device_scale_factor(opts.device_scale_factor)
            .mobile(opts.mobile)
            .screen_width(size.0)
            .screen_height(size.1)
            .build()
            .map_err(protocol)?,
    )
    .await
    .map_err(protocol)?;
    page.execute(SetTouchEmulationEnabledParams::new(opts.touch))
        .await
        .map_err(protocol)?;
    Ok(())
}
async fn eval(page: &Page, expression: &str) -> Result<Value, RenderError> {
    page.evaluate(expression)
        .await
        .map_err(protocol)?
        .into_value()
        .map_err(protocol)
}
async fn document_height(page: &Page) -> Result<u32, RenderError> {
    Ok(eval(page, "Math.max(document.documentElement.scrollHeight, document.body ? document.body.scrollHeight : 0)").await?.as_u64().unwrap_or(1).clamp(1,8192) as u32)
}
async fn run(
    page: &Page,
    ws: &str,
    url: &str,
    opts: &RenderOpts,
    launch: &super::LaunchOpts,
) -> Result<RenderedPage, RenderError> {
    metrics(page, opts, opts.viewport).await?;
    if let Some(ua) = &opts.user_agent {
        page.set_user_agent(ua.clone()).await.map_err(protocol)?;
    }
    page.execute(SetExtraHttpHeadersParams::new(Headers::new(json!(
        opts.headers
    ))))
    .await
    .map_err(protocol)?;
    page.execute(SetBypassServiceWorkerParams::new(true))
        .await
        .map_err(protocol)?;
    // Also prevent registrations: bypass alone still permits worker side effects.
    page.evaluate_on_new_document("if ('serviceWorker' in navigator) { Object.defineProperty(ServiceWorkerContainer.prototype, 'register', {value: () => Promise.reject(new DOMException('Service workers disabled', 'NotAllowedError'))}); }").await.map_err(protocol)?;
    if !opts.cookies.is_empty() {
        let mut cookies = opts.cookies.clone();
        for cookie in &mut cookies {
            if cookie.url.is_none() && cookie.domain.is_none() {
                cookie.url = Some(url.into());
            }
        }
        page.execute(SetCookiesParams::new(cookies))
            .await
            .map_err(protocol)?;
    }
    let capture = Capture::start(page, opts.js_error_reporting).await?;
    let mut load = page
        .event_listener::<EventLoadEventFired>()
        .await
        .map_err(protocol)?;
    let intercept = Intercept::start(ws, page, url, opts, launch, capture.data.clone()).await?;
    capture.begin_navigation();
    let start = Instant::now();
    // Use raw Page.navigate: Page.goto waits for chromiumoxide's navigation/idle
    // heuristic, which is not the load event required by CRAWL-080.
    let navigation = async {
        match page.execute(NavigateParams::new(url)).await {
            Ok(nav) => {
                if let Some(error) = nav.result.error_text {
                    return Err(protocol(error));
                }
            }
            // chromiumoxide 0.9's CommandFuture has a fixed 30-second timer.
            // The navigation continues in Chrome; our response timeout owns the
            // deadline, including configured values above 30 seconds.
            Err(chromiumoxide::error::CdpError::Timeout) => {}
            Err(error) => return Err(protocol(error)),
        }
        load.next()
            .await
            .ok_or_else(|| RenderError::Crashed("Tab geschlossen".into()))?;
        Ok::<_, RenderError>(())
    };
    let loaded = match tokio::time::timeout(
        Duration::from_millis(opts.response_timeout_ms.into()),
        navigation,
    )
    .await
    {
        Ok(result) => {
            if capture.data.lock().expect("capture").proxy_auth_failed {
                return Err(RenderError::ProxyAuthentication);
            }
            result?;
            true
        }
        Err(_) => false,
    };
    let load_ms = start.elapsed().as_millis() as u32;
    let deadline = Instant::now() + Duration::from_millis(opts.ajax_timeout_ms.into());
    capture.data.lock().expect("capture").console_until_ms =
        Some(load_ms as u64 + opts.ajax_timeout_ms as u64);
    if loaded && opts.device != Device::Fixed {
        let height = document_height(page).await?;
        metrics(page, opts, (opts.viewport.0, height)).await?;
    }
    tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;
    if !loaded {
        page.execute(StopLoadingParams::default())
            .await
            .map_err(protocol)?;
    }
    let dom_html = page.content().await.map_err(protocol)?;
    let final_url = eval(page, "location.href")
        .await?
        .as_str()
        .unwrap_or(url)
        .to_owned();
    let image_values = eval(page, "Array.from(document.images, img => { const r=img.getBoundingClientRect(); return {url:img.currentSrc || img.src,w:img.naturalWidth,h:img.naturalHeight,dw:Math.round(r.width),dh:Math.round(r.height),visible:r.bottom>0 && r.right>0 && r.top<innerHeight && r.left<innerWidth}; })").await?;
    let mut images: Vec<_> = image_values
        .as_array()
        .into_iter()
        .flatten()
        .map(|v| {
            (
                v["url"].as_str().unwrap_or_default().to_owned(),
                ImageRenderFacts {
                    real_w: v["w"].as_u64().filter(|n| *n > 0).map(|v| v as u32),
                    real_h: v["h"].as_u64().filter(|n| *n > 0).map(|v| v as u32),
                    display_w: v["dw"].as_u64().map(|v| v as u32),
                    display_h: v["dh"].as_u64().map(|v| v as u32),
                    in_viewport: v["visible"].as_bool(),
                    bytes: None,
                },
            )
        })
        .collect();
    let mut screenshot_tiles = Vec::new();
    if opts.screenshots {
        let mut size = opts.screenshot.size.unwrap_or(opts.viewport);
        // Establish width before measuring height (responsive layout).
        metrics(page, opts, size).await?;
        if opts.screenshot.fit_content {
            size.1 = document_height(page).await?;
            metrics(page, opts, size).await?;
        }
        tokio::time::sleep(Duration::from_millis(opts.screenshot.resize_wait_ms.into())).await;
        let mut params = CaptureScreenshotParams::builder()
            .format(match opts.screenshot.format {
                ScreenshotFormat::Png => CaptureScreenshotFormat::Png,
                ScreenshotFormat::Jpeg => CaptureScreenshotFormat::Jpeg,
            })
            .capture_beyond_viewport(false)
            .from_surface(true);
        if opts.screenshot.format == ScreenshotFormat::Jpeg {
            params = params.quality(opts.screenshot.jpeg_quality);
        }
        let result = page.execute(params.build()).await.map_err(protocol)?;
        let bytes = BASE64_STANDARD
            .decode(AsRef::<[u8]>::as_ref(&result.result.data))
            .map_err(protocol)?;
        screenshot_tiles.push(ScreenshotTile {
            tile: 0,
            data: bytes,
            width: (size.0 as f64 * opts.device_scale_factor).round() as u32,
            height: (size.1 as f64 * opts.device_scale_factor).round() as u32,
        });
    }
    drop(intercept);
    let mut data = capture.data.lock().expect("capture");
    data.console.sort_by_key(|e| e.timestamp_ms);
    for (url, facts) in &mut images {
        facts.bytes = data
            .network
            .iter()
            .rev()
            .find(|r| &r.url == url && r.status > 0)
            .map(|r| r.bytes);
    }
    Ok(RenderedPage {
        dom_html,
        final_url,
        console: std::mem::take(&mut data.console),
        console_dropped: data.dropped,
        blocked: std::mem::take(&mut data.blocked),
        network: std::mem::take(&mut data.network),
        screenshot_tiles,
        timings: RenderTimings {
            load_ms,
            wait_ms: opts.ajax_timeout_ms,
            total_ms: start.elapsed().as_millis() as u32,
        },
        load_timed_out: !loaded,
        images,
    })
}
