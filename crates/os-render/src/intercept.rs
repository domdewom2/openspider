//! A separate CDP session owns Fetch. chromiumoxide's page handler automatically
//! answers auth challenges with one shared credential pair; isolating Fetch lets
//! us distinguish proxy credentials from origin credentials without leaking either.
use super::{capture::Data, protocol, LaunchOpts, RenderError, RenderOpts};
use base64::{prelude::BASE64_STANDARD, Engine};
use chromiumoxide::{
    cdp::browser_protocol::target::SessionId,
    types::{CdpJsonEventMessage, Message},
    Connection, Page,
};
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

pub struct Intercept(tokio::task::JoinHandle<()>);
impl Drop for Intercept {
    fn drop(&mut self) {
        self.0.abort();
    }
}
async fn command(
    conn: &mut Connection<CdpJsonEventMessage>,
    session: Option<SessionId>,
    method: &'static str,
    params: Value,
) -> Result<Value, RenderError> {
    let id = conn
        .submit_command(method.into(), session, params)
        .map_err(protocol)?;
    while let Some(message) = conn.next().await {
        if let Message::Response(response) = message.map_err(protocol)? {
            if response.id == id {
                return response
                    .result
                    .ok_or_else(|| protocol(format!("CDP {method}: {:?}", response.error)));
            }
        }
    }
    Err(RenderError::Crashed("CDP-Verbindung geschlossen".into()))
}
impl Intercept {
    pub async fn start(
        ws: &str,
        page: &Page,
        url: &str,
        opts: &RenderOpts,
        launch: &LaunchOpts,
        data: Arc<Mutex<Data>>,
    ) -> Result<Self, RenderError> {
        let mut conn = Connection::<CdpJsonEventMessage>::connect(ws)
            .await
            .map_err(protocol)?;
        let attached = command(
            &mut conn,
            None,
            "Target.attachToTarget",
            json!({"targetId": page.target_id(), "flatten": true}),
        )
        .await?;
        let session = SessionId::from(
            attached["sessionId"]
                .as_str()
                .ok_or_else(|| protocol("CDP ohne Sitzung"))?
                .to_owned(),
        );
        command(
            &mut conn,
            Some(session.clone()),
            "Fetch.enable",
            json!({"patterns":[{"urlPattern":"*"}],"handleAuthRequests":true}),
        )
        .await?;
        let policy = opts.resource_policy.clone();
        let mut response = opts.main_response.clone();
        let main_url = url.split('#').next().unwrap_or(url).to_owned();
        let response_timeout_ms = opts.response_timeout_ms;
        // Ask Chromium to canonicalize the origin (including default ports/IDNs).
        let origin: String = page
            .evaluate(format!(
                "new URL({}).origin",
                serde_json::to_string(url).map_err(protocol)?
            ))
            .await
            .map_err(protocol)?
            .into_value()
            .map_err(protocol)?;
        let credentials = opts.credentials.clone();
        let proxy = launch.proxy_credentials.clone();
        let launch = launch.clone();
        let task = tokio::spawn(async move {
            let mut authenticated = HashSet::new();
            while let Some(Ok(message)) = conn.next().await {
                let Message::Event(event) = message else {
                    continue;
                };
                let p = event.params;
                let id = p["requestId"].clone();
                let (method, params) = match event.method.as_ref() {
                    "Fetch.authRequired" => {
                        let source = p["authChallenge"]["source"].as_str().unwrap_or_default();
                        let candidate = if source == "Proxy" {
                            proxy.as_ref()
                        } else if p["authChallenge"]["origin"].as_str() == Some(origin.as_str()) {
                            credentials.as_ref()
                        } else {
                            None
                        };
                        let key = (id.to_string(), source.to_owned());
                        let auth = match candidate.filter(|_| authenticated.insert(key)) {
                            Some(c) => {
                                json!({"response":"ProvideCredentials", "username":c.username, "password":c.password})
                            }
                            None => {
                                if source == "Proxy" {
                                    data.lock().expect("capture").proxy_auth_failed = true;
                                }
                                json!({"response":"CancelAuth"})
                            }
                        };
                        (
                            "Fetch.continueWithAuth",
                            json!({"requestId":id,"authChallengeResponse":auth}),
                        )
                    }
                    "Fetch.requestPaused" => {
                        let resource_url = p["request"]["url"].as_str().unwrap_or_default();
                        let allowed = match &policy {
                            Some(policy) => policy.allowed(resource_url).await,
                            None => true,
                        };
                        if !allowed {
                            data.lock().expect("capture").block(
                                resource_url.into(),
                                "robots",
                                None,
                            );
                            (
                                "Fetch.failRequest",
                                json!({"requestId":id,"errorReason":"BlockedByClient"}),
                            )
                        } else if resource_url == main_url
                            && p["resourceType"] == "Document"
                            && response.is_some()
                        {
                            let response = response.take().expect("main response");
                            let headers: Vec<_> = response
                                .headers
                                .into_iter()
                                .filter(|(k, _)| {
                                    ![
                                        "content-length",
                                        "content-encoding",
                                        "transfer-encoding",
                                        "connection",
                                    ]
                                    .contains(&k.to_ascii_lowercase().as_str())
                                })
                                .map(|(name, value)| json!({"name":name,"value":value}))
                                .collect();
                            (
                                "Fetch.fulfillRequest",
                                json!({"requestId":id,"responseCode":response.status,"responseHeaders":headers,"body":BASE64_STANDARD.encode(response.body)}),
                            )
                        } else if super::trust::needs_bridge(&launch, resource_url) {
                            match super::trust::fetch(
                                &launch,
                                &p["request"],
                                &origin,
                                credentials.as_ref(),
                                response_timeout_ms,
                            )
                            .await
                            {
                                Ok(response) => {
                                    let headers: Vec<_> = response
                                        .headers
                                        .into_iter()
                                        .map(|(name, value)| json!({"name":name,"value":value}))
                                        .collect();
                                    (
                                        "Fetch.fulfillRequest",
                                        json!({"requestId":id,"responseCode":response.status,"responseHeaders":headers,"body":BASE64_STANDARD.encode(response.body)}),
                                    )
                                }
                                Err(error) => {
                                    let mut capture = data.lock().expect("capture");
                                    if error == RenderError::ProxyAuthentication {
                                        capture.proxy_auth_failed = true;
                                    }
                                    capture.block(resource_url.into(), "error", None);
                                    drop(capture);
                                    (
                                        "Fetch.failRequest",
                                        json!({"requestId":id,"errorReason":"Failed"}),
                                    )
                                }
                            }
                        } else {
                            ("Fetch.continueRequest", json!({"requestId":id}))
                        }
                    }
                    _ => continue,
                };
                if conn
                    .submit_command(method.into(), Some(session.clone()), params)
                    .is_err()
                {
                    break;
                }
            }
        });
        Ok(Self(task))
    }
}
