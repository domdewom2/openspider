//! Chrome has no portable per-profile custom root store. For a configured
//! truststore, fulfill HTTPS requests through the shared, validating HTTP client.
//! Chromium still runs JS and owns cookies/cache/navigation; TLS verification is
//! never globally disabled. This also works when servers omit their root CA.
use super::{protocol, Credentials, LaunchOpts, MainResponse, RenderError};
use os_model::{settings::NetworkSettings, ProxyKind, ProxyMode};
use serde_json::Value;
use std::time::Duration;

pub fn validate(network: &NetworkSettings) -> Result<(), RenderError> {
    for path in &network.trusted_certificates {
        let data = std::fs::read(path).map_err(|e| protocol(format!("Truststore {path}: {e}")))?;
        let valid = if data.starts_with(b"-----BEGIN") {
            reqwest::Certificate::from_pem_bundle(&data).is_ok_and(|certs| !certs.is_empty())
        } else {
            reqwest::Certificate::from_der(&data).is_ok()
        };
        if !valid {
            return Err(protocol(format!(
                "Ungültiges Truststore-Zertifikat: {path}"
            )));
        }
    }
    Ok(())
}

pub async fn fetch(
    launch: &LaunchOpts,
    request: &Value,
    origin: &str,
    credentials: Option<&Credentials>,
    response_timeout_ms: u32,
) -> Result<MainResponse, RenderError> {
    let url = reqwest::Url::parse(
        request["url"]
            .as_str()
            .ok_or_else(|| protocol("URL fehlt"))?,
    )
    .map_err(protocol)?;
    let mut builder = os_fetch::net::client_builder(&launch.network)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_millis(response_timeout_ms.into()));
    let proxy = &launch.network.proxy;
    if proxy.mode == ProxyMode::Manual {
        let scheme = match proxy.kind {
            ProxyKind::Http => "http",
            ProxyKind::Https => "https",
            ProxyKind::Socks5 => "socks5h",
        };
        let host = if proxy.host.contains(':') && !proxy.host.starts_with('[') {
            format!("[{}]", proxy.host)
        } else {
            proxy.host.clone()
        };
        let mut p = reqwest::Proxy::all(format!("{scheme}://{host}:{}", proxy.port))
            .map_err(protocol)?
            .no_proxy(reqwest::NoProxy::from_string(&proxy.bypass.join(",")));
        if let Some(c) = &launch.proxy_credentials {
            p = p.basic_auth(&c.username, &c.password);
        }
        builder = builder.no_proxy().proxy(p);
    }
    // Mirror Chromium MAP rules for this origin, including wildcard test hosts.
    for rule in &launch.host_resolver_rules {
        let words: Vec<_> = rule.split_whitespace().collect();
        if words.len() == 3 && words[0].eq_ignore_ascii_case("MAP") {
            if let Some(host) = url.host_str() {
                let matches = words[1] == host
                    || words[1]
                        .strip_prefix("*.")
                        .is_some_and(|suffix| host.ends_with(&format!(".{suffix}")));
                if matches {
                    let addr = words[2].parse::<std::net::SocketAddr>().ok().or_else(|| {
                        words[2].parse::<std::net::IpAddr>().ok().map(|ip| {
                            std::net::SocketAddr::new(
                                ip,
                                url.port_or_known_default().unwrap_or(443),
                            )
                        })
                    });
                    if let Some(addr) = addr {
                        builder = builder.resolve(host, addr);
                    }
                }
            }
        }
    }
    let client = builder.build().map_err(protocol)?;
    let method =
        reqwest::Method::from_bytes(request["method"].as_str().unwrap_or("GET").as_bytes())
            .map_err(protocol)?;
    let build_request = || {
        let mut req = client.request(method.clone(), url.clone());
        if let Some(headers) = request["headers"].as_object() {
            for (key, value) in headers {
                if let Some(value) = value.as_str() {
                    req = req.header(key, value);
                }
            }
        }
        if let Some(body) = request["postData"].as_str() {
            req = req.body(body.to_owned());
        }
        req
    };
    let mut response = build_request().send().await.map_err(protocol)?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED
        && url.origin().ascii_serialization() == origin
    {
        if let Some(c) = credentials {
            response = build_request()
                .basic_auth(&c.username, Some(&c.password))
                .send()
                .await
                .map_err(protocol)?;
        }
    }
    let status = response.status().as_u16();
    if status == 407 {
        return Err(RenderError::ProxyAuthentication);
    }
    let headers = response
        .headers()
        .iter()
        .filter(|(k, _)| !["transfer-encoding", "connection"].contains(&k.as_str()))
        .map(|(k, v)| {
            (
                k.as_str().to_owned(),
                v.to_str().unwrap_or_default().to_owned(),
            )
        })
        .collect();
    let body = response.bytes().await.map_err(protocol)?.to_vec();
    Ok(MainResponse {
        status,
        headers,
        body,
    })
}

pub fn needs_bridge(launch: &LaunchOpts, url: &str) -> bool {
    (url.starts_with("https://") && !launch.network.trusted_certificates.is_empty())
        || ((url.starts_with("https://") || url.starts_with("http://"))
            && launch.network.proxy.mode == ProxyMode::Manual
            && launch.network.proxy.kind == ProxyKind::Socks5
            && launch.proxy_credentials.is_some())
}
