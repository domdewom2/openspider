//! Snapshots der vier `--print-config`-Ausgaben je Betriebssystem (M1-P18, AGENT-001).
//!
//! Die Ausgabe hängt nur vom Binärpfad und den Argumenten ab; deshalb prüft jeder Lauf beide
//! Betriebssystem-Varianten mit festen, typischen Installationspfaden. Neu erzeugen mit `UPDATE_GOLDEN=1`
//! (erst nach grünen spec-abgeleiteten Tests, Änderung im Commit begründen).

use std::path::PathBuf;

use os_mcp::print_config::{render, Client};

const MACOS_EXE: &str = "/Applications/OpenSpider.app/Contents/MacOS/openspider";
const WINDOWS_EXE: &str = "C:\\Program Files\\OpenSpider\\openspider.exe";

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("golden")
        .join("mcp-print-config")
}

fn check(os: &str, exe: &str) {
    for client in Client::ALL {
        let actual = render(client, exe, &["mcp".to_string()]).unwrap();
        let path = golden_dir()
            .join(os)
            .join(format!("{}.{}", client.name(), client.extension()));
        if std::env::var_os("UPDATE_GOLDEN").is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &actual).unwrap();
        }
        let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "Golden {} fehlt ({e}); UPDATE_GOLDEN=1 erzeugt sie",
                path.display()
            )
        });
        assert_eq!(
            actual,
            expected.replace("\r\n", "\n"),
            "{os}/{}",
            client.name()
        );
    }
}

// spec: AGENT-001
#[test]
fn snapshots_macos() {
    check("macos", MACOS_EXE);
}

// spec: AGENT-001
#[test]
fn snapshots_windows() {
    check("windows", WINDOWS_EXE);
}

// spec: AGENT-001
#[test]
fn snapshots_are_valid_json_or_toml_with_the_exact_path() {
    for (os, exe) in [("macos", MACOS_EXE), ("windows", WINDOWS_EXE)] {
        for client in Client::ALL {
            let path =
                golden_dir()
                    .join(os)
                    .join(format!("{}.{}", client.name(), client.extension()));
            let text = std::fs::read_to_string(&path).unwrap();
            let command = if client == Client::Codex {
                let t: toml::Table = toml::from_str(&text).unwrap();
                t["mcp_servers"]["openspider"]["command"]
                    .as_str()
                    .unwrap()
                    .to_string()
            } else {
                let v: serde_json::Value = serde_json::from_str(&text).unwrap();
                v["mcpServers"]["openspider"]["command"]
                    .as_str()
                    .unwrap()
                    .to_string()
            };
            assert_eq!(command, exe, "{os}/{}", client.name());
        }
    }
}
