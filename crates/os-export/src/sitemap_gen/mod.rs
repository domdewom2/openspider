//! XML-Sitemap-Generator (OUT-120, OUT-129, OUT-131).
//!
//! Erzeugt aus den gecrawlten Seiten eine Sitemap nach dem sitemaps.org-Protokoll 0.9.
//!
//! * **Auswahl (OUT-120):** interne Zeilen der Tabellen (`in_tables`), Inhaltstyp HTML, Status 200,
//!   indexierbar – kein noindex, nicht auf eine andere URL kanonisiert, nicht mit `rel=prev`, nicht durch
//!   robots.txt gesperrt. Die Prüfung stützt sich auf die gespeicherten Seitenfakten (`effective_index`,
//!   `canonical_self`, `has_rel_prev`) und nicht allein auf die Spalte `indexability`. Reihenfolge ist die
//!   Crawl-Reihenfolge (Entdeckungsreihenfolge, Start-URL zuerst); jede URL steht höchstens einmal darin.
//! * **Aufteilung (OUT-129):** höchstens `max_urls_per_file` (Standard 49 999) URLs und `max_bytes_per_file`
//!   (Standard 50 MiB, unkomprimiert) je Datei. Eine Datei heißt `sitemap.xml`; bei mehreren entstehen
//!   `sitemap_1.xml`, `sitemap_2.xml` … und `sitemap_index.xml` mit absoluten `<loc>` unter der Basis-URL
//!   (`SitemapOptions::base_url`, Standard: Ursprung der Start-URL plus `/`) und `<lastmod>` = Erzeugungsdatum.
//! * **Kodierung (OUT-131):** UTF-8 mit XML-Deklaration, ohne BOM; `& < > " '` als Entitäten; Pfade und Query
//!   prozentkodiert (`/über-uns` → `/%C3%BCber-uns`), IDN-Hosts in Punycode; URLs über 2 048 Zeichen fehlen in
//!   der Sitemap und stehen im Bericht ([`SitemapReport::skipped_too_long`]).
//!
//! Nicht Teil von M1 (P1/P2): `lastmod`, `changefreq`, `priority`, Bilder, hreflang, Einschlussoptionen,
//! Regex-Ausschluss, Segmente und gzip (OUT-121 ff.). Die Sitemap enthält deshalb je URL nur `<loc>`.
//! Ohne passende URL entsteht keine Datei (eine leere `urlset` verletzt das Schema).

use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use os_model::{Clock, SystemClock};
use os_store::CrawlFile;

use crate::error::ExportError;
use crate::fs_paths;
use crate::types::{ExportFileInfo, SitemapOptions, SitemapReport};

/// Namensraum des Protokolls.
pub const NAMESPACE: &str = "http://www.sitemaps.org/schemas/sitemap/0.9";
/// Dateiname bei genau einer Datei.
pub const SINGLE_FILE: &str = "sitemap.xml";
/// Dateiname des Index.
pub const INDEX_FILE: &str = "sitemap_index.xml";
/// Längste erlaubte `<loc>` in Zeichen (Protokoll).
pub const MAX_LOC_CHARS: usize = 2_048;
/// Höchste Zahl URLs, die das Protokoll je Datei erlaubt.
pub const PROTOCOL_MAX_URLS: u32 = 50_000;

const XML_DECLARATION: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n";
const URLSET_OPEN: &str = "<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n";
const URLSET_CLOSE: &str = "</urlset>\n";

/// Auswahl der Seiten (OUT-120), in Crawl-Reihenfolge.
pub const SELECTION_SQL: &str = "SELECT url FROM urls \
    WHERE is_internal = 1 AND in_tables = 1 \
      AND content_class = 'HTML' AND status_code = 200 \
      AND robots_blocked = 0 \
      AND indexability = 'Indexable' \
      AND COALESCE(effective_index, 1) = 1 \
      AND (canonical_url IS NULL OR canonical_url = '' OR COALESCE(canonical_self, 1) = 1) \
      AND COALESCE(has_rel_prev, 0) = 0 \
    ORDER BY id";

/// Erzeugt `sitemap.xml` bzw. `sitemap_N.xml` und bei Bedarf `sitemap_index.xml` (Basis-URL aus der
/// Start-URL, Erzeugungsdatum der Systemuhr).
pub fn generate_sitemap(
    file: &CrawlFile,
    opts: &SitemapOptions,
    out_dir: &Path,
) -> Result<SitemapReport, ExportError> {
    generate_sitemap_with(file, opts, None, &SystemClock, out_dir)
}

