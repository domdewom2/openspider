//! Gültigkeitsprüfung von Aufgaben (OUT-168): beim Speichern, beim App-Start und vor jedem Lauf.
//!
//! Jeder Befund trägt den Pfad des Feldes, einen i18n-Schlüssel mit Argumenten und die Angabe, ob er die
//! Aufgabe ungültig macht (Fehler) oder nur ein Hinweis ist (Warnung). Äußere Zustände (Dateien, Konten,
//! Zugangsdaten) liefert das Merkmal [`Env`]; [`FsEnv`] ist die Umsetzung für App und CLI.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use os_fetch::SecretStore;
use os_model::Timestamp;

use super::model::{Schedule, Source, Task};
use super::schedule::{parse_date, parse_instant, parse_local, parse_time};
use super::zone::{SystemZone, Zone};

/// Die Integrationen, die eine Aufgabe abfragen darf (OUT-164).
pub const API_KINDS: [&str; 6] = ["ga4", "gsc", "psi", "majestic", "ahrefs", "moz"];

/// Ein Befund der Prüfung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// Pfad des Feldes (z. B. `crawl.source.url`).
    pub path: String,
    /// i18n-Schlüssel der Meldung.
    pub key: String,
    /// Argumente der Meldung.
    pub args: BTreeMap<String, String>,
    /// `true` = Hinweis, die Aufgabe bleibt gültig.
    pub warning: bool,
}

impl Issue {
    fn error(path: &str, key: &str) -> Issue {
        Issue {
            path: path.into(),
            key: key.into(),
            args: BTreeMap::new(),
            warning: false,
        }
    }

    fn with(mut self, name: &str, value: impl Into<String>) -> Issue {
        self.args.insert(name.into(), value.into());
        self
    }

    fn warn(mut self) -> Issue {
        self.warning = true;
        self
    }

    /// Kurztext für Historie und Log: `<pfad>: <schlüssel> {args}`.
    pub fn summary(&self) -> String {
        if self.args.is_empty() {
            format!("{}: {}", self.path, self.key)
        } else {
            let args: Vec<String> = self.args.iter().map(|(k, v)| format!("{k}={v}")).collect();
            format!("{}: {} ({})", self.path, self.key, args.join(", "))
        }
    }
}

/// Zustand der Zugangsdaten einer Integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Credentials {
    /// Vorhanden und verwendbar.
    Ready,
    /// Keine Zugangsdaten hinterlegt.
    Missing,
    /// Abgelaufen bzw. nicht mehr gültig.
    Expired,
}

/// Äußere Zustände, die die Prüfung braucht.
pub trait Env {
    /// Jetzt.
    fn now(&self) -> Timestamp;
    /// Zeitzone.
    fn zone(&self) -> &dyn Zone;
    /// Gibt es das Konfigurationsprofil bzw. die `.osconfig`-Datei?
    fn config_exists(&self, reference: &str) -> bool;
    /// Gibt es das Authentifizierungsprofil bzw. die `.osauth`-Datei?
    fn auth_profile_exists(&self, reference: &str) -> bool;
    /// Gibt es die Datei?
    fn file_exists(&self, path: &str) -> bool {
        Path::new(path).is_file()
    }
    /// Ist der Ordner (oder der nächste vorhandene übergeordnete Ordner) beschreibbar?
    fn dir_writable(&self, path: &Path) -> bool;
    /// Standard-Exportordner.
    fn default_export_dir(&self) -> PathBuf;
    /// Zugangsdaten einer Integration.
    fn credentials(&self, api: &str) -> Credentials;
    /// Ist das Google-Konto verbunden und gültig?
    fn google_account_ready(&self, account: &str) -> bool;
    /// Gibt es das Mail-Konto?
    fn mail_account_exists(&self, id: &str) -> bool;
    /// Ist die Metrik im aktuellen Katalog? `None` = Katalog nicht verfügbar.
    fn metric_known(&self, _name: &str) -> Option<bool> {
        None
    }
    /// Gibt es die Exportvorlage?
    fn preset_exists(&self, name: &str) -> bool;
    /// Ist der Name schon an eine andere Aufgabe vergeben?
    fn name_taken(&self, name: &str, except_id: Option<&str>) -> bool;
}

