//! Paritätsbelege für `verified` (Abschnitt 5.4 Punkt 3, A9).
//!
//! `tests/parity/evidence.toml` ordnet Spec-IDs Exporten des Paritätslaufs zu:
//!
//! ```toml
//! [[evidence]]
//! specs = ["ONPAGE-120"]          # IDs, die der Export belegt
//! exports = ["h1_all"]            # Mapping-Slugs der Exporte in report.json
//! columns = ["H1-1", "H1-1 Length"]   # optional: nur diese Referenzspalten müssen fehlerfrei sein
//! ```
//!
//! Eine ID mit Status `implemented` wird `verified`, wenn für **jeden** Eintrag, der sie nennt:
//! * mindestens ein Export der genannten Slugs verglichen wurde (Sites mit Status `present`),
//! * in jedem verglichenen Bericht die URL-Menge identisch ist (nach dokumentierten Abweichungen),
//! * die genannten Spalten (ohne Angabe: alle verglichenen) keine Abweichung zeigen und
//! * keine Referenzeinschränkung der Free-Version (`SF-FREE-nn`, K-53) auf den Bericht gewirkt hat – solche
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
    /// Optional: nur diese Referenzspalten prüfen.
    #[serde(default)]
    pub columns: Vec<String>,
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
        let reports: Vec<&Compared<'_>> = compared.iter().filter(|c| c.export == slug).collect();
        if reports.is_empty() {
            return Err(format!("Export {slug} wurde nicht verglichen"));
        }
        for c in reports {
            let r = c.report;
            if r.get("url_set_identical").and_then(|v| v.as_bool()) != Some(true) {
                return Err(format!("{}/{slug}: URL-Menge nicht identisch", c.site));
            }
            for col in r
                .get("columns")
                .and_then(|c| c.as_array())
                .into_iter()
                .flatten()
            {
                let name = col.get("sf_column").and_then(|s| s.as_str()).unwrap_or("");
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
                .filter(|id| crate::report::is_sf_free(id))
                .collect();
            if !free.is_empty() {
                return Err(format!(
                    "{}/{slug}: Referenzeinschränkung SF Free wirkt ({})",
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
        let res = check(ev, &compared);
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
            "columns":[{"sf_column":"H1-1","errors":errors},{"sf_column":"H1-2","errors":0}],
            "applied_deviations": if free { json!([{"id":"SF-FREE-01"}]) } else { json!([{"id":"DEV-001"}]) }
        }}]}]})
    }

    fn ev(columns: &[&str]) -> Vec<Evidence> {
        vec![Evidence {
            specs: vec!["ONPAGE-120".into()],
            exports: vec!["h1_all".into()],
            columns: columns.iter().map(|c| c.to_string()).collect(),
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
        let free = evaluate(&ev(&[]), &parity(0, true, true));
        assert!(free["ONPAGE-120"].as_ref().unwrap_err().contains("SF Free"));
        let missing = evaluate(
            &[Evidence {
                specs: vec!["X-1".into()],
                exports: vec!["nicht_da".into()],
                columns: vec![],
            }],
            &parity(0, true, false),
        );
        assert!(missing["X-1"].is_err());
    }
}
