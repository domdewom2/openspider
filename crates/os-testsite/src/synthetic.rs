//! Synthetische Sites: deterministisch, ohne Dateien auf Platte, on-the-fly berechnet.
//!
//! Jede Antwort ist eine reine Funktion von `(seed, Pfad)` (bei robots.txt und Sitemaps
//! zusätzlich der Basis-URL). Es wird nichts vorgehalten; der Speicherbedarf ist
//! unabhängig von der Seitenzahl (1.000.000 Seiten kosten nur ein paar Bytes Layout).
//!
//! # Aufbau der Site
//!
//! * Seite 0 ist die Startseite `/`, alle anderen Seiten liegen unter `/s<id%100>/p<id>.html`.
//! * Die Seiten sind in Ebenen (= Klicktiefe) eingeteilt, konfigurierbar über
//!   `depth_distribution` (relative Gewichte je Ebene ab Tiefe 1). Ohne Angabe wächst jede
//!   Ebene um den Faktor `max(2, links_per_page / 3)`.
//! * Jede Seite hat genau einen Strukturelternteil in der Ebene davor (Round-Robin), der auf
//!   sie verlinkt. Dadurch ist jede Seite von der Startseite erreichbar, und die kürzeste
//!   Klicktiefe einer Seite ist exakt ihre Ebene (Zusatzlinks gehen nie tiefer als eine
//!   Ebene weiter und sind damit keine Abkürzungen).
//! * Jede Seite verlinkt auf ihre Strukturkinder, ihren Elternteil und füllt bis
//!   `links_per_page` mit deterministisch gewürfelten Zusatzlinks auf. Hat eine Seite mehr
//!   Strukturkinder als `links_per_page`, werden alle Kinder verlinkt (Erreichbarkeit geht vor).
//! * Seiten mit Strukturkindern und die Startseite haben immer Status 200. Nur Blattseiten
//!   können 301/404/5xx sein; die Wahrscheinlichkeiten werden so skaliert, dass die Anteile
//!   bezogen auf *alle* Seiten stimmen.

use std::fmt::Write as _;
use std::time::Duration;

use serde::Deserialize;

/// Maximale Anzahl URLs je Sitemap-Datei.
pub const SITEMAP_PAGE_SIZE: u64 = 50_000;

/// Reservierter Pfad für das Request-Log (JSON), wenn das Logging aktiv ist.
pub const REQUEST_LOG_PATH: &str = "/__testsite/requests";

const SALT_STATUS: u64 = 0x01;
const SALT_STATUS_KIND: u64 = 0x02;
const SALT_NOINDEX: u64 = 0x03;
const SALT_CANONICAL: u64 = 0x04;
const SALT_DUP_TITLE: u64 = 0x05;
const SALT_LARGE: u64 = 0x06;
const SALT_SLOW: u64 = 0x07;
const SALT_LINK: u64 = 0x08;
const SALT_TEXT: u64 = 0x09;

const WORDS: [&str; 32] = [
    "crawler",
    "seite",
    "inhalt",
    "link",
    "struktur",
    "tiefe",
    "titel",
    "analyse",
    "daten",
    "bericht",
    "technik",
    "ranking",
    "index",
    "signal",
    "qualität",
    "geschwindigkeit",
    "wachstum",
    "archiv",
    "thema",
    "kategorie",
    "produkt",
    "ratgeber",
    "vergleich",
    "werkzeug",
    "ergebnis",
    "beispiel",
    "projekt",
    "variante",
    "übersicht",
    "detail",
    "quelle",
    "verlauf",
];

/// Konfiguration einer synthetischen Site (`[synthetic]` in `scenario.toml`).
///
/// Alle `*_share`-Werte sind Anteile an allen Seiten zwischen 0 und 1 (0,05 = 5 %).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct SyntheticConfig {
    /// Anzahl Seiten inklusive Startseite.
    pub pages: u64,
    /// Zielanzahl Links je Seite.
    pub links_per_page: u32,
    /// Seed für alle Zufallsentscheidungen.
    pub seed: u64,
    /// Relative Gewichte der Ebenen ab Tiefe 1 (Tiefe 0 ist immer nur die Startseite).
    pub depth_distribution: Vec<f64>,
    /// Anteil Seiten mit 301-Weiterleitung auf die Elternseite.
    pub status_301_share: f64,
    /// Anteil Seiten mit Status 404.
    pub status_404_share: f64,
    /// Anteil Seiten mit Status 5xx (500 oder 503).
    pub status_5xx_share: f64,
    /// Anteil Seiten mit `<meta name="robots" content="noindex,follow">`.
    pub noindex_share: f64,
    /// Anteil Seiten, deren Canonical auf eine andere URL (Elternseite) zeigt.
    pub canonical_other_share: f64,
    /// Anteil Seiten, die sich einen von 25 Titeln teilen.
    pub duplicate_title_share: f64,
    /// Anteil „großer“ Seiten.
    pub large_page_share: f64,
    /// Größe großer Seiten in KiB.
    pub large_page_kb: u32,
    /// Künstliche Antwortverzögerung in Millisekunden (für langsame Seiten).
    pub delay_ms: u64,
    /// Anteil Seiten, die um `delay_ms` verzögert antworten (nur wirksam bei `delay_ms > 0`).
    pub slow_share: f64,
}

