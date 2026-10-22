//! Streamable HTTP on both loopbacks with mandatory Bearer, Host and Origin checks (AGENT-002).
use crate::{server::McpServer, McpError};
use axum::extract::{Request, State};
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::Response;
use os_fetch::secrets::SecretStoreMut;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpListener;

/// Secret-store key shared with the app; the value is never persisted in mcp.toml.
pub const TOKEN_KEY: &str = "mcp/token";

/// Token lifecycle. Clones share the store and rotation lock; Debug never includes the token.
#[derive(Clone)]
pub struct TokenStore {
    inner: Arc<TokenInner>,
}
struct TokenInner {
    store: Arc<dyn SecretStoreMut>,
    token: Mutex<String>,
}
impl std::fmt::Debug for TokenStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TokenStore(<redacted>)")
    }
}
fn failure(e: impl std::fmt::Display) -> McpError {
    McpError::Transport(e.to_string())
}
impl TokenStore {
    /// Load a token or generate 256 bits with the OS random source and save through SecretStoreMut.
    pub fn open(store: Arc<dyn SecretStoreMut>) -> Result<Self, McpError> {
        let token = store.try_get(TOKEN_KEY).map_err(failure)?;
        if token
            .as_ref()
            .is_some_and(|s| s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err(failure("Invalid stored MCP token; regenerate it"));
        }
        let missing = token.is_none();
        let this = Self {
            inner: Arc::new(TokenInner {
                store,
                token: Mutex::new(token.unwrap_or_default()),
            }),
        };
        if missing {
            this.rotate()?;
        }
        Ok(this)
    }
    /// Explicit local UI operation; never exposed as an MCP tool.
    pub fn copy_token(&self) -> Result<String, McpError> {
        self.inner
            .token
            .lock()
            .map(|token| token.clone())
            .map_err(failure)
    }
    /// Generate a new token; the next request immediately rejects the old value.
    pub fn rotate(&self) -> Result<String, McpError> {
        let mut current = self.inner.token.lock().map_err(failure)?;
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(failure)?;
        let token = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        self.inner.store.set(TOKEN_KEY, &token).map_err(failure)?;
        *current = token.clone();
        Ok(token)
    }
    fn accepts(&self, value: &str) -> bool {
        let Ok(expected) = self.inner.token.lock() else {
            return false;
        };
        if value.len() != expected.len() {
            return false;
        }
        value
            .bytes()
            .zip(expected.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
    }
}

#[derive(Clone)]
struct Guard {
    token: TokenStore,
    port: u16,
}
fn single<'a>(headers: &'a HeaderMap, key: &str) -> Option<&'a str> {
    let mut values = headers.get_all(key).iter();
    let value = values.next()?.to_str().ok()?;
    if values.next().is_some() {
        None
    } else {
        Some(value)
    }
}
fn loopback(host: &str) -> bool {
    matches!(
        host.to_ascii_lowercase().as_str(),
        "localhost" | "127.0.0.1" | "[::1]"
    )
}
fn authorize(headers: &HeaderMap, guard: &Guard) -> Result<(), StatusCode> {
    let token = single(headers, "authorization").and_then(|s| s.strip_prefix("Bearer "));
    if !token.is_some_and(|token| guard.token.accepts(token)) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let host = single(headers, "host").and_then(|s| s.parse::<axum::http::uri::Authority>().ok());
    if !host.is_some_and(|a| {
        loopback(a.host()) && a.port_u16() == Some(guard.port) && !a.as_str().contains('@')
    }) {
        return Err(StatusCode::FORBIDDEN);
    }
    if headers.contains_key("origin") {
        let origin = single(headers, "origin").and_then(|s| s.parse::<Uri>().ok());
        if !origin.is_some_and(|u| {
            matches!(u.scheme_str(), Some("http" | "https"))
                && u.authority()
                    .is_some_and(|a| loopback(a.host()) && !a.as_str().contains('@'))
                && u.path_and_query()
                    .is_none_or(|p| p.as_str() == "/" || p.as_str().is_empty())
        }) {
            return Err(StatusCode::FORBIDDEN);
        }
    }
    Ok(())
}
async fn check(
    State(guard): State<Guard>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    authorize(request.headers(), &guard)?;
    Ok(next.run(request).await)
}

