//! Custom Search: decoded HTML, text regions and XPath (INTEG-020 … 035).

use std::collections::HashSet;
use std::time::{Duration, Instant};

use ego_tree::iter::Edge;
use fancy_regex::{Regex, RegexBuilder};
use os_model::{
    ConfigIssue, ConfigIssueKind, ContentAreaConfig, CustomSearchConfig, ParsedPage, SearchFilter,
    SearchHit, SearchKind, SearchScope,
};
use scraper::{ElementRef, Node, Selector};
use xee_xpath::{Documents, Queries, Query};

use crate::{dom::Document, PageCtx, ParseInput};

/// Evaluate every configured filter, including zero counts and errors.
pub fn run(ctx: &PageCtx<'_>, page: &mut ParsedPage) {
    page.custom_search = if ctx.html.is_empty() {
        Vec::new()
    } else {
        evaluate(ctx.doc, ctx.html, ctx.cfg)
    };
}

/// The engine already restricts this entry point to internal HTML responses.
pub fn run_on_body(input: &ParseInput<'_>) -> Vec<SearchHit> {
    if input.body.is_empty() || input.cfg.custom_search.filters.is_empty() {
        return Vec::new();
    }
    let (html, _) = crate::charset::decode(input.body, input.content_type, input.headers);
    evaluate(&Document::parse(&html), &html, input.cfg)
}

/// Effective name shared by validation, columns and exports.
pub fn filter_name(filter: &SearchFilter, index: usize) -> String {
    if filter.name.trim().is_empty() {
        format!("Filter {}", index + 1)
    } else {
        filter.name.trim().to_owned()
    }
}

fn regex(expression: &str) -> Result<Regex, fancy_regex::Error> {
    // A deterministic VM step cap bounds even a single find(), where elapsed-time
    // checks cannot interrupt execution. No worker threads survive a timed-out page.
    RegexBuilder::new(expression)
        .backtrack_limit(50_000)
        .build()
}

fn evaluate(doc: &Document, html: &str, cfg: &crate::ParseConfig) -> Vec<SearchHit> {
    cfg.custom_search
        .filters
        .iter()
        .enumerate()
        .map(|(i, filter)| {
            let result = regions(doc, html, &cfg.content_area, filter)
                .and_then(|regions| count(&regions, filter, cfg.custom_search.regex_timeout_ms));
            match result {
                Ok(count) => SearchHit {
                    def_index: i as u32,
                    count,
                    error: None,
                },
                Err(error) => SearchHit {
                    def_index: i as u32,
                    count: 0,
                    error: Some(error),
                },
            }
        })
        .collect()
}

fn regions(
    doc: &Document,
    html: &str,
    area: &ContentAreaConfig,
    filter: &SearchFilter,
) -> Result<Vec<String>, String> {
    Ok(match filter.scope {
        SearchScope::Html => vec![html.to_owned()],
        SearchScope::HtmlHead | SearchScope::HtmlBody => {
            let selector = Selector::parse(if filter.scope == SearchScope::HtmlHead {
                "head"
            } else {
                "body"
            })
            .unwrap();
            doc.html()
                .select(&selector)
                .map(|el| el.inner_html())
                .collect()
        }
        SearchScope::PageText => vec![visible_text(doc, false, None)],
        SearchScope::PageTextNoAnchors => vec![visible_text(doc, true, None)],
        SearchScope::ContentArea => vec![visible_text(doc, false, Some(area))],
        SearchScope::XPath => xpath_text(doc, filter.xpath.as_deref().unwrap_or(""))?,
    })
}

fn matches_rule(
    el: &scraper::node::Element,
    names: &[String],
    classes: &[String],
    ids: &[String],
) -> bool {
    names
        .iter()
        .any(|s| s.trim().eq_ignore_ascii_case(el.name()))
        || classes.iter().any(|s| {
            el.has_class(
                s.trim().trim_start_matches('.'),
                scraper::CaseSensitivity::CaseSensitive,
            )
        })
        || ids
            .iter()
            .any(|s| el.attr("id") == Some(s.trim().trim_start_matches('#')))
}

