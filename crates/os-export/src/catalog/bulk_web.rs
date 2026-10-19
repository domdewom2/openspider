//! Web-Daten: Quelltext, Text, Header und Cookies (OUT-045 bis OUT-049).
use crate::catalog::reports::ReportRequirement;
use crate::catalog::{TableEntry, TableJob};
use crate::{ExportError, ExportFileInfo, Format, TableWriter};
use os_analyze::ColumnKind;
use os_model::{CrawlConfig, UrlId, Value};
use os_store::CrawlFile;
use std::path::Path;

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

const ENTRIES: &[TableEntry] = &[
    TableEntry {
        name: "Web:All Page Source",
        aliases: &[],
        file_stem: "Web:All Page Source",
        header: &["Address", "File", "Source Type"],
        header_fn: None,
        requires: &[ReportRequirement::Config {
            key: "extraction.store_html",
            what: "HTML storage",
        }],
        owner: "M2-D18",
        run: page_index,
    },
    TableEntry {
        name: "Web:All Page Text",
        aliases: &[],
        file_stem: "Web:All Page Text",
        header: &["Address", "File"],
        header_fn: None,
        requires: &[ReportRequirement::Config {
            key: "extraction.store_html",
            what: "HTML storage",
        }],
        owner: "M2-D18",
        run: page_index,
    },
    TableEntry {
        name: "Web:All Rendered Page Source",
        aliases: &["Web:All Page Source:Rendered"],
        file_stem: "Web:All Rendered Page Source",
        header: &["Address", "File", "Source Type"],
        header_fn: None,
        requires: &[
            ReportRequirement::Rendering,
            ReportRequirement::Config {
                key: "extraction.store_rendered_html",
                what: "rendered HTML storage",
            },
        ],
        owner: "M2-D18",
        run: page_index,
    },
    TableEntry {
        name: "Content:All Page Text",
        aliases: &[],
        file_stem: "Content:All Page Text",
        header: &["Address", "Text"],
        header_fn: None,
        requires: &[ReportRequirement::Config {
            key: "extraction.store_html",
            what: "HTML storage",
        }],
        owner: "M2-D18",
        run: page_text,
    },
    TableEntry {
        name: "Web:All HTTP Request Headers",
        aliases: &[],
        file_stem: "Web:All HTTP Request Headers",
        header: &["Address", "Header Name", "Header Value"],
        header_fn: None,
        requires: &[ReportRequirement::Config {
            key: "extraction.http_headers",
            what: "HTTP header extraction",
        }],
        owner: "M2-D18",
        run: headers,
    },
    TableEntry {
        name: "Web:All HTTP Response Headers",
        aliases: &[],
        file_stem: "Web:All HTTP Response Headers",
        header: &["Address", "Header Name", "Header Value"],
        header_fn: None,
        requires: &[ReportRequirement::Config {
            key: "extraction.http_headers",
            what: "HTTP header extraction",
        }],
        owner: "M2-D18",
        run: headers,
    },
    TableEntry {
        name: "Web:All Cookies",
        aliases: &[],
        file_stem: "Web:All Cookies",
        header: &[
            "Address",
            "Cookie Name",
            "Cookie Value",
            "Domain",
            "Path",
            "Expires",
            "Max-Age",
            "Secure",
            "HttpOnly",
            "SameSite",
            "Type",
            "Session",
            "First-Party",
        ],
        requires: &[ReportRequirement::Config {
            key: "extraction.cookies",
            what: "cookie extraction",
        }],
        owner: "M2-D18",
        run: cookies,
        header_fn: None,
    },
];

/// File exports are deliberately separate from CSV/XLSX table exports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageExport {
    Original,
    Rendered,
    Text,
}

pub fn page_export(name: &str) -> Option<PageExport> {
    match name {
        "Web:All Page Source" => Some(PageExport::Original),
        "Web:All Rendered Page Source" => Some(PageExport::Rendered),
        "Web:All Page Text" => Some(PageExport::Text),
        _ => None,
    }
}

/// A rendered source choice requires a stored DOM, including when settings changed after the crawl.
pub fn availability_warnings(name: &str, file: &CrawlFile) -> Vec<String> {
    if page_export(name) != Some(PageExport::Rendered) {
        return Vec::new();
    }
    match file.conn().query_row(
        "SELECT EXISTS(SELECT 1 FROM render_pages r JOIN urls u ON u.id=r.url_id WHERE r.html IS NOT NULL AND u.is_internal=1 AND u.in_tables=1 AND u.content_class='HTML')",
        [], |r| r.get::<_, bool>(0),
    ) {
        Ok(true) => Vec::new(),
        Ok(false) => vec![format!("report '{name}' requires stored rendered HTML")],
        Err(error) => vec![format!("report '{name}': rendered HTML availability: {error}")],
    }
}

