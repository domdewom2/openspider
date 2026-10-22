use std::{collections::BTreeSet, path::PathBuf, sync::Arc};

use os_model::{ApiKind, ApiValueWrite, Clock, CrawlConfig};
use os_store::CrawlFile;

use super::db;
use crate::{ApiCtx, ApiError, ApiSource, UrlRef};

pub fn enabled(cfg: &CrawlConfig) -> Vec<ApiKind> {
    [
        (ApiKind::Ga4, cfg.api.ga4.enabled),
        (ApiKind::Gsc, cfg.api.gsc.enabled),
        (ApiKind::Psi, cfg.api.psi.enabled),
        (ApiKind::Majestic, cfg.api.majestic.enabled),
        (ApiKind::Ahrefs, cfg.api.ahrefs.enabled),
        (ApiKind::Moz, cfg.api.moz.enabled),
    ]
    .into_iter()
    .filter_map(|(kind, on)| on.then_some(kind))
    .collect()
}

/// Scheduling selects sources without replacing account, property or stream settings from the profile.
pub fn select(cfg: &mut CrawlConfig, sources: &[String]) -> Result<(), ApiError> {
    for source in sources {
        match source.as_str() {
            "ga4" => cfg.api.ga4.enabled = true,
            "gsc" => cfg.api.gsc.enabled = true,
            "psi" => cfg.api.psi.enabled = true,
            "majestic" => cfg.api.majestic.enabled = true,
            "ahrefs" => cfg.api.ahrefs.enabled = true,
            "moz" => cfg.api.moz.enabled = true,
            _ => {
                return Err(ApiError::NotConnected(format!(
                    "Unbekannte Integration: {source}"
                )))
            }
        }
    }
    Ok(())
}

fn account(cfg: &CrawlConfig, kind: ApiKind) -> Option<&str> {
    match kind {
        ApiKind::Ga4 => cfg.api.ga4.account.as_deref(),
        ApiKind::Gsc => cfg.api.gsc.account.as_deref(),
        ApiKind::Psi => cfg.api.psi.account.as_deref(),
        ApiKind::Majestic => cfg.api.majestic.account.as_deref(),
        ApiKind::Ahrefs => cfg.api.ahrefs.account.as_deref(),
        ApiKind::Moz => cfg.api.moz.account.as_deref(),
        _ => None,
    }
}

pub(super) fn connected(ctx: &ApiCtx, kind: ApiKind) -> bool {
    crate::credentials::read_account(ctx.secrets.as_ref(), kind, account(&ctx.cfg, kind)).is_ok_and(
        |c| {
            c.api_key
                .as_ref()
                .or(c.access_token.as_ref())
                .is_some_and(|s| !s.is_empty())
        },
    )
}

/// No network request: missing selected accounts fail before a crawl can be created when required.
pub fn preflight(ctx: &ApiCtx, require: bool) -> Result<Vec<String>, ApiError> {
    let mut warnings = Vec::new();
    for kind in enabled(&ctx.cfg) {
        if !connected(ctx, kind) {
            let message = format!(
                "{kind}: Konto {} nicht verbunden",
                account(&ctx.cfg, kind).unwrap_or("Standard")
            );
            if require {
                return Err(ApiError::NotConnected(message));
            }
            warnings.push(message);
        }
    }
    Ok(warnings)
}

/// Sources live for the whole run so bulk reports are fetched only once.
pub struct ApiRun {
    ctx: ApiCtx,
    clock: Arc<dyn Clock>,
    ga4: Arc<crate::ga4::Ga4Source>,
    gsc: Arc<crate::gsc::GscSource>,
    sources: Vec<Arc<dyn ApiSource>>,
}
impl ApiRun {
    pub fn new(ctx: ApiCtx, clock: Arc<dyn Clock>, ledger: PathBuf) -> Self {
        let ga4 = Arc::new(crate::ga4::Ga4Source::new(clock.clone()));
        let gsc = Arc::new(crate::gsc::GscSource::new(
            clock.clone(),
            Arc::new(crate::quota::QuotaLedger::new(ledger)),
        ));
        let sources = enabled(&ctx.cfg)
            .into_iter()
            .map(|kind| -> Arc<dyn ApiSource> {
                match kind {
                    ApiKind::Ga4 => ga4.clone(),
                    ApiKind::Gsc => gsc.clone(),
                    ApiKind::Psi => {
                        Arc::new(crate::psi::PsiSource::new().with_clock(clock.clone()))
                    }
                    kind => Arc::new(crate::linkmetrics::LinkMetrics::with_clock(
                        kind,
                        clock.clone(),
                    )),
                }
            })
            .collect();
        Self {
            ctx,
            clock,
            ga4,
            gsc,
            sources,
        }
    }

