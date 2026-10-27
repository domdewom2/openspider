//! CRAWL-120: active form sessions, scoped cookies/storage and state-changing URL protection.
use crate::{frontier::Core, EngineDeps, EngineError};
use os_fetch::{CookieJar, SecretStore};
use os_model::{CrawlConfig, CrawlEvent, WriteBatch};
use os_render::login::CapturedSession;
use os_url::{RuleDecision, RuleSet};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};

pub struct SessionGuard {
    rules: RuleSet,
    logins: Vec<LoginState>,
    pub storage: Vec<os_render::login::OriginStorage>,
}
struct LoginState {
    url: url::Url,
    profile: String,
    redirects: AtomicU32,
}
impl SessionGuard {
    pub fn blocks(&self, url: &str) -> bool {
        matches!(self.rules.decide(url), RuleDecision::Excluded(_))
    }
}
/// Profile names use the same byte encoding as auth.saveProfile (OUT-223).
pub fn read_profile(dir: &Path, name: &str, secrets: &dyn SecretStore) -> Option<CapturedSession> {
    let filename: String = name.as_bytes().iter().map(|b| format!("{b:02x}")).collect();
    let bytes = std::fs::read(dir.join(format!("{filename}.osauth"))).ok()?;
    let value = os_store::auth_profiles::open(&bytes, None, |id| secrets.get(id)).ok()?;
    if value["profile"]["kind"] != "form" {
        return None;
    }
    serde_json::from_str(value["profile"]["secret"].as_str()?).ok()
}
// Adapt only the form-session namespace; all pre-existing secret lookups delegate unchanged.
struct ProfileSecrets {
    dir: PathBuf,
    inner: Arc<dyn SecretStore>,
}
impl SecretStore for ProfileSecrets {
    fn get(&self, id: &str) -> Option<String> {
        if let Some(name) = id.strip_prefix("form-session/") {
            return serde_json::to_string(&read_profile(&self.dir, name, &*self.inner)?).ok();
        }
        self.inner.get(id)
    }
    fn try_get(&self, id: &str) -> Result<Option<String>, os_fetch::secrets::SecretError> {
        if id.starts_with("form-session/") {
            Ok(self.get(id))
        } else {
            self.inner.try_get(id)
        }
    }
}
impl EngineDeps {
    /// Override the standard encrypted form-profile directory, without changing other dependencies.
    pub fn with_form_profile_dir(mut self, dir: PathBuf) -> Self {
        self.secrets = Arc::new(ProfileSecrets {
            dir,
            inner: self.secrets,
        });
        self
    }
}
pub(crate) fn initialize(
    core: &mut Core,
    cfg: &CrawlConfig,
    deps: &EngineDeps,
) -> Result<(), EngineError> {
    let active: Vec<_> = cfg
        .auth
        .form
        .iter()
        .filter(|f| f.profile.is_some())
        .collect();
    if active.is_empty() {
        return Ok(());
    }
    let patterns = cfg
        .auth
        .session_guard
        .iter()
        .map(|p| format!("(?i){p}"))
        .collect::<Vec<_>>();
    let rules = RuleSet::compile(&[], &patterns, false)
        .map_err(|_| EngineError::Invalid("Ungültiges Sitzungsschutz-Muster".into()))?;
    let mut sessions = Vec::new();
    let mut urls = Vec::new();
    for form in active {
        let name = form.profile.as_deref().expect("active profile");
        let session = deps
            .secrets
            .get(&format!("form-session/{name}"))
            .and_then(|s| serde_json::from_str(&s).ok())
            .or_else(|| {
                read_profile(
                    &os_store::paths::app_paths().settings.join("auth"),
                    name,
                    &*deps.secrets,
                )
            })
            .ok_or_else(|| EngineError::Invalid("Formular-Profil nicht verfügbar".into()))?;
        let url = url::Url::parse(&form.login_url)
            .map_err(|_| EngineError::Invalid("Ungültige Login-URL".into()))?;
        urls.push(LoginState {
            url,
            profile: name.into(),
            redirects: AtomicU32::new(0),
        });
        sessions.push(session);
    }
    let jar = Arc::new(CookieJar::new());
    let mut storage = Vec::new();
    for session in sessions {
        for c in session.cookies {
            let host = c.domain.trim_start_matches('.');
            let Ok(origin) = url::Url::parse(&format!(
                "{}://{host}/",
                if c.secure { "https" } else { "http" }
            )) else {
                continue;
            };
            let mut raw = format!("{}={}; Path={}", c.name, c.value, c.path);
            if c.domain.starts_with('.') {
                raw.push_str(&format!("; Domain={}", c.domain));
            }
            if c.secure {
                raw.push_str("; Secure");
            }
            if c.http_only {
                raw.push_str("; HttpOnly");
            }
            if let Some(expires) = c.expires {
                raw.push_str(&format!(
                    "; Max-Age={}",
                    expires
                        .saturating_sub(deps.clock.now().unix_millis() / 1000)
                        .max(0)
                ));
            }
            jar.insert_raw(&origin, &raw);
        }
        storage.extend(session.storage);
    }
    // Active form logins share the authenticated browser session across crawl seeds.
    for slot in &mut core.sessions {
        *slot = Some(jar.clone());
    }
    core.session_guard = Some(Arc::new(SessionGuard {
        rules,
        logins: urls,
        storage,
    }));
    Ok(())
}
pub(crate) fn observe_redirect(
    core: &Core,
    target: Option<&crate::redirect::RedirectTarget>,
    at: os_model::Timestamp,
    batch: &mut WriteBatch,
) {
    let (Some(guard), Some(target)) = (&core.session_guard, target) else {
        return;
    };
    let Ok(url) = url::Url::parse(&target.target.encoded) else {
        return;
    };
    for login in &guard.logins {
        if login.url.origin() == url.origin()
            && login.url.path() == url.path()
            && login.url.query() == url.query()
            && login.redirects.fetch_add(1, Ordering::AcqRel) == 2
        {
            batch.counters.push(("session_lost".into(), 1));
            batch
                .counters
                .push((format!("session_lost_profile:{}", login.profile), 1));
            batch.events.push(CrawlEvent {
                at,
                kind: "warn".into(),
                code: "session_lost".into(),
                message: "Sitzung im Crawl verloren".into(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // spec: CRAWL-120
    #[test]
    fn fixture_patterns_are_case_insensitive_full_matches_and_inactive_without_profiles() {
        let patterns = vec!["https://fixture.test/logout".to_owned()];
        let mut cfg = CrawlConfig::default();
        cfg.auth.session_guard = patterns;
        let mut core = Core::new(0, None);
        let deps = crate::testing::test_deps();
        initialize(&mut core, &cfg, &deps).unwrap();
        assert!(core.session_guard.is_none());
        let guard = SessionGuard {
            rules: RuleSet::compile(&[], &["(?i)https://fixture.test/logout".into()], false)
                .unwrap(),
            logins: vec![],
            storage: vec![],
        };
        for (url, blocked) in [
            ("https://fixture.test/LOGOUT", true),
            ("https://fixture.test/logout/extra", false),
            ("https://other.test/logout", false),
        ] {
            assert_eq!(guard.blocks(url), blocked);
        }
    }
}
