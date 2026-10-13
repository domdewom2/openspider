//! JSON-Schema der `.osconfig` (OUT-221): eingecheckte Datei = Ausgabe der Strukturen, und alle
//! Fixture-Konfigurationen werden dagegen geprüft.

use std::fs;
use std::path::{Path, PathBuf};

use os_model::*;
use serde_json::Value;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Alle `*.osconfig` unterhalb von `dir`, rekursiv und sortiert.
fn osconfigs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.map(|e| e.unwrap().path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            osconfigs(&p, out);
        } else if p.extension().is_some_and(|e| e == "osconfig") {
            out.push(p);
        }
    }
}

fn is_expected_invalid(p: &Path) -> bool {
    p.components().any(|c| c.as_os_str() == "invalid")
}

fn json_of_toml(text: &str) -> Result<Value, String> {
    let table: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
    serde_json::to_value(table).map_err(|e| e.to_string())
}

// --- ein kleiner JSON-Schema-Prüfer für genau die Schlüsselwörter, die schemars hier erzeugt ------------

fn type_matches(t: &str, v: &Value) -> bool {
    match t {
        "null" => v.is_null(),
        "boolean" => v.is_boolean(),
        "string" => v.is_string(),
        "array" => v.is_array(),
        "object" => v.is_object(),
        "number" => v.is_number(),
        "integer" => v.is_i64() || v.is_u64(),
        _ => false,
    }
}

fn check(root: &Value, schema: &Value, v: &Value, path: &str, errs: &mut Vec<String>) {
    if let Some(r) = schema.get("$ref").and_then(Value::as_str) {
        match root.pointer(r.trim_start_matches('#')) {
            Some(target) => check(root, target, v, path, errs),
            None => errs.push(format!("{path}: unauflösbares $ref {r}")),
        }
    }
    if let Some(all) = schema.get("allOf").and_then(Value::as_array) {
        all.iter().for_each(|s| check(root, s, v, path, errs));
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(alts) = schema.get(key).and_then(Value::as_array) {
            let ok = alts.iter().any(|s| {
                let mut e = Vec::new();
                check(root, s, v, path, &mut e);
                e.is_empty()
            });
            if !ok {
                errs.push(format!("{path}: passt zu keiner Alternative von {key}"));
            }
        }
    }
    if let Some(c) = schema.get("const") {
        if c != v {
            errs.push(format!("{path}: erwartet {c}, ist {v}"));
        }
    }
    if let Some(list) = schema.get("enum").and_then(Value::as_array) {
        if !list.contains(v) {
            errs.push(format!("{path}: {v} nicht in {list:?}"));
        }
    }
    if let Some(t) = schema.get("type") {
        let names: Vec<&str> = match t {
            Value::String(s) => vec![s.as_str()],
            Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
            _ => vec![],
        };
        if !names.iter().any(|n| type_matches(n, v)) {
            errs.push(format!("{path}: Typ {names:?} erwartet, ist {v}"));
            return;
        }
    }
    if let Some(n) = v.as_f64() {
        let bound = |k: &str| schema.get(k).and_then(Value::as_f64);
        if bound("minimum").is_some_and(|m| n < m) {
            errs.push(format!(
                "{path}: {n} < minimum {}",
                bound("minimum").unwrap()
            ));
        }
        if bound("maximum").is_some_and(|m| n > m) {
            errs.push(format!(
                "{path}: {n} > maximum {}",
                bound("maximum").unwrap()
            ));
        }
        if bound("exclusiveMinimum").is_some_and(|m| n <= m) {
            errs.push(format!("{path}: {n} ≤ exclusiveMinimum"));
        }
    }
    if let (Some(obj), Some(props)) = (
        v.as_object(),
        schema.get("properties").and_then(Value::as_object),
    ) {
        for (k, child) in obj {
            if let Some(s) = props.get(k) {
                check(root, s, child, &format!("{path}.{k}"), errs);
            }
        }
        if let Some(req) = schema.get("required").and_then(Value::as_array) {
            for r in req.iter().filter_map(Value::as_str) {
                if !obj.contains_key(r) {
                    errs.push(format!("{path}: Pflichtfeld {r} fehlt"));
                }
            }
        }
    }
    if let (Some(items), Some(schema_items)) = (v.as_array(), schema.get("items")) {
        for (i, item) in items.iter().enumerate() {
            check(root, schema_items, item, &format!("{path}[{i}]"), errs);
        }
    }
}

fn schema_errors(v: &Value) -> Vec<String> {
    let mut errs = Vec::new();
    let root = json_schema();
    check(root, root, v, "$", &mut errs);
    errs
}

// --------------------------------------------------------------------------------------------------------

// spec: OUT-221
#[test]
fn committed_schema_file_matches_the_generated_schema() {
    let path = repo_root().join("docs/spec/schemas/osconfig.schema.json");
    let generated = json_schema_string();
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, &generated).unwrap();
    }
    let on_disk = fs::read_to_string(&path)
        .expect("docs/spec/schemas/osconfig.schema.json fehlt (UPDATE_GOLDEN=1)");
    assert_eq!(
        on_disk, generated,
        "Schema veraltet: UPDATE_GOLDEN=1 cargo test -p os-model --test config_schema"
    );
    let v: Value = serde_json::from_str(&on_disk).unwrap();
    assert_eq!(v["title"], "OpenSpider .osconfig");
    assert_eq!(v["$schema"], "https://json-schema.org/draft/2020-12/schema");
}

