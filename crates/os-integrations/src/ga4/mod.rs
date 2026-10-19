//! Google Analytics 4 (04 F, INTEG-090 … INTEG-101): Quelle [`Ga4Source`] für den API-Rahmen (M2-I01).
//!
//! - **Verbindung und Auswahl** (INTEG-090): [`Ga4Source::accounts`] liefert Konten mit Properties
//!   ([`AccountTree`]), [`Ga4Source::streams`] die Web-Datenstreams einer Property; die Auswahl liegt als
//!   `properties/<id>[/dataStreams/<id>]` im Profil ([`settings`]). Ein gewählter Stream schränkt jede Anfrage über
//!   einen `hostName`-Filter ein, Zeilen anderer Hosts werden nicht zugeordnet.
//! - **Report** (INTEG-091 … 096): [`report::load`] fragt je Seitendimension höchstens zehn Metriken je Anfrage ab,
//!   paginiert bis `rowCount` bzw. zum Ergebnislimit und führt die Teilantworten über die URL zusammen.
//! - **Abruf je URL**: Der Rahmen ruft [`ApiSource::fetch`] je URL auf; die Quelle lädt den Report einmal je
//!   Einstellungsstand (Cache) und ordnet über den URL-Abgleich (INTEG-014) zu. Ohne GA4-Zeile entstehen keine Werte
//!   (Filter `No GA Data`); ein geliefertes `0` wird gespeichert. [`report::Report::assign`] ordnet alle URLs auf
//!   einmal zu und meldet Orphan-Kandidaten (für M2-I06).
//! - **Kontingente** (INTEG-099): `returnPropertyQuota` in jeder Anfrage, Zurückhaltung bis zur nächsten vollen Stunde
//!   unter 5 % Stundenkontingent, höchstens zehn gleichzeitige Anfragen je Property, `RESOURCE_EXHAUSTED` pausiert.
//! - **Kompatibilität** (INTEG-101): [`Ga4Source::check_compatibility`] und [`compat::CompatibilityChecker`]; kein
//!   `runReport` enthält eine zuvor als inkompatibel gemeldete Kombination.
//!
//! Werte stehen in `api_values` unter der Quelle `ga4`, Metrik = API-Name (bei Landingpage mit Zusatz `@landing`).

pub mod catalog;
pub mod client;
pub mod compat;
pub mod report;
pub mod settings;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use os_model::{ApiKind, ApiValueWrite, Clock};
use tokio::sync::Semaphore;

use crate::core::ProviderQuota;
use crate::credentials::{read_account, Credential};
use crate::{ApiCtx, ApiError, ApiSource, ApiState, ApiStatus, BoxFuture, UrlRef};
use catalog::MetricTree;
use client::{Ga4Account, Ga4Client, Ga4Property, Ga4Stream, Retry, MAX_CONCURRENT_PER_PROPERTY};
use compat::{Compatibility, CompatibilityChecker};
use report::{QuotaTracker, Report};
use settings::{resolve_dates, Ga4Settings, SettingsError};

/// Hinweis bei einem Konto ohne GA4-Property (INTEG-090).
pub const NO_PROPERTIES: &str = "Keine GA4-Properties gefunden";
/// Anzeige der Option „Alle Datenstreams“.
pub const ALL_STREAMS: &str = "Alle Datenstreams";
/// Wie lange ein fehlgeschlagener Report-Abruf für dieselben Einstellungen gilt (keine Wiederholung je URL).
const FAILURE_TTL: Duration = Duration::from_secs(60);

/// Status `Verbunden (<Konto> / <Property> / <Stream>)` (INTEG-090).
pub fn status_label(account: &str, property: &str, stream: Option<&str>) -> String {
    format!(
        "Verbunden ({account} / {property} / {})",
        stream.unwrap_or(ALL_STREAMS)
    )
}

/// Konten mit ihren Properties (abhängige Dropdowns, INTEG-090).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AccountTree {
    /// Konten in der Reihenfolge der Admin API.
    pub accounts: Vec<Ga4Account>,
}

