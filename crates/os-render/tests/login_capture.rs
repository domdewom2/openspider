use os_render::{
    login::{LoginOpts, LoginSession},
    LaunchOpts, RenderOpts, Renderer,
};
use os_testsite::mocks::form_login::FormLoginSite;
use std::time::Duration;

// spec: CRAWL-120
#[tokio::test]
async fn form_and_two_factor_capture_http_only_cookies_and_both_storages() {
    let site = FormLoginSite::start().await.unwrap();
    for path in ["/login", "/login-2fa"] {
        let mut login = LoginSession::open(
            &site.url(path),
            LoginOpts {
                headless: true,
                user_agent: Some("OpenSpider-Login-Fixture".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        for _ in 0..100 {
            if login
                .page()
                .evaluate("!!document.querySelector('form')")
                .await
                .ok()
                .and_then(|v| v.into_value::<bool>().ok())
                == Some(true)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        login.page().evaluate("document.querySelector('[name=user]').value='fixture';document.querySelector('[name=password]').value='secret';document.querySelector('[name=code]').value='123456';document.querySelector('form').requestSubmit();").await.unwrap();
        let captured = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let Ok(session) = login.capture().await {
                    if session.final_url.ends_with("/mitglieder/")
                        && session
                            .storage
                            .iter()
                            .any(|s| s.local_storage.contains_key("token"))
                    {
                        let loaded = login
                            .page()
                            .evaluate("document.readyState !== 'loading'")
                            .await
                            .ok()
                            .and_then(|v| v.into_value::<bool>().ok())
                            == Some(true);
                        if loaded {
                            break session;
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(captured.cookies.len(), 1);
        assert!(captured.cookies[0].http_only);
        assert_eq!(captured.cookies[0].value, "COOKIE-FIXTURE");
        assert!(captured
            .storage
            .iter()
            .any(|s| s.session_storage["token"] == "SESSION-FIXTURE"));
        assert!(!format!("{captured:?}").contains("FIXTURE"));
        assert!(site
            .requests()
            .iter()
            .filter(|(p, _)| p.starts_with("/login"))
            .all(|(_, ua)| ua == "OpenSpider-Login-Fixture"));
        login.close().await;
        let renderer = Renderer::launch(LaunchOpts::default()).await.unwrap();
        let opts = RenderOpts {
            login_storage: captured.storage.clone(),
            ajax_timeout_ms: 100,
            screenshots: false,
            ..Default::default()
        };
        let rendered = renderer
            .render(
                &site.url("/mitglieder/app"),
                &RenderOpts {
                    cookies: captured
                        .cookies
                        .iter()
                        .map(|c| os_render::api::Cookie {
                            name: c.name.clone(),
                            value: c.value.clone(),
                            domain: Some(c.domain.clone()),
                            path: Some(c.path.clone()),
                            secure: c.secure,
                            http_only: c.http_only,
                            expires: c.expires,
                        })
                        .collect(),
                    ..opts.clone()
                },
            )
            .await
            .unwrap();
        assert!(rendered.dom_html.contains("<title>Token ok</title>"));
        let foreign = renderer
            .render(
                &site.url("/storage-probe").replace("127.0.0.1", "localhost"),
                &opts,
            )
            .await
            .unwrap();
        assert!(foreign.dom_html.contains("<title>Token missing</title>"));
        let isolated = renderer
            .render(
                &site.url("/storage-probe"),
                &RenderOpts {
                    login_storage: vec![],
                    ..opts
                },
            )
            .await
            .unwrap();
        assert!(isolated.dom_html.contains("<title>Token missing</title>"));
        renderer.close().await;
    }
}

// spec: CRAWL-120
#[tokio::test]
async fn capture_expiry_is_earliest_cookie_and_rejects_non_web_urls() {
    assert!(LoginSession::open("file:///tmp/test", LoginOpts::default())
        .await
        .is_err());
    let session = os_render::login::CapturedSession {
        cookies: vec![os_render::login::LoginCookie {
            name: "a".into(),
            value: "secret".into(),
            domain: "site.test".into(),
            path: "/".into(),
            secure: true,
            http_only: true,
            expires: Some(100),
        }],
        ..Default::default()
    };
    assert!(!session.expired(99));
    assert!(session.expired(100));
}

// spec: CRAWL-120
#[tokio::test]
async fn renewing_profile_restores_each_origin_once_and_keeps_rotated_or_removed_tokens() {
    use os_render::login::{CapturedSession, OriginStorage};
    let site = FormLoginSite::start().await.unwrap();
    let origin = site.url("");
    let foreign = origin.replace("127.0.0.1", "localhost");
    let initial = CapturedSession {
        storage: [origin.clone(), foreign.clone()]
            .into_iter()
            .map(|origin| OriginStorage {
                origin,
                local_storage: [("token".into(), "OLD-LOCAL".into())].into(),
                session_storage: [("token".into(), "OLD-SESSION".into())].into(),
            })
            .collect(),
        ..Default::default()
    };
    let mut login = LoginSession::open(
        &site.url("/login-2fa"),
        LoginOpts {
            headless: true,
            initial: Some(initial),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    wait_for_token(&mut login, "OLD-LOCAL", &site.url("/login-2fa")).await;
    login.page().evaluate("document.querySelector('[name=user]').value='fixture';document.querySelector('[name=password]').value='secret';document.querySelector('[name=code]').value='123456';document.querySelector('form').requestSubmit();").await.unwrap();
    let captured = wait_for_token(&mut login, "LOCAL-FIXTURE", &site.url("/mitglieder/")).await;
    assert!(captured.final_url.ends_with("/mitglieder/"));
    assert_eq!(
        captured.storage[0].session_storage["token"],
        "SESSION-FIXTURE"
    );
    login
        .page()
        .goto(format!("{foreign}/storage-probe"))
        .await
        .unwrap();
    wait_for_token(&mut login, "OLD-LOCAL", &format!("{foreign}/storage-probe")).await;
    login.page().goto(site.url("/storage-probe")).await.unwrap();
    wait_for_token(&mut login, "LOCAL-FIXTURE", &site.url("/storage-probe")).await;
    // Clearing storage is also a legitimate session change; no bootstrap marker remains.
    login
        .page()
        .evaluate(
            "localStorage.clear();sessionStorage.clear();location.href='/storage-probe?cleared';",
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(session) = login.capture().await {
                if session.final_url.ends_with("?cleared") {
                    let storage = session.storage.iter().find(|s| s.origin == origin).unwrap();
                    assert!(storage.local_storage.is_empty());
                    assert!(storage.session_storage.is_empty());
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    login.close().await;
}
async fn wait_for_token(
    login: &mut LoginSession,
    token: &str,
    url: &str,
) -> os_render::login::CapturedSession {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(session) = login.capture().await {
                if session.final_url == url
                    && session.storage.iter().any(|s| {
                        session.final_url.starts_with(&s.origin)
                            && s.local_storage.get("token").is_some_and(|t| t == token)
                    })
                {
                    let loaded = login
                        .page()
                        .evaluate("document.readyState !== 'loading'")
                        .await
                        .ok()
                        .and_then(|v| v.into_value::<bool>().ok())
                        == Some(true);
                    if loaded {
                        break session;
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap()
}
