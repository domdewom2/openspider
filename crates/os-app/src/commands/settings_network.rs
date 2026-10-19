//! Netzwerk-Commands; Geheimnisse bleiben ausschließlich im SecretStore.
use crate::{
    dto::{CertificateChainDto, CertificateProbeArgs, CmdError, ErrorCode, ProxyPasswordArgs},
    registry::{CmdResult, CommandMeta, Registry},
    runtime::{
        error::{parse_args, to_json},
        App, CallCtx,
    },
};
use futures_util::future::BoxFuture;
fn probe<'a>(app: &'a App, _: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: CertificateProbeArgs = parse_args(args)?;
        let settings = app.settings_snapshot().network;
        let secrets = app.config().secrets.clone();
        let password =
            tokio::task::spawn_blocking(move || secrets.try_get(os_fetch::net::PROXY_PASSWORD))
                .await
                .map_err(|_| error("Schlüsselbund-Aufgabe abgebrochen"))?
                .map_err(|_| error("Proxy-Passwort nicht verfügbar"))?;
        let result =
            os_fetch::net::certificates::probe(&args.url, &settings, password.as_deref(), &[])
                .await
                .map_err(|e| error(&e))?;
        to_json(&CertificateChainDto {
            certificates: result.certificates,
            trusted: result.trusted,
        })
    })
}
fn password<'a>(app: &'a App, _: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: ProxyPasswordArgs = parse_args(args)?;
        let secrets = app.config().secrets.clone();
        tokio::task::spawn_blocking(move || match args.password {
            Some(value) => secrets.set(os_fetch::net::PROXY_PASSWORD, &value),
            None => secrets.delete(os_fetch::net::PROXY_PASSWORD),
        })
        .await
        .map_err(|_| error("Schlüsselbund-Aufgabe abgebrochen"))?
        .map_err(|_| error("Proxy-Passwort konnte nicht gespeichert werden"))?;
        Ok(serde_json::Value::Null)
    })
}
fn error(detail: &str) -> CmdError {
    CmdError::new(ErrorCode::Io, "err-network").with_detail(detail.to_string())
}
pub fn register(r: &mut Registry) {
    r.register(
        CommandMeta {
            name: "settings.probeCertificates",
            args: "CertificateProbeArgs",
            ret: "CertificateChainDto",
            long_running: false,
        },
        probe,
    );
    r.register(
        CommandMeta {
            name: "settings.setProxyPassword",
            args: "ProxyPasswordArgs",
            ret: "void",
            long_running: false,
        },
        password,
    );
}

/// Fachvalidierung wird vor dem atomaren Speichern ausgeführt.
pub fn on_startup(app: &App) {
    app.settings().add_validator(|settings| {
        os_fetch::net::certificates::roots(&settings.network)
            .map(|_| ())
            .map_err(|e| {
                CmdError::invalid_input("err-invalid-certificate", "network.trustedCertificates")
                    .with_detail(e)
            })
    });
}

/// Netzwerkzustände werden mit dem vorhandenen Tick-Coalescer ausgeliefert.
pub fn collect_notices(app: &App, parts: &mut crate::events::TickParts) {
    for session in app.sessions().all() {
        if let Some(engine) = session.engine {
            if let Ok(handle) = engine.handle.try_lock() {
                if let Some(handle) = handle.as_ref() {
                    for message in handle.take_network_notices() {
                        parts.notices.push(crate::dto::Notice {
                            level: crate::dto::NoticeLevel::Warning,
                            code: if message.starts_with("Crawl-delay") {
                                "crawl-delay-capped"
                            } else {
                                "network-throttled"
                            }
                            .into(),
                            args: std::collections::BTreeMap::from([(
                                "message".into(),
                                message.into(),
                            )]),
                        });
                    }
                }
            }
        }
    }
}

/// Gemeinsamer Einstieg für den Crawl-Start (K04): Netzwerkeinstellungen und
/// Schlüsselbund werden beim Start kopiert; Fortsetzen verwendet den vorhandenen Fetcher.
pub async fn crawl_fetcher(
    app: &App,
    cfg: &os_model::CrawlConfig,
) -> Result<os_fetch::HttpFetcher, CmdError> {
    let network = app.settings_snapshot().network;
    os_fetch::net::probe_proxy(&network)
        .await
        .map_err(|e| error(&e.to_string()))?;
    if cfg.http.ignore_certificate_errors {
        app.push_notice(crate::dto::Notice {
            level: crate::dto::NoticeLevel::Warning,
            code: "tls-verification-disabled".into(),
            args: std::collections::BTreeMap::from([(
                "message".into(),
                "Zertifikatsfehler werden für diesen Crawl ignoriert".into(),
            )]),
        });
    }
    let secrets = app.config().secrets.clone();
    let clock = app.config().clock.clone();
    let cfg = cfg.clone();
    tokio::task::spawn_blocking(move || {
        let mut config = os_fetch::FetchConfig::from_crawl_config(&cfg, &*secrets, clock)
            .map_err(|e| error(&e.to_string()))?
            .with_network(&network);
        if network.proxy.username.is_some() {
            config.proxy_password = secrets
                .try_get(os_fetch::net::PROXY_PASSWORD)
                .map_err(|_| error("Proxy-Passwort nicht verfügbar"))?;
        }
        os_fetch::HttpFetcher::new(config).map_err(|e| error(&e.to_string()))
    })
    .await
    .map_err(|_| error("Netzwerkinitialisierung abgebrochen"))?
}
