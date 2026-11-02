//! Chromium-Bereitstellung nach K3-01 (M3-K01): Status, Download auf Anfrage, Abbruch, Entfernen.
//!
//! Der Download läuft als App-Aufgabe unabhängig vom aufrufenden Fenster und von Crawls; die Oberfläche fragt den
//! Fortschritt mit `render_provision.status` ab. Transport: HTTP-Client mit dem app-weiten Proxy und den
//! vertrauenswürdigen Zertifikaten (OUT-269, CRAWL-122), Prüfsumme vor dem Entpacken (OUT-265).
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use futures_util::{future::BoxFuture, StreamExt};
use os_model::CrawlState;
use os_render::provision::{
    self, download, Cancel, Lookup, Manifest, Phase, Platform, ProvisionError, Response, Source,
    Transport,
};
use serde_json::Value;

use crate::{
    dto::{
        ChromiumFailure, ChromiumJob, ChromiumSource, ChromiumStatus, ChromiumStatusArgs, CmdError,
        ErrorCode,
    },
    registry::{CmdResult, CommandMeta, Handler, Registry},
    runtime::{
        error::{parse_args, to_json},
        App, CallCtx,
    },
};

pub fn register(r: &mut Registry) {
    for (name, handler) in [
        ("render_provision.status", status as Handler),
        ("render_provision.download", start as Handler),
        ("render_provision.cancel", cancel as Handler),
        ("render_provision.remove", remove as Handler),
    ] {
        r.register(
            CommandMeta {
                name,
                args: "ChromiumStatusArgs",
                ret: "ChromiumStatus",
                long_running: false,
            },
            handler,
        );
    }
}

/// Vorgaben für Tests und die E2E-Bridge; ohne Vorgabe gelten die Werte dieses Prozesses.
#[derive(Clone, Default)]
pub struct Overrides {
    pub root: Option<PathBuf>,
    pub manifest: Option<Manifest>,
    pub platform: Option<Option<Platform>>,
    /// Wert von `OPENSPIDER_CHROMIUM`.
    pub environment: Option<Option<PathBuf>>,
    /// Ergebnis der Systemsuche.
    pub system: Option<Option<PathBuf>>,
    pub transport: Option<Arc<dyn Transport>>,
}

struct Inner {
    job: ChromiumJob,
    cancel: Option<Cancel>,
    running: bool,
    overrides: Overrides,
}

/// App-weiter Zustand der Bereitstellung (eine Aufgabe zur Zeit).
pub struct ProvisionService {
    inner: Mutex<Inner>,
}

impl Default for ProvisionService {
    fn default() -> Self {
        ProvisionService {
            inner: Mutex::new(Inner {
                job: ChromiumJob::Idle,
                cancel: None,
                running: false,
                overrides: Overrides::default(),
            }),
        }
    }
}

impl ProvisionService {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }
}

/// Setzt die Vorgaben (Tests, Bridge).
pub fn set_overrides(app: &App, overrides: Overrides) {
    app.ext::<ProvisionService>().lock().overrides = overrides;
}

fn overrides(app: &App) -> Overrides {
    app.ext::<ProvisionService>().lock().overrides.clone()
}

/// Ablage: Vorgabe, sonst Standardablage dieses Prozesses (`OPENSPIDER_HOME/browser` bzw. lokaler Datenordner); eine
/// App mit eigenem Basisordner (Tests, Bridge) legt unter `<Einstellungen>/browser` ab.
fn root(app: &App, o: &Overrides) -> Option<PathBuf> {
    if let Some(root) = &o.root {
        return Some(root.clone());
    }
    let config = app.config();
    let process = os_store::paths::app_paths();
    if config.paths.settings == process.settings {
        provision::default_root()
    } else {
        Some(config.paths.settings.join("browser"))
    }
}

fn manifest(o: &Overrides) -> Result<Manifest, ProvisionError> {
    match &o.manifest {
        Some(m) => Ok(m.clone()),
        None => Manifest::bundled().cloned(),
    }
}

fn platform(o: &Overrides) -> Option<Platform> {
    o.platform.unwrap_or_else(Platform::current)
}