impl Default for SyntheticConfig {
    fn default() -> Self {
        Self {
            pages: 1000,
            links_per_page: 30,
            seed: 42,
            depth_distribution: Vec::new(),
            status_301_share: 0.0,
            status_404_share: 0.0,
            status_5xx_share: 0.0,
            noindex_share: 0.0,
            canonical_other_share: 0.0,
            duplicate_title_share: 0.0,
            large_page_share: 0.0,
            large_page_kb: 200,
            delay_ms: 0,
            slow_share: 1.0,
        }
    }
}

impl SyntheticConfig {
    /// Parst die Kurzform `synthetic:n=1000000,links=30[,seed=7,...]`.
    ///
    /// Erlaubt sind alle Feldnamen der Konfiguration (außer `depth_distribution`) sowie die
    /// Kurzformen `n` (= `pages`) und `links` (= `links_per_page`).
    pub fn parse_short(spec: &str) -> Result<Self, String> {
        let rest = spec
            .strip_prefix("synthetic:")
            .ok_or_else(|| format!("Kurzform muss mit `synthetic:` beginnen: {spec}"))?;
        let mut table = toml::Table::new();
        for pair in rest.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let (key, value) = pair
                .split_once('=')
                .ok_or_else(|| format!("Erwartet key=value, gefunden: {pair}"))?;
            let key = match key.trim() {
                "n" => "pages",
                "links" => "links_per_page",
                other => other,
            };
            let value = value.trim();
            let parsed = if let Ok(i) = value.parse::<i64>() {
                toml::Value::Integer(i)
            } else if let Ok(f) = value.parse::<f64>() {
                toml::Value::Float(f)
            } else {
                return Err(format!("Ungültiger Wert für {key}: {value}"));
            };
            table.insert(key.to_string(), parsed);
        }
        let cfg: Self = toml::Value::Table(table)
            .try_into()
            .map_err(|e: toml::de::Error| e.to_string())?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Prüft die Konfiguration auf Plausibilität (ohne Layout-Berechnung).
    pub fn validate(&self) -> Result<(), String> {
        if self.pages == 0 {
            return Err("pages muss >= 1 sein".into());
        }
        if self.pages > 1_000_000_000_000 {
            return Err("pages ist unplausibel groß".into());
        }
        let shares = [
            ("status_301_share", self.status_301_share),
            ("status_404_share", self.status_404_share),
            ("status_5xx_share", self.status_5xx_share),
            ("noindex_share", self.noindex_share),
            ("canonical_other_share", self.canonical_other_share),
            ("duplicate_title_share", self.duplicate_title_share),
            ("large_page_share", self.large_page_share),
            ("slow_share", self.slow_share),
        ];
        for (name, v) in shares {
            if !(0.0..=1.0).contains(&v) {
                return Err(format!("{name} muss zwischen 0 und 1 liegen (ist {v})"));
            }
        }
        if self.status_301_share + self.status_404_share + self.status_5xx_share > 1.0 {
            return Err("Summe der Statusanteile darf 1 nicht überschreiten".into());
        }
        if self
            .depth_distribution
            .iter()
            .any(|w| !w.is_finite() || *w <= 0.0)
        {
            return Err("depth_distribution braucht positive Gewichte".into());
        }
        Ok(())
    }
}

/// Status einer Seite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageStatus {
    /// 200 OK.
    Ok,
    /// 301 auf die Elternseite.
    Redirect,
    /// 404.
    NotFound,
    /// 500 oder 503.
    ServerError(u16),
}

impl PageStatus {
    /// HTTP-Statuscode.
    pub fn code(self) -> u16 {
        match self {
            PageStatus::Ok => 200,
            PageStatus::Redirect => 301,
            PageStatus::NotFound => 404,
            PageStatus::ServerError(c) => c,
        }
    }
}

/// Eigenschaften einer Seite (rein aus `(seed, id)` berechnet).
#[derive(Debug, Clone, PartialEq)]
pub struct PageInfo {
    /// Seiten-ID (0 = Startseite).
    pub id: u64,
    /// Ebene = Klicktiefe von der Startseite.
    pub depth: usize,
    /// HTTP-Status.
    pub status: PageStatus,
    /// `noindex` per Meta-Robots.
    pub noindex: bool,
    /// Canonical zeigt auf eine andere URL.
    pub canonical_other: bool,
    /// Gruppe des geteilten Titels (0..25), falls Duplikat-Titel.
    pub duplicate_title_group: Option<u64>,
    /// Große Seite.
    pub large: bool,
    /// Verzögerte Antwort.
    pub slow: bool,
}

