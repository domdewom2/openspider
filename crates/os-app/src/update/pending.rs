//! Process-local installation choice and durable reopen ticket (never installer bytes).
use super::InstallWhen;
use crate::{App, CallCtx};
use std::{path::PathBuf, sync::Mutex};
#[derive(Default)]
pub struct Pending {
    pub when: Mutex<Option<InstallWhen>>,
}
fn ticket(app: &App) -> PathBuf {
    app.config().paths.settings.join("update-reopen.json")
}
pub async fn remember(app: &App) -> Result<(), String> {
    let path = app
        .sessions()
        .current(&crate::WindowId("main".into()))
        .map(|s| s.path);
    let file = ticket(app);
    app.handle()
        .spawn_blocking(move || {
            let bytes =
                serde_json::to_vec(&path).map_err(|_| "Update-Wiederherstellung fehlgeschlagen")?;
            if let Some(parent) = file.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|_| "Update-Wiederherstellung fehlgeschlagen")?;
            }
            let tmp = file.with_extension("tmp");
            use std::io::Write;
            let mut out = std::fs::File::create(&tmp)
                .map_err(|_| "Update-Wiederherstellung fehlgeschlagen")?;
            out.write_all(&bytes)
                .and_then(|_| out.sync_all())
                .map_err(|_| "Update-Wiederherstellung fehlgeschlagen")?;
            std::fs::rename(tmp, file).map_err(|_| "Update-Wiederherstellung fehlgeschlagen")
        })
        .await
        .map_err(|_| "Update-Wiederherstellung fehlgeschlagen")?
        .map_err(str::to_owned)
}
pub async fn forget(app: &App) {
    let file = ticket(app);
    let result = app
        .handle()
        .spawn_blocking(move || std::fs::remove_file(file))
        .await;
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {}
        _ => tracing::warn!("Update-Wiederöffnungs-Ticket konnte nicht entfernt werden"),
    }
}
pub async fn reopen(app: &App) {
    let file = ticket(app);
    let path = app
        .handle()
        .spawn_blocking(move || {
            let bytes = std::fs::read(&file).ok()?;
            let path = serde_json::from_slice::<Option<PathBuf>>(&bytes).ok()?;
            let _ = std::fs::remove_file(file);
            path
        })
        .await
        .ok()
        .flatten();
    if let Some(path) = path {
        if app
            .dispatch(
                CallCtx::for_window("main"),
                "crawl.open",
                serde_json::json!({"kind":"path","path":path}),
            )
            .await
            .is_err()
        {
            tracing::warn!("Letzter Crawl konnte nach Update nicht geöffnet werden");
        }
    }
}
