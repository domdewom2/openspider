//! Majestic OpenApps: Bündelantwort, Indexwahl und Berechtigungsfehler.
use super::{ahrefs::Requests, common, MockApi};
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Form, Json,
};
use serde_json::json;
use std::{collections::HashMap, sync::Arc};
#[derive(Default, Clone)]
pub struct MajesticMock {
    pub requests: Requests,
}
impl MockApi for MajesticMock {
    fn name(&self) -> &'static str {
        "majestic"
    }
    fn base_url_key(&self) -> Option<&'static str> {
        Some("api.majestic.base_url")
    }
    fn scenario_path(&self) -> &'static str {
        "/api_command"
    }
    fn router(&self) -> axum::Router {
        axum::Router::new()
            .route("/api_command", post(handle).get(scenario))
            .route("/api/json", get(scenario))
            .with_state(self.clone())
    }
}
pub fn mock() -> Option<Arc<dyn MockApi>> {
    Some(Arc::new(MajesticMock::default()))
}
async fn scenario(headers: HeaderMap, Query(q): Query<HashMap<String, String>>) -> Response {
    common::scenario_response(&common::scenario(&headers, &q), 0)
        .await
        .unwrap_or_else(|| Json(json!({"Code":"OK"})).into_response())
}
async fn handle(State(s): State<MajesticMock>, Form(q): Form<HashMap<String, String>>) -> Response {
    if !q.contains_key("accesstoken") || !q.contains_key("privatekey") {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if q["accesstoken"] == "bad" {
        return Json(json!({"Code":"NoAccess","ErrorMessage":"API permission missing for bad"}))
            .into_response();
    }
    if q.get("cmd").map(String::as_str) == Some("GetSubscriptionInfo") {
        return Json(json!({"Code":"OK"})).into_response();
    }
    let mut safe = q.clone();
    safe.remove("accesstoken");
    safe.remove("privatekey");
    s.requests.record(json!(safe));
    let n = q
        .get("items")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
    if n == 0
        || n > 100
        || !q
            .get("datasource")
            .is_some_and(|s| matches!(s.as_str(), "fresh" | "historic"))
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let rows: Vec<_> = (0..n).map(|i| json!({"Item":q[&format!("item{i}")],"TrustFlow":75,"CitationFlow":80,"ExtBackLinks":5000000000_i64,"RefDomains":123,"RefIPs":99,"RefSubNets":88,"IndexedURLs":77,"ExtBackLinksEDU":66,"ExtBackLinksGOV":55,"RefDomainsEDU":44,"RefDomainsGOV":33})).collect();
    Json(json!({"Code":"OK","DataTables":{"Results":{"Data":rows}}})).into_response()
}
