//! Connection-time MCP target protection. Every request (including robots/resources/redirects)
//! uses a briefly cached, validated DNS answer and connects to those same addresses.
use std::collections::VecDeque;
use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use os_fetch::http::{FetchOutcome, FetchRequest, FetchResult};
use os_fetch::{Fetch, FetchConfig, HttpFetcher};
use os_model::{Clock, NoResponseReason, SystemClock};

use crate::permissions::{is_private_ip, DomainPolicy};

const CACHE_CAPACITY: usize = 64;
const DNS_TTL: Duration = Duration::from_secs(2);
type AddressResult = Result<Vec<SocketAddr>, (NoResponseReason, String)>;
type ClientKey = (String, u16, Vec<SocketAddr>);

pub(crate) struct ProtectedFetcher {
    inner: HttpFetcher,
    resolver: os_fetch::resolve::HostResolver,
    allow_private: bool,
    policy: DomainPolicy,
    addresses: tokio::sync::Mutex<VecDeque<(String, Instant, AddressResult)>>,
    clients: Mutex<VecDeque<(ClientKey, Arc<HttpFetcher>)>>,
}

impl ProtectedFetcher {
    pub fn new(
        cfg: FetchConfig,
        policy: DomainPolicy,
        allow_private: bool,
    ) -> Result<Self, String> {
        let resolver = os_fetch::resolve::HostResolver::new(&cfg.resolve, cfg.response_timeout)?;
        Ok(Self {
            inner: HttpFetcher::new(cfg).map_err(|e| e.to_string())?,
            resolver,
            allow_private,
            policy,
            addresses: Default::default(),
            clients: Default::default(),
        })
    }

    async fn addresses(
        &self,
        req: &FetchRequest,
    ) -> Result<Vec<SocketAddr>, (NoResponseReason, String)> {
        // Share approved answers between policy hooks and fetch without serializing unrelated DNS.
        let mut cache = self.addresses.lock().await;
        cache.retain(|(_, at, _)| at.elapsed() < DNS_TTL);
        let url = req.url.as_str();
        if let Some(index) = cache.iter().position(|(key, _, _)| key == url) {
            let entry = cache.remove(index).unwrap();
            let result = entry.2.clone();
            cache.push_back(entry);
            return result;
        }
        drop(cache);
        let result = self.resolve_addresses(req).await;
        let mut cache = self.addresses.lock().await;
        cache.retain(|(key, at, _)| key != url && at.elapsed() < DNS_TTL);
        if cache.len() == CACHE_CAPACITY {
            cache.pop_front();
        }
        cache.push_back((url.to_owned(), Instant::now(), result.clone()));
        result
    }

    fn client(
        &self,
        req: &FetchRequest,
        mut addrs: Vec<SocketAddr>,
    ) -> Result<Arc<HttpFetcher>, String> {
        addrs.sort_unstable();
        addrs.dedup();
        let host = req.url.host_str().unwrap_or("").to_owned();
        let key = (host, req.url.port_or_known_default().unwrap_or(80), addrs);
        let mut cache = self.clients.lock().map_err(|e| e.to_string())?;
        if let Some(index) = cache.iter().position(|(cached, _)| *cached == key) {
            let entry = cache.remove(index).unwrap();
            let client = entry.1.clone();
            cache.push_back(entry);
            return Ok(client);
        }
        let client = Arc::new(
            self.inner
                .pinned_client(&key.0, &key.2)
                .map_err(|e| e.to_string())?,
        );
        if cache.len() == CACHE_CAPACITY {
            cache.pop_front();
        }
        cache.push_back((key, client.clone()));
        Ok(client)
    }

