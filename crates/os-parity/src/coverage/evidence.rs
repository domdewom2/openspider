//! Paritätsbelege für `verified` (Abschnitt 5.4 Punkt 3, A9).
//!
//! `tests/parity/evidence.toml` ordnet Spec-IDs Exporten des Paritätslaufs zu:
//!
//! ```toml
//! [[evidence]]
//! specs = ["ONPAGE-120"]          # IDs, die der Export belegt
//! exports = ["h1_all"]            # Mapping-Slugs der Exporte in report.json
//! columns = ["H1-1", "H1-1 Length"]   # optional: nur diese Referenzspalten müssen fehlerfrei sein
//! url_set = false                 # optional (Standard true): URL-Menge muss nicht identisch sein
//! ```
//!
//! Eine ID mit Status `implemented` wird `verified`, wenn für **jeden** Eintrag, der sie nennt:
//! * mindestens ein Export der genannten Slugs verglichen wurde (Sites mit Status `present`),
//! * in jedem verglichenen Bericht die URL-Menge identisch ist (nach dokumentierten Abweichungen; `url_set = false`
//!   hebt das für Belege über einzelne Spalten auf),
//! * die genannten Spalten (ohne Angabe: alle verglichenen) keine Abweichung zeigen und
//! * keine Referenzeinschränkung der Free-Version (`REF-FREE-nn`, K-53) auf den Bericht gewirkt hat – solche
//!   Filter bleiben `implemented`, ihr Beleg ist der spec-abgeleitete Fixture-Test.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::ParityError;

/// Ein Beleg-Eintrag.
#[derive(Debug, Clone, Deserialize)]
pub struct Evidence {
    /// Belegte Spec-IDs.
    pub specs: Vec<String>,
    /// Mapping-Slugs der Exporte.
    pub exports: Vec<String>,
    /// Optionaler Site-Selektor.
    #[serde(default)]
    pub sites: Vec<String>,
    /// Optionaler Referenzdatei-Selektor (unterscheidet Filter mit gleichem Mapping).
    #[serde(default)]
    pub files: Vec<String>,
    /// Optional: nur diese Referenzspalten prüfen.
    #[serde(default)]
    pub columns: Vec<String>,
    /// Die URL-Menge des Exports muss identisch sein (Standard). Bei einem Beleg über einzelne Spalten
    /// (`columns`) darf `url_set = false` stehen: dann zählen nur die zugeordneten Zeilen.
    #[serde(default = "yes")]
    pub url_set: bool,
}

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
struct RawEvidence {
    #[serde(default)]
    evidence: Vec<Evidence>,
}

/// Lädt `evidence.toml`.
pub fn load_evidence(path: &Path) -> Result<Vec<Evidence>, ParityError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| ParityError::Invalid(format!("{} nicht lesbar: {e}", path.display())))?;
    let raw: RawEvidence = toml::from_str(&text)
        .map_err(|e| ParityError::Invalid(format!("{}: {e}", path.display())))?;
    Ok(raw.evidence)
}

/// Ein verglichener Export aus `report.json`.
struct Compared<'a> {
    site: &'a str,
    export: &'a str,
    file: &'a str,
    report: &'a Value,
}

fn compared_reports(parity: &Value) -> Vec<Compared<'_>> {
    let mut out = Vec::new();
    for site in parity
        .get("sites")
        .and_then(|s| s.as_array())
        .into_iter()
        .flatten()
    {
        let name = site.get("site").and_then(|s| s.as_str()).unwrap_or("");
        for e in site
            .get("exports")
            .and_then(|e| e.as_array())
            .into_iter()
            .flatten()
        {
            if e.get("outcome").and_then(|o| o.as_str()) != Some("compared") {
                continue;
            }
            let Some(report) = e.get("report") else {
                continue;
            };
            let export = report.get("export").and_then(|x| x.as_str()).unwrap_or("");
            out.push(Compared {
                site: name,
                export,
                file: e.get("ref_file").and_then(Value::as_str).unwrap_or(""),
                report,
            });
        }
    }
    out
}

