//! Release-script fixtures run on Linux and macOS; real codesign evidence comes from CI.
#![cfg(unix)]

use std::path::PathBuf;
use std::process::Command;

fn fixture_test(name: &str) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let result = Command::new("python3")
        .arg(root.join("tests/release/test_macos_bundle.py"))
        .arg(format!("BundleTests.{name}"))
        .current_dir(root)
        .output()
        .expect("python3 is required for release-script fixtures");
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

// spec: OUT-272 [M3]
#[test]
fn macos_file_associations_and_icon() {
    fixture_test("test_file_associations_and_icon");
}

// spec: OUT-260
// spec: OUT-261
#[test]
fn macos_minimum_platform_and_cli() {
    fixture_test("test_minimum_platform_and_cli");
}

// spec: OUT-265
// spec: OUT-261 [P1]
#[test]
fn macos_every_macho_slice_is_checked() {
    fixture_test(
        "test_every_macho_slice_requires_signature_runtime_timestamp_and_empty_entitlements",
    );
}