    pub fn validate(&mut self, require: bool) -> Result<Vec<String>, ApiError> {
        let warnings = preflight(&self.ctx, require)?;
        if require && self.sources.is_empty() {
            return Err(ApiError::NotConnected(
                "Mindestens eine Integration muss verbunden sein".into(),
            ));
        }
        self.sources.retain(|s| connected(&self.ctx, s.kind()));
        Ok(warnings)
    }

    async fn validate_ga4_selection(&self) -> Result<(), ApiError> {
        let Some(account) = &self.ctx.cfg.api.ga4.analytics_account else {
            return Ok(());
        };
        let name = if account.starts_with("accounts/") {
            account.clone()
        } else {
            format!("accounts/{account}")
        };
        let (property, _) = crate::ga4::settings::parse_property(
            self.ctx.cfg.api.ga4.property.as_deref().unwrap_or_default(),
        )
        .map_err(db)?;
        let tree = self.ga4.accounts(&self.ctx).await?;
        if tree.properties(&name).is_ok_and(|properties| {
            properties
                .iter()
                .any(|p| p.name == format!("properties/{property}"))
        }) {
            return Ok(());
        }
        Err(ApiError::NotConnected(
            "GA4-Property gehört nicht zum gewählten Analytics-Konto".into(),
        ))
    }