/// Fertig berechnete Antwort einer synthetischen Site.
#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    /// HTTP-Statuscode.
    pub status: u16,
    /// Wert des Content-Type-Headers.
    pub content_type: &'static str,
    /// Wert des Location-Headers bei Weiterleitungen.
    pub location: Option<String>,
    /// Antwort-Body.
    pub body: Vec<u8>,
    /// Künstliche Verzögerung vor der Antwort.
    pub delay: Option<Duration>,
}

/// Berechnet Layout und Antworten einer synthetischen Site.
#[derive(Debug, Clone)]
pub struct SyntheticSite {
    cfg: SyntheticConfig,
    /// Seitenanzahl je Ebene; `sizes[0] == 1`.
    sizes: Vec<u64>,
    /// Erste ID je Ebene, letzter Eintrag = `pages`.
    starts: Vec<u64>,
    /// Faktor `pages / Anzahl statusfähiger Blattseiten`.
    status_scale: f64,
}

impl SyntheticSite {
    /// Erzeugt die Site; prüft die Konfiguration.
    pub fn new(cfg: SyntheticConfig) -> Result<Self, String> {
        cfg.validate()?;
        let sizes = build_sizes(&cfg)?;
        let mut starts = Vec::with_capacity(sizes.len() + 1);
        let mut acc = 0u64;
        for s in &sizes {
            starts.push(acc);
            acc += s;
        }
        starts.push(acc);
        debug_assert_eq!(acc, cfg.pages);

        let non_leaf: u64 = sizes.windows(2).map(|w| w[0].min(w[1])).sum();
        let eligible = cfg.pages - non_leaf - u64::from(cfg.pages == 1);
        let status_sum = cfg.status_301_share + cfg.status_404_share + cfg.status_5xx_share;
        let status_scale = if eligible == 0 {
            if status_sum > 0.0 {
                return Err("Statusanteile > 0 benötigen Blattseiten".into());
            }
            1.0
        } else {
            cfg.pages as f64 / eligible as f64
        };
        if status_sum * status_scale > 1.0 {
            return Err(format!(
                "Statusanteile ({status_sum}) sind zu groß: nur {eligible} von {} Seiten können \
                 Statusfehler tragen",
                cfg.pages
            ));
        }
        Ok(Self {
            cfg,
            sizes,
            starts,
            status_scale,
        })
    }

    /// Konfiguration der Site.
    pub fn config(&self) -> &SyntheticConfig {
        &self.cfg
    }

    /// Anzahl Seiten.
    pub fn pages(&self) -> u64 {
        self.cfg.pages
    }

    /// Seitenanzahl je Ebene (Index = Klicktiefe).
    pub fn level_sizes(&self) -> &[u64] {
        &self.sizes
    }

    /// Anzahl Sitemap-Dateien.
    pub fn sitemap_count(&self) -> u64 {
        self.cfg.pages.div_ceil(SITEMAP_PAGE_SIZE)
    }

    /// URL-Pfad einer Seite.
    pub fn page_path(id: u64) -> String {
        if id == 0 {
            "/".to_string()
        } else {
            format!("/s{}/p{}.html", id % 100, id)
        }
    }

    /// Seiten-ID zu einem URL-Pfad (nur kanonische Schreibweise).
    pub fn parse_page_path(&self, path: &str) -> Option<u64> {
        if path == "/" {
            return Some(0);
        }
        let rest = path.strip_prefix("/s")?;
        let (_, file) = rest.split_once("/p")?;
        let id: u64 = file.strip_suffix(".html")?.parse().ok()?;
        (id >= 1 && id < self.cfg.pages && Self::page_path(id) == path).then_some(id)
    }

    fn level_of(&self, id: u64) -> usize {
        self.starts.partition_point(|&s| s <= id) - 1
    }

    fn parent_of(&self, id: u64) -> Option<u64> {
        let k = self.level_of(id);
        if k == 0 {
            return None;
        }
        let j = id - self.starts[k];
        Some(self.starts[k - 1] + j % self.sizes[k - 1])
    }

    fn has_children(&self, id: u64) -> bool {
        let k = self.level_of(id);
        k + 1 < self.sizes.len() && id - self.starts[k] < self.sizes[k + 1]
    }

    fn unit(&self, id: u64, salt: u64) -> f64 {
        unit(hash(self.cfg.seed, id, salt))
    }

