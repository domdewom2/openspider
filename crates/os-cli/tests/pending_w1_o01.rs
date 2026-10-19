//! OUT-243 criterion 4: full crawl through L01/I0x, enabled by M2-N01 after integration.
mod common;
use common::{site, Env};
use os_fetch::secrets::secret_store_from_env;
use os_model::config::{BasicAuth, HeaderEntry};
use os_store::auth_profiles::{self, Protection};
use serde_json::json;
use std::fs;

const CHILD: &str = "OPENSPIDER_O01_SECRET_CRAWL_CHILD";
const SECRETS: &[(&str, &str)] = &[
    ("Authorization", "AUTH-FIXTURE-243"),
    ("Proxy-Authorization", "PROXY-AUTH-FIXTURE-243"),
    ("Cookie", "COOKIE-FIXTURE-243"),
    ("Set-Cookie", "SET-COOKIE-FIXTURE-243"),
    ("X-Api-Key", "API-KEY-FIXTURE-243"),
    ("X-Access-Token", "OAUTH-FIXTURE-243"),
    ("password", "PASSWORD-FIXTURE-243"),
    ("passphrase", "PASSPHRASE-FIXTURE-243"),
];

// spec: OUT-243
#[test]
#[ignore = "wartet auf M2-L01/I0x: out-schedule, Auth-Profil-Anwendung und Mock-APIs"]
fn crawl_with_all_secret_types_leaves_zero_plaintext_matches_in_all_logs() {
    if let Some(home) = std::env::var_os(CHILD) {
        assert_eq!(std::env::var("OPENSPIDER_SECRET_STORE").unwrap(), "memory");
        let home = std::path::PathBuf::from(home);
        // Memory backend must live in the crawl process; a parent-process store is not inherited.
        let keys = secret_store_from_env();
        let key = auth_profiles::new_key();
        keys.set("auth/support/qa", &key).unwrap();
        keys.set("openspider/proxy/password", "PROXY-PASSWORD-FIXTURE-243")
            .unwrap();
        keys.set("api/psi", SECRETS[4].1).unwrap();
        keys.set("api/gsc/refresh-token", SECRETS[5].1).unwrap();
        keys.set("fixture/basic", SECRETS[6].1).unwrap();
        let profile = auth_profiles::seal(
            &json!({"profile":{"name":"support/qa","kind":"basic","host":"127.0.0.1","username":"fixture-user","secret":SECRETS[6].1},"createdAt":"2026-10-19T00:00:00Z"}),
            Protection::Machine { key_ref: "auth/support/qa", key: &key },
        ).unwrap();
        let profile_path = home.join("fixture.osauth");
        fs::write(&profile_path, profile).unwrap();
        let code = os_cli::main_entry([
            std::ffi::OsString::from("openspider"),
            "--crawl".into(),
            std::env::var_os("OPENSPIDER_O01_CRAWL_URL").unwrap(),
            "--config".into(),
            home.join("fixture.osconfig").into_os_string(),
            "--auth-config".into(),
            profile_path.into_os_string(),
            "--log-level".into(),
            "trace".into(),
        ]);
        assert_eq!(code, 0);
        return;
    }
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let site = site("out-schedule").await;
        let mocks = os_testsite::mocks::MockServer::start_all().await.unwrap();
        assert!(os_testsite::mocks::gsc::mock().is_some(), "M2-I03 mock required");
        assert!(os_testsite::mocks::psi::mock().is_some(), "M2-I04 mock required");
        let env = Env::new();
        let mut config = os_model::CrawlConfig::default();
        config.limits.max_urls = 10;
        config.auth.basic.push(BasicAuth { prefix: site.base_url(), username: "fixture-user".into(), secret_ref: "fixture/basic".into() });
        config.http.headers = SECRETS.iter().map(|(name, value)| HeaderEntry { name: (*name).into(), value: (*value).into(), host: None }).collect();
        config.api.gsc.enabled = true;
        config.api.gsc.site = Some(site.base_url());
        config.api.gsc.base_url = Some(mocks.base_url("gsc"));
        config.api.psi.enabled = true;
        config.api.psi.base_url = Some(mocks.base_url("psi"));
        fs::write(env.home.path().join("fixture.osconfig"), config.to_toml_string().unwrap()).unwrap();
        let mut url = site.base_url();
        url.push_str("?key=QUERY-KEY-FIXTURE-243&api_key=QUERY-API-FIXTURE-243&token=QUERY-TOKEN-FIXTURE-243&access_token=QUERY-OAUTH-FIXTURE-243&password=QUERY-PASSWORD-FIXTURE-243&secret=QUERY-SECRET-FIXTURE-243");
        // URL userinfo is separately represented in the start URL and must be redacted too.
        url = url.replacen("http://", "http://URL-USER-FIXTURE-243:URL-PASSWORD-FIXTURE-243@", 1);
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command.args(["--exact", "crawl_with_all_secret_types_leaves_zero_plaintext_matches_in_all_logs", "--ignored"])
            .env(CHILD, env.home.path())
            .env("OPENSPIDER_HOME", env.home.path())
            .env("OPENSPIDER_SECRET_STORE", "memory")
            .env("OPENSPIDER_O01_CRAWL_URL", url)
            .env_remove("OPENSPIDER_LOG")
            .env_remove("RUST_LOG");
        let output = tokio::task::spawn_blocking(move || command.output().unwrap()).await.unwrap();
        assert!(output.status.success(), "{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        let logs = fs::read_dir(env.home.path().join("logs")).unwrap().map(|entry| fs::read_to_string(entry.unwrap().path()).unwrap()).collect::<String>();
        assert!(logs.contains("Crawl gestartet") && logs.contains("Crawl beendet"));
        assert!(logs.contains("Abruf abgeschlossen"), "nonempty debug coverage required");
        assert!(!logs.contains("FIXTURE-243"), "plaintext secret in crawl logs: {logs}");
        mocks.stop().await;
        drop(site);
    });
}
