//! Mobile-Audits aus Lighthouse, identische Score-Umrechnung für beide Quellen (INTEG-140).
use super::lighthouse::{Cell, Outcome};
use os_model::CrawlConfig;
use serde_json::Value;

/// Spalte und Audit-ID in Tab-Reihenfolge (TECH-231).
pub const AUDITS: [(&str, &str); 5] = [
    ("Viewport", "viewport"),
    ("Tap Targets", "target-size"),
    ("Content Width", "content-width"),
    ("Font Display", "font-size"),
    ("Plugins", "plugins"),
];
/// Präsenzmarker: ein geliefertes, aber fehlerhaftes/manuelles Plugin-Audit darf keinen DOM-Rückfall auslösen.
pub const PLUGINS_AUDIT: &str = "mobile.plugins_audit";

/// Score 0–1, kaufmännisch gerundet; nicht anwendbar besteht, manuell/fehlerhaft bleibt leer.
pub fn score(audit: &Value) -> Option<u8> {
    match audit.get("scoreDisplayMode").and_then(Value::as_str) {
        Some("notApplicable") => return Some(100),
        Some("manual" | "error" | "informative") => return None,
        _ => {}
    }
    audit
        .get("score")
        .and_then(Value::as_f64)
        .filter(|s| s.is_finite() && (0.0..=1.0).contains(s))
        .map(|s| (s * 100.0).round() as u8)
}

/// Fügt nur Mobile-Werte hinzu. Laufzeitfehler werden vom gemeinsamen Parser vorab behandelt.
pub fn append(lh: &Value, out: &mut Outcome) {
    if !out.is_success() {
        return;
    }
    let audits = &lh["audits"];
    for (column, id) in AUDITS {
        let audit = audits.get(id).or_else(|| {
            (id == "target-size")
                .then(|| audits.get("tap-targets"))
                .flatten()
        });
        if id == "plugins" && audit.is_some() {
            out.cells.push((PLUGINS_AUDIT.into(), Cell::Num(1.0)));
        }
        if let Some(a) = audit {
            if let Some(score) = score(a) {
                out.cells.push((column.into(), Cell::Num(f64::from(score))));
            }
            if let Some(details) = a.get("details") {
                out.cells.push((
                    format!("mobile.details:{id}"),
                    Cell::Text(details.to_string()),
                ));
            }
        }
    }
}

/// Die PSI-API liefert keine Inhaltsbreiten-Prüfung (TECH-234), auch bei unerwarteten Mock-Werten.
pub fn remote(out: &mut Outcome) {
    out.cells
        .retain(|(name, _)| name != "Content Width" && name != "mobile.details:content-width");
}

/// Metrikgruppe Mobile ist gewählt; leere Gruppenliste = alle Gruppen (INTEG-133).
pub fn selected(cfg: &CrawlConfig) -> bool {
    cfg.api.psi.enabled
        && (cfg.api.psi.metric_groups.is_empty()
            || cfg
                .api
                .psi
                .metric_groups
                .iter()
                .any(|g| g.trim().eq_ignore_ascii_case("mobile")))
}

/// Wird am Ausgang der Quelle aufgerufen; Status und übrige PSI-Metriken bleiben unabhängig verfügbar.
pub fn apply_config(out: &mut Outcome, cfg: &CrawlConfig) {
    if !selected(cfg) {
        out.cells.retain(|(name, _)| {
            !AUDITS.iter().any(|(column, _)| name == column) && !name.starts_with("mobile.")
        });
    } else if !cfg.api.psi.local {
        remote(out);
    }
}

/// Mobile benötigt die SEO- und Accessibility-Audits zusätzlich zu Performance.
pub fn request_categories(mut url: url::Url, cfg: &CrawlConfig) -> url::Url {
    if selected(cfg) {
        url.query_pairs_mut()
            .append_pair("category", "seo")
            .append_pair("category", "accessibility");
    }
    url
}

#[cfg(test)]
mod tests {
    use super::super::{
        lighthouse::{parse_lighthouse, parse_psi},
        local::{Job, Launcher, LocalOptions, LocalRunner, NodeLighthouse},
        remote, PsiSource,
    };
    use super::*;
    use serde_json::json;
    use std::{
        path::Path,
        process::Command,
        sync::{Arc, Mutex},
    };

    fn fixture(name: &str) -> Value {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/tech-m2-mobile")
            .join(name);
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }
    fn config() -> CrawlConfig {
        let mut cfg = CrawlConfig::default();
        cfg.api.psi.enabled = true;
        cfg
    }

