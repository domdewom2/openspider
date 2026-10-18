//! M2-Q01 ist Infrastruktur ohne eigene Spec-IDs. Die Test-Tags in den Fixtures belegen die
//! Zuordnungsregeln aus M2.md B2/C9/K2-58; sie werden nicht als Produktabdeckung gutgeschrieben.

use os_parity::coverage::{run, CoverageArgs, Milestone};
use serde_json::Value;
use std::path::{Path, PathBuf};

fn data() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/coverage_m2")
}
fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let name = e.file_name().to_string_lossy().into_owned();
        if e.file_type().unwrap().is_dir() {
            copy(&e.path(), &to.join(name));
        } else {
            std::fs::copy(
                e.path(),
                to.join(name.strip_suffix(".txt").unwrap_or(&name)),
            )
            .unwrap();
        }
    }
}
struct Fixture {
    dir: tempfile::TempDir,
    args: CoverageArgs,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        copy(&data().join("ws"), dir.path());
        std::fs::copy(
            data().join("packages.json"),
            dir.path().join("docs/plan/M2-packages.json"),
        )
        .unwrap();
        let args = CoverageArgs {
            milestone: Milestone::M2,
            gate: false,
            frontend_junit: vec![data().join("vitest.xml"), data().join("playwright.xml")],
            packages: None,
            root: dir.path().into(),
            test_output: Some(data().join("cargo.txt")),
            junit: None,
            write: Some(dir.path().join("M2-status.json")),
            strict: false,
            parity_report: None,
            evidence: None,
        };
        Self { dir, args }
    }
    fn check(&self) -> (bool, Value) {
        let ok = run(&self.args).unwrap();
        let value = serde_json::from_str(
            &std::fs::read_to_string(self.args.write.as_ref().unwrap()).unwrap(),
        )
        .unwrap();
        (ok, value)
    }
    fn append(&self, file: &str, content: &str) {
        let p = self.dir.path().join(file);
        let mut s = std::fs::read_to_string(&p).unwrap();
        s.push_str(content);
        std::fs::write(p, s).unwrap();
    }
}

#[test]
fn m2_development_reads_rust_vitest_playwright_measurements_and_appendix() {
    let f = Fixture::new();
    let (ok, doc) = f.check();
    assert!(ok, "{doc:#}");
    assert_eq!(doc["summary"]["total"], 9);
    for id in ["CRAWL-001", "UI-001", "UI-002", "UI-003", "PERF-001"] {
        assert_eq!(doc["specs"][id]["status"], "implemented", "{id}");
    }
    assert_eq!(doc["specs"]["CRAWL-002"]["status"], "deferred");
    assert_eq!(doc["specs"]["UI-004"]["status"], "deferred");
    assert_eq!(doc["specs"]["CRAWL-003"]["status"], "open");
    assert_eq!(doc["placeholders"].as_array().unwrap().len(), 1);
    assert_eq!(
        doc["appendix_b"]["OUT-020"]["M2-D12"]["status"],
        "implemented"
    );
    assert_eq!(
        doc["appendix_b"]["OUT-020"]["M2-U10"]["status"],
        "implemented"
    );
    assert_eq!(doc["appendix_b"]["OUT-020"]["M2-U20"]["status"], "open");
    assert_eq!(
        doc["appendix_b"]["OUT-021"]["M2-U10"]["status"],
        "implemented"
    );
    assert!(doc["appendix_b"].get("CRAWL-049").is_none());
    let first = std::fs::read(f.args.write.as_ref().unwrap()).unwrap();
    f.check();
    assert_eq!(
        first,
        std::fs::read(f.args.write.as_ref().unwrap()).unwrap()
    );
}

#[test]
fn gate_requires_all_waves_and_rejects_deferred_and_c9() {
    let mut f = Fixture::new();
    f.args.gate = true;
    let (ok, doc) = f.check();
    assert!(!ok);
    assert_eq!(doc["blocking_appendix_b"], 1);
    for id in ["CRAWL-002", "CRAWL-003", "UI-004", "UI-090"] {
        assert!(doc["blocking_specs"]
            .as_array()
            .unwrap()
            .contains(&Value::from(id)));
    }
    assert_eq!(
        doc["ignored_without_valid_reason"][0]["reason"],
        "wartet auf M2-E03"
    );
}