fn enabled(cfg: &CrawlConfig, kind: PageExport) -> bool {
    match kind {
        PageExport::Original | PageExport::Text => cfg.extraction.store_html,
        PageExport::Rendered => {
            cfg.extraction.store_rendered_html
                && cfg.rendering.mode == os_model::RenderingMode::JavaScript
        }
    }
}

/// Visits one stored page at a time; raw bytes never pass through a text decoder.
fn pages(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    kind: PageExport,
    mut visit: impl FnMut(i64, String, Vec<u8>) -> Result<(), ExportError>,
) -> Result<u64, ExportError> {
    if !enabled(cfg, kind) {
        return Ok(0);
    }
    let mut stmt = crate::links_export::sql(file.conn().prepare(
        "SELECT id,address FROM urls WHERE is_internal=1 AND in_tables=1 AND content_class='HTML' AND (?1=0 OR status_code=200) ORDER BY address COLLATE BINARY,id",
    ))?;
    let mut rows = crate::links_export::sql(stmt.query([kind == PageExport::Text]))?;
    let mut count = 0;
    while let Some(row) = crate::links_export::sql(rows.next())? {
        let id = crate::links_export::sql(row.get::<_, i64>(0))?;
        let html = if kind == PageExport::Rendered {
            os_store::queries::stored_rendered_html(file, UrlId(id))?
        } else {
            os_store::queries::stored_html(file, UrlId(id))?
        };
        let Some(html) = html else { continue };
        let bytes = if kind == PageExport::Text {
            let content_type = html
                .charset
                .map(|charset| format!("text/html; charset={charset}"));
            let (decoded, _) = os_parse::charset::decode(&html.bytes, content_type.as_deref(), &[]);
            os_parse::content::area_text(
                &os_parse::dom::Document::parse(&decoded),
                &cfg.content.area,
            )
            .into_bytes()
        } else {
            html.bytes
        };
        visit(id, crate::links_export::sql(row.get(1))?, bytes)?;
        count += 1;
    }
    Ok(count)
}

fn page_file(id: i64, kind: PageExport) -> String {
    match kind {
        PageExport::Original => format!("{id}-original.html"),
        PageExport::Rendered => format!("{id}-rendered.html"),
        PageExport::Text => format!("{id}.txt"),
    }
}

fn index_values(address: String, file: String, kind: PageExport) -> Vec<Value> {
    let mut values = vec![Value::Text(address), Value::Text(file)];
    if kind != PageExport::Text {
        values.push(Value::Text(
            if kind == PageExport::Rendered {
                "Rendered"
            } else {
                "Original"
            }
            .into(),
        ));
    }
    values
}

fn page_index(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    let kind = page_export(job.entry_name).unwrap_or(PageExport::Original);
    pages(job.file, job.cfg, kind, |id, address, _| {
        let values = index_values(address, page_file(id, kind), kind);
        writer.row(&vec![ColumnKind::Text; values.len()], &values)
    })
}

fn page_text(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    pages(job.file, job.cfg, PageExport::Text, |_, address, bytes| {
        writer.row(
            &[ColumnKind::Url, ColumnKind::Text],
            &[
                Value::Text(address),
                Value::Text(String::from_utf8(bytes).expect("area_text is UTF-8")),
            ],
        )
    })
}

