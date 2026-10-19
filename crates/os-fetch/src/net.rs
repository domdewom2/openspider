//! Gemeinsamer HTTP-Client-Aufbau aus app-weiten Netzwerkeinstellungen (K2-33).
pub mod certificates;
use os_model::{settings::NetworkSettings, ProxyKind, ProxyMode};
pub const PROXY_PASSWORD: &str = "openspider/proxy/password";
pub fn client_builder(s: &NetworkSettings) -> reqwest::ClientBuilder {
    let store = crate::secrets::secret_store_from_env();
    let password = if s.proxy.username.is_some() {
        store.get(PROXY_PASSWORD)
    } else {
        None
    };
    client_builder_with_password(s, password.as_deref())
}
/// Injizierbare Variante für Crawl-Snapshots und Tests ohne Schlüsselbund.
pub fn client_builder_with_password(
    s: &NetworkSettings,
    password: Option<&str>,
) -> reqwest::ClientBuilder {
    let mut builder = reqwest::Client::builder();
    match s.proxy.mode {
        ProxyMode::None => builder = builder.no_proxy(),
        ProxyMode::System => {}
        ProxyMode::Manual => {
            let scheme = match s.proxy.kind {
                ProxyKind::Http => "http",
                ProxyKind::Https => "https",
                ProxyKind::Socks5 => "socks5h",
            };
            let host = if s.proxy.host.contains(':') && !s.proxy.host.starts_with('[') {
                format!("[{}]", s.proxy.host)
            } else {
                s.proxy.host.clone()
            };
            builder = builder.no_proxy();
            if let Ok(mut proxy) =
                reqwest::Proxy::all(format!("{scheme}://{host}:{}", s.proxy.port))
            {
                if let Some(username) = &s.proxy.username {
                    proxy = proxy.basic_auth(username, password.unwrap_or(""));
                }
                builder = builder.proxy(
                    proxy.no_proxy(reqwest::NoProxy::from_string(&s.proxy.bypass.join(","))),
                );
            }
        }
    }
    for path in &s.trusted_certificates {
        if let Ok(bytes) = std::fs::read(path) {
            if let Some(certificates) = reqwest::Certificate::from_pem_bundle(&bytes)
                .ok()
                .filter(|certs| !certs.is_empty())
            {
                builder = builder.tls_certs_merge(certificates);
            } else if let Ok(certificate) = reqwest::Certificate::from_der(&bytes) {
                builder = builder.tls_certs_merge([certificate]);
            } else {
                tracing::warn!(path = %path, "Zertifikatsdatei enthält kein lesbares PEM- oder DER-Zertifikat");
            }
        } else {
            tracing::warn!(path = %path, "Zertifikatsdatei nicht lesbar");
        }
    }
    builder
}

/// Prüft den konfigurierten Proxy vor dem Crawl-Start. Keine Ziel-URL wird dabei abgerufen.
pub async fn probe_proxy(settings: &NetworkSettings) -> Result<(), crate::FetchError> {
    if settings.proxy.mode != ProxyMode::Manual {
        return Ok(());
    }
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::net::TcpStream::connect((settings.proxy.host.as_str(), settings.proxy.port)),
    )
    .await
    .map_err(|_| crate::FetchError::Config("Proxy-Verbindungstest: Zeitüberschreitung".into()))?
    .map_err(|_| crate::FetchError::Config("Proxy nicht erreichbar".into()))?;
    Ok(())
}