/// Wie [`generate_sitemap`], mit einer Uhr für das Erzeugungsdatum. Die Basis-URL für die `<loc>` des Index
/// (OUT-129) ist `base_url`, sonst [`SitemapOptions::base_url`], sonst der Ursprung der Start-URL.
pub fn generate_sitemap_with(
    file: &CrawlFile,
    opts: &SitemapOptions,
    base_url: Option<&str>,
    clock: &dyn Clock,
    out_dir: &Path,
) -> Result<SitemapReport, ExportError> {
    if opts.max_urls_per_file == 0 || opts.max_urls_per_file > PROTOCOL_MAX_URLS {
        return Err(ExportError::Write(format!(
            "max_urls_per_file muss zwischen 1 und {PROTOCOL_MAX_URLS} liegen (war {})",
            opts.max_urls_per_file
        )));
    }
    fs_paths::create_dir_all(out_dir).map_err(|source| ExportError::Io {
        path: out_dir.to_path_buf(),
        source,
    })?;

    let mut report = SitemapReport::default();
    let mut parts: Vec<Part> = Vec::new();
    let mut open: Option<Open> = None;

    let mut stmt = db(file.conn().prepare(SELECTION_SQL))?;
    let mut rows = db(stmt.query([]))?;
    while let Some(row) = db(rows.next())? {
        let stored: String = db(row.get(0))?;
        let Some(loc) = sitemap_loc(&stored) else {
            tracing::warn!("URL nicht für die Sitemap darstellbar, ausgelassen: {stored}");
            continue;
        };
        if loc.chars().count() > MAX_LOC_CHARS {
            report.skipped_too_long.push(loc);
            continue;
        }
        let entry = format!("  <url>\n    <loc>{}</loc>\n  </url>\n", escape_xml(&loc));
        let full = match &open {
            None => true,
            Some(o) => {
                o.urls >= opts.max_urls_per_file
                    || (o.urls > 0
                        && o.bytes + entry.len() as u64 + URLSET_CLOSE.len() as u64
                            > opts.max_bytes_per_file)
            }
        };
        if full {
            if let Some(o) = open.take() {
                parts.push(o.finish()?);
            }
            open = Some(Open::create(out_dir, parts.len() + 1)?);
        }
        if let Some(o) = open.as_mut() {
            o.write(entry.as_bytes())?;
            o.urls += 1;
        }
        report.urls += 1;
    }
    if let Some(o) = open.take() {
        parts.push(o.finish()?);
    }

    match parts.len() {
        0 => {}
        1 => {
            let part = parts.remove(0);
            let target = out_dir.join(SINGLE_FILE);
            std::fs::rename(fs_paths::fs_path(&part.path), fs_paths::fs_path(&target)).map_err(
                |source| ExportError::Io {
                    path: target.clone(),
                    source,
                },
            )?;
            report.files.push(ExportFileInfo {
                path: target,
                rows: part.urls,
                bytes: part.bytes,
                warnings: Vec::new(),
            });
        }
        _ => {
            let n_parts = parts.len() as u64;
            let base = base_url_for(file, base_url.or(opts.base_url.as_deref()))?;
            let date = clock.now().to_db();
            let date = date.get(..10).unwrap_or(&date).to_string();
            let mut index = String::new();
            index.push_str(XML_DECLARATION);
            index.push_str(&format!("<sitemapindex xmlns=\"{NAMESPACE}\">\n"));
            for part in &parts {
                let name = part
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                index.push_str(&format!(
                    "  <sitemap>\n    <loc>{}</loc>\n    <lastmod>{date}</lastmod>\n  </sitemap>\n",
                    escape_xml(&format!("{base}{name}"))
                ));
            }
            index.push_str("</sitemapindex>\n");
            let index_path = out_dir.join(INDEX_FILE);
            write_all(&index_path, index.as_bytes())?;
            for part in parts {
                report.files.push(ExportFileInfo {
                    path: part.path,
                    rows: part.urls,
                    bytes: part.bytes,
                    warnings: Vec::new(),
                });
            }
            report.files.push(ExportFileInfo {
                path: index_path,
                rows: n_parts,
                bytes: index.len() as u64,
                warnings: Vec::new(),
            });
        }
    }
    Ok(report)
}

fn db<T, E: Into<os_store::StoreError>>(r: Result<T, E>) -> Result<T, ExportError> {
    r.map_err(|e| ExportError::Store(e.into()))
}