/// Writes the requested directory and its CSV index, regardless of table format.
/// A caller may provide cancellation/progress for a batch export.
pub fn export_pages(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    kind: PageExport,
    directory: &Path,
    ctx: Option<&crate::types::ExportCtx<'_>>,
) -> Result<Vec<ExportFileInfo>, ExportError> {
    let entry = entries()
        .iter()
        .find(|entry| page_export(entry.name) == Some(kind))
        .expect("registered page export");
    let dir = directory.join(if kind == PageExport::Text {
        "page_text"
    } else {
        "page_source"
    });
    crate::fs_paths::create_dir_all(&dir).map_err(|source| ExportError::Io {
        path: dir.clone(),
        source,
    })?;
    if let Some(ctx) = ctx {
        ctx.checkpoint()?;
    }
    let open_index = || -> Result<Box<dyn TableWriter>, ExportError> {
        let mut writer = crate::output::open_writer(
            &dir.join("index.csv"),
            Format::Csv,
            entry.name,
            &crate::pipeline::writer_options(cfg),
        )?;
        writer.header(
            &entry
                .header
                .iter()
                .map(|s| (*s).to_owned())
                .collect::<Vec<_>>(),
        )?;
        Ok(writer)
    };
    let mut writer = if ctx.is_some_and(|ctx| ctx.skip_empty) {
        None
    } else {
        Some(open_index()?)
    };
    let mut files = Vec::new();
    pages(file, cfg, kind, |id, address, bytes| {
        if let Some(ctx) = ctx {
            ctx.checkpoint()?;
        }
        let name = page_file(id, kind);
        let path = dir.join(&name);
        let pending = crate::output::PendingFile::new(&path)?;
        std::fs::write(crate::fs_paths::fs_path(&pending.path), &bytes).map_err(|source| {
            ExportError::Io {
                path: path.clone(),
                source,
            }
        })?;
        if let Some(ctx) = ctx {
            ctx.checkpoint()?;
        }
        files.push(pending.publish(ExportFileInfo {
            path,
            rows: 0,
            bytes: bytes.len() as u64,
            warnings: Vec::new(),
        })?);
        let values = index_values(address, name, kind);
        if writer.is_none() {
            writer = Some(open_index()?);
        }
        writer
            .as_mut()
            .expect("opened index")
            .row(&vec![ColumnKind::Text; values.len()], &values)?;
        Ok(())
    })?;
    if let Some(ctx) = ctx {
        ctx.checkpoint()?;
    }
    let Some(writer) = writer else {
        return Ok(files);
    };
    let mut info = writer.finish()?;
    info.warnings
        .extend(crate::reports::prereq::requirement_warnings(
            entry.name,
            entry.requires,
            cfg,
            &[],
        ));
    info.warnings
        .extend(availability_warnings(entry.name, file));
    files.push(info);
    Ok(files)
}

fn headers(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    if !job.cfg.extraction.http_headers {
        return Ok(0);
    }
    let direction = if job.entry_name.to_ascii_lowercase().contains("request") {
        "request"
    } else {
        "response"
    };
    let mut stmt = crate::links_export::sql(job.file.conn().prepare(
        "SELECT u.address,h.name,h.value FROM http_headers h JOIN urls u ON u.id=h.url_id WHERE h.direction=?1 ORDER BY u.address COLLATE BINARY,h.position",
    ))?;
    let mut rows = crate::links_export::sql(stmt.query([direction]))?;
    let mut count = 0;
    while let Some(row) = crate::links_export::sql(rows.next())? {
        let name: String = crate::links_export::sql(row.get(1))?;
        let secret = direction == "request"
            && matches!(
                name.to_ascii_lowercase().as_str(),
                "authorization" | "proxy-authorization" | "cookie"
            );
        let value = if secret && !job.cfg.export.show_credentials {
            "<redacted>".into()
        } else {
            crate::links_export::sql(row.get(2))?
        };
        writer.row(
            &[ColumnKind::Url, ColumnKind::Text, ColumnKind::Text],
            &[
                Value::Text(crate::links_export::sql(row.get(0))?),
                Value::Text(name),
                Value::Text(value),
            ],
        )?;
        count += 1;
    }
    Ok(count)
}

/// Cookie values shared by the bulk and summary exports (UI-043).
pub(crate) struct Cookie {
    pub url_id: i64,
    pub address: String,
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub expires: Option<String>,
    pub max_age: Option<i64>,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: String,
    pub kind: String,
    pub first_party: bool,
    pub at: Option<String>,
}
impl Cookie {
    pub fn session(&self) -> bool {
        self.expires.is_none() && self.max_age.is_none()
    }
    /// Days from the response timestamp, never from the wall clock.
    pub fn persistence(&self) -> Value {
        if self.session() {
            return Value::Text("Session".into());
        }
        let seconds = self.max_age.map(|seconds| seconds as f64).or_else(|| {
            let expires = cookie_expiry(self.expires.as_deref()?)?;
            let at = os_model::Timestamp::from_db(self.at.as_deref()?).ok()?;
            Some((expires - at.0).as_seconds_f64())
        });
        seconds.map_or(Value::Null, |seconds| {
            Value::Float(seconds.max(0.0) / 86400.0)
        })
    }
}

