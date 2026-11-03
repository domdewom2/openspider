//! Shared, Tauri-free check policy. Also compiled by the CLI through a source module.
use super::manifest::{https_url, platform_key, UpdateManifest};
use os_model::settings::{NetworkSettings, UpdateSettings};
use std::{sync::Mutex, time::Duration};
pub const USER_AGENT: &str = "OpenSpider-Updater";
pub const STABLE_ENDPOINT: &str =
    "https://github.com/wojcikdominik/openspider/releases/latest/download/latest.json";
pub const BETA_ENDPOINT: &str =
    "https://github.com/wojcikdominik/openspider/releases/latest/download/latest-beta.json";
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
        let beta = option_env!("OPENSPIDER_UPDATE_BETA") == Some("1");
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
            installer: cfg!(windows).then(|| "nsis".into()),
            pubkey: option_env!("OPENSPIDER_UPDATER_PUBKEY")
                .unwrap_or("")
                .into(),
            settings: UpdateSettings::default(),
            // Gemeinsamer Richtlinien-Resolver (OUT-253; M3-C01 nutzt ihn für DisableCrashReports).
            disabled_by_environment: update_check_disabled(&SystemPolicy),
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

// --- Verwaltete Richtlinien (OUT-253, gemeinsamer Resolver; M3-C01 ergänzt `DisableCrashReports`, OUT-247) ---

/// Name des Richtlinienwerts für Absturzberichte (OUT-247).
pub const POLICY_DISABLE_CRASH_REPORTS: &str = "DisableCrashReports";
/// Umgebungsvariable für Absturzberichte (OUT-247).
pub const ENV_DISABLE_CRASH_REPORTS: &str = "OPENSPIDER_DISABLE_CRASH_REPORTS";
/// Name des Richtlinienwerts für die Update-Prüfung (OUT-253).
pub const POLICY_DISABLE_UPDATE_CHECK: &str = "DisableUpdateCheck";
/// Umgebungsvariable für die Update-Prüfung (OUT-250).
pub const ENV_DISABLE_UPDATE_CHECK: &str = "OPENSPIDER_DISABLE_UPDATE_CHECK";

/// Ein gelesener Richtlinienwert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyValue {
    /// Nicht vorhanden.
    Missing,
    /// `1`/`true` bzw. `0`/`false`.
    Set(bool),
    /// Vorhanden, aber falscher Typ (z. B. REG_SZ „ja“): wird ignoriert, Warnung im Log.
    Invalid,
}

/// Quellen der Richtlinie in der Rangfolge von OUT-253: Rechner (HKLM bzw. macOS-Konfigurationsprofil) > Benutzer
/// (HKCU) > Umgebungsvariable. Die Benutzereinstellung (`settings.toml`) entscheidet nur, wenn keine Quelle einen
/// gültigen Wert hat.
pub trait PolicySource {
    /// HKLM bzw. Konfigurationsprofil (Domain `org.openspider.desktop`).
    fn machine(&self, name: &str) -> PolicyValue;
    /// HKCU.
    fn user(&self, name: &str) -> PolicyValue;
    /// Rohwert der Umgebungsvariable.
    fn env(&self, var: &str) -> Option<String>;
}

/// Wer hat entschieden?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyOrigin {
    Machine,
    User,
    Environment,
}

/// Umgebungswert lesen: `1`/`true` → an, `0`/`false` → aus, leer → fehlt, sonst ungültig.
pub fn env_value(raw: Option<&str>) -> PolicyValue {
    match raw.map(|v| v.trim().to_ascii_lowercase()) {
        None => PolicyValue::Missing,
        Some(v) if v.is_empty() => PolicyValue::Missing,
        Some(v) if v == "1" || v == "true" => PolicyValue::Set(true),
        Some(v) if v == "0" || v == "false" => PolicyValue::Set(false),
        Some(_) => PolicyValue::Invalid,
    }
}

/// Gemeinsamer Resolver: der erste gültige Wert in der Rangfolge entscheidet; `None` = keine Richtlinie, die
/// Benutzereinstellung gilt. Ungültige Werte werden mit Warnung übergangen.
pub fn resolve_policy(
    source: &dyn PolicySource,
    name: &str,
    env_var: &str,
) -> Option<(bool, PolicyOrigin)> {
    let candidates = [
        (source.machine(name), PolicyOrigin::Machine),
        (source.user(name), PolicyOrigin::User),
        (
            env_value(source.env(env_var).as_deref()),
            PolicyOrigin::Environment,
        ),
    ];
    for (value, origin) in candidates {
        match value {
            PolicyValue::Set(v) => return Some((v, origin)),
            PolicyValue::Invalid => {
                tracing::warn!(
                    policy = name,
                    ?origin,
                    "Richtlinienwert hat einen ungültigen Typ und wird ignoriert"
                );
            }
            PolicyValue::Missing => {}
        }
    }
    None
}

