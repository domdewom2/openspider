//! App logging with immediate filter reload, bounded files and panic backtraces.
use crate::diag::{cleanup, LogFile, LogLayer};
use std::{
    sync::{Mutex, OnceLock},
    time::SystemTime,
};
use tracing_subscriber::{prelude::*, reload, EnvFilter};

type Reload = reload::Handle<EnvFilter, tracing_subscriber::Registry>;
#[derive(Default)]
struct State(Mutex<Option<Reload>>);

pub fn init_for_app(app: &crate::App) {
    let settings = app.settings_snapshot();
    if let Err(e) = cleanup(
        &app.config().paths.logs,
        SystemTime::now(),
        settings.logging.retention_days,
        500 * 1024 * 1024,
    ) {
        eprintln!("Log-Bereinigung fehlgeschlagen: {}", e.kind());
    }
    // OUT-246 (M3-C01): Absturzberichte – Kontext, Sitzungsmarke und Aufbewahrung, bevor der Lauf ins Log schreibt.
    let session = crate::crash::on_app_start(app);
    let filter =
        EnvFilter::try_new(&settings.logging.level).unwrap_or_else(|_| EnvFilter::new("info"));
    let (filter, handle) = reload::Layer::new(filter);
    let file = LogFile::new(app.config().paths.logs.join("openspider.log"), true);
    let subscriber = tracing_subscriber::registry().with(filter).with(LogLayer {
        file: file.clone(),
        json: false,
        stderr: false,
    });
    if subscriber.try_init().is_ok() {
        *app.ext::<State>()
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = Some(handle);
        static HOOK: OnceLock<()> = OnceLock::new();
        HOOK.get_or_init(|| {
            let old = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                let backtrace = std::backtrace::Backtrace::force_capture().to_string();
                let line = crate::diag::redact_text(&format!(
                    "{} ERROR panic: {}\n{}",
                    crate::diag::timestamp(),
                    info,
                    backtrace
                ));
                let _ = file.write_line(&line.replace('\n', "\\n"));
                // OUT-246: Absturzbericht, bevor der Prozess endet; Fehler beim Schreiben bleiben folgenlos.
                let _ = crate::crash::shared::on_panic(info, &backtrace);
                old(info);
            }));
        });
    }
    #[cfg(debug_assertions)]
    crate::crash::service::test_trigger();
    if let Some(session) = session {
        // Startzeile des Laufs: daran findet der nächste Start das Log eines abgestürzten Laufs (OUT-246).
        tracing::info!(session = %session, "App-Sitzung gestartet");
    }
}
pub fn apply_settings(app: &crate::App) {
    let state = app.ext::<State>();
    if let Some(handle) = &*state.0.lock().unwrap_or_else(|p| p.into_inner()) {
        if let Ok(filter) = EnvFilter::try_new(&app.settings_snapshot().logging.level) {
            let _ = handle.reload(filter);
        }
    };
}
