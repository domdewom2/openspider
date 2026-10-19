//! Google Search Console (04 G und H, INTEG-110 … 128 ohne 126): Properties, Search Analytics und URL Inspection.
//!
//! Aufbau:
//! - [`options`]: Einstellungen und ihre Prüfung (Zeitraum, Suchtyp, Dimensionsfilter mit RE2-Prüfung, Lese-Timeout).
//! - [`dates`]: Zeitraum und Datenstand (heute − 2 in US-Pazifikzeit, höchstens 16 Monate).
//! - [`sites`]: Property-Liste, Beschriftung, Domain-Abgleich und spezifischste Property je URL.
//! - [`client`]: HTTP mit Lese-Timeout je Anfrage, Wiederholungen und Drosselung je Property.
//! - [`analytics`]: `searchanalytics.query` mit Paginierung und Zuordnung zu gecrawlten URLs.
//! - [`inspection`]: `urlInspection/index:inspect` mit Tageskontingent, Fehlern und den 17 Spalten.
//!
//! Werte landen als `api_values` mit Quelle [`SOURCE`] und den Metriken aus [`metric`]; der Tab „Search Console“
//! (`os-analyze`, `tabs::search_console`) liest sie von dort. Zugangsdaten nur über `os_fetch::secrets` (C13).

pub mod analytics;
pub mod client;
pub mod dates;
pub mod inspection;
pub mod options;
pub mod sites;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use os_model::{ApiKind, ApiValueWrite, Clock};

use self::analytics::Report;
use self::client::{Endpoint, Failure, GscHttp};
use self::inspection::{Inspector, Target};
use self::options::GscOptions;
use self::sites::Property;
use crate::quota::QuotaLedger;
use crate::{ApiCtx, ApiError, ApiSource, ApiState, ApiStatus, BoxFuture, UrlRef};

/// Quelle in `api_values`.
pub const SOURCE: &str = "gsc";

/// Metriken in `api_values` (Quelle `gsc`).
pub mod metric {
    pub const CLICKS: &str = "clicks";
    pub const IMPRESSIONS: &str = "impressions";
    pub const CTR: &str = "ctr";
    pub const POSITION: &str = "position";
    pub const SUMMARY: &str = "summary";
    pub const COVERAGE: &str = "coverage";
    pub const LAST_CRAWL: &str = "last_crawl";
    pub const CRAWLED_AS: &str = "crawled_as";
    pub const CRAWL_ALLOWED: &str = "crawl_allowed";
    pub const PAGE_FETCH: &str = "page_fetch";
    pub const INDEXING_ALLOWED: &str = "indexing_allowed";
    pub const USER_CANONICAL: &str = "user_canonical";
    pub const GOOGLE_CANONICAL: &str = "google_canonical";
    pub const MOBILE_USABILITY: &str = "mobile_usability";
    pub const MOBILE_USABILITY_ISSUES: &str = "mobile_usability_issues";
    pub const AMP_RESULTS: &str = "amp_results";
    pub const AMP_ISSUES: &str = "amp_issues";
    pub const RICH_RESULTS: &str = "rich_results";
    pub const RICH_RESULTS_TYPES: &str = "rich_results_types";
    pub const RICH_RESULTS_TYPES_ERRORS: &str = "rich_results_types_errors";
    pub const RICH_RESULTS_WARNINGS: &str = "rich_results_warnings";
    pub const INSPECTION_PROPERTY: &str = "inspection_property";
    /// 1, wenn User-Declared und Google-Selected Canonical nach Normalisierung verschieden sind, sonst 0 (nur wenn
    /// beide gesetzt sind; INTEG-125).
    pub const CANONICAL_NOT_SELECTED: &str = "canonical_not_selected";
}

/// Properties des Kontos (`GET /webmasters/v3/sites`, INTEG-110).
pub async fn list_properties(http: &GscHttp) -> Result<Vec<Property>, Failure> {
    let json = http
        .get(Endpoint::Analytics, "/webmasters/v3/sites")
        .await?;
    Ok(sites::parse_sites(&json))
}

/// Zugriffstoken des gewählten Kontos aus der Geheimnis-Sicht (C13).
fn access_token(ctx: &ApiCtx, options: &GscOptions) -> Result<Option<String>, ApiError> {
    let credential = crate::credentials::read_account(
        ctx.secrets.as_ref(),
        ApiKind::Gsc,
        options.account.as_deref(),
    )?;
    Ok(credential.access_token.or(credential.api_key))
}

/// Die Search-Console-Quelle für den API-Rahmen (`ApiSource`).
///
/// Search Analytics ist ein Gesamtreport: Er wird beim ersten Abruf einmal geladen und für alle weiteren Abrufe
/// derselben Quelle wiederverwendet ([`GscSource::reset`] verwirft ihn, etwa nach einer Konfigurationsänderung).
/// Mit eingeschalteter URL Inspection prüft jeder Abruf zusätzlich die übergebenen URLs; die Fakten aus dem Crawl
/// (HTML, Indexierbarkeit) setzt der Aufrufer über [`GscSource::set_targets`], sonst gelten die URLs als
/// indexierbare HTML-Seiten.
pub struct GscSource {
    clock: Arc<dyn Clock>,
    ledger: Arc<QuotaLedger>,
    report: tokio::sync::Mutex<Option<Arc<Report>>>,
    properties: tokio::sync::Mutex<Option<Arc<Vec<Property>>>>,
    targets: Mutex<HashMap<String, Target>>,
    status: Mutex<ApiStatus>,
}

