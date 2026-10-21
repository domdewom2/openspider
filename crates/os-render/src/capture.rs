use super::{protocol, RenderError};
use chromiumoxide::cdp::browser_protocol::network::EventDataReceived;
use chromiumoxide::{
    cdp::{
        browser_protocol::network::{
            EventLoadingFailed, EventLoadingFinished, EventRequestWillBeSent, EventResponseReceived,
        },
        js_protocol::runtime::{EventConsoleApiCalled, EventExceptionThrown},
    },
    Page,
};
use futures_util::StreamExt;
use os_model::{BlockedResource, ConsoleEntry, NetworkEntry};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Default)]
pub struct Data {
    body_bytes: HashMap<usize, (u64, u64, Option<u64>)>,
    pub console: Vec<ConsoleEntry>,
    pub dropped: u64,
    pub blocked: Vec<BlockedResource>,
    pub network: Vec<NetworkEntry>,
    requests: HashMap<String, (usize, f64)>,
    pub console_until_ms: Option<u64>,
    pub navigation_epoch_ms: u64,
    pub proxy_auth_failed: bool,
}
pub struct Capture {
    pub data: Arc<Mutex<Data>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Capture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Data {
    pub fn body_network(&self) -> Vec<NetworkEntry> {
        self.network
            .iter()
            .enumerate()
            .map(|(idx, entry)| {
                let mut entry = entry.clone();
                entry.bytes = self
                    .body_bytes
                    .get(&idx)
                    .map_or(0, |(encoded, decoded, length)| {
                        if *encoded > 0 {
                            *encoded
                        } else {
                            length.unwrap_or(*decoded)
                        }
                    });
                entry
            })
            .collect()
    }

    fn console(&mut self, mut entry: ConsoleEntry, enabled: bool) {
        entry.timestamp_ms = entry
            .timestamp_ms
            .map(|t| t.saturating_sub(self.navigation_epoch_ms));
        if !enabled
            || self
                .console_until_ms
                .is_some_and(|until| entry.timestamp_ms.is_some_and(|t| t >= until))
        {
            return;
        }
        if self.console.len() < 500 {
            self.console.push(entry);
        } else {
            self.dropped += 1;
        }
    }
    pub fn block(&mut self, url: String, reason: &str, status: Option<u16>) {
        if !self
            .blocked
            .iter()
            .any(|b| b.url == url && b.reason == reason && b.status == status)
        {
            self.blocked.push(BlockedResource {
                url,
                reason: reason.into(),
                status,
            });
        }
    }
}
impl Capture {
    pub fn begin_navigation(&self) {
        self.data.lock().expect("capture").navigation_epoch_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
    }
    pub async fn start(page: &Page, enabled: bool) -> Result<Self, RenderError> {
        let mut body_data = page
            .event_listener::<EventDataReceived>()
            .await
            .map_err(protocol)?;
        let mut console = page
            .event_listener::<EventConsoleApiCalled>()
            .await
            .map_err(protocol)?;
        let mut exceptions = page
            .event_listener::<EventExceptionThrown>()
            .await
            .map_err(protocol)?;
        let mut requests = page
            .event_listener::<EventRequestWillBeSent>()
            .await
            .map_err(protocol)?;
        let mut responses = page
            .event_listener::<EventResponseReceived>()
            .await
            .map_err(protocol)?;
        let mut finished = page
            .event_listener::<EventLoadingFinished>()
            .await
            .map_err(protocol)?;
        let mut failed = page
            .event_listener::<EventLoadingFailed>()
            .await
            .map_err(protocol)?;
        let data = Arc::new(Mutex::new(Data::default()));
        let shared = data.clone();
        let origin = 0.0;
        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    biased;
                    Some(e) = console.next() => {
                        let v = serde_json::to_value(&*e).unwrap_or_default();
                        let frame = &v["stackTrace"]["callFrames"][0];
                        let text = v["args"].as_array().map(|args| args.iter().map(|a| {
                            a["value"].as_str().map(str::to_owned).or_else(|| a.get("value").map(Value::to_string))
                                .or_else(|| a["description"].as_str().map(str::to_owned)).unwrap_or_else(|| a["type"].as_str().unwrap_or("undefined").into())
                        }).collect::<Vec<_>>().join(" ")).unwrap_or_default();
                        let kind = v["type"].as_str().unwrap_or("log");
                        let level = match kind { "warning" => "warning", "error" | "assert" => "error", "info" | "debug" => "info", _ => "log" };
                        shared.lock().expect("capture").console(ConsoleEntry { level: level.into(), text,
                            source: frame["url"].as_str().filter(|s| !s.is_empty()).map(str::to_owned),
                            line: frame["lineNumber"].as_u64().map(|n| n as u32 + 1), column: frame["columnNumber"].as_u64().map(|n| n as u32 + 1),
                            timestamp_ms: Some((v["timestamp"].as_f64().unwrap_or(origin) - origin).max(0.0) as u64) }, enabled);
                    }
                    Some(e) = exceptions.next() => {
                        let v = serde_json::to_value(&*e).unwrap_or_default(); let d = &v["exceptionDetails"];
                        let text = d["exception"]["description"].as_str().or(d["text"].as_str()).unwrap_or("JavaScript error").to_owned();
                        shared.lock().expect("capture").console(ConsoleEntry { level: "error".into(), text,
                            source: d["url"].as_str().or(d["stackTrace"]["callFrames"][0]["url"].as_str()).map(str::to_owned),
                            line: d["lineNumber"].as_u64().map(|n| n as u32 + 1), column: d["columnNumber"].as_u64().map(|n| n as u32 + 1),
                            timestamp_ms: Some((v["timestamp"].as_f64().unwrap_or(origin) - origin).max(0.0) as u64) }, enabled);
                    }
                    Some(e) = requests.next() => {
                        let v = serde_json::to_value(&*e).unwrap_or_default(); let id = v["requestId"].as_str().unwrap_or_default().to_owned();
                        let mut d = shared.lock().expect("capture");
                        if let Some((idx, _)) = d.requests.get(&id).copied() {
                            if let Some(status) = v["redirectResponse"]["status"].as_f64() { d.network[idx].status = status as u16; }
                        }
                        let index = d.network.len();
                        d.network.push(NetworkEntry { url: v["request"]["url"].as_str().unwrap_or_default().into(), status: 0,
                            resource_type: v["type"].as_str().unwrap_or("other").to_lowercase(), bytes: 0, duration_ms: None });
                        d.requests.insert(id, (index, v["timestamp"].as_f64().unwrap_or(0.0)));
                    }
                    Some(e) = responses.next() => {
                        let v = serde_json::to_value(&*e).unwrap_or_default();
                        let mut d = shared.lock().expect("capture");
                        let status = v["response"]["status"].as_f64().unwrap_or(0.0) as u16;
                        if let Some((idx, _)) = d.requests.get(v["requestId"].as_str().unwrap_or_default()).copied() { d.network[idx].status = status; }
                        if let Some((idx, _)) = d.requests.get(v["requestId"].as_str().unwrap_or_default()).copied() {
                            let length = v["response"]["headers"].as_object().and_then(|headers| headers.iter().find(|(key, _)| key.eq_ignore_ascii_case("content-length"))).and_then(|(_, value)| value.as_str()).and_then(|value| value.parse().ok());
                            d.body_bytes.entry(idx).or_default().2 = length;
                        }
                        if status >= 400 { d.block(v["response"]["url"].as_str().unwrap_or_default().into(), "error", Some(status)); }
                    }
                    Some(e) = body_data.next() => {
                        let v = serde_json::to_value(&*e).unwrap_or_default();
                        let mut d = shared.lock().expect("capture");
                        if let Some((idx, _)) = d.requests.get(v["requestId"].as_str().unwrap_or_default()).copied() {
                            let entry = d.body_bytes.entry(idx).or_default();
                            entry.0 += v["encodedDataLength"].as_u64().unwrap_or(0);
                            entry.1 += v["dataLength"].as_u64().unwrap_or(0);
                        }
                    }
                    Some(e) = finished.next() => {
                        let v = serde_json::to_value(&*e).unwrap_or_default(); let mut d = shared.lock().expect("capture");
                        if let Some((idx, start)) = d.requests.get(v["requestId"].as_str().unwrap_or_default()).copied() {
                            d.network[idx].bytes = v["encodedDataLength"].as_f64().unwrap_or(0.0) as u64;
                            d.network[idx].duration_ms = Some(((v["timestamp"].as_f64().unwrap_or(start) - start)*1000.0).max(0.0) as u32);
                        }
                    }
                    Some(e) = failed.next() => {
                        let v = serde_json::to_value(&*e).unwrap_or_default(); let mut d = shared.lock().expect("capture");
                        if let Some((idx, _)) = d.requests.get(v["requestId"].as_str().unwrap_or_default()).copied() {
                            let url = d.network[idx].url.clone();
                            if !d.blocked.iter().any(|b| b.url == url) { d.block(url, "error", None); }
                        }
                    }
                    else => break,
                }
            }
        });
        Ok(Self { data, task })
    }
}
