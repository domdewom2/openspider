//! Prüft die Referenz-Fixtures des Paritätsabgleichs (M1-P10).
//!
//! Die Datei liegt unter `tests/parity/` und wird als Integrationstest des Pakets `os-testsite`
//! eingebunden (`[[test]] name = "parity_fixtures"`, `path = "../../tests/parity/fixtures_check.rs"`):
//! Sie braucht nur `os_testsite`, `tokio`, `toml` und `flate2`.
//!
//! Was geprüft wird:
//! 1. Jede Site (`out-basic-site`, `parity-*`) bleibt bei einem einfachen BFS-Rohcrawl über den
//!    os-testsite-Client unter 500 URLs, ist selbstenthalten (nur `127.0.0.1` intern, `localhost` extern,
//!    keine `.test`-Hosts, kein Inline-JavaScript, keine Platzhalter außer den dokumentierten) und hat keine
//!    Verzögerung über 1 s.
//! 2. Die Absichtserklärung je Tab/Filter (`tests/parity/coverage/<site>.toml`, vom Generator erzeugt) trifft
//!    auf die ausgelieferten Seiten zu: jede Treffer-URL erfüllt die Filterbedingung, jede Nicht-Treffer-URL
//!    nicht (kleines, unabhängiges Orakel nach den Spec-Regeln in diesem Test).
//! 3. Jeder Tab der Gruppen Internal … Sitemaps hat Treffer und Nicht-Treffer; jeder Filter ist abgedeckt oder
//!    mit Grund ausgenommen.
//! 4. Manifeste, Mapping-Dateien, Abweichungsliste, Profil und README sind vorhanden und konsistent.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use os_testsite::client::get_once;
use os_testsite::{Scenario, SiteOptions, TestSite};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const SITES: [(&str, u16); 5] = [
    ("out-basic-site", 8900),
    ("parity-onpage", 8902),
    ("parity-technical", 8903),
    ("parity-response", 8904),
    ("parity-synthetic", 8905),
];

/// Tabs, für die jede Site-Gruppe Treffer und Nicht-Treffer liefern muss (M1.md 5.1).
const GROUP_TABS: [&str; 15] = [
    "Internal",
    "External",
    "Response Codes",
    "URL",
    "Page Titles",
    "Meta Description",
    "H1",
    "H2",
    "Content",
    "Images",
    "Canonicals",
    "Directives",
    "Hreflang",
    "Links",
    "Sitemaps",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} nicht lesbar: {e}", p.display()))
}

fn read_toml(rel: &str) -> toml::Table {
    read(rel)
        .parse::<toml::Table>()
        .unwrap_or_else(|e| panic!("{rel}: ungültiges TOML: {e}"))
}

// ------------------------------------------------------------------------------------------------
// HTTP

