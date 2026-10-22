//! Bundled axe-core audit, executed in the existing page (TECH-250).
use crate::RenderError;
use chromiumoxide::{cdp::js_protocol::runtime::EvaluateParams, Page};
use os_model::{A11yNode, A11yRuleResult, A11yWrite, UrlId};
use serde_json::Value;

pub const BUNDLE: &str = include_str!("../../resources/axe/axe.min.js");
pub const RULES: &str = include_str!("../../resources/axe/rules.json");

/// The wire payload is kept independent of the URL ID assigned by the engine.
pub async fn audit(page: &Page) -> Result<String, RenderError> {
    page.evaluate(BUNDLE).await.map_err(error)?;
    let script = format!(
        r#"(async () => {{
      const requested = {RULES}.map(r => r.id);
      const available = new Set(axe.getRules().map(r => r.ruleId));
      const ids = requested.filter(id => available.has(id));
      const missing = requested.filter(id => !available.has(id));
      const results = await axe.run(document, {{runOnly: {{type:'rule', values:ids}},
        rules:Object.fromEntries(ids.map(id => [id, {{enabled:true}}])),
        resultTypes:['violations','incomplete','inapplicable','passes']}});
      for (const kind of ['passes','inapplicable']) for (const r of results[kind]) {{r.nodes=[]; r.impact=null;}}
      return JSON.stringify({{results,missing,ids}});
    }})()"#
    );
    let result = page
        .evaluate(
            EvaluateParams::builder()
                .expression(script)
                .await_promise(true)
                .return_by_value(true)
                .build()
                .map_err(error)?,
        )
        .await
        .map_err(error)?;
    let payload: String = result.into_value().map_err(error)?;
    let parsed: Value = serde_json::from_str(&payload).map_err(error)?;
    static LOGGED: std::sync::Once = std::sync::Once::new();
    LOGGED.call_once(|| {
        if let Some(missing) = parsed["missing"].as_array() {
            for id in missing {
                tracing::warn!(rule = %id, "Bundled axe rule unavailable; skipping");
            }
        }
    });
    // Reject incomplete results rather than silently recording an unchecked page as clean.
    decode(&payload, UrlId(0))?;
    Ok(payload)
}
fn error(e: impl std::fmt::Display) -> RenderError {
    RenderError::Protocol(format!("axe: {e}"))
}

/// Preserve all result types; retain nodes only for violations and incomplete results.
pub fn decode(payload: &str, url_id: UrlId) -> Result<A11yWrite, RenderError> {
    let value: Value = serde_json::from_str(payload).map_err(error)?;
    let catalog: Vec<Value> = serde_json::from_str(RULES).map_err(error)?;
    let mut write = A11yWrite {
        url_id,
        rules: vec![],
        nodes: vec![],
    };
    let mut seen = std::collections::BTreeSet::new();
    for (key, status) in [
        ("violations", "violation"),
        ("incomplete", "incomplete"),
        ("passes", "pass"),
        ("inapplicable", "inapplicable"),
    ] {
        let rows = value["results"][key]
            .as_array()
            .ok_or_else(|| error("Missing result category"))?;
        for row in rows {
            let id = row["id"].as_str().ok_or_else(|| error("Missing rule ID"))?;
            let rule = catalog
                .iter()
                .find(|r| r["id"] == id)
                .ok_or_else(|| error("Unexpected rule"))?;
            if !seen.insert(id.to_owned()) {
                // axe reports a rule in multiple categories when different nodes differ.
                // A page has one outcome per rule: violation > incomplete > pass > inapplicable.
                continue;
            }
            let nodes = status == "violation" || status == "incomplete";
            write.rules.push(A11yRuleResult {
                rule_id: id.into(),
                status: status.into(),
                impact: if nodes {
                    row["impact"].as_str().map(str::to_owned)
                } else {
                    None
                },
            });
            if nodes {
                for node in row["nodes"]
                    .as_array()
                    .ok_or_else(|| error("Missing nodes"))?
                {
                    let target = selector(&node["target"])?;
                    write.nodes.push(A11yNode {rule_id:id.into(),target,snippet:node["html"].as_str().unwrap_or_default().chars().take(500).collect(),summary:serde_json::json!({"message":rule["issue"],"axe_message":node["failureSummary"].as_str().unwrap_or_default()}).to_string()});
                }
            }
        }
    }
    let ids = value["ids"]
        .as_array()
        .ok_or_else(|| error("Missing activated rules"))?;
    let expected: std::collections::BTreeSet<_> = ids
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    if expected != seen || ids.len() != seen.len() {
        return Err(error("Incomplete rule results"));
    }
    Ok(write)
}

