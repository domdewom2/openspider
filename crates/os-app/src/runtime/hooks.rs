//! Einhängepunkte, die spätere Pakete füllen, ohne die Laufzeit zu ändern.

use futures_util::future::BoxFuture;

use crate::quit::QuitDecision;

use super::App;

/// Entscheidung beim Beenden (F-11). Die Sitzung (M2-K04) hängt hier die Rückfrage bei laufendem Crawl ein.
pub trait QuitHook: Send + Sync {
    /// Wird von `App::request_quit` aufgerufen.
    fn request_quit(&self, app: App) -> BoxFuture<'static, QuitDecision>;
}

/// Antwort auf eine Anfrage an das URI-Schema `ospider://` (Screenshots, Bilder).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetResponse {
    /// HTTP-Status.
    pub status: u16,
    /// Medientyp.
    pub mime: String,
    /// Inhalt.
    pub body: Vec<u8>,
}

impl AssetResponse {
    /// 404 ohne Inhalt.
    pub fn not_found() -> AssetResponse {
        AssetResponse {
            status: 404,
            mime: "text/plain".to_string(),
            body: Vec::new(),
        }
    }
}

/// Löst `ospider://shot/<crawl>/<urlId>/<tile>` und `ospider://img/…` auf. STUB(M2-K09): die Umsetzung liefert
/// `os_app::assets` und trägt sich mit `App::set_asset_resolver` ein.
pub trait AssetResolver: Send + Sync {
    /// Löst `uri` (vollständig, z. B. `ospider://shot/…`) auf.
    fn resolve(&self, app: App, uri: String) -> BoxFuture<'static, AssetResponse>;
}
