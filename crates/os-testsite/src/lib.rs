//! `os-testsite` – lokaler Fixture-Server (axum) für deterministische Tests.
//!
//! Der Server liefert ein Verzeichnis `tests/fixtures/<szenario>/` statisch aus. Optional
//! beschreibt eine `scenario.toml` im Szenario-Verzeichnis programmatische Antworten
//! (Statuscodes, Header, Weiterleitungen, Verzögerungen):
//!
//! ```toml
//! [[route]]
//! path = "/old-page"
//! status = 301                 # optional; mit `location` ist der Standard 302, sonst 200
//! location = "/about.html"     # setzt den Location-Header
//! body = "..."                 # optional; ohne body wird die statische Datei (falls vorhanden) genutzt
//! content_type = "text/html"   # optional
//! delay_ms = 250               # optional
//! [route.headers]
//! X-Robots-Tag = "noindex"
//! ```
//!
//! In textuellen statischen Dateien (html, xml, txt, css, js, json) wird der Platzhalter
//! `{{BASE_URL}}` durch die Basis-URL des laufenden Servers ersetzt.
//!
//! # Synthetische Sites
//!
//! Enthält die `scenario.toml` eine Sektion `[synthetic]` (oder wird die Kurzform
//! `synthetic:n=1000000,links=30` übergeben), erzeugt der Server die Site deterministisch
//! on-the-fly aus `(seed, Pfad)` – ohne Dateien auf Platte, siehe [`synthetic`]:
//!
//! ```toml
//! [synthetic]
//! pages = 1000000
//! links_per_page = 30
//! seed = 42
//! depth_distribution = [10, 100, 1000, 10000]  # relative Gewichte je Ebene ab Tiefe 1
//! status_301_share = 0.02    # Anteile an allen Seiten (0..1)
//! status_404_share = 0.01
//! status_5xx_share = 0.005
//! noindex_share = 0.05
//! canonical_other_share = 0.03
//! duplicate_title_share = 0.10
//! large_page_share = 0.01
//! large_page_kb = 200
//! delay_ms = 250             # zusammen mit slow_share
//! slow_share = 0.01
//! ```
//!
//! Mit [`SiteOptions::log_requests`] zählt der Server Requests je Pfad
//! ([`TestSite::request_counts`], als JSON unter [`synthetic::REQUEST_LOG_PATH`]).

pub mod client;
pub mod synthetic;

use std::collections::{BTreeMap, HashMap};
use std::io;
use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderName, HeaderValue, Request, Response, StatusCode};
use axum::Router;
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

pub use synthetic::{SyntheticConfig, SyntheticSite};

/// Dateiname der optionalen Szenario-Beschreibung.
pub const SCENARIO_FILE: &str = "scenario.toml";

/// Platzhalter, der in textuellen Fixture-Dateien durch die Basis-URL ersetzt wird.
pub const BASE_URL_PLACEHOLDER: &str = "{{BASE_URL}}";

/// Programmatisch definierte Antwort für einen Pfad.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    /// Exakter URL-Pfad (ohne Query), z. B. `/old-page`.
    pub path: String,
    /// HTTP-Statuscode.
    pub status: Option<u16>,
    /// Ziel für den `Location`-Header (Weiterleitung).
    pub location: Option<String>,
    /// Zusätzliche Antwort-Header.
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    /// Antwort-Body; ohne Angabe wird die statische Datei zum Pfad verwendet.
    pub body: Option<String>,
    /// Wert für den `Content-Type`-Header.
    pub content_type: Option<String>,
    /// Künstliche Verzögerung in Millisekunden.
    pub delay_ms: Option<u64>,
}

/// Inhalt einer `scenario.toml`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    /// Alle `[[route]]`-Einträge.
    #[serde(default, rename = "route")]
    pub routes: Vec<Route>,
    /// Optionale synthetische Site (`[synthetic]`); ersetzt die statischen Dateien.
    pub synthetic: Option<SyntheticConfig>,
}

