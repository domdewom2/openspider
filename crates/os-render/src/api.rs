//! Verträge des Chromium-Renderers. Die Engine übergibt Richtlinien und Antworten;
//! dieses Crate steuert Browser, begrenzte Erfassung und Wiederherstellung.
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use os_model::settings::NetworkSettings;
use os_model::{
    BlockedResource, ConsoleEntry, ImageRenderFacts, NetworkEntry, RenderTimings, ScreenshotTile,
};

#[path = "browser.rs"]
mod browser;
#[path = "capture.rs"]
mod capture;
#[path = "intercept.rs"]
mod intercept;
#[path = "memory.rs"]
mod memory;
#[path = "page.rs"]
mod page;
#[path = "trust.rs"]
mod trust;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RenderError {
    #[error("nicht implementiert: {0}")]
    NotImplemented(&'static str),
    #[error("err-chromium-missing: Chromium fehlt unter {path}. Chrome/Chromium installieren oder OPENSPIDER_CHROMIUM bzw. rendering.chromium_path setzen.")]
    ChromiumMissing { path: String },
    #[error("Chromium nicht startbar: {0}")]
    Launch(String),
    #[error("Zeitüberschreitung beim Rendern: {0}")]
    Timeout(String),
    #[error("Browser abgestürzt: {0}")]
    Crashed(String),
    #[error("Proxy-Authentifizierung fehlgeschlagen (407)")]
    ProxyAuthentication,
    #[error("Renderfehler: {0}")]
    Protocol(String),
}

/// Zugangsdaten stammen aus dem SecretStore des Aufrufers und werden hier nicht gespeichert.
#[derive(Clone, PartialEq, Eq)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}
impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Credentials([redacted])")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchOpts {
    pub chromium_path: Option<PathBuf>,
    pub headless: bool,
    pub user_data_dir: Option<PathBuf>,
    /// E05 bestimmt den Standard aus RAM, Threads und Profil (K2-36); mindestens ein Tab.
    pub max_tabs: usize,
    /// Null deaktiviert den jeweiligen Neustartauslöser.
    pub restart_after_pages: u64,
    pub memory_limit_mb: u64,
    pub network: NetworkSettings,
    pub proxy_credentials: Option<Credentials>,
    /// Chromium-Regeln aus der Host-Zuordnung des Crawls.
    pub host_resolver_rules: Vec<String>,
    pub launch_timeout_ms: u32,
    /// Explizite Freigabe für Container ohne Chromium-Sandbox.
    pub no_sandbox: bool,
}
impl Default for LaunchOpts {
    fn default() -> Self {
        Self {
            chromium_path: None,
            headless: true,
            user_data_dir: None,
            max_tabs: 1,
            restart_after_pages: 500,
            memory_limit_mb: 1500,
            network: NetworkSettings::default(),
            proxy_credentials: None,
            host_resolver_rules: Vec::new(),
            launch_timeout_ms: 20_000,
            no_sandbox: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Device {
    GooglebotMobile,
    GooglebotDesktop,
    Fixed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenshotFormat {
    Png,
    Jpeg,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenshotOpts {
    pub size: Option<(u32, u32)>,
    pub fit_content: bool,
    pub resize_wait_ms: u32,
    pub format: ScreenshotFormat,
    pub jpeg_quality: u8,
}
impl Default for ScreenshotOpts {
    fn default() -> Self {
        Self {
            size: None,
            fit_content: false,
            resize_wait_ms: 300,
            format: ScreenshotFormat::Png,
            jpeg_quality: 80,
        }
    }
}

/// Bereits von der Engine abgerufene Antwort, über CDP an der Original-URL eingespielt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MainResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    /// Dekodierter Inhalt; Content-Encoding/Length werden beim Einspielen entfernt.
    pub body: Vec<u8>,
}
/// robots-Richtlinie der Engine einschließlich eigener Regeln und Ignorieren-Schalter.
/// Bei false gelangt die Anfrage nicht ins Netz.
pub trait ResourcePolicy: Send + Sync + std::fmt::Debug {
    fn allowed<'a>(
        &'a self,
        url: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + 'a>>;
}

/// Cookie der Engine; ohne Bindung an Chromium/CDP. Ohne Domain gilt die Render-URL.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: Option<String>,
    pub path: Option<String>,
    pub secure: bool,
    pub http_only: bool,
    /// Unix-Zeit in Sekunden; None bezeichnet ein Sitzungscookie.
    pub expires: Option<i64>,
}

/// Renderoptionen enthalten eine dynamische Ressourcenrichtlinie und implementieren
/// deshalb bewusst kein PartialEq.
#[derive(Debug, Clone)]
pub struct RenderOpts {
    pub viewport: (u32, u32),
    pub ajax_timeout_ms: u32,
    pub screenshots: bool,
    pub user_agent: Option<String>,
    pub device: Device,
    pub device_scale_factor: f64,
    pub mobile: bool,
    pub touch: bool,
    pub response_timeout_ms: u32,
    /// Harte Grenze auch bei blockierter JS-Engine. Null leitet sie aus Laden,
    /// AJAX und Screenshot sowie fünf Sekunden für die Extraktion ab.
    pub page_timeout_ms: u32,
    pub screenshot: ScreenshotOpts,
    pub js_error_reporting: bool,
    pub headers: BTreeMap<String, String>,
    pub cookies: Vec<Cookie>,
    pub credentials: Option<Credentials>,
    pub resource_policy: Option<Arc<dyn ResourcePolicy>>,
    pub main_response: Option<MainResponse>,
}
impl Default for RenderOpts {
    fn default() -> Self {
        Self {
            viewport: (411, 731),
            ajax_timeout_ms: 5_000,
            screenshots: true,
            user_agent: None,
            device: Device::GooglebotMobile,
            device_scale_factor: 1.0,
            mobile: true,
            touch: true,
            response_timeout_ms: 20_000,
            page_timeout_ms: 0,
            screenshot: ScreenshotOpts::default(),
            js_error_reporting: true,
            headers: BTreeMap::new(),
            cookies: Vec::new(),
            credentials: None,
            resource_policy: None,
            main_response: None,
        }
    }
}
impl RenderOpts {
    pub fn googlebot_desktop() -> Self {
        Self {
            viewport: (1024, 768),
            device: Device::GooglebotDesktop,
            mobile: false,
            touch: false,
            ..Self::default()
        }
    }
    pub fn fixed_viewport(width: u32, height: u32) -> Self {
        Self {
            viewport: (width, height),
            device: Device::Fixed,
            mobile: false,
            touch: false,
            ..Self::default()
        }
    }
    fn validate(&self) -> Result<(), RenderError> {
        if self.viewport.0 == 0
            || self.viewport.1 == 0
            || self.viewport.0 > 16_384
            || self.viewport.1 > 16_384
            || !self.device_scale_factor.is_finite()
            || !(0.1..=4.0).contains(&self.device_scale_factor)
            || self.ajax_timeout_ms > 60_000
            || !self.ajax_timeout_ms.is_multiple_of(100)
            || self.response_timeout_ms == 0
            || self.response_timeout_ms > 3_600_000
            || self.screenshot.resize_wait_ms > 10_000
            || self.screenshot.jpeg_quality > 100
            || self
                .screenshot
                .size
                .is_some_and(|(w, h)| w == 0 || h == 0 || w > 16_384 || h > 16_384)
        {
            return Err(RenderError::Protocol(
                "Ungültige Rendering-Einstellungen".into(),
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RenderedPage {
    pub dom_html: String,
    pub console: Vec<ConsoleEntry>,
    pub console_dropped: u64,
    pub blocked: Vec<BlockedResource>,
    pub network: Vec<NetworkEntry>,
    pub screenshot_tiles: Vec<ScreenshotTile>,
    pub timings: RenderTimings,
    /// Kein load-Ereignis; das bis dahin verfügbare DOM wird dennoch zurückgegeben.
    pub load_timed_out: bool,
    pub final_url: String,
    /// DOM-Bildreihenfolge einschließlich gleicher URLs mit verschiedenen Anzeigegrößen.
    pub images: Vec<(String, ImageRenderFacts)>,
}

pub use browser::Renderer;

fn protocol(error: impl std::fmt::Display) -> RenderError {
    RenderError::Protocol(error.to_string())
}