impl AccountTree {
    /// Properties des gewählten Kontos; ohne Property der Hinweis [`NO_PROPERTIES`].
    pub fn properties(&self, account: &str) -> Result<&[Ga4Property], &'static str> {
        match self.accounts.iter().find(|a| a.name == account) {
            Some(a) if !a.properties.is_empty() => Ok(&a.properties),
            _ => Err(NO_PROPERTIES),
        }
    }

    /// Suchfeld bei vielen Properties: Treffer in Name oder Kennung (ohne Groß-/Kleinschreibung).
    pub fn search(&self, account: &str, query: &str) -> Vec<&Ga4Property> {
        let q = query.to_lowercase();
        self.properties(account)
            .unwrap_or_default()
            .iter()
            .filter(|p| p.display_name.to_lowercase().contains(&q) || p.name.contains(&q))
            .collect()
    }

    /// Konto, zu dem eine Property gehört (Vorauswahl bei erneuter Verbindung).
    pub fn account_of(&self, property: &str) -> Option<&Ga4Account> {
        let name = if property.starts_with("properties/") {
            property.split('/').take(2).collect::<Vec<_>>().join("/")
        } else {
            format!("properties/{property}")
        };
        self.accounts
            .iter()
            .find(|a| a.properties.iter().any(|p| p.name == name))
    }
}

struct Cached {
    fingerprint: String,
    result: Result<Arc<Report>, ApiError>,
    at: Duration,
}

struct StatusState {
    state: ApiState,
    done: u64,
    failed: u64,
    notices: Vec<String>,
}

/// GA4-Quelle.
pub struct Ga4Source {
    clock: Arc<dyn Clock>,
    quota: QuotaTracker,
    permits: Mutex<HashMap<String, Arc<Semaphore>>>,
    cache: tokio::sync::Mutex<Option<Cached>>,
    compat: Mutex<Option<Compatibility>>,
    status: Mutex<StatusState>,
}

fn settings_error(e: SettingsError) -> ApiError {
    ApiError::Provider {
        status: 400,
        message: e.to_string(),
    }
}

fn token(ctx: &ApiCtx) -> Result<String, ApiError> {
    read_account(
        ctx.secrets.as_ref(),
        ApiKind::Ga4,
        ctx.cfg.api.ga4.account.as_deref(),
    )?
    .access_token
    .ok_or_else(|| ApiError::NotConnected("ga4".into()))
}

impl Ga4Source {
    /// Quelle mit der Uhr der App (Zeitstempel, Zeitraum, Kontingent-Stundenwechsel).
    pub fn new(clock: Arc<dyn Clock>) -> Ga4Source {
        Ga4Source {
            clock,
            quota: QuotaTracker::default(),
            permits: Mutex::default(),
            cache: tokio::sync::Mutex::default(),
            compat: Mutex::default(),
            status: Mutex::new(StatusState {
                state: ApiState::Idle,
                done: 0,
                failed: 0,
                notices: Vec::new(),
            }),
        }
    }

    fn permits(&self, property: &str) -> Arc<Semaphore> {
        self.permits
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .entry(property.to_string())
            .or_insert_with(|| Arc::new(Semaphore::new(MAX_CONCURRENT_PER_PROPERTY)))
            .clone()
    }

    fn client(&self, ctx: &ApiCtx, property: &str) -> Result<Ga4Client, ApiError> {
        Ga4Client::new(
            ctx,
            &token(ctx)?,
            self.permits(property),
            self.clock.clone(),
        )
    }

    fn set_status(&self, state: ApiState, notices: Vec<String>) {
        let mut s = self.status.lock().unwrap_or_else(|p| p.into_inner());
        s.state = state;
        s.notices = notices;
    }

    /// Konten und Properties des verbundenen Google-Kontos (INTEG-090).
    pub async fn accounts(&self, ctx: &ApiCtx) -> Result<AccountTree, ApiError> {
        let client = self.client(ctx, "")?;
        Ok(AccountTree {
            accounts: client.account_summaries(Retry::Default).await?,
        })
    }

    /// Web-Datenstreams einer Property (`properties/<id>` oder `<id>`).
    pub async fn streams(&self, ctx: &ApiCtx, property: &str) -> Result<Vec<Ga4Stream>, ApiError> {
        let (id, _) = settings::parse_property(property).map_err(settings_error)?;
        self.client(ctx, &id)?.data_streams(&id).await
    }

