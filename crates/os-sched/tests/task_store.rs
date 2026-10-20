//! Aufgabenablage, Gültigkeitsprüfung und Export/Import (OUT-160, OUT-162, OUT-164, OUT-165, OUT-168,
//! OUT-170) gegen `tests/fixtures/out-m2-tasks/nightly.ostask`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use os_fetch::MemorySecretStore;
use os_model::{Clock, Timestamp};
use os_sched::tasks::bundle;
use os_sched::tasks::store::{folder_name, valid_id};
use os_sched::tasks::validate::{self, Credentials, Env, FsEnv, Issue};
use os_sched::tasks::{EuZone, ExportFormat, Schedule, Source, StoreError, Task, TaskStore, Zone};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/out-m2-tasks/nightly.ostask")
}

fn nightly() -> Task {
    TaskStore::read_file(&fixture()).expect("Fixture lesbar")
}

/// Umgebung mit einstellbaren Zuständen.
struct TestEnv {
    now: Timestamp,
    zone: EuZone,
    configs: BTreeSet<String>,
    auth: BTreeSet<String>,
    files: BTreeSet<String>,
    writable: bool,
    credentials: Vec<(&'static str, Credentials)>,
    google: bool,
    mail: BTreeSet<String>,
    presets: BTreeSet<String>,
    names: BTreeSet<String>,
    unknown_metrics: BTreeSet<String>,
}

impl TestEnv {
    /// Alles vorhanden, was die Fixture-Aufgabe braucht.
    fn complete() -> TestEnv {
        TestEnv {
            now: Timestamp::from_db("2026-10-20T12:00:00.000Z").unwrap(),
            zone: EuZone::central_europe(),
            configs: ["shop".to_string()].into(),
            auth: ["shop-login".to_string()].into(),
            files: BTreeSet::new(),
            writable: true,
            credentials: vec![("gsc", Credentials::Ready), ("psi", Credentials::Ready)],
            google: true,
            mail: ["work".to_string()].into(),
            presets: ["weekly-report".to_string()].into(),
            names: BTreeSet::new(),
            unknown_metrics: BTreeSet::new(),
        }
    }
}

impl Env for TestEnv {
    fn now(&self) -> Timestamp {
        self.now
    }
    fn zone(&self) -> &dyn Zone {
        &self.zone
    }
    fn config_exists(&self, r: &str) -> bool {
        self.configs.contains(r)
    }
    fn auth_profile_exists(&self, r: &str) -> bool {
        self.auth.contains(r)
    }
    fn file_exists(&self, p: &str) -> bool {
        self.files.contains(p)
    }
    fn dir_writable(&self, _: &Path) -> bool {
        self.writable
    }
    fn default_export_dir(&self) -> PathBuf {
        PathBuf::from("exports")
    }
    fn credentials(&self, api: &str) -> Credentials {
        self.credentials
            .iter()
            .find(|(k, _)| *k == api)
            .map(|(_, c)| *c)
            .unwrap_or(Credentials::Missing)
    }
    fn google_account_ready(&self, _: &str) -> bool {
        self.google
    }
    fn mail_account_exists(&self, id: &str) -> bool {
        self.mail.contains(id)
    }
    fn metric_known(&self, name: &str) -> Option<bool> {
        Some(!self.unknown_metrics.contains(name))
    }
    fn preset_exists(&self, name: &str) -> bool {
        self.presets.contains(name)
    }
    fn name_taken(&self, name: &str, _: Option<&str>) -> bool {
        self.names.contains(name)
    }
}

fn keys(issues: &[Issue]) -> Vec<&str> {
    issues.iter().map(|i| i.key.as_str()).collect()
}

// spec: OUT-160
// spec: OUT-161
// spec: OUT-162
// spec: OUT-164
// spec: OUT-165
#[test]
fn the_fixture_task_is_read_completely_and_survives_a_save_roundtrip() {
    let task = nightly();
    assert_eq!(task.id, "nightly");
    assert_eq!(task.project_name(), "Shop");
    assert_eq!(
        task.schedule,
        Schedule::Weekly {
            days: vec![1, 3, 5],
            time: "02:30".into(),
            from: None
        }
    );
    assert_eq!(
        task.crawl.source,
        Source::Spider {
            url: "https://example.test/".into()
        }
    );
    assert_eq!(
        task.crawl
            .overrides
            .get("speed.threads")
            .map(String::as_str),
        Some("10")
    );
    assert_eq!(task.apis, ["gsc", "psi"]);
    assert_eq!(task.execution.timeout_min, Some(120));
    assert!(task.execution.auto_compare);
    let exports = task.effective_exports();
    assert_eq!(
        exports.items,
        [
            ("tab".to_string(), "Internal:All".to_string()),
            ("report".to_string(), "Crawl Overview".to_string())
        ]
    );
    assert_eq!(exports.format, ExportFormat::Xlsx);
    assert!(exports.single_workbook && exports.skip_empty && exports.sitemap_pages);
    assert!(!exports.sitemap_images);

    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::in_settings(dir.path());
    let saved = store.save(task.clone()).unwrap();
    assert_eq!(saved, task);
    assert_eq!(store.get("nightly").unwrap(), task);
    assert_eq!(store.list(), vec![task]);
}

// spec: OUT-160
#[test]
fn names_are_unique_ids_are_safe_and_delete_removes_only_the_task_file() {
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::in_settings(dir.path());
    let mut first = nightly();
    first.id = String::new();
    let first = store.save(first).unwrap();
    assert!(valid_id(&first.id), "{}", first.id);
    assert!(first.id.starts_with("nightly-"));

    let mut same_name = nightly();
    same_name.id = String::new();
    same_name.name = "NIGHTLY".into();
    assert!(matches!(
        store.save(same_name),
        Err(StoreError::NameTaken(_))
    ));
    let mut empty = nightly();
    empty.name = "  ".into();
    assert!(matches!(store.save(empty), Err(StoreError::NameEmpty)));

    // Pfadausbrüche sind keine Kennungen.
    for bad in ["../x", "a/b", "A", "", "x y", &"a".repeat(65)] {
        assert!(!valid_id(bad), "{bad}");
        assert!(
            matches!(store.get(bad), Err(StoreError::InvalidId(_))),
            "{bad}"
        );
    }
    assert!(matches!(store.get("unknown"), Err(StoreError::NotFound(_))));

    // Löschen entfernt nur die Aufgabendatei; ein erzeugter Export bleibt.
    let export = dir.path().join("exports").join("internal_all.csv");
    std::fs::create_dir_all(export.parent().unwrap()).unwrap();
    std::fs::write(&export, "x").unwrap();
    store.delete(&first.id).unwrap();
    assert!(store.list().is_empty());
    assert!(export.is_file());
    assert!(matches!(
        store.delete(&first.id),
        Err(StoreError::NotFound(_))
    ));
}

// spec: OUT-161
#[test]
fn folder_names_are_sanitized_but_the_display_name_stays() {
    assert_eq!(folder_name("Shop: Nightly/Crawl?"), "Shop_ Nightly_Crawl_");
    assert_eq!(folder_name("con"), "_con");
    assert_eq!(folder_name("Lpt1.txt"), "_Lpt1.txt");
    assert_eq!(folder_name("  Ende. "), "Ende");
    assert_eq!(folder_name("???"), "___");
    assert_eq!(folder_name(""), "task");
    assert_eq!(folder_name(&"ä".repeat(200)).chars().count(), 80);
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::in_settings(dir.path());
    let mut task = nightly();
    task.name = "Shop: Nightly/Crawl?".into();
    let saved = store.save(task).unwrap();
    assert_eq!(saved.name, "Shop: Nightly/Crawl?");
    assert_eq!(folder_name(&saved.name), "Shop_ Nightly_Crawl_");
}

// spec: OUT-168
#[test]
fn a_complete_environment_makes_the_fixture_task_valid() {
    let issues = validate::validate(&nightly(), &TestEnv::complete());
    assert!(issues.is_empty(), "{issues:?}");
}

// spec: OUT-168
// spec: OUT-162
// spec: OUT-164
#[test]
fn every_reason_of_the_spec_is_reported_with_its_field() {
    let mut env = TestEnv::complete();
    env.configs.clear();
    env.auth.clear();
    env.writable = false;
    env.credentials = vec![("gsc", Credentials::Missing), ("psi", Credentials::Expired)];
    env.google = false;
    env.mail.clear();
    let issues = validate::validate(&nightly(), &env);
    let found = keys(&issues);
    for expected in [
        "tasks-issue-config-missing",
        "tasks-issue-auth-missing",
        "tasks-issue-output-not-writable",
        "tasks-issue-api-credentials",
        "tasks-issue-api-expired",
        "tasks-issue-google-account",
        "tasks-issue-mail-account-missing",
    ] {
        assert!(found.contains(&expected), "{expected} fehlt in {found:?}");
    }
    let api = issues
        .iter()
        .find(|i| i.key == "tasks-issue-api-credentials")
        .unwrap();
    assert_eq!(api.args.get("api").map(String::as_str), Some("gsc"));
    assert!(issues.iter().all(|i| !i.warning));
    assert_eq!(validate::errors(&issues).len(), issues.len());
}

// spec: OUT-168
// spec: OUT-162
#[test]
fn start_url_list_file_preset_and_name_are_checked() {
    let env = TestEnv::complete();
    let mut task = nightly();
    task.crawl.source = Source::Spider {
        url: "not a url".into(),
    };
    assert_eq!(
        keys(&validate::validate(&task, &env)),
        ["tasks-issue-start-url-invalid"]
    );
    task.crawl.source = Source::Spider {
        url: "example.test/start".into(),
    };
    assert!(validate::validate(&task, &env).is_empty());

    task.crawl.source = Source::List {
        path: "missing.txt\nhttps://example.test/sitemap_index.xml".into(),
    };
    let issues = validate::validate(&task, &env);
    assert_eq!(keys(&issues), ["tasks-issue-list-file-missing"]);
    assert_eq!(issues[0].args["path"], "missing.txt");

    let mut env = TestEnv::complete();
    env.files.insert("urls.txt".into());
    task.crawl.source = Source::List {
        path: "urls.txt".into(),
    };
    assert!(validate::validate(&task, &env).is_empty());
    task.crawl.source = Source::List {
        path: String::new(),
    };
    assert_eq!(
        keys(&validate::validate(&task, &env)),
        ["tasks-issue-list-source-missing"]
    );

    env.presets.clear();
    env.names.insert("Nightly".into());
    let issues = validate::validate(&nightly(), &env);
    assert!(keys(&issues).contains(&"tasks-issue-preset-missing"));
    assert!(keys(&issues).contains(&"tasks-issue-name-taken"));
}

// spec: OUT-168
// spec: OUT-161
#[test]
fn schedule_errors_use_the_dialog_messages() {
    let env = TestEnv::complete();
    let mut task = nightly();
    task.schedule = Schedule::Once {
        at: "2026-10-19T03:00:00+02:00".into(),
    };
    assert_eq!(
        keys(&validate::validate(&task, &env)),
        ["tasks-once-future"]
    );
    task.schedule = Schedule::Once {
        at: "2026-10-21T03:00:00+02:00".into(),
    };
    assert!(validate::validate(&task, &env).is_empty());
    task.schedule = Schedule::Weekly {
        days: vec![],
        time: "03:00".into(),
        from: None,
    };
    assert_eq!(
        keys(&validate::validate(&task, &env)),
        ["tasks-weekday-required"]
    );
    task.schedule = Schedule::Monthly {
        day: 32,
        time: "03:00".into(),
        from: None,
    };
    assert_eq!(
        keys(&validate::validate(&task, &env)),
        ["tasks-month-day-invalid"]
    );
    task.schedule = Schedule::Daily {
        time: "3am".into(),
        from: None,
    };
    assert_eq!(
        keys(&validate::validate(&task, &env)),
        ["tasks-issue-time-invalid"]
    );
}

// spec: OUT-168
// spec: OUT-211 [P2]
// spec: OUT-210
#[test]
fn unavailable_metrics_warn_but_keep_the_task_valid() {
    let mut env = TestEnv::complete();
    env.unknown_metrics.insert("Internal:HTML".into());
    let issues = validate::validate(&nightly(), &env);
    assert_eq!(keys(&issues), ["tasks-metric-unavailable"]);
    assert!(issues[0].warning);
    assert_eq!(issues[0].args["name"], "Internal:HTML");
    assert!(validate::errors(&issues).is_empty());
}

// spec: OUT-168
#[test]
fn a_missing_registration_is_an_issue_only_for_enabled_tasks() {
    let mut task = nightly();
    assert!(validate::registration_issue(&task, true).is_none());
    assert_eq!(
        validate::registration_issue(&task, false).unwrap().key,
        "tasks-not-registered"
    );
    task.enabled = false;
    assert!(validate::registration_issue(&task, false).is_none());
}

// spec: OUT-164
// spec: OUT-168
#[test]
fn filesystem_environment_reads_credentials_from_the_secret_store() {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecretStore::new());
    secrets.set(
        "api/gsc",
        r#"{"account":"a@example.test","expiresAt":"2999-01-01T00:00:00Z"}"#,
    );
    secrets.set("api/psi", r#"{"expiresAt":"2000-01-01T00:00:00Z"}"#);
    secrets.set("api/ahrefs", "plain-key");
    secrets.set("mail/work", "smtp-password");
    let env = FsEnv::system(dir.path(), secrets, dir.path().join("exports"));
    assert_eq!(env.credentials("gsc"), Credentials::Ready);
    assert_eq!(env.credentials("psi"), Credentials::Expired);
    assert_eq!(env.credentials("ahrefs"), Credentials::Ready);
    assert_eq!(env.credentials("moz"), Credentials::Missing);
    assert!(env.mail_account_exists("work"));
    assert!(!env.mail_account_exists("other"));
    assert!(!env.config_exists("shop"));
    std::fs::create_dir_all(dir.path().join("profiles")).unwrap();
    std::fs::write(dir.path().join("profiles").join("shop.osconfig"), "").unwrap();
    assert!(env.config_exists("shop"));
    assert!(env.dir_writable(&dir.path().join("exports").join("deep").join("er")));
    assert!(env.now().0 > Timestamp::UNIX_EPOCH.0);
}

// spec: OUT-170
#[test]
fn the_exported_file_contains_no_secrets_and_roundtrips_on_another_machine() {
    let mut task = nightly();
    task.crawl.source = Source::Spider {
        url: "https://user:topsecret-passwort@example.test/".into(),
    };
    task.crawl
        .overrides
        .insert("auth.token".into(), "tok_live_0123456789".into());
    task.crawl.overrides.insert(
        "network.proxy_url".into(),
        "http://proxyuser:proxy-geheim@proxy.test:8080".into(),
    );
    let text = bundle::export_text(std::slice::from_ref(&task)).unwrap();
    for secret in [
        "geheim-proxy-kennwort",
        "topsecret-passwort",
        "tok_live_0123456789",
        "proxy-geheim",
        "proxyuser",
    ] {
        assert!(!text.contains(secret), "{secret} im Export:\n{text}");
    }
    assert!(text.contains("speed.threads"));

    // Import auf einem anderen Rechner: gleiche Aufgabe, aber Abhängigkeiten fehlen.
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::in_settings(dir.path());
    let imported = bundle::import_into(&store, &text).unwrap();
    assert_eq!(imported.len(), 1);
    assert_eq!(imported[0].id, "nightly");
    assert_eq!(imported[0].schedule, task.schedule);
    assert_eq!(imported[0].exports, task.exports);
    let mut other = TestEnv::complete();
    other.configs.clear();
    other.auth.clear();
    other.mail.clear();
    other.credentials.clear();
    other.google = false;
    let issues = validate::validate(&imported[0], &other);
    let found = keys(&issues);
    for expected in [
        "tasks-issue-config-missing",
        "tasks-issue-auth-missing",
        "tasks-issue-mail-account-missing",
        "tasks-issue-api-credentials",
        "tasks-issue-google-account",
    ] {
        assert!(found.contains(&expected), "{expected} in {found:?}");
    }

    // Zweiter Import derselben Datei: neue Kennung und Namensanhang statt Überschreiben.
    let again = bundle::import_into(&store, &text).unwrap();
    assert_ne!(again[0].id, "nightly");
    assert_eq!(again[0].name, "Nightly (2)");
    assert_eq!(store.list().len(), 2);
}

// spec: OUT-170
#[test]
fn single_task_files_and_newer_versions_are_handled() {
    let single = std::fs::read_to_string(fixture()).unwrap();
    let tasks = bundle::import_text(&single).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].name, "Nightly");
    let newer = single.replacen("version = 1", "version = 9", 1);
    assert!(bundle::import_text(&newer).is_err());
    assert!(bundle::import_text("kein toml [").is_err());
    assert!(bundle::is_secret_key("proxy.PASSWORD"));
    assert!(!bundle::is_secret_key("speed.threads"));
    assert_eq!(
        bundle::strip_userinfo("https://a:b@h.test/x?y=1"),
        "https://h.test/x?y=1"
    );
    assert_eq!(
        bundle::strip_userinfo("https://h.test/a@b"),
        "https://h.test/a@b"
    );
}

// spec: OUT-160
#[test]
fn unreadable_files_are_reported_but_do_not_hide_the_others() {
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::in_settings(dir.path());
    store.save(nightly()).unwrap();
    std::fs::write(store.dir().join("broken.ostask"), "version = [").unwrap();
    std::fs::write(store.dir().join("Not Valid.ostask"), "x").unwrap();
    let (tasks, errors) = store.list_with_errors();
    assert_eq!(tasks.len(), 1);
    assert_eq!(errors.len(), 1);
    let _clock: &dyn Clock = &os_model::SystemClock;
}
