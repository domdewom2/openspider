//! Ein URL-Eintrag samt optionalen Erweiterungen (OUT-122 … 128).
use fancy_regex::Regex;
use os_model::{ChangeFreqBasis, CrawlConfig, LastmodSource, SitemapGenConfig, Timestamp};
use os_store::CrawlFile;
use time::{format_description::well_known::Iso8601, Date};

use super::{db, escape_xml, sitemap_loc, MAX_LOC_CHARS, NAMESPACE, XML_DECLARATION};
use crate::ExportError;

const IMAGE_NS: &str = "http://www.google.com/schemas/sitemap-image/1.1";
const XHTML_NS: &str = "http://www.w3.org/1999/xhtml";

/// Ob der Crawl die benötigten Annotationen gespeichert hat (OUT-128).
pub fn hreflang_available(config: &CrawlConfig) -> bool {
    config.spider.hreflang.store
}

pub(super) struct Page<'a> {
    pub loc: &'a str,
    pub id: i64,
    pub depth: u32,
    pub modified: Option<&'a str>,
    pub crawled: Option<&'a str>,
}

pub(super) struct Settings<'a> {
    cfg: &'a SitemapGenConfig,
    exclude: Vec<Regex>,
    external: Vec<Regex>,
    images: bool,
    images_only: bool,
}

impl<'a> Settings<'a> {
    pub fn new(config: &'a CrawlConfig, images_only: bool) -> Result<Self, ExportError> {
        let cfg = &config.sitemap_gen;
        let invalid = |message: &str| ExportError::Write(format!("sitemap_gen: {message}"));
        if cfg.lastmod.enabled && cfg.lastmod.source == LastmodSource::Custom {
            let date = cfg.lastmod.custom_date.as_deref().unwrap_or("");
            if date.len() != 10 || Date::parse(date, &Iso8601::DATE).is_err() {
                return Err(invalid(
                    "lastmod.custom_date muss ein gültiges Datum YYYY-MM-DD sein",
                ));
            }
        }
        if cfg.priority.enabled
            && (cfg.priority.by_depth.is_empty()
                || cfg.priority.by_depth.iter().any(|v| {
                    !v.is_finite()
                        || !(0.0..=1.0).contains(v)
                        || (v * 10.0 - (v * 10.0).round()).abs() > 1e-9
                }))
        {
            return Err(invalid(
                "priority.by_depth benötigt Werte von 0.0 bis 1.0 in Schritten von 0.1",
            ));
        }
        if cfg.changefreq.enabled
            && cfg.changefreq.basis == ChangeFreqBasis::Depth
            && cfg.changefreq.by_depth.is_empty()
        {
            return Err(invalid("changefreq.by_depth darf nicht leer sein"));
        }
        if cfg.hreflang.enabled && !hreflang_available(config) {
            return Err(invalid(
                "hreflang benötigt im Crawl gespeicherte Annotationen",
            ));
        }
        Ok(Self {
            cfg,
            exclude: compile(&cfg.exclude_patterns)?,
            external: compile(&cfg.images.external_regex)?,
            images: images_only || cfg.images.enabled,
            images_only,
        })
    }

    pub fn header(&self) -> String {
        let mut header = format!("{XML_DECLARATION}<urlset xmlns=\"{NAMESPACE}\"");
        if self.images {
            header.push_str(&format!(" xmlns:image=\"{IMAGE_NS}\""));
        }
        if self.cfg.hreflang.enabled {
            header.push_str(&format!(" xmlns:xhtml=\"{XHTML_NS}\""));
        }
        header.push_str(">\n");
        header
    }

    pub fn excluded(&self, url: &str) -> Result<bool, ExportError> {
        matches(&self.exclude, url)
    }