    async fn resolve_addresses(&self, req: &FetchRequest) -> AddressResult {
        // Normalized URL comes from the engine, so noncanonical IP spellings are normalized too.
        self.policy
            .check_url(req.url.as_str())
            .map_err(|e| (NoResponseReason::BlockedPrivateTarget, e))?;
        let host = req.url.host_str().unwrap_or("").trim_matches(['[', ']']);
        let port = req.url.port_or_known_default().unwrap_or(80);
        let addrs = if let Ok(ip) = host.parse::<IpAddr>() {
            vec![SocketAddr::new(ip, port)]
        } else if let Some(addrs) = self.resolver.lookup_override(host) {
            addrs
                .into_iter()
                .map(|a| {
                    SocketAddr::new(
                        a.ip(),
                        req.url
                            .port()
                            .unwrap_or(if a.port() == 0 { port } else { a.port() }),
                    )
                })
                .collect()
        } else {
            tokio::time::timeout(
                Duration::from_secs(10),
                tokio::net::lookup_host((host, port)),
            )
            .await
            .map_err(|_| (NoResponseReason::ConnectionTimeout, "DNS timeout".into()))?
            .map_err(|e| (NoResponseReason::DnsLookupFailed, e.to_string()))?
            .collect()
        };
        if !self.allow_private && addrs.iter().any(|a| is_private_ip(a.ip())) {
            return Err((
                NoResponseReason::BlockedPrivateTarget,
                "Blocked (private target) after DNS resolution".into(),
            ));
        }
        Ok(addrs)
    }
}

