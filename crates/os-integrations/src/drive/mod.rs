//! Drive/Sheets transport for OUT-005 and OUT-183. Only drive.file is requested.
//! Values use RAW; requests never follow redirects with bearer credentials.
mod auth;
pub use auth::{connect, connect_in_browser, refresh_account};

use crate::{credentials::read_account, http, ApiCtx, ApiError};
use os_model::{ApiKind, Timestamp};
use reqwest::Method;
use serde_json::{json, Value};
use std::time::Duration;

pub const SCOPE: &str = "https://www.googleapis.com/auth/drive.file";
pub const FOLDER: &str = "application/vnd.google-apps.folder";
pub const SPREADSHEET: &str = "application/vnd.google-apps.spreadsheet";
pub const RECONNECT: &str = "Google-Konto erneut verbinden";

#[derive(Clone)]
pub struct DriveClient {
    client: reqwest::Client,
    token: String,
    drive: String,
    sheets: String,
    timeout: Duration,
}
impl DriveClient {
    pub fn new(ctx: &ApiCtx, account: &str, now: Timestamp) -> Result<Self, ApiError> {
        let credential = read_account(ctx.secrets.as_ref(), ApiKind::Drive, Some(account))
            .map_err(|_| reconnect())?;
        if !credential.scopes.iter().any(|s| s == SCOPE)
            || credential.expires_at.is_some_and(|expiry| expiry <= now)
        {
            return Err(reconnect());
        }
        let token = credential
            .access_token
            .filter(|t| !t.is_empty())
            .ok_or_else(reconnect)?;
        let drive = http::base_url(
            ApiKind::Drive,
            ctx.cfg.api.drive.base_url.as_deref(),
            "https://www.googleapis.com",
            ctx.allow_api_override,
        );
        let sheets = if drive == "https://www.googleapis.com" {
            "https://sheets.googleapis.com".into()
        } else {
            drive.clone()
        };
        Ok(Self {
            client: http::client_for(ctx)?,
            token,
            drive,
            sheets,
            timeout: Duration::from_secs(30),
        })
    }
    /// Request timeout override, useful for deterministic local timeout fixtures.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
    fn url(&self, sheets: bool, parts: &[&str]) -> Result<url::Url, ApiError> {
        let mut url = url::Url::parse(if sheets { &self.sheets } else { &self.drive })
            .map_err(|_| protocol())?;
        url.path_segments_mut()
            .map_err(|_| protocol())?
            .pop_if_empty()
            .extend(parts);
        Ok(url)
    }
    /// Exactly five retries: 1, 2, 4, 8, 16 seconds. PUTs address fixed ranges,
    /// including time-series rows, so an ambiguous timeout cannot append twice.
    async fn request(
        &self,
        method: Method,
        url: url::Url,
        body: Option<Value>,
    ) -> Result<Value, ApiError> {
        self.request_limited(method, url, body, 5).await
    }
    async fn request_limited(
        &self,
        method: Method,
        url: url::Url,
        body: Option<Value>,
        retries: u32,
    ) -> Result<Value, ApiError> {
        for attempt in 0..=retries {
            let mut request = self
                .client
                .request(method.clone(), url.clone())
                .bearer_auth(&self.token)
                .timeout(self.timeout);
            if let Some(body) = &body {
                request = request
                    .header("Content-Type", "application/json")
                    .body(serde_json::to_vec(body).map_err(|_| protocol())?);
            }
            let error = match request.send().await {
                Ok(response) => {
                    let status = response.status();
                    if matches!(status.as_u16(), 401 | 403) {
                        return Err(reconnect());
                    }
                    if status.is_success() {
                        match response.bytes().await {
                            Ok(bytes) if bytes.is_empty() => return Ok(json!({})),
                            Ok(bytes) => {
                                return serde_json::from_slice(&bytes).map_err(|_| protocol())
                            }
                            Err(e) if e.is_timeout() => timeout(),
                            Err(_) => return Err(protocol()),
                        }
                    } else {
                        let error = ApiError::Provider {
                            status: status.as_u16(),
                            message: "Drive/Sheets-Anfrage fehlgeschlagen".into(),
                        };
                        if status.as_u16() != 429 && !status.is_server_error() {
                            return Err(error);
                        }
                        error
                    }
                }
                Err(e) if e.is_timeout() => timeout(),
                Err(_) => {
                    return Err(ApiError::Transport(
                        "Drive/Sheets-Verbindung fehlgeschlagen".into(),
                    ))
                }
            };
            if attempt == retries {
                return Err(error);
            }
            tokio::time::sleep(Duration::from_secs(1 << attempt)).await;
        }
        unreachable!()
    }
    async fn find(&self, q: &str) -> Result<Option<String>, ApiError> {
        let mut page = String::new();
        loop {
            let mut url = self.url(false, &["drive", "v3", "files"])?;
            url.query_pairs_mut().extend_pairs([
                ("q", q),
                ("fields", "files(id),nextPageToken"),
                ("orderBy", "createdTime"),
                ("pageToken", page.as_str()),
            ]);
            let result = self.request(Method::GET, url, None).await?;
            if let Some(id) = result.pointer("/files/0/id").and_then(Value::as_str) {
                return Ok(Some(id.into()));
            }
            match result["nextPageToken"].as_str() {
                Some(next) if !next.is_empty() => page = next.into(),
                _ => return Ok(None),
            }
        }
    }
    pub async fn find_or_create(
        &self,
        parent: &str,
        name: &str,
        mime: &str,
    ) -> Result<String, ApiError> {
        if name.trim().is_empty() {
            return Err(protocol());
        }
        let escape = |s: &str| s.replace('\\', "\\\\").replace('\'', "\\'");
        let q = format!(
            "trashed = false and '{}' in parents and name = '{}' and mimeType = '{}'",
            escape(parent),
            escape(name),
            escape(mime)
        );
        for attempt in 0..=5 {
            // Re-query after ambiguous create timeouts before retrying POST. Drive
            // native spreadsheets cannot be created with caller-assigned file IDs.
            if let Some(id) = self.find(&q).await? {
                return Ok(id);
            }
            let result = self
                .request_limited(
                    Method::POST,
                    self.url(false, &["drive", "v3", "files"])?,
                    Some(json!({"name":name,"mimeType":mime,"parents":[parent]})),
                    0,
                )
                .await;
            match result {
                Ok(value) => {
                    return value["id"]
                        .as_str()
                        .map(str::to_string)
                        .ok_or_else(protocol)
                }
                Err(error) if attempt < 5 && retryable(&error) => {
                    tokio::time::sleep(Duration::from_secs(1 << attempt)).await
                }
                Err(error) => return Err(error),
            }
        }
        unreachable!()
    }
    pub async fn folder(
        &self,
        root: &str,
        project: &str,
        crawl: Option<&str>,
    ) -> Result<String, ApiError> {
        let root = self.find_or_create("root", root, FOLDER).await?;
        let project = self.find_or_create(&root, project, FOLDER).await?;
        match crawl {
            Some(name) => self.find_or_create(&project, name, FOLDER).await,
            None => Ok(project),
        }
    }
    pub async fn sheets(&self, id: &str) -> Result<Vec<Sheet>, ApiError> {
        let result = self
            .request(
                Method::GET,
                self.url(true, &["v4", "spreadsheets", id])?,
                None,
            )
            .await?;
        result["sheets"]
            .as_array()
            .ok_or_else(protocol)?
            .iter()
            .map(|s| {
                Ok(Sheet {
                    id: s["properties"]["sheetId"].as_i64().ok_or_else(protocol)?,
                    title: s["properties"]["title"]
                        .as_str()
                        .ok_or_else(protocol)?
                        .into(),
                })
            })
            .collect()
    }
    pub async fn batch(&self, id: &str, requests: Vec<Value>) -> Result<(), ApiError> {
        let result = self
            .request(
                Method::POST,
                self.url(true, &["v4", "spreadsheets", &format!("{id}:batchUpdate")])?,
                Some(json!({"requests":&requests})),
            )
            .await;
        if let Err(ApiError::Provider { status: 400, .. }) = &result {
            // A committed structural batch followed by a timeout can fail on
            // retry because its add/delete IDs already changed. Sheets batches
            // are atomic: verify those structural postconditions before accepting.
            let sheets = self.sheets(id).await?;
            let mut structural = false;
            let mut applied = true;
            for request in &requests {
                if let Some(add) = request.pointer("/addSheet/properties") {
                    structural = true;
                    applied &= sheets
                        .iter()
                        .any(|s| json!(s.id) == add["sheetId"] && json!(s.title) == add["title"]);
                }
                if let Some(delete) = request.get("deleteSheet") {
                    structural = true;
                    applied &= !sheets.iter().any(|s| json!(s.id) == delete["sheetId"]);
                }
            }
            if structural && applied {
                return Ok(());
            }
        }
        result.map(|_| ())
    }
    pub async fn values(&self, id: &str, range: &str) -> Result<Vec<Vec<Value>>, ApiError> {
        let mut url = self.url(true, &["v4", "spreadsheets", id, "values", range])?;
        url.query_pairs_mut()
            .append_pair("valueRenderOption", "UNFORMATTED_VALUE");
        let result = self.request(Method::GET, url, None).await?;
        serde_json::from_value(result.get("values").cloned().unwrap_or_else(|| json!([])))
            .map_err(|_| protocol())
    }
    pub async fn write(
        &self,
        id: &str,
        range: &str,
        values: &[Vec<Value>],
    ) -> Result<(), ApiError> {
        if values.len() > 10_000 {
            return Err(protocol());
        }
        let mut url = self.url(true, &["v4", "spreadsheets", id, "values", range])?;
        url.query_pairs_mut().append_pair("valueInputOption", "RAW");
        self.request(
            Method::PUT,
            url,
            Some(json!({"majorDimension": "ROWS", "values": values})),
        )
        .await?;
        Ok(())
    }
    /// All cells, all worksheets, in worksheet/row/cell order. No deduplication:
    /// the engine's existing list planner retains upload positions and duplicates.
    pub async fn list_items(&self, sheet_url: &str) -> Result<Vec<String>, ApiError> {
        let id = spreadsheet_id(sheet_url)?;
        let mut items = Vec::new();
        for sheet in self.sheets(&id).await? {
            for row in self.values(&id, &quote_sheet(&sheet.title)).await? {
                for cell in row {
                    match cell {
                        Value::String(s) if !s.trim().is_empty() => items.push(s),
                        Value::Null | Value::String(_) => {}
                        other => items.push(other.to_string()),
                    }
                }
            }
        }
        Ok(items)
    }
}
#[derive(Debug, Clone)]
pub struct Sheet {
    pub id: i64,
    pub title: String,
}
pub fn link(id: &str) -> String {
    format!("https://docs.google.com/spreadsheets/d/{id}/edit")
}
pub fn quote_sheet(name: &str) -> String {
    format!("'{}'", name.replace('\'', "''"))
}
pub fn spreadsheet_id(input: &str) -> Result<String, ApiError> {
    let url = url::Url::parse(input).map_err(|_| protocol())?;
    if url.scheme() != "https" || url.host_str() != Some("docs.google.com") {
        return Err(protocol());
    }
    let parts: Vec<_> = url.path_segments().ok_or_else(protocol)?.collect();
    if parts.first() != Some(&"spreadsheets") || parts.get(1) != Some(&"d") {
        return Err(protocol());
    }
    parts
        .get(2)
        .filter(|s| {
            !s.is_empty()
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
        .map(|s| (*s).into())
        .ok_or_else(protocol)
}
fn reconnect() -> ApiError {
    ApiError::Unauthorized(RECONNECT.into())
}
fn protocol() -> ApiError {
    ApiError::Transport("Ungültige Drive/Sheets-Daten".into())
}

fn timeout() -> ApiError {
    ApiError::Transport("Drive/Sheets-Zeitüberschreitung".into())
}
fn retryable(error: &ApiError) -> bool {
    matches!(
        error,
        ApiError::Provider {
            status: 429 | 500..=599,
            ..
        }
    ) || matches!(error,ApiError::Transport(message) if message=="Drive/Sheets-Zeitüberschreitung")
}