/// Aktueller Zustand; `configured` ist der Entwurfswert von `rendering.chromium_path`.
pub fn snapshot(app: &App, configured: Option<&str>) -> ChromiumStatus {
    let o = overrides(app);
    let manifest = manifest(&o).ok();
    let platform = platform(&o);
    let plan = manifest
        .as_ref()
        .zip(platform)
        .and_then(|(m, p)| m.plan(p).ok());
    let root = root(app, &o);
    let installed = root
        .as_deref()
        .zip(platform)
        .and_then(|(r, p)| provision::installed(r, p));
    let lookup = Lookup {
        configured: configured
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(PathBuf::from),
        environment: o.environment.clone().unwrap_or_else(|| {
            std::env::var_os(provision::CHROMIUM_ENV)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        }),
        root: root.clone(),
        platform,
    };
    let system = o.system.clone();
    let (active, active_path) = match provision::resolve_with(&lookup, || {
        system.unwrap_or_else(provision::system_chromium)
    }) {
        Ok(found) => (
            match found.source {
                Source::Configured => ChromiumSource::Configured,
                Source::Environment => ChromiumSource::Environment,
                Source::Provisioned => ChromiumSource::Provisioned,
                Source::System => ChromiumSource::System,
            },
            Some(found.path.display().to_string()),
        ),
        Err(_) => (
            ChromiumSource::None,
            lookup
                .configured
                .or(lookup.environment)
                .map(|p| p.display().to_string()),
        ),
    };
    let partial_bytes = match (&root, &plan) {
        (Some(root), Some(plan)) => provision::partial_bytes(root, plan),
        _ => 0,
    };
    ChromiumStatus {
        supported: plan.is_some(),
        version: manifest.map(|m| m.version).unwrap_or_default(),
        download_bytes: plan.as_ref().map(|p| p.archive.size),
        installed: installed.as_ref().map(|i| i.version.clone()),
        installed_path: installed.map(|i| i.executable.display().to_string()),
        active,
        active_path,
        partial_bytes,
        job: app.ext::<ProvisionService>().lock().job.clone(),
    }
}

fn error(code: ErrorCode, e: &ProvisionError) -> CmdError {
    CmdError::new(code, e.message_key()).with_detail(e.to_string())
}

fn args(args: Value) -> Result<ChromiumStatusArgs, CmdError> {
    if args.is_null() {
        return Ok(ChromiumStatusArgs::default());
    }
    parse_args(args)
}

fn status<'a>(app: &'a App, _: CallCtx, a: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let a = args(a)?;
        to_json(&snapshot(app, a.configured_path.as_deref()))
    })
}

fn start<'a>(app: &'a App, _: CallCtx, a: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let a = args(a)?;
        begin(app)?;
        to_json(&snapshot(app, a.configured_path.as_deref()))
    })
}

/// Startet den Download im Hintergrund (läuft bereits einer, bleibt es bei diesem).
pub fn begin(app: &App) -> Result<(), CmdError> {
    let o = overrides(app);
    let plan = platform(&o)
        .ok_or(ProvisionError::Unsupported)
        .and_then(|p| manifest(&o)?.plan(p))
        .map_err(|e| error(ErrorCode::Unsupported, &e))?;
    let root = root(app, &o).ok_or_else(|| {
        error(
            ErrorCode::Io,
            &ProvisionError::Io("kein App-Datenordner".into()),
        )
    })?;
    let service = app.ext::<ProvisionService>();
    let cancel = Cancel::new();
    {
        let mut inner = service.lock();
        if inner.running {
            return Ok(());
        }
        inner.running = true;
        inner.cancel = Some(cancel.clone());
        inner.job = ChromiumJob::Downloading {
            done: provision::partial_bytes(&root, &plan),
            total: plan.archive.size,
        };
    }
    let app = app.clone();
    app.handle().clone().spawn(async move {
        let service = app.ext::<ProvisionService>();
        let transport = match o.transport {
            Some(t) => Ok(t),
            None => http_transport(&app)
                .await
                .map(|t| Arc::new(t) as Arc<dyn Transport>),
        };
        let progress = {
            let service = service.clone();
            move |p: download::Progress| {
                service.lock().job = match p.phase {
                    Phase::Downloading => ChromiumJob::Downloading {
                        done: p.done,
                        total: p.total,
                    },
                    Phase::Verifying => ChromiumJob::Verifying,
                    Phase::Installing => ChromiumJob::Installing,
                };
            }
        };
        let result = match transport {
            Ok(transport) => {
                download::provision(&plan, &root, transport.as_ref(), &cancel, &progress).await
            }
            Err(e) => Err(e),
        };
        let mut inner = service.lock();
        inner.running = false;
        inner.cancel = None;
        inner.job = match result {
            Ok(installed) => {
                tracing::info!(version = %installed.version, "Chromium bereitgestellt");
                ChromiumJob::Idle
            }
            Err(e) => {
                tracing::warn!("Chromium-Bereitstellung: {e}");
                ChromiumJob::Failed(ChromiumFailure {
                    message_key: e.message_key(),
                    detail: e.to_string(),
                })
            }
        };
    });
    Ok(())
}

