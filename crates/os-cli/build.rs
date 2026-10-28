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

fn column_labels() {
    use std::{collections::BTreeMap, path::PathBuf};
    let root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../../apps/desktop/src/i18n/canon/i18n");
    let read = |locale: &str| {
        let path = root.join(format!("{locale}.ftl"));
        println!("cargo:rerun-if-changed={}", path.display());
        std::fs::read_to_string(path)
            .expect("UI column translations")
            .lines()
            .filter_map(|line| line.strip_prefix("column-")?.split_once(" = "))
            .map(|(key, value)| {
                assert!(
                    !value.contains('{'),
                    "column translation must be a literal: {key}"
                );
                (key.to_string(), value.to_string())
            })
            .collect::<BTreeMap<_, _>>()
    };
    let en = read("en");
    let de = read("de");
    let mut generated = String::from("const GERMAN: &[(&str, &str)] = &[\n");
    for (key, label) in en {
        let translated = de.get(&key).expect("German UI column translation missing");
        generated.push_str(&format!("({label:?}, {translated:?}),\n"));
    }
    generated.push_str("];\n");
    std::fs::write(
        PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("column_labels.rs"),
        generated,
    )
    .unwrap();
}

fn main() {
    column_labels();
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