impl GscSource {
    /// Quelle mit Uhr (Datenstand, Kontingenttag) und Kontingentzähler.
    pub fn new(clock: Arc<dyn Clock>, ledger: Arc<QuotaLedger>) -> GscSource {
        GscSource {
            clock,
            ledger,
            report: tokio::sync::Mutex::default(),
            properties: tokio::sync::Mutex::default(),
            targets: Mutex::default(),
            status: Mutex::new(ApiStatus {
                kind: ApiKind::Gsc,
                state: ApiState::Idle,
                urls_total: 0,
                urls_done: 0,
                urls_failed: 0,
                detail: None,
            }),
        }
    }

    /// Setzt die Fakten der Crawl-URLs für die URL Inspection.
    pub fn set_targets(&self, targets: Vec<Target>) {
        *self.targets.lock().unwrap_or_else(|p| p.into_inner()) =
            targets.into_iter().map(|t| (t.url.clone(), t)).collect();
    }

    /// Verwirft den geladenen Report und die Property-Liste.
    pub async fn reset(&self) {
        *self.report.lock().await = None;
        *self.properties.lock().await = None;
    }

    /// Der Search-Analytics-Report (einmal geladen).
    pub async fn report(&self, http: &GscHttp, options: &GscOptions) -> Arc<Report> {
        let mut cached = self.report.lock().await;
        if let Some(report) = cached.as_ref() {
            return report.clone();
        }
        let report = Arc::new(analytics::fetch_report(http, options, self.clock.now()).await);
        let mut status = self.status.lock().unwrap_or_else(|p| p.into_inner());
        status.detail = report.status_message();
        if report.error.is_some() {
            status.state = ApiState::Failed;
        }
        drop(status);
        *cached = Some(report.clone());
        report
    }

    async fn properties(&self, http: &GscHttp) -> Result<Arc<Vec<Property>>, ApiError> {
        let mut cached = self.properties.lock().await;
        if let Some(p) = cached.as_ref() {
            return Ok(p.clone());
        }
        let list = Arc::new(list_properties(http).await.map_err(|e| e.to_api_error())?);
        *cached = Some(list.clone());
        Ok(list)
    }
}

impl ApiSource for GscSource {
    fn kind(&self) -> ApiKind {
        ApiKind::Gsc
    }

    fn fetch<'a>(
        &'a self,
        urls: &'a [UrlRef],
        ctx: &'a ApiCtx,
    ) -> BoxFuture<'a, Result<Vec<ApiValueWrite>, ApiError>> {
        Box::pin(async move {
            let options = GscOptions::from_config(&ctx.cfg.api.gsc)
                .map_err(|e| ApiError::Transport(e.to_string()))?;
            let http = GscHttp::new(
                ctx,
                &options,
                access_token(ctx, &options)?,
                self.clock.clone(),
            )?;
            let report = self.report(&http, &options).await;
            if report.rows.is_empty() {
                if let Some(error) = &report.error {
                    return Err(error.to_api_error());
                }
            }
            let now = self.clock.now();
            // Je Abruf nur die übergebenen URLs; API-Zeilen ohne Crawl-URL liefert `analytics::values` mit `true`.
            let mut values = analytics::values(&report.rows, urls, &options.url_match, now, false);
            if options.url_inspection {
                let properties = if options.multiple_properties {
                    self.properties(&http).await?
                } else {
                    Arc::new(Vec::new())
                };
                let targets: Vec<Target> = {
                    let known = self.targets.lock().unwrap_or_else(|p| p.into_inner());
                    urls.iter()
                        .map(|u| {
                            known.get(&u.url).cloned().unwrap_or(Target {
                                url_id: u.url_id,
                                url: u.url.clone(),
                                is_html: true,
                                indexable: true,
                            })
                        })
                        .collect()
                };
                let inspector = Inspector::new(
                    &http,
                    &self.ledger,
                    self.clock.clone(),
                    &options,
                    &properties,
                );
                let outcomes = inspector.run(&targets).await?;
                for (target, outcome) in targets.iter().zip(&outcomes) {
                    values.extend(outcome.values(target, now));
                }
            }
            let mut status = self.status.lock().unwrap_or_else(|p| p.into_inner());
            status.urls_total += urls.len() as u64;
            status.urls_done += urls.len() as u64;
            Ok(values)
        })
    }

    fn test_connection<'a>(
        &'a self,
        ctx: &'a ApiCtx,
        credential: &'a crate::credentials::Credential,
    ) -> BoxFuture<'a, Result<crate::credentials::Credential, ApiError>> {
        Box::pin(async move {
            // Genau ein Prüfabruf: die Property-Liste (INTEG-001), ohne Wiederholung.
            let mut options = GscOptions::from_config(&ctx.cfg.api.gsc).unwrap_or_default();
            options.limits.retries = 0;
            let token = credential
                .access_token
                .clone()
                .or(credential.api_key.clone());
            let http = GscHttp::new(ctx, &options, token, self.clock.clone())?;
            let properties = list_properties(&http).await.map_err(|e| e.to_api_error())?;
            // Kennung und Beschriftung aus der Property-Liste (OAuth ohne Identitätsscope, siehe `oauth`).
            let (account_id, label) = sites::account_identity(&properties);
            let mut credential = credential.clone();
            credential.account_id = account_id;
            credential.label = label;
            Ok(credential)
        })
    }

    fn status(&self) -> ApiStatus {
        self.status
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
}
