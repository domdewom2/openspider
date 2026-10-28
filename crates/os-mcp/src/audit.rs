//! Bounded invocation metadata. Arbitrary strings are omitted so credentials in URLs or SQL cannot leak.
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug)]
pub struct CallRecord {
    pub at: os_model::Timestamp,
    pub client: Option<String>,
    pub tool: String,
    pub class: String,
    pub args: Value,
    pub duration_ms: u32,
    pub result_bytes: u32,
    pub ok: bool,
}
#[derive(Clone, Default, Debug)]
pub struct CallLog(Arc<Mutex<VecDeque<CallRecord>>>);
impl CallLog {
    pub fn push(&self, record: CallRecord) {
        let mut rows = self.0.lock().unwrap_or_else(|p| p.into_inner());
        if rows.len() == 500 {
            rows.pop_front();
        }
        rows.push_back(record);
    }
    pub fn rows(&self) -> Vec<CallRecord> {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .rev()
            .cloned()
            .collect()
    }
}
pub fn redacted(args: &Value) -> Value {
    // Even parameter names can be attacker controlled; only schema keys are recorded.
    let keys = [
        "crawl_id",
        "id",
        "a",
        "b",
        "url",
        "url_list",
        "urls",
        "start_url",
        "config_profile",
        "overrides",
        "sql",
        "path",
        "tab",
        "filter",
        "element",
        "sections",
        "limit",
        "offset",
        "format",
        "page",
        "headers",
        "cookies",
        "cursor",
        "confirm",
    ];
    let mut result = serde_json::Map::new();
    for key in keys {
        if let Some(value) = args.get(key) {
            result.insert(
                key.into(),
                match (key, value) {
                    (
                        "crawl_id" | "id" | "tab" | "filter" | "limit" | "offset" | "format"
                        | "page",
                        Value::String(_) | Value::Number(_) | Value::Bool(_) | Value::Null,
                    )
                    | ("confirm", Value::Bool(_)) => value.clone(),
                    (_, Value::Array(a)) => json!({"items":a.len()}),
                    _ => json!("[redacted]"),
                },
            );
        }
    }
    Value::Object(result)
}