fn is_http(text: &str) -> bool {
    let lower = text.trim().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

fn is_sheet(text: &str) -> bool {
    text.contains("docs.google.com/spreadsheets")
}

/// Ist `text` eine verwendbare Start-URL (Schema wird bei Bedarf als `http://` ergänzt)?
pub fn start_url_valid(text: &str) -> bool {
    let text = text.trim();
    if text.is_empty() || text.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return false;
    }
    let full = if text.contains("://") {
        text.to_string()
    } else {
        format!("http://{text}")
    };
    if !is_http(&full) {
        return false;
    }
    let rest = full.split_once("://").map_or("", |(_, r)| r);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = host.rsplit_once(':').map_or(host, |(h, p)| {
        if p.chars().all(|c| c.is_ascii_digit()) {
            h
        } else {
            host
        }
    });
    !host.is_empty() && !host.starts_with('.') && !host.ends_with('.')
}

/// Prüft die Aufgabe. Ergebnis leer = gültig.
pub fn validate(task: &Task, env: &dyn Env) -> Vec<Issue> {
    let mut out = Vec::new();

    // OUT-152: Auto Compare braucht einen expliziten Projektnamen; der Aufgabenname ist kein Projekt.
    if task.execution.auto_compare && task.project.trim().is_empty() {
        out.push(Issue::error(
            "project",
            "tasks-issue-auto-compare-project-required",
        ));
    }

    // Allgemein
    if task.name.trim().is_empty() {
        out.push(Issue::error("name", "tasks-issue-name-required"));
    } else if env.name_taken(
        &task.name,
        (!task.id.is_empty()).then_some(task.id.as_str()),
    ) {
        out.push(Issue::error("name", "tasks-issue-name-taken").with("name", task.name.trim()));
    }
    schedule_issues(&task.schedule, env, &mut out);

    // Spider
    match &task.crawl.source {
        Source::Spider { url } => {
            if !start_url_valid(url) {
                out.push(
                    Issue::error("crawl.source.url", "tasks-issue-start-url-invalid")
                        .with("url", url.trim()),
                );
            }
        }
        Source::List { .. } => {
            let sources = task.crawl.source.list_sources();
            if sources.is_empty() {
                out.push(Issue::error(
                    "crawl.source.path",
                    "tasks-issue-list-source-missing",
                ));
            }
            for source in sources {
                if is_sheet(&source) {
                    if !env.google_account_ready("drive") {
                        out.push(
                            Issue::error("crawl.source.path", "tasks-issue-google-account")
                                .with("account", "drive"),
                        );
                    }
                } else if is_http(&source) {
                    if !start_url_valid(&source) {
                        out.push(
                            Issue::error("crawl.source.path", "tasks-issue-start-url-invalid")
                                .with("url", source),
                        );
                    }
                } else if !env.file_exists(&source) {
                    out.push(
                        Issue::error("crawl.source.path", "tasks-issue-list-file-missing")
                            .with("path", source),
                    );
                }
            }
        }
    }
    if let Some(config) = task
        .crawl
        .config
        .as_deref()
        .filter(|c| !c.trim().is_empty())
    {
        if !env.config_exists(config) {
            out.push(
                Issue::error("crawl.config", "tasks-issue-config-missing").with("config", config),
            );
        }
    }
    if let Some(auth) = task
        .crawl
        .auth_profile
        .as_deref()
        .filter(|c| !c.trim().is_empty())
    {
        if !env.auth_profile_exists(auth) {
            out.push(
                Issue::error("crawl.authProfile", "tasks-issue-auth-missing").with("profile", auth),
            );
        }
    }

    // APIs
    for api in &task.apis {
        if !API_KINDS.contains(&api.as_str()) {
            out.push(Issue::error("apis", "tasks-issue-api-unknown").with("api", api.as_str()));
            continue;
        }
        match env.credentials(api) {
            Credentials::Ready => {}
            Credentials::Missing => out.push(
                Issue::error("apis", "tasks-issue-api-credentials").with("api", api.as_str()),
            ),
            Credentials::Expired => {
                out.push(Issue::error("apis", "tasks-issue-api-expired").with("api", api.as_str()))
            }
        }
    }

    // Exporte
    let exports = task.effective_exports();
    let produces_files = !exports.items.is_empty()
        || exports.sitemap_pages
        || exports.sitemap_images
        || task.output.save_crawl
        || task.preset.is_some();
    if produces_files {
        let dir = exports
            .dir
            .as_deref()
            .filter(|d| !d.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| env.default_export_dir());
        if !env.dir_writable(&dir) {
            out.push(
                Issue::error("exports.dir", "tasks-issue-output-not-writable")
                    .with("path", dir.to_string_lossy()),
            );
        }
    }
    if let Some(preset) = task.preset.as_deref().filter(|p| !p.trim().is_empty()) {
        if !env.preset_exists(preset) {
            out.push(
                Issue::error("exportPreset", "tasks-issue-preset-missing").with("preset", preset),
            );
        }
    }
    if let Some(summary) = &task.custom_summary {
        if !env.google_account_ready(&summary.account) {
            out.push(
                Issue::error("customSummary.account", "tasks-issue-google-account")
                    .with("account", summary.account.as_str()),
            );
        }
        for metric in &summary.metrics {
            if env.metric_known(metric) == Some(false) {
                out.push(
                    Issue::error("customSummary.metrics", "tasks-metric-unavailable")
                        .with("name", metric.as_str())
                        .warn(),
                );
            }
        }
    }

    // Benachrichtigung
    if (task.notify.on_success || task.notify.on_failure)
        && task.notify.to.iter().any(|a| !a.trim().is_empty())
    {
        match task.notify.account.as_deref() {
            Some(id) if env.mail_account_exists(id) => {}
            other => out.push(
                Issue::error("notify.account", "tasks-issue-mail-account-missing")
                    .with("account", other.unwrap_or("")),
            ),
        }
    }
    out
}