fn cancel<'a>(app: &'a App, _: CallCtx, a: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let a = args(a)?;
        if let Some(cancel) = &app.ext::<ProvisionService>().lock().cancel {
            cancel.cancel();
        }
        to_json(&snapshot(app, a.configured_path.as_deref()))
    })
}

fn remove<'a>(app: &'a App, _: CallCtx, a: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let a = args(a)?;
        let service = app.ext::<ProvisionService>();
        if service.lock().running {
            return Err(CmdError::new(ErrorCode::Busy, "err-chromium-busy"));
        }
        // Ein laufender oder pausierter Crawl startet seine Browser-Prozesse ggf. neu (CRAWL-078).
        if app
            .sessions()
            .all()
            .iter()
            .any(|s| matches!(s.state, CrawlState::Running | CrawlState::Paused))
        {
            return Err(CmdError::new(
                ErrorCode::CrawlRunning,
                "err-chromium-crawl-running",
            ));
        }
        if let Some(root) = root(app, &overrides(app)) {
            let result = app
                .handle()
                .spawn_blocking(move || provision::remove(&root))
                .await
                .map_err(|e| ProvisionError::Io(e.to_string()))
                .and_then(|r| r);
            result.map_err(|e| error(ErrorCode::Io, &e))?;
        }
        service.lock().job = ChromiumJob::Idle;
        to_json(&snapshot(app, a.configured_path.as_deref()))
    })
}

/// HTTP-Zugang mit dem app-weiten Proxy (Passwort aus dem Schlüsselbund, abseits des UI-Threads gelesen).
struct HttpTransport(reqwest::Client);

impl Transport for HttpTransport {
    fn get<'a>(&'a self, url: &'a str, offset: u64) -> BoxFuture<'a, Result<Response, String>> {
        Box::pin(async move {
            let mut request = self.0.get(url);
            if offset > 0 {
                request = request.header(reqwest::header::RANGE, format!("bytes={offset}-"));
            }
            let response = request.send().await.map_err(|e| e.to_string())?;
            let content_range = response
                .headers()
                .get(reqwest::header::CONTENT_RANGE)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string);
            Ok(Response {
                status: response.status().as_u16(),
                content_range,
                body: response
                    .bytes_stream()
                    .map(|chunk| chunk.map(|b| b.to_vec()).map_err(|e| e.to_string()))
                    .boxed(),
            })
        })
    }
}

async fn http_transport(app: &App) -> Result<HttpTransport, ProvisionError> {
    let network = app.settings_snapshot().network;
    let secrets = app.config().secrets.clone();
    let client = app
        .handle()
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
                .user_agent(concat!("OpenSpider/", env!("CARGO_PKG_VERSION")))
                .referer(false)
                .connect_timeout(Duration::from_secs(30))
                .read_timeout(Duration::from_secs(60))
                .redirect(reqwest::redirect::Policy::limited(5))
                .build()
        })
        .await
        .map_err(|e| ProvisionError::Io(e.to_string()))?
        .map_err(|e| ProvisionError::Network(e.to_string()))?;
    Ok(HttpTransport(client))
}
