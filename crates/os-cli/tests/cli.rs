use std::process::Command;

fn openspider() -> Command {
    Command::new(env!("CARGO_BIN_EXE_openspider"))
}

#[test]
fn version_flag_prints_version() {
    let out = openspider().arg("--version").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.starts_with("openspider "), "unexpected: {stdout}");
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn crawl_stub_succeeds() {
    let out = openspider()
        .args(["crawl", "https://example.com/"])
        .output()
        .unwrap();
    assert!(out.status.success());
}