fn write_all(path: &Path, bytes: &[u8]) -> Result<(), ExportError> {
    let mut f = fs_paths::create_file(path).map_err(|source| ExportError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    f.write_all(bytes).map_err(|source| ExportError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Basis-URL (mit `/` am Ende) für die `<loc>` des Index: ausdrücklich angegeben, sonst Ursprung der
/// Start-URL, im Listenmodus der Ursprung der ersten internen URL.
fn base_url_for(file: &CrawlFile, explicit: Option<&str>) -> Result<String, ExportError> {
    let from_url = |s: &str| {
        url::Url::parse(s).ok().map(|u| {
            let mut origin = u.origin().ascii_serialization();
            origin.push('/');
            origin
        })
    };
    let base = match explicit.map(str::trim).filter(|b| !b.is_empty()) {
        Some(b) => Some(if b.ends_with('/') {
            b.to_string()
        } else {
            format!("{b}/")
        }),
        None => {
            let meta = db(file.meta())?;
            meta.start_url.as_deref().and_then(from_url).or_else(|| {
                let first: Option<String> = file
                    .conn()
                    .query_row(
                        "SELECT url FROM urls WHERE is_internal = 1 AND in_tables = 1 ORDER BY id LIMIT 1",
                        [],
                        |r| r.get(0),
                    )
                    .ok();
                first.as_deref().and_then(from_url)
            })
        }
    };
    base.ok_or_else(|| {
        ExportError::Write(
            "keine Basis-URL für den Sitemap-Index (weder angegeben noch aus dem Crawl ableitbar)"
                .to_string(),
        )
    })
}

/// Eine fertig geschriebene Teildatei.
struct Part {
    path: PathBuf,
    urls: u64,
    bytes: u64,
}

/// Eine gerade geschriebene Teildatei.
struct Open {
    path: PathBuf,
    out: BufWriter<std::fs::File>,
    urls: u32,
    bytes: u64,
}

impl Open {
    fn create(dir: &Path, number: usize) -> Result<Self, ExportError> {
        let path = dir.join(format!("sitemap_{number}.xml"));
        let file = fs_paths::create_file(&path).map_err(|source| ExportError::Io {
            path: path.clone(),
            source,
        })?;
        let mut open = Open {
            path,
            out: BufWriter::with_capacity(256 * 1024, file),
            urls: 0,
            bytes: 0,
        };
        open.write(XML_DECLARATION.as_bytes())?;
        open.write(URLSET_OPEN.as_bytes())?;
        Ok(open)
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), ExportError> {
        self.out
            .write_all(bytes)
            .map_err(|source| ExportError::Io {
                path: self.path.clone(),
                source,
            })?;
        self.bytes += bytes.len() as u64;
        Ok(())
    }

    fn finish(mut self) -> Result<Part, ExportError> {
        self.write(URLSET_CLOSE.as_bytes())?;
        self.out.flush().map_err(|source| ExportError::Io {
            path: self.path.clone(),
            source,
        })?;
        Ok(Part {
            path: self.path,
            urls: u64::from(self.urls),
            bytes: self.bytes,
        })
    }
}

/// Maskiert Text für XML-Inhalt: `&`, `<`, `>`, `"` und `'` als Entitäten (OUT-131).
pub fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            other => out.push(other),
        }
    }
    out
}

/// `<loc>`-Form einer gespeicherten URL: absolut, ohne Fragment, Host in Punycode, Pfad und Query
/// prozentkodiert, rein ASCII. `None`, wenn sich die URL nicht parsen lässt.
pub fn sitemap_loc(stored: &str) -> Option<String> {
    let mut u = url::Url::parse(stored).ok()?;
    u.set_fragment(None);
    let s = u.as_str();
    // Der URL-Parser kodiert bereits alles außerhalb von ASCII; verbleibende Nicht-ASCII-Zeichen oder
    // Leerzeichen (nach Normalisierungsfehlern) werden zusätzlich als UTF-8-Bytes prozentkodiert.
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii() && !c.is_ascii_control() && c != ' ' {
            out.push(c);
        } else {
            let mut buf = [0u8; 4];
            for b in c.encode_utf8(&mut buf).bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: OUT-131
    #[test]
    fn escapes_the_five_xml_characters() {
        assert_eq!(
            escape_xml("a&b<c>d\"e'f"),
            "a&amp;b&lt;c&gt;d&quot;e&apos;f"
        );
    }

    // spec: OUT-131
    #[test]
    fn loc_is_percent_encoded_punycode_and_without_fragment() {
        assert_eq!(
            sitemap_loc("https://site.test/über-uns").as_deref(),
            Some("https://site.test/%C3%BCber-uns")
        );
        assert_eq!(
            sitemap_loc("https://bücher.example/x#frag").as_deref(),
            Some("https://xn--bcher-kva.example/x")
        );
        assert_eq!(
            sitemap_loc("https://site.test/a b").as_deref(),
            Some("https://site.test/a%20b")
        );
        assert_eq!(sitemap_loc("kein url"), None);
    }
}