/// Running HTTP listeners. Dropping aborts both listeners; use stop for graceful shutdown.
pub struct HttpServer {
    port: u16,
    shutdown: tokio::sync::watch::Sender<bool>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
    cancel: Box<dyn Fn() + Send + Sync>,
    manager: Arc<LocalSessionManager>,
}
impl HttpServer {
    /// Bind both loopbacks, or fail atomically. Port 0 chooses an ephemeral port.
    pub async fn bind(server: McpServer, port: u16, token: TokenStore) -> Result<Self, McpError> {
        let v4 = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port)))
            .await
            .map_err(failure)?;
        let port = v4.local_addr().map_err(failure)?.port();
        let v6 = TcpListener::bind(SocketAddr::from((Ipv6Addr::LOCALHOST, port)))
            .await
            .map_err(|e| failure(format!("IPv6-Loopback nicht verfügbar ([::1]:{port}): {e}")))?;
        let mut manager = LocalSessionManager::default();
        manager.session_config.keep_alive = Some(Duration::from_secs(600));
        let mut config = StreamableHttpServerConfig::default();
        config.allowed_hosts = vec![
            format!("127.0.0.1:{port}"),
            format!("localhost:{port}"),
            format!("[::1]:{port}"),
        ];
        config.allowed_origins = ["http", "https"]
            .iter()
            .flat_map(|s| ["localhost", "127.0.0.1", "[::1]"].map(move |h| format!("{s}://{h}:*")))
            .collect();
        let cancellation = config.cancellation_token.clone();
        let manager = Arc::new(manager);
        let service =
            StreamableHttpService::new(move || Ok(server.clone()), manager.clone(), config);
        let router = axum::Router::new()
            .nest_service("/mcp", service)
            .layer(middleware::from_fn_with_state(Guard { token, port }, check));
        let (shutdown, rx) = tokio::sync::watch::channel(false);
        let tasks = [v4, v6]
            .into_iter()
            .map(|listener| {
                let router = router.clone();
                let mut rx = rx.clone();
                let cancellation = cancellation.clone();
                tokio::spawn(async move {
                    let result = axum::serve(listener, router)
                        .with_graceful_shutdown(async move {
                            let _ = rx.changed().await;
                            cancellation.cancel();
                        })
                        .await;
                    if let Err(e) = result {
                        tracing::error!(error=%e,"MCP HTTP listener failed");
                    }
                })
            })
            .collect();
        Ok(Self {
            port,
            manager,
            shutdown,
            tasks,
            cancel: Box::new(move || cancellation.cancel()),
        })
    }
    /// Active transport sessions, maintained by rmcp (DELETE, idle expiry and disconnect).
    pub async fn clients(&self) -> u32 {
        self.manager
            .sessions
            .read()
            .await
            .len()
            .min(u32::MAX as usize) as u32
    }
    /// Port shared by the two listeners.
    pub fn port(&self) -> u16 {
        self.port
    }
    /// Stop accepting requests and drain current requests (at most one second).
    pub async fn stop(mut self) {
        (self.cancel)();
        let _ = self.shutdown.send(true);
        for mut task in self.tasks.drain(..) {
            if tokio::time::timeout(Duration::from_secs(1), &mut task)
                .await
                .is_err()
            {
                task.abort();
            }
        }
    }
}
impl Drop for HttpServer {
    fn drop(&mut self) {
        (self.cancel)();
        let _ = self.shutdown.send(true);
        for task in &self.tasks {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_fetch::secrets::{MemorySecretStore, SecretBackend, SecretError, SecretStore};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    #[derive(Default)]
    struct CountingStore {
        memory: MemorySecretStore,
        reads: AtomicUsize,
        fail_write: AtomicBool,
    }
    impl SecretStore for CountingStore {
        fn get(&self, key: &str) -> Option<String> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            self.memory.get(key)
        }
    }
    impl SecretStoreMut for CountingStore {
        fn set(&self, key: &str, value: &str) -> Result<(), SecretError> {
            if self.fail_write.load(Ordering::SeqCst) {
                return Err(SecretError::new("write failed"));
            }
            self.memory.set(key, value);
            Ok(())
        }
        fn delete(&self, key: &str) -> Result<(), SecretError> {
            SecretStoreMut::delete(&self.memory, key)
        }
        fn backend(&self) -> SecretBackend {
            SecretBackend::Memory
        }
    }
    // spec: AGENT-002
    #[test]
    fn requests_only_read_memory_and_rotation_updates_store_and_all_clones_atomically() {
        let store = Arc::new(CountingStore::default());
        let tokens = TokenStore::open(store.clone()).unwrap();
        let clone = tokens.clone();
        let old = tokens.copy_token().unwrap();
        for _ in 0..10 {
            assert!(clone.accepts(&old));
        }
        assert_eq!(store.reads.load(Ordering::SeqCst), 1);
        store.fail_write.store(true, Ordering::SeqCst);
        assert!(tokens.rotate().is_err());
        assert!(clone.accepts(&old));
        store.fail_write.store(false, Ordering::SeqCst);
        let new = tokens.rotate().unwrap();
        assert!(!clone.accepts(&old));
        assert!(clone.accepts(&new));
        assert_eq!(store.reads.load(Ordering::SeqCst), 1);
        assert_eq!(TokenStore::open(store).unwrap().copy_token().unwrap(), new);
    }
}
