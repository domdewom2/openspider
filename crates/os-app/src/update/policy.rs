//! Shared, Tauri-free check policy. Also compiled by the CLI through a source module.
use super::manifest::{https_url, platform_key, UpdateManifest};
use os_model::settings::{NetworkSettings, UpdateSettings};
use std::{sync::Mutex, time::Duration};
pub const USER_AGENT: &str = "OpenSpider-Updater";
pub const STABLE_ENDPOINT: &str =
    "https://github.com/domdewom2/openspider/releases/latest/download/latest.json";
pub const BETA_ENDPOINT: &str =
    "https://github.com/domdewom2/openspider/releases/latest/download/latest-beta.json";
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disabled {
    Settings,
    Environment,
    MissingPublicKey,
    InvalidPublicKey,
}
#[derive(Debug)]
pub enum Outcome {
    Disabled(Disabled),
    Throttled,
    Current,
    Available(UpdateManifest),
}
pub struct UpdateContext {
    pub endpoint: String,
    pub beta: bool,
    pub current: semver::Version,
    pub os: String,
    pub arch: String,
    pub installer: Option<String>,
    pub pubkey: String,
    pub settings: UpdateSettings,
    pub disabled_by_environment: bool,
    pub network: NetworkSettings,
    pub proxy_password: Option<String>,
    pub now: time::OffsetDateTime,
    /// Serializes checks and reserves the timestamp before any request, including failed requests.
    pub last_attempt: Mutex<Option<time::OffsetDateTime>>,
}
impl Default for UpdateContext {
    fn default() -> Self {
        let beta = option_env!("OPENSPIDER_UPDATE_CHANNEL") == Some("beta");
        Self {
            endpoint: if beta { BETA_ENDPOINT } else { STABLE_ENDPOINT }.into(),
            beta,
            current: semver::Version::parse(env!("CARGO_PKG_VERSION")).expect("package SemVer"),
            os: match std::env::consts::OS {
                "macos" => "darwin",
                other => other,
            }
            .into(),
            arch: std::env::consts::ARCH.into(),
            installer: option_env!("OPENSPIDER_UPDATE_INSTALLER")
                .map(str::to_owned)
                .or_else(|| cfg!(windows).then(|| "nsis".into())),
            pubkey: option_env!("OPENSPIDER_UPDATER_PUBKEY")
                .unwrap_or("")
                .into(),
            settings: UpdateSettings::default(),
            disabled_by_environment: std::env::var("OPENSPIDER_DISABLE_UPDATE_CHECK").as_deref()
                == Ok("1"),
            network: NetworkSettings::default(),
            proxy_password: None,
            now: os_model::Clock::now(&os_model::SystemClock).0,
            last_attempt: Mutex::new(None),
        }
    }
}
impl UpdateContext {
    pub fn disabled(&self) -> Option<Disabled> {
        if self.disabled_by_environment {
            Some(Disabled::Environment)
        } else if !self.settings.check {
            Some(Disabled::Settings)
        } else if self.pubkey.is_empty() {
            Some(Disabled::MissingPublicKey)
        } else if super::verify::public_key(&self.pubkey).is_err() {
            Some(Disabled::InvalidPublicKey)
        } else {
            None
        }
    }
    pub fn target(&self) -> String {
        platform_key(&self.os, &self.arch, self.installer.as_deref())
    }
}
/// Five seconds covers DNS, TLS, redirects and response streaming together.
pub fn reserve(ctx: &UpdateContext, manual: bool) -> Result<Option<Outcome>, String> {
    if let Some(reason) = ctx.disabled() {
        return Ok(Some(Outcome::Disabled(reason)));
    }
    let endpoint = https_url(&ctx.endpoint)?;
    if endpoint.query().is_some() {
        return Err("Update-Endpunkt darf keine Query enthalten".into());
    }
    {
        let mut last = ctx.last_attempt.lock().unwrap_or_else(|p| p.into_inner());
        let persisted = ctx.settings.last_check.as_deref().and_then(|s| {
            time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).ok()
        });
        if !manual
            && last
                .max(persisted)
                .is_some_and(|at| ctx.now - at < time::Duration::hours(24))
        {
            return Ok(Some(Outcome::Throttled));
        }
        *last = Some(ctx.now);
    }
    Ok(None)
}
pub async fn check(ctx: &UpdateContext, manual: bool) -> Result<Outcome, String> {
    match reserve(ctx, manual)? {
        Some(outcome) => Ok(outcome),
        None => fetch(ctx).await,
    }
}
/// Call only after reserve; the app persists the reservation before fetching.
pub async fn fetch(ctx: &UpdateContext) -> Result<Outcome, String> {
    let mut endpoint = https_url(&ctx.endpoint)?;
    endpoint.query_pairs_mut().extend_pairs([
        ("version", ctx.current.to_string().as_str()),
        ("os", &ctx.os),
        ("arch", &ctx.arch),
    ]);
    let request = async {
        let client = os_fetch::net::client_builder_with_password(
            &ctx.network,
            ctx.proxy_password.as_deref(),
        )
        .user_agent(USER_AGENT)
        .referer(false)
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 || https_url(attempt.url().as_str()).is_err() {
                attempt.error("Unsichere Update-Weiterleitung")
            } else {
                attempt.follow()
            }
        }))
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|_| "Update-Client nicht verfügbar")?;
        let mut response = client
            .get(endpoint)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|_| "Update-Server nicht erreichbar")?
            .error_for_status()
            .map_err(|_| "Update-Server meldet Fehler")?;
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "Update-Antwort abgebrochen")?
        {
            if bytes.len() + chunk.len() > 1024 * 1024 {
                return Err("Update-Manifest zu groß".to_string());
            }
            bytes.extend_from_slice(&chunk);
        }
        let manifest: UpdateManifest =
            serde_json::from_slice(&bytes).map_err(|_| "Update-Manifest ungültig")?;
        manifest.validate(ctx.beta)?;
        if !manifest.is_newer_than(&ctx.current) {
            return Ok(Outcome::Current);
        }
        if !manifest.platforms.contains_key(&ctx.target()) {
            return Err("Kein Update für diese Plattform".into());
        }
        Ok(Outcome::Available(manifest))
    };
    let result = tokio::time::timeout(Duration::from_secs(5), request)
        .await
        .map_err(|_| "Update-Prüfung: Zeitüberschreitung".to_string())
        .and_then(|r| r);
    if result.is_err() {
        tracing::info!("Update-Prüfung fehlgeschlagen (Netzwerk oder Manifest)");
    }
    result
}