impl Fetch for ProtectedFetcher {
    fn target_denial<'a>(
        &'a self,
        url: &'a os_fetch::http::FetchUrl,
    ) -> Pin<Box<dyn Future<Output = Option<FetchResult>> + Send + 'a>> {
        Box::pin(async move {
            let req = FetchRequest {
                url: url.clone(),
                purpose: os_fetch::http::FetchPurpose::Page,
                cookies: None,
                max_body_bytes: 0,
                attempt: 1,
                body_limit: Default::default(),
            };
            match self.addresses(&req).await {
                Err((NoResponseReason::BlockedPrivateTarget, detail)) => Some(FetchResult {
                    url: url.clone(),
                    started_at: SystemClock.now(),
                    outcome: FetchOutcome::NoResponse {
                        reason: NoResponseReason::BlockedPrivateTarget,
                        detail,
                        elapsed_ms: 0,
                    },
                }),
                _ => None,
            }
        })
    }

    fn fetch(&self, req: FetchRequest) -> Pin<Box<dyn Future<Output = FetchResult> + Send + '_>> {
        Box::pin(async move {
            let started_at = SystemClock.now();
            let failure = match self.addresses(&req).await {
                Ok(addrs) => match self.client(&req, addrs) {
                    Ok(client) => return self.inner.fetch_with_pinned(req, &client).await,
                    Err(e) => (NoResponseReason::ConnectionError, e.to_string()),
                },
                Err(failure) => failure,
            };
            FetchResult {
                url: req.url,
                started_at,
                outcome: FetchOutcome::NoResponse {
                    reason: failure.0,
                    detail: failure.1,
                    elapsed_ms: 0,
                },
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_fetch::http::{BodyLimit, FetchPurpose};
    use os_fetch::secrets::MemorySecretStore;
    use os_model::CrawlConfig;
    use std::sync::Arc;
    fn request(url: &str, purpose: FetchPurpose) -> FetchRequest {
        FetchRequest {
            url: url.parse().unwrap(),
            purpose,
            cookies: None,
            max_body_bytes: 1024 * 1024,
            attempt: 1,
            body_limit: BodyLimit::default(),
        }
    }
    fn protected(cfg: &CrawlConfig, allow_private: bool) -> ProtectedFetcher {
        let config =
            FetchConfig::from_crawl_config(cfg, &MemorySecretStore::new(), Arc::new(SystemClock))
                .unwrap();
        ProtectedFetcher::new(config, DomainPolicy::new(&[], allow_private), allow_private).unwrap()
    }
    // spec: AGENT-020
    #[tokio::test]
    async fn approved_addresses_and_connections_are_reused_but_expire_and_change_safely() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let connections = Arc::new(AtomicUsize::new(0));
        let count = connections.clone();
        let server = tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                count.fetch_add(1, Ordering::SeqCst);
                tokio::spawn(async move {
                    let mut byte = [0];
                    loop {
                        let mut header = Vec::new();
                        while !header.ends_with(b"\r\n\r\n") {
                            if stream.read_exact(&mut byte).await.is_err() {
                                return;
                            }
                            header.push(byte[0]);
                        }
                        if stream
                            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }
                });
            }
        });
        let mut cfg = CrawlConfig::default();
        cfg.network.resolve = vec![format!("public.test={addr}")];
        let guard = protected(&cfg, true);
        let req = request("http://public.test/", FetchPurpose::Page);
        guard.target_denial(&req.url).await;
        let at = guard.addresses.lock().await[0].1;
        guard.target_denial(&req.url).await;
        for _ in 0..3 {
            assert!(matches!(
                guard.fetch(req.clone()).await.outcome,
                FetchOutcome::Response(_)
            ));
        }
        assert_eq!(
            guard.addresses.lock().await[0].1,
            at,
            "hooks and fetch use one lookup"
        );
        assert_eq!(
            connections.load(Ordering::SeqCst),
            1,
            "keep-alive reuses the TCP connection"
        );
        let client = guard.client(&req, vec![addr]).unwrap();
        assert!(Arc::ptr_eq(
            &client,
            &guard.client(&req, vec![addr, addr]).unwrap()
        ));
        assert!(!Arc::ptr_eq(
            &client,
            &guard
                .client(
                    &req,
                    vec![SocketAddr::new(addr.ip(), addr.port().saturating_sub(1))]
                )
                .unwrap()
        ));
        guard.addresses.lock().await[0].1 = Instant::now() - DNS_TTL;
        guard.addresses(&req).await.unwrap();
        assert!(guard.addresses.lock().await[0].1 > at);
        for port in 1..=70 {
            guard
                .client(&req, vec![SocketAddr::new(addr.ip(), port)])
                .unwrap();
            guard
                .addresses(&request(
                    &format!("http://public.test/{port}"),
                    FetchPurpose::Page,
                ))
                .await
                .unwrap();
        }
        assert_eq!(guard.clients.lock().unwrap().len(), CACHE_CAPACITY);
        assert_eq!(guard.addresses.lock().await.len(), CACHE_CAPACITY);
        server.abort();
    }

    // spec: AGENT-020
    #[tokio::test]
    async fn dns_overrides_and_every_fetch_purpose_are_guarded_before_any_request() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("tests")
            .join("fixtures")
            .join("agent-m2-crawl");
        let site = os_testsite::TestSite::start_dir(path, 0).await.unwrap();
        let mut cfg = CrawlConfig::default();
        cfg.network.resolve = vec![format!("public.test={}", site.addr())];
        let guard = protected(&cfg, false);
        for purpose in [
            FetchPurpose::Page,
            FetchPurpose::External,
            FetchPurpose::Resource,
            FetchPurpose::RobotsTxt,
            FetchPurpose::Sitemap,
        ] {
            let result = guard.fetch(request("http://public.test/", purpose)).await;
            assert!(matches!(
                result.outcome,
                FetchOutcome::NoResponse {
                    reason: NoResponseReason::BlockedPrivateTarget,
                    ..
                }
            ));
        }
        assert!(site.requests().is_empty());
        for ip in [
            "127.0.0.1",
            "127.255.255.254",
            "10.1.2.3",
            "172.16.1.2",
            "172.31.255.1",
            "192.168.1.2",
            "169.254.169.254",
            "100.64.0.1",
            "100.127.255.254",
            "0.0.0.0",
            "[::]",
            "[::1]",
            "[fc00::1]",
            "[fdff::1]",
            "[fe80::1]",
            "[::ffff:127.0.0.1]",
        ] {
            let req = request(&format!("http://{ip}/"), FetchPurpose::Page);
            assert!(guard.addresses(&req).await.is_err(), "{ip}");
        }
        let allowed = protected(&cfg, true);
        let result = allowed
            .fetch(request("http://public.test/", FetchPurpose::Page))
            .await;
        assert!(matches!(result.outcome,FetchOutcome::Response(ref r) if r.status==200));
        assert_eq!(site.requests().last().unwrap().host, "public.test");
        let mut tls =
            FetchConfig::from_crawl_config(&cfg, &MemorySecretStore::new(), Arc::new(SystemClock))
                .unwrap();
        tls.extra_root_certs = vec![site.ca_cert_pem().into_bytes()];
        let tls = ProtectedFetcher::new(tls, DomainPolicy::new(&[], true), true).unwrap();
        let response = tls
            .fetch(request("https://public.test/", FetchPurpose::Page))
            .await;
        assert!(matches!(response.outcome,FetchOutcome::Response(ref r) if r.status==200));
        let last = site.requests().last().unwrap().clone();
        assert!(last.tls);
        assert_eq!(last.sni.as_deref(), Some("public.test"));
        // Reject a mixed public/private DNS answer as a whole, before opening any socket.
        cfg.network.resolve = vec![
            "public.test=93.184.216.34".into(),
            format!("public.test={}", site.addr()),
        ];
        assert!(protected(&cfg, false)
            .addresses(&request("http://public.test/", FetchPurpose::Page))
            .await
            .is_err());
    }

    // spec: AGENT-020
    #[tokio::test]
    async fn engine_redirect_to_metadata_is_blocked_and_persisted_with_no_request() {
        // A synthetic public response provides the redirect without needing an external network.
        // Every subsequent URL is passed to the production protected fetcher, including resources.
        struct PublicRedirect {
            fixture: HttpFetcher,
            guard: ProtectedFetcher,
        }
        impl Fetch for PublicRedirect {
            fn target_denial<'a>(
                &'a self,
                url: &'a os_fetch::http::FetchUrl,
            ) -> Pin<Box<dyn Future<Output = Option<FetchResult>> + Send + 'a>> {
                if url.host_str() == Some("public.test") {
                    Box::pin(async { None })
                } else {
                    self.guard.target_denial(url)
                }
            }
            fn fetch(
                &self,
                req: FetchRequest,
            ) -> Pin<Box<dyn Future<Output = FetchResult> + Send + '_>> {
                Box::pin(async move {
                    if req.url.host_str() == Some("public.test") {
                        self.fixture.fetch(req).await
                    } else {
                        self.guard.fetch(req).await
                    }
                })
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("tests")
            .join("fixtures")
            .join("agent-m2-crawl");
        let site = os_testsite::TestSite::start_dir(path, 0).await.unwrap();
        let mut cfg = CrawlConfig::default();
        cfg.network.resolve = vec![format!("public.test={}", site.addr())];
        let guard = protected(&cfg, false);
        let deps = os_engine::testing::test_deps();
        let config =
            FetchConfig::from_crawl_config(&cfg, &*deps.secrets, deps.clock.clone()).unwrap();
        let fetcher = Arc::new(PublicRedirect {
            fixture: HttpFetcher::new(config).unwrap(),
            guard,
        });
        let file = dir.path().join("crawl.ospider");
        let req = os_engine::CrawlRequest {
            config: cfg,
            source: os_engine::CrawlSource::Spider {
                start_url: "http://public.test/redirect".into(),
            },
            file: file.clone(),
            project: String::new(),
            name: "redirect guard".into(),
        };
        os_engine::start_crawl(fetcher, req, deps)
            .await
            .unwrap()
            .wait()
            .await
            .unwrap();
        let crawl = os_store::CrawlFile::open(&file, os_store::OpenMode::ReadOnly).unwrap();
        let (reason, status): (String, String) = crawl
            .conn()
            .query_row(
                "SELECT no_response_reason, status FROM urls WHERE url LIKE '%169.254.169.254%'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(reason, "blocked_private_target");
        assert_eq!(status, "Blocked (private target)");
        assert!(site.requests().iter().all(|r| r.host == "public.test"));
    }
}
