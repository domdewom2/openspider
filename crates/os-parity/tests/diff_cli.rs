//! Tests des Binärs `os-parity diff`: Exit-Codes und Berichtsdateien.

use std::path::Path;
use std::process::Command;

const MAPPING: &str = "key = [\"Address\"]\ndrop_sf_columns = [\"Description\"]\n[columns]\n\"Address\" = \"Address\"\n\"Status Code\" = \"Status Code\"\n";

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_os-parity"))
}

fn setup(sf: &str, os: &str) -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("sf.csv"), sf).unwrap();
    std::fs::write(d.path().join("os.csv"), os).unwrap();
    std::fs::write(d.path().join("internal_all.toml"), MAPPING).unwrap();
    d
}

fn diff(d: &Path, extra: &[&str]) -> std::process::Output {
    bin()
        .args(["diff", "--sf"])
        .arg(d.join("sf.csv"))
        .arg("--os")
        .arg(d.join("os.csv"))
        .arg("--mapping")
        .arg(d.join("internal_all.toml"))
        .args([
            "--sf-base",
            "http://127.0.0.1:8902/",
            "--os-base",
            "http://127.0.0.1:5555",
        ])
        .args(extra)
        .output()
        .unwrap()
}

#[test]
fn identical_exports_exit_zero_and_write_reports() {
    let d = setup(
        "Address,Status Code,Description\nhttp://127.0.0.1:8902/,200,Text\n",
        "Address,Status Code\nhttp://127.0.0.1:5555/,200\n",
    );
    let out = d.path().join("rep");
    let r = diff(d.path(), &["--out", out.to_str().unwrap()]);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("report.json")).unwrap()).unwrap();
    assert_eq!(json["export"], "internal_all");
    assert_eq!(json["matched"], 1);
    assert_eq!(json["passed"], true);
    assert!(out.join("report.md").is_file());
    assert!(String::from_utf8_lossy(&r.stdout).contains("bestanden"));
}

#[test]
fn differences_below_threshold_exit_one() {
    let d = setup(
        "Address,Status Code\nhttp://127.0.0.1:8902/,200\nhttp://127.0.0.1:8902/x,404\n",
        "Address,Status Code\nhttp://127.0.0.1:5555/,200\nhttp://127.0.0.1:5555/x,200\n",
    );
    let r = diff(d.path(), &[]);
    assert_eq!(r.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&r.stdout).contains("nicht bestanden"));
}

#[test]
fn thresholds_and_deviations_files_can_be_passed() {
    let d = setup(
        "Address,Status Code\nhttp://127.0.0.1:8902/,200\nhttp://127.0.0.1:8902/x,404\n",
        "Address,Status Code\nhttp://127.0.0.1:5555/,200\nhttp://127.0.0.1:5555/x,200\n",
    );
    std::fs::write(
        d.path().join("t.toml"),
        "default_agreement_pct = 50.0\n[agreement_pct]\n\"internal_*\" = 50.0\n",
    )
    .unwrap();
    let r = diff(
        d.path(),
        &["--thresholds", d.path().join("t.toml").to_str().unwrap()],
    );
    assert!(r.status.success());

    std::fs::write(
        d.path().join("d.toml"),
        "[[deviation]]\nspec=\"CRAWL-001\"\nkind=\"Annahme\"\ncolumn=\"Status Code\"\nkey=\"{BASE}/x\"\n",
    )
    .unwrap();
    let r = diff(
        d.path(),
        &["--deviations", d.path().join("d.toml").to_str().unwrap()],
    );
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stdout));
}

#[test]
fn unreadable_input_exits_two() {
    let d = setup("Address,Status Code\n", "Address,Status Code\n");
    std::fs::remove_file(d.path().join("os.csv")).unwrap();
    let r = diff(d.path(), &[]);
    assert_eq!(r.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&r.stderr).contains("os-parity"));
}