fn visible_text(doc: &Document, no_anchors: bool, area: Option<&ContentAreaConfig>) -> String {
    let Some(body) = doc.html().select(&Selector::parse("body").unwrap()).next() else {
        return String::new();
    };
    let has_include = area.is_some_and(|a| {
        !a.include_elements.is_empty() || !a.include_classes.is_empty() || !a.include_ids.is_empty()
    });
    let mut out = String::new();
    let mut skip = 0usize;
    let mut included = 0usize;
    let mut marks = Vec::new();
    for edge in body.traverse() {
        match edge {
            Edge::Open(node) => match node.value() {
                Node::Element(el) => {
                    if skip > 0 {
                        skip += 1;
                        continue;
                    }
                    let name = el.name();
                    if crate::dom::is_block(name) {
                        out.push(' ');
                    }
                    if matches!(name, "script" | "style" | "noscript" | "template" | "title")
                        || (no_anchors && name == "a")
                        || area.is_some_and(|a| {
                            name == "svg"
                                || matches_rule(
                                    el,
                                    &a.exclude_elements,
                                    &a.exclude_classes,
                                    &a.exclude_ids,
                                )
                        })
                    {
                        skip = 1;
                        continue;
                    }
                    let hit = area.is_some_and(|a| {
                        matches_rule(el, &a.include_elements, &a.include_classes, &a.include_ids)
                    });
                    marks.push(hit);
                    included += usize::from(hit);
                }
                Node::Text(text) if skip == 0 && (!has_include || included > 0) => {
                    out.push_str(text)
                }
                _ => {}
            },
            Edge::Close(node) => {
                if let Node::Element(el) = node.value() {
                    if skip > 0 {
                        skip -= 1;
                    } else {
                        included -= usize::from(marks.pop() == Some(true));
                    }
                    if skip == 0 && crate::dom::is_block(el.name()) {
                        out.push(' ');
                    }
                }
            }
        }
    }
    crate::text::normalize_ws(&out)
}

fn xpath_text(doc: &Document, expression: &str) -> Result<Vec<String>, String> {
    let mut documents = Documents::new();
    let xot = documents.xot_mut();
    let root = xot.new_document();
    let mut stack = vec![root];
    // Build XPath's tree from the repaired HTML5 DOM, rather than parsing HTML as XML.
    // HTML element names deliberately have no namespace (the configuration contract).
    for edge in doc.html().root_element().traverse() {
        match edge {
            Edge::Open(node) => match node.value() {
                Node::Element(el) => {
                    let name = xot.add_name(el.name());
                    let child = xot.new_element(name);
                    for (name, value) in el.attrs() {
                        let name = xot.add_name(name);
                        xot.set_attribute(child, name, value);
                    }
                    xot.append(*stack.last().unwrap(), child)
                        .map_err(|e| e.to_string())?;
                    stack.push(child);
                }
                Node::Text(text) => xot
                    .append_text(*stack.last().unwrap(), text)
                    .map_err(|e| e.to_string())?,
                Node::Comment(text) => xot
                    .append_comment(*stack.last().unwrap(), text)
                    .map_err(|e| e.to_string())?,
                _ => {}
            },
            Edge::Close(node) if ElementRef::wrap(node).is_some() => {
                stack.pop();
            }
            _ => {}
        }
    }
    let queries = Queries::default();
    let query = queries
        .many(&format!("({expression}) ! string(.)"), |_, item| {
            Ok(item.try_into_value::<String>()?)
        })
        .map_err(|e| e.to_string())?;
    query
        .execute(&mut documents, root)
        .map_err(|e| e.to_string())
}