// spec: OUT-221
#[test]
fn schema_describes_sections_ranges_and_enums() {
    let v = json_schema();
    let props = v["properties"].as_object().unwrap();
    for key in [
        "schema_version",
        "generator",
        "mode",
        "scope",
        "spider",
        "limits",
        "speed",
        "user_agent",
        "robots",
        "advanced",
        "auth",
        "network",
        "rendering",
        "extraction",
        "content",
        "title",
        "meta_description",
        "url",
        "h1",
        "h2",
        "image",
        "duplicates",
        "issues",
        "pdf",
        "links",
        "hreflang",
        "sitemaps",
        "analysis",
        "validation",
        "export",
    ] {
        assert!(props.contains_key(key), "{key}");
    }
    let threads = &v["$defs"]["SpeedConfig"]["properties"]["threads"];
    assert_eq!(
        (threads["minimum"].as_i64(), threads["maximum"].as_i64()),
        (Some(1), Some(200))
    );
    let rate = &v["$defs"]["SpeedConfig"]["properties"]["max_urls_per_sec"];
    assert_eq!(rate["exclusiveMinimum"], 0);
    let near = &v["$defs"]["DuplicatesConfig"]["properties"]["near_threshold_pct"];
    assert_eq!(
        (near["minimum"].as_i64(), near["maximum"].as_i64()),
        (Some(50), Some(100))
    );
    assert_eq!(v["properties"]["schema_version"]["minimum"], 1);
    assert!(v.to_string().contains("ignore_but_report"));
    // Vorwärtskompatibilität: unbekannte Schlüssel bleiben im Schema erlaubt
    assert!(v.get("additionalProperties").is_none());
}

// spec: OUT-221
#[test]
fn the_checker_accepts_defaults_and_rejects_violations() {
    let default = json_of_toml(&CrawlConfig::default().to_osconfig_string().unwrap()).unwrap();
    assert_eq!(schema_errors(&default), Vec::<String>::new());

    let mut bad = default.clone();
    bad["speed"]["threads"] = Value::from(0);
    assert!(schema_errors(&bad)[0].contains("speed.threads"));
    bad["speed"]["threads"] = Value::from("viele");
    assert!(schema_errors(&bad)[0].contains("speed.threads"));
    let mut bad = default.clone();
    bad["mode"] = Value::from("turbo");
    assert!(!schema_errors(&bad).is_empty());
    let mut bad = default.clone();
    bad["auth"]["basic"] = serde_json::json!([{ "prefix": "p", "username": "u" }]);
    assert!(schema_errors(&bad)[0].contains("secret_ref"));
    let mut bad = default;
    bad["limits"]["max_depth"] = Value::from("abc");
    assert!(!schema_errors(&bad).is_empty());
}

// spec: OUT-221
#[test]
fn all_fixture_configurations_validate_against_the_schema() {
    let root = repo_root();
    let mut files = Vec::new();
    osconfigs(&root.join("tests/fixtures"), &mut files);
    osconfigs(&root.join("tests/parity/profiles"), &mut files); // Paritätsprofile (M1-P10), sobald vorhanden
    let (invalid, valid): (Vec<_>, Vec<_>) =
        files.into_iter().partition(|p| is_expected_invalid(p));

    assert!(valid.len() >= 7, "Fixtures fehlen: {valid:?}");
    for file in &valid {
        let text = fs::read_to_string(file).unwrap();
        let json = json_of_toml(&text).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        assert_eq!(
            schema_errors(&json),
            Vec::<String>::new(),
            "{}",
            file.display()
        );
        // dieselbe Datei muss sich laden lassen und die Validierung bestehen
        let loaded = CrawlConfig::load_file(file).unwrap_or_else(|e| panic!("{e}"));
        assert!(
            loaded.validate().is_ok(),
            "{}: {:?}",
            file.display(),
            loaded.validate().errors
        );
    }

    // Die fehlerhaften Fixtures werden von Schema bzw. Laden/Validierung abgelehnt.
    assert!(invalid.len() >= 5, "{invalid:?}");
    for file in &invalid {
        let text = fs::read_to_string(file).unwrap();
        match json_of_toml(&text) {
            Err(_) => assert!(CrawlConfig::load_file(file).is_err(), "{}", file.display()),
            Ok(json) => assert!(
                !schema_errors(&json).is_empty(),
                "{} sollte dem Schema widersprechen",
                file.display()
            ),
        }
        let rejected = match CrawlConfig::load_file(file) {
            Err(_) => true,
            Ok(l) => !l.validate().is_ok(),
        };
        assert!(rejected, "{} sollte abgelehnt werden", file.display());
    }
}

// spec: OUT-221, CRAWL-132
#[test]
fn schema_covers_every_key_the_writer_emits() {
    // Jeder geschriebene Schlüssel ist im Schema bekannt (sonst würde ihn das Laden als unbekannt melden).
    let mut cfg = CrawlConfig::default();
    cfg.limits.max_depth = Some(1);
    cfg.speed.max_per_host = Some(1);
    cfg.speed.max_urls_per_sec = Some(1.0);
    cfg.user_agent.http = Some("x".into());
    cfg.user_agent.robots_token = Some("x".into());
    cfg.auth.basic.push(BasicAuth {
        prefix: "p".into(),
        username: "u".into(),
        secret_ref: "s".into(),
    });
    let text = cfg.to_osconfig_string().unwrap();
    let loaded = CrawlConfig::load_str(&text).unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    assert_eq!(loaded.config.limits.max_depth, Some(1));
}
