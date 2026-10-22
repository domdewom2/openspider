//! Encrypted profile bridge for CLI and scheduling (OUT-187, OUT-223).
use std::{future::Future, path::Path, pin::Pin, sync::Arc};

use os_fetch::{
    BasicAuthEntry, CookieJar, Fetch, FetchConfig, FetchRequest, FetchResult, HttpFetcher,
    SecretStore,
};
use os_model::{Clock, CrawlConfig, SystemClock};
use os_store::paths::AppPaths;
use serde_json::Value;

use crate::{exit::CliError, reporter::Reporter};

#[derive(Default)]
pub struct AuthSession {
    basic: Vec<BasicAuthEntry>,
    cookies: Arc<CookieJar>,
    secrets: os_fetch::MemorySecretStore,
}
impl std::fmt::Debug for AuthSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AuthSession([redacted])")
    }
}

pub fn profile_path(paths: &AppPaths, profile: &str) -> std::path::PathBuf {
    let path = Path::new(profile);
    if path.is_file()
        || path.components().count() > 1
        || path.extension().is_some_and(|e| e == "osauth")
    {
        path.to_path_buf()
    } else {
        let encoded: String = profile
            .as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        paths
            .settings
            .join("auth")
            .join(format!("{encoded}.osauth"))
    }
}

pub fn load(cfg: &CrawlConfig, paths: &AppPaths, rep: &Reporter) -> Result<AuthSession, CliError> {
    load_with(
        cfg,
        paths,
        rep,
        os_fetch::secret_store_from_env().as_ref(),
        std::env::var("OPENSPIDER_AUTH_PASSPHRASE").ok().as_deref(),
    )
}

pub fn load_with(
    cfg: &CrawlConfig,
    paths: &AppPaths,
    rep: &Reporter,
    secrets: &dyn SecretStore,
    passphrase: Option<&str>,
) -> Result<AuthSession, CliError> {
    let mut session = AuthSession::default();
    for entry in &cfg.auth.basic {
        let password = secrets.get(&entry.secret_ref).ok_or_else(|| {
            CliError::usage(format!(
                "Passwort nicht verfügbar: SecretStore-Eintrag {:?} fehlt",
                entry.secret_ref
            ))
        })?;
        session.secrets.set(&entry.secret_ref, &password);
    }
    for profile in &cfg.auth.profile_refs {
        let bytes = std::fs::read(profile_path(paths, profile))
            .map_err(|_| CliError::usage("Authentifizierungsprofil nicht lesbar"))?;
        let value = os_store::auth_profiles::open(&bytes, passphrase, |id| secrets.get(id))
            .map_err(|e| CliError::usage(format!("Authentifizierungsprofil: {e}; portable Profile benötigen OPENSPIDER_AUTH_PASSPHRASE")))?;
        session.add(&value, rep)?;
    }
    Ok(session)
}

fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str, CliError> {
    value[key]
        .as_str()
        .ok_or_else(|| CliError::usage("Ungültiger Inhalt des Authentifizierungsprofils"))
}
impl AuthSession {
    fn add(&mut self, value: &Value, rep: &Reporter) -> Result<(), CliError> {
        let p = &value["profile"];
        let host = string(p, "host")?;
        let prefix = if host.contains("://") {
            host.to_string()
        } else {
            format!("https://{host}/")
        };
        let url = os_url::parse_start_url(&prefix)
            .map_err(|_| CliError::usage("Ungültiger Host im Authentifizierungsprofil"))?
            .url;
        if !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(CliError::usage(
                "Ungültiger Host im Authentifizierungsprofil",
            ));
        }
        match string(p, "kind")? {
            "basic" => self.basic.push(BasicAuthEntry {
                prefix,
                username: string(p, "username")?.into(),
                password: string(p, "secret")?.into(),
            }),
            "form" => {
                let storage: Value = serde_json::from_str(string(p, "secret")?).map_err(|_| {
                    CliError::usage("Ungültige Sitzungsdaten im Authentifizierungsprofil")
                })?;
                let cookies = storage["cookies"]
                    .as_array()
                    .ok_or_else(|| CliError::usage("Cookies fehlen im Authentifizierungsprofil"))?;
                let now = SystemClock.now().unix_millis() as f64 / 1000.0;
                let mut expired = false;
                for cookie in cookies {
                    let name = string(cookie, "name")?;
                    let value = string(cookie, "value")?;
                    let domain = cookie["domain"]
                        .as_str()
                        .unwrap_or_else(|| url.host_str().unwrap_or_default());
                    let path = cookie["path"].as_str().unwrap_or("/");
                    if [name, value, domain, path]
                        .iter()
                        .any(|s| s.contains([';', '\r', '\n']))
                        || name.contains('=')
                    {
                        return Err(CliError::usage(
                            "Ungültiges Cookie im Authentifizierungsprofil",
                        ));
                    }
                    let expires = cookie["expires"].as_f64().filter(|e| *e > 0.0);
                    if expires.is_some_and(|e| e <= now) {
                        expired = true;
                        continue;
                    }
                    let mut raw = format!("{name}={value}; Path={path}");
                    // CDP host-only cookies have no leading dot; retain their exact host scope.
                    let mut origin = url.clone();
                    if origin
                        .set_host(Some(domain.trim_start_matches('.')))
                        .is_err()
                    {
                        return Err(CliError::usage("Ungültige Cookie-Domain"));
                    }
                    if domain.starts_with('.') {
                        raw.push_str(&format!("; Domain={domain}"));
                    }
                    if cookie["secure"].as_bool() == Some(true) {
                        raw.push_str("; Secure");
                    }
                    if let Some(expires) = expires {
                        raw.push_str(&format!("; Max-Age={}", (expires - now) as u64));
                    }
                    self.cookies.insert_raw(&origin, &raw);
                }
                if expired {
                    rep.warn("authentication cookies expired");
                    tracing::warn!("authentication cookies expired");
                }
            }
            "digest" => {
                return Err(CliError::usage(
                    "Digest-Authentifizierung wird noch nicht unterstützt",
                ))
            }
            "ntlm" => {
                return Err(CliError::usage(
                    "NTLM-Authentifizierung wird noch nicht unterstützt",
                ))
            }
            _ => return Err(CliError::usage("Unbekannte Authentifizierungsart")),
        }
        Ok(())
    }

    pub fn fetcher(
        &self,
        cfg: &CrawlConfig,
        clock: Arc<dyn Clock>,
    ) -> Result<Arc<ProfileFetcher>, CliError> {
        let mut config = FetchConfig::from_crawl_config(cfg, &self.secrets, clock)
            .map_err(|e| CliError::usage(e.to_string()))?;
        config.basic_auth.extend(self.basic.clone());
        let fetcher = HttpFetcher::new(config).map_err(|e| CliError::usage(e.to_string()))?;
        Ok(Arc::new(ProfileFetcher {
            fetcher,
            cookies: Arc::clone(&self.cookies),
        }))
    }
}
pub struct ProfileFetcher {
    fetcher: HttpFetcher,
    cookies: Arc<CookieJar>,
}
impl Fetch for ProfileFetcher {
    fn render_config(&self) -> Option<FetchConfig> {
        self.fetcher.render_config()
    }
    fn render_cookie_jar(&self, session: Option<Arc<CookieJar>>) -> Option<Arc<CookieJar>> {
        self.fetcher.render_cookie_jar(if self.cookies.is_empty() {
            session
        } else {
            Some(self.cookies.clone())
        })
    }
    fn fetch(
        &self,
        mut req: FetchRequest,
    ) -> Pin<Box<dyn Future<Output = FetchResult> + Send + '_>> {
        if !self.cookies.is_empty() {
            req.cookies = Some(self.cookies.clone());
        }
        self.fetcher.fetch(req)
    }
}