fn schedule_issues(schedule: &Schedule, env: &dyn Env, out: &mut Vec<Issue>) {
    let time_bad = |time: &str, out: &mut Vec<Issue>| {
        if parse_time(time).is_none() {
            out.push(Issue::error("schedule.time", "tasks-issue-time-invalid").with("time", time));
            true
        } else {
            false
        }
    };
    let from_bad = |from: &Option<String>, out: &mut Vec<Issue>| {
        if let Some(from) = from {
            if parse_date(from).is_none() {
                out.push(
                    Issue::error("schedule.from", "tasks-issue-date-invalid")
                        .with("date", from.as_str()),
                );
            }
        }
    };
    match schedule {
        Schedule::Once { at } => match parse_instant(at) {
            None => out.push(Issue::error("schedule.at", "tasks-once-future")),
            Some(t) if t <= env.now().0 => {
                out.push(Issue::error("schedule.at", "tasks-once-future"))
            }
            Some(_) => {}
        },
        Schedule::Hourly { start } | Schedule::EveryDays { start, .. } => {
            if parse_local(start).is_none() {
                out.push(
                    Issue::error("schedule.start", "tasks-issue-date-invalid")
                        .with("date", start.as_str()),
                );
            }
            if matches!(schedule, Schedule::EveryDays { every: 0, .. }) {
                out.push(Issue::error(
                    "schedule.every",
                    "tasks-issue-interval-invalid",
                ));
            }
        }
        Schedule::Daily { time, from } => {
            time_bad(time, out);
            from_bad(from, out);
        }
        Schedule::Weekly { days, time, from } => {
            time_bad(time, out);
            from_bad(from, out);
            if days.is_empty() || days.iter().any(|d| !(1..=7).contains(d)) {
                out.push(Issue::error("schedule.days", "tasks-weekday-required"));
            }
        }
        Schedule::Monthly { day, time, from } => {
            time_bad(time, out);
            from_bad(from, out);
            if !(1..=31).contains(day) {
                out.push(Issue::error("schedule.day", "tasks-month-day-invalid"));
            }
        }
    }
}

/// Befund „nicht beim Betriebssystem registriert“ (OUT-168): nur für eingeschaltete Aufgaben ein Fehler.
pub fn registration_issue(task: &Task, registered: bool) -> Option<Issue> {
    (task.enabled && !registered).then(|| Issue::error("registered", "tasks-not-registered"))
}

/// Nur die Fehler (ohne Hinweise).
pub fn errors(issues: &[Issue]) -> Vec<&Issue> {
    issues.iter().filter(|i| !i.warning).collect()
}

/// Umsetzung von [`Env`] über das Dateisystem und den Geheimnisspeicher; für App und CLI.
pub struct FsEnv {
    /// Einstellungsordner.
    pub settings: PathBuf,
    /// Geheimnisse (`api/<kind>`, `mail/<id>`).
    pub secrets: Arc<dyn SecretStore>,
    /// Aufgabenordner (Namensprüfung).
    pub store: super::store::TaskStore,
    /// Uhr.
    pub clock: Arc<dyn os_model::Clock>,
    /// Zeitzone.
    pub zone: Box<dyn Zone>,
    /// Standard-Exportordner.
    pub export_dir: PathBuf,
}

impl FsEnv {
    /// Umgebung für den Betrieb: Systemuhr, lokale Zeit.
    pub fn system(settings: &Path, secrets: Arc<dyn SecretStore>, export_dir: PathBuf) -> FsEnv {
        FsEnv {
            settings: settings.to_path_buf(),
            secrets,
            store: super::store::TaskStore::in_settings(settings),
            clock: Arc::new(os_model::SystemClock),
            zone: Box::new(SystemZone),
            export_dir,
        }
    }

