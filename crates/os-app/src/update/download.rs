//! Download transport boundary. Only complete, signature-verified bytes become installable.
use super::{UpdateManifest, UpdateState};
use crate::{commands::update::UpdateService, App};
use futures_util::future::BoxFuture;
use std::sync::{Arc, Mutex};

pub type Progress = Arc<dyn Fn(u64, Option<u64>) + Send + Sync>;
/// The desktop implements this with Tauri's updater. Tests inject an interrupted transport.
pub trait UpdateBackend: Send + Sync {
    fn download(
        &self,
        manifest: UpdateManifest,
        progress: Progress,
    ) -> BoxFuture<'static, Result<Vec<u8>, String>>;
    fn install(
        &self,
        manifest: UpdateManifest,
        bytes: Vec<u8>,
        restart: bool,
    ) -> BoxFuture<'static, Result<(), String>>;
}
#[derive(Default)]
pub struct Downloads {
    pub backend: Mutex<Option<Arc<dyn UpdateBackend>>>,
    pub(crate) artifact: Mutex<Option<(UpdateManifest, Vec<u8>)>>,
}
pub fn set_backend(app: &App, backend: Arc<dyn UpdateBackend>) {
    *app.ext::<Downloads>()
        .backend
        .lock()
        .unwrap_or_else(|p| p.into_inner()) = Some(backend);
}
pub fn state(app: &App, state: UpdateState) -> UpdateState {
    *app.ext::<UpdateService>()
        .state
        .lock()
        .unwrap_or_else(|p| p.into_inner()) = state.clone();
    state
}
/// Returns immediately; the process owns the task, independent of the invoking window.
/// Bytes live only in memory: a crash discards the download, including any incomplete payload.
pub fn download(app: &App) -> UpdateState {
    let service = app.ext::<UpdateService>();
    let Ok(guard) = service.gate.try_lock() else {
        return service
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
    };
    let current = service
        .state
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    if matches!(
        current,
        UpdateState::Downloading { .. } | UpdateState::Ready(_)
    ) {
        return current;
    }
    let manifest = service
        .manifest
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    let backend = app
        .ext::<Downloads>()
        .backend
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    let (Some(manifest), Some(backend)) = (manifest, backend) else {
        return state(
            app,
            UpdateState::Failed("Update-Download nicht verfügbar".into()),
        );
    };
    let initial = state(
        app,
        UpdateState::Downloading {
            done: 0,
            total: None,
        },
    );
    drop(guard);
    let app = app.clone();
    app.handle().spawn(async move {
        let service = app.ext::<UpdateService>();
        let _guard = service.gate.lock().await;
        let progress_app = app.clone();
        let progress = Arc::new(move |done, total| {
            state(&progress_app, UpdateState::Downloading { done, total });
        });
        match backend.download(manifest.clone(), progress).await {
            Ok(bytes) => {
                *app.ext::<Downloads>()
                    .artifact
                    .lock()
                    .unwrap_or_else(|p| p.into_inner()) = Some((manifest.clone(), bytes));
                state(
                    &app,
                    UpdateState::Ready(super::PendingInstall {
                        version: manifest.version.to_string(),
                    }),
                );
            }
            Err(_) => {
                app.ext::<Downloads>()
                    .artifact
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .take();
                tracing::warn!("Update-Download abgebrochen; unvollständige Daten verworfen");
                state(
                    &app,
                    UpdateState::Failed("Update-Download fehlgeschlagen".into()),
                );
            }
        }
    });
    initial
}

/// Shared proxy/trust settings, including credentials fetched away from the UI thread.
pub async fn client_builder(app: &App) -> Result<reqwest::ClientBuilder, String> {
    let network = app.settings_snapshot().network;
    let secrets = app.config().secrets.clone();
    app.handle()
        .spawn_blocking(move || {
            let password = if network.proxy.username.is_some() {
                secrets
                    .try_get(os_fetch::net::PROXY_PASSWORD)
                    .ok()
                    .flatten()
            } else {
                None
            };
            os_fetch::net::client_builder_with_password(&network, password.as_deref())
                .user_agent(super::USER_AGENT)
                .referer(false)
                .timeout(std::time::Duration::from_secs(3600))
                .redirect(reqwest::redirect::Policy::custom(|attempt| {
                    if attempt.previous().len() >= 5
                        || super::manifest::https_url(attempt.url().as_str()).is_err()
                    {
                        attempt.error("Unsichere Update-Weiterleitung")
                    } else {
                        attempt.follow()
                    }
                }))
        })
        .await
        .map_err(|_| "Update-Client nicht verfügbar".into())
}
