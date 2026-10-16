//! Wire-Format der DTOs: handgeschriebene Beispiele unter `apps/desktop/src/ipc/__fixtures__/` (M2-F01) werden in
//! den Rust-Typ gelesen und müssen unverändert wieder herauskommen (Feldnamen camelCase, Tags, Nullwerte). Dieselben
//! Dateien nutzen die vitest-Tests der Oberfläche als Vertragsbeispiele.

use std::fs;
use std::path::PathBuf;

use os_app::dto::*;
use serde::de::DeserializeOwned;
use serde::Serialize;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/desktop/src/ipc/__fixtures__")
}

fn round_trip<T: Serialize + DeserializeOwned>(file: &str, text: &str) {
    let original: serde_json::Value =
        serde_json::from_str(text).unwrap_or_else(|e| panic!("{file}: {e}"));
    let typed: T =
        serde_json::from_value(original.clone()).unwrap_or_else(|e| panic!("{file}: {e}"));
    let again = serde_json::to_value(&typed).unwrap();
    assert_eq!(
        again, original,
        "{file}: Rust-Typ gibt das Beispiel nicht unverändert zurück"
    );
}

/// Typ zu einem Dateinamen `Typ[.variante].json`.
fn check(file: &str, text: &str) -> bool {
    let ty = file.split('.').next().unwrap();
    macro_rules! types {
        ($($t:ident),* $(,)?) => {
            match ty {
                $( stringify!($t) => round_trip::<$t>(file, text), )*
                _ => return false,
            }
        };
    }
    types!(
        CmdError,
        ViewSpec,
        SearchSpec,
        Selection,
        ViewHandle,
        RowWindow,
        Tick,
        UiEvent,
        FilterInfo,
        StartRequest,
        CrawlSummaryDto,
        AppSettingsDto,
        CrawlConfigDto,
        SegmentDef,
        ApiFetchArgs,
        OpStatus,
        TaskDef,
        SerpMeasure,
        IssueRow,
        PlannerPreview,
        ErrorCode,
    );
    true
}

// spec: UI-018
#[test]
fn every_fixture_maps_to_a_type_and_round_trips() {
    let mut seen = Vec::new();
    for entry in fs::read_dir(dir()).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let file = path.file_name().unwrap().to_string_lossy().into_owned();
        let text = fs::read_to_string(&path).unwrap();
        assert!(
            check(&file, &text),
            "{file}: kein Typ zugeordnet (Test dto_fixtures.rs ergänzen)"
        );
        seen.push(file);
    }
    assert!(seen.len() >= 20, "zu wenige Beispiele: {seen:?}");
    // die Kernverträge der Oberfläche haben je ein Beispiel
    for ty in [
        "ViewSpec",
        "RowWindow",
        "Tick",
        "CmdError",
        "Selection",
        "SearchSpec",
        "AppSettingsDto",
        "TaskDef",
    ] {
        assert!(
            seen.iter().any(|f| f.starts_with(&format!("{ty}."))),
            "{ty} ohne Beispiel"
        );
    }
}

// spec: UI-140
#[test]
fn row_window_cells_are_scalar_json() {
    let text = fs::read_to_string(dir().join("RowWindow.json")).unwrap();
    let w: RowWindow = serde_json::from_str(&text).unwrap();
    assert_eq!(
        w.offset, 4_999_990,
        "5 Mio. Zeilen: Offsets über 2^22, aber unter 2^53"
    );
    assert!(w
        .cells
        .iter()
        .flatten()
        .all(|c| c.is_string() || c.is_number() || c.is_boolean() || c.is_null()));
    assert_eq!(w.cells[1][1], serde_json::Value::Null);
}

// spec: UI-011
#[test]
fn search_rules_nest_two_levels() {
    let tree = RuleNode::Group {
        op: RuleBool::And,
        children: vec![RuleNode::Group {
            op: RuleBool::Or,
            children: vec![RuleNode::Cond {
                column: Some("Address".into()),
                op: RuleOp::Contains,
                value: Some("x".into()),
            }],
        }],
    };
    let json = serde_json::to_string(&tree).unwrap();
    let back: RuleNode = serde_json::from_str(&json).unwrap();
    assert_eq!(back, tree);
}

#[test]
fn cmd_error_builders_fill_the_documented_fields() {
    let e = CmdError::unsupported("view.rows", "M2-K06");
    assert_eq!(e.code, ErrorCode::Unsupported);
    assert_eq!(e.message_key, "err-not-implemented");
    assert_eq!(e.args["command"], "view.rows");
    assert_eq!(e.args["owner"], "M2-K06");
    assert!(!e.retryable);
    let e = CmdError::invalid_input("err-bad", "scope.include[0]")
        .with_detail("x")
        .retryable();
    assert_eq!(e.field.as_deref(), Some("scope.include[0]"));
    assert!(e.retryable && e.detail.is_some());
    assert!(e.to_string().contains("InvalidInput"));
}
