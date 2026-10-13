//! Antwort-Aufbau: aus Request, Route und statischen Dateien wird eine [`Reply`].

use std::path::{Component, Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::http::StatusCode;

use crate::media::png_of_size;
use crate::{
    synthetic, AppState, Echo, Route, BASE_URL_PLACEHOLDER, HOSTS_DIR, PORT2_PLACEHOLDER,
    PORT_PLACEHOLDER, SCENARIO_FILE,
};

/// Ein geparster Request.
pub(crate) struct Request {
    pub method: String,
    /// Request-Ziel (Pfad plus Query).
    pub target: String,
    /// Pfad ohne Query.
    pub path: String,
    /// Host-Header in Kleinbuchstaben ohne Port.
    pub host: String,
    pub headers: Vec<(String, String)>,
    pub tls: bool,
}

/// Wie die Antwort ausgeliefert wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Normal,
    Hang,
    AbortAfterHeaders,
    ResetMidBody,
}

/// Eine 1xx-Zwischenantwort, die vor der finalen Antwort gesendet wird.
pub(crate) struct Informational {
    pub status: u16,
    pub reason: String,
    pub headers: Vec<(String, String)>,
}

/// Die fertig berechnete Antwort (noch ohne Framing-Header).
pub(crate) struct Reply {
    /// 1xx-Zwischenantworten in Sendereihenfolge (Route-Option `informational`).
    pub informational: Vec<Informational>,
    pub status: u16,
    pub reason: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub chunked: bool,
    pub trickle: Option<u64>,
    pub mode: Mode,
}

impl Reply {
    fn new(status: u16) -> Self {
        Self {
            informational: Vec::new(),
            status,
            reason: canonical_reason(status),
            headers: Vec::new(),
            body: Vec::new(),
            chunked: false,
            trickle: None,
            mode: Mode::Normal,
        }
    }

    fn header(&mut self, name: &str, value: impl Into<String>) {
        self.headers.push((name.to_string(), value.into()));
    }
}

pub(crate) fn canonical_reason(status: u16) -> String {
    StatusCode::from_u16(status)
        .ok()
        .and_then(|s| s.canonical_reason())
        .unwrap_or("")
        .to_string()
}

/// Berechnet die Antwort (inklusive künstlicher Verzögerung).
pub(crate) async fn build_reply(state: &AppState, req: &Request) -> Reply {
    if let Some(site) = &state.synthetic {
        return synthetic_reply(site, &state.base_url, &req.path).await;
    }

    let decoded = percent_decode(&req.path);
    let route_idx = state.find_route(&req.host, &req.path).or_else(|| {
        decoded
            .as_deref()
            .and_then(|d| state.find_route(&req.host, d))
    });
    let route = route_idx.map(|i| &state.routes[i]);
    // Zustandsbehaftete Optionen: wievielter Abruf dieser Route (0-basiert).
    let hit = route_idx.map_or(0, |i| state.route_hits[i].fetch_add(1, Ordering::SeqCst));

    if let Some(ms) = route.and_then(|r| r.delay_ms) {
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }

    let file = match route {
        Some(r) if r.body.is_some() || r.echo.is_some() || r.png_size.is_some() => None,
        _ => static_file(state, &req.host, &req.path, decoded.as_deref()).await,
    };

    let Some(route) = route else {
        return match file {
            Some((ct, bytes)) => {
                let mut reply = Reply::new(200);
                reply.header("Content-Type", ct);
                reply.body = bytes;
                reply
            }
            None => {
                let mut reply = Reply::new(404);
                reply.header("Content-Type", "text/plain; charset=utf-8");
                reply.body = b"404 Not Found".to_vec();
                reply
            }
        };
    };
    route_reply(route, hit, req, file)
}

fn route_reply(route: &Route, hit: u64, req: &Request, file: Option<(String, Vec<u8>)>) -> Reply {
    let mut status = match (route.status, &route.location) {
        (Some(code), _) => code,
        (None, Some(_)) => 302,
        (None, None) => 200,
    };
    if let Some(seq) = &route.sequence {
        let idx = usize::try_from(hit)
            .unwrap_or(usize::MAX)
            .min(seq.len() - 1);
        status = seq[idx];
    }
    if route.fail_first_n.is_some_and(|n| hit < u64::from(n)) {
        status = route.fail_status.unwrap_or(503);
    }

    let mut reply = Reply::new(status);
    let interim_headers: Vec<(String, String)> = route
        .informational_headers
        .iter()
        .flat_map(|(name, values)| {
            values
                .values()
                .into_iter()
                .map(|v| (name.clone(), v.to_string()))
        })
        .collect();
    reply.informational = route
        .informational
        .iter()
        .map(|&code| Informational {
            status: code,
            reason: canonical_reason(code),
            headers: interim_headers.clone(),
        })
        .collect();
    if let Some(reason) = &route.reason {
        reply.reason.clone_from(reason);
    }
    if let Some(loc) = &route.location {
        reply.header("Location", loc.clone());
    }
    for (name, values) in &route.headers {
        for v in values.values() {
            reply.header(name, v);
        }
    }
    if let Some(cookies) = &route.set_cookie {
        for v in cookies.values() {
            reply.header("Set-Cookie", v);
        }
    }

    // Body und Standard-Content-Type.
    let mut content_type: Option<String> = None;
    let mut body: Vec<u8> = Vec::new();
    if route.echo == Some(Echo::Headers) {
        content_type = Some("application/json".to_string());
        body = echo_json(req).into_bytes();
    } else if let Some(size) = route.png_size {
        content_type = Some("image/png".to_string());
        body = png_of_size(size).unwrap_or_default();
    } else if let Some(text) = &route.body {
        content_type = Some("text/html; charset=utf-8".to_string());
        body = text.clone().into_bytes();
    } else if let Some((ct, bytes)) = file {
        content_type = Some(ct);
        body = bytes;
    }
    if let Some(size) = route.body_size {
        if content_type.is_none() {
            content_type = Some("text/plain; charset=utf-8".to_string());
        }
        body.resize(size, b' ');
    }
    if let Some(ct) = &route.content_type {
        content_type = Some(ct.clone());
    }
    if route.no_content_type {
        content_type = None;
    }
    if let Some(ct) = content_type {
        reply.header("Content-Type", ct);
    }

    let bodyless = status == 204 || status == 304 || status < 200;
    if let (Some(enc), false) = (route.encoding, bodyless) {
        body = enc.encode(&body);
        reply.header("Content-Encoding", enc.header_value());
    }
    reply.body = body;
    reply.chunked = route.chunked;
    reply.trickle = route.trickle_bytes_per_s;
    reply.mode = if route.hang {
        Mode::Hang
    } else if route.abort_after_headers {
        Mode::AbortAfterHeaders
    } else if route.reset_mid_body {
        Mode::ResetMidBody
    } else {
        Mode::Normal
    };
    reply
}