    // spec: INTEG-140
    #[test]
    fn score_conversion_preserves_missing_manual_error_na_and_legacy_precedence() {
        assert_eq!(score(&json!({"score":0.5})), Some(50));
        assert_eq!(score(&json!({"score":0.125})), Some(13));
        for mode in ["manual", "error", "informative"] {
            assert_eq!(score(&json!({"score":0,"scoreDisplayMode":mode})), None);
        }
        assert_eq!(
            score(&json!({"score":null,"scoreDisplayMode":"notApplicable"})),
            Some(100)
        );
        for bad in [
            json!(null),
            json!({}),
            json!({"score":-1}),
            json!({"score":2}),
            json!({"score":"0"}),
        ] {
            assert_eq!(score(&bad), None);
        }
        let legacy = fixture("psi-mobile-legacy.json");
        assert_eq!(
            parse_psi(&legacy).get("Tap Targets"),
            Some(&Cell::Num(50.0))
        );
        let mut both = legacy;
        both["lighthouseResult"]["audits"]["target-size"] = json!({"score":1});
        assert_eq!(parse_psi(&both).get("Tap Targets"), Some(&Cell::Num(100.0)));
        both["lighthouseResult"]["audits"]["target-size"] =
            json!({"score":null,"scoreDisplayMode":"manual"});
        assert_eq!(parse_psi(&both).get("Tap Targets"), None);
    }

    // spec: TECH-230
    // spec: INTEG-140
    #[test]
    fn identical_local_and_remote_reports_match_mobile_golden_scores() {
        let remote = fixture("psi-mobile-remote.json");
        let local = parse_lighthouse(&remote["lighthouseResult"]);
        let remote = parse_psi(&remote);
        for (name, expected) in [("Viewport", 0.0), ("Tap Targets", 50.0), ("Plugins", 0.0)] {
            assert_eq!(local.get(name), Some(&Cell::Num(expected)));
            assert_eq!(local.get(name), remote.get(name));
        }
        assert!(remote.get("Content Width").is_none());
        assert!(remote.get("Font Display").is_none());
        let all = fixture("lighthouse-cases.json");
        let expected = fixture("expected.json");
        for (page, report) in all.as_object().unwrap() {
            let out = parse_lighthouse(report);
            for (name, _) in AUDITS {
                if name != "Plugins" {
                    assert_eq!(
                        out.get(name),
                        Some(&Cell::Num(expected[page][name].as_f64().unwrap())),
                        "{page} {name}"
                    );
                }
            }
        }
    }

    // spec: TECH-233
    // spec: TECH-235
    #[test]
    fn mobile_audit_details_preserve_selectors_geometry_and_font_percentages() {
        let all = fixture("lighthouse-cases.json");
        for (page, metric, selector) in [
            (
                "partial-target.html",
                "mobile.details:target-size",
                "#small",
            ),
            ("small-font-41.html", "mobile.details:font-size", "span"),
        ] {
            let out = parse_lighthouse(&all[page]);
            let Some(Cell::Text(raw)) = out.get(metric) else {
                panic!("details missing")
            };
            let parsed: Value = serde_json::from_str(raw).unwrap();
            assert_eq!(parsed["items"][0]["node"]["selector"], selector);
            assert_eq!(
                parsed,
                all[page]["audits"][metric.strip_prefix("mobile.details:").unwrap()]["details"]
            );
        }
    }

    // spec: INTEG-146
    #[test]
    fn delivered_plugin_audits_including_manual_errors_do_not_request_dom_fallback() {
        for mode in ["manual", "error", "notApplicable", "binary"] {
            let out = parse_lighthouse(
                &json!({"audits":{"plugins":{"scoreDisplayMode":mode,"score":null}}}),
            );
            assert_eq!(out.get(PLUGINS_AUDIT), Some(&Cell::Num(1.0)));
            assert_eq!(
                out.get("Plugins"),
                if mode == "notApplicable" {
                    Some(&Cell::Num(100.0))
                } else {
                    None
                }
            );
        }
        assert!(parse_lighthouse(&json!({"audits":{}}))
            .get(PLUGINS_AUDIT)
            .is_none());
    }

    // spec: INTEG-140
    #[tokio::test]
    async fn remote_fetch_uses_mock_response_and_only_requests_extra_categories_for_mobile() {
        use crate::{
            credentials::{Credential, Credentials},
            ApiCtx,
        };
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/tech-m2-mobile");
        let site = os_testsite::TestSite::start_dir(&dir, 0).await.unwrap();
        let secrets = Arc::new(os_fetch::MemorySecretStore::new());
        Credentials::new(secrets.clone())
            .put(
                os_model::ApiKind::Psi,
                Credential {
                    account_id: "mobile-fixture".into(),
                    label: "Fixture".into(),
                    api_key: Some("fixture-key".into()),
                    access_token: None,
                    refresh_token: None,
                    expires_at: None,
                    client_secret: None,
                    scopes: vec![],
                },
                true,
            )
            .unwrap();
        let mut c = config();
        c.api.psi.base_url = Some(site.base_url());
        let mut ctx = ApiCtx {
            cfg: c,
            secrets,
            network: Default::default(),
            allow_api_override: false,
        };
        let source = PsiSource::new();
        let out = source
            .fetch_outcome("https://site.test/page", &ctx)
            .await
            .unwrap();
        assert_eq!(out.get("Tap Targets"), Some(&Cell::Num(50.0)));
        let request = site.requests().last().unwrap().path_and_query.clone();
        let request = url::Url::parse(&format!("http://localhost{request}")).unwrap();
        assert_eq!(
            request
                .query_pairs()
                .filter(|(k, _)| k == "category")
                .map(|(_, v)| v.to_string())
                .collect::<Vec<_>>(),
            ["performance", "seo", "accessibility"]
        );
        ctx.cfg.api.psi.metric_groups = vec!["lighthouse".into()];
        let out = source
            .fetch_outcome("https://site.test/page", &ctx)
            .await
            .unwrap();
        assert!(out.get("Tap Targets").is_none());
        let request = site.requests().last().unwrap().path_and_query.clone();
        assert!(!request.contains("category=seo") && !request.contains("category=accessibility"));
        let base = remote::request_url(
            "http://localhost",
            "https://site.test/",
            "fixture-key",
            os_model::PsiDevice::Mobile,
        )
        .unwrap();
        assert_eq!(request_categories(base.clone(), &ctx.cfg), base);
    }