#[test]
fn appendix_requires_the_assigned_package_and_each_marker() {
    let f = Fixture::new();
    let p = f.dir.path().join("docs/plan/M2-packages.json");
    let mut packages: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    packages[0]["extra_spec_ids"] =
        serde_json::json!(["OUT-020", "OUT-021", "TECH-001", "TECH-002"]);
    std::fs::write(p, packages.to_string()).unwrap();
    let (ok, doc) = f.check();
    assert!(!ok);
    assert_eq!(
        doc["appendix_b"]["OUT-021"]["M2-D12"]["status"], "open",
        "UI-Tests zählen nicht für D12"
    );
    assert_eq!(
        doc["appendix_b"]["TECH-001"]["M2-D12"]["missing_markers"],
        serde_json::json!(["M2"])
    );
    assert_eq!(
        doc["appendix_b"]["TECH-002"]["M2-D12"]["missing_markers"],
        serde_json::json!(["P1"])
    );
}

#[test]
fn untagged_and_misplaced_placeholders_and_old_m2_ignores_are_invalid() {
    let f = Fixture::new();
    f.append(
        "crates/alpha/tests/check.rs",
        r#"
#[test]
#[ignore = "M2: alter Grund"]
fn old() {}
#[test]
#[ignore = "wartet auf M2-E03"]
fn misplaced() {}
#[test]
#[ignore]
fn bare() {}
"#,
    );
    let (ok, doc) = f.check();
    assert!(!ok);
    assert_eq!(
        doc["ignored_without_valid_reason"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn failed_frontend_result_overrides_ok_and_missing_results_stay_open() {
    let mut f = Fixture::new();
    let xml = f.dir.path().join("failed.xml");
    std::fs::write(&xml, r#"<testsuite name="src/features/table/table.test.ts"><testcase name="Tabelle &gt; kopiert" classname="src/features/table/table.test.ts"><failure/></testcase></testsuite>"#).unwrap();
    f.args.frontend_junit.push(xml);
    let (ok, doc) = f.check();
    assert!(!ok);
    assert_eq!(doc["extra"]["OUT-020"]["status"], "open");
    assert_eq!(doc["appendix_b"]["OUT-020"]["M2-U10"]["status"], "open");
    f.args.frontend_junit.clear();
    let (ok, doc) = f.check();
    assert!(!ok);
    assert_eq!(doc["specs"]["UI-001"]["tests"][0]["result"], "no_result");
}

#[test]
fn ambiguous_frontend_titles_never_receive_credit() {
    let f = Fixture::new();
    f.append("apps/desktop/src/features/table/table.test.ts", "\ndescribe('Tabelle', () => {\n// spec: UI-001\ntest('zeigt Zeilen & Größen', () => {});\n});\n");
    let (ok, doc) = f.check();
    assert!(!ok);
    assert_eq!(doc["specs"]["UI-001"]["status"], "open");
    assert!(doc["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s.as_str().unwrap().contains("2 Kandidaten")));
}

#[test]
fn malformed_junit_is_an_error() {
    for broken in [
        "<foo/>",
        "<testsuite><testcase name='x'>",
        "<testsuite><testcase name='x'/></wrong>",
        "<testsuite><!--",
        "<testsuite><![CDATA[",
    ] {
        let mut f = Fixture::new();
        let p = f.dir.path().join("broken.xml");
        std::fs::write(&p, broken).unwrap();
        f.args.frontend_junit = vec![p];
        assert!(run(&f.args).is_err(), "{broken}");
    }
}

#[test]
fn all_new_ignore_prefixes_are_milestone_specific() {
    use os_parity::coverage::{m2, valid_defer_reason};
    for reason in ["M3: x", "M4: x", "M5: x", "P2: x", "Klärung K2-12: x"] {
        assert!(m2::valid_defer_reason(reason), "{reason}");
    }
    for reason in [
        "M2: x",
        "Klärung K-12",
        "Klärung K2-x",
        "Klärung K2-12foo",
        "wartet auf M2-E03",
        "Benchmark: x",
        "",
    ] {
        assert!(!m2::valid_defer_reason(reason), "{reason}");
    }
    assert!(valid_defer_reason("M2: bleibt für M1 gültig"));
    assert!(!valid_defer_reason("M4: M1 unverändert"));
}

#[test]
fn cli_defaults_and_multiple_frontend_reports() {
    let f = Fixture::new();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_os-parity"))
        .args(["coverage", "--milestone", "M2", "--root"])
        .arg(f.dir.path())
        .arg("--test-output")
        .arg(data().join("cargo.txt"))
        .arg("--frontend-junit")
        .arg(data().join("vitest.xml"))
        .arg("--frontend-junit")
        .arg(data().join("playwright.xml"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("Anhang B"));
}

#[test]
fn typescript_regex_literals_do_not_corrupt_groups_or_create_tags() {
    use os_parity::coverage::frontend::scan_source;
    let src = r#"
const pattern = /"([^\"]+)"/g;
const other = /[/*]test('fake', function() {})/;
const escaped = /\/\/ spec: UI-999/;
const division = 12 / 3;
describe('Gruppe', () => {
  const regex = /[{}'"`]/;
  // spec: UI-001 [P1]
  it('danach', () => {});
});
// spec: UI-002
it('außerhalb', () => {});
"#;
    let scan = scan_source("apps/desktop/src/sample.test.ts", src);
    assert_eq!(scan.tests.len(), 2);
    assert_eq!(scan.tests[0].name, "Gruppe > danach");
    assert_eq!(scan.tests[0].ids, ["UI-001"]);
    assert_eq!(scan.tests[1].name, "außerhalb");
}

#[test]
fn repository_m2_report_includes_all_586_ids_including_spikes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let args = CoverageArgs {
        milestone: Milestone::M2,
        gate: false,
        frontend_junit: vec![],
        packages: None,
        root: root.into(),
        test_output: None,
        junit: None,
        write: Some(dir.path().join("status.json")),
        strict: false,
        parity_report: None,
        evidence: None,
    };
    assert!(!run(&args).unwrap());
    let doc: Value =
        serde_json::from_str(&std::fs::read_to_string(args.write.unwrap()).unwrap()).unwrap();
    assert_eq!(doc["summary"]["total"], 586);
    for id in ["UI-007", "UI-012", "UI-140", "PERF-001"] {
        assert!(doc["specs"].get(id).is_some(), "{id}");
    }
}

#[test]
fn junit_numeric_entities_and_windows_paths_match_static_titles() {
    let mut f = Fixture::new();
    let xml = std::fs::read_to_string(data().join("vitest.xml"))
        .unwrap()
        .replace(
            "src/features/table/table.test.ts",
            "src\\features\\table\\table.test.ts",
        )
        .replace("Größen", "Gr&#246;&#xDF;en");
    let p = f.dir.path().join("windows.xml");
    std::fs::write(&p, xml).unwrap();
    f.args.frontend_junit[0] = p;
    let (ok, doc) = f.check();
    assert!(ok, "{doc:#}");
    assert_eq!(doc["specs"]["UI-001"]["status"], "implemented");
}

#[test]
fn a_complete_gate_can_pass_with_all_appendix_packages() {
    let mut f = Fixture::new();
    f.args.gate = true;
    f.args.strict = true;
    for file in [
        "crates/alpha/tests/check.rs",
        "crates/alpha/tests/pending_w1_d12.rs",
    ] {
        let p = f.dir.path().join(file);
        let src = std::fs::read_to_string(&p)
            .unwrap()
            .lines()
            .filter(|l| !l.starts_with("#[ignore"))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(p, src).unwrap();
    }
    let p = f.dir.path().join("apps/desktop/e2e/table/table.spec.ts");
    let src = std::fs::read_to_string(&p).unwrap().replace(
        "test.fixme(process.env.SKIP_BROWSER === '1', 'M4: Browserfunktion');",
        "",
    );
    std::fs::write(p, src).unwrap();
    let cargo = std::fs::read_to_string(data().join("cargo.txt"))
        .unwrap()
        .replace("ignored, M3: spätere Funktion", "ok")
        .replace("ignored, wartet auf M2-E03", "ok");
    let p = f.dir.path().join("cargo.txt");
    std::fs::write(&p, cargo).unwrap();
    f.args.test_output = Some(p);
    let xml = std::fs::read_to_string(data().join("playwright.xml"))
        .unwrap()
        .replace("<skipped message=\"M4: Browserfunktion\"/>", "");
    let p = f.dir.path().join("playwright.xml");
    std::fs::write(&p, xml).unwrap();
    f.args.frontend_junit[1] = p;
    let p = f.dir.path().join("later.xml");
    std::fs::write(&p, r#"<testsuite name="src/features/later/later.test.ts"><testcase name="zukünftig"/><testcase name="später kopieren"/></testsuite>"#).unwrap();
    f.args.frontend_junit.push(p);
    let (ok, doc) = f.check();
    assert!(ok, "{doc:#}");
    assert_eq!(doc["summary"]["implemented"], 9);
    assert_eq!(doc["summary"]["open"], 0);
    assert_eq!(doc["blocking_appendix_b"], 0);
}

#[test]
fn c9_rejects_a_placeholder_in_a_crate_without_owned_tests() {
    let f = Fixture::new();
    let p = f.dir.path().join("docs/plan/M2-packages.json");
    let mut packages: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    packages[0]["owns"] = serde_json::json!(["crates/*/tests/pending_w1_d12.rs"]);
    std::fs::write(p, packages.to_string()).unwrap();
    let (ok, doc) = f.check();
    assert!(!ok);
    assert_eq!(doc["placeholders"], serde_json::json!([]));
    assert_eq!(
        doc["ignored_without_valid_reason"][0]["reason"],
        "wartet auf M2-E03"
    );
}

#[test]
fn c9_appendix_placeholders_warn_in_development_but_block_the_gate() {
    let mut f = Fixture::new();
    f.append(
        "crates/alpha/tests/pending_w1_d12.rs",
        r#"
// spec: TECH-001 [M2] [P1]
#[test]
#[ignore = "wartet auf M2-E03"]
fn pending_criterion() { assert_eq!(2+2,4); }
"#,
    );
    let p = f.dir.path().join("docs/plan/M2-packages.json");
    let mut packages: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    packages[0]["extra_spec_ids"] = serde_json::json!(["OUT-020", "TECH-001"]);
    std::fs::write(p, packages.to_string()).unwrap();
    let output = std::fs::read_to_string(data().join("cargo.txt")).unwrap()
        + "test pending_criterion ... ignored, wartet auf M2-E03\n";
    let p = f.dir.path().join("cargo.txt");
    std::fs::write(&p, output).unwrap();
    f.args.test_output = Some(p);
    let (ok, doc) = f.check();
    assert!(ok, "{doc:#}");
    assert_eq!(doc["appendix_b"]["TECH-001"]["M2-D12"]["status"], "open");
    f.args.gate = true;
    let (ok, doc) = f.check();
    assert!(!ok);
    assert_eq!(doc["blocking_appendix_b"], 2);
}

#[test]
fn c9_recognizes_crate_wide_test_ownership() {
    let f = Fixture::new();
    let p = f.dir.path().join("docs/plan/M2-packages.json");
    let mut packages: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    packages[0]["owns"] = serde_json::json!(["crates/alpha/** (ohne Cargo.toml)"]);
    std::fs::write(p, packages.to_string()).unwrap();
    let (ok, doc) = f.check();
    assert!(ok, "{doc:#}");
    assert_eq!(doc["placeholders"].as_array().unwrap().len(), 1);
}

#[test]
fn template_titles_group_cases_and_fail_if_any_instance_fails() {
    let mut f = Fixture::new();
    f.append(
        "apps/desktop/src/features/table/table.test.ts",
        r#"
describe(`Theme ${theme}`, () => {
  // spec: UI-001
  test(`Kontrast ${size}`, () => {});
});
"#,
    );
    let p = f.dir.path().join("templates.xml");
    std::fs::write(&p, r#"<testsuite name="src/features/table/table.test.ts"><testcase name="Theme light &gt; Kontrast small"/><testcase name="Theme dark &gt; Kontrast large"/></testsuite>"#).unwrap();
    f.args.frontend_junit.push(p.clone());
    let (ok, doc) = f.check();
    assert!(ok, "{doc:#}");
    assert_eq!(doc["specs"]["UI-001"]["tests"][1]["result"], "ok");
    std::fs::write(&p, r#"<testsuite name="src/features/table/table.test.ts"><testcase name="Theme light &gt; Kontrast small"/><testcase name="Theme dark &gt; Kontrast large"><failure/></testcase></testsuite>"#).unwrap();
    let (ok, doc) = f.check();
    assert!(!ok);
    assert_eq!(doc["specs"]["UI-001"]["status"], "open");
    assert_eq!(doc["specs"]["UI-001"]["tests"][1]["result"], "failed");
}

#[test]
fn overlapping_template_patterns_are_ambiguous() {
    let mut f = Fixture::new();
    f.append(
        "apps/desktop/src/features/table/table.test.ts",
        r#"
// spec: UI-001
test(`Theme ${theme}`, () => {});
// spec: UI-002
test(`Theme ${other}`, () => {});
"#,
    );
    let p = f.dir.path().join("ambiguous.xml");
    std::fs::write(&p, r#"<testsuite name="src/features/table/table.test.ts"><testcase name="Theme light"/></testsuite>"#).unwrap();
    f.args.frontend_junit.push(p);
    let (_, doc) = f.check();
    for id in ["UI-001", "UI-002"] {
        assert_eq!(doc["specs"][id]["tests"][1]["result"], "no_result");
    }
    assert!(doc["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|w| w.as_str().unwrap().contains("2 Kandidaten")));
}
