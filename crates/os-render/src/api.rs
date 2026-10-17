//! Chromium renderer contracts. The engine supplies request policy and fetched responses;
//! this crate owns browser execution, bounded capture and browser recovery.
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

/// Secrets are supplied by the caller's SecretStore, never read or persisted here.
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
    /// E05 resolves the RAM/threads/profile default (K2-36); at least one tab.
    pub max_tabs: usize,
    /// Zero disables the respective recycling trigger.
    pub restart_after_pages: u64,
    pub memory_limit_mb: u64,
    pub network: NetworkSettings,
    pub proxy_credentials: Option<Credentials>,
    /// Chromium host resolver rules, supplied from the crawl's host mapping.
    pub host_resolver_rules: Vec<String>,
    pub launch_timeout_ms: u32,
    /// Explicit opt-in for containers that cannot provide Chromium's sandbox.
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

/// A response already fetched by the engine. Fulfilled via CDP at the original URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MainResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    /// Decoded body; transport Content-Encoding/Length headers are removed on replay.
    pub body: Vec<u8>,
}
/// Engine-owned robots policy, including custom robots and the ignore switch.
/// Returning false prevents the request from reaching the network.
pub trait ResourcePolicy: Send + Sync + std::fmt::Debug {
    fn allowed<'a>(
        &'a self,
        url: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + 'a>>;
}

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
    /// Hard ceiling including an unresponsive JS engine. Zero derives it from load,
    /// AJAX and screenshot budgets plus a bounded five-second extraction allowance.
    pub page_timeout_ms: u32,
    pub screenshot: ScreenshotOpts,
    pub js_error_reporting: bool,
    pub headers: BTreeMap<String, String>,
    pub cookies: Vec<chromiumoxide::cdp::browser_protocol::network::CookieParam>,
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
    /// True when load did not fire; the available DOM is still returned.
    pub load_timed_out: bool,
    pub final_url: String,
    /// DOM image order, including duplicate URLs (different display sizes).
    pub images: Vec<(String, ImageRenderFacts)>,
}

pub use browser::Renderer;

fn protocol(error: impl std::fmt::Display) -> RenderError {
    RenderError::Protocol(error.to_string())
}