    /// Testbinary itself writes the fixture LHR, so local tests need neither a shell nor Node/Chrome.
    // spec: TECH-230
    #[test]
    fn fake_mobile_lighthouse() {
        let Ok(output) = std::env::var("D23_LIGHTHOUSE_OUTPUT") else {
            return;
        };
        std::fs::write(
            output,
            fixture("lighthouse-cases.json")["wide-content.html"].to_string(),
        )
        .unwrap();
    }
    struct FakeLocal(Mutex<Vec<bool>>);
    impl Launcher for FakeLocal {
        fn command(&self, job: &Job) -> Command {
            self.0.lock().unwrap().push(false);
            let mut cmd = Command::new(std::env::current_exe().unwrap());
            cmd.args([
                "psi::mobile::tests::fake_mobile_lighthouse",
                "--exact",
                "--nocapture",
            ])
            .env("D23_LIGHTHOUSE_OUTPUT", &job.output);
            cmd
        }
        fn mobile_command(&self, job: &Job) -> Command {
            let cmd = self.command(job);
            *self.0.lock().unwrap().last_mut().unwrap() = true;
            cmd
        }
        fn version_command(&self) -> Command {
            Command::new(std::env::current_exe().unwrap())
        }
    }
    // spec: TECH-230
    // spec: INTEG-140
    #[tokio::test]
    async fn local_mobile_path_works_in_text_mode_without_key_and_group_off_keeps_normal_run() {
        let fake = Arc::new(FakeLocal(Mutex::new(Vec::new())));
        let runner = Arc::new(LocalRunner::new(fake.clone(), &LocalOptions::default()));
        let mut c = config();
        c.api.psi.local = true;
        assert_eq!(c.rendering.mode, os_model::RenderingMode::Text);
        let mut ctx = crate::ApiCtx {
            cfg: c,
            secrets: Arc::new(os_fetch::MemorySecretStore::new()),
            network: Default::default(),
            allow_api_override: false,
        };
        let source = PsiSource::new().with_local(runner);
        let out = source
            .fetch_outcome("https://site.test/wide-content.html", &ctx)
            .await
            .unwrap();
        assert!(out.is_success());
        assert_eq!(out.get("Content Width"), Some(&Cell::Num(0.0)));
        ctx.cfg.api.psi.metric_groups = vec!["lighthouse".into()];
        let out = source
            .fetch_outcome("https://site.test/wide-content.html", &ctx)
            .await
            .unwrap();
        assert!(out.is_success());
        assert!(out.get("Content Width").is_none());
        assert_eq!(*fake.0.lock().unwrap(), [true, false]);
        let node = NodeLighthouse {
            program: "node".into(),
            prefix: vec!["lighthouse.js".into()],
        };
        let job = Job {
            url: "https://site.test/".into(),
            device: os_model::PsiDevice::Mobile,
            headers_file: None,
            output: "report.json".into(),
        };
        let mobile = node.mobile_command(&job);
        let args: Vec<_> = mobile
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(args.contains(&"--only-categories=performance,seo,accessibility".into()));
        assert!(!args.contains(&"--only-categories=performance".into()));
    }

    // spec: TECH-230
    #[test]
    fn runtime_errors_and_group_selection_clear_mobile_values_without_changing_other_metrics() {
        let mut lh = fixture("lighthouse-cases.json")["ok.html"].clone();
        for code in ["ERRORED_DOCUMENT_REQUEST", "Timeout", "LIGHTHOUSE_CRASHED"] {
            lh["runtimeError"] = json!({"code":code,"message":"fixture failure"});
            let out = parse_lighthouse(&lh);
            assert!(!out.is_success());
            for (name, _) in AUDITS {
                assert!(out.get(name).is_none());
            }
        }
        let mut out = parse_lighthouse(&fixture("lighthouse-cases.json")["wide-content.html"]);
        let mut c = config();
        c.api.psi.metric_groups = vec!["lighthouse".into()];
        apply_config(&mut out, &c);
        assert!(out.is_success());
        assert!(out.cells.iter().all(
            |(name, _)| !AUDITS.iter().any(|(n, _)| name == n) && !name.starts_with("mobile.")
        ));
    }
}