    /// Records every same-host API URL, including already linked URLs, as report provenance.
    /// Admission into the frontier remains the engine's job (rules, robots, limits).
    pub async fn discover(&self, file: &CrawlFile) -> Result<Vec<String>, ApiError> {
        let urls = urls(file)?;
        let mut hosts: BTreeSet<_> = urls.iter().filter_map(|u| host(&u.url)).collect();
        if let Some(start) = file
            .get_meta("start_url")
            .map_err(db)?
            .and_then(|u| host(&u))
        {
            hosts.insert(start);
        }
        let mut candidates = BTreeSet::new();
        if self.ctx.cfg.api.ga4.enabled && self.sources.iter().any(|s| s.kind() == ApiKind::Ga4) {
            self.validate_ga4_selection().await?;
            let report = self.ga4.report(&self.ctx).await?;
            let addresses: BTreeSet<_> = report
                .rows
                .values()
                .flat_map(|rows| rows.keys().cloned())
                .collect();
            let capture = file.conn().unchecked_transaction().map_err(db)?;
            for address in addresses {
                if host(&address).is_some_and(|h| hosts.contains(&h)) {
                    record_source(file, &address, "ga4", self.clock.now())?;
                    if self.ctx.cfg.api.ga4.crawl_new_urls {
                        candidates.insert(address);
                    }
                }
            }
            capture.execute("INSERT INTO crawl_meta(key,value) VALUES('api_raw_hosts.ga4',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [serde_json::to_string(&hosts).map_err(db)?]).map_err(db)?;
            capture.commit().map_err(db)?;
        }
        if self.ctx.cfg.api.gsc.enabled && self.sources.iter().any(|s| s.kind() == ApiKind::Gsc) {
            let options =
                crate::gsc::options::GscOptions::from_config(&self.ctx.cfg.api.gsc).map_err(db)?;
            let token = crate::credentials::read_account(
                self.ctx.secrets.as_ref(),
                ApiKind::Gsc,
                self.ctx.cfg.api.gsc.account.as_deref(),
            )?
            .access_token;
            let http =
                crate::gsc::client::GscHttp::new(&self.ctx, &options, token, self.clock.clone())?;
            let report = self.gsc.report(&http, &options).await;
            if let Some(e) = &report.error {
                return Err(e.to_api_error());
            }
            let capture = file.conn().unchecked_transaction().map_err(db)?;
            for row in &report.rows {
                if host(&row.page).is_some_and(|h| hosts.contains(&h)) {
                    record_source(file, &row.page, "gsc", self.clock.now())?;
                    if self.ctx.cfg.api.gsc.crawl_new_urls {
                        candidates.insert(row.page.clone());
                    }
                }
            }
            capture.execute("INSERT INTO crawl_meta(key,value) VALUES('api_raw_hosts.gsc',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [serde_json::to_string(&hosts).map_err(db)?]).map_err(db)?;
            capture.commit().map_err(db)?;
        }
        Ok(candidates.into_iter().collect())
    }

    /// Completion barrier: all results and per-URL failures are durable before returning.
    pub async fn collect(&self, file: &CrawlFile) -> Result<(), ApiError> {
        let urls = urls(file)?;
        let apis = file.get_meta("source_mode").map_err(db)?.as_deref() == Some("apis");
        if !apis {
            let mut stmt = file.conn().prepare("SELECT id,url,content_class='HTML',indexability='Indexable' FROM urls WHERE is_internal=1 AND in_tables=1").map_err(db)?;
            let targets = stmt
                .query_map([], |r| {
                    Ok(crate::gsc::inspection::Target {
                        url_id: Some(os_model::UrlId(r.get(0)?)),
                        url: r.get(1)?,
                        is_html: r.get::<_, Option<bool>>(2)?.unwrap_or(false),
                        indexable: r.get::<_, Option<bool>>(3)?.unwrap_or(false),
                    })
                })
                .map_err(db)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(db)?;
            self.gsc.set_targets(targets);
        }
        for source in &self.sources {
            if source.kind() == ApiKind::Ga4 {
                self.validate_ga4_selection().await?;
            }
            let kind = source.kind().to_string();
            let mut failed = 0;
            let mut done = 0;
            let eligible = if !apis && source.kind() == ApiKind::Psi {
                Some(crate::psi::eligible_urls(file)?)
            } else {
                None
            };
            let source_urls = eligible.as_deref().unwrap_or(&urls);
            let link_metrics = crate::linkmetrics::is_linkmetrics(source.kind());
            let source = if link_metrics {
                crate::linkmetrics::batch_source(source.clone(), source_urls.to_vec())
            } else {
                source.clone()
            };
            for chunk in source_urls.chunks(if link_metrics { 1 } else { 100 }) {
                let outcomes = if matches!(source.kind(), ApiKind::Ga4 | ApiKind::Gsc) {
                    let values = source.fetch(chunk, &self.ctx).await;
                    chunk
                        .iter()
                        .map(|url| {
                            (
                                url.clone(),
                                values
                                    .as_ref()
                                    .map(|values| {
                                        values
                                            .iter()
                                            .filter(|v| v.url_id == url.url_id)
                                            .cloned()
                                            .collect()
                                    })
                                    .map_err(Clone::clone),
                            )
                        })
                        .collect()
                } else {
                    source.fetch_outcomes(chunk, &self.ctx).await
                };
                for (url, outcome) in outcomes {
                    done += 1;
                    match outcome {
                        Ok(values) => persist(file, &kind, &url, &values)?,
                        Err(error) => {
                            failed += 1;
                            os_store::queries::api::record_error(
                                file.conn(),
                                &kind,
                                &url.url,
                                "api_request",
                                &error.public_message(),
                                self.clock.now(),
                            )
                            .map_err(db)?;
                        }
                    }
                    os_store::queries::api::set_status(
                        file.conn(),
                        &os_store::queries::api::Status {
                            source: kind.clone(),
                            state: if done == source_urls.len() {
                                "idle"
                            } else {
                                "fetching"
                            }
                            .into(),
                            total: source_urls.len() as u64,
                            done: done as u64,
                            failed,
                            detail: (failed > 0).then(|| "API-Abrufe mit Fehlern".into()),
                        },
                    )
                    .map_err(db)?;
                }
            }
        }
        Ok(())
    }
}

pub fn urls(file: &CrawlFile) -> Result<Vec<UrlRef>, ApiError> {
    file.conn()
        .prepare("SELECT id,url FROM urls WHERE is_internal=1 AND in_tables=1 ORDER BY id")
        .map_err(db)?
        .query_map([], |r| {
            Ok(UrlRef {
                url_id: Some(os_model::UrlId(r.get(0)?)),
                url: r.get(1)?,
            })
        })
        .map_err(db)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db)
}
fn host(address: &str) -> Option<String> {
    let url = url::Url::parse(address).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    url.host_str().map(str::to_lowercase)
}
fn record_source(
    file: &CrawlFile,
    url: &str,
    source: &str,
    at: os_model::Timestamp,
) -> Result<(), ApiError> {
    file.conn()
        .execute(
            "INSERT OR IGNORE INTO api_urls(url,source,first_seen) VALUES(?1,?2,?3)",
            (url, source, at.to_db()),
        )
        .map_err(db)?;
    Ok(())
}
fn persist(
    file: &CrawlFile,
    source: &str,
    url: &UrlRef,
    values: &[ApiValueWrite],
) -> Result<(), ApiError> {
    let tx = file.conn().unchecked_transaction().map_err(db)?;
    tx.execute(
        "DELETE FROM api_values WHERE url_id=?1 AND source=?2",
        (url.url_id.map(|id| id.0), source),
    )
    .map_err(db)?;
    for value in values {
        if let Some(id) = value.url_id {
            tx.execute("INSERT OR REPLACE INTO api_values(url_id,source,metric,level,v_real,v_text,fetched_at) VALUES(?1,?2,?3,?4,?5,?6,?7)", (id.0,&value.source,&value.metric,&value.level,value.v_real,&value.v_text,value.fetched_at.to_db())).map_err(db)?;
        }
    }
    os_store::queries::api::clear_error(&tx, source, &url.url).map_err(db)?;
    let seq = os_store::queries::change_seq(&tx).map_err(db)? + 1;
    tx.execute(
        "UPDATE urls SET change_seq=?1 WHERE id=?2",
        (seq, url.url_id.map(|id| id.0)),
    )
    .map_err(db)?;
    tx.execute(
        "UPDATE crawl_meta SET value=?1 WHERE key='change_seq'",
        [seq.to_string()],
    )
    .map_err(db)?;
    tx.commit().map_err(db)
}