/// Prüft einen Eintrag; `Ok` liefert die Beleg-Zeilen, `Err` den Grund für das Verfehlen.
fn check(ev: &Evidence, compared: &[Compared<'_>]) -> Result<Vec<Value>, String> {
    let mut lines = Vec::new();
    for slug in &ev.exports {
        let reports: Vec<&Compared<'_>> = compared
            .iter()
            .filter(|c| {
                c.export == slug
                    && (ev.sites.is_empty() || ev.sites.iter().any(|s| s == c.site))
                    && (ev.files.is_empty() || ev.files.iter().any(|f| f == c.file))
            })
            .collect();
        if reports.is_empty() {
            return Err(format!("Export {slug} wurde nicht verglichen"));
        }
        for c in reports {
            let r = c.report;
            for field in [
                "missing_ref_columns",
                "missing_os_columns",
                "unmapped_ref_columns",
            ] {
                if r.get(field)
                    .and_then(Value::as_array)
                    .is_some_and(|v| !v.is_empty())
                {
                    return Err(format!(
                        "{}/{slug}: Spaltenabdeckung unvollständig ({field})",
                        c.site
                    ));
                }
            }
            for requested in &ev.columns {
                if !r["columns"]
                    .as_array()
                    .is_some_and(|cols| cols.iter().any(|c| c["ref_column"] == *requested))
                {
                    return Err(format!("{}/{slug}: Belegspalte {requested} fehlt", c.site));
                }
            }
            if ev.url_set && r.get("url_set_identical").and_then(|v| v.as_bool()) != Some(true) {
                return Err(format!("{}/{slug}: URL-Menge nicht identisch", c.site));
            }
            for col in r
                .get("columns")
                .and_then(|c| c.as_array())
                .into_iter()
                .flatten()
            {
                let name = col.get("ref_column").and_then(|s| s.as_str()).unwrap_or("");
                if !ev.columns.is_empty() && !ev.columns.iter().any(|c| c == name) {
                    continue;
                }
                let errors = col.get("errors").and_then(|e| e.as_u64()).unwrap_or(0);
                if errors > 0 {
                    return Err(format!(
                        "{}/{slug}: Spalte {name} mit {errors} Abweichungen",
                        c.site
                    ));
                }
            }
            let free: Vec<&str> = r
                .get("applied_deviations")
                .and_then(|d| d.as_array())
                .into_iter()
                .flatten()
                .filter_map(|d| d.get("id").and_then(|i| i.as_str()))
                .filter(|id| crate::report::is_ref_free(id))
                .collect();
            if !free.is_empty() {
                return Err(format!(
                    "{}/{slug}: Referenzeinschränkung (Free) wirkt ({})",
                    c.site,
                    free.join(", ")
                ));
            }
            lines.push(json!({
                "site": c.site,
                "export": slug,
                "cells_compared": r.get("cells_compared"),
                "agreement_pct": r.get("agreement_pct"),
                "deviations": r.get("applied_deviations").and_then(|d| d.as_array()).map(|d| {
                    d.iter().filter_map(|x| x.get("id").cloned()).collect::<Vec<_>>()
                }),
            }));
        }
    }
    Ok(lines)
}

/// Ergebnis je Spec-ID: Beleg oder Grund, warum keiner entsteht.
pub fn evaluate(evidence: &[Evidence], parity: &Value) -> BTreeMap<String, Result<Value, String>> {
    let compared = compared_reports(parity);
    let mut per_id: BTreeMap<String, Vec<Result<Vec<Value>, String>>> = BTreeMap::new();
    for ev in evidence {
        let unavailable = parity["sites"]
            .as_array()
            .into_iter()
            .flatten()
            .find_map(|site| {
                let name = site["site"].as_str().unwrap_or("");
                if !ev.sites.is_empty() && !ev.sites.iter().any(|s| s == name) {
                    return None;
                }
                site["exports"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|e| {
                        e["status"] == "present"
                            && ev.exports.iter().any(|s| e["mapping"] == *s)
                            && (ev.files.is_empty() || ev.files.iter().any(|f| e["ref_file"] == *f))
                            && e["outcome"] != "compared"
                    })
                    .map(|e| format!("{name}/{}: nicht verglichen", e["ref_file"]))
            });
        let res = unavailable.map_or_else(|| check(ev, &compared), Err);
        for id in &ev.specs {
            per_id.entry(id.clone()).or_default().push(res.clone());
        }
    }
    per_id
        .into_iter()
        .map(|(id, results)| {
            let mut lines = Vec::new();
            let mut failure = None;
            for r in results {
                match r {
                    Ok(l) => lines.extend(l),
                    Err(why) => {
                        failure = Some(why);
                        break;
                    }
                }
            }
            let out = match failure {
                Some(why) => Err(why),
                None => Ok(json!({ "exports": lines })),
            };
            (id, out)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parity(errors: u64, same: bool, free: bool) -> Value {
        json!({"sites":[{"site":"s1","exports":[{"outcome":"compared","report":{
            "export":"h1_all","url_set_identical":same,"cells_compared":10,"agreement_pct":100.0,
            "columns":[{"ref_column":"H1-1","errors":errors},{"ref_column":"H1-2","errors":0}],
            "applied_deviations": if free { json!([{"id":"REF-FREE-01"}]) } else { json!([{"id":"DEV-001"}]) }
        }}]}]})
    }

    fn ev(columns: &[&str]) -> Vec<Evidence> {
        vec![Evidence {
            specs: vec!["ONPAGE-120".into()],
            exports: vec!["h1_all".into()],
            sites: vec![],
            files: vec![],
            columns: columns.iter().map(|c| c.to_string()).collect(),
            url_set: true,
        }]
    }

    #[test]
    fn clean_export_verifies_the_spec() {
        let r = evaluate(&ev(&[]), &parity(0, true, false));
        let v = r.get("ONPAGE-120").unwrap().as_ref().unwrap();
        assert_eq!(v["exports"][0]["deviations"][0], "DEV-001");
    }

    #[test]
    fn column_errors_url_set_and_free_restrictions_block_verification() {
        assert!(evaluate(&ev(&[]), &parity(2, true, false))["ONPAGE-120"].is_err());
        assert!(evaluate(&ev(&["H1-2"]), &parity(2, true, false))["ONPAGE-120"].is_ok());
        assert!(evaluate(&ev(&[]), &parity(0, false, false))["ONPAGE-120"].is_err());
        let mut columns_only = ev(&["H1-2"]);
        columns_only[0].url_set = false;
        assert!(evaluate(&columns_only, &parity(0, false, false))["ONPAGE-120"].is_ok());
        let free = evaluate(&ev(&[]), &parity(0, true, true));
        assert!(free["ONPAGE-120"]
            .as_ref()
            .unwrap_err()
            .contains("Referenzeinschränkung (Free)"));
        let missing = evaluate(
            &[Evidence {
                specs: vec!["X-1".into()],
                exports: vec!["nicht_da".into()],
                sites: vec![],
                files: vec![],
                columns: vec![],
                url_set: true,
            }],
            &parity(0, true, false),
        );
        assert!(missing["X-1"].is_err());
    }
}

/// SQ-01: Filterbelege aus den kanonischen Katalogen und dem vollständigen Laufjournal.
/// Jeder Tab-All-Export und jeder vorhandene eigene Filterexport muss streng grün sein.
pub fn filter_summary(root: &Path, parity: &Value) -> Result<Value, ParityError> {
    let excluded_tabs = [
        "JavaScript",
        "Structured Data",
        "PageSpeed",
        "Mobile",
        "Accessibility",
        "Custom Search",
        "Custom Extraction",
        "Custom JavaScript",
        "Analytics",
        "Search Console",
        "Link Metrics",
        "Sitemaps",
        "Change Detection",
    ];
    let excluded_ids = [
        "ONPAGE-053",
        "ONPAGE-057",
        "ONPAGE-155",
        "ONPAGE-156",
        "ONPAGE-178",
        "ONPAGE-180",
    ];
    let mut checks: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let compared = compared_reports(parity);
    for milestone in ["M1", "M2"] {
        let catalog: toml::Value = toml::from_str(&std::fs::read_to_string(
            root.join(format!("docs/plan/{milestone}-filters.toml")),
        )?)
        .map_err(|e| ParityError::Invalid(e.to_string()))?;
        for tab in catalog["tab"].as_array().into_iter().flatten() {
            let name = tab["id"].as_str().unwrap_or("");
            if excluded_tabs.contains(&name) {
                continue;
            }
            for filter in tab["filters"].as_array().into_iter().flatten() {
                let id = filter["id"].as_str().unwrap_or("");
                let spec = filter["spec"].as_str().unwrap_or("");
                if filter.get("extension").and_then(toml::Value::as_bool) == Some(true)
                    || id.contains('<')
                    || excluded_ids.contains(&spec)
                {
                    continue;
                }
                let label = filter
                    .get("default_label")
                    .and_then(toml::Value::as_str)
                    .unwrap_or(id);
                let all = format!("tab:{name}:All");
                let own = format!("tab:{name}:{label}");
                let mut selected = 0;
                let mut all_count = 0;
                let mut failure = None;
                for site in parity["sites"].as_array().into_iter().flatten() {
                    let site_name = site["site"].as_str().unwrap_or("");
                    for export in site["exports"].as_array().into_iter().flatten() {
                        let os = export["os"].as_str().unwrap_or("");
                        if export["status"] != "present" || (os != all && os != own) {
                            continue;
                        }
                        selected += 1;
                        if os == all {
                            all_count += 1;
                        }
                        if export["outcome"] != "compared" {
                            failure.get_or_insert_with(|| {
                                format!("{site_name}/{}: nicht verglichen", export["ref_file"])
                            });
                            continue;
                        }
                        let ev = Evidence {
                            specs: vec![spec.to_string()],
                            exports: vec![export["mapping"].as_str().unwrap_or("").to_string()],
                            sites: vec![site_name.to_string()],
                            files: vec![export["ref_file"].as_str().unwrap_or("").to_string()],
                            columns: vec![],
                            url_set: true,
                        };
                        if let Err(why) = check(&ev, &compared) {
                            failure.get_or_insert(why);
                        }
                    }
                }
                if all_count == 0 || selected == 0 {
                    failure.get_or_insert("Tab-All wurde nicht verglichen".into());
                }
                if ["Internal", "Links"].contains(&name) {
                    failure.get_or_insert("SQ-10: TECH-145 OS-Festlegung, Kalibrierung M3".into());
                }
                checks.entry(spec.to_string()).or_default().push(json!({
                    "tab": name, "filter": id, "passed": failure.is_none(),
                    "reason": failure.unwrap_or_else(|| "vollständig fehlerfrei verglichen".into()),
                    "exports_checked": selected,
                }));
            }
        }
    }
    let verified: Vec<_> = checks
        .iter()
        .filter(|(_, rows)| rows.iter().all(|r| r["passed"] == true))
        .map(|(id, _)| id.clone())
        .collect();
    Ok(
        json!({"denominator": checks.len(), "referencable_ids": checks.keys().collect::<Vec<_>>(),
        "verified": verified.len(), "pct": 100.0 * verified.len() as f64 / checks.len() as f64,
        "verified_ids": verified, "checks": checks, "target_pct": 60,
        "definition": "SQ-01: je Filter-ID, Tab-All auf allen Sites plus eigener Filterexport; URL-Menge, null Zellfehler, vollständige Spalten, DEV erlaubt, REF-FREE sperrt; ohne Change Detection"}),
    )
}

#[cfg(test)]
mod strict_tests {
    use super::*;

    // spec: OUT-080
    #[test]
    fn incomplete_columns_and_missing_requested_columns_never_verify() {
        let mut parity = json!({"sites":[{"site":"s","exports":[{"ref_file":"all.csv","outcome":"compared","report":{
            "export":"h1_all","url_set_identical":true,"columns":[],"applied_deviations":[],
            "missing_ref_columns":[],"missing_os_columns":[],"unmapped_ref_columns":[]
        }}]}]});
        let mut ev = Evidence {
            specs: vec!["ONPAGE-120".into()],
            exports: vec!["h1_all".into()],
            sites: vec!["s".into()],
            files: vec!["all.csv".into()],
            columns: vec![],
            url_set: true,
        };
        assert!(evaluate(&[ev.clone()], &parity)["ONPAGE-120"].is_ok());
        for field in [
            "missing_ref_columns",
            "missing_os_columns",
            "unmapped_ref_columns",
        ] {
            parity["sites"][0]["exports"][0]["report"][field] = json!(["H1-1"]);
            assert!(evaluate(&[ev.clone()], &parity)["ONPAGE-120"].is_err());
            parity["sites"][0]["exports"][0]["report"][field] = json!([]);
        }
        ev.columns = vec!["H1-1".into()];
        assert!(evaluate(&[ev.clone()], &parity)["ONPAGE-120"].is_err());
        ev.columns.clear();
        ev.files = vec!["filter.csv".into()];
        assert!(evaluate(&[ev], &parity)["ONPAGE-120"].is_err());
    }

    // spec: OUT-080
    #[test]
    fn sq01_denominator_excludes_change_detection() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let summary = filter_summary(&root, &json!({"sites":[]})).unwrap();
        assert_eq!(summary["denominator"], 155);
        assert_eq!(summary["verified"], 0);
        assert!(summary["referencable_ids"]
            .as_array()
            .unwrap()
            .iter()
            .all(|id| !id.as_str().unwrap().starts_with("TECH-30")));
    }
}
