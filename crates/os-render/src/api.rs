//! Öffentliche Oberfläche von `os-render` (M2.md 3.5): Chromium starten, eine Seite rendern, Ergebnis lesen.
//!
//! STUB(M2-R01/M2-E05): [`Renderer::launch`] und [`Renderer::render`] liefern bis zur Umsetzung
//! [`RenderError::NotImplemented`]. Die Ergebnistypen (`ConsoleEntry`, `BlockedResource`, `NetworkEntry`, `ScreenshotTile`,
//! `RenderTimings`) stammen aus `os-model` und sind dieselben, die der Writer in `render_*` und `screenshots` ablegt.

use std::path::PathBuf;

use os_model::{BlockedResource, ConsoleEntry, NetworkEntry, RenderTimings, ScreenshotTile};

/// Fehler beim Starten von Chromium oder beim Rendern.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RenderError {
    /// Noch nicht umgesetzt (Stub).
    #[error("nicht implementiert: {0}")]
    NotImplemented(&'static str),
    /// Chromium nicht gefunden oder nicht startbar.
    #[error("Chromium nicht startbar: {0}")]
    Launch(String),
    /// Zeitüberschreitung beim Laden oder Warten (AJAX-Timeout).
    #[error("Zeitüberschreitung beim Rendern: {0}")]
    Timeout(String),
    /// Der Browser ist abgestürzt (Neustart durch den Aufrufer).
    #[error("Browser abgestürzt: {0}")]
    Crashed(String),
    /// Sonstiger Fehler des Protokolls.
    #[error("Renderfehler: {0}")]
    Protocol(String),
}

/// Optionen für den Start von Chromium.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LaunchOpts {
    /// Pfad zur Chromium-Binärdatei; `None` = suchen (`OPENSPIDER_CHROMIUM`, bekannte Installationsorte).
    pub chromium_path: Option<PathBuf>,
    /// Ohne Fenster starten (Standard im Crawl; die Anmeldung zeigt das Fenster, `login`).
    pub headless: bool,
    /// Profilordner; `None` = frisches temporäres Profil.
    pub user_data_dir: Option<PathBuf>,
}

/// Optionen für eine Seite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderOpts {
    /// Fenstergröße in Pixeln (Breite, Höhe).
    pub viewport: (u32, u32),
    /// Wartezeit auf nachladende Inhalte in Millisekunden (AJAX-Timeout, CRAWL-078).
    pub ajax_timeout_ms: u32,
    /// Screenshots anfertigen (CRAWL-082).
    pub screenshots: bool,
    /// User-Agent-Header; `None` = der von Chromium.
    pub user_agent: Option<String>,
}

impl Default for RenderOpts {
    fn default() -> Self {
        RenderOpts {
            viewport: (1280, 1024),
            ajax_timeout_ms: 5_000,
            screenshots: false,
            user_agent: None,
        }
    }
}

/// Ergebnis des Renderns einer Seite.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RenderedPage {
    /// Das gerenderte DOM als HTML.
    pub dom_html: String,
    /// Konsolenmeldungen (CRAWL-084).
    pub console: Vec<ConsoleEntry>,
    /// Blockierte oder fehlgeschlagene Ressourcen (CRAWL-083).
    pub blocked: Vec<BlockedResource>,
    /// Netzwerkanfragen des Browsers.
    pub network: Vec<NetworkEntry>,
    /// Kacheln des Seiten-Screenshots.
    pub screenshot_tiles: Vec<ScreenshotTile>,
    /// Zeiten.
    pub timings: RenderTimings,
}

/// Ein gestarteter Chromium.
#[derive(Debug)]
pub struct Renderer {
    _private: (),
}

// STUB(M2-R01)
impl Renderer {
    /// Startet Chromium.
    pub async fn launch(_opts: LaunchOpts) -> Result<Renderer, RenderError> {
        Err(RenderError::NotImplemented("Renderer::launch (M2-R01)"))
    }

    /// Rendert eine Seite.
    pub async fn render(&self, _url: &str, _opts: &RenderOpts) -> Result<RenderedPage, RenderError> {
        Err(RenderError::NotImplemented("Renderer::render (M2-R01)"))
    }
}