/// Quellen dieses Rechners. Registry- (Windows) und Profil-Leser (macOS) gibt es in dieser Fassung noch nicht
/// (OUT-253, manuelle Prüfung M3-Q03); wirksam ist die Umgebungsvariable.
pub struct SystemPolicy;

impl PolicySource for SystemPolicy {
    fn machine(&self, _: &str) -> PolicyValue {
        PolicyValue::Missing
    }
    fn user(&self, _: &str) -> PolicyValue {
        PolicyValue::Missing
    }
    fn env(&self, var: &str) -> Option<String> {
        std::env::var(var).ok()
    }
}

/// Sind Absturzberichte durch eine Richtlinie deaktiviert (`DisableCrashReports`, OUT-247)?
pub fn crash_reports_disabled(source: &dyn PolicySource) -> bool {
    resolve_policy(
        source,
        POLICY_DISABLE_CRASH_REPORTS,
        ENV_DISABLE_CRASH_REPORTS,
    )
    .is_some_and(|(disabled, _)| disabled)
}

/// Ist die Update-Prüfung durch eine Richtlinie deaktiviert (`DisableUpdateCheck`, OUT-253)? Derselbe Resolver.
pub fn update_check_disabled(source: &dyn PolicySource) -> bool {
    resolve_policy(
        source,
        POLICY_DISABLE_UPDATE_CHECK,
        ENV_DISABLE_UPDATE_CHECK,
    )
    .is_some_and(|(disabled, _)| disabled)
}

#[cfg(test)]
mod policy_tests {
    use super::*;

    struct Fixture {
        machine: PolicyValue,
        user: PolicyValue,
        env: Option<&'static str>,
    }
    impl PolicySource for Fixture {
        fn machine(&self, _: &str) -> PolicyValue {
            self.machine
        }
        fn user(&self, _: &str) -> PolicyValue {
            self.user
        }
        fn env(&self, _: &str) -> Option<String> {
            self.env.map(str::to_string)
        }
    }

    // spec: OUT-247
    // spec: OUT-253 [M3]
    #[test]
    fn rank_order_machine_user_environment_for_both_policies() {
        use PolicyValue::{Invalid, Missing, Set};
        let values = [Missing, Set(false), Set(true)];
        for machine in values {
            for user in values {
                for env in [None, Some("0"), Some("1")] {
                    let f = Fixture { machine, user, env };
                    let expected = match (machine, user) {
                        (Set(m), _) => m,
                        (Missing, Set(u)) => u,
                        _ => env == Some("1"),
                    };
                    assert_eq!(
                        crash_reports_disabled(&f),
                        expected,
                        "{machine:?} {user:?} {env:?}"
                    );
                    assert_eq!(
                        update_check_disabled(&f),
                        expected,
                        "{machine:?} {user:?} {env:?}"
                    );
                }
            }
        }
        // HKLM=0, HKCU=1 → aktiv
        let f = Fixture {
            machine: Set(false),
            user: Set(true),
            env: Some("1"),
        };
        assert!(!crash_reports_disabled(&f));
        // Ungültiger Typ wird ignoriert, die nächste Quelle entscheidet.
        let f = Fixture {
            machine: Invalid,
            user: Missing,
            env: Some("1"),
        };
        assert_eq!(
            resolve_policy(&f, POLICY_DISABLE_CRASH_REPORTS, ENV_DISABLE_CRASH_REPORTS),
            Some((true, PolicyOrigin::Environment))
        );
        let f = Fixture {
            machine: Invalid,
            user: Invalid,
            env: Some("ja"),
        };
        assert_eq!(
            resolve_policy(&f, POLICY_DISABLE_CRASH_REPORTS, ENV_DISABLE_CRASH_REPORTS),
            None
        );
    }

    // spec: OUT-247
    #[test]
    fn environment_values() {
        assert_eq!(env_value(Some("1")), PolicyValue::Set(true));
        assert_eq!(env_value(Some(" TRUE ")), PolicyValue::Set(true));
        assert_eq!(env_value(Some("0")), PolicyValue::Set(false));
        assert_eq!(env_value(Some("")), PolicyValue::Missing);
        assert_eq!(env_value(None), PolicyValue::Missing);
        assert_eq!(env_value(Some("ja")), PolicyValue::Invalid);
    }
}
