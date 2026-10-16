//! Erweiterungsschlüssel `ext` (M2-F01, `docs/plan/M2.md` 3.3): Namensräume je Paket.

use std::collections::BTreeSet;
use std::path::Path;

use os_model::ext::*;
use os_model::*;

fn packages() -> Vec<serde_json::Value> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/plan/M2-packages.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

// spec: OUT-226
#[test]
fn namespaces_are_unique_well_formed_and_owned_by_wave_1_to_3_packages() {
    let pkgs = packages();
    let mut seen = BTreeSet::new();
    for (ns, pkg) in NAMESPACES {
        assert!(seen.insert(*ns), "Namensraum {ns} doppelt");
        let key =
            ExtKey::new(ns, "x").unwrap_or_else(|_| panic!("{ns} ist kein gültiger Namensraum"));
        assert_eq!(key.namespace(), *ns);
        let entry = pkgs
            .iter()
            .find(|p| p["id"] == *pkg)
            .unwrap_or_else(|| panic!("{pkg} (Namensraum {ns}) steht nicht in M2-packages.json"));
        let wave = entry["wave"].to_string();
        assert!(
            ["1", "2", "3", "\"1\"", "\"2\"", "\"3\""].contains(&wave.as_str()),
            "{pkg}: Welle {wave} ist keine Welle 1 – 3"
        );
    }
    // die Beispiele aus dem Plan
    assert_eq!(owner_of("security"), Some("M2-D11"));
    assert_eq!(owner_of("head"), Some("M2-D01"));
    assert_eq!(owner_of("readability"), Some("M2-D02"));
    assert_eq!(owner_of("unbekannt"), None);
}

// spec: OUT-226
#[test]
fn every_fachpaket_with_a_tab_has_a_namespace() {
    let owners: BTreeSet<&str> = NAMESPACES.iter().map(|(_, p)| *p).collect();
    // D-Pakete, die Tabs/Filter und damit skalare Fakten je URL liefern; D17 (Reports) und D18 (Exporte) schreiben nichts je URL
    for id in [
        "M2-D01", "M2-D02", "M2-D03", "M2-D04", "M2-D05", "M2-D06", "M2-D07", "M2-D08", "M2-D09",
        "M2-D10", "M2-D11", "M2-D12", "M2-D13", "M2-D14", "M2-D15", "M2-D16", "M2-D19", "M2-D20",
        "M2-D21", "M2-D22", "M2-D23", "M2-E01", "M2-E02", "M2-E03", "M2-E04", "M2-E05", "M2-R02",
        "M2-I01", "M2-I02", "M2-I03", "M2-I04", "M2-I05", "M2-I06", "M2-K14", "M2-C01", "M2-C02",
    ] {
        assert!(owners.contains(id), "{id} hat keinen Namensraum");
    }
}

// spec: OUT-226
#[test]
fn keys_are_parsed_and_checked_against_the_owner() {
    let k = ExtKey::parse("security.csp_present").unwrap();
    assert_eq!(k.namespace(), "security");
    assert_eq!(k.owner(), Some("M2-D11"));
    assert!(key_allowed_for("M2-D11", &k));
    assert!(!key_allowed_for("M2-D01", &k));
    assert_eq!(k.to_string(), "security.csp_present");
    for bad in [
        "",
        "security",
        ".x",
        "x.",
        "Security.x",
        "a.b.c",
        "a b.c",
        "1a.b",
    ] {
        assert!(
            ExtKey::parse(bad).is_err(),
            "{bad:?} darf kein Schlüssel sein"
        );
    }
    assert_eq!(
        ExtKey::from_static("head.title_outside").namespace(),
        "head"
    );
}

// spec: OUT-226
#[test]
fn ext_writes_and_parsed_pages_carry_values() {
    let w = ExtWrite {
        url: UrlId(7),
        key: ExtKey::parse("readability.flesch").unwrap(),
        value: Value::Float(61.5),
    };
    assert_eq!(w.key.owner(), Some("M2-D02"));
    let mut page = ParsedPage::default();
    assert!(page.ext.is_empty() && page.structured_data.is_empty() && page.forms.is_empty());
    assert!(page.validation.is_none() && page.minhash.is_none());
    page.ext.push((w.key.clone(), w.value.clone()));
    assert_eq!(page.ext.len(), 1);
}
