//! Gemeinsamer HTTP-Client-Aufbau aus app-weiten Netzwerkeinstellungen (K2-33).
use os_model::{settings::NetworkSettings, ProxyKind, ProxyMode};
pub fn client_builder(s: &NetworkSettings) -> reqwest::ClientBuilder {
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
            if let Ok(proxy) = reqwest::Proxy::all(format!("{scheme}://{host}:{}", s.proxy.port)) {
                builder = builder.proxy(
                    proxy.no_proxy(reqwest::NoProxy::from_string(&s.proxy.bypass.join(","))),
                );
            }
        }
    }
    for path in &s.trusted_certificates {
        if let Ok(bytes) = std::fs::read(path) {
            if let Ok(certificates) = reqwest::Certificate::from_pem_bundle(&bytes) {
                builder = builder.tls_certs_merge(certificates);
            } else if let Ok(certificate) = reqwest::Certificate::from_der(&bytes) {
                builder = builder.tls_certs_merge([certificate]);
            }
        }
    }
    builder
}
