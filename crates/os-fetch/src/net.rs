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

/// Verified TLS over an existing tunnel (SMTP and HTTPS proxies, OUT-280 / K2-61).
/// Uses the same platform verifier and additional roots as other app connections.
pub async fn tls_tunnel<S>(
    mut stream: S,
    host: &str,
    settings: &NetworkSettings,
) -> Result<tokio::io::DuplexStream, String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin + 'static,
{
    use std::{
        io::{Read, Write},
        sync::Arc,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let verifier = rustls_platform_verifier::Verifier::new_with_extra_roots(
        certificates::roots(settings)?,
        provider.clone(),
    )
    .map_err(|_| "TLS-Vertrauensspeicher nicht verfügbar")?;
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|_| "TLS-Konfiguration ungültig")?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    let name = rustls::pki_types::ServerName::try_from(host.to_owned())
        .map_err(|_| "TLS-Hostname ungültig")?;
    let mut tls = rustls::ClientConnection::new(Arc::new(config), name)
        .map_err(|_| "TLS-Konfiguration ungültig")?;
    let (client, mut plain) = tokio::io::duplex(64 * 1024);
    let (ready, wait) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let mut ready = Some(ready);
        let result: Result<(), std::io::Error> = async {
            let mut encrypted = [0u8; 16384];
            let mut clear = [0u8; 16384];
            loop {
                while tls.wants_write() {
                    let mut bytes = Vec::new();
                    tls.write_tls(&mut bytes)?;
                    stream.write_all(&bytes).await?;
                    stream.flush().await?;
                }
                if !tls.is_handshaking() {
                    if let Some(tx) = ready.take() {
                        let _ = tx.send(());
                    }
                    loop {
                        match tls.reader().read(&mut clear) {
                            Ok(0) => return Ok(()),
                            Ok(n) => plain.write_all(&clear[..n]).await?,
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                            Err(e) => return Err(e),
                        }
                    }
                }
                tokio::select! {
                    _ = async { match ready.as_mut() { Some(tx) => tx.closed().await, None => std::future::pending::<()>().await } } => return Ok(()),
                    n = stream.read(&mut encrypted) => {
                        let n=n?; if n==0 { return Ok(()); }
                        tls.read_tls(&mut &encrypted[..n])?;
                        tls.process_new_packets().map_err(std::io::Error::other)?;
                    }
                    n = plain.read(&mut clear), if !tls.is_handshaking() => {
                        let n=n?; if n==0 { tls.send_close_notify(); return Ok(()); }
                        tls.writer().write_all(&clear[..n])?;
                    }
                }
            }
        }
        .await;
        let _ = result; // Never expose peer text or credentials in errors/logs.
    });
    wait.await
        .map_err(|_| "TLS-Verbindung oder Zertifikatsprüfung fehlgeschlagen")?;
    Ok(client)
}

/// OAuth client for mail, including the caller's injected proxy credential (OUT-280 / K2-33).
pub fn mail_oauth_client(
    settings: &NetworkSettings,
    password: Option<&str>,
) -> Result<reqwest::Client, String> {
    client_builder_with_password(settings, password)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|_| "OAuth-Client nicht verfügbar".into())
}

/// OAuth token request sharing app-wide proxy/roots; errors never contain the response body.
pub async fn mail_token_response(
    settings: &NetworkSettings,
    password: Option<&str>,
    endpoint: &str,
    form: &[(&str, &str)],
) -> Result<String, String> {
    let client = mail_oauth_client(settings, password)?;
    let body = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(form.iter().copied())
        .finish();
    let response = client
        .post(endpoint)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await
        .map_err(|_| "OAuth-Verbindung fehlgeschlagen")?;
    if !response.status().is_success() {
        return Err(format!(
            "OAuth-Token abgelehnt (HTTP {})",
            response.status().as_u16()
        ));
    }
    response
        .text()
        .await
        .map_err(|_| "OAuth-Antwort nicht lesbar".into())
}

/// Resolve a system proxy for SMTP. Manual settings never consult this resolver.
pub fn mail_system_proxy(host: &str, _port: u16) -> Option<os_model::settings::ProxySettings> {
    let mut bypass: Vec<String> = std::env::var("no_proxy")
        .or_else(|_| std::env::var("NO_PROXY"))
        .unwrap_or_default()
        .split(',')
        .map(str::to_string)
        .collect();
    let value = [
        "all_proxy",
        "ALL_PROXY",
        "https_proxy",
        "HTTPS_PROXY",
        "http_proxy",
        "HTTP_PROXY",
    ]
    .iter()
    .find_map(|key| std::env::var(key).ok());
    let _ = host;
    let value = value.or_else(|| {
        let native = mail_platform_proxy()?;
        bypass.extend(native.bypass);
        Some(native.url)
    })?;
    let url = url::Url::parse(&value).ok()?;
    Some(os_model::settings::ProxySettings {
        mode: ProxyMode::Manual,
        kind: match url.scheme() {
            "http" => ProxyKind::Http,
            "https" => ProxyKind::Https,
            "socks5" | "socks5h" => ProxyKind::Socks5,
            _ => return None,
        },
        host: url.host_str()?.into(),
        port: url.port_or_known_default().or(Some(1080))?,
        username: (!url.username().is_empty()).then(|| url.username().into()),
        bypass,
    })
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn mail_platform_proxy() -> Option<mail_proxy_platform::NativeProxy> {
    None
}
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn mail_platform_proxy() -> Option<mail_proxy_platform::NativeProxy> {
    mail_proxy_platform::resolve()
}
mod mail_proxy_platform;
