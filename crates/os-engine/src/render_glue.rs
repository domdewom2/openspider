//! Anbindung des JavaScript-Renderings an die Engine (CRAWL-078 ff.): das Trait [`Renderer`], das `EngineDeps.renderer`
//! trägt, und die Umsetzung für `os_render::Renderer`.
//!
//! STUB(M2-E05): Der Hook ist angelegt; Rendering-Modus, Link-Herkunft roh/gerendert und JS-Weiterleitungen setzt
//! M2-E05 um. Ohne Renderer (Textmodus) ändert sich am Ablauf nichts.

use futures_util::future::BoxFuture;
use os_render::{RenderError, RenderOpts, RenderedPage};

/// Rendert eine Seite im Browser. Objektsicher, damit die Engine `Arc<dyn Renderer>` hält und Tests einen
/// Mock-Renderer einstecken.
pub trait Renderer: Send + Sync {
    /// Lädt `url`, wartet laut `opts` und liefert das gerenderte DOM samt Konsole, blockierten Ressourcen und Netzwerk.
    fn render<'a>(
        &'a self,
        url: &'a str,
        opts: &'a RenderOpts,
    ) -> BoxFuture<'a, Result<RenderedPage, RenderError>>;
}

impl Renderer for os_render::Renderer {
    fn render<'a>(
        &'a self,
        url: &'a str,
        opts: &'a RenderOpts,
    ) -> BoxFuture<'a, Result<RenderedPage, RenderError>> {
        Box::pin(os_render::Renderer::render(self, url, opts))
    }
}