    /// Berechnet die Eigenschaften einer Seite.
    pub fn page_info(&self, id: u64) -> PageInfo {
        let depth = self.level_of(id);
        let cfg = &self.cfg;
        let status = if id == 0 || self.has_children(id) {
            PageStatus::Ok
        } else {
            let u = self.unit(id, SALT_STATUS);
            let s = self.status_scale;
            let p301 = cfg.status_301_share * s;
            let p404 = p301 + cfg.status_404_share * s;
            let p5xx = p404 + cfg.status_5xx_share * s;
            if u < p301 {
                PageStatus::Redirect
            } else if u < p404 {
                PageStatus::NotFound
            } else if u < p5xx {
                let code = if hash(cfg.seed, id, SALT_STATUS_KIND) & 1 == 0 {
                    500
                } else {
                    503
                };
                PageStatus::ServerError(code)
            } else {
                PageStatus::Ok
            }
        };
        let ok_inner = id != 0;
        PageInfo {
            id,
            depth,
            status,
            noindex: ok_inner && self.unit(id, SALT_NOINDEX) < cfg.noindex_share,
            canonical_other: ok_inner && self.unit(id, SALT_CANONICAL) < cfg.canonical_other_share,
            duplicate_title_group: (ok_inner
                && self.unit(id, SALT_DUP_TITLE) < cfg.duplicate_title_share)
                .then(|| hash(cfg.seed, id, SALT_DUP_TITLE) % 25),
            large: self.unit(id, SALT_LARGE) < cfg.large_page_share,
            slow: cfg.delay_ms > 0 && self.unit(id, SALT_SLOW) < cfg.slow_share,
        }
    }

    /// Ausgehende Links einer Seite in Dokumentreihenfolge: Elternseite, Strukturkinder,
    /// danach Zusatzlinks bis `links_per_page`.
    pub fn links_of(&self, id: u64) -> Vec<u64> {
        let target = self.cfg.links_per_page as usize;
        let mut links = Vec::with_capacity(target.max(8));
        if let Some(p) = self.parent_of(id) {
            links.push(p);
        }
        let k = self.level_of(id);
        if k + 1 < self.sizes.len() {
            let step = self.sizes[k];
            let mut child = self.starts[k + 1] + (id - self.starts[k]);
            let end = self.starts[k + 2];
            while child < end {
                links.push(child);
                child += step;
            }
        }
        // Zusatzlinks nur auf Seiten bis eine Ebene tiefer: keine Abkürzungen.
        let limit = self.starts[(k + 2).min(self.sizes.len())];
        let mut slot = 0u64;
        while links.len() < target {
            let mut candidate = id;
            for attempt in 0..4 {
                candidate =
                    hash(self.cfg.seed, id, SALT_LINK ^ (slot << 8) ^ (attempt << 40)) % limit;
                if candidate != id && !links.contains(&candidate) {
                    break;
                }
            }
            slot += 1;
            if candidate == id {
                if limit == 1 {
                    break; // Einzelseite hat keine Linkziele.
                }
                continue;
            }
            links.push(candidate);
        }
        links
    }

    /// Berechnet die Antwort für einen Pfad (ohne Query). `None` = unbekannter Pfad (404).
    pub fn render(&self, path: &str, base_url: &str) -> Option<Rendered> {
        match path {
            "/robots.txt" => {
                let body = format!(
                    "User-agent: *\nDisallow: /private/\nSitemap: {base_url}/sitemap.xml\n"
                );
                return Some(simple(200, "text/plain; charset=utf-8", body));
            }
            "/sitemap.xml" => return Some(self.sitemap_index(base_url)),
            _ => {}
        }
        if let Some(n) = path
            .strip_prefix("/sitemap-")
            .and_then(|r| r.strip_suffix(".xml"))
            .and_then(|r| r.parse::<u64>().ok())
        {
            if n >= 1 && n <= self.sitemap_count() && path == format!("/sitemap-{n}.xml") {
                return Some(self.sitemap(n, base_url));
            }
            return None;
        }
        let id = self.parse_page_path(path)?;
        Some(self.render_page(id))
    }

    fn sitemap_index(&self, base_url: &str) -> Rendered {
        let mut s = String::with_capacity(256 + 96 * self.sitemap_count() as usize);
        s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        s.push_str("<sitemapindex xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");
        for n in 1..=self.sitemap_count() {
            let _ = writeln!(
                s,
                "<sitemap><loc>{base_url}/sitemap-{n}.xml</loc></sitemap>"
            );
        }
        s.push_str("</sitemapindex>\n");
        simple(200, "application/xml", s)
    }

    /// Sitemap `n` (1-basiert) enthält die Seiten-IDs `[(n-1)*50000, n*50000)` mit Status 200.
    fn sitemap(&self, n: u64, base_url: &str) -> Rendered {
        let first = (n - 1) * SITEMAP_PAGE_SIZE;
        let last = (first + SITEMAP_PAGE_SIZE).min(self.cfg.pages);
        let mut s = String::with_capacity(128 + 80 * (last - first) as usize);
        s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        s.push_str("<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");
        for id in first..last {
            if self.page_info(id).status == PageStatus::Ok {
                let _ = writeln!(s, "<url><loc>{base_url}{}</loc></url>", Self::page_path(id));
            }
        }
        s.push_str("</urlset>\n");
        simple(200, "application/xml", s)
    }

