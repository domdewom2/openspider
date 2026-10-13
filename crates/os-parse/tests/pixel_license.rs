//! UI-038: Die Lizenzangaben im Repository passen zu den tatsächlich eingecheckten Schriftdateien. Die name-Tabelle
//! der Schrift (Version, Copyright, Lizenzhinweis) ist maßgeblich; NOTICE und der Lizenztext unter
//! `resources/fonts/` dürfen nichts anderes behaupten (Liberation 1.x ist GPLv2 mit Schriftarten-Ausnahme,
//! erst Liberation 2.x und Arimo stehen unter der SIL OFL).

use std::path::PathBuf;

use ttf_parser::Face;

fn crate_file(rel: &str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for part in rel.split('/') {
        path.push(part);
    }
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn name_entry(file: &str, id: u16) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("resources");
    path.push("fonts");
    path.push(file);
    let data = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let face = Face::parse(&data, 0).unwrap();
    face.names()
        .into_iter()
        .filter(|n| n.name_id == id)
        .find_map(|n| n.to_string())
        .unwrap_or_else(|| panic!("{file}: name-Eintrag {id} fehlt"))
}

fn notice() -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("..");
    path.push("..");
    path.push("NOTICE");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

const FILES: [&str; 2] = ["LiberationSans-Regular.ttf", "LiberationSans-Bold.ttf"];

// spec: UI-038
#[test]
fn notice_names_the_version_and_copyright_of_the_font_files() {
    let notice = notice();
    for file in FILES {
        let version = name_entry(file, 5);
        assert!(
            notice.contains(&version),
            "NOTICE nennt nicht {version:?} ({file})"
        );
        // Erste Copyright-Zeile der Schrift (ohne Vorspann "Copyright (c) "): Jahr und Inhaber.
        let copyright = name_entry(file, 0);
        let holder = copyright
            .split(". All rights")
            .next()
            .unwrap()
            .trim_end_matches('.');
        assert!(
            notice.contains(holder),
            "NOTICE nennt nicht {holder:?} ({file})"
        );
    }
}

// spec: UI-038
#[test]
fn license_claims_match_the_license_in_the_font_name_table() {
    let notice = notice();
    let license_file = crate_file("resources/fonts/LICENSE-LiberationSans.txt");
    for file in FILES {
        let license = name_entry(file, 13);
        let is_ofl = license.contains("SIL Open Font License");
        if is_ofl {
            assert!(
                license_file.contains("SIL OPEN FONT LICENSE Version 1.1"),
                "Lizenzdatei enthält den OFL-Text nicht ({file})"
            );
            assert!(notice.contains("SIL Open Font License"), "NOTICE ({file})");
        } else {
            // Eine nicht-OFL-Schrift (Liberation 1.x): weder NOTICE noch Lizenzdatei dürfen die OFL
            // als geltende Lizenz ausgeben.
            assert!(
                !license_file
                    .contains("This Font Software is licensed under the SIL Open Font License"),
                "Lizenzdatei gibt OFL für {file} an, name-Tabelle sagt: {license}"
            );
            assert!(
                !notice.contains("Lizenz: SIL Open Font License"),
                "NOTICE gibt OFL für {file} an, name-Tabelle sagt: {license}"
            );
            assert!(
                notice.contains("Liberation Fonts license"),
                "NOTICE nennt die Lizenz aus der name-Tabelle nicht ({file})"
            );
        }
    }
}

// spec: UI-038
#[test]
fn generated_metrics_header_carries_the_font_provenance() {
    let metrics = crate_file("src/pixel/metrics.rs");
    for file in FILES {
        assert!(
            metrics.contains(&name_entry(file, 5)),
            "metrics.rs nennt die Version aus {file} nicht"
        );
        assert!(
            metrics.contains(&name_entry(file, 13)),
            "metrics.rs nennt die Lizenz aus {file} nicht"
        );
    }
}
