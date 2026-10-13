//! Build-Skript: Git-Hash, Betriebssystem und Architektur für `openspider --version` (OUT-181).
//!
//! Ausgabe der Version: `<semver> (<git-hash>, <os>-<arch>)`. Der Hash kommt aus `OPENSPIDER_GIT_HASH`
//! (Release-Builds) oder aus `git rev-parse`; ohne Git steht `unknown` da.

use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=OPENSPIDER_GIT_HASH");

    let hash = std::env::var("OPENSPIDER_GIT_HASH")
        .ok()
        .filter(|h| !h.trim().is_empty())
        .or_else(|| git(&["rev-parse", "--short=10", "HEAD"]))
        .unwrap_or_else(|| "unknown".to_string());

    // Neu bauen, wenn sich HEAD oder der aktuelle Zweig ändert (Worktrees eingeschlossen).
    if let Some(path) = git(&["rev-parse", "--git-path", "HEAD"]) {
        println!("cargo:rerun-if-changed={path}");
    }
    if let Some(reference) = git(&["symbolic-ref", "-q", "HEAD"]) {
        if let Some(path) = git(&["rev-parse", "--git-path", &reference]) {
            println!("cargo:rerun-if-changed={path}");
        }
    }

    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| "unknown".into());
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| "unknown".into());
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();

    println!("cargo:rustc-env=OS_CLI_GIT_HASH={hash}");
    println!("cargo:rustc-env=OS_CLI_VERSION={version} ({hash}, {os}-{arch})");
}
