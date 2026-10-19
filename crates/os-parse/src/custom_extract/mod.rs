//! Custom Extraction: page-local XPath, CSS and bounded regular expressions.
mod html;
mod xpath;

use std::time::{Duration, Instant};

use fancy_regex::RegexBuilder;
use os_model::{
    ConfigIssue, CustomExtractionConfig, ExtKey, ExtractedValue, ExtractorMode, ExtractorOutput,
    ParsedPage, Value,
};
use scraper::{ElementRef, Selector};

use crate::PageCtx;

const MAX_VALUES: usize = 1_000;

/// Evaluate extractors in configuration order. The caller gates internal HTML and 2xx responses.
pub fn run(ctx: &PageCtx<'_>, page: &mut ParsedPage) {
    let cfg = &ctx.cfg.custom_extraction;
    let ordered = cfg
        .extractors
        .iter()
        .any(|e| {
            matches!(
                e.output,
                ExtractorOutput::HtmlElement | ExtractorOutput::InnerHtml
            )
        })
        .then(|| html::OrderedHtml::parse(ctx.html));
    let doc = ordered.as_ref().map_or(ctx.doc.html(), |d| &d.html);
    let mut xpath = None;
    for (def_index, extractor) in cfg.extractors.iter().take(100).enumerate() {
        // Probe one extra value so exactly 1,000 matches do not produce a warning.
        let remaining = MAX_VALUES.saturating_sub(page.custom_extraction.len());
        let values = match extractor.mode {
            ExtractorMode::Css => Selector::parse(&extractor.expression)
                .map_err(|e| format!("CSS: {e}"))
                .map(|selector| {
                    doc.select(&selector)
                        .filter_map(|element| {
                            if let Some(attr) =
                                extractor.attribute.as_deref().filter(|a| !a.is_empty())
                            {
                                element.value().attr(attr).map(str::to_owned)
                            } else {
                                Some(element_value(element, extractor.output, ordered.as_ref()))
                            }
                        })
                        .take(remaining + 1)
                        .collect()
                }),
            ExtractorMode::XPath => xpath
                .get_or_insert_with(|| xpath::Document::new(doc))
                .evaluate(extractor, remaining + 1, ordered.as_ref()),
            ExtractorMode::Regex => regex_values(
                &extractor.expression,
                ctx.html,
                ctx.cfg.custom_search.regex_timeout_ms,
                remaining + 1,
            ),
        };
        let (values, error) = match values {
            Ok(values) => (values, None),
            Err(error) => (vec![error.clone()], Some(error)),
        };
        let limited = values.len() > remaining;
        page.custom_extraction
            .extend(
                values
                    .into_iter()
                    .take(remaining)
                    .enumerate()
                    .map(|(number, value)| ExtractedValue {
                        def_index: def_index as u32,
                        number: number as u32,
                        value,
                        error: error.clone(),
                    }),
            );
        if limited {
            page.ext.push((
                ExtKey::from_static("custom_extract.warnings"),
                Value::Text("Extraction limit reached".into()),
            ));
            break;
        }
    }
}

fn element_value(
    element: ElementRef<'_>,
    output: ExtractorOutput,
    ordered: Option<&html::OrderedHtml>,
) -> String {
    match output {
        ExtractorOutput::HtmlElement | ExtractorOutput::InnerHtml => ordered
            .expect("HTML serializer initialized")
            .serialize(element, output == ExtractorOutput::InnerHtml),
        _ => {
            let mut text = String::new();
            // Unlike content-area extraction, only script/style are excluded here.
            for edge in element.traverse() {
                match edge {
                    ego_tree::iter::Edge::Open(node)
                        if !node.ancestors().chain(std::iter::once(node)).any(|a| {
                            a.value()
                                .as_element()
                                .is_some_and(|e| matches!(e.name(), "script" | "style"))
                        }) =>
                    {
                        if let Some(t) = node.value().as_text() {
                            text.push_str(t);
                        }
                        if node.value().as_element().is_some_and(|e| e.name() == "br") {
                            text.push(' ');
                        }
                    }
                    _ => {}
                }
            }
            crate::text::normalize_ws(&text)
        }
    }
}

fn regex_values(
    expression: &str,
    source: &str,
    timeout_ms: u32,
    limit: usize,
) -> Result<Vec<String>, String> {
    let regex = regex(expression).map_err(|e| format!("Regex: {e}"))?;
    let start = Instant::now();
    let budget = Duration::from_millis(u64::from(timeout_ms));
    let mut values = Vec::new();
    for captures in regex.captures_iter(source) {
        if start.elapsed() >= budget {
            return Err("Regex timeout".into());
        }
        let captures = captures.map_err(|_| "Regex timeout".to_string())?;
        let index = usize::from(captures.len() > 1);
        if let Some(value) = captures.get(index) {
            values.push(value.as_str().to_owned());
            if values.len() == limit {
                break;
            }
        }
    }
    Ok(values)
}

fn regex(expression: &str) -> Result<fancy_regex::Regex, fancy_regex::Error> {
    // A finite VM step budget also bounds a single captures_iter step, where a wall
    // clock check cannot interrupt backtracking. DOTALL applies only to extraction.
    RegexBuilder::new(expression)
        .dot_matches_new_line(true)
        .backtrack_limit(100_000)
        .build()
}

/// Compile expressions without evaluating page content or performing I/O.
pub fn validate(cfg: &CustomExtractionConfig) -> Vec<ConfigIssue> {
    let mut issues = Vec::new();
    if cfg.extractors.len() > 100 {
        issues.push(ConfigIssue {
            kind: os_model::ConfigIssueKind::InvalidValue,
            line: None,
            column: None,
            path: "custom_extraction.extractors".into(),
            code: "limit".into(),
            message: "Maximal 100 Extraktoren".into(),
            position: None,
        });
    }
    for (i, extractor) in cfg.extractors.iter().enumerate() {
        if extractor.output == ExtractorOutput::FunctionValue
            && extractor.mode != ExtractorMode::XPath
        {
            issues.push(ConfigIssue {
                kind: os_model::ConfigIssueKind::InvalidValue,
                line: None,
                column: None,
                path: format!("custom_extraction.extractors[{i}].output"),
                code: "output_mode".into(),
                message: "Funktionswert ist nur für XPath verfügbar".into(),
                position: None,
            });
        }
        let result = match extractor.mode {
            ExtractorMode::XPath => xee_xpath::Queries::default()
                .sequence(&extractor.expression)
                .map(|_| ())
                .map_err(|e| (e.to_string(), e.span.map(|s| s.range().start as u32))),
            ExtractorMode::Css => Selector::parse(&extractor.expression)
                .map(|_| ())
                .map_err(|e| (e.to_string(), None)),
            ExtractorMode::Regex => regex(&extractor.expression).map(|_| ()).map_err(|e| {
                let position = match &e {
                    fancy_regex::Error::ParseError(position, _) => Some(*position as u32),
                    _ => None,
                };
                (e.to_string(), position)
            }),
        };
        if let Err((message, position)) = result {
            issues.push(ConfigIssue {
                kind: os_model::ConfigIssueKind::InvalidValue,
                line: None,
                column: None,
                path: format!("custom_extraction.extractors[{i}].expression"),
                code: "syntax".into(),
                message,
                position,
            });
        }
    }
    issues
}
