//! Chromium connects only through a local SOCKS5 gateway. DNS is resolved and checked once,
//! then the gateway connects to that exact IP. The gateway never implements UDP or BIND.
use crate::permissions::{is_private_ip, DomainPolicy};
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

pub struct GuardedRenderer {
    renderer: os_render::Renderer,
    gateway: tokio::task::JoinHandle<()>,
}
impl Drop for GuardedRenderer {
    fn drop(&mut self) {
        self.gateway.abort();
    }
}
impl os_engine::Renderer for GuardedRenderer {
    fn render<'a>(
        &'a self,
        url: &'a str,
        opts: &'a os_render::RenderOpts,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<os_render::RenderedPage, os_render::RenderError>,
                > + Send
                + 'a,
        >,
    > {
        Box::pin(self.renderer.render(url, opts))
    }
}
pub async fn launch(
    cfg: &os_model::CrawlConfig,
    policy: DomainPolicy,
) -> Result<Arc<dyn os_engine::Renderer>, String> {
    let (port, gateway) = gateway(policy, &cfg.network.resolve)
        .await
        .map_err(|e| e.to_string())?;
    let mut opts = os_render::LaunchOpts {
        chromium_path: cfg.rendering.chromium_path.as_ref().map(Into::into),
        headless: true,
        protected_network: true,
        ..Default::default()
    };
    opts.network.proxy = os_model::settings::ProxySettings {
        mode: os_model::ProxyMode::Manual,
        kind: os_model::ProxyKind::Socks5,
        host: "127.0.0.1".into(),
        port,
        username: None,
        bypass: vec![],
    };
    // Chromium must not resolve targets itself (SOCKS carries the original host).
    opts.host_resolver_rules = vec!["MAP * ~NOTFOUND , EXCLUDE 127.0.0.1".into()];
    match os_render::Renderer::launch(opts).await {
        Ok(renderer) => Ok(Arc::new(GuardedRenderer { renderer, gateway })),
        Err(e) => {
            gateway.abort();
            Err(e.to_string())
        }
    }
}
async fn gateway(
    policy: DomainPolicy,
    rules: &[String],
) -> std::io::Result<(u16, tokio::task::JoinHandle<()>)> {
    let resolver = Arc::new(
        os_fetch::resolve::HostResolver::new(rules, Duration::from_secs(10))
            .map_err(std::io::Error::other)?,
    );
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let port = listener.local_addr()?.port();
    let task = tokio::spawn(async move {
        let mut connections = tokio::task::JoinSet::new();
        let limit = Arc::new(tokio::sync::Semaphore::new(64));
        loop {
            tokio::select! {
                accepted = listener.accept() => {
                    let Ok((stream, _)) = accepted else { break };
                    let Ok(permit) = limit.clone().try_acquire_owned() else { continue };
                    let policy = policy.clone();
                    let resolver = resolver.clone();
                    connections.spawn(async move {
                        let _permit = permit;
                        let _ = tokio::time::timeout(Duration::from_secs(3600), connection(stream, policy, resolver)).await;
                    });
                }
                _ = connections.join_next(), if !connections.is_empty() => {}
            }
        }
    });
    Ok((port, task))
}
fn invalid() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Blocked MCP target")
}
async fn connection(
    mut stream: TcpStream,
    policy: DomainPolicy,
    resolver: Arc<os_fetch::resolve::HostResolver>,
) -> std::io::Result<()> {
    let connected = tokio::time::timeout(Duration::from_secs(10), async {
        if stream.read_u8().await? != 5 {
            return Err(invalid());
        }
        let count = stream.read_u8().await?;
        let mut methods = vec![0; count as usize];
        stream.read_exact(&mut methods).await?;
        if !methods.contains(&0) {
            return Err(invalid());
        }
        stream.write_all(&[5, 0]).await?;
        let mut header = [0; 4];
        stream.read_exact(&mut header).await?;
        if header[..3] != [5, 1, 0] {
            return Err(invalid());
        }
        let host = match header[3] {
            1 => {
                let mut bytes = [0; 4];
                stream.read_exact(&mut bytes).await?;
                Ipv4Addr::from(bytes).to_string()
            }
            4 => {
                let mut bytes = [0; 16];
                stream.read_exact(&mut bytes).await?;
                format!("[{}]", Ipv6Addr::from(bytes))
            }
            3 => {
                let len = stream.read_u8().await?;
                let mut bytes = vec![0; len as usize];
                stream.read_exact(&mut bytes).await?;
                String::from_utf8(bytes).map_err(|_| invalid())?
            }
            _ => return Err(invalid()),
        };
        let port = stream.read_u16().await?;
        if host.contains(['/', '@', '#', '?', '\\']) {
            return Err(invalid());
        }
        policy
            .check_url(&format!("http://{host}:{port}/"))
            .map_err(|_| invalid())?;
        let host = host.trim_matches(['[', ']']);
        let addresses = if let Ok(ip) = host.parse::<IpAddr>() {
            vec![std::net::SocketAddr::new(ip, port)]
        } else if let Some(addresses) = resolver.lookup_override(host) {
            addresses
                .into_iter()
                .map(|a| {
                    std::net::SocketAddr::new(a.ip(), if a.port() == 0 { port } else { a.port() })
                })
                .collect()
        } else {
            tokio::net::lookup_host((host, port))
                .await?
                .collect::<Vec<_>>()
        };
        if addresses.is_empty()
            || (!policy.allows_private_targets() && addresses.iter().any(|a| is_private_ip(a.ip())))
        {
            return Err(invalid());
        }
        // SocketAddr, never the hostname: a second DNS lookup cannot rebind this connection.
        TcpStream::connect(addresses.as_slice()).await
    })
    .await
    .map_err(|_| invalid())?;
    match connected {
        Ok(mut remote) => {
            stream.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
            tokio::io::copy_bidirectional(&mut stream, &mut remote).await?;
        }
        Err(e) => {
            let _ = stream.write_all(&[5, 2, 0, 1, 0, 0, 0, 0, 0, 0]).await;
            return Err(e);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    // spec: AGENT-020 [M2]
    #[tokio::test]
    async fn chromium_gateway_rejects_private_dns_and_ips_and_only_connects_when_enabled() {
        let target = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let address = target.local_addr().unwrap();
        for allowed in [false, true] {
            let (port, task) = gateway(DomainPolicy::new(&[], allowed), &[]).await.unwrap();
            let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port))
                .await
                .unwrap();
            stream.write_all(&[5, 1, 0]).await.unwrap();
            let mut greeting = [0; 2];
            stream.read_exact(&mut greeting).await.unwrap();
            assert_eq!(greeting, [5, 0]);
            let mut request = vec![5, 1, 0, 3, 9];
            request.extend(b"localhost");
            request.extend(address.port().to_be_bytes());
            stream.write_all(&request).await.unwrap();
            let mut response = [0; 10];
            stream.read_exact(&mut response).await.unwrap();
            assert_eq!(response[1], if allowed { 0 } else { 2 });
            if allowed {
                let (mut peer, _) = target.accept().await.unwrap();
                stream.write_all(b"fixture").await.unwrap();
                let mut bytes = [0; 7];
                peer.read_exact(&mut bytes).await.unwrap();
                assert_eq!(&bytes, b"fixture");
            } else {
                assert!(
                    tokio::time::timeout(Duration::from_millis(30), target.accept())
                        .await
                        .is_err()
                );
            }
            task.abort();
        }
    }
}