    fn render_page(&self, id: u64) -> Rendered {
        let info = self.page_info(id);
        let delay = info.slow.then(|| Duration::from_millis(self.cfg.delay_ms));
        let parent_path = self.parent_of(id).map(Self::page_path);
        match info.status {
            PageStatus::Redirect => {
                let target = parent_path.unwrap_or_else(|| "/".to_string());
                return Rendered {
                    status: 301,
                    content_type: "text/html; charset=utf-8",
                    location: Some(target),
                    body: b"<!doctype html><title>Moved</title>".to_vec(),
                    delay,
                };
            }
            PageStatus::NotFound => {
                return Rendered {
                    status: 404,
                    content_type: "text/html; charset=utf-8",
                    location: None,
                    body: b"<!doctype html><html><head><title>Nicht gefunden</title></head>\
                            <body><h1>404</h1></body></html>"
                        .to_vec(),
                    delay,
                };
            }
            PageStatus::ServerError(code) => {
                return Rendered {
                    status: code,
                    content_type: "text/html; charset=utf-8",
                    location: None,
                    body: b"<!doctype html><html><head><title>Serverfehler</title></head>\
                            <body><h1>Serverfehler</h1></body></html>"
                        .to_vec(),
                    delay,
                };
            }
            PageStatus::Ok => {}
        }

        let path = Self::page_path(id);
        let title = match info.duplicate_title_group {
            Some(g) => format!("Gemeinsamer Seitentitel {g}"),
            None if id == 0 => "Startseite – Synthetic Site".to_string(),
            None => format!("Seite {id} – Synthetic Site"),
        };
        let canonical = if info.canonical_other {
            parent_path.as_deref().unwrap_or("/")
        } else {
            path.as_str()
        };

        let mut s = String::with_capacity(4096);
        s.push_str("<!doctype html>\n<html lang=\"de\">\n<head>\n<meta charset=\"utf-8\">\n");
        let _ = writeln!(s, "<title>{title}</title>");
        let _ = writeln!(
            s,
            "<meta name=\"description\" content=\"Beschreibung zu Seite {id}.\">"
        );
        if info.noindex {
            s.push_str("<meta name=\"robots\" content=\"noindex,follow\">\n");
        }
        let _ = writeln!(s, "<link rel=\"canonical\" href=\"{canonical}\">");
        s.push_str("</head>\n<body>\n");
        let _ = writeln!(s, "<h1>Seite {id}</h1>");
        s.push_str("<p>");
        self.push_words(&mut s, id, 90);
        s.push_str("</p>\n<ul>\n");
        for target in self.links_of(id) {
            let _ = writeln!(
                s,
                "<li><a href=\"{}\">Seite {target}</a></li>",
                Self::page_path(target)
            );
        }
        s.push_str("</ul>\n");
        if info.large {
            let want = self.cfg.large_page_kb as usize * 1024;
            let mut block = String::new();
            self.push_words(&mut block, id ^ 0xA5A5, 120);
            while s.len() < want {
                s.push_str("<p>");
                s.push_str(&block);
                s.push_str("</p>\n");
            }
        }
        s.push_str("</body>\n</html>\n");

        Rendered {
            status: 200,
            content_type: "text/html; charset=utf-8",
            location: None,
            body: s.into_bytes(),
            delay,
        }
    }

    fn push_words(&self, out: &mut String, id: u64, count: usize) {
        let mut state = hash(self.cfg.seed, id, SALT_TEXT);
        for i in 0..count {
            state = mix(state);
            if i > 0 {
                out.push(' ');
            }
            out.push_str(WORDS[(state & 31) as usize]);
        }
        out.push('.');
    }
}

fn simple(status: u16, content_type: &'static str, body: String) -> Rendered {
    Rendered {
        status,
        content_type,
        location: None,
        body: body.into_bytes(),
        delay: None,
    }
}

/// Bestimmt die Seitenanzahl je Ebene (Index 0 = Startseite).
fn build_sizes(cfg: &SyntheticConfig) -> Result<Vec<u64>, String> {
    let mut sizes = vec![1u64];
    if cfg.pages == 1 {
        return Ok(sizes);
    }
    let rest = cfg.pages - 1;
    if cfg.depth_distribution.is_empty() {
        let ratio = u64::from(cfg.links_per_page / 3).max(2);
        let mut remaining = rest;
        let mut level = 1u64;
        while remaining > 0 {
            level = level.saturating_mul(ratio);
            let take = level.min(remaining);
            sizes.push(take);
            remaining -= take;
        }
        return Ok(sizes);
    }
    let levels = (cfg.depth_distribution.len() as u64).min(rest) as usize;
    let weights = &cfg.depth_distribution[..levels];
    let total: f64 = weights.iter().sum();
    let mut counts: Vec<u64> = weights
        .iter()
        .map(|w| ((w * rest as f64 / total).floor() as u64).max(1))
        .collect();
    let mut sum: u64 = counts.iter().sum();
    let argmax = |c: &[u64]| {
        c.iter()
            .enumerate()
            .max_by_key(|(_, v)| **v)
            .map(|(i, _)| i)
            .unwrap_or(0)
    };
    if sum < rest {
        let i = argmax(&counts);
        counts[i] += rest - sum;
        sum = rest;
    }
    while sum > rest {
        let i = argmax(&counts);
        counts[i] -= 1;
        sum -= 1;
    }
    sizes.extend(counts);
    Ok(sizes)
}

