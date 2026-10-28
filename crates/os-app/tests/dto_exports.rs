//! TypeScript-Ausgabe der DTOs (ts-rs): die eingecheckten Dateien unter `apps/desktop/src/ipc/gen/` sind genau die
//! Ausgabe von `os_app::dto::write_gen`. `UPDATE_GOLDEN=1` schreibt sie neu (oder `os-app-manifest`).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use os_app::dto;

fn gen_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/desktop/src/ipc/gen")
}

fn read_tree(dir: &Path) -> BTreeMap<String, String> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        for e in rd {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(base, &p, out);
            } else if p.extension().is_some_and(|x| x == "ts") {
                let rel = p
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, fs::read_to_string(&p).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// Erzeugt die Ausgabe in einem frischen Ordner. ts-rs merkt sich je Prozess, welche Pfade es schon geschrieben hat,
/// und schreibt denselben Pfad kein zweites Mal: jeder Aufruf braucht deshalb einen eigenen Ordner.
fn fresh() -> BTreeMap<String, String> {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let tmp = std::env::temp_dir().join(format!("os-app-gen-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    dto::write_gen(&tmp).unwrap();
    let t = read_tree(&tmp);
    let _ = fs::remove_dir_all(&tmp);
    t
}

// spec: UI-140
#[test]
fn committed_ts_files_match_the_generated_output() {
    let generated = fresh();
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        dto::write_gen(&gen_dir()).unwrap();
    }
    let on_disk = read_tree(&gen_dir());
    let missing: Vec<_> = generated
        .keys()
        .filter(|k| !on_disk.contains_key(*k))
        .collect();
    let stale: Vec<_> = on_disk
        .keys()
        .filter(|k| !generated.contains_key(*k))
        .collect();
    assert!(
        missing.is_empty(),
        "fehlende Dateien (UPDATE_GOLDEN=1): {missing:?}"
    );
    assert!(
        stale.is_empty(),
        "veraltete Dateien (UPDATE_GOLDEN=1): {stale:?}"
    );
    for (name, text) in &generated {
        assert_eq!(
            &on_disk[name], text,
            "{name} weicht ab (UPDATE_GOLDEN=1 schreibt neu)"
        );
    }
}

// spec: UI-140
#[test]
fn one_file_per_type_and_every_root_type_is_exported() {
    let files = fresh();
    for name in dto::TYPE_NAMES {
        assert!(files.contains_key(&format!("{name}.ts")), "{name}.ts fehlt");
    }
    assert!(
        dto::TYPE_NAMES.len() > 250,
        "{} Typen",
        dto::TYPE_NAMES.len()
    );
    let mut sorted: Vec<_> = dto::TYPE_NAMES.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), dto::TYPE_NAMES.len(), "doppelter Typname");
    // keine Sammeldatei
    assert!(!files.contains_key("index.ts"));
}

// spec: UI-140
#[test]
fn ids_and_counters_are_numbers_not_bigint() {
    for (name, text) in fresh() {
        assert!(
            !text.contains("bigint"),
            "{name} enthält bigint (UrlId und Zähler sind JSON-Zahlen)"
        );
    }
    let row = &fresh()["RowWindow.ts"];
    assert!(row.contains("offset: number"), "{row}");
    assert!(row.contains("rowKeys: Array<number>"), "{row}");
}

// spec: UI-016
#[test]
fn optional_fields_are_optional_and_nullable_in_typescript() {
    let files = fresh();
    let handle = &files["ViewHandle.ts"];
    assert!(handle.contains("total?: number | null"), "{handle}");
    assert!(
        handle.contains("emptyState?: EmptyState | null"),
        "{handle}"
    );
    let err = &files["CmdError.ts"];
    assert!(err.contains("field?: string | null"), "{err}");
}

// spec: UI-140
#[test]
fn every_derive_ts_type_in_the_sources_is_in_the_export_list() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/dto");
    let mut declared = Vec::new();
    for e in fs::read_dir(src).unwrap() {
        let p = e.unwrap().path();
        if p.file_name().is_some_and(|n| n == "mod.rs") || p.extension().is_none_or(|x| x != "rs") {
            continue;
        }
        let text = fs::read_to_string(&p).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        for (i, l) in lines.iter().enumerate() {
            let l = l.trim_start();
            let Some(rest) = l
                .strip_prefix("pub struct ")
                .or_else(|| l.strip_prefix("pub enum "))
            else {
                continue;
            };
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            // die zugehörige derive-Zeile liegt davor (Attribute und Doku dazwischen)
            let has_ts = lines[..i]
                .iter()
                .rev()
                .take_while(|p| {
                    p.trim_start().starts_with("#[")
                        || p.trim_start().starts_with("///")
                        || p.trim().is_empty()
                })
                .any(|p| p.contains("derive(") && p.contains("TS"));
            if has_ts {
                declared.push(name);
            }
        }
    }
    assert!(declared.len() > 250);
    for n in &declared {
        assert!(
            dto::TYPE_NAMES.contains(&n.as_str()),
            "{n} ist `derive(TS)`, fehlt aber in dto::TYPE_NAMES/export_all (dto/mod.rs)"
        );
    }
    assert_eq!(
        declared.len(),
        dto::TYPE_NAMES.len(),
        "Liste und Quelltext weichen ab"
    );
}

// spec: UI-001
#[test]
fn quit_decision_is_tagged_and_preserve_upload_order_is_optional() {
    let files = fresh();
    let decision = &files["QuitDecision.ts"];
    assert!(decision.contains("\"kind\": \"quit\""), "{decision}");
    assert!(decision.contains("\"kind\": \"confirm\""), "{decision}");
    assert!(
        decision.contains("reasons: Array<QuitReason>"),
        "{decision}"
    );
    assert!(files["ExportViewArgs.ts"].contains("preserveUploadOrder?: boolean"));
}

// spec: UI-140
#[test]
fn ui_state_contract_has_no_trailing_whitespace() {
    let generated = fresh();
    for line in generated["UiStateJson.ts"].lines() {
        assert_eq!(line, line.trim_end());
    }
}