    /// Pfad der `.osauth`-Datei zu einem Profilnamen (Namen sind hexcodiert, wie in der Profilverwaltung).
    pub fn auth_profile_path(&self, name: &str) -> PathBuf {
        let encoded: String = name.as_bytes().iter().map(|b| format!("{b:02x}")).collect();
        self.settings.join("auth").join(format!("{encoded}.osauth"))
    }

    /// Pfad der Exportvorlage `<name>.ospreset`.
    pub fn preset_path(&self, name: &str) -> PathBuf {
        self.settings
            .join("export-presets")
            .join(format!("{name}.ospreset"))
    }
}

impl Env for FsEnv {
    fn now(&self) -> Timestamp {
        self.clock.now()
    }

    fn zone(&self) -> &dyn Zone {
        self.zone.as_ref()
    }

    fn config_exists(&self, reference: &str) -> bool {
        let as_file = Path::new(reference);
        if as_file.extension().is_some_and(|e| e == "osconfig") || as_file.components().count() > 1
        {
            return as_file.is_file();
        }
        self.settings
            .join("profiles")
            .join(format!("{reference}.osconfig"))
            .is_file()
    }

    fn auth_profile_exists(&self, reference: &str) -> bool {
        let as_file = Path::new(reference);
        if as_file.extension().is_some_and(|e| e == "osauth") || as_file.components().count() > 1 {
            return as_file.is_file();
        }
        self.auth_profile_path(reference).is_file()
    }

    fn dir_writable(&self, path: &Path) -> bool {
        let mut probe = Some(path);
        while let Some(p) = probe {
            if p.exists() {
                return p.is_dir()
                    && std::fs::metadata(p).is_ok_and(|m| !m.permissions().readonly())
                    && can_create_in(p);
            }
            probe = p.parent().filter(|q| !q.as_os_str().is_empty());
        }
        false
    }

    fn default_export_dir(&self) -> PathBuf {
        self.export_dir.clone()
    }

    fn credentials(&self, api: &str) -> Credentials {
        let env_key = match api {
            "psi" => Some("OPENSPIDER_PSI_KEY"),
            "ahrefs" => Some("OPENSPIDER_AHREFS_KEY"),
            "majestic" => Some("OPENSPIDER_MAJESTIC_KEY"),
            "moz" => Some("OPENSPIDER_MOZ_KEY"),
            _ => None,
        };
        if env_key.is_some_and(|k| std::env::var_os(k).is_some_and(|v| !v.is_empty())) {
            return Credentials::Ready;
        }
        let Some(raw) = self.secrets.get(&format!("api/{api}")) else {
            return Credentials::Missing;
        };
        // OAuth-Konten tragen `expires_at` (Unix-Sekunden oder RFC 3339), Schlüssel nicht.
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
            return Credentials::Ready;
        };
        let expired = match value.get("expiresAt").or_else(|| value.get("expires_at")) {
            Some(serde_json::Value::String(s)) => {
                parse_instant(s).is_some_and(|t| t <= self.now().0)
                    && value
                        .get("refreshToken")
                        .or_else(|| value.get("refresh_token"))
                        .is_none()
            }
            _ => false,
        };
        if expired {
            Credentials::Expired
        } else {
            Credentials::Ready
        }
    }

    fn google_account_ready(&self, account: &str) -> bool {
        // Drive und Sheets teilen sich die Google-Verbindung `api/drive` (INTEG-002).
        if self.credentials("drive") != Credentials::Ready {
            return false;
        }
        let account = account.trim();
        if account.is_empty() || account == "drive" {
            return true;
        }
        let stored = self
            .secrets
            .get("api/drive")
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
            .and_then(|v| {
                v.get("account")
                    .and_then(|a| a.as_str())
                    .map(str::to_string)
            });
        stored.is_none_or(|s| s == account)
    }

    fn mail_account_exists(&self, id: &str) -> bool {
        self.secrets.get(&format!("mail/{id}")).is_some()
    }

    fn preset_exists(&self, name: &str) -> bool {
        self.preset_path(name).is_file()
    }

    fn name_taken(&self, name: &str, except_id: Option<&str>) -> bool {
        self.store.name_taken(name, except_id)
    }
}

/// Kann in `dir` eine Datei angelegt werden? Prüft mit einer kurzlebigen Probedatei.
fn can_create_in(dir: &Path) -> bool {
    let probe = dir.join(format!(".openspider-write-test-{}", std::process::id()));
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}