    pub fn entry(
        &self,
        file: &CrawlFile,
        page: Page<'_>,
    ) -> Result<(Option<String>, Vec<String>), ExportError> {
        let mut warnings = Vec::new();
        let images = if self.images {
            self.images(file, page.id, page.loc, &mut warnings)?
        } else {
            Vec::new()
        };
        if self.images_only && images.is_empty() {
            return Ok((None, warnings));
        }
        let mut entry = format!("  <url>\n    <loc>{}</loc>\n", escape_xml(page.loc));
        let modified = page.modified.and_then(|m| Timestamp::from_db(m).ok());
        let crawled = page.crawled.and_then(|c| Timestamp::from_db(c).ok());
        if self.cfg.lastmod.enabled {
            let lastmod = match self.cfg.lastmod.source {
                LastmodSource::Server => modified
                    .or(crawled)
                    .map(|m| m.0.to_offset(time::UtcOffset::UTC).date().to_string()),
                LastmodSource::Custom => self.cfg.lastmod.custom_date.clone(),
            };
            if let Some(date) = lastmod {
                entry.push_str(&format!("    <lastmod>{date}</lastmod>\n"));
            }
        }
        if self.cfg.changefreq.enabled {
            let freq = match self.cfg.changefreq.basis {
                ChangeFreqBasis::Depth => self.cfg.changefreq.by_depth
                    [(page.depth as usize).min(self.cfg.changefreq.by_depth.len() - 1)]
                .as_str(),
                ChangeFreqBasis::LastModified => {
                    let age = crawled.zip(modified).map(|(c, m)| c.0 - m.0);
                    if modified.is_none()
                        || age.is_some_and(|a| {
                            a >= time::Duration::ZERO && a < time::Duration::hours(24)
                        })
                    {
                        "daily"
                    } else {
                        "monthly"
                    }
                }
            };
            entry.push_str(&format!("    <changefreq>{freq}</changefreq>\n"));
        }
        if self.cfg.priority.enabled {
            let value = self.cfg.priority.by_depth
                [(page.depth as usize).min(self.cfg.priority.by_depth.len() - 1)];
            entry.push_str(&format!("    <priority>{value:.1}</priority>\n"));
        }
        for image in images {
            entry.push_str(&format!(
                "    <image:image><image:loc>{}</image:loc></image:image>\n",
                escape_xml(&image)
            ));
        }
        if self.cfg.hreflang.enabled {
            // Nur vorhandene HTML-/HTTP-Annotationen, keine erfundenen oder aus Sitemaps übernommenen.
            let mut stmt = db(file.conn().prepare_cached(
                "SELECT h.code_raw, h.url FROM hreflang h LEFT JOIN urls t ON t.id = h.target_id \
                 WHERE h.url_id = ?1 AND h.source IN ('html', 'http') AND h.url IS NOT NULL \
                 AND (?2 = 0 OR (t.status_code = 200 AND t.indexability = 'Indexable' \
                      AND t.robots_blocked = 0 AND COALESCE(t.effective_index, 1) = 1 \
                      AND (t.canonical_url IS NULL OR t.canonical_url = '' OR COALESCE(t.canonical_self, 1) = 1))) \
                 ORDER BY h.source, h.position"))?;
            let mut rows = db(stmt.query((page.id, self.cfg.hreflang.only_valid_targets)))?;
            while let Some(row) = db(rows.next())? {
                let code: String = db(row.get(0))?;
                let target: String = db(row.get(1))?;
                if let Some(target) = usable_loc(&target) {
                    entry.push_str(&format!(
                        "    <xhtml:link rel=\"alternate\" hreflang=\"{}\" href=\"{}\"/>\n",
                        escape_xml(&code),
                        escape_xml(&target)
                    ));
                }
            }
        }
        entry.push_str("  </url>\n");
        Ok((Some(entry), warnings))
    }

    fn images(
        &self,
        file: &CrawlFile,
        page: i64,
        loc: &str,
        warnings: &mut Vec<String>,
    ) -> Result<Vec<String>, ExportError> {
        // image_instances ist nach Seite indiziert; url_counters hält die Zahl verschiedener
        // einbindender Seiten. Mehrere Instanzen desselben Bildes erzeugen nur einen Eintrag.
        let mut stmt = db(file.conn().prepare_cached(
            "SELECT u.url, u.is_internal FROM image_instances i JOIN urls u ON u.id = i.image_id \
             LEFT JOIN url_counters c ON c.url_id = u.id \
             WHERE i.page_id = ?1 AND u.status_code = 200 \
               AND (?2 IS NULL OR COALESCE(c.img_inlinks, 0) <= ?2) \
             GROUP BY u.id ORDER BY MIN(i.position), u.id",
        ))?;
        let mut rows = db(stmt.query((page, self.cfg.images.max_inlinks)))?;
        let mut images = Vec::new();
        while let Some(row) = db(rows.next())? {
            let url: String = db(row.get(0))?;
            let internal: bool = db(row.get(1))?;
            if !internal && !matches(&self.external, &url)? {
                continue;
            }
            let Some(url) = usable_loc(&url) else {
                continue;
            };
            if images.len() == 1_000 {
                let warning = format!("{loc}: mehr als 1000 Bilder; weitere Bilder ausgelassen");
                tracing::warn!("{warning}");
                warnings.push(warning);
                break;
            }
            images.push(url);
        }
        Ok(images)
    }
}

fn usable_loc(raw: &str) -> Option<String> {
    sitemap_loc(raw).filter(|s| s.chars().count() <= MAX_LOC_CHARS)
}

fn compile(patterns: &[String]) -> Result<Vec<Regex>, ExportError> {
    patterns
        .iter()
        .map(|p| Regex::new(p).map_err(|e| ExportError::Write(format!("Sitemap-Regex {p:?}: {e}"))))
        .collect()
}

fn matches(patterns: &[Regex], url: &str) -> Result<bool, ExportError> {
    for pattern in patterns {
        if pattern.is_match(url).map_err(|e| {
            ExportError::Write(format!(
                "Sitemap-Regex konnte nicht ausgewertet werden: {e}"
            ))
        })? {
            return Ok(true);
        }
    }
    Ok(false)
}