/// SplitMix64-Finalizer.
#[inline]
fn mix(x: u64) -> u64 {
    let mut x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

#[inline]
fn hash(seed: u64, id: u64, salt: u64) -> u64 {
    mix(seed ^ mix(id ^ mix(salt)))
}

/// Bildet 64 Bit gleichverteilt auf `[0, 1)` ab.
#[inline]
fn unit(x: u64) -> f64 {
    (x >> 11) as f64 / (1u64 << 53) as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, VecDeque};

    fn site(cfg: SyntheticConfig) -> SyntheticSite {
        SyntheticSite::new(cfg).unwrap()
    }

    /// Extrahiert die Ziele aller `<a href>`-Links.
    pub(crate) fn anchors(html: &str) -> Vec<String> {
        html.split("<a href=\"")
            .skip(1)
            .filter_map(|p| p.split('"').next())
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn short_form_parses() {
        let cfg = SyntheticConfig::parse_short("synthetic:n=1000000,links=30").unwrap();
        assert_eq!(cfg.pages, 1_000_000);
        assert_eq!(cfg.links_per_page, 30);
        assert_eq!(cfg.seed, 42);
        let cfg =
            SyntheticConfig::parse_short("synthetic:n=500,seed=7,status_404_share=0.1").unwrap();
        assert_eq!(cfg.seed, 7);
        assert!((cfg.status_404_share - 0.1).abs() < 1e-12);
        assert!(SyntheticConfig::parse_short("synthetic:n=abc").is_err());
        assert!(SyntheticConfig::parse_short("synthetic:bogus=1").is_err());
        assert!(SyntheticConfig::parse_short("synthetic:n=0").is_err());
        assert!(SyntheticConfig::parse_short("n=10").is_err());
    }

    #[test]
    fn config_from_toml() {
        let cfg: SyntheticConfig = toml::from_str(
            "pages = 1000000\nlinks_per_page = 30\nseed = 42\ndepth_distribution = [1, 5, 50]\nstatus_301_share = 0.02",
        )
        .unwrap();
        assert_eq!(cfg.pages, 1_000_000);
        assert_eq!(cfg.depth_distribution, vec![1.0, 5.0, 50.0]);
        assert!(toml::from_str::<SyntheticConfig>("unknown = 1").is_err());
    }

    #[test]
    fn rejects_invalid_configs() {
        let bad = |f: &dyn Fn(&mut SyntheticConfig)| {
            let mut c = SyntheticConfig::default();
            f(&mut c);
            assert!(SyntheticSite::new(c).is_err());
        };
        bad(&|c| c.pages = 0);
        bad(&|c| c.noindex_share = 1.5);
        bad(&|c| {
            c.status_404_share = 0.7;
            c.status_5xx_share = 0.7;
        });
        bad(&|c| c.depth_distribution = vec![1.0, -2.0]);
        // Mehr Fehlerseiten als Blattseiten möglich sind.
        bad(&|c| {
            c.pages = 12;
            c.status_404_share = 0.9;
        });
    }

    #[test]
    fn deterministic_render() {
        let cfg = SyntheticConfig {
            pages: 1000,
            status_301_share: 0.05,
            status_404_share: 0.05,
            status_5xx_share: 0.02,
            noindex_share: 0.1,
            canonical_other_share: 0.1,
            duplicate_title_share: 0.1,
            large_page_share: 0.02,
            ..Default::default()
        };
        let a = site(cfg.clone());
        let b = site(cfg.clone());
        for id in 0..1000 {
            let p = SyntheticSite::page_path(id);
            assert_eq!(a.render(&p, "http://x"), b.render(&p, "http://x"), "{p}");
            assert_eq!(a.render(&p, "http://x"), a.render(&p, "http://x"));
        }
        assert_eq!(
            a.render("/sitemap-1.xml", "http://x"),
            b.render("/sitemap-1.xml", "http://x")
        );
        let other = site(SyntheticConfig { seed: 43, ..cfg });
        let differing = (0..1000)
            .filter(|&id| {
                let p = SyntheticSite::page_path(id);
                a.render(&p, "http://x") != other.render(&p, "http://x")
            })
            .count();
        assert!(differing > 900, "anderer Seed ändert fast alle Seiten");
    }

    #[test]
    fn path_roundtrip_and_rejects_noncanonical() {
        let s = site(SyntheticConfig::default());
        assert_eq!(s.parse_page_path("/"), Some(0));
        for id in [1u64, 7, 99, 100, 101, 999] {
            assert_eq!(s.parse_page_path(&SyntheticSite::page_path(id)), Some(id));
        }
        assert_eq!(s.parse_page_path("/s1/p1000.html"), None);
        assert_eq!(s.parse_page_path("/s2/p1.html"), None);
        assert_eq!(s.parse_page_path("/s1/p01.html"), None);
        assert_eq!(s.parse_page_path("/s0/p0.html"), None);
        assert_eq!(s.parse_page_path("/nothing"), None);
        assert!(s.render("/nothing", "http://x").is_none());
        assert!(s.render("/sitemap-0.xml", "http://x").is_none());
        assert!(s.render("/sitemap-2.xml", "http://x").is_none());
    }

    #[test]
    fn all_pages_reachable_via_bfs_10k() {
        let n = 10_000u64;
        let s = site(SyntheticConfig {
            pages: n,
            status_301_share: 0.03,
            status_404_share: 0.03,
            status_5xx_share: 0.02,
            ..Default::default()
        });
        // BFS über die tatsächlich gerenderten HTML-Seiten, Weiterleitungen werden gefolgt.
        let mut depth: HashMap<String, usize> = HashMap::new();
        let mut queue = VecDeque::new();
        depth.insert("/".into(), 0);
        queue.push_back("/".to_string());
        let mut visited_ids = std::collections::HashSet::new();
        while let Some(path) = queue.pop_front() {
            let d = depth[&path];
            let id = s
                .parse_page_path(&path)
                .expect("nur gültige Pfade verlinkt");
            visited_ids.insert(id);
            let r = s.render(&path, "http://x").unwrap();
            let next: Vec<String> = match r.status {
                200 => anchors(std::str::from_utf8(&r.body).unwrap()),
                301 => vec![r.location.unwrap()],
                _ => vec![],
            };
            for link in next {
                if !depth.contains_key(&link) {
                    depth.insert(link.clone(), d + 1);
                    queue.push_back(link);
                }
            }
        }
        assert_eq!(visited_ids.len() as u64, n, "alle Seiten erreichbar");
        // Klicktiefe == Ebene.
        for (path, d) in &depth {
            let id = s.parse_page_path(path).unwrap();
            assert_eq!(s.page_info(id).depth, *d, "{path}");
        }
        let max = depth.values().max().unwrap();
        assert_eq!(*max + 1, s.level_sizes().len());
    }

    #[test]
    fn reachability_with_explicit_depth_distribution() {
        for n in [1u64, 2, 3, 10, 257, 5000] {
            let s = site(SyntheticConfig {
                pages: n,
                links_per_page: 10,
                depth_distribution: vec![1.0, 4.0, 20.0, 30.0, 50.0, 80.0],
                ..Default::default()
            });
            assert_eq!(s.level_sizes().iter().sum::<u64>(), n, "n={n}");
            assert!(s.level_sizes().iter().all(|&c| c >= 1));
            let mut seen = vec![false; n as usize];
            let mut queue = VecDeque::from([0u64]);
            seen[0] = true;
            let mut count = 1;
            while let Some(id) = queue.pop_front() {
                for t in s.links_of(id) {
                    assert!(t < n);
                    if !seen[t as usize] {
                        seen[t as usize] = true;
                        count += 1;
                        queue.push_back(t);
                    }
                }
            }
            assert_eq!(count, n, "n={n}");
        }
    }

    #[test]
    fn link_count_matches_target() {
        let s = site(SyntheticConfig {
            pages: 20_000,
            ..Default::default()
        });
        let mut total = 0usize;
        for id in 0..20_000 {
            let l = s.links_of(id);
            assert!(l.len() >= 30, "id {id}: {}", l.len());
            assert!(!l.contains(&id));
            total += l.len();
        }
        let avg = total as f64 / 20_000.0;
        assert!((30.0..31.0).contains(&avg), "avg {avg}");
    }

    fn pct(count: usize, n: usize) -> f64 {
        count as f64 / n as f64 * 100.0
    }

    #[test]
    fn status_shares_within_one_percent() {
        let n = 10_000usize;
        let s = site(SyntheticConfig {
            pages: n as u64,
            status_301_share: 0.05,
            status_404_share: 0.04,
            status_5xx_share: 0.03,
            ..Default::default()
        });
        let (mut r301, mut r404, mut r5xx, mut ok) = (0, 0, 0, 0);
        for id in 0..n as u64 {
            match s
                .render(&SyntheticSite::page_path(id), "http://x")
                .unwrap()
                .status
            {
                301 => r301 += 1,
                404 => r404 += 1,
                500 | 503 => r5xx += 1,
                200 => ok += 1,
                other => panic!("unerwarteter Status {other}"),
            }
        }
        assert!((pct(r301, n) - 5.0).abs() <= 1.0, "301: {}", pct(r301, n));
        assert!((pct(r404, n) - 4.0).abs() <= 1.0, "404: {}", pct(r404, n));
        assert!((pct(r5xx, n) - 3.0).abs() <= 1.0, "5xx: {}", pct(r5xx, n));
        assert_eq!(r301 + r404 + r5xx + ok, n);
    }

    #[test]
    fn attribute_shares_within_one_percent() {
        let n = 10_000usize;
        let s = site(SyntheticConfig {
            pages: n as u64,
            noindex_share: 0.10,
            canonical_other_share: 0.06,
            duplicate_title_share: 0.20,
            large_page_share: 0.02,
            large_page_kb: 64,
            delay_ms: 5,
            slow_share: 0.07,
            ..Default::default()
        });
        let infos: Vec<PageInfo> = (0..n as u64).map(|i| s.page_info(i)).collect();
        let share = |f: &dyn Fn(&PageInfo) -> bool| pct(infos.iter().filter(|i| f(i)).count(), n);
        assert!((share(&|i| i.noindex) - 10.0).abs() <= 1.0);
        assert!((share(&|i| i.canonical_other) - 6.0).abs() <= 1.0);
        assert!((share(&|i| i.duplicate_title_group.is_some()) - 20.0).abs() <= 1.0);
        assert!((share(&|i| i.large) - 2.0).abs() <= 1.0);
        assert!((share(&|i| i.slow) - 7.0).abs() <= 1.0);
        assert!(
            infos[0].status == PageStatus::Ok && !infos[0].noindex && !infos[0].canonical_other
        );
    }

    #[test]
    fn page_features_appear_in_html() {
        let s = site(SyntheticConfig {
            pages: 200,
            noindex_share: 1.0,
            canonical_other_share: 1.0,
            duplicate_title_share: 1.0,
            large_page_share: 1.0,
            large_page_kb: 64,
            delay_ms: 30,
            ..Default::default()
        });
        let r = s
            .render(&SyntheticSite::page_path(150), "http://x")
            .unwrap();
        let html = std::str::from_utf8(&r.body).unwrap();
        assert!(html.contains("content=\"noindex,follow\""));
        assert!(html.contains("<title>Gemeinsamer Seitentitel "));
        assert!(html.len() >= 64 * 1024);
        assert_eq!(r.delay, Some(Duration::from_millis(30)));
        let info = s.page_info(150);
        let parent = SyntheticSite::page_path(s.parent_of(150).unwrap());
        assert!(info.canonical_other);
        assert!(html.contains(&format!("<link rel=\"canonical\" href=\"{parent}\">")));
        // Startseite bleibt unverändert indexierbar.
        let home = s.render("/", "http://x").unwrap();
        let home = std::str::from_utf8(&home.body).unwrap();
        assert!(!home.contains("noindex"));
        assert!(home.contains("<link rel=\"canonical\" href=\"/\">"));
    }

    #[test]
    fn sitemap_index_and_files() {
        let n = 120_000u64;
        let s = site(SyntheticConfig {
            pages: n,
            ..Default::default()
        });
        assert_eq!(s.sitemap_count(), 3);
        let idx = s.render("/sitemap.xml", "http://h:1").unwrap();
        let idx = std::str::from_utf8(&idx.body).unwrap();
        assert_eq!(idx.matches("<sitemap>").count(), 3);
        assert!(idx.contains("<loc>http://h:1/sitemap-3.xml</loc>"));
        let counts: Vec<usize> = (1..=3)
            .map(|k| {
                let r = s
                    .render(&format!("/sitemap-{k}.xml"), "http://h:1")
                    .unwrap();
                std::str::from_utf8(&r.body)
                    .unwrap()
                    .matches("<loc>")
                    .count()
            })
            .collect();
        assert_eq!(counts, vec![50_000, 50_000, 20_000]);
        let robots = s.render("/robots.txt", "http://h:1").unwrap();
        assert!(std::str::from_utf8(&robots.body)
            .unwrap()
            .contains("Sitemap: http://h:1/sitemap.xml"));
    }

    #[test]
    fn sitemap_lists_only_ok_pages() {
        let s = site(SyntheticConfig {
            pages: 2000,
            status_404_share: 0.1,
            ..Default::default()
        });
        let r = s.render("/sitemap-1.xml", "http://h").unwrap();
        let xml = std::str::from_utf8(&r.body).unwrap();
        let listed = xml.matches("<loc>").count();
        let ok = (0..2000)
            .filter(|&i| s.page_info(i).status == PageStatus::Ok)
            .count();
        assert_eq!(listed, ok);
        assert!(listed < 2000);
    }

    #[test]
    fn million_pages_layout_is_cheap_and_default_depths() {
        let s = site(SyntheticConfig {
            pages: 1_000_000,
            ..Default::default()
        });
        assert_eq!(s.level_sizes()[..4], [1, 10, 100, 1000]);
        assert_eq!(s.level_sizes().iter().sum::<u64>(), 1_000_000);
        let last = SyntheticSite::page_path(999_999);
        let r = s.render(&last, "http://x").unwrap();
        assert_eq!(r.status, 200);
        assert!(s.links_of(999_999).len() >= 30);
    }
}