fn selector(value: &Value) -> Result<String, RenderError> {
    match value {
        Value::String(text) => Ok(text.clone()),
        Value::Array(path) => path
            .iter()
            .map(selector)
            .collect::<Result<Vec<_>, _>>()
            .map(|parts| parts.join(" >>> ")),
        _ => Err(error("Missing selector")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // spec: TECH-250
    // spec: TECH-252
    // spec: TECH-253
    // spec: TECH-254
    // spec: TECH-255
    // spec: TECH-256
    // spec: TECH-257
    #[tokio::test]
    async fn requires_chromium_audits_all_available_rules_in_same_render() {
        let renderer = crate::Renderer::launch(crate::LaunchOpts::default())
            .await
            .expect("requires Chromium");
        let opts = crate::RenderOpts {
            accessibility: true,
            screenshots: false,
            ajax_timeout_ms: 0,
            page_timeout_ms: 60_000,
            main_response: Some(crate::api::MainResponse {
                status: 200,
                headers: vec![("Content-Type".into(), "text/html".into())],
                body: include_bytes!("../../../../tests/fixtures/ui/fx-ui-a11y/result-types.html")
                    .to_vec(),
            }),
            ..Default::default()
        };
        let page = renderer
            .render("https://a11y.invalid/", &opts)
            .await
            .unwrap();
        let payload = page.accessibility.as_deref().unwrap();
        let write = decode(payload, UrlId(1)).unwrap();
        let raw: Value = serde_json::from_str(payload).unwrap();
        assert_eq!(
            write.rules.len() + raw["missing"].as_array().unwrap().len(),
            93
        );
        assert_eq!(
            write
                .rules
                .iter()
                .find(|r| r.rule_id == "image-alt")
                .unwrap()
                .status,
            "violation"
        );
        assert_eq!(
            write
                .nodes
                .iter()
                .filter(|n| n.rule_id == "image-alt")
                .count(),
            2
        );
        assert_eq!(
            write
                .rules
                .iter()
                .find(|r| r.rule_id == "html-has-lang")
                .unwrap()
                .status,
            "pass"
        );
        assert_eq!(
            write
                .rules
                .iter()
                .find(|r| r.rule_id == "video-caption")
                .unwrap()
                .status,
            "inapplicable"
        );
        assert!(write.nodes.iter().all(|n| n.snippet.chars().count() <= 500));
        let clean_opts = crate::RenderOpts {
            main_response: Some(crate::api::MainResponse {
                status: 200,
                headers: vec![("Content-Type".into(), "text/html".into())],
                body: include_bytes!("../../../../tests/fixtures/ui/fx-ui-a11y/clean.html")
                    .to_vec(),
            }),
            ..opts
        };
        let clean = renderer
            .render("https://a11y.invalid/clean", &clean_opts)
            .await
            .unwrap();
        let clean = decode(clean.accessibility.as_deref().unwrap(), UrlId(2)).unwrap();
        assert!(!clean.rules.iter().any(|r| r.status == "violation"));
        renderer.close().await;
    }
    // spec: TECH-250
    // spec: TECH-259
    #[test]
    fn incomplete_retains_nodes_but_passes_do_not_and_missing_results_fail() {
        let node = serde_json::json!({"target":["iframe",["#host","#x"]],"html":"界".repeat(550),"failureSummary":"runtime message"});
        let mut payload = serde_json::json!({"ids":["color-contrast","html-has-lang","video-caption"],"results":{"violations":[],"incomplete":[{"id":"color-contrast","impact":"serious","nodes":[node.clone()]}],"passes":[{"id":"html-has-lang","nodes":[node]}],"inapplicable":[{"id":"video-caption","nodes":[]}]}});
        let w = decode(&payload.to_string(), UrlId(1)).unwrap();
        assert_eq!(w.nodes.len(), 1);
        assert_eq!(w.nodes[0].target, "iframe >>> #host >>> #x");
        assert_eq!(w.nodes[0].snippet.chars().count(), 500);
        assert!(w.nodes[0].summary.contains("axe_message"));
        payload["results"]["passes"] = serde_json::json!([]);
        assert!(decode(&payload.to_string(), UrlId(1)).is_err());
    }
}
