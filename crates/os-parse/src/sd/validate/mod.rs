//! Validierung strukturierter Daten (M2-D20): schema.org (TECH-187) und Google Rich Results (TECH-188).
//!
//! `run` prüft die extrahierten Elemente einer Seite und legt die Befunde in `ParsedPage.sd_findings` ab; der
//! Transport (`to_writes`) bildet sie auf `sd_issues` ab. Befund-IDs sind stabil (TECH-189):
//! `sd.schema.*` (Validierungsart `schema_org`) und `sd.google.*` (`rich_results`). Ein erkanntes Feature ist ein
//! Seitenfakt mit Schweregrad `info` und Code [`FEATURE_DETECTED`] (ohne Element, damit es nicht als Befund zählt).
//! Die Versionen von Vokabular und Regeldatei stehen je geprüfter Seite in `sdval.vocab_version` und
//! `sdval.rules_version`.

pub mod node;
pub mod rich;
pub mod schema;
pub mod values;
pub mod vocab;

#[cfg(test)]
mod tests;

use os_model::{ExtKey, ParsedPage, SdFinding, SdIssueWrite, UrlId, Value as ModelValue};
use serde_json::Value;

use crate::PageCtx;

/// Code des Seitenfakts „Rich-Result-Feature erkannt“.
pub const FEATURE_DETECTED: &str = "sd.google.feature";
/// Validierungsart der schema.org-Befunde in `sd_issues`.
pub const SCHEMA_ORG: &str = "schema_org";
/// Validierungsart der Rich-Result-Befunde in `sd_issues`.
pub const RICH_RESULTS: &str = "rich_results";

/// Höchstzahl der Befunde je Seite (Schutz vor entarteten Dokumenten).
pub const MAX_FINDINGS: usize = 5_000;
/// Höchste geprüfte Verschachtelungstiefe.
pub(crate) const MAX_DEPTH: usize = 32;

/// Ein Befund einer Prüfung, bezogen auf ein Element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// `error` oder `warning`.
    pub severity: &'static str,
    /// Stabile Befund-ID.
    pub code: &'static str,
    /// Pfad ab dem Element.
    pub path: String,
    /// Index des Features in der Regeldatei (nur Rich Results).
    pub feature: Option<usize>,
}

/// Prüft alle lesbaren Elemente der Seite gemäß `extraction.structured_data`.
pub fn run(ctx: &PageCtx<'_>, page: &mut ParsedPage) {
    let cfg = &ctx.cfg.structured_data;
    if !(cfg.schema_org_validation || cfg.rich_results_validation) {
        return;
    }
    let vocab = vocab::get();
    let rules = rich::rules();
    let html = ctx.doc.html().root_element();
    let rich_page = rich::Page {
        amp: html.value().attr("amp").is_some() || html.value().attr("⚡").is_some(),
        case_sensitive: cfg.case_sensitive,
    };
    let mut findings = Vec::new();
    let mut features: Vec<usize> = Vec::new();
    let mut checked = false;
    for (i, item) in page.structured_data.iter().enumerate() {
        if item.parse_error.is_some() {
            continue;
        }
        let Ok(root) = serde_json::from_str::<Value>(&item.json) else {
            continue;
        };
        checked = true;
        let mut push = |validation: &str, hit: Hit| {
            findings.push(SdFinding {
                item: Some(i as u32),
                validation: validation.into(),
                feature: hit.feature.map(|f| rules.features[f].name.clone()),
                severity: hit.severity.into(),
                property: Some(hit.path),
                code: hit.code.into(),
            });
        };
        if cfg.schema_org_validation {
            for hit in schema::check(&root, vocab, cfg.case_sensitive) {
                push(SCHEMA_ORG, hit);
            }
        }
        if cfg.rich_results_validation {
            let (hits, detected) = rich::check(&root, vocab, rules, rich_page);
            for hit in hits {
                push(RICH_RESULTS, hit);
            }
            for f in detected {
                if !features.contains(&f) {
                    features.push(f);
                }
            }
        }
    }
    findings.truncate(MAX_FINDINGS);
    findings.extend(features.into_iter().map(|f| SdFinding {
        item: None,
        validation: RICH_RESULTS.into(),
        feature: Some(rules.features[f].name.clone()),
        severity: "info".into(),
        property: None,
        code: FEATURE_DETECTED.into(),
    }));
    page.sd_findings.extend(findings);
    if checked {
        page.ext.push((
            ExtKey::from_static("sdval.vocab_version"),
            ModelValue::Text(vocab.version.clone()),
        ));
        if cfg.rich_results_validation {
            page.ext.push((
                ExtKey::from_static("sdval.rules_version"),
                ModelValue::Text(rules.version.clone()),
            ));
        }
    }
}

/// Bildet die Befunde einer Seite auf `sd_issues` ab (`item` zählt in den Elementen derselben Seite).
pub fn to_writes(url: UrlId, findings: &[SdFinding]) -> Vec<SdIssueWrite> {
    findings
        .iter()
        .map(|f| SdIssueWrite {
            url_id: url,
            item: f.item,
            validation: f.validation.clone(),
            feature: f.feature.clone(),
            severity: f.severity.clone(),
            property: f.property.clone(),
            code: f.code.clone(),
        })
        .collect()
}