struct Resp {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Resp {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
    fn ctype(&self) -> String {
        self.header("content-type").unwrap_or("").to_string()
    }
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// Testsite mit Zähler für jede tatsächlich gesendete Anfrage (Abgleich mit dem Request-Log der Testsite).
struct Net {
    site: TestSite,
    /// Fester Manifest-Port der Site (wie in den Links der Fixtures).
    port: u16,
    sent: AtomicUsize,
}

impl Net {
    fn sent(&self) -> usize {
        self.sent.load(Ordering::SeqCst)
    }
}

async fn fetch(net: &Net, target: &str) -> Resp {
    let site = &net.site;
    net.sent.fetch_add(1, Ordering::SeqCst);
    let resp = match get_once(site.addr(), target).await {
        Ok(r) => Resp {
            status: r.status,
            headers: r.headers,
            body: r.body,
        },
        Err(_) => {
            // 204/304 und Antworten ohne Content-Length: einfacher Client mit `Connection: close`.
            net.sent.fetch_add(1, Ordering::SeqCst);
            fetch_raw(site, target).await
        }
    };
    localize(net, resp)
}

/// Die Testsite ersetzt `{{BASE_URL}}` durch ihre tatsächliche (zufällige) Adresse; für den Vergleich mit den
/// festen Manifest-Ports wird sie in Textantworten auf `http://127.0.0.1:<Manifest-Port>` zurückgesetzt.
fn localize(net: &Net, mut resp: Resp) -> Resp {
    let textual = resp.ctype().starts_with("text/") || resp.ctype().contains("xml");
    if textual {
        let real = format!("http://{}", net.site.addr());
        let fixed = format!("http://127.0.0.1:{}", net.port);
        resp.body = resp.text().replace(&real, &fixed).into_bytes();
    }
    resp
}

async fn fetch_raw(site: &TestSite, target: &str) -> Resp {
    let mut stream = tokio::net::TcpStream::connect(site.addr()).await.unwrap();
    let req = format!(
        "GET {target} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        site.addr()
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.unwrap();
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .unwrap_or_else(|| panic!("keine Antwort auf {target}"));
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let mut lines = head.lines();
    let status: u16 = lines
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let headers = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    Resp {
        status,
        headers,
        body: raw[split + 4..].to_vec(),
    }
}

// ------------------------------------------------------------------------------------------------
// HTML-Scanner (bewusst klein: genügt für die generierten Fixtures)

#[derive(Debug, Clone)]
struct Tag {
    name: String,
    closing: bool,
    attrs: Vec<(String, String)>,
    start: usize,
    end: usize,
}

impl Tag {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
    fn tokens(&self, name: &str) -> Vec<String> {
        self.attr(name)
            .unwrap_or("")
            .split_whitespace()
            .map(|t| t.to_ascii_lowercase())
            .collect()
    }
}

fn decode_entities(s: &str) -> String {
    s.replace("&nbsp;", "\u{a0}")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

fn scan_tags(html: &str) -> Vec<Tag> {
    let b = html.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        if html[i..].starts_with("<!--") {
            i = html[i..].find("-->").map_or(b.len(), |p| i + p + 3);
            continue;
        }
        let mut j = i + 1;
        let closing = j < b.len() && b[j] == b'/';
        if closing {
            j += 1;
        }
        let ns = j;
        while j < b.len() && b[j].is_ascii_alphanumeric() {
            j += 1;
        }
        if j == ns {
            i += 1;
            continue;
        }
        let name = html[ns..j].to_ascii_lowercase();
        let mut attrs = Vec::new();
        loop {
            while j < b.len() && (b[j] as char).is_ascii_whitespace() {
                j += 1;
            }
            if j >= b.len() {
                break;
            }
            if b[j] == b'>' {
                j += 1;
                break;
            }
            if b[j] == b'/' {
                j += 1;
                continue;
            }
            let ks = j;
            while j < b.len() && !matches!(b[j], b' ' | b'\t' | b'\n' | b'\r' | b'=' | b'>' | b'/')
            {
                j += 1;
            }
            let key = html[ks..j].to_ascii_lowercase();
            while j < b.len() && (b[j] as char).is_ascii_whitespace() {
                j += 1;
            }
            let mut val = String::new();
            if j < b.len() && b[j] == b'=' {
                j += 1;
                while j < b.len() && (b[j] as char).is_ascii_whitespace() {
                    j += 1;
                }
                if j < b.len() && (b[j] == b'"' || b[j] == b'\'') {
                    let q = b[j];
                    j += 1;
                    let vs = j;
                    while j < b.len() && b[j] != q {
                        j += 1;
                    }
                    val = decode_entities(&html[vs..j.min(b.len())]);
                    j += 1;
                } else {
                    let vs = j;
                    while j < b.len() && !(b[j] as char).is_ascii_whitespace() && b[j] != b'>' {
                        j += 1;
                    }
                    val = decode_entities(&html[vs..j]);
                }
            }
            if !key.is_empty() {
                attrs.push((key, val));
            }
        }
        out.push(Tag {
            name,
            closing,
            attrs,
            start: i,
            end: j,
        });
        i = j.max(i + 1);
    }
    out
}

fn normalize(text: &str) -> String {
    decode_entities(text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => {
                in_tag = true;
                out.push(' ');
            }
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// Innentext des Elements, das mit `tag` beginnt (bis zum nächsten schließenden Tag gleichen Namens).
fn inner_html<'a>(html: &'a str, tag: &Tag) -> &'a str {
    let lower = html[tag.end..].to_ascii_lowercase();
    let close = format!("</{}", tag.name);
    let stop = lower.find(&close).map_or(html.len(), |p| tag.end + p);
    &html[tag.end..stop]
}

fn remove_elements(html: &str, names: &[&str]) -> String {
    let mut s = html.to_string();
    for name in names {
        loop {
            let lower = s.to_ascii_lowercase();
            let Some(a) = lower.find(&format!("<{name}")) else {
                break;
            };
            let b = lower[a..]
                .find(&format!("</{name}>"))
                .map_or(s.len(), |p| a + p + name.len() + 3);
            s.replace_range(a..b, " ");
        }
    }
    s
}

// ------------------------------------------------------------------------------------------------
// Adressen

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Target {
    Internal(String),
    External(String),
    Dead(String),
    Skip,
}

fn encode_path(raw: &str) -> String {
    let mut out = String::new();
    for &byte in raw.as_bytes() {
        if byte <= 0x20 || byte >= 0x7f || b"\"<>`{}|\\^".contains(&byte) {
            out.push_str(&format!("%{byte:02X}"));
        } else {
            out.push(byte as char);
        }
    }
    out
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok();
            if let Some(v) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn resolve(base_key: &str, href: &str, port: u16) -> Target {
    let href = href.trim();
    let href = href.split('#').next().unwrap_or("");
    if href.is_empty()
        || ["mailto:", "javascript:", "data:", "tel:"]
            .iter()
            .any(|p| href.to_ascii_lowercase().starts_with(p))
        || href.starts_with("//")
    {
        return Target::Skip;
    }
    let internal = format!("http://127.0.0.1:{port}");
    let external = format!("http://localhost:{port}");
    if let Some(rest) = href.strip_prefix(&internal) {
        let rest = if rest.is_empty() { "/" } else { rest };
        return Target::Internal(encode_path(rest));
    }
    if let Some(rest) = href.strip_prefix(&external) {
        let rest = if rest.is_empty() { "/" } else { rest };
        return Target::External(encode_path(rest));
    }
    if href.starts_with("http://") || href.starts_with("https://") {
        return Target::Dead(href.to_string());
    }
    if href.starts_with('/') {
        return Target::Internal(encode_path(href));
    }
    let base_path = base_key.split('?').next().unwrap_or("/");
    let dir = &base_path[..=base_path.rfind('/').unwrap_or(0)];
    let joined = format!("{dir}{href}");
    let (path, query) = match joined.split_once('?') {
        Some((p, q)) => (p.to_string(), format!("?{q}")),
        None => (joined, String::new()),
    };
    let mut segs: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "." => {}
            ".." => {
                segs.pop();
            }
            s => segs.push(s),
        }
    }
    let mut p = segs.join("/");
    if !p.starts_with('/') {
        p.insert(0, '/');
    }
    Target::Internal(encode_path(&format!("{p}{query}")))
}

// ------------------------------------------------------------------------------------------------
// Seitenmodell

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Html,
    JavaScript,
    Css,
    Images,
    Pdf,
    Flash,
    Other,
    Unknown,
}

fn class_of(ctype: &str) -> Class {
    let mime = ctype
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    match mime.as_str() {
        "" => Class::Unknown,
        "text/html" | "application/xhtml+xml" => Class::Html,
        "application/javascript"
        | "text/javascript"
        | "application/x-javascript"
        | "application/ecmascript"
        | "text/ecmascript"
        | "application/x-ecmascript"
        | "text/x-javascript" => Class::JavaScript,
        "text/css" => Class::Css,
        "application/pdf" => Class::Pdf,
        "application/x-shockwave-flash" | "application/futuresplash" => Class::Flash,
        m if m.starts_with("image/") => Class::Images,
        m if m.contains('/') && !m.contains(' ') => Class::Other,
        _ => Class::Unknown,
    }
}

#[derive(Debug, Clone)]
struct Link {
    target: Target,
    rel: Vec<String>,
    anchor_empty: bool,
}

#[derive(Debug, Clone)]
struct Page {
    key: String,
    status: u16,
    class: Class,
    body: Vec<u8>,
    /// 2xx und Inhaltstyp HTML.
    html: bool,
    titles: Vec<String>,
    descs: Vec<String>,
    h1s: Vec<String>,
    h2s: Vec<String>,
    robots: Vec<String>,
    /// Aufgelöste Canonicals: (Ziel, aus Header, im Head).
    canon: Vec<(Target, bool, bool)>,
    hreflang: Vec<(String, Target)>,
    refresh: Vec<Target>,
    links: Vec<Link>,
    imgs: Vec<(Target, Option<String>)>,
    words: usize,
    location: Option<Target>,
}

fn split_tokens(content: &str) -> Vec<String> {
    content
        .split(',')
        .map(|t| t.trim().to_ascii_lowercase())
        .filter(|t| !t.is_empty())
        .collect()
}

fn parse_refresh(content: &str, base: &str, port: u16) -> Target {
    // "0; url=/x", "5;URL='/x'", "30" (ohne Ziel = Selbst-Refresh)
    let lower = content.to_ascii_lowercase();
    let Some(pos) = lower.find("url=") else {
        return Target::Skip;
    };
    let url = content[pos + 4..]
        .trim()
        .trim_matches(|c| c == '\'' || c == '"');
    let t = resolve(base, url, port);
    if t == Target::Internal(base.to_string()) {
        Target::Skip
    } else {
        t
    }
}

fn parse_link_header(value: &str, base: &str, port: u16) -> Vec<(String, Target)> {
    // <url>; rel="canonical", <url2>; rel="alternate"
    let mut out = Vec::new();
    for part in value.split(',') {
        let part = part.trim();
        let Some(rest) = part.strip_prefix('<') else {
            continue;
        };
        let Some((url, params)) = rest.split_once('>') else {
            continue;
        };
        let rel = params
            .split(';')
            .filter_map(|p| p.trim().strip_prefix("rel="))
            .map(|v| v.trim_matches('"').to_ascii_lowercase())
            .next()
            .unwrap_or_default();
        out.push((rel, resolve(base, url, port)));
    }
    out
}

fn parse_page(key: &str, resp: &Resp, port: u16) -> Page {
    let class = class_of(&resp.ctype());
    let is_html = class == Class::Html && (200..300).contains(&resp.status);
    let mut page = Page {
        key: key.to_string(),
        status: resp.status,
        class,
        body: resp.body.clone(),
        html: is_html,
        titles: vec![],
        descs: vec![],
        h1s: vec![],
        h2s: vec![],
        robots: vec![],
        canon: vec![],
        hreflang: vec![],
        refresh: vec![],
        links: vec![],
        imgs: vec![],
        words: 0,
        location: None,
    };
    if let Some(v) = resp.header("x-robots-tag") {
        page.robots.extend(split_tokens(v));
    }
    if let Some(v) = resp.header("link") {
        for (rel, t) in parse_link_header(v, key, port) {
            if rel.split_whitespace().any(|r| r == "canonical") {
                page.canon.push((t, true, true));
            }
        }
    }
    if let Some(v) = resp.header("refresh") {
        page.refresh.push(parse_refresh(v, key, port));
    }
    if let Some(v) = resp.header("location") {
        page.location = Some(resolve(key, v, port));
    }
    if !is_html {
        return page;
    }
    let html = resp.text();
    let head_end = html
        .to_ascii_lowercase()
        .find("</head>")
        .unwrap_or(html.len());
    for tag in scan_tags(&html).iter().filter(|t| !t.closing) {
        let inner = || normalize(&strip_tags(inner_html(&html, tag)));
        match tag.name.as_str() {
            "title" => page.titles.push(normalize(inner_html(&html, tag))),
            "h1" => page.h1s.push(inner()),
            "h2" => page.h2s.push(inner()),
            "meta" => {
                let name = tag.attr("name").unwrap_or("").trim().to_ascii_lowercase();
                let content = tag.attr("content").unwrap_or("");
                if name == "description" {
                    page.descs.push(normalize(content));
                } else if name == "robots" || name == "googlebot" {
                    page.robots.extend(split_tokens(content));
                } else if tag
                    .attr("http-equiv")
                    .is_some_and(|v| v.eq_ignore_ascii_case("refresh"))
                {
                    page.refresh.push(parse_refresh(content, key, port));
                }
            }
            "link" => {
                let rel = tag.tokens("rel");
                if let Some(href) = tag.attr("href") {
                    if rel.iter().any(|r| r == "canonical") {
                        page.canon
                            .push((resolve(key, href, port), false, tag.start < head_end));
                    }
                    if rel.iter().any(|r| r == "alternate") {
                        if let Some(code) = tag.attr("hreflang").filter(|c| !c.is_empty()) {
                            page.hreflang
                                .push((code.to_string(), resolve(key, href, port)));
                        }
                    }
                }
            }
            "a" | "area" => {
                if let Some(href) = tag.attr("href") {
                    let (text, img_alt) = if tag.name == "a" {
                        let inner_raw = inner_html(&html, tag);
                        let alt = scan_tags(inner_raw)
                            .iter()
                            .filter(|t| t.name == "img")
                            .any(|t| !t.attr("alt").unwrap_or("").trim().is_empty());
                        (normalize(&strip_tags(inner_raw)), alt)
                    } else {
                        (String::new(), false)
                    };
                    page.links.push(Link {
                        target: resolve(key, href, port),
                        rel: tag.tokens("rel"),
                        anchor_empty: text.is_empty() && !img_alt,
                    });
                }
            }
            "img" => {
                if let Some(src) = tag.attr("src") {
                    page.imgs
                        .push((resolve(key, src, port), tag.attr("alt").map(normalize)));
                }
            }
            _ => {}
        }
    }
    // Wortzahl im Inhaltsbereich: nav, footer, script, style, head ausgeschlossen
    let body = remove_elements(&html, &["head", "nav", "footer", "script", "style"]);
    page.words = decode_entities(&strip_tags(&body))
        .split_whitespace()
        .count();
    page
}

/// Alle Verweise, denen ein Rohcrawl folgt (Hyperlinks, Ressourcen, Canonical/hreflang, Refresh, Location).
fn crawl_refs(html: &str, resp: &Resp, key: &str, port: u16) -> Vec<Target> {
    let mut out = Vec::new();
    if let Some(l) = resp.header("location") {
        out.push(resolve(key, l, port));
    }
    if let Some(v) = resp.header("refresh") {
        out.push(parse_refresh(v, key, port));
    }
    if let Some(v) = resp.header("link") {
        out.extend(parse_link_header(v, key, port).into_iter().map(|(_, t)| t));
    }
    if class_of(&resp.ctype()) != Class::Html {
        return out;
    }
    for tag in scan_tags(html).iter().filter(|t| !t.closing) {
        match tag.name.as_str() {
            "a" | "area" | "link" => {
                if let Some(h) = tag.attr("href") {
                    out.push(resolve(key, h, port));
                }
            }
            "img" | "script" | "source" | "iframe" => {
                if let Some(h) = tag.attr("src") {
                    out.push(resolve(key, h, port));
                }
            }
            "meta"
                if tag
                    .attr("http-equiv")
                    .is_some_and(|v| v.eq_ignore_ascii_case("refresh")) =>
            {
                out.push(parse_refresh(tag.attr("content").unwrap_or(""), key, port));
            }
            _ => {}
        }
    }
    out
}

// ------------------------------------------------------------------------------------------------
// Rohcrawl

struct Crawl {
    port: u16,
    /// Interne Antworten (BFS ab `/`, danach ab Sitemap-URLs).
    pages: BTreeMap<String, Resp>,
    parsed: BTreeMap<String, Page>,
    /// Klickpfad-Tiefe ab `/` (ohne Sitemap-Kanten).
    depth: BTreeMap<String, u32>,
    reached: BTreeSet<String>,
    externals: BTreeSet<String>,
    dead: BTreeSet<String>,
    robots: Option<String>,
    /// Sitemap-Dateien: Pfad → enthaltene URL-Schlüssel (nur `urlset`).
    sitemaps: BTreeMap<String, BTreeSet<String>>,
    ext_cache: BTreeMap<String, Resp>,
}

impl Crawl {
    fn total_urls(&self) -> usize {
        self.pages.len() + self.externals.len() + self.dead.len() + self.sitemaps.len()
    }
    fn sitemap_urls(&self) -> BTreeSet<String> {
        self.sitemaps.values().flatten().cloned().collect()
    }
    fn robots_blocked(&self, key: &str) -> bool {
        let Some(txt) = &self.robots else {
            return false;
        };
        let path = key.split('?').next().unwrap_or("/");
        let mut applies = false;
        for line in txt.lines() {
            let line = line.trim();
            let lower = line.to_ascii_lowercase();
            if let Some(v) = lower.strip_prefix("user-agent:") {
                applies = v.trim() == "*";
            } else if applies {
                if let Some(v) = line.get(9..).filter(|_| lower.starts_with("disallow:")) {
                    let rule = v.trim();
                    if !rule.is_empty() && path.starts_with(rule) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

fn gunzip(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(data)
        .read_to_end(&mut out)
        .expect("gzip");
    out
}

fn locs(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(a) = rest.find("<loc>") {
        let after = &rest[a + 5..];
        let Some(b) = after.find("</loc>") else { break };
        out.push(decode_entities(after[..b].trim()));
        rest = &after[b + 6..];
    }
    out
}

async fn crawl_site(net: &Net, port: u16) -> Crawl {
    let mut c = Crawl {
        port,
        pages: BTreeMap::new(),
        parsed: BTreeMap::new(),
        depth: BTreeMap::new(),
        reached: BTreeSet::new(),
        externals: BTreeSet::new(),
        dead: BTreeSet::new(),
        robots: None,
        sitemaps: BTreeMap::new(),
        ext_cache: BTreeMap::new(),
    };

    // robots.txt und Sitemaps
    let robots = fetch(net, "/robots.txt").await;
    let mut sitemap_seeds: Vec<String> = Vec::new();
    if robots.status == 200 {
        let txt = robots.text();
        let mut queue: VecDeque<String> = txt
            .lines()
            .filter_map(|l| {
                l.trim()
                    .strip_prefix("Sitemap:")
                    .map(|v| v.trim().to_string())
            })
            .filter_map(|u| match resolve("/", &u, port) {
                Target::Internal(k) => Some(k),
                _ => None,
            })
            .collect();
        while let Some(path) = queue.pop_front() {
            if c.sitemaps.contains_key(&path) || path.is_empty() {
                continue;
            }
            let r = fetch(net, &path).await;
            let data = if path.ends_with(".gz") || r.body.starts_with(&[0x1f, 0x8b]) {
                gunzip(&r.body)
            } else {
                r.body.clone()
            };
            let xml = String::from_utf8_lossy(&data).into_owned();
            let keys: Vec<String> = locs(&xml)
                .iter()
                .filter_map(|l| match resolve("/", l, port) {
                    Target::Internal(k) => Some(k),
                    _ => None,
                })
                .collect();
            if xml.contains("<sitemapindex") {
                c.sitemaps.insert(path.clone(), BTreeSet::new());
                queue.extend(keys);
            } else {
                sitemap_seeds.extend(keys.iter().cloned());
                c.sitemaps.insert(path.clone(), keys.into_iter().collect());
            }
        }
        c.robots = Some(txt);
    }
    // Sitemap-Dateien selbst zählen nicht als Seiten; Sitemap-Indexe haben leere Mengen.
    let sitemap_files: BTreeSet<String> = c.sitemaps.keys().cloned().collect();

    // Phase 1: BFS ab "/" (ohne Sitemap-Kanten); Phase 2: ab Sitemap-URLs, die Phase 1 nicht fand.
    for phase in 0..2 {
        let mut queue: VecDeque<(String, u32)> = VecDeque::new();
        if phase == 0 {
            queue.push_back(("/".to_string(), 0));
        } else {
            for s in &sitemap_seeds {
                if !c.pages.contains_key(s) {
                    queue.push_back((s.clone(), 0));
                }
            }
        }
        let mut seen: BTreeSet<String> = c.pages.keys().cloned().collect();
        seen.extend(sitemap_files.iter().cloned());
        while let Some((key, depth)) = queue.pop_front() {
            if !seen.insert(key.clone()) {
                continue;
            }
            assert!(c.pages.len() < 700, "Rohcrawl läuft aus dem Ruder: {key}");
            let resp = fetch(net, &key).await;
            let html = if class_of(&resp.ctype()) == Class::Html {
                resp.text()
            } else {
                String::new()
            };
            for t in crawl_refs(&html, &resp, &key, port) {
                match t {
                    Target::Internal(k) => queue.push_back((k, depth + 1)),
                    Target::External(k) => {
                        c.externals.insert(k);
                    }
                    Target::Dead(u) => {
                        c.dead.insert(u);
                    }
                    Target::Skip => {}
                }
            }
            if phase == 0 {
                c.depth.insert(key.clone(), depth);
                c.reached.insert(key.clone());
            }
            c.parsed.insert(key.clone(), parse_page(&key, &resp, port));
            c.pages.insert(key, resp);
        }
    }
    c
}

async fn ext_resp<'a>(net: &Net, c: &'a mut Crawl, key: &str) -> &'a Resp {
    if !c.ext_cache.contains_key(key) {
        let r = fetch(net, key).await;
        c.ext_cache.insert(key.to_string(), r);
    }
    &c.ext_cache[key]
}

// ------------------------------------------------------------------------------------------------
// Orakel

/// Eine geprüfte Adresse aus der Coverage-Datei.
enum Subject<'a> {
    Internal(&'a str, &'a Page),
    External(&'a str, &'a Resp),
    Dead,
}

fn indexable(c: &Crawl, p: &Page) -> bool {
    if p.status != 200 || !p.html || c.robots_blocked(&p.key) {
        return false;
    }
    if p.robots.iter().any(|t| t == "noindex" || t == "none") {
        return false;
    }
    match auth_canonical(p) {
        Some(Target::Internal(k)) => k == p.key,
        Some(_) => false,
        None => true,
    }
}

/// Maßgebliche Canonical (TECH-017): erster Header-Eintrag, sonst erstes HTML-Element im Head, sonst außerhalb.
fn auth_canonical(p: &Page) -> Option<Target> {
    p.canon
        .iter()
        .find(|(t, hdr, _)| *hdr && *t != Target::Skip)
        .or_else(|| {
            p.canon
                .iter()
                .find(|(t, _, head)| *head && *t != Target::Skip)
        })
        .or_else(|| p.canon.iter().find(|(t, _, _)| *t != Target::Skip))
        .map(|(t, _, _)| t.clone())
}

fn limit_of(filter: &str) -> usize {
    filter
        .split_whitespace()
        .find_map(|w| w.parse().ok())
        .unwrap_or_else(|| panic!("keine Zahl in Filtername {filter}"))
}

fn follow_hops(c: &Crawl, start: &str) -> (usize, bool) {
    let mut seen = vec![start.to_string()];
    let mut cur = start.to_string();
    loop {
        let Some(p) = c.parsed.get(&cur) else {
            return (seen.len() - 1, false);
        };
        let next = if (300..400).contains(&p.status) {
            p.location.clone()
        } else if p.html || p.refresh.iter().any(|t| *t != Target::Skip) {
            p.refresh.iter().find(|t| **t != Target::Skip).cloned()
        } else {
            None
        };
        match next {
            Some(Target::Internal(k)) => {
                if seen.contains(&k) {
                    return (seen.len(), true);
                }
                seen.push(k.clone());
                cur = k;
                assert!(seen.len() < 30, "Kette zu lang ab {start}");
            }
            _ => return (seen.len() - 1, false),
        }
    }
}

const LANGS: [&str; 12] = [
    "en", "de", "fr", "es", "it", "nl", "pt", "pl", "sv", "da", "fi", "ja",
];
const REGIONS: [&str; 12] = [
    "GB", "US", "DE", "FR", "AT", "CH", "BR", "ES", "IT", "NL", "CA", "AU",
];

fn hreflang_code_valid(code: &str) -> bool {
    let lower = code.to_ascii_lowercase();
    if lower == "x-default" {
        return true;
    }
    let mut parts = lower.split('-');
    let lang = parts.next().unwrap_or("");
    let region = parts.next();
    if parts.next().is_some() || !LANGS.contains(&lang) {
        return false;
    }
    region.is_none_or(|r| REGIONS.iter().any(|x| x.eq_ignore_ascii_case(r)))
}

/// `Some(true/false)`, wenn für (Tab, Filter) ein Orakel existiert; `None` = nur Existenz prüfen.
fn holds(c: &Crawl, tab: &str, filter: &str, subject: &Subject<'_>) -> Option<bool> {
    let internal = matches!(subject, Subject::Internal(..));
    let page = match subject {
        Subject::Internal(_, p) => Some(*p),
        _ => None,
    };
    let result = match tab {
        "Internal" | "External" => {
            let scope = if tab == "Internal" {
                internal
            } else {
                !internal
            };
            let class = match subject {
                Subject::Internal(_, p) => p.class,
                Subject::External(_, r) => class_of(&r.ctype()),
                Subject::Dead => Class::Unknown,
            };
            let want = match filter {
                "All" => return Some(scope),
                "HTML" => Class::Html,
                "JavaScript" => Class::JavaScript,
                "CSS" => Class::Css,
                "Images" => Class::Images,
                "PDF" => Class::Pdf,
                "Flash" => Class::Flash,
                "Other" => Class::Other,
                "Unknown" => Class::Unknown,
                _ => return None,
            };
            scope && class == want
        }
        "Response Codes" => {
            let (scope_ok, base) = if let Some(b) = filter.strip_prefix("Internal ") {
                (internal, b)
            } else if let Some(b) = filter.strip_prefix("External ") {
                (!internal, b)
            } else {
                (true, filter)
            };
            let status = match subject {
                Subject::Internal(_, p) => Some(p.status),
                Subject::External(_, r) => Some(r.status),
                Subject::Dead => None,
            };
            let key = match subject {
                Subject::Internal(k, _) => Some(*k),
                _ => None,
            };
            let refresh_page = page.is_some_and(|p| p.refresh.iter().any(|t| *t != Target::Skip));
            let r = match base {
                "All" => true,
                "Blocked by Robots.txt" => key.is_some_and(|k| c.robots_blocked(k)),
                "No Response" => status.is_none(),
                "Success (2XX)" => status.is_some_and(|s| (200..300).contains(&s)) && !refresh_page,
                "Redirection (3XX)" => status.is_some_and(|s| (300..400).contains(&s)),
                "Redirection (Meta Refresh)" => {
                    page.is_some_and(|p| p.html && p.refresh.iter().any(|t| *t != Target::Skip))
                }
                "Redirect Chain" => key.is_some_and(|k| {
                    let (hops, looped) = follow_hops(c, k);
                    hops >= 2 && !looped
                }),
                "Redirect Loop" => key.is_some_and(|k| follow_hops(c, k).1),
                "Client Error (4XX)" => status.is_some_and(|s| (400..500).contains(&s)),
                "Server Error (5XX)" => status.is_some_and(|s| (500..600).contains(&s)),
                "Other" => status.is_some_and(|s| !(100..600).contains(&s)),
                _ => return None,
            };
            scope_ok && r
        }
        "URL" => {
            let Subject::Internal(k, _) = subject else {
                return Some(false);
            };
            let display = percent_decode(&format!("http://127.0.0.1:{}{k}", c.port));
            let tail = percent_decode(k);
            let path = tail.split('?').next().unwrap_or("");
            match filter {
                "All" => true,
                "Non ASCII Characters" => !display.is_ascii(),
                "Underscores" => path.contains('_'),
                "Uppercase" => tail.chars().any(|ch| ch.is_uppercase()),
                "Multiple Slashes" => path.contains("//"),
                "Contains A Space" => display.contains(' '),
                "Parameters" => tail.contains('?') || tail.contains('&'),
                f if f.starts_with("Over ") => display.chars().count() > limit_of(f),
                _ => return None,
            }
        }
        "Page Titles" | "Meta Description" | "H1" | "H2" => {
            let Some(p) = page.filter(|p| p.html) else {
                return Some(false);
            };
            let values = match tab {
                "Page Titles" => &p.titles,
                "Meta Description" => &p.descs,
                "H1" => &p.h1s,
                _ => &p.h2s,
            };
            let first = values.first().map_or("", |s| s.as_str());
            match filter {
                "All" => true,
                "Missing" => values.iter().all(|v| v.is_empty()),
                "Duplicate" => {
                    indexable(c, p)
                        && !first.is_empty()
                        && c.parsed.values().any(|q| {
                            q.key != p.key
                                && q.html
                                && indexable(c, q)
                                && match tab {
                                    "Page Titles" => q.titles.first(),
                                    "Meta Description" => q.descs.first(),
                                    "H1" => q.h1s.first(),
                                    _ => q.h2s.first(),
                                }
                                .is_some_and(|v| v == first)
                        })
                }
                "Multiple" => values.len() > 1,
                f if f.starts_with("Over ") => first.chars().count() > limit_of(f),
                f if f.starts_with("Below ") => {
                    let n = first.chars().count();
                    n > 0 && n < limit_of(f)
                }
                _ => return None,
            }
        }
        "Content" => {
            let Some(p) = page.filter(|p| p.html) else {
                return Some(false);
            };
            match filter {
                "All" => true,
                "Exact Duplicates" => {
                    indexable(c, p)
                        && c.parsed.values().any(|q| {
                            q.key != p.key && q.html && indexable(c, q) && q.body == p.body
                        })
                }
                "Low Content Pages" => indexable(c, p) && p.words < 200,
                _ => return None,
            }
        }
        "Images" => {
            // Population: als Bild referenziert oder Inhaltstyp image/*
            let (is_ref_key, status, size, class) = match subject {
                Subject::Internal(k, p) => (format!("i:{k}"), p.status, p.body.len(), p.class),
                Subject::External(k, r) => (
                    format!("e:{k}"),
                    r.status,
                    r.body.len(),
                    class_of(&r.ctype()),
                ),
                Subject::Dead => return Some(false),
            };
            let mut instances: Vec<Option<String>> = Vec::new();
            for q in c.parsed.values() {
                for (t, alt) in &q.imgs {
                    let k = match t {
                        Target::Internal(k) => format!("i:{k}"),
                        Target::External(k) => format!("e:{k}"),
                        _ => continue,
                    };
                    if k == is_ref_key {
                        instances.push(alt.clone());
                    }
                }
            }
            let in_pop = !instances.is_empty() || class == Class::Images;
            match filter {
                "All" => in_pop,
                f if f.starts_with("Over ") && f.ends_with("KB") => {
                    in_pop && status == 200 && size > limit_of(f) * 1024
                }
                "Missing Alt Text" => instances
                    .iter()
                    .any(|a| a.as_deref().is_some_and(str::is_empty)),
                "Missing Alt Attribute" => instances.iter().any(|a| a.is_none()),
                f if f.starts_with("Alt Text Over") => instances.iter().any(|a| {
                    a.as_deref()
                        .is_some_and(|t| t.chars().count() > limit_of(f))
                }),
                _ => return None,
            }
        }
        "Canonicals" => {
            let Some(p) = page.filter(|p| p.html && p.status == 200) else {
                return Some(false);
            };
            let auth = auth_canonical(p);
            let valid: Vec<&Target> = p
                .canon
                .iter()
                .map(|(t, _, _)| t)
                .filter(|t| **t != Target::Skip)
                .collect();
            let self_ref = matches!(&auth, Some(Target::Internal(k)) if *k == p.key);
            match filter {
                "All" => true,
                "Contains Canonical" => !p.canon.is_empty(),
                "Self Referencing" => self_ref,
                "Canonicalised" => auth.is_some() && !self_ref,
                "Missing" => p.canon.is_empty(),
                "Multiple" => p.canon.len() >= 2,
                "Multiple Conflicting" => valid.iter().collect::<BTreeSet<_>>().len() >= 2,
                "Non-Indexable Canonical" => match &auth {
                    Some(Target::Internal(k)) if *k != p.key => {
                        c.parsed.get(k).is_none_or(|q| !indexable(c, q))
                    }
                    _ => false,
                },
                _ => return None,
            }
        }
        "Directives" => {
            let Some(p) = page else { return Some(false) };
            let has = |t: &str| p.robots.iter().any(|x| x == t);
            match filter {
                "All" => true,
                "Index" => has("index"),
                "Noindex" => has("noindex"),
                "Follow" => has("follow"),
                "Nofollow" => has("nofollow"),
                "None" => has("none"),
                "Refresh" => !p.refresh.is_empty(),
                _ => return None,
            }
        }
        "Hreflang" => {
            let Some(p) = page.filter(|p| p.html && p.status == 200) else {
                return Some(false);
            };
            match filter {
                "All" => true,
                "Contains Hreflang" => !p.hreflang.is_empty(),
                "Missing" => p.hreflang.is_empty(),
                "Non-200 Hreflang URLs" => p.hreflang.iter().any(|(_, t)| {
                    matches!(t, Target::Internal(k) if c.parsed.get(k).is_some_and(|q| q.status != 200))
                }),
                "Missing Return Links" => p.hreflang.iter().any(|(_, t)| match t {
                    Target::Internal(k) => c.parsed.get(k).is_some_and(|q| {
                        q.html
                            && q.status == 200
                            && !q.hreflang.iter().any(|(_, b)| *b == Target::Internal(p.key.clone()))
                    }),
                    _ => false,
                }),
                "Incorrect Language & Region Codes" => p.hreflang.iter().any(|(code, _)| !hreflang_code_valid(code)),
                _ => return None,
            }
        }
        "Links" => {
            let Some(p) = page.filter(|p| p.html && p.status == 200) else {
                return Some(false);
            };
            let internal_links = || {
                p.links
                    .iter()
                    .filter(|l| matches!(&l.target, Target::Internal(k) if *k != p.key))
            };
            match filter {
                "All" => true,
                "Pages With High Crawl Depth" => c.depth.get(&p.key).is_some_and(|d| *d >= 4),
                "Pages Without Internal Outlinks" => internal_links().next().is_none(),
                "Internal Nofollow Outlinks" => internal_links().any(|l| {
                    l.rel
                        .iter()
                        .any(|r| r == "nofollow" || r == "ugc" || r == "sponsored")
                }),
                "Internal Outlinks With No Anchor Text" => internal_links().any(|l| l.anchor_empty),
                _ => return None,
            }
        }
        "Sitemaps" => {
            let in_sitemap = |k: &str| c.sitemap_urls().contains(k);
            let Subject::Internal(k, p) = subject else {
                return Some(false);
            };
            match filter {
                "All" => true,
                "URLs In Sitemap" => in_sitemap(k),
                "URLs Not In Sitemap" => {
                    !c.sitemaps.is_empty() && p.html && indexable(c, p) && !in_sitemap(k)
                }
                "Orphan URLs" => {
                    (in_sitemap(k) || !c.reached.contains(*k)) && !c.reached.contains(*k) && p.html
                }
                "Non-Indexable URLs In Sitemap" => in_sitemap(k) && !indexable(c, p),
                _ => return None,
            }
        }
        _ => return None,
    };
    Some(result)
}

// ------------------------------------------------------------------------------------------------
// Filterliste

/// (Tab, Anzeigename des Filters) für alle Tabs der 15 Gruppen aus docs/plan/M1-filters.toml.
fn group_filters() -> Vec<(String, String)> {
    let doc = read_toml("docs/plan/M1-filters.toml");
    let mut out = Vec::new();
    for tab in doc["tab"].as_array().unwrap() {
        let id = tab["id"].as_str().unwrap();
        if !GROUP_TABS.contains(&id) {
            continue;
        }
        for f in tab["filters"].as_array().unwrap() {
            let label = f
                .get("default_label")
                .or_else(|| f.get("id"))
                .and_then(|v| v.as_str())
                .unwrap();
            out.push((id.to_string(), label.to_string()));
        }
    }
    out
}

struct Case {
    tab: String,
    filter: String,
    hit: Vec<String>,
    miss: Vec<String>,
}

fn load_cases(site: &str) -> (Vec<Case>, Vec<(String, String, String)>) {
    let doc = read_toml(&format!("tests/parity/coverage/{site}.toml"));
    assert_eq!(doc["site"].as_str(), Some(site));
    let list = |v: &toml::Value| -> Vec<String> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().to_string())
            .collect()
    };
    let cases = doc
        .get("case")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .map(|c| Case {
                    tab: c["tab"].as_str().unwrap().to_string(),
                    filter: c["filter"].as_str().unwrap().to_string(),
                    hit: list(&c["hit"]),
                    miss: list(&c["miss"]),
                })
                .collect()
        })
        .unwrap_or_default();
    let exempt = doc
        .get("exempt")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .map(|c| {
                    (
                        c["tab"].as_str().unwrap().to_string(),
                        c["filter"].as_str().unwrap().to_string(),
                        c["reason"].as_str().unwrap().to_string(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    (cases, exempt)
}

fn fixture_dir(site: &str) -> PathBuf {
    repo_root().join("tests/fixtures").join(site)
}

async fn start(name: &str) -> Net {
    let site = TestSite::start_dir_with(
        fixture_dir(name),
        SiteOptions {
            log_requests: true,
            ..Default::default()
        },
    )
    .await
    .unwrap_or_else(|e| panic!("{name}: {e}"));
    let port = SITES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, p)| *p)
        .unwrap();
    Net {
        site,
        port,
        sent: AtomicUsize::new(0),
    }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let p = entry.unwrap().path();
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Tests

#[tokio::test]
async fn every_site_stays_within_500_urls_and_is_self_contained() {
    for (name, port) in SITES {
        let net = start(name).await;
        let crawl = crawl_site(&net, port).await;
        let own = net.sent();
        let total = crawl.total_urls();
        println!(
            "{name}: {total} URLs (intern {}, extern {}, ohne Antwort {}, Sitemaps {}), {own} Anfragen",
            crawl.pages.len(),
            crawl.externals.len(),
            crawl.dead.len(),
            crawl.sitemaps.len()
        );
        assert!(
            total <= 500,
            "{name}: {total} URLs im Rohcrawl (Grenze 500)"
        );
        // Alle Anfragen des Tests stehen im Request-Log der Testsite (Wiederholungen eingeschlossen).
        assert_eq!(
            net.site.total_requests() as usize,
            own,
            "{name}: Request-Log weicht ab"
        );
        match name {
            "parity-synthetic" => assert!(
                (400..=410).contains(&crawl.pages.len()),
                "{name}: {} interne URLs",
                crawl.pages.len()
            ),
            "out-basic-site" => assert!(crawl.pages.len() >= 15, "{name}: {}", crawl.pages.len()),
            "parity-response" => assert!(crawl.pages.len() >= 35, "{name}: {}", crawl.pages.len()),
            _ => assert!(crawl.pages.len() >= 60, "{name}: {}", crawl.pages.len()),
        }
        // Wurzel erreichbar, keine interne Seite außer den vorgesehenen Fehlerfällen ist unerwartet kaputt.
        assert_eq!(crawl.pages["/"].status, 200, "{name}: Startseite");
    }
}

#[test]
fn site_files_use_only_allowed_hosts_and_no_javascript_or_placeholders() {
    for (name, port) in SITES {
        let dir = fixture_dir(name);
        let mut files = Vec::new();
        walk(&dir, &mut files);
        assert!(!files.is_empty(), "{name}: keine Dateien");
        let scenario_text = std::fs::read_to_string(dir.join("scenario.toml")).unwrap();
        let scenario = Scenario::parse(&scenario_text).unwrap_or_else(|e| panic!("{name}: {e}"));
        for r in &scenario.routes {
            assert!(
                r.delay_ms.unwrap_or(0) <= 1000,
                "{name}: Verzögerung > 1 s auf {}",
                r.path
            );
        }
        let allowed = [
            format!("http://127.0.0.1:{port}"),
            format!("http://localhost:{port}"),
            "http://localhost:1/".to_string(),
            "http://www.sitemaps.org/schemas/sitemap/0.9".to_string(),
            "http://www.w3.org/".to_string(),
        ];
        let mut texts: Vec<(String, String)> = vec![("scenario.toml".into(), scenario_text)];
        for f in files {
            let rel = f
                .strip_prefix(&dir)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let is_text = ["html", "xml", "txt", "css", "js", "svg", "json"]
                .iter()
                .any(|e| rel.ends_with(&format!(".{e}")));
            if is_text && rel != "scenario.toml" {
                texts.push((rel, std::fs::read_to_string(&f).unwrap()));
            }
        }
        for (rel, text) in &texts {
            // Platzhalter: nur {{BASE_URL}} in fünf Dateien von out-basic-site (siehe Generator-Kommentar)
            let stripped = text.replace("{{BASE_URL}}", "");
            assert!(!stripped.contains("{{"), "{name}/{rel}: Platzhalter");
            if text.contains("{{BASE_URL}}") {
                assert!(
                    name == "out-basic-site"
                        && [
                            "index.html",
                            "robots.txt",
                            "sitemap.xml",
                            "about.html",
                            "products.html"
                        ]
                        .contains(&rel.as_str()),
                    "{name}/{rel}: {{{{BASE_URL}}}} nicht erlaubt"
                );
            }
            assert!(!text.contains(".test"), "{name}/{rel}: .test-Host");
            for (i, _) in text.match_indices("http://") {
                let rest = &text[i..];
                assert!(
                    allowed.iter().any(|a| rest.starts_with(a.as_str())),
                    "{name}/{rel}: nicht erlaubte URL {}",
                    &rest[..rest.len().min(50)]
                );
            }
            assert!(!text.contains("https://"), "{name}/{rel}: https");
            if rel.ends_with(".html") || rel == "scenario.toml" {
                for tag in scan_tags(&text.replace("\\\"", "\"").replace("\\n", "\n")) {
                    if tag.name == "script" && !tag.closing {
                        assert!(tag.attr("src").is_some(), "{name}/{rel}: Inline-Skript");
                    }
                    if !tag.closing {
                        for attr in ["href", "src"] {
                            if let Some(v) = tag.attr(attr) {
                                if tag.name == "a" || tag.name == "img" || tag.name == "script" {
                                    assert!(
                                        v.starts_with('/') || v.starts_with("http://"),
                                        "{name}/{rel}: relativer Link {v}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[tokio::test]
async fn filter_cases_match_the_served_pages() {
    for (name, port) in SITES {
        if !tests_has_coverage(name) {
            continue;
        }
        let net = start(name).await;
        let mut crawl = crawl_site(&net, port).await;
        let (cases, _) = load_cases(name);
        let valid: BTreeSet<(String, String)> = group_filters().into_iter().collect();
        let mut failures = Vec::new();
        let mut checked = 0usize;
        for case in &cases {
            assert!(
                valid.contains(&(case.tab.clone(), case.filter.clone())),
                "{name}: unbekannter Filter {}:{}",
                case.tab,
                case.filter
            );
            for (list, expected) in [(&case.hit, true), (&case.miss, false)] {
                for tok in list {
                    let verdict = evaluate(&net, &mut crawl, &case.tab, &case.filter, tok).await;
                    checked += 1;
                    if verdict != Ok(expected) {
                        failures.push(format!(
                            "{name}: {}:{} {} {} -> {:?}",
                            case.tab,
                            case.filter,
                            if expected { "Treffer" } else { "Nicht-Treffer" },
                            tok,
                            verdict
                        ));
                    }
                }
            }
        }
        println!("{name}: {} Fälle, {checked} Adressen geprüft", cases.len());
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}

fn tests_has_coverage(site: &str) -> bool {
    repo_root()
        .join(format!("tests/parity/coverage/{site}.toml"))
        .exists()
}

fn no_oracle(tab: &str, filter: &str) -> String {
    format!("kein Orakel für {tab}:{filter}")
}

/// Ok(true/false) = Orakelurteil, Err(grund) = Adresse nicht erreichbar bzw. kein Orakel (nur Existenz).
async fn evaluate(
    net: &Net,
    c: &mut Crawl,
    tab: &str,
    filter: &str,
    tok: &str,
) -> Result<bool, String> {
    if let Some(url) = tok.strip_prefix("dead:") {
        if !c.dead.contains(url) {
            return Err(format!("{url} wird nirgends verlinkt"));
        }
        return holds(c, tab, filter, &Subject::Dead).ok_or_else(|| no_oracle(tab, filter));
    }
    if let Some(path) = tok.strip_prefix("ext:") {
        let key = encode_path(path);
        if !c.externals.contains(&key) {
            return Err(format!("{path} wird nirgends als externe URL verlinkt"));
        }
        ext_resp(net, c, &key).await;
        let r = &c.ext_cache[&key];
        return holds(c, tab, filter, &Subject::External(&key, r))
            .ok_or_else(|| no_oracle(tab, filter));
    }
    let key = encode_path(tok);
    let Some(p) = c.parsed.get(&key) else {
        return Err(format!("{tok} wurde vom Rohcrawl nicht erreicht"));
    };
    holds(c, tab, filter, &Subject::Internal(&key, p)).ok_or_else(|| no_oracle(tab, filter))
}

/// Endziel einer Weiterleitung (HTTP 3xx, Meta Refresh, Refresh-Header) nach der Filterzuordnung der Fachspec:
/// "Blocked by Robots.txt", "No Response", "Client Error (4XX)", "Server Error (5XX)"; sonst "OK" bzw. "Redirect Loop".
async fn redirect_end(net: &Net, c: &mut Crawl, start: &str) -> String {
    fn by_status(status: u16) -> String {
        match status {
            400..=499 => "Client Error (4XX)".to_string(),
            500..=599 => "Server Error (5XX)".to_string(),
            _ => "OK".to_string(),
        }
    }
    let mut seen = vec![start.to_string()];
    let mut cur = start.to_string();
    loop {
        if seen.len() > 1 && c.robots_blocked(&cur) {
            return "Blocked by Robots.txt".to_string();
        }
        let p = c
            .parsed
            .get(&cur)
            .unwrap_or_else(|| panic!("{cur} wurde vom Rohcrawl nicht erreicht"));
        let next = if (300..400).contains(&p.status) {
            p.location.clone()
        } else {
            p.refresh.iter().find(|t| **t != Target::Skip).cloned()
        };
        match next {
            Some(Target::Internal(k)) => {
                if seen.contains(&k) {
                    return "Redirect Loop".to_string();
                }
                seen.push(k.clone());
                cur = k;
            }
            Some(Target::Dead(_)) => return "No Response".to_string(),
            Some(Target::External(k)) => {
                let status = ext_resp(net, c, &k).await.status;
                return by_status(status);
            }
            Some(Target::Skip) | None => return by_status(p.status),
        }
    }
}

// spec: OUT-084
#[tokio::test]
async fn redirects_to_errors_report_has_error_targets_and_non_targets() {
    let name = "parity-response";
    let net = start(name).await;
    let mut crawl = crawl_site(&net, 8904).await;
    let doc = read_toml(&format!("tests/parity/coverage/{name}.toml"));
    let reports = doc["report"].as_array().expect("[[report]] fehlt");
    let report = reports
        .iter()
        .find(|r| r["name"].as_str() == Some("Redirects to Errors"))
        .expect("Report Redirects to Errors");
    let list = |key: &str| -> Vec<String> {
        report[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().to_string())
            .collect()
    };
    let errors = [
        "Blocked by Robots.txt",
        "No Response",
        "Client Error (4XX)",
        "Server Error (5XX)",
    ];
    let mut kinds = BTreeSet::new();
    for entry in list("hit") {
        let (addr, expected) = entry
            .split_once(" -> ")
            .unwrap_or_else(|| panic!("Eintrag ohne Endziel: {entry}"));
        assert!(
            errors.contains(&expected),
            "{entry}: kein Fehlerziel in der Absichtserklärung"
        );
        let end = redirect_end(&net, &mut crawl, &encode_path(addr)).await;
        assert_eq!(end, expected, "{addr}: Endziel");
        kinds.insert(expected.to_string());
    }
    for kind in errors {
        assert!(
            kinds.contains(kind),
            "Report ohne Treffer mit Endziel {kind}"
        );
    }
    for addr in list("miss") {
        let end = redirect_end(&net, &mut crawl, &encode_path(&addr)).await;
        assert!(
            !errors.contains(&end.as_str()),
            "{addr} endet in {end}, darf nicht im Report stehen"
        );
    }
    // Sprungzahlen wie im Akzeptanzkriterium (Number of Redirects): 1 Hop auf 404, 2 Hops bis zum 500.
    assert_eq!(follow_hops(&crawl, "/r-to-404").0, 1);
    assert_eq!(follow_hops(&crawl, "/r-to-blocked").0, 1);
    assert_eq!(follow_hops(&crawl, "/r-chain-500").0, 2);
    assert!(follow_hops(&crawl, "/loop1").1, "/loop1 ist eine Schleife");
    assert_eq!(
        redirect_end(&net, &mut crawl, "/r301").await,
        "OK",
        "/r301 endet in einer 200-Seite"
    );
}

#[test]
fn every_group_tab_and_filter_has_hits_and_non_hits_or_is_exempt() {
    let filters = group_filters();
    let mut hit: BTreeSet<(String, String)> = BTreeSet::new();
    let mut miss: BTreeSet<(String, String)> = BTreeSet::new();
    let mut exempt: BTreeSet<(String, String)> = BTreeSet::new();
    for (name, _) in SITES {
        if !tests_has_coverage(name) {
            continue;
        }
        let (cases, ex) = load_cases(name);
        for c in cases {
            let k = (c.tab.clone(), c.filter.clone());
            if !c.hit.is_empty() {
                hit.insert(k.clone());
            }
            if !c.miss.is_empty() {
                miss.insert(k);
            }
        }
        for (tab, filter, reason) in ex {
            assert!(!reason.trim().is_empty());
            assert!(
                filters.contains(&(tab.clone(), filter.clone())),
                "Ausnahme für unbekannten Filter {tab}:{filter}"
            );
            exempt.insert((tab, filter));
        }
    }
    for tab in GROUP_TABS {
        assert!(hit.iter().any(|(t, _)| t == tab), "Tab {tab}: kein Treffer");
        assert!(
            miss.iter().any(|(t, _)| t == tab),
            "Tab {tab}: kein Nicht-Treffer"
        );
    }
    let uncovered: Vec<String> = filters
        .iter()
        .filter(|f| !(hit.contains(*f) && miss.contains(*f)) && !exempt.contains(*f))
        .map(|(t, f)| format!("{t}:{f}"))
        .collect();
    assert!(
        uncovered.is_empty(),
        "Filter ohne Treffer/Nicht-Treffer und ohne Ausnahme: {uncovered:?}"
    );
}

#[test]
fn out_basic_site_matches_the_out_001_description() {
    let dir = fixture_dir("out-basic-site");
    let mut files = Vec::new();
    walk(&dir, &mut files);
    let count = |ext: &str| {
        files
            .iter()
            .filter(|f| f.extension().is_some_and(|e| e == ext))
            .count()
    };
    assert_eq!(count("html"), 12, "12 HTML-Seiten");
    assert_eq!(count("png"), 2, "2 Bilder");
    assert_eq!(count("css"), 1, "1 CSS");
    assert_eq!(count("js"), 1, "1 JS");
    let scenario = Scenario::parse(&read("tests/fixtures/out-basic-site/scenario.toml")).unwrap();
    let status = |p: &str| {
        scenario
            .routes
            .iter()
            .find(|r| r.path == p)
            .and_then(|r| r.status)
    };
    assert_eq!(status("/missing.html"), Some(404));
    assert_eq!(status("/old-page"), Some(301));
    let index = read("tests/fixtures/out-basic-site/index.html");
    assert!(
        index.contains("http://localhost:8900/"),
        "externer Link fest auf Port 8900"
    );
    // exclude.osconfig / columns.toml
    let exclude = read_toml("tests/fixtures/out-basic-site/exclude.osconfig");
    assert_eq!(exclude["schema_version"].as_integer(), Some(1));
    let rules = exclude["scope"]["exclude"].as_array().unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].as_str(), Some(".*/privat/.*"));
    let columns = read_toml("tests/fixtures/out-basic-site/columns.toml");
    let internal: Vec<&str> = columns["tab"]["Internal"]["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(internal[0], "Address");
    let pos = |n: &str| internal.iter().position(|c| *c == n);
    assert!(
        pos("Title 1") < pos("Status Code"),
        "Title 1 vor Status Code"
    );
    assert!(pos("Title 1").is_some() && pos("Status Code").is_some());
    assert!(
        pos("Meta Keywords 1").is_none(),
        "Meta Keywords 1 ausgeblendet"
    );
}

#[test]
fn manifests_mappings_and_profile_are_consistent() {
    let filters = group_filters();
    let all_filters: Vec<(String, String)> = {
        // Auch Tabs außerhalb der 15 Gruppen (z. B. Security) sind als Export gültig.
        let doc = read_toml("docs/plan/M1-filters.toml");
        let mut v = Vec::new();
        for tab in doc["tab"].as_array().unwrap() {
            for f in tab["filters"].as_array().unwrap() {
                let label = f
                    .get("default_label")
                    .or_else(|| f.get("id"))
                    .unwrap()
                    .as_str()
                    .unwrap();
                v.push((tab["id"].as_str().unwrap().to_string(), label.to_string()));
            }
        }
        v
    };
    assert!(!filters.is_empty());
    let mut used_mappings = BTreeSet::new();
    let mut seen_ports = BTreeSet::new();
    for (name, port) in SITES {
        let m = read_toml(&format!("tests/golden/parity/{name}/manifest.toml"));
        assert_eq!(m["site"].as_str(), Some(name));
        assert_eq!(
            m["port"].as_integer(),
            Some(i64::from(port)),
            "{name}: Port"
        );
        assert!(seen_ports.insert(port), "Port doppelt");
        assert!(
            m.contains_key("ref_version")
                && m.contains_key("exported_at")
                && m.contains_key("ref_settings")
        );
        let exports = m["export"]
            .as_array()
            .unwrap_or_else(|| panic!("{name}: keine Exporte"));
        assert!(!exports.is_empty());
        let mut ref_files = BTreeSet::new();
        for e in exports {
            let status = e["status"]
                .as_str()
                .unwrap_or_else(|| panic!("{name}: status"));
            assert!(
                matches!(status, "pending" | "present" | "unavailable"),
                "{name}: unbekannter status {status}"
            );
            let ref_file = e["ref_file"].as_str().unwrap();
            assert!(ref_file.ends_with(".csv") && !ref_file.contains('/'));
            assert!(
                ref_files.insert(ref_file.to_string()),
                "{name}: {ref_file} doppelt"
            );
            let os = e["os"].as_str().unwrap();
            let mapping = e["mapping"].as_str().unwrap();
            used_mappings.insert(mapping.to_string());
            let (kind, rest) = os
                .split_once(':')
                .unwrap_or_else(|| panic!("os-Angabe {os}"));
            match kind {
                "tab" => {
                    let (tab, filter) =
                        rest.split_once(':').unwrap_or_else(|| panic!("tab:{rest}"));
                    assert!(
                        all_filters
                            .iter()
                            .any(|(t, f)| t.eq_ignore_ascii_case(tab)
                                && f.eq_ignore_ascii_case(filter)),
                        "{name}: unbekannter Filter {os}"
                    );
                }
                "bulk" | "report" => assert!(!rest.is_empty()),
                _ => panic!("{name}: unbekannte os-Art {os}"),
            }
            assert!(
                repo_root()
                    .join(format!("tests/parity/mappings/{mapping}.toml"))
                    .exists(),
                "Mapping {mapping}"
            );
        }
        // ref/ gehört dem Nutzer (A8): genau die Dateien der Einträge mit status = present – vorhanden, nicht
        // leer, Endung .csv, keine Unterordner, nichts sonst. pending/unavailable haben keine Datei.
        let present: BTreeSet<String> = exports
            .iter()
            .filter(|e| e["status"].as_str() == Some("present"))
            .map(|e| e["ref_file"].as_str().unwrap().to_string())
            .collect();
        let ref_dir = repo_root().join(format!("tests/golden/parity/{name}/ref"));
        let mut found = BTreeSet::new();
        if ref_dir.exists() {
            for entry in std::fs::read_dir(&ref_dir).unwrap() {
                let entry = entry.unwrap();
                let file_name = entry.file_name().to_string_lossy().into_owned();
                assert!(
                    entry.file_type().unwrap().is_file(),
                    "{name}: ref/{file_name} ist keine Datei (keine Unterordner erlaubt)"
                );
                assert!(
                    file_name.ends_with(".csv"),
                    "{name}: ref/{file_name} ist keine .csv-Datei"
                );
                assert!(
                    entry.metadata().unwrap().len() > 0,
                    "{name}: ref/{file_name} ist leer"
                );
                found.insert(file_name);
            }
        }
        assert_eq!(
            found, present,
            "{name}: ref/ muss genau die Dateien der Einträge mit status = present enthalten"
        );
    }
    // Mappings
    let mut mapping_files = Vec::new();
    walk(
        &repo_root().join("tests/parity/mappings"),
        &mut mapping_files,
    );
    assert!(mapping_files.len() >= 15, "zu wenige Mapping-Dateien");
    for f in mapping_files {
        let slug = f.file_stem().unwrap().to_string_lossy().into_owned();
        assert!(
            used_mappings.contains(&slug),
            "Mapping {slug} wird von keinem Manifest benutzt"
        );
        let m = read_toml(&format!("tests/parity/mappings/{slug}.toml"));
        let key: Vec<&str> = m["key"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(!key.is_empty());
        let drop: Vec<&str> = m["drop_ref_columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(
            drop.contains(&"Description") && drop.contains(&"How To Fix"),
            "{slug}: drop_ref_columns"
        );
        let cols = m["columns"].as_table().unwrap();
        assert!(!cols.is_empty());
        for k in &key {
            if *k != "#" {
                assert!(
                    cols.values().any(|v| v.as_str() == Some(k)
                        || v.get("os").and_then(|o| o.as_str()) == Some(k)),
                    "{slug}: Schlüsselspalte {k} nicht in [columns] abgebildet"
                );
            }
        }
        for (reference, v) in cols {
            assert!(
                !drop.contains(&reference.as_str()),
                "{slug}: {reference} wird verworfen und abgebildet"
            );
            match v {
                toml::Value::String(_) => {}
                toml::Value::Table(t) => assert!(
                    t.contains_key("os") || t.get("ignore").and_then(|i| i.as_bool()) == Some(true),
                    "{slug}: {reference}"
                ),
                _ => panic!("{slug}: {reference}"),
            }
        }
        let n = m["normalize"].as_table().unwrap();
        assert_eq!(n["base_url"].as_bool(), Some(true));
    }
    // Abweichungsliste, Profil, README, Schwellen
    let dev = read_toml("tests/parity/deviations.toml");
    assert!(dev.get("deviation").is_none_or(|d| d.as_array().is_some()));
    let profile = read_toml("tests/parity/profiles/parity.osconfig");
    assert_eq!(profile["schema_version"].as_integer(), Some(1));
    assert_eq!(profile["robots"]["mode"].as_str(), Some("respect"));
    // K-53: die Referenz (Referenzprodukt Free) hat Hreflang und verlinkte Sitemaps nicht gecrawlt.
    assert_eq!(
        profile["spider"]["hreflang"]["crawl"].as_bool(),
        Some(false)
    );
    assert_eq!(profile["sitemaps"]["crawl_linked"].as_bool(), Some(false));
    let readme = read("tests/golden/parity/README.md");
    for (name, port) in SITES {
        assert!(
            readme.contains(name) && readme.contains(&port.to_string()),
            "README: {name}"
        );
    }
    // Schwellen: nur bekannte Exporte; die Redirects-Reports und das Crawl-Overview sind bewertet (A3).
    let thresholds = read_toml("tests/parity/thresholds.toml");
    assert!(thresholds["min_sites_present"].as_integer().is_some());
    let pct = thresholds["agreement_pct"].as_table().unwrap();
    for (slug, v) in pct {
        assert!(
            repo_root()
                .join(format!("tests/parity/mappings/{slug}.toml"))
                .exists(),
            "Schwelle für unbekannten Export {slug}"
        );
        let v = v.as_float().unwrap_or_else(|| panic!("{slug}: keine Zahl"));
        assert!((0.0..=100.0).contains(&v), "{slug}: {v}");
    }
    for slug in ["redirect_chains", "redirects_to_errors", "crawl_overview"] {
        assert!(pct.contains_key(slug), "A3: Schwelle für {slug} fehlt");
    }
}
