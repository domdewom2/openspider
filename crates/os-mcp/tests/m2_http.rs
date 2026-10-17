use os_fetch::secrets::MemorySecretStore;
use os_mcp::{
    http::{HttpServer, TokenStore},
    permissions::Permissions,
    server::McpServer,
    tools, McpSettings, ToolContext,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn request(
    address: std::net::SocketAddr,
    token: Option<&str>,
    origin: Option<&str>,
    host: Option<&str>,
    method: &str,
    session: Option<&str>,
    body: Value,
) -> String {
    let body = body.to_string();
    let mut header=format!("{method} /mcp HTTP/1.1\r\nHost: {}\r\nAccept: application/json, text/event-stream\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n",host.map(str::to_owned).unwrap_or_else(||format!("127.0.0.1:{}",address.port())),body.len());
    if let Some(token) = token {
        header.push_str(&format!("Authorization: Bearer {token}\r\n"));
    }
    if let Some(origin) = origin {
        header.push_str(&format!("Origin: {origin}\r\n"));
    }
    if let Some(session) = session {
        header.push_str(&format!(
            "Mcp-Session-Id: {session}\r\nMCP-Protocol-Version: 2025-06-18\r\n"
        ));
    }
    header.push_str("\r\n");
    header.push_str(&body);
    let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
    stream.write_all(header.as_bytes()).await.unwrap();
    let mut bytes = Vec::new();
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        stream.read_to_end(&mut bytes),
    )
    .await
    .unwrap()
    .unwrap();
    String::from_utf8(bytes).unwrap()
}
fn status(response: &str) -> u16 {
    response.split_whitespace().nth(1).unwrap().parse().unwrap()
}
fn init() -> Value {
    json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"m01-http-test","version":"1"}}})
}
fn server(dir: &std::path::Path) -> McpServer {
    McpServer::new(
        tools::all(),
        Permissions::resolve(&[], false, &McpSettings::default()).unwrap(),
        ToolContext::new(dir.to_path_buf()),
    )
}

// spec: AGENT-002
// spec: OUT-285 [P1]
#[tokio::test]
async fn streamable_http_requires_token_origin_host_on_both_loopbacks_and_after_rotation() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(MemorySecretStore::new());
    let tokens = TokenStore::open(store.clone()).unwrap();
    let token = tokens.copy_token().unwrap();
    assert_eq!(token.len(), 64);
    assert!(!format!("{tokens:?}").contains(&token));
    assert_eq!(
        TokenStore::open(store).unwrap().copy_token().unwrap(),
        token
    );
    let http = HttpServer::bind(server(dir.path()), 0, tokens.clone())
        .await
        .unwrap();
    let addr = ([127, 0, 0, 1], http.port()).into();
    let fixtures: Vec<Value> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/agent-m2-http/requests.json"
    ))
    .unwrap();
    for case in fixtures {
        let credential = match case["token"].as_str() {
            Some("missing") => None,
            Some("wrong") => Some("wrong"),
            _ => Some(token.as_str()),
        };
        let host = case["host"]
            .as_str()
            .map(|s| s.replace("PORT", &http.port().to_string()));
        let response = request(
            addr,
            credential,
            case["origin"].as_str(),
            host.as_deref(),
            "POST",
            None,
            init(),
        )
        .await;
        assert_eq!(
            status(&response),
            case["status"].as_u64().unwrap() as u16,
            "{case}: {response}"
        );
    }
    let response = request(addr, Some(&token), None, None, "POST", None, init()).await;
    let session = response
        .lines()
        .find_map(|l| {
            l.to_ascii_lowercase()
                .strip_prefix("mcp-session-id:")
                .map(|s| s.trim().to_owned())
        })
        .expect("session ID after initialize");
    assert!(response.contains("openspider"));
    let initialized = json!({"jsonrpc":"2.0","method":"notifications/initialized"});
    assert_eq!(
        status(
            &request(
                addr,
                Some(&token),
                None,
                None,
                "POST",
                Some(&session),
                initialized
            )
            .await
        ),
        202
    );
    let list = json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}});
    let response = request(addr, Some(&token), None, None, "POST", Some(&session), list).await;
    assert_eq!(status(&response), 200);
    assert!(response.contains("sql_query"));
    assert!(!response.contains("start_crawl"));
    let response = request(
        ([0, 0, 0, 0, 0, 0, 0, 1], http.port()).into(),
        Some(&token),
        Some("http://[::1]"),
        Some(&format!("[::1]:{}", http.port())),
        "POST",
        None,
        init(),
    )
    .await;
    assert_eq!(status(&response), 200, "{response}");
    // Binding is to 127.0.0.1 specifically, never INADDR_ANY (nor another local address).
    // Some kernels silently drop SYNs to unassigned loopback aliases instead of refusing them.
    let other_loopback = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        tokio::net::TcpStream::connect((std::net::Ipv4Addr::new(127, 0, 0, 2), http.port())),
    )
    .await;
    assert!(!matches!(other_loopback, Ok(Ok(_))));
    let new = tokens.rotate().unwrap();
    assert_ne!(new, token);
    for method in ["POST", "GET", "DELETE"] {
        assert_eq!(
            status(
                &request(
                    addr,
                    Some(&token),
                    None,
                    None,
                    method,
                    Some(&session),
                    json!({})
                )
                .await
            ),
            401
        );
    }
    assert_eq!(
        status(
            &request(
                addr,
                Some(&new),
                None,
                None,
                "DELETE",
                Some(&session),
                json!({})
            )
            .await
        ),
        202
    );
    let port = http.port();
    http.stop().await;
    assert!(
        tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port))
            .await
            .is_err()
    );
}

// spec: AGENT-002
#[tokio::test]
async fn occupied_port_fails_without_a_partially_started_server() {
    let dir = tempfile::tempdir().unwrap();
    let occupied = tokio::net::TcpListener::bind((std::net::Ipv6Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let port = occupied.local_addr().unwrap().port();
    let token = TokenStore::open(Arc::new(MemorySecretStore::new())).unwrap();
    assert!(HttpServer::bind(server(dir.path()), port, token)
        .await
        .is_err());
    let _reusable = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
        .await
        .unwrap();
}