impl Scenario {
    /// Parst eine `scenario.toml`.
    pub fn parse(text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(text)
    }
}

/// Wurzelverzeichnis der Fixtures (`<workspace>/tests/fixtures`).
pub fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

/// Optionen beim Start eines Servers.
#[derive(Debug, Clone, Default)]
pub struct SiteOptions {
    /// Port; `0` wählt einen freien Port.
    pub port: u16,
    /// Requests je Pfad zählen.
    pub log_requests: bool,
}

struct AppState {
    dir: PathBuf,
    base_url: String,
    routes: BTreeMap<String, Route>,
    synthetic: Option<SyntheticSite>,
    log: Option<Mutex<HashMap<String, u64>>>,
}

/// Laufender Testserver. Wird beim Drop beendet.
pub struct TestSite {
    addr: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    handle: JoinHandle<()>,
    state: Arc<AppState>,
}

impl TestSite {
    /// Startet das Szenario `tests/fixtures/<name>` auf einem zufälligen freien Port.
    pub async fn start(name: &str) -> io::Result<Self> {
        Self::start_dir(fixtures_root().join(name), 0).await
    }

    /// Startet einen Server für ein beliebiges Verzeichnis; `port = 0` wählt einen freien Port.
    pub async fn start_dir(dir: impl Into<PathBuf>, port: u16) -> io::Result<Self> {
        Self::start_dir_with(
            dir,
            SiteOptions {
                port,
                ..Default::default()
            },
        )
        .await
    }

    /// Wie [`TestSite::start_dir`], aber mit [`SiteOptions`].
    pub async fn start_dir_with(dir: impl Into<PathBuf>, opts: SiteOptions) -> io::Result<Self> {
        let dir: PathBuf = dir.into();
        if !dir.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Szenario-Verzeichnis nicht gefunden: {}", dir.display()),
            ));
        }
        let scenario = match std::fs::read_to_string(dir.join(SCENARIO_FILE)) {
            Ok(text) => Scenario::parse(&text)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => Scenario::default(),
            Err(e) => return Err(e),
        };

        let synthetic = scenario
            .synthetic
            .map(SyntheticSite::new)
            .transpose()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let routes = scenario
            .routes
            .into_iter()
            .map(|r| (r.path.clone(), r))
            .collect();
        Self::launch(dir, routes, synthetic, opts).await
    }

    /// Startet eine synthetische Site ohne Dateien auf Platte.
    pub async fn start_synthetic(cfg: SyntheticConfig, opts: SiteOptions) -> io::Result<Self> {
        let site =
            SyntheticSite::new(cfg).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Self::launch(PathBuf::new(), BTreeMap::new(), Some(site), opts).await
    }

    /// Startet anhand einer Szenario-Angabe: `synthetic:n=1000,links=30` (Kurzform) oder
    /// der Name eines Fixture-Verzeichnisses unter `tests/fixtures`.
    pub async fn start_spec(spec: &str, opts: SiteOptions) -> io::Result<Self> {
        if spec.starts_with("synthetic:") {
            let cfg = SyntheticConfig::parse_short(spec)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
            Self::start_synthetic(cfg, opts).await
        } else {
            Self::start_dir_with(fixtures_root().join(spec), opts).await
        }
    }

    async fn launch(
        dir: PathBuf,
        routes: BTreeMap<String, Route>,
        synthetic: Option<SyntheticSite>,
        opts: SiteOptions,
    ) -> io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", opts.port)).await?;
        let addr = listener.local_addr()?;
        let state = Arc::new(AppState {
            dir,
            base_url: format!("http://{addr}"),
            routes,
            synthetic,
            log: opts.log_requests.then(|| Mutex::new(HashMap::new())),
        });

        let app = Router::new()
            .fallback(handle)
            .with_state(Arc::clone(&state));
        let (tx, rx) = oneshot::channel::<()>();
        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = rx.await;
                })
                .await;
        });

        Ok(Self {
            addr,
            shutdown: Some(tx),
            handle,
            state,
        })
    }

    /// Socket-Adresse des Servers.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Basis-URL ohne abschließenden Slash, z. B. `http://127.0.0.1:34567`.
    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Vollständige URL für einen Pfad (`/index.html`).
    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url(), path)
    }

    /// Die synthetische Site, falls dieses Szenario eine ist.
    pub fn synthetic(&self) -> Option<&SyntheticSite> {
        self.state.synthetic.as_ref()
    }

    /// Requests je Pfad (leer, wenn das Logging nicht aktiviert wurde).
    pub fn request_counts(&self) -> BTreeMap<String, u64> {
        self.state
            .log
            .as_ref()
            .map(|m| lock(m).iter().map(|(k, v)| (k.clone(), *v)).collect())
            .unwrap_or_default()
    }

    /// Gesamtzahl geloggter Requests.
    pub fn total_requests(&self) -> u64 {
        self.state
            .log
            .as_ref()
            .map_or(0, |m| lock(m).values().sum())
    }

    /// Pfade, die mehr als einmal abgerufen wurden (Duplikat-Abrufe), mit Anzahl.
    pub fn duplicate_requests(&self) -> Vec<(String, u64)> {
        self.request_counts()
            .into_iter()
            .filter(|(_, c)| *c > 1)
            .collect()
    }

    /// Setzt das Request-Log zurück.
    pub fn reset_request_log(&self) {
        if let Some(m) = &self.state.log {
            lock(m).clear();
        }
    }
}