async fn synthetic_reply(site: &synthetic::SyntheticSite, base_url: &str, path: &str) -> Reply {
    let Some(rendered) = site.render(path, base_url) else {
        let mut reply = Reply::new(404);
        reply.header("Content-Type", "text/plain; charset=utf-8");
        reply.body = b"404 Not Found".to_vec();
        return reply;
    };
    if let Some(delay) = rendered.delay {
        tokio::time::sleep(delay).await;
    }
    let mut reply = Reply::new(rendered.status);
    reply.header("Content-Type", rendered.content_type);
    if let Some(location) = rendered.location {
        reply.header("Location", location);
    }
    reply.body = rendered.body;
    reply
}

/// JSON der Request-Header für `echo = "headers"`.
fn echo_json(req: &Request) -> String {
    let mut headers: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    let mut cookies: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    for (name, value) in &req.headers {
        let key = name.to_ascii_lowercase();
        let entry = headers
            .entry(key.clone())
            .or_insert_with(|| serde_json::Value::Array(Vec::new()));
        if let Some(list) = entry.as_array_mut() {
            list.push(serde_json::Value::String(value.clone()));
        }
        if key == "cookie" {
            for pair in value.split(';') {
                if let Some((k, v)) = pair.trim().split_once('=') {
                    cookies.insert(
                        k.trim().to_string(),
                        serde_json::Value::String(v.to_string()),
                    );
                }
            }
        }
    }
    serde_json::json!({
        "method": req.method,
        "host": req.host,
        "path": req.target,
        "tls": req.tls,
        "headers": headers,
        "cookies": cookies,
    })
    .to_string()
}

/// Prozent-Dekodierung; `None`, wenn nichts zu dekodieren ist oder das Ergebnis kein UTF-8 ist.
pub(crate) fn percent_decode(s: &str) -> Option<String> {
    if !s.contains('%') {
        return None;
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
            if let Ok(b) = u8::from_str_radix(hex, 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).ok()
}

/// Liest die statische Datei zu einem URL-Pfad. Gibt `(Content-Type, Bytes)` zurück.
async fn static_file(
    state: &AppState,
    host: &str,
    raw_path: &str,
    decoded: Option<&str>,
) -> Option<(String, Vec<u8>)> {
    if state.dir.as_os_str().is_empty() {
        return None;
    }
    let (root, is_host_dir) = if state.host_dirs.contains(host) {
        (state.dir.join(HOSTS_DIR).join(host), true)
    } else {
        (state.dir.clone(), false)
    };
    let mut found = read_in(state, &root, is_host_dir, raw_path).await;
    if found.is_none() {
        if let Some(d) = decoded {
            found = read_in(state, &root, is_host_dir, d).await;
        }
    }
    found
}

async fn read_in(
    state: &AppState,
    root: &Path,
    is_host_dir: bool,
    url_path: &str,
) -> Option<(String, Vec<u8>)> {
    let mut rel = PathBuf::new();
    for segment in url_path.split('/').filter(|s| !s.is_empty()) {
        // Nur normale Segmente zulassen (kein "..", keine Laufwerks-/Backslash-Tricks).
        let mut comps = Path::new(segment).components();
        match (comps.next(), comps.next()) {
            (Some(Component::Normal(_)), None) if !segment.contains(['\\', ':', '\0']) => {
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
    // Die Host-Verzeichnisse sind nur über den Host-Header erreichbar, nicht als Pfad.
    if !is_host_dir && !state.host_dirs.is_empty() && rel.starts_with(HOSTS_DIR) {
        return None;
    }
    let full = root.join(&rel);
    let bytes = tokio::fs::read(&full).await.ok()?;
    let content_type = content_type_for(&full);
    let bytes = if is_text(content_type) {
        match String::from_utf8(bytes) {
            Ok(text) => text
                .replace(BASE_URL_PLACEHOLDER, &state.base_url)
                .replace(PORT2_PLACEHOLDER, &state.port2.to_string())
                .replace(PORT_PLACEHOLDER, &state.port.to_string())
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

pub(crate) fn content_type_for(path: &Path) -> &'static str {
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

    #[test]
    fn percent_decoding() {
        assert_eq!(percent_decode("/a%20b").as_deref(), Some("/a b"));
        assert_eq!(percent_decode("/%C3%A4").as_deref(), Some("/ä"));
        assert_eq!(percent_decode("/plain"), None);
        assert_eq!(percent_decode("/bad%2").as_deref(), Some("/bad%2"));
        assert_eq!(percent_decode("/bad%zz").as_deref(), Some("/bad%zz"));
    }
}