pub(crate) fn visit_cookies(
    job: &TableJob<'_>,
    mut visit: impl FnMut(Cookie) -> Result<(), ExportError>,
) -> Result<u64, ExportError> {
    if !job.cfg.extraction.cookies {
        return Ok(0);
    }
    let mut stmt = crate::links_export::sql(job.file.conn().prepare(
        "SELECT u.id,u.address,c.name,c.value,c.domain,c.path,c.expires,c.max_age,c.secure,c.http_only,c.same_site,c.origin,COALESCE(u.crawl_timestamp,(SELECT value FROM crawl_meta WHERE key='created_at')) FROM cookies c JOIN urls u ON u.id=c.url_id WHERE c.origin<>'javascript' OR ?1 ORDER BY u.address COLLATE BINARY,u.id,c.position",
    ))?;
    let mut rows = crate::links_export::sql(
        stmt.query([job.cfg.rendering.mode == os_model::RenderingMode::JavaScript]),
    )?;
    let mut count = 0;
    while let Some(row) = crate::links_export::sql(rows.next())? {
        let address: String = crate::links_export::sql(row.get(1))?;
        let url = url::Url::parse(&address).map_err(|e| ExportError::Write(e.to_string()))?;
        let host = url.host_str().unwrap_or_default();
        let domain: Option<String> = crate::links_export::sql(row.get(4))?;
        let domain = domain
            .filter(|d| !d.is_empty())
            .unwrap_or_else(|| host.into())
            .trim_end_matches('.')
            .to_ascii_lowercase();
        let path: Option<String> = crate::links_export::sql(row.get(5))?;
        let path = path.filter(|p| p.starts_with('/')).unwrap_or_else(|| {
            let parent = url.path().rsplit_once('/').map_or("", |(parent, _)| parent);
            if parent.is_empty() {
                "/".into()
            } else {
                parent.into()
            }
        });
        let site = |host: &str| {
            os_url::registrable_domain(host).unwrap_or_else(|| host.to_ascii_lowercase())
        };
        let first_party = site(host) == site(domain.trim_start_matches('.'));
        let origin: String = crate::links_export::sql(row.get(11))?;
        let same_site: Option<String> = crate::links_export::sql(row.get(10))?;
        let same_site = match same_site.as_deref().map(str::to_ascii_lowercase).as_deref() {
            Some("strict") => "Strict",
            Some("lax") => "Lax",
            Some("none") => "None",
            _ => "",
        }
        .into();
        visit(Cookie {
            url_id: crate::links_export::sql(row.get(0))?,
            address,
            name: crate::links_export::sql(row.get(2))?,
            value: crate::links_export::sql(row.get(3))?,
            domain,
            path,
            expires: crate::links_export::sql(row.get::<_, Option<String>>(6))?
                .and_then(|s| cookie_expiry(&s))
                .and_then(|at| {
                    at.to_offset(time::UtcOffset::UTC)
                        .format(&time::format_description::well_known::Rfc3339)
                        .ok()
                }),
            max_age: crate::links_export::sql(row.get(7))?,
            secure: crate::links_export::sql(row.get(8))?,
            http_only: origin != "javascript" && crate::links_export::sql(row.get(9))?,
            same_site,
            kind: if origin.eq_ignore_ascii_case("javascript") {
                "JavaScript"
            } else {
                "HTTP"
            }
            .into(),
            first_party,
            at: crate::links_export::sql(row.get(12))?,
        })?;
        count += 1;
    }
    Ok(count)
}

fn cookies(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    visit_cookies(job, |cookie| {
        writer.row(
            &[ColumnKind::Text; 13],
            &[
                Value::Text(cookie.address.clone()),
                Value::Text(cookie.name.clone()),
                Value::Text(cookie.value.clone()),
                Value::Text(cookie.domain.clone()),
                Value::Text(cookie.path.clone()),
                cookie.expires.clone().map_or(Value::Null, Value::Text),
                cookie.max_age.map_or(Value::Null, Value::Int),
                boolean(cookie.secure),
                boolean(cookie.http_only),
                Value::Text(cookie.same_site.clone()),
                Value::Text(cookie.kind.clone()),
                boolean(cookie.session()),
                boolean(cookie.first_party),
            ],
        )
    })
}

/// Cookie exports have capitalized truth values as specified by UI-043.
pub(crate) fn boolean(value: bool) -> Value {
    Value::Text(if value { "True" } else { "False" }.into())
}
fn cookie_expiry(value: &str) -> Option<time::OffsetDateTime> {
    time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
        .ok()
        .or_else(|| {
            time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc2822).ok()
        })
}
