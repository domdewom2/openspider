//! Eine eigene CDP-Sitzung steuert Fetch. So werden Proxy- und Origin-Zugangsdaten
//! getrennt beantwortet; chromiumoxide würde sonst dasselbe Paar für beide nutzen.
use super::{capture::Data, protocol, LaunchOpts, RenderError, RenderOpts};
use base64::{prelude::BASE64_STANDARD, Engine};
use chromiumoxide::{
    cdp::browser_protocol::target::{SessionId, TargetId},
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

/// Browser-Ereignisse bleiben auch nach dem Verlust der Seiten-Session erreichbar.
/// Der Kanal ist unabhängig von Fetch, damit auch eine wartende Ressourcenrichtlinie
/// die Crash-Erkennung nicht blockiert.
pub(super) struct TargetWatch {
    conn: Connection<CdpJsonEventMessage>,
    target: TargetId,
    session: SessionId,
}
impl TargetWatch {
    pub async fn start(ws: &str, target: &TargetId) -> Result<Self, RenderError> {
        let mut conn = Connection::<CdpJsonEventMessage>::connect(ws)
            .await
            .map_err(protocol)?;
        command(
            &mut conn,
            None,
            "Target.setDiscoverTargets",
            json!({"discover": true}),
        )
        .await?;
        let attached = command(
            &mut conn,
            None,
            "Target.attachToTarget",
            json!({"targetId": target, "flatten": true}),
        )
        .await?;
        let session = attached["sessionId"]
            .as_str()
            .ok_or_else(|| protocol("CDP ohne Sitzung"))?;
        Ok(Self {
            conn,
            target: target.clone(),
            session: SessionId::from(session.to_owned()),
        })
    }

    pub async fn wait(&mut self) -> Result<(), RenderError> {
        while let Some(message) = self.conn.next().await {
            if let Message::Event(event) = message.map_err(protocol)? {
                if target_lost(&event, &self.target, &self.session) {
                    return Ok(());
                }
            }
        }
        Err(RenderError::Crashed("CDP-Verbindung geschlossen".into()))
    }

    #[cfg(test)]
    pub fn crash(&mut self) {
        // Page.crash bestätigt den Befehl nicht zuverlässig; nur das Ereignis zählt.
        self.conn
            .submit_command("Page.crash".into(), Some(self.session.clone()), json!({}))
            .expect("Crash-Befehl senden");
    }
}

fn target_lost(event: &CdpJsonEventMessage, target: &TargetId, session: &SessionId) -> bool {
    match event.method.as_ref() {
        "Target.targetCrashed" => event.params["targetId"].as_str() == Some(target.as_ref()),
        // targetId ist bei detachedFromTarget optional. Die eigene Session bindet
        // das Ereignis eindeutig ans Ziel; fremde Sessions dürfen weiterlaufen.
        "Target.detachedFromTarget" => event.params["sessionId"].as_str() == Some(session.as_ref()),
        _ => false,
    }
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
        let main_url: String = page
            .evaluate(format!(
                "(() => {{ const u = new URL({}); u.hash = ''; return u.href; }})()",
                serde_json::to_string(url).map_err(protocol)?
            ))
            .await
            .map_err(protocol)?
            .into_value()
            .map_err(protocol)?;
        // Chromium normalisiert den Origin einschließlich Standardports und IDNs.
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

pub async fn dispose_context(
    websocket: &str,
    context: chromiumoxide::cdp::browser_protocol::browser::BrowserContextId,
    target: Option<chromiumoxide::cdp::browser_protocol::target::TargetId>,
) -> Result<(), RenderError> {
    let mut conn = Connection::<CdpJsonEventMessage>::connect(websocket)
        .await
        .map_err(protocol)?;
    if let Some(target) = target {
        let _ = command(
            &mut conn,
            None,
            "Target.closeTarget",
            json!({"targetId":target}),
        )
        .await;
    }
    command(
        &mut conn,
        None,
        "Target.disposeBrowserContext",
        json!({"browserContextId":context}),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // spec: CRAWL-078
    fn crash_events_are_scoped_to_the_target_and_own_session() {
        let target = TargetId::from("render-target".to_owned());
        let session = SessionId::from("watch-session".to_owned());
        let matches = |method: &'static str, params| {
            target_lost(
                &CdpJsonEventMessage {
                    method: method.into(),
                    session_id: None,
                    params,
                },
                &target,
                &session,
            )
        };
        assert!(matches("Target.targetCrashed", json!({"targetId": target})));
        assert!(!matches(
            "Target.targetCrashed",
            json!({"targetId": "neighbor"})
        ));
        assert!(matches(
            "Target.detachedFromTarget",
            json!({"sessionId": session})
        ));
        assert!(!matches(
            "Target.detachedFromTarget",
            json!({"targetId": target, "sessionId": "other-session"})
        ));
        assert!(!matches("Target.detachedFromTarget", json!({})));
        assert!(!matches("Inspector.targetCrashed", json!({})));
        assert!(!matches(
            "Target.targetCreated",
            json!({"targetId": target})
        ));
    }
}