fn count(regions: &[String], filter: &SearchFilter, timeout_ms: u32) -> Result<u32, String> {
    if filter.expression.is_empty() {
        return Err("Empty search expression".into());
    }
    let started = Instant::now();
    let budget = Duration::from_millis(u64::from(timeout_ms));
    let mut total = 0u32;
    if filter.kind == SearchKind::Regex {
        let re = regex(&filter.expression).map_err(|e| e.to_string())?;
        for text in regions {
            for found in re.find_iter(text) {
                if started.elapsed() >= budget {
                    return Err("Regex timeout".into());
                }
                let found = found.map_err(|_| "Regex timeout".to_owned())?;
                if found.start() != found.end() {
                    total = total.saturating_add(1);
                }
            }
            if started.elapsed() >= budget {
                return Err("Regex timeout".into());
            }
        }
    } else {
        let needle = if filter.case_sensitive {
            filter.expression.clone()
        } else {
            filter.expression.to_lowercase()
        };
        for text in regions {
            let text = if filter.case_sensitive {
                text.clone()
            } else {
                text.to_lowercase()
            };
            total = total
                .saturating_add(u32::try_from(text.matches(&needle).count()).unwrap_or(u32::MAX));
        }
    }
    Ok(total)
}

/// Validate before starting a crawl, including XPath syntax.
pub fn validate(cfg: &CustomSearchConfig) -> Vec<ConfigIssue> {
    let mut issues = Vec::new();
    let mut add = |path: String, message: String| {
        issues.push(ConfigIssue {
            code: "invalid_custom_search".into(),
            position: None,
            kind: ConfigIssueKind::InvalidValue,
            path,
            line: None,
            column: None,
            message,
        })
    };
    if cfg.filters.len() > 100 {
        add(
            "custom_search.filters".into(),
            "Maximal 100 Suchfilter".into(),
        );
    }
    let mut names = HashSet::new();
    for (i, filter) in cfg.filters.iter().enumerate() {
        let path = format!("custom_search.filters[{i}]");
        if !names.insert(filter_name(filter, i).to_lowercase()) {
            add(format!("{path}.name"), "Name ist nicht eindeutig".into());
        }
        if filter.expression.trim().is_empty() {
            add(
                format!("{path}.expression"),
                "Suchausdruck darf nicht leer sein".into(),
            );
        } else if filter.kind == SearchKind::Regex {
            if let Err(e) = regex(&filter.expression) {
                add(format!("{path}.expression"), e.to_string());
            }
        }
        if filter.scope == SearchScope::XPath {
            let xpath = filter.xpath.as_deref().unwrap_or("");
            if xpath.trim().is_empty() {
                add(format!("{path}.xpath"), "XPath erforderlich".into());
            } else if let Err(e) = Queries::default().sequence(xpath) {
                add(format!("{path}.xpath"), e.to_string());
            }
        }
    }
    issues
}

/// Summary for the Bulk Add dialog (INTEG-032). File decoding/CSV reading belongs
/// to the caller; this accepts the same single-column records as pasted lines.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct BulkAddReport {
    pub added: usize,
    pub empty: usize,
    pub duplicates: usize,
    pub over_limit: usize,
}

/// Append distinct expressions using the shared options, preserving input order.
pub fn bulk_add(
    cfg: &mut CustomSearchConfig,
    expressions: impl IntoIterator<Item = String>,
    template: &SearchFilter,
) -> BulkAddReport {
    let mut report = BulkAddReport::default();
    let mut names: HashSet<String> = cfg
        .filters
        .iter()
        .enumerate()
        .map(|(i, f)| filter_name(f, i).to_lowercase())
        .collect();
    let mut seen: HashSet<String> = cfg.filters.iter().map(|f| f.expression.clone()).collect();
    for expression in expressions {
        let expression = expression.trim().to_owned();
        if expression.is_empty() {
            report.empty += 1;
            continue;
        }
        if !seen.insert(expression.clone()) || !names.insert(expression.to_lowercase()) {
            report.duplicates += 1;
            continue;
        }
        if cfg.filters.len() >= 100 {
            report.over_limit += 1;
            continue;
        }
        cfg.filters.push(SearchFilter {
            name: expression.clone(),
            expression,
            ..template.clone()
        });
        report.added += 1;
    }
    report
}
