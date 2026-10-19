//! Lokaler Ahrefs-v3-/PKCE-Vertrag, ausschließlich synthetische Werte.
use super::{common, MockApi};
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Form, Json,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
#[derive(Clone, Debug)]
pub struct Request {
    pub at: tokio::time::Instant,
    pub body: Value,
}
#[derive(Clone, Default)]
pub struct Requests(Arc<Mutex<Vec<Request>>>);
impl Requests {
    pub fn record(&self, body: Value) {
        self.0.lock().unwrap().push(Request {
            at: tokio::time::Instant::now(),
            body,
        });
    }
    pub fn all(&self) -> Vec<Request> {
        self.0.lock().unwrap().clone()
    }
}
#[derive(Default, Clone)]
pub struct AhrefsMock {
    pub requests: Requests,
    pub log: common::CallLog,
    pub unit_limit: Option<u64>,
    challenges: Arc<Mutex<HashMap<String, String>>>,
}
impl AhrefsMock {
    pub fn with_unit_limit(unit_limit: u64) -> Self {
        Self {
            unit_limit: Some(unit_limit),
            ..Self::default()
        }
    }
}
impl MockApi for AhrefsMock {
    fn name(&self) -> &'static str {
        "ahrefs"
    }
    fn base_url_key(&self) -> Option<&'static str> {
        Some("api.ahrefs.base_url")
    }
    fn scenario_path(&self) -> &'static str {
        "/v3/subscription-info/limits-and-usage"
    }
    fn router(&self) -> axum::Router {
        axum::Router::new()
            .route("/authorize", get(authorize))
            .route("/token", post(token))
            .route("/v3/subscription-info/limits-and-usage", get(usage))
            .route("/v3/batch-analysis/batch-analysis", post(metrics))
            .with_state(self.clone())
    }
}
pub fn mock() -> Option<Arc<dyn MockApi>> {
    Some(Arc::new(AhrefsMock::default()))
}
async fn authorize(
    State(s): State<AhrefsMock>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    if q.get("scope").map(String::as_str) != Some("apiv3-integration-apps")
        || q.get("code_challenge_method").map(String::as_str) != Some("s256")
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let code = q.get("state").cloned().unwrap_or_default();
    s.challenges.lock().unwrap().insert(
        code.clone(),
        q.get("code_challenge").cloned().unwrap_or_default(),
    );
    let Some(redirect) = q.get("redirect_uri") else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(parsed) = redirect.parse::<axum::http::Uri>() else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if parsed.host() != Some("127.0.0.1")
        || !code
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    Redirect::temporary(&format!("{redirect}?code={code}&state={code}")).into_response()
}
async fn token(State(s): State<AhrefsMock>, Form(q): Form<HashMap<String, String>>) -> Response {
    let expected = q
        .get("code")
        .and_then(|code| s.challenges.lock().unwrap().remove(code));
    let valid = expected.is_some_and(|challenge| {
        q.get("code_verifier")
            .is_some_and(|v| common::pkce_challenge(v) == challenge)
    });
    if !valid || q.contains_key("client_secret") {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    Json(json!({"access_token":"ahrefs-fixture-token","expires_in":31536000,"scope":"apiv3-integration-apps","account":"Fixture Workspace","account_id":"workspace-1"})).into_response()
}
async fn usage(
    State(s): State<AhrefsMock>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    s.log.record("/v3/subscription-info/limits-and-usage");
    if let Some(response) = common::scenario_response(&common::scenario(&headers, &q), 0).await {
        return response;
    }
    if !headers.contains_key("X-Scenario")
        && headers
            .get("Authorization")
            .is_none_or(|v| v == "Bearer bad")
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    Json(json!({"limits_and_usage":{"units_usage_api_key":200,"units_limit_api_key":s.unit_limit.unwrap_or(10000),"units_usage_workspace":500,"units_limit_workspace":20000}})).into_response()
}
async fn metrics(
    State(s): State<AhrefsMock>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    if headers
        .get("Authorization")
        .is_none_or(|v| v == "Bearer bad")
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    s.requests.record(body.clone());
    let Some(targets) = body["targets"].as_array() else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if targets.is_empty() || targets.len() > 100 {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let rows: Vec<_> = targets
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let mut row =
                json!({"index":i,"url":t["url"],"mode":t["mode"],"protocol":t["protocol"]});
            for field in body["select"].as_array().unwrap() {
                let field = field.as_str().unwrap();
                row[field] = json!(match field {
                    "backlinks" => 5_000_000_000_i64,
                    "org_traffic"
                        if body["country"] == "de" && body["volume_mode"] == "average" =>
                        950,
                    "org_traffic" if body["country"] == "de" => 1200,
                    "org_traffic" => 2000,
                    _ => 42,
                });
            }
            row
        })
        .collect();
    (
        [("x-api-units-cost-total-actual", "50")],
        Json(json!({"targets":rows})),
    )
        .into_response()
}
