//! Moz-v3-JSON-RPC-Vertrag mit 50er-Bündeln und synthetischen Ebenenwerten.
use super::{ahrefs::Requests, common, MockApi};
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json,
};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};
#[derive(Default, Clone)]
pub struct MozMock {
    pub requests: Requests,
    pub throttle_once: Arc<std::sync::atomic::AtomicBool>,
}
impl MockApi for MozMock {
    fn name(&self) -> &'static str {
        "moz"
    }
    fn base_url_key(&self) -> Option<&'static str> {
        Some("api.moz.base_url")
    }
    fn scenario_path(&self) -> &'static str {
        "/jsonrpc"
    }
    fn router(&self) -> axum::Router {
        axum::Router::new()
            .route("/jsonrpc", post(handle).get(scenario))
            .with_state(self.clone())
    }
}
pub fn mock() -> Option<Arc<dyn MockApi>> {
    Some(Arc::new(MozMock::default()))
}
async fn scenario(headers: HeaderMap, Query(q): Query<HashMap<String, String>>) -> Response {
    common::scenario_response(&common::scenario(&headers, &q), 0)
        .await
        .unwrap_or_else(|| Json(json!({"result":{}})).into_response())
}
async fn handle(State(s): State<MozMock>, headers: HeaderMap, Json(q): Json<Value>) -> Response {
    if headers.get("x-moz-token").is_none_or(|v| v == "bad") {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":{"message":"Invalid token"}})),
        )
            .into_response();
    }
    if q["method"] == "quota.lookup" {
        return Json(
            json!({"jsonrpc":"2.0","id":q["id"],"result":{"quota":{"used":1,"limit":50}}}),
        )
        .into_response();
    }
    if q["method"] == "data.site.metrics.brand.authority.fetch" {
        s.requests.record(q.clone());
        return Json(json!({"jsonrpc":"2.0","id":q["id"],"result":{"site_metrics":{"brand_authority_score":77},"site_query":q.pointer("/params/data/site_query")}})).into_response();
    }
    if q["method"] != "data.site.metrics.fetch.multiple" {
        return StatusCode::BAD_REQUEST.into_response();
    }
    s.requests.record(q.clone());
    if s.throttle_once
        .swap(false, std::sync::atomic::Ordering::SeqCst)
    {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [("Retry-After", "7")],
            Json(json!({"error":{"message":"rate limit"}})),
        )
            .into_response();
    }
    let Some(queries) = q
        .pointer("/params/data/site_queries")
        .and_then(Value::as_array)
    else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if queries.is_empty() || queries.len() > 50 {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let rows: Vec<_> = queries.iter().map(|query| {
        let mut metrics = json!({"page_authority":45,"domain_authority":65,"spam_score":3,"brand_authority_score":77,"link_propensity":0.15,"last_crawled":"2026-10-18"});
        for suffix in ["page","subdomain","root_domain"] { for prefix in ["root_domains_to_","external_pages_to_","pages_to_","nofollow_root_domains_to_"] { metrics[format!("{prefix}{suffix}")] = json!(5000000000_i64); } }
        json!({"site_query":query,"site_metrics":metrics})
    }).collect();
    Json(
        json!({"jsonrpc":"2.0","id":q["id"],"result":{"results_by_site":rows,"errors_by_site":[]}}),
    )
    .into_response()
}
