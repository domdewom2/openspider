//! Lokaler OAuth-Protokollmock mit einmaligen Codes, PKCE-Prüfung und Widerruf.
use super::{
    common::{scenario, scenario_response, CallLog},
    MockApi,
};
use axum::{
    extract::{Form, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Default)]
pub struct OAuthMock {
    pub log: CallLog,
    codes: Mutex<HashMap<String, (String, String, String)>>,
    attempts: Mutex<HashMap<String, usize>>,
}
impl MockApi for OAuthMock {
    fn name(&self) -> &'static str {
        "oauth"
    }
    fn base_url_key(&self) -> Option<&'static str> {
        None
    }
    fn router(&self) -> Router {
        Router::new()
            .route("/authorize", get(authorize))
            .route("/token", post(token))
            .route("/revoke", post(revoke))
            .with_state(Arc::new(Self {
                log: self.log.clone(),
                ..Self::default()
            }))
    }
}
pub fn mock() -> Option<Arc<dyn MockApi>> {
    Some(Arc::new(OAuthMock::default()))
}
async fn authorize(
    State(s): State<Arc<OAuthMock>>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    s.log.record("authorize");
    let Some(redirect) = q.get("redirect_uri") else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(parsed) = redirect.parse::<axum::http::Uri>() else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if parsed.host() != Some("127.0.0.1")
        || q.get("code_challenge_method").map(String::as_str) != Some("S256")
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let mut codes = s.codes.lock().unwrap_or_else(|p| p.into_inner());
    let code = format!("fixture-code-{}", s.log.calls().len());
    codes.insert(
        code.clone(),
        (
            q.get("code_challenge").cloned().unwrap_or_default(),
            q.get("scope").cloned().unwrap_or_default(),
            q.get("client_id").cloned().unwrap_or_default(),
        ),
    );
    let target = format!(
        "{redirect}?code={code}&state={}",
        q.get("state").map(String::as_str).unwrap_or_default()
    );
    Redirect::temporary(&target).into_response()
}
async fn token(
    State(s): State<Arc<OAuthMock>>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    s.log.record("token");
    let scenario = scenario(&headers, &q);
    let attempt = {
        let mut a = s.attempts.lock().unwrap_or_else(|p| p.into_inner());
        let count = a.entry(scenario.clone()).or_default();
        let n = *count;
        *count += 1;
        n
    };
    if let Some(response) = scenario_response(&scenario, attempt).await {
        return response;
    }
    if scenario == "invalid_grant" || form.get("refresh_token").is_some_and(|v| v == "invalid") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":"invalid_grant"})),
        )
            .into_response();
    }
    let scope = if form
        .get("grant_type")
        .is_some_and(|v| v == "authorization_code")
    {
        let entry = s
            .codes
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(form.get("code").map(String::as_str).unwrap_or_default());
        let Some((challenge, scope, client_id)) = entry else {
            return StatusCode::BAD_REQUEST.into_response();
        };
        // PKCE S256 wird vor jedem Code-Austausch geprüft.
        if super::common::pkce_challenge(
            form.get("code_verifier")
                .map(String::as_str)
                .unwrap_or_default(),
        ) != challenge
            || form.get("client_id") != Some(&client_id)
        {
            return StatusCode::BAD_REQUEST.into_response();
        }
        scope
    } else {
        "https://www.googleapis.com/auth/analytics.readonly".into()
    };
    Json(serde_json::json!({"access_token":"fixture-access","refresh_token":"fixture-refresh","expires_in":if scenario=="short"{2}else{3600},"token_type":"Bearer","scope":scope,"account":"Fixture Google","account_id":"fixture-account"})).into_response()
}
async fn revoke(State(s): State<Arc<OAuthMock>>, headers: HeaderMap) -> Response {
    s.log.record("revoke");
    if headers.get("X-Scenario").is_some_and(|v| v == "error") {
        StatusCode::SERVICE_UNAVAILABLE.into_response()
    } else {
        StatusCode::OK.into_response()
    }
}