    /// Metrik-Baum einer Property aus `getMetadata`; ohne Antwort der statische Katalog (INTEG-092).
    pub async fn metric_tree(&self, ctx: &ApiCtx, property: &str) -> MetricTree {
        let Ok((id, _)) = settings::parse_property(property) else {
            return MetricTree::from_catalog();
        };
        match self.client(ctx, &id) {
            Ok(client) => match client.metadata(&id).await {
                Ok(meta) => MetricTree::from_metadata(&meta),
                Err(_) => MetricTree::from_catalog(),
            },
            Err(_) => MetricTree::from_catalog(),
        }
    }

    /// Prüft die Auswahl mit genau einer Anfrage an `checkCompatibility` (INTEG-101). Das Ergebnis gilt für spätere
    /// Report-Abrufe mit denselben Einstellungen.
    pub async fn check_compatibility(&self, ctx: &ApiCtx, settings: &Ga4Settings) -> Compatibility {
        let result = match self.client(ctx, &settings.property) {
            Ok(client) => match client
                .check_compatibility(&settings.property, &compat::request_body(settings))
                .await
            {
                Ok(response) => Compatibility::from_response(settings, &response),
                Err(_) => Compatibility::unreachable(settings),
            },
            Err(_) => Compatibility::unreachable(settings),
        };
        *self.compat.lock().unwrap_or_else(|p| p.into_inner()) = Some(result.clone());
        result
    }

    /// Entprellte Prüfung für den Dialog (INTEG-101).
    pub fn checker(self: &Arc<Self>, ctx: ApiCtx) -> CompatibilityChecker {
        let source = self.clone();
        let ctx = Arc::new(ctx);
        CompatibilityChecker::spawn(Arc::new(move |settings| {
            let source = source.clone();
            let ctx = ctx.clone();
            Box::pin(async move { source.check_compatibility(&ctx, &settings).await })
        }))
    }

    /// Hinweise des letzten Reports (Filter, `(other)`, Schwellenwerte, entfernte Metriken, Fehler).
    pub fn notices(&self) -> Vec<String> {
        self.status
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .notices
            .clone()
    }

    /// Verwirft den zwischengespeicherten Report (z. B. nach „Löschen und neu laden“, INTEG-010).
    pub async fn invalidate(&self) {
        *self.cache.lock().await = None;
    }

    /// Lädt den Report der Einstellungen in `ctx` (einmal je Einstellungsstand und Tag).
    pub async fn report(&self, ctx: &ApiCtx) -> Result<Arc<Report>, ApiError> {
        let settings = Ga4Settings::from_config(&ctx.cfg.api.ga4).map_err(settings_error)?;
        let today = self.clock.now().0.date();
        let fingerprint = format!(
            "{}|{:?}|{today}|{:?}",
            settings.fingerprint(),
            ctx.cfg.api.ga4.account,
            ctx.cfg.api.ga4.base_url
        );
        let mut cache = self.cache.lock().await;
        if let Some(c) = cache.as_ref() {
            let fresh =
                c.result.is_ok() || self.clock.monotonic().saturating_sub(c.at) < FAILURE_TTL;
            if c.fingerprint == fingerprint && fresh {
                return c.result.clone();
            }
        }
        self.set_status(
            ApiState::Fetching,
            settings.filter_hint().into_iter().collect(),
        );
        let result = self.load(ctx, &settings, today).await.map(Arc::new);
        match &result {
            Ok(report) => self.set_status(ApiState::Idle, report.notices.clone()),
            Err(ApiError::QuotaExceeded(m)) => self.set_status(
                ApiState::Paused,
                vec![format!(
                    "GA4-Kontingent erschöpft – Integration pausiert ({m})"
                )],
            ),
            Err(e) => self.set_status(ApiState::Failed, vec![e.public_message()]),
        }
        *cache = Some(Cached {
            fingerprint,
            result: result.clone(),
            at: self.clock.monotonic(),
        });
        result
    }