fn lock(m: &Mutex<HashMap<String, u64>>) -> std::sync::MutexGuard<'_, HashMap<String, u64>> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Drop for TestSite {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        self.handle.abort();
    }
}

async fn handle(State(state): State<Arc<AppState>>, req: Request<Body>) -> Response<Body> {
    let path = req.uri().path().to_string();

    if let Some(log) = &state.log {
        if path == synthetic::REQUEST_LOG_PATH {
            return request_log_response(log);
        }
        let mut map = lock(log);
        if let Some(c) = map.get_mut(&path) {
            *c += 1;
        } else {
            map.insert(path.clone(), 1);
        }
    }

    if let Some(site) = &state.synthetic {
        return handle_synthetic(site, &state.base_url, &path).await;
    }

    let route = state.routes.get(&path);

    if let Some(ms) = route.and_then(|r| r.delay_ms) {
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }

    let file = static_file(&state, &path).await;

    let mut status = StatusCode::OK;
    let mut content_type: Option<String> = None;
    let mut body: Vec<u8> = Vec::new();
    let mut extra_headers: Vec<(String, String)> = Vec::new();

    match (route, file) {
        (Some(route), file) => {
            status = match (route.status, &route.location) {
                (Some(code), _) => StatusCode::from_u16(code).unwrap_or(StatusCode::OK),
                (None, Some(_)) => StatusCode::FOUND,
                (None, None) => StatusCode::OK,
            };
            if let Some(loc) = &route.location {
                extra_headers.push((header::LOCATION.to_string(), loc.clone()));
            }
            extra_headers.extend(route.headers.iter().map(|(k, v)| (k.clone(), v.clone())));
            if let Some(text) = &route.body {
                body = text.clone().into_bytes();
                content_type = Some("text/html; charset=utf-8".to_string());
            } else if let Some((ct, bytes)) = file {
                content_type = Some(ct);
                body = bytes;
            }
            if let Some(ct) = &route.content_type {
                content_type = Some(ct.clone());
            }
        }
        (None, Some((ct, bytes))) => {
            content_type = Some(ct);
            body = bytes;
        }
        (None, None) => {
            status = StatusCode::NOT_FOUND;
            content_type = Some("text/plain; charset=utf-8".to_string());
            body = b"404 Not Found".to_vec();
        }
    }

    let mut builder = Response::builder().status(status);
    if let Some(ct) = content_type {
        builder = builder.header(header::CONTENT_TYPE, ct);
    }
    for (name, value) in extra_headers {
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(&value),
        ) {
            builder = builder.header(name, value);
        }
    }
    builder
        .body(Body::from(body))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

