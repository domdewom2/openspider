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
                let line = crate::diag::redact_text(&format!(
                    "{} ERROR panic: {}\n{}",
                    crate::diag::timestamp(),
                    info,
                    std::backtrace::Backtrace::force_capture()
                ));
                let _ = file.write_line(&line.replace('\n', "\\n"));
                old(info);
            }));
        });
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
