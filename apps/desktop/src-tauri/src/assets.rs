//! URI-Schema `ospider://` (`ospider://shot/<crawl>/<urlId>/<tile>`, `ospider://img/…`).
//!
//! Die Anfragen gehen an `App::resolve_asset`; der Auflöser (Screenshots, Bilder) kommt mit M2-K09 und trägt sich über
//! `App::set_asset_resolver` ein. Bis dahin antwortet das Schema mit 404. Die Antworten sind nie cachebar über
//! Sitzungsgrenzen hinaus (`Cache-Control: no-store`), weil die Quelle eine Crawl-Datei ist, die sich ändern kann.

use os_app::App;
use tauri::http::{header, Response};
use tauri::{Runtime, UriSchemeContext, UriSchemeResponder};

/// Name des Schemas.
pub const SCHEME: &str = "ospider";

/// Handler für `register_asynchronous_uri_scheme_protocol`.
pub fn handle<R: Runtime>(
    app: &App,
    _ctx: UriSchemeContext<'_, R>,
    request: tauri::http::Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let uri = request.uri().to_string();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let asset = app.resolve_asset(&uri).await;
        let response = Response::builder()
            .status(asset.status)
            .header(header::CONTENT_TYPE, asset.mime)
            .header(header::CACHE_CONTROL, "no-store")
            .body(asset.body)
            .unwrap_or_else(|_| Response::new(Vec::new()));
        responder.respond(response);
    });
}