    async fn load(
        &self,
        ctx: &ApiCtx,
        settings: &Ga4Settings,
        today: time::Date,
    ) -> Result<Report, ApiError> {
        let dates = resolve_dates(&settings.date_range, today).map_err(settings_error)?;
        if let Some(c) = self
            .compat
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .filter(|c| c.fingerprint == settings.fingerprint())
        {
            let conflicts = c.conflicts(settings);
            if !conflicts.is_empty() {
                return Err(ApiError::Provider {
                    status: 400,
                    message: format!(
                        "Nicht kombinierbar: {} (Auswahl im GA4-Dialog anpassen)",
                        conflicts.join(", ")
                    ),
                });
            }
        }
        let client = self.client(ctx, &settings.property)?;
        let stream_host = match &settings.stream {
            None => None,
            Some(stream) => {
                let name = format!("properties/{}/dataStreams/{stream}", settings.property);
                let streams = client.data_streams(&settings.property).await?;
                let found = streams
                    .into_iter()
                    .find(|s| s.name == name)
                    .ok_or_else(|| ApiError::Provider {
                        status: 404,
                        message: format!("Datenstream {name} nicht gefunden"),
                    })?;
                Some(found.host.ok_or_else(|| ApiError::Provider {
                    status: 404,
                    message: format!("Datenstream {name} hat keine Domain"),
                })?)
            }
        };
        let currency = client
            .property(&settings.property)
            .await
            .ok()
            .and_then(|p| p.currency);
        let concurrency =
            (ctx.cfg.api.ga4.limits.concurrency as usize).clamp(1, MAX_CONCURRENT_PER_PROPERTY);
        let mut report = report::load(&report::LoadCtx {
            client: &client,
            settings,
            dates: &dates,
            stream_host: stream_host.as_deref(),
            quota: &self.quota,
            clock: self.clock.as_ref(),
            concurrency,
        })
        .await?;
        report.currency = currency;
        Ok(report)
    }
}

impl ApiSource for Ga4Source {
    fn kind(&self) -> ApiKind {
        ApiKind::Ga4
    }

    fn fetch<'a>(
        &'a self,
        urls: &'a [UrlRef],
        ctx: &'a ApiCtx,
    ) -> BoxFuture<'a, Result<Vec<ApiValueWrite>, ApiError>> {
        Box::pin(async move {
            let result = self.report(ctx).await;
            let mut s = self.status.lock().unwrap_or_else(|p| p.into_inner());
            s.done += urls.len() as u64;
            let report = match result {
                Ok(r) => r,
                Err(e) => {
                    s.failed += urls.len() as u64;
                    return Err(e);
                }
            };
            drop(s);
            let now = self.clock.now();
            Ok(urls
                .iter()
                .filter(|u| u.url_id.is_some())
                .flat_map(|u| report.values_for(u, &ctx.cfg.api.ga4.url_match, now))
                .collect())
        })
    }

    fn test_connection<'a>(
        &'a self,
        ctx: &'a ApiCtx,
        credential: &'a Credential,
    ) -> BoxFuture<'a, Result<Credential, ApiError>> {
        Box::pin(async move {
            let token = credential
                .access_token
                .as_deref()
                .ok_or_else(|| ApiError::NotConnected("ga4".into()))?;
            let client = Ga4Client::new(ctx, token, self.permits(""), self.clock.clone())?;
            let mut accounts = client.account_summaries(Retry::Once).await?;
            accounts.sort_by(|a, b| a.name.cmp(&b.name));
            let mut out = credential.clone();
            if let Some(first) = accounts.first() {
                // Ohne Identitätsscope kennt GA4 keine Nutzerkennung: das Konto mit der kleinsten Kennung ist der stabile
                // Schlüssel des Google-Zugangs, der Anzeigename das Label (I01-Erweiterungspunkt).
                out.account_id = format!("ga4:{}", first.name);
                out.label = match accounts.len() {
                    1 => first.display_name.clone(),
                    n => format!("{} (+{})", first.display_name, n - 1),
                };
            }
            Ok(out)
        })
    }

    fn quotas(&self) -> Vec<ProviderQuota> {
        self.quota.quotas()
    }

    fn status(&self) -> ApiStatus {
        let s = self.status.lock().unwrap_or_else(|p| p.into_inner());
        ApiStatus {
            kind: ApiKind::Ga4,
            state: s.state,
            urls_total: s.done,
            urls_done: s.done,
            urls_failed: s.failed,
            detail: (!s.notices.is_empty()).then(|| s.notices.join("; ")),
        }
    }
}