fn request_log_response(log: &Mutex<HashMap<String, u64>>) -> Response<Body> {
    let counts: BTreeMap<String, u64> = lock(log).iter().map(|(k, v)| (k.clone(), *v)).collect();
    let total: u64 = counts.values().sum();
    let json = serde_json::json!({ "total": total, "paths": counts });
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json.to_string()))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

async fn handle_synthetic(site: &SyntheticSite, base_url: &str, path: &str) -> Response<Body> {
    let Some(rendered) = site.render(path, base_url) else {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from("404 Not Found"))
            .unwrap_or_else(|_| Response::new(Body::empty()));
    };
    if let Some(delay) = rendered.delay {
        tokio::time::sleep(delay).await;
    }
    let mut builder = Response::builder()
        .status(StatusCode::from_u16(rendered.status).unwrap_or(StatusCode::OK))
        .header(header::CONTENT_TYPE, rendered.content_type);
    if let Some(location) = &rendered.location {
        builder = builder.header(header::LOCATION, location.as_str());
    }
    builder
        .body(Body::from(rendered.body))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

/// Liest die statische Datei zu einem URL-Pfad. Gibt `(Content-Type, Bytes)` zurück.
async fn static_file(state: &AppState, url_path: &str) -> Option<(String, Vec<u8>)> {
    let mut rel = PathBuf::new();
    for segment in url_path.split('/').filter(|s| !s.is_empty()) {
        // Nur normale Segmente zulassen (kein "..", keine Laufwerks-/Backslash-Tricks).
        let mut comps = Path::new(segment).components();
        match (comps.next(), comps.next()) {
            (Some(Component::Normal(_)), None) if !segment.contains(['\\', ':']) => {
                rel.push(segment)
            }
            _ => return None,
        }
    }
    if url_path.ends_with('/') || rel.as_os_str().is_empty() {
        rel.push("index.html");
    }
    if rel == Path::new(SCENARIO_FILE) {
        return None;
    }
    let full = state.dir.join(&rel);
    let bytes = tokio::fs::read(&full).await.ok()?;
    let content_type = content_type_for(&full);
    let bytes = if is_text(content_type) {
        match String::from_utf8(bytes) {
            Ok(text) => text
                .replace(BASE_URL_PLACEHOLDER, &state.base_url)
                .into_bytes(),
            Err(e) => e.into_bytes(),
        }
    } else {
        bytes
    };
    Some((content_type.to_string(), bytes))
}

fn is_text(content_type: &str) -> bool {
    content_type.starts_with("text/")
        || content_type.starts_with("application/xml")
        || content_type.starts_with("application/json")
        || content_type.starts_with("application/javascript")
}

fn content_type_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("json") => "application/json",
        Some("xml") => "application/xml",
        Some("txt") => "text/plain; charset=utf-8",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scenario_toml() {
        let scenario = Scenario::parse(
            r#"
            [[route]]
            path = "/old"
            status = 301
            location = "/new"
            [route.headers]
            X-Test = "1"
            "#,
        )
        .unwrap();
        assert_eq!(scenario.routes.len(), 1);
        assert_eq!(scenario.routes[0].status, Some(301));
        assert_eq!(scenario.routes[0].headers["X-Test"], "1");
    }

    #[test]
    fn content_types() {
        assert_eq!(
            content_type_for(Path::new("a.html")),
            "text/html; charset=utf-8"
        );
        assert_eq!(
            content_type_for(Path::new("sitemap.xml")),
            "application/xml"
        );
    }
}
