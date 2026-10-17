//! Protokoll der E2E-Bridge (`docs/plan/M2.md` 3.6), unabhängig vom Transport.
//!
//! * Anfrage `{"id":1,"cmd":"view.rows","args":{…}}` → Antwort `{"id":1,"ok":<Wert>}` oder `{"id":1,"err":<CmdError>}`;
//!   optional `"window":"main"` (Standard `main`) bestimmt das aufrufende Fenster.
//! * `{"cmd":"__subscribe","window":"main"}` (oder `events.subscribe` mit `args.window`) ersetzt den Tauri-Channel:
//!   Ereignisse kommen als `{"event":"tick","window":"main","data":<Tick>}`, `settingsChanged` und `notice`
//!   entsprechend. `__unsubscribe` beendet das Abonnement.
//! * `{"cmd":"__abort","target":<id>}` bricht eine laufende Anfrage ab (AbortSignal des Clients); schließt die
//!   Verbindung, werden alle laufenden Anfragen abgebrochen und die Abonnements beendet.
//!
//! [`BridgeSession`] enthält die Logik einer Verbindung; der WebSocket-Transport (`bridge`, Feature `e2e-bridge`) ist
//! nur Verpackung.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use os_analyze::analysis::CancelToken;
use serde_json::{json, Value};
use tokio::sync::mpsc::UnboundedSender;

use crate::dto::{CmdError, ErrorCode, SubscriptionId, UiEvent};
use crate::events::{EventSink, SinkClosed};

use super::{App, CallCtx, WindowId};

/// Fenster, wenn die Anfrage keines nennt.
pub const DEFAULT_WINDOW: &str = "main";

/// Antwortnachricht auf eine Anfrage.
pub fn response_message(id: &Value, result: &Result<Value, CmdError>) -> String {
    match result {
        Ok(v) => json!({ "id": id, "ok": v }),
        Err(e) => json!({ "id": id, "err": e }),
    }
    .to_string()
}

/// Nachricht für ein Ereignis an ein Fenster.
pub fn event_message(window: &str, ev: &UiEvent) -> String {
    let (name, data) = match ev {
        UiEvent::FocusUrl {
            crawl_id,
            url_id,
            is_internal,
        } => (
            "focusUrl",
            Ok(json!({"crawlId": crawl_id, "urlId": url_id, "isInternal": is_internal})),
        ),
        UiEvent::Tick { tick } => ("tick", serde_json::to_value(tick)),
        UiEvent::SettingsChanged { keys } => ("settingsChanged", Ok(json!({ "keys": keys }))),
        UiEvent::Notice { notice } => ("notice", serde_json::to_value(notice)),
    };
    json!({ "event": name, "window": window, "data": data.unwrap_or(Value::Null) }).to_string()
}

/// Senke, die Ereignisse als Bridge-Nachrichten in einen Kanal schreibt.
struct ChannelSink {
    window: String,
    tx: UnboundedSender<String>,
}

impl EventSink for ChannelSink {
    fn send(&self, ev: UiEvent) -> Result<(), SinkClosed> {
        self.tx
            .send(event_message(&self.window, &ev))
            .map_err(|_| SinkClosed)
    }
}

/// Eine Verbindung der Bridge. Antworten und Ereignisse gehen in `out`.
pub struct BridgeSession {
    app: App,
    out: UnboundedSender<String>,
    inflight: Arc<Mutex<HashMap<String, CancelToken>>>,
    subs: Mutex<Vec<SubscriptionId>>,
}

fn id_key(id: &Value) -> String {
    id.to_string()
}

impl BridgeSession {
    /// Neue Verbindung.
    pub fn new(app: App, out: UnboundedSender<String>) -> BridgeSession {
        BridgeSession {
            app,
            out,
            inflight: Arc::new(Mutex::new(HashMap::new())),
            subs: Mutex::new(Vec::new()),
        }
    }

    fn reply(&self, id: &Value, result: &Result<Value, CmdError>) {
        let _ = self.out.send(response_message(id, result));
    }

    fn subscribe(&self, window: &str) -> SubscriptionId {
        let sink = Arc::new(ChannelSink {
            window: window.to_string(),
            tx: self.out.clone(),
        });
        let sub = self.app.subscribe(WindowId(window.to_string()), sink);
        self.subs
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(sub.clone());
        sub
    }

    /// Verarbeitet eine Textnachricht des Clients. Anfragen laufen nebenläufig auf der Laufzeit der App.
    pub fn handle_text(&self, text: &str) {
        let msg: Value = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(e) => {
                self.reply(
                    &Value::Null,
                    &Err(
                        CmdError::new(ErrorCode::InvalidInput, "err-bridge-bad-json")
                            .with_detail(e.to_string()),
                    ),
                );
                return;
            }
        };
        let id = msg.get("id").cloned().unwrap_or(Value::Null);
        let Some(cmd) = msg.get("cmd").and_then(Value::as_str) else {
            self.reply(
                &id,
                &Err(CmdError::invalid_input("err-bridge-missing-cmd", "cmd")),
            );
            return;
        };
        let window = msg
            .get("window")
            .and_then(Value::as_str)
            .or_else(|| {
                msg.get("args")
                    .and_then(|a| a.get("window"))
                    .and_then(Value::as_str)
            })
            .unwrap_or(DEFAULT_WINDOW)
            .to_string();
        match cmd {
            "__subscribe" | "events.subscribe" => {
                let sub = self.subscribe(&window);
                if !id.is_null() {
                    self.reply(&id, &Ok(json!({ "id": sub.id })));
                }
            }
            "__unsubscribe" => {
                let removed = self
                    .subs
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .drain(..)
                    .filter(|s| self.app.unsubscribe(s))
                    .count();
                if !id.is_null() {
                    self.reply(&id, &Ok(json!({ "removed": removed })));
                }
            }
            "__abort" => {
                let target = msg.get("target").cloned().unwrap_or(Value::Null);
                let token = self
                    .inflight
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .get(&id_key(&target))
                    .cloned();
                let aborted = token.is_some_and(|t| {
                    t.cancel();
                    self.app.interrupt_hub().poke();
                    true
                });
                if !id.is_null() {
                    self.reply(&id, &Ok(json!({ "aborted": aborted })));
                }
            }
            _ => self.spawn_call(
                id,
                cmd.to_string(),
                msg.get("args").cloned().unwrap_or(Value::Null),
                window,
            ),
        }
    }

    fn spawn_call(&self, id: Value, cmd: String, args: Value, window: String) {
        let token = CancelToken::new();
        let key = id_key(&id);
        if !id.is_null() {
            self.inflight
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .insert(key.clone(), token.clone());
        }
        let app = self.app.clone();
        let out = self.out.clone();
        let inflight = Arc::clone(&self.inflight);
        self.app.handle().spawn(async move {
            let ctx = CallCtx {
                window: WindowId(window),
                cancel: token,
            };
            let result = app.dispatch(ctx, &cmd, args).await;
            inflight
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .remove(&key);
            let _ = out.send(response_message(&id, &result));
        });
    }

    /// Die Verbindung ist zu Ende: laufende Anfragen abbrechen, Abonnements beenden.
    pub fn close(&self) {
        let tokens: Vec<CancelToken> = self
            .inflight
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .values()
            .cloned()
            .collect();
        for t in &tokens {
            t.cancel();
        }
        if !tokens.is_empty() {
            self.app.interrupt_hub().poke();
        }
        for s in self
            .subs
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .drain(..)
        {
            self.app.unsubscribe(&s);
        }
    }
}

impl Drop for BridgeSession {
    fn drop(&mut self) {
        self.close();
    }
}
