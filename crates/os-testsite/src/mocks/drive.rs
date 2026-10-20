//! Stateful, local Drive/Sheets fixture. No Google traffic or real credentials.
use super::MockApi;
use axum::{
    body::Bytes,
    extract::{OriginalUri, State},
    http::{Method, StatusCode},
    response::{IntoResponse, Response},
    routing::any,
    Json, Router,
};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Clone, Default)]
pub struct DriveMock(Arc<Mutex<Fixture>>);
#[derive(Default)]
struct Fixture {
    files: BTreeMap<String, File>,
    next: usize,
    requests: Vec<Request>,
    failures: VecDeque<Failure>,
}
#[derive(Clone, Debug)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub query: BTreeMap<String, String>,
    pub body: Value,
}
#[derive(Clone, Debug)]
pub struct File {
    pub id: String,
    pub name: String,
    pub parent: String,
    pub mime: String,
    pub trashed: bool,
    pub sheets: Vec<Sheet>,
}
#[derive(Clone, Debug)]
pub struct Sheet {
    pub id: i64,
    pub title: String,
    pub rows: Vec<Vec<Value>>,
    pub date_format: Option<String>,
}
struct Failure {
    method: String,
    path: String,
    status: u16,
    delay: Duration,
    after: bool,
}
impl DriveMock {
    pub fn requests(&self) -> Vec<Request> {
        self.0.lock().unwrap().requests.clone()
    }
    pub fn files(&self) -> Vec<File> {
        self.0.lock().unwrap().files.values().cloned().collect()
    }
    pub fn trash(&self, id: &str) {
        self.0.lock().unwrap().files.get_mut(id).unwrap().trashed = true;
    }
    pub fn delete(&self, id: &str) {
        self.0.lock().unwrap().files.remove(id);
    }
    pub fn seed(&self, name: &str, parent: &str, mime: &str, sheets: Vec<Sheet>) -> String {
        let mut fixture = self.0.lock().unwrap();
        fixture.next += 1;
        let id = format!("file-{}", fixture.next);
        fixture.files.insert(
            id.clone(),
            File {
                id: id.clone(),
                name: name.into(),
                parent: parent.into(),
                mime: mime.into(),
                trashed: false,
                sheets,
            },
        );
        id
    }
    pub fn fail_next(&self, method: &str, path: &str, status: u16) {
        self.0.lock().unwrap().failures.push_back(Failure {
            method: method.into(),
            path: path.into(),
            status,
            delay: Duration::ZERO,
            after: false,
        });
    }
    /// Simulates an ambiguous timeout: persist the write, then delay the reply.
    pub fn timeout_next_write(&self, delay: Duration) {
        self.timeout_next("PUT", "/values/", delay);
    }
    pub fn timeout_next(&self, method: &str, path: &str, delay: Duration) {
        self.0.lock().unwrap().failures.push_back(Failure {
            method: method.into(),
            path: path.into(),
            status: 200,
            delay,
            after: true,
        });
    }
}
impl MockApi for DriveMock {
    fn name(&self) -> &'static str {
        "drive"
    }
    fn base_url_key(&self) -> Option<&'static str> {
        Some("api.drive.base_url")
    }
    fn router(&self) -> Router {
        Router::new().fallback(any(handle)).with_state(self.clone())
    }
    fn scenario_path(&self) -> &'static str {
        "/drive/v3/files"
    }
}
pub fn mock() -> Option<Arc<dyn MockApi>> {
    Some(Arc::new(DriveMock::default()))
}
async fn handle(
    State(mock): State<DriveMock>,
    OriginalUri(uri): OriginalUri,
    method: Method,
    headers: axum::http::HeaderMap,
    bytes: Bytes,
) -> Response {
    let path = uri
        .path()
        .strip_prefix("/drive")
        .unwrap_or(uri.path())
        .to_string();
    // Google path segments may contain percent-encoded A1 ranges.
    let path = percent_decode(&path);
    let query: BTreeMap<String, String> = uri
        .query()
        .unwrap_or_default()
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| {
            (
                percent_decode(&k.replace('+', " ")),
                percent_decode(&v.replace('+', " ")),
            )
        })
        .collect();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    let request = Request {
        method: method.to_string(),
        path,
        query,
        body,
    };
    let (failure, value) = {
        let mut f = mock.0.lock().unwrap();
        f.requests.push(request.clone());
        let scenario = headers
            .get("X-Scenario")
            .and_then(|s| s.to_str().ok())
            .unwrap_or_default();
        if matches!(scenario, "unauthorized" | "401") {
            return StatusCode::UNAUTHORIZED.into_response();
        }
        if matches!(scenario, "429" | "rate-limit") {
            return StatusCode::TOO_MANY_REQUESTS.into_response();
        }
        let failure = if f
            .failures
            .front()
            .is_some_and(|e| e.method == request.method && request.path.contains(&e.path))
        {
            f.failures.pop_front()
        } else {
            None
        };
        let value = if failure.as_ref().is_none_or(|e| e.after) {
            execute(&mut f, &request)
        } else {
            Ok(json!({}))
        };
        (failure, value)
    };
    if let Some(failure) = failure {
        tokio::time::sleep(failure.delay).await;
        if failure.status != 200 {
            return (
                StatusCode::from_u16(failure.status).unwrap(),
                Json(json!({"error":"fixture"})),
            )
                .into_response();
        }
    }
    match value {
        Ok(value) => Json(value).into_response(),
        Err(code) => code.into_response(),
    }
}
fn execute(f: &mut Fixture, r: &Request) -> Result<Value, StatusCode> {
    if r.path.ends_with("/drive/v3/files") {
        if r.method == "GET" {
            let q = r.query.get("q").cloned().unwrap_or_default();
            let quoted = quoted_values(&q);
            let files: Vec<_> = f
                .files
                .values()
                .filter(|file| {
                    !file.trashed
                        && (q.is_empty()
                            || (quoted.first() == Some(&file.parent)
                                && quoted.get(1) == Some(&file.name)
                                && quoted.get(2) == Some(&file.mime)))
                })
                .map(|file| json!({"id":file.id}))
                .collect();
            return Ok(json!({"files":files}));
        }
        f.next += 1;
        let id = format!("file-{}", f.next);
        f.files.insert(
            id.clone(),
            File {
                id: id.clone(),
                name: r.body["name"].as_str().unwrap_or_default().into(),
                parent: r.body["parents"][0].as_str().unwrap_or_default().into(),
                mime: r.body["mimeType"].as_str().unwrap_or_default().into(),
                trashed: false,
                sheets: if r.body["mimeType"] == "application/vnd.google-apps.spreadsheet" {
                    vec![Sheet {
                        id: 0,
                        title: "Sheet1".into(),
                        rows: vec![],
                        date_format: None,
                    }]
                } else {
                    vec![]
                },
            },
        );
        return Ok(json!({"id":id}));
    }
    let suffix = r
        .path
        .split("/v4/spreadsheets/")
        .nth(1)
        .ok_or(StatusCode::NOT_FOUND)?;
    let id = suffix.split(['/', ':']).next().unwrap();
    let file = f
        .files
        .get_mut(id)
        .filter(|file| !file.trashed)
        .ok_or(StatusCode::NOT_FOUND)?;
    if suffix.ends_with(":batchUpdate") {
        let mut changed = file.clone();
        for update in r.body["requests"]
            .as_array()
            .ok_or(StatusCode::BAD_REQUEST)?
        {
            if let Some(p) = update.get("addSheet") {
                let p = &p["properties"];
                let id = p["sheetId"].as_i64().ok_or(StatusCode::BAD_REQUEST)?;
                if changed.sheets.iter().any(|s| s.id == id) {
                    return Err(StatusCode::BAD_REQUEST);
                }
                {
                    changed.sheets.push(Sheet {
                        id,
                        title: p["title"].as_str().unwrap().into(),
                        rows: vec![],
                        date_format: None,
                    });
                }
            }
            if let Some(p) = update.get("deleteSheet") {
                if !changed.sheets.iter().any(|s| s.id == p["sheetId"]) {
                    return Err(StatusCode::BAD_REQUEST);
                }
                changed.sheets.retain(|s| s.id != p["sheetId"]);
            }
            if let Some(p) = update.get("updateSheetProperties") {
                let p = &p["properties"];
                if let Some(sheet) = changed.sheets.iter_mut().find(|s| s.id == p["sheetId"]) {
                    if let Some(title) = p["title"].as_str() {
                        sheet.title = title.into();
                    }
                }
            }
            if let Some(p) = update.get("updateCells") {
                if let Some(sheet) = changed
                    .sheets
                    .iter_mut()
                    .find(|s| s.id == p["range"]["sheetId"])
                {
                    sheet.rows.clear();
                }
            }
            if let Some(p) = update.get("repeatCell") {
                if let Some(sheet) = changed
                    .sheets
                    .iter_mut()
                    .find(|s| s.id == p["range"]["sheetId"])
                {
                    sheet.date_format = p
                        .pointer("/cell/userEnteredFormat/numberFormat/pattern")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                }
            }
        }
        *file = changed;
        return Ok(json!({"replies":[]}));
    }
    if let Some(range) = suffix.split("/values/").nth(1) {
        let (title, cells) = range.rsplit_once('!').unwrap_or((range, "A1"));
        let title = title.trim_matches('\'').replace("''", "'");
        let sheet = file
            .sheets
            .iter_mut()
            .find(|s| s.title == title)
            .ok_or(StatusCode::BAD_REQUEST)?;
        if r.method == "GET" {
            return Ok(json!({"values":sheet.rows}));
        }
        if r.query.get("valueInputOption").map(String::as_str) != Some("RAW") {
            return Err(StatusCode::BAD_REQUEST);
        }
        let row = cells
            .chars()
            .skip_while(|c| c.is_ascii_alphabetic())
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse::<usize>()
            .unwrap_or(1)
            - 1;
        for (n, values) in r.body["values"]
            .as_array()
            .ok_or(StatusCode::BAD_REQUEST)?
            .iter()
            .enumerate()
        {
            if sheet.rows.len() <= row + n {
                sheet.rows.resize(row + n + 1, vec![]);
            }
            sheet.rows[row + n] = values.as_array().ok_or(StatusCode::BAD_REQUEST)?.clone();
        }
        return Ok(json!({"updatedRows":r.body["values"].as_array().unwrap().len()}));
    }
    Ok(
        json!({"sheets":file.sheets.iter().map(|s|json!({"properties":{"sheetId":s.id,"title":s.title}})).collect::<Vec<_>>()}),
    )
}
fn quoted_values(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = None;
    let mut escaped = false;
    for c in s.chars() {
        if escaped {
            if let Some(v) = &mut current {
                String::push(v, c);
            }
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '\'' {
            if let Some(v) = current.take() {
                out.push(v);
            } else {
                current = Some(String::new());
            }
        } else if let Some(v) = &mut current {
            v.push(c);
        }
    }
    out
}
fn percent_decode(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(n) = u8::from_str_radix(&path[i + 1..i + 3], 16) {
                out.push(n);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
