//! Fensterereignisse: Sichtbarkeit (UI-143) und Schließen/Beenden (F-11).
//!
//! * Sichtbarkeit: `Focused`, `Resized` und `Moved` lösen eine Neuberechnung aus; sichtbar ist ein Fenster, wenn es
//!   angezeigt und nicht minimiert ist. Das Ergebnis geht an `App::set_visibility` (Quelle 2 von 2); die zweite Quelle
//!   ist `visibilitychange` im Frontend. Der Verdeckt-Zustand von macOS (`NSWindowOcclusionState`) ist über Tauri nicht
//!   erreichbar; die Quelle des Frontends deckt ihn ab, soweit die WebView ihn meldet.
//! * Schließen: `CloseRequested` des Hauptfensters und `RunEvent::ExitRequested` (Strg/Cmd+Q) werden angehalten und
//!   als `App::request_quit` weitergegeben; erst bei `QuitDecision::Quit` beendet die Hülle den Prozess. Die Rückfrage
//!   bei laufendem Crawl stellt das Frontend.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use os_app::quit::QuitDecision;
use os_app::{App, WindowId};
use tauri::{AppHandle, Manager, Runtime, Window, WindowEvent};

/// Label des Hauptfensters.
pub const MAIN_WINDOW: &str = "main";

/// Merker, dass der Prozess nach `request_quit` kontrolliert beendet wird.
#[derive(Clone, Default)]
pub struct Quitting(Arc<AtomicBool>);

impl Quitting {
    /// Läuft das kontrollierte Beenden?
    pub fn is_set(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    pub(crate) fn set(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// Reagiert auf ein Fensterereignis.
pub fn on_window_event<R: Runtime>(
    app: &App,
    quitting: &Quitting,
    window: &Window<R>,
    event: &WindowEvent,
) {
    match event {
        WindowEvent::Focused(_) | WindowEvent::Resized(_) | WindowEvent::Moved(_) => {
            report_visibility(app, window);
        }
        WindowEvent::CloseRequested { api, .. } if window.label() == MAIN_WINDOW => {
            if !quitting.is_set() {
                api.prevent_close();
                request_quit(app.clone(), window.app_handle().clone(), quitting.clone());
            }
        }
        WindowEvent::Destroyed => {
            app.window_closed(&WindowId(window.label().to_string()));
        }
        _ => {}
    }
}

fn report_visibility<R: Runtime>(app: &App, window: &Window<R>) {
    let visible = window.is_visible().unwrap_or(true) && !window.is_minimized().unwrap_or(false);
    app.set_visibility(WindowId(window.label().to_string()), visible);
}

/// Fragt den Kern, ob beendet werden darf, und beendet bei `Quit`.
pub fn request_quit<R: Runtime>(app: App, handle: AppHandle<R>, quitting: Quitting) {
    tauri::async_runtime::spawn(async move {
        match app.request_quit().await {
            QuitDecision::Quit => {
                quitting.set();
                handle.exit(0);
            }
            // The App menu is also available while Settings is in front. Its
            // confirmation lives in main and must be made visible there.
            QuitDecision::Confirm { .. } => focus_main(&handle),
        }
    });
}

/// Holt das Hauptfenster nach vorn (zweiter Start, geöffnete Datei).
pub fn focus_main<R: Runtime>(handle: &AppHandle<R>) {
    if let Some(w) = handle.get_webview_window(MAIN_WINDOW) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}
