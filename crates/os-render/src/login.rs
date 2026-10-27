//! Interactive Chromium login and origin-scoped session capture (CRAWL-120).
use crate::{api, LaunchOpts, RenderError, Renderer};
use chromiumoxide::{
    cdp::{
        browser_protocol::{
            emulation::SetDeviceMetricsOverrideParams,
            page::{NavigateParams, RemoveScriptToEvaluateOnNewDocumentParams},
        },
        js_protocol::runtime::{AddBindingParams, EventBindingCalled},
    },
    Page,
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SESSION_GUARD_DEFAULTS: [&str; 4] = [
    "(?i).*(log-?out|sign-?out|abmelden|ausloggen).*",
    "(?i).*(delete|remove|loeschen|löschen|entfernen).*",
    "(?i).*/(admin|wp-admin|backend)/.*",
    "(?i).*[?&](action|do)=(delete|remove|logout).*",
];
#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OriginStorage {
    pub origin: String,
    pub local_storage: BTreeMap<String, String>,
    pub session_storage: BTreeMap<String, String>,
}
impl std::fmt::Debug for OriginStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OriginStorage([redacted])")
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    pub expires: Option<i64>,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapturedSession {
    pub final_url: String,
    pub cookies: Vec<LoginCookie>,
    pub storage: Vec<OriginStorage>,
}
impl std::fmt::Debug for CapturedSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CapturedSession")
            .field("cookies", &self.cookies.len())
            .finish_non_exhaustive()
    }
}
impl CapturedSession {
    pub fn expired(&self, unix_seconds: i64) -> bool {
        self.cookies
            .iter()
            .filter_map(|c| c.expires)
            .any(|t| t <= unix_seconds)
    }
}
#[derive(Clone, Debug)]
pub struct LoginOpts {
    pub headless: bool,
    pub launch: LaunchOpts,
    pub user_agent: Option<String>,
    pub viewport: (u32, u32),
    pub initial: Option<CapturedSession>,
}
impl Default for LoginOpts {
    fn default() -> Self {
        Self {
            headless: false,
            launch: LaunchOpts::default(),
            user_agent: None,
            viewport: (1024, 768),
            initial: None,
        }
    }
}
pub struct LoginSession {
    renderer: Renderer,
    page: Page,
    websocket: String,
    storage: BTreeMap<String, OriginStorage>,
    _proxy: Option<Box<dyn Send + Sync>>,
    restore_task: Option<tokio::task::JoinHandle<()>>,
}
fn error(_: impl std::fmt::Display) -> RenderError {
    RenderError::Protocol("Anmeldefenster nicht erreichbar".into())
}
impl LoginSession {
    pub async fn open(url: &str, mut opts: LoginOpts) -> Result<Self, RenderError> {
        let authority = url
            .strip_prefix("http://")
            .or_else(|| url.strip_prefix("https://"))
            .unwrap_or_default()
            .split(['/', '?', '#'])
            .next()
            .unwrap_or_default();
        if authority.is_empty()
            || authority.contains('@')
            || url.chars().any(char::is_whitespace)
            || opts.viewport.0 == 0
            || opts.viewport.1 == 0
        {
            return Err(RenderError::Protocol(
                "HTTP(S)-Login-URL ohne Zugangsdaten erforderlich".into(),
            ));
        }
        opts.launch.headless = opts.headless;
        opts.launch.user_data_dir = None;
        opts.launch.restart_after_pages = 0;
        let renderer = Renderer::launch(opts.launch.clone()).await?;
        let (page, websocket) = renderer.open_login_page().await?;
        page.execute(SetDeviceMetricsOverrideParams::new(
            opts.viewport.0,
            opts.viewport.1,
            1.0,
            false,
        ))
        .await
        .map_err(error)?;
        if let Some(ua) = opts.user_agent {
            page.set_user_agent(ua).await.map_err(error)?;
        }
        let proxy = if opts.launch.proxy_credentials.is_some() {
            Some(
                Box::new(api::login_proxy(&websocket, &page, url, &opts.launch).await?)
                    as Box<dyn Send + Sync>,
            )
        } else {
            None
        };
        // Each origin gets a separate initializer, removed as soon as it has run.
        // No marker is stored in website storage, so clear/remove operations stay intact.
        let restore_task = if let Some(initial) = &opts.initial {
            let binding = "__openspiderStorageRestored";
            page.execute(AddBindingParams::new(binding))
                .await
                .map_err(error)?;
            let mut events = page
                .event_listener::<EventBindingCalled>()
                .await
                .map_err(error)?;
            let mut scripts = BTreeMap::new();
            for storage in &initial.storage {
                let origin = serde_json::to_string(&storage.origin).map_err(error)?;
                let script = storage_script(std::slice::from_ref(storage))?;
                let id = page
                    .evaluate_on_new_document(format!(
                    "if(location.origin==={origin}) {{ {script}; window.{binding}({origin}); }}"
                ))
                    .await
                    .map_err(error)?;
                scripts.insert(storage.origin.clone(), id);
            }
            let restore_page = page.clone();
            let task = tokio::spawn(async move {
                while let Some(event) = events.next().await {
                    if event.name == binding {
                        if let Some(id) = scripts.remove(&event.payload) {
                            let _ = restore_page
                                .execute(RemoveScriptToEvaluateOnNewDocumentParams::new(id))
                                .await;
                        }
                        if scripts.is_empty() {
                            break;
                        }
                    }
                }
            });
            let cookies = initial
                .cookies
                .iter()
                .map(|c| {
                    let mut p = chromiumoxide::cdp::browser_protocol::network::CookieParam::new(
                        &c.name, &c.value,
                    );
                    p.domain = Some(c.domain.clone());
                    p.path = Some(c.path.clone());
                    p.secure = Some(c.secure);
                    p.http_only = Some(c.http_only);
                    p.expires = c.expires.map(|t| {
                        chromiumoxide::cdp::browser_protocol::network::TimeSinceEpoch::new(t as f64)
                    });
                    p
                })
                .collect();
            if !initial.cookies.is_empty() {
                page.execute(
                    chromiumoxide::cdp::browser_protocol::network::SetCookiesParams::new(cookies),
                )
                .await
                .map_err(error)?;
            }
            Some(task)
        } else {
            None
        };
        page.execute(NavigateParams::new(url))
            .await
            .map_err(error)?;
        Ok(Self {
            renderer,
            page,
            websocket,
            storage: BTreeMap::new(),
            _proxy: proxy,
            restore_task,
        })
    }
    /// CDP access for automated fixture interaction; application commands never expose this handle.
    pub fn page(&self) -> &Page {
        &self.page
    }
    pub async fn capture(&mut self) -> Result<CapturedSession, RenderError> {
        let value: serde_json::Value = self.page.evaluate("({origin:location.origin, finalUrl:location.href, localStorage:Object.fromEntries(Object.entries(localStorage)), sessionStorage:Object.fromEntries(Object.entries(sessionStorage))})").await.map_err(error)?.into_value().map_err(error)?;
        let storage: OriginStorage = serde_json::from_value(value.clone()).map_err(error)?;
        if storage.origin != "null" {
            self.storage.insert(storage.origin.clone(), storage);
        }
        let cookies = api::login_cookies(&self.websocket, &self.page)
            .await?
            .into_iter()
            .map(|c| LoginCookie {
                name: c.name,
                value: c.value,
                domain: c.domain,
                path: c.path,
                secure: c.secure,
                http_only: c.http_only,
                expires: (!c.session).then_some(c.expires as i64),
            })
            .collect();
        Ok(CapturedSession {
            final_url: value["finalUrl"].as_str().unwrap_or_default().into(),
            cookies,
            storage: self.storage.values().cloned().collect(),
        })
    }
    pub async fn close(mut self) {
        if let Some(task) = self.restore_task.take() {
            task.abort();
        }
        self.renderer.close().await;
    }
}
impl Drop for LoginSession {
    fn drop(&mut self) {
        if let Some(task) = &self.restore_task {
            task.abort();
        }
    }
}
/// JSON encoding prevents storage keys/values from becoming executable script; origin check prevents token leakage.
pub fn storage_script(storage: &[OriginStorage]) -> Result<String, RenderError> {
    let data = serde_json::to_string(storage).map_err(error)?;
    let data = serde_json::to_string(&data).map_err(error)?;
    Ok(format!("(() => {{ const entries=JSON.parse({data}); const s=entries.find(s=>s.origin===location.origin); if(s) {{ for(const [k,v] of Object.entries(s.localStorage)) localStorage.setItem(k,v); for(const [k,v] of Object.entries(s.sessionStorage)) sessionStorage.setItem(k,v); }} }})()"))
}
