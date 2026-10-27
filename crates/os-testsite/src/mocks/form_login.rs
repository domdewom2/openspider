//! Opt-in CRAWL-120 fixture: real form submission, two-factor code and revocable server session.
use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    Router,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

#[derive(Clone, Default)]
struct Session {
    active: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<(String, String)>>>,
}
pub struct FormLoginSite {
    base: String,
    session: Session,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for FormLoginSite {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl FormLoginSite {
    pub async fn start() -> std::io::Result<Self> {
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
        let base = format!("http://{}", listener.local_addr()?);
        let session = Session::default();
        let router = Router::new().fallback(handle).with_state(session.clone());
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        Ok(Self {
            base,
            session,
            task,
        })
    }
    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }
    pub fn requests(&self) -> Vec<(String, String)> {
        self.session.requests.lock().unwrap().clone()
    }
    pub fn clear_requests(&self) {
        self.session.requests.lock().unwrap().clear();
    }
}
async fn handle(State(s): State<Session>, req: Request) -> Response {
    let path = req.uri().path().to_owned();
    let query = req.uri().query().unwrap_or_default().to_owned();
    let cookie = req
        .headers()
        .get(header::COOKIE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let ua = req
        .headers()
        .get(header::USER_AGENT)
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    s.requests.lock().unwrap().push((
        format!(
            "{path}{}",
            if query.is_empty() {
                String::new()
            } else {
                format!("?{query}")
            }
        ),
        ua,
    ));
    if path == "/robots.txt" {
        return (StatusCode::OK, "User-agent: *\nDisallow:\n").into_response();
    }
    if path == "/login" || path == "/login-2fa" {
        let two_factor = path == "/login-2fa";
        return Html(format!(r#"<!doctype html><title>Sign in</title><form><label>User<input name=user></label><label>Password<input name=password type=password></label><label>Code<input name=code></label><button>Sign in</button></form><script>
        document.querySelector('form').onsubmit=async e=>{{e.preventDefault();const f=new FormData(e.target); const r=await fetch('/session?twoFactor={two_factor}',{{method:'POST',body:new URLSearchParams(f)}});if(r.ok){{localStorage.token='LOCAL-FIXTURE';sessionStorage.token='SESSION-FIXTURE';location.href='/mitglieder/';}}}};</script>"#)).into_response();
    }
    if path == "/session" {
        let body = axum::body::to_bytes(req.into_body(), 4096)
            .await
            .unwrap_or_default();
        let body = String::from_utf8_lossy(&body);
        if !body.contains("user=fixture")
            || !body.contains("password=secret")
            || (query == "twoFactor=true" && !body.contains("code=123456"))
        {
            return StatusCode::UNAUTHORIZED.into_response();
        }
        s.active.store(true, Ordering::SeqCst);
        return (
            [(
                header::SET_COOKIE,
                "session=COOKIE-FIXTURE; Path=/; HttpOnly; SameSite=Lax",
            )],
            "ok",
        )
            .into_response();
    }
    if path == "/storage-probe" {
        return Html("<!doctype html><title>Token missing</title><script>document.title=localStorage.token==='LOCAL-FIXTURE'&&sessionStorage.token==='SESSION-FIXTURE'?'Token ok':'Token missing';</script>").into_response();
    }
    if path == "/logout" {
        s.active.store(false, Ordering::SeqCst);
        return (
            [(header::SET_COOKIE, "session=; Path=/; Max-Age=0")],
            "Logged out",
        )
            .into_response();
    }
    if path.starts_with("/mitglieder/") || path.starts_with("/konto/") {
        if !cookie
            .split(';')
            .any(|c| c.trim() == "session=COOKIE-FIXTURE")
            || !s.active.load(Ordering::SeqCst)
        {
            return (StatusCode::FOUND, [(header::LOCATION, "/login")]).into_response();
        }
        if path == "/mitglieder/app" {
            return Html("<!doctype html><title>Token missing</title><script>document.title=localStorage.token==='LOCAL-FIXTURE'&&sessionStorage.token==='SESSION-FIXTURE'?'Token ok':'Token missing';</script>").into_response();
        }
        let links = if path == "/mitglieder/" {
            (0..20)
                .map(|n| format!("<a href='/mitglieder/{n}'>Member {n}</a>"))
                .collect::<String>()
        } else {
            String::new()
        };
        return Html(format!("<!doctype html><title>Members</title><a href='/logout'>Log out</a><a href='/konto/loeschen?id=1'>Delete</a><a href='/mitglieder/app'>App</a>{links}")).into_response();
    }
    StatusCode::NOT_FOUND.into_response()
}
