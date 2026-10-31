//! Connection-bound NTLM fixtures; no external network or credential store.
#[path = "http_support.rs"]
mod support;

use base64::{engine::general_purpose::STANDARD, Engine};
use os_fetch::{BasicAuthEntry, FetchConfig, HttpFetcher, MemorySecretStore};
use os_model::{CrawlConfig, SystemClock};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use support::*;

async fn server() -> MiniServer {
    let state = Mutex::new(HashMap::<usize, (u8, String)>::new());
    // MS-NLMP challenge fields with a terminating target-info AV pair.
    let mut challenge = vec![0; 52];
    challenge[..12].copy_from_slice(b"NTLMSSP\0\x02\0\0\0");
    challenge[20..24].copy_from_slice(&0x0088_0201u32.to_le_bytes());
    challenge[24..32].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
    challenge[40..44].copy_from_slice(&[4, 0, 4, 0]);
    challenge[44..48].copy_from_slice(&48u32.to_le_bytes());
    let challenge = format!("NTLM {}", STANDARD.encode(challenge));
    MiniServer::start(move |req| {
        let mut states = state.lock().unwrap();
        let (stage, user) = states.entry(req.conn).or_default();
        if req.path() == "/public" {
            assert!(req.header("authorization").is_none());
            assert!(
                user.is_empty(),
                "authenticated connection escaped its credential scope"
            );
            return ok_text("public");
        }
        let auth = req.header("authorization");
        if *stage == 2 {
            assert!(
                auth.is_none(),
                "reuse must rely on connection authentication"
            );
            return authenticated_reply(req, user);
        }
        let header = if *stage == 0 {
            assert!(
                auth.is_none(),
                "NTLM credentials were sent before a challenge"
            );
            *stage = 1;
            "NTLM"
        } else {
            let message = STANDARD
                .decode(auth.unwrap().strip_prefix("NTLM ").unwrap())
                .unwrap();
            match u32::from_le_bytes(message[8..12].try_into().unwrap()) {
                1 => &challenge,
                3 => {
                    let length =
                        usize::from(u16::from_le_bytes(message[36..38].try_into().unwrap()));
                    let offset = u32::from_le_bytes(message[40..44].try_into().unwrap()) as usize;
                    let chars: Vec<_> = message[offset..offset + length]
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|c| u16::from_le_bytes(*c))
                        .collect();
                    *user = String::from_utf16(&chars).unwrap();
                    *stage = 2;
                    return authenticated_reply(req, user);
                }
                other => panic!("unexpected NTLM message {other}"),
            }
        };
        // Nonempty challenge bodies must be drained before the next handshake leg.
        Reply::Raw(response(
            401,
            "Unauthorized",
            &[("www-authenticate", header)],
            b"challenge body",
        ))
    })
    .await
}

fn authenticated_reply(req: &Request, user: &str) -> Reply {
    if req.path().starts_with("/slow") {
        Reply::Trickle {
            head: head_only(200, "OK", &[], user.len()),
            body: user.as_bytes().to_vec(),
            interval: Duration::from_secs(3),
        }
    } else {
        ok_text(user)
    }
}

fn pool_config(threads: u32, per_host: Option<u32>) -> FetchConfig {
    let mut crawl = CrawlConfig::default();
    crawl.speed.threads = threads;
    crawl.speed.max_per_host = per_host;
    FetchConfig::from_crawl_config(&crawl, &MemorySecretStore::new(), Arc::new(SystemClock))
        .unwrap()
}

fn profile(server: &MiniServer, path: &str, user: &str) -> BasicAuthEntry {
    BasicAuthEntry {
        prefix: server.url(path).to_string(),
        username: user.into(),
        password: "Password".into(),
    }
}

async fn check(fetcher: &HttpFetcher, server: &MiniServer, path: &str, user: &str) {
    let response = expect_response(get(fetcher, server.url(path)).await);
    assert_eq!(response.status, 200);
    assert_eq!(response.body.as_ref(), user.as_bytes());
}

// spec: CRAWL-121 [M3]
#[tokio::test]
async fn repeated_and_parallel_requests_reuse_the_origin_connection() {
    let server = server().await;
    let fetcher = HttpFetcher::new(pool_config(5, Some(2)))
        .unwrap()
        .with_challenge_auth(&[], &[profile(&server, "/", "User")])
        .unwrap();
    tokio::join!(
        check(&fetcher, &server, "/first", "User"),
        check(&fetcher, &server, "/third", "User"),
        check(&fetcher, &server, "/second", "User")
    );
    check(&fetcher, &server, "/fourth", "User").await;
    let connections = server.connections();
    assert!((1..=2).contains(&connections));
    for _ in 0..4 {
        check(&fetcher, &server, "/again", "User").await;
    }
    assert_eq!(
        server.connections(),
        connections,
        "reuse existing connections"
    );
    assert_eq!(server.requests().len(), 8 + 2 * connections);
}

// spec: CRAWL-121 [M3]
#[tokio::test]
async fn configured_parallel_slow_bodies_are_not_serialized() {
    let server = server().await;
    // No explicit host limit: the configured thread count is the effective limit.
    let cfg = pool_config(3, None);
    assert_eq!(cfg.max_parallel_per_host, 3);
    let fetcher = HttpFetcher::new(cfg)
        .unwrap()
        .with_challenge_auth(&[], &[profile(&server, "/", "User")])
        .unwrap();
    // Each body takes 12s to stream. Three serialized bodies take at least 36s;
    // 25s allows over twice the parallel duration for a busy build server.
    tokio::time::timeout(Duration::from_secs(25), async {
        tokio::join!(
            check(&fetcher, &server, "/slow/one", "User"),
            check(&fetcher, &server, "/slow/two", "User"),
            check(&fetcher, &server, "/slow/three", "User")
        );
    })
    .await
    .expect("configured host concurrency must include body streaming");
    assert_eq!(server.connections(), 3);
    assert_eq!(pool_config(2, Some(5)).max_parallel_per_host, 2);
    assert_eq!(pool_config(500, Some(500)).max_parallel_per_host, 200);
}

// spec: CRAWL-121 [M3]
#[tokio::test]
async fn origins_and_credential_scopes_never_share_authentication() {
    let first = server().await;
    let second = server().await;
    let fetcher = HttpFetcher::new(config(Arc::new(SystemClock)))
        .unwrap()
        .with_challenge_auth(
            &[],
            &[
                profile(&first, "/a/", "Alice"),
                profile(&first, "/b/", "Bob"),
                profile(&second, "/a/", "Carol"),
            ],
        )
        .unwrap();
    tokio::join!(
        check(&fetcher, &first, "/a/one", "Alice"),
        check(&fetcher, &first, "/b/one", "Bob"),
        check(&fetcher, &second, "/a/one", "Carol")
    );
    check(&fetcher, &first, "/a/two", "Alice").await;
    check(&fetcher, &first, "/b/two", "Bob").await;
    check(&fetcher, &first, "/public", "public").await;
    assert_eq!(first.connections(), 3);
    assert_eq!(second.connections(), 1);
    // Replacing the profile must discard its authenticated pool, including an unchanged prefix.
    let fetcher = fetcher
        .with_challenge_auth(&[], &[profile(&first, "/a/", "Dave")])
        .unwrap();
    check(&fetcher, &first, "/a/three", "Dave").await;
    assert_eq!(first.connections(), 4);
}
