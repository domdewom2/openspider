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

// Nur für `TargetWatch::kill_renderers`; spart eine Manifestabhängigkeit (SIGKILL ist 9 unter Linux und macOS).
#[cfg(all(test, unix))]
extern "C" {
    fn kill(pid: i32, signal: i32) -> i32;
}
#[cfg(all(test, unix))]
const SIGKILL: i32 = 9;

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
    pub async fn crash(&mut self, timeout: std::time::Duration) -> Result<(), RenderError> {
        // Page.crash bestätigt Erfolg nicht zuverlässig; nur das Ereignis zählt.
        // Eine Ablehnung (z. B. während Navigation) ist dagegen sofort ein Fehler.
        let id = self
            .conn
            .submit_command("Page.crash".into(), Some(self.session.clone()), json!({}))
            .map_err(protocol)?;
        self.await_loss(Some(id), timeout).await
    }

    /// Beendet die Renderer-Prozesse des Browsers hart (SIGKILL). Page.crash läuft über den
    /// Absturzbehandler des Renderers; unter Linux (Chrome 155) blieb dort jedes Ereignis aus.
    /// Das Ziel ist der einzige Tab des Test-Browsers; ein Reserve-Renderer stirbt harmlos mit.
    #[cfg(all(test, unix))]
    pub async fn kill_renderers(
        &mut self,
        timeout: std::time::Duration,
    ) -> Result<(), RenderError> {
        let info = command(&mut self.conn, None, "SystemInfo.getProcessInfo", json!({})).await?;
        let pids: Vec<i32> = info["processInfo"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|process| process["type"] == "renderer")
            .filter_map(|process| process["id"].as_i64()?.try_into().ok())
            .collect();
        if pids.is_empty() {
            return Err(protocol(format!("Kein Renderer-Prozess: {info}")));
        }
        for pid in &pids {
            // SAFETY: kill(2) mit gültiger PID und Signal greift auf keinen Speicher zu.
            if unsafe { kill(*pid, SIGKILL) } != 0 {
                return Err(protocol(format!(
                    "SIGKILL für Renderer {pid}: {}",
                    std::io::Error::last_os_error()
                )));
            }
        }
        self.await_loss(None, timeout).await
    }

    /// Wartet auf das Verlustereignis des Ziels und protokolliert jede empfangene Nachricht,
    /// damit eine Zeitüberschreitung ihre Ursache nennt statt nur `Elapsed`.
    #[cfg(test)]
    async fn await_loss(
        &mut self,
        crash_command: Option<chromiumoxide::types::CallId>,
        timeout: std::time::Duration,
    ) -> Result<(), RenderError> {
        let started = std::time::Instant::now();
        let mut log = Vec::new();
        let wait = tokio::time::timeout(timeout, async {
            while let Some(message) = self.conn.next().await {
                let at = started.elapsed().as_millis();
                match message.map_err(protocol)? {
                    Message::Event(event) => {
                        log.push(format!(
                            "+{at}ms Ereignis {} session={:?} params={}",
                            event.method, event.session_id, event.params
                        ));
                        if target_lost(&event, &self.target, &self.session) {
                            return Ok(());
                        }
                    }
                    Message::Response(response) => {
                        log.push(format!(
                            "+{at}ms Antwort id={} result={:?} error={:?}",
                            response.id, response.result, response.error
                        ));
                        if Some(response.id) == crash_command {
                            if let Some(error) = response.error {
                                return Err(protocol(format!(
                                    "CDP Page.crash abgelehnt: {error:?}"
                                )));
                            }
                        }
                    }
                }
            }
            Err(RenderError::Crashed("CDP-Verbindung geschlossen".into()))
        })
        .await;
        if let Ok(result) = wait {
            return result;
        }
        let targets = command(&mut self.conn, None, "Target.getTargets", json!({}));
        let targets = tokio::time::timeout(std::time::Duration::from_secs(5), targets)
            .await
            .map_or_else(|_| "Zeitüberschreitung".to_owned(), |r| format!("{r:?}"));
        Err(protocol(format!(
            "Kein Crash-Ereignis nach {timeout:?} (target={:?}, session={:?}):\n{}\nTarget.getTargets: {targets}",
            self.target,
            self.session,
            log.join("\n")
        )))
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
        let cookie_context = if opts.disable_cookies {
            let info = command(
                &mut conn,
                None,
                "Target.getTargetInfo",
                json!({"targetId":page.target_id()}),
            )
            .await?;
            Some(info["targetInfo"]["browserContextId"].clone())
        } else {
            None
        };
        let mut cookie_control = if opts.disable_cookies {
            Some(
                Connection::<CdpJsonEventMessage>::connect(ws)
                    .await
                    .map_err(protocol)?,
            )
        } else {
            None
        };
        let disable_cookies = opts.disable_cookies;
        let mut patterns = vec![json!({"urlPattern":"*"})];
        if disable_cookies || opts.request_identity.is_some() {
            patterns.push(json!({"urlPattern":"*", "requestStage":"Response"}));
        }
        command(
            &mut conn,
            Some(session.clone()),
            "Fetch.enable",
            json!({"patterns":patterns,"handleAuthRequests":true}),
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
        let main_frame = page.mainframe().await.map_err(protocol)?;
        let identity = opts.request_identity.clone();
        let credentials = opts.credentials.clone();
        let proxy = launch.proxy_credentials.clone();
        let snippets = opts.snippets.clone();
        let snippet_target = page.target_id().as_ref().to_owned();
        let task = tokio::spawn(async move {
            let mut authenticated = HashSet::new();
            let mut auth_retried = HashSet::new();
            while let Some(Ok(message)) = conn.next().await {
                let Message::Event(event) = message else {
                    continue;
                };
                let p = event.params;
                if event.method.as_ref() == "Fetch.requestPaused" {
                    if let (Some(control), Some(context)) = (&mut cookie_control, &cookie_context) {
                        if command(
                            control,
                            None,
                            "Storage.clearCookies",
                            json!({"browserContextId": context}),
                        )
                        .await
                        .is_err()
                        {
                            break;
                        }
                    }
                }
                let id = p["requestId"].clone();
                let (method, params) = match event.method.as_ref() {
                    "Fetch.authRequired" => {
                        let source = p["authChallenge"]["source"].as_str().unwrap_or_default();
                        let candidate = if source == "Proxy" {
                            proxy.as_ref()
                        } else if identity.is_some() {
                            // The scoped transport handles this 401 at response stage.
                            // Giving Chromium credentials populates its origin-wide auth cache.
                            None
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
                    "Fetch.requestPaused" if p["responseStatusCode"].is_number() => {
                        let resource_url = p["request"]["url"].as_str().unwrap_or_default();
                        let authenticated = if p["responseStatusCode"] == 401
                            && p["request"]["method"] == "GET"
                            && auth_retried.insert(id.to_string())
                        {
                            match &identity {
                                Some(identity) => identity.auth_response(resource_url).await,
                                None => None,
                            }
                        } else {
                            None
                        };
                        if let Some(response) = authenticated {
                            fulfill(id, response, disable_cookies)
                        } else if disable_cookies {
                            let headers = p["responseHeaders"]
                                .as_array()
                                .cloned()
                                .unwrap_or_default()
                                .into_iter()
                                .filter(|h| {
                                    !h["name"]
                                        .as_str()
                                        .unwrap_or_default()
                                        .eq_ignore_ascii_case("set-cookie")
                                })
                                .collect::<Vec<_>>();
                            (
                                "Fetch.continueResponse",
                                json!({"requestId":id,"responseCode":p["responseStatusCode"],"responseHeaders":headers}),
                            )
                        } else {
                            ("Fetch.continueResponse", json!({"requestId":id}))
                        }
                    }
                    "Fetch.requestPaused" => {
                        let resource_url = p["request"]["url"].as_str().unwrap_or_default();
                        let resource_type = p["resourceType"].as_str().unwrap_or_default();
                        let resource_type = if resource_type == "Document"
                            && p["frameId"].as_str()
                                == main_frame.as_ref().map(|frame| frame.as_ref())
                        {
                            "Navigation"
                        } else {
                            resource_type
                        };
                        // INTEG-061/072: stop passive page resources during extraction;
                        // explicit Fetch/XHR still pass through the existing policy.
                        if snippets.blocks_resource(&snippet_target, resource_type) {
                            conn.submit_command(
                                "Fetch.failRequest".into(),
                                Some(session.clone()),
                                json!({"requestId":id,"errorReason":"BlockedByClient"}),
                            )
                            .map_err(protocol)
                            .ok();
                            continue;
                        }
                        let allowed = match &policy {
                            Some(policy) if resource_url != main_url => {
                                policy.allowed_resource(resource_url, resource_type).await
                            }
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
                            if disable_cookies {
                                if let Some(response) = &mut response {
                                    response.headers.retain(|(name, _)| {
                                        !name.eq_ignore_ascii_case("set-cookie")
                                    });
                                }
                            }
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
                        } else if identity.is_some() || disable_cookies {
                            let mut headers = p["request"]["headers"]
                                .as_object()
                                .cloned()
                                .unwrap_or_default();
                            if identity.is_some() {
                                headers.retain(|name, _| {
                                    !name.eq_ignore_ascii_case("accept-language")
                                        && !name.eq_ignore_ascii_case("authorization")
                                });
                            }
                            if disable_cookies {
                                headers.retain(|name, _| !name.eq_ignore_ascii_case("cookie"));
                            }
                            for (name, value) in identity
                                .as_ref()
                                .map(|i| i.headers(resource_url))
                                .unwrap_or_default()
                            {
                                headers.retain(|key, _| !key.eq_ignore_ascii_case(&name));
                                headers.insert(name, json!(value));
                            }
                            let headers = headers
                                .into_iter()
                                .map(|(name, value)| json!({"name":name,"value":value}))
                                .collect::<Vec<_>>();
                            (
                                "Fetch.continueRequest",
                                json!({"requestId":id,"headers":headers}),
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

fn fulfill(
    id: Value,
    response: super::MainResponse,
    disable_cookies: bool,
) -> (&'static str, Value) {
    let headers = response
        .headers
        .into_iter()
        .filter(|(name, _)| {
            ![
                "content-length",
                "content-encoding",
                "transfer-encoding",
                "connection",
            ]
            .contains(&name.to_ascii_lowercase().as_str())
                && !(disable_cookies && name.eq_ignore_ascii_case("set-cookie"))
        })
        .map(|(name, value)| json!({"name":name,"value":value}))
        .collect::<Vec<_>>();
    (
        "Fetch.fulfillRequest",
        json!({"requestId":id,"responseCode":response.status,"responseHeaders":headers,"body":BASE64_STANDARD.encode(response.body)}),
    )
}

/// Read the complete isolated context via the browser endpoint, including cookies
/// whose paths/domains are absent from the current document.
pub async fn cookies(
    ws: &str,
    target: &TargetId,
) -> Result<Vec<chromiumoxide::cdp::browser_protocol::network::Cookie>, RenderError> {
    let mut conn = Connection::<CdpJsonEventMessage>::connect(ws)
        .await
        .map_err(protocol)?;
    let info = command(
        &mut conn,
        None,
        "Target.getTargetInfo",
        json!({"targetId": target}),
    )
    .await?;
    let value = command(
        &mut conn,
        None,
        "Storage.getCookies",
        json!({"browserContextId": info["targetInfo"]["browserContextId"]}),
    )
    .await?;
    Ok(
        serde_json::from_value::<chromiumoxide::cdp::browser_protocol::storage::GetCookiesReturns>(
            value,
        )
        .map_err(protocol)?
        .cookies,
    )
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
