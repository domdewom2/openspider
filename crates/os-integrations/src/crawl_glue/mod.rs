//! Shared API lifecycle for desktop, scheduled and command-line crawls (INTEG-012 … 017).
//! EnvironmentSecrets gives configured environment credentials precedence over the keychain (INTEG-002).
mod runtime;
pub use runtime::{enabled, preflight, select, ApiRun};

use crate::{ApiCtx, ApiError};
use os_model::{CrawlConfig, SystemClock};
use os_store::CrawlFile;
use std::sync::Arc;

/// Existing service entry point. Inputs have already been normalized and inserted by the caller.
pub async fn start_apis_mode(file: &CrawlFile, cfg: &CrawlConfig) -> Result<(), ApiError> {
    let ctx = ApiCtx {
        cfg: cfg.clone(),
        secrets: environment_view_for(os_fetch::secret_store_from_env(), cfg),
        network: Default::default(),
        allow_api_override: false,
    };
    start_apis_mode_with(file, ctx, Arc::new(SystemClock)).await
}

/// Explicit context keeps session credentials, network settings and test clocks with the caller.
pub async fn start_apis_mode_with(
    file: &CrawlFile,
    ctx: ApiCtx,
    clock: Arc<dyn os_model::Clock>,
) -> Result<(), ApiError> {
    validate_apis(&ctx)?;
    let mut run = ApiRun::new(ctx, clock, quota_ledger_path());
    run.validate(false)?;
    file.set_meta("source_mode", "apis").map_err(db)?;
    run.collect(file).await
}

/// Shared quota ledger in the application settings directory (INTEG-006).
pub fn quota_ledger_path() -> std::path::PathBuf {
    os_store::paths::app_paths()
        .settings
        .join("api-quotas.json")
}

pub(crate) fn db(_: impl std::fmt::Display) -> ApiError {
    ApiError::Transport("API-Daten konnten nicht gespeichert oder gelesen werden".into())
}

/// Read-only credential overlay. Environment keys are never written to disk or logged.
pub fn environment_view(inner: Arc<dyn os_fetch::SecretStore>) -> Arc<dyn os_fetch::SecretStore> {
    environment_view_for(inner, &CrawlConfig::default())
}
/// Profile account references also work when their key comes from the environment.
pub fn environment_view_for(
    inner: Arc<dyn os_fetch::SecretStore>,
    cfg: &CrawlConfig,
) -> Arc<dyn os_fetch::SecretStore> {
    let accounts = [
        ("api/psi", &cfg.api.psi.account),
        ("api/ahrefs", &cfg.api.ahrefs.account),
        ("api/majestic", &cfg.api.majestic.account),
        ("api/moz", &cfg.api.moz.account),
    ]
    .into_iter()
    .filter_map(|(kind, account)| account.as_ref().map(|a| (kind.to_string(), a.clone())))
    .collect();
    Arc::new(EnvironmentSecrets { inner, accounts })
}
struct EnvironmentSecrets {
    inner: Arc<dyn os_fetch::SecretStore>,
    accounts: std::collections::BTreeMap<String, String>,
}
impl os_fetch::SecretStore for EnvironmentSecrets {
    fn get(&self, id: &str) -> Option<String> {
        self.try_get(id).ok().flatten()
    }
    fn try_get(&self, id: &str) -> Result<Option<String>, os_fetch::SecretError> {
        let variable = match id {
            "api/psi" => "OPENSPIDER_PSI_KEY",
            "api/ahrefs" => "OPENSPIDER_AHREFS_KEY",
            "api/majestic" => "OPENSPIDER_MAJESTIC_KEY",
            "api/moz" => "OPENSPIDER_MOZ_KEY",
            _ => return self.inner.try_get(id),
        };
        let Some(key) = std::env::var(variable).ok().filter(|s| !s.is_empty()) else {
            return self.inner.try_get(id);
        };
        let account = self
            .accounts
            .get(id)
            .map(String::as_str)
            .unwrap_or("environment");
        Ok(Some(serde_json::json!({"accounts":{(account):{
            "account_id":account,"label":"Environment","api_key":key,
            "access_token":(id == "api/ahrefs").then_some(key.clone()),"refresh_token":null,"expires_at":null,"client_secret":null,"scopes":[]
        }}}).to_string()))
    }
}

/// APIs mode requires at least one connected, selected integration.
pub fn validate_apis(ctx: &ApiCtx) -> Result<(), ApiError> {
    if enabled(&ctx.cfg)
        .into_iter()
        .any(|kind| runtime::connected(ctx, kind))
    {
        return Ok(());
    }
    Err(ApiError::NotConnected(
        "Mindestens eine Integration muss verbunden sein".into(),
    ))
}

/// CLI argument mapping shared with scheduled/headless hosts (OUT-201).
pub fn configure_google(
    cfg: &mut CrawlConfig,
    ga4: Option<&[String]>,
    gsc: Option<&[String]>,
) -> Result<(), ApiError> {
    if let Some(values) = ga4 {
        cfg.api.ga4.enabled = true;
        if !values.is_empty() {
            if values.len() != 4 {
                return Err(ApiError::NotConnected(
                    "--use-google-analytics-4 erwartet KONTO ACCOUNT PROPERTY STROM".into(),
                ));
            }
            cfg.api.ga4.account = Some(values[0].clone());
            cfg.api.ga4.analytics_account = Some(values[1].clone());
            let property = values[2].trim_start_matches("properties/");
            let stream = values[3].rsplit('/').next().unwrap_or(&values[3]);
            cfg.api.ga4.property = Some(format!("properties/{property}/dataStreams/{stream}"));
        }
    }
    if let Some(values) = gsc {
        cfg.api.gsc.enabled = true;
        if !values.is_empty() {
            if values.len() != 2 {
                return Err(ApiError::NotConnected(
                    "--use-google-search-console erwartet KONTO WEBSITE".into(),
                ));
            }
            cfg.api.gsc.account = Some(values[0].clone());
            cfg.api.gsc.site = Some(values[1].clone());
        }
    }
    Ok(())
}

/// Optional host-provided credential view for scheduled/headless runs; Debug never reveals keys.
#[derive(Clone)]
pub struct SecretSnapshot(pub Arc<dyn os_fetch::SecretStore>);
impl std::fmt::Debug for SecretSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretSnapshot([redacted])")
    }
}
