//! Gemeinsame Szenarien und redigiertes Request-Log für API-Mocks.
use axum::{
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};
#[derive(Debug, Clone)]
pub struct Call {
    pub path: String,
    pub at: Instant,
}
#[derive(Clone, Default)]
pub struct CallLog(Arc<Mutex<Vec<Call>>>);
impl CallLog {
    pub fn record(&self, path: &str) {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).push(Call {
            path: path.split('?').next().unwrap_or_default().into(),
            at: Instant::now(),
        });
    }
    pub fn calls(&self) -> Vec<Call> {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
}
pub fn scenario(headers: &HeaderMap, query: &HashMap<String, String>) -> String {
    headers
        .get("X-Scenario")
        .and_then(|h| h.to_str().ok())
        .map(str::to_string)
        .or_else(|| query.get("scenario").cloned())
        .unwrap_or_else(|| "ok".into())
}
pub async fn scenario_response(scenario: &str, attempt: usize) -> Option<Response> {
    match scenario {
        "hang" => {
            std::future::pending::<()>().await;
            None
        }
        "slow-2s" => {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            None
        }
        "empty" => Some(Json(serde_json::json!({"items":[]})).into_response()),
        "error" | "4xx" => Some(
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error":"invalid_credentials"})),
            )
                .into_response(),
        ),
        "429" | "throttle" => Some(
            (
                StatusCode::TOO_MANY_REQUESTS,
                Json(serde_json::json!({"error":"rate_limit"})),
            )
                .into_response(),
        ),
        "429x2" if attempt < 2 => Some(
            (
                StatusCode::TOO_MANY_REQUESTS,
                Json(serde_json::json!({"error":"rate_limit"})),
            )
                .into_response(),
        ),
        "retry-after-7" if attempt == 0 => Some(
            (
                StatusCode::TOO_MANY_REQUESTS,
                [("Retry-After", "7")],
                Json(serde_json::json!({"error":"rate_limit"})),
            )
                .into_response(),
        ),
        "500-always" => Some(
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error":"temporary"})),
            )
                .into_response(),
        ),
        _ => None,
    }
}

/// S256 über den bereits vorhandenen TLS-Kryptoprovider (keine eigene Hashimplementierung).
pub fn pkce_challenge(verifier: &str) -> String {
    use base64::Engine;
    let suite = rustls::crypto::aws_lc_rs::cipher_suite::TLS13_AES_128_GCM_SHA256
        .tls13()
        .expect("TLS 1.3");
    let digest = suite.common.hash_provider.hash(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest.as_ref())
}

/// Quellenunabhängiger Vertragsmock für den API-Rahmen; Fachmocks bleiben bei I02–I05.
#[derive(Default, Clone)]
pub struct FixtureApi {
    pub log: CallLog,
    pub attempts: Arc<Mutex<HashMap<String, usize>>>,
}
impl super::MockApi for FixtureApi {
    fn name(&self) -> &'static str {
        "common"
    }
    fn base_url_key(&self) -> Option<&'static str> {
        None
    }
    fn router(&self) -> axum::Router {
        axum::Router::new()
            .fallback(fixture)
            .with_state(self.clone())
    }
}
async fn fixture(
    axum::extract::State(s): axum::extract::State<FixtureApi>,
    headers: HeaderMap,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
    uri: axum::http::Uri,
) -> Response {
    s.log.record(uri.path());
    let scenario = scenario(&headers, &q);
    let attempt = {
        let mut counts = s.attempts.lock().unwrap_or_else(|p| p.into_inner());
        let n = counts.entry(scenario.clone()).or_default();
        let value = *n;
        *n += 1;
        value
    };
    if headers
        .get("Authorization")
        .is_some_and(|v| v == "Bearer bad")
        || q.get("key").is_some_and(|v| v == "bad")
    {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error":"invalid_credentials"})),
        )
            .into_response();
    }
    if let Some(response) = scenario_response(&scenario, attempt).await {
        return response;
    }
    Json(serde_json::json!({"items":[{"id":"fixture"}],"value":42,"host":headers.get("Host").and_then(|h|h.to_str().ok())})).into_response()
}
