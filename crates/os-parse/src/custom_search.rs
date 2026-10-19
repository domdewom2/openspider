//! Custom Search: decoded HTML, text regions and XPath (INTEG-020 … 035).
//!
//! Head/body HTML is reserialized via `inner_html()` from the repaired HTML5 DOM
//! (INTEG-024/025); entity spelling and attribute order can differ from the source.
//! Content Area uses `content::area_text`, sharing ONPAGE-140 rules and traversal
//! with word count. Regions, their lowercase text and the XPath tree are cached per page.

use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use ego_tree::iter::Edge;
use fancy_regex::{Regex, RegexBuilder};
use os_model::{
    ConfigIssue, ConfigIssueKind, ContentAreaConfig, CustomSearchConfig, ParsedPage, SearchFilter,
    SearchHit, SearchKind, SearchScope,
};
use scraper::{ElementRef, Node, Selector};
use xee_xpath::{Documents, Item, Queries, Query};

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

fn regex(expression: &str, region_bytes: usize) -> Result<Regex, fancy_regex::Error> {
    // A find() includes unsuccessful start positions in its backtrack count.
    // Keep at least the library default, growing with input size so ordinary
    // lookarounds on large pages do not exhaust a tiny fixed cap. The cap still
    // bounds a single find(), which wall-clock checks cannot interrupt.
    RegexBuilder::new(expression)
        .backtrack_limit(1_000_000.max(region_bytes.saturating_mul(8)))
        .build()
}

struct Region {
    original: String,
    lower: OnceCell<String>,
}

impl Region {
    fn new(original: String) -> Self {
        Self {
            original,
            lower: OnceCell::new(),
        }
    }

    fn text(&self, case_sensitive: bool) -> &str {
        if case_sensitive {
            &self.original
        } else {
            self.lower.get_or_init(|| self.original.to_lowercase())
        }
    }
}

fn evaluate(doc: &Document, html: &str, cfg: &crate::ParseConfig) -> Vec<SearchHit> {
    let started = Instant::now();
    let budget = Duration::from_millis(u64::from(cfg.custom_search.regex_timeout_ms));
    let mut regions_by_scope = HashMap::new();
    let mut xpath = None;
    cfg.custom_search
        .filters
        .iter()
        .enumerate()
        .map(|(i, filter)| {
            let key = (
                filter.scope,
                if filter.scope == SearchScope::XPath {
                    filter.xpath.clone()
                } else {
                    None
                },
            );
            let regions = regions_by_scope.entry(key).or_insert_with(|| {
                regions(doc, html, &cfg.content_area, filter, &mut xpath)
                    .map(|texts| texts.into_iter().map(Region::new).collect::<Vec<_>>())
            });
            let result = match regions {
                Ok(regions) => count(regions, filter, started, budget),
                Err(error) => Err(error.clone()),
            };
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
    xpath: &mut Option<Result<XPathDocument, String>>,
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
        SearchScope::PageText => vec![visible_text(doc, false)],
        SearchScope::PageTextNoAnchors => vec![visible_text(doc, true)],
        SearchScope::ContentArea => vec![crate::content::area_text(doc, area)],
        SearchScope::XPath => match xpath.get_or_insert_with(|| XPathDocument::new(doc)) {
            Ok(xpath) => xpath.text(filter.xpath.as_deref().unwrap_or(""))?,
            Err(error) => return Err(error.clone()),
        },
    })
}

fn visible_text(doc: &Document, no_anchors: bool) -> String {
    let Some(body) = doc.html().select(&Selector::parse("body").unwrap()).next() else {
        return String::new();
    };
    let mut out = String::new();
    let mut skip = 0usize;
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
                    {
                        skip = 1;
                        continue;
                    }
                }
                Node::Text(text) if skip == 0 => out.push_str(text),
                _ => {}
            },
            Edge::Close(node) => {
                if let Node::Element(el) = node.value() {
                    skip = skip.saturating_sub(1);
                    if skip == 0 && crate::dom::is_block(el.name()) {
                        out.push(' ');
                    }
                }
            }
        }
    }
    crate::text::normalize_ws(&out)
}

struct XPathDocument {
    documents: Documents,
    root: Item,
}

impl XPathDocument {
    fn new(doc: &Document) -> Result<Self, String> {
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
        Ok(Self {
            documents,
            root: Item::Node(root),
        })
    }

    fn text(&mut self, expression: &str) -> Result<Vec<String>, String> {
        let queries = Queries::default();
        let query = queries
            .many(&format!("({expression}) ! string(.)"), |_, item| {
                Ok(item.try_into_value::<String>()?)
            })
            .map_err(|e| e.to_string())?;
        query
            .execute(&mut self.documents, &self.root)
            .map_err(|e| e.to_string())
    }
}

fn count(
    regions: &[Region],
    filter: &SearchFilter,
    started: Instant,
    budget: Duration,
) -> Result<u32, String> {
    if filter.expression.is_empty() {
        return Err("Empty search expression".into());
    }
    let mut total = 0u32;
    if filter.kind == SearchKind::Regex {
        if started.elapsed() >= budget {
            return Err("Regex timeout".into());
        }
        let region_bytes = regions.iter().map(|r| r.original.len()).max().unwrap_or(0);
        let re = regex(&filter.expression, region_bytes).map_err(|e| e.to_string())?;
        for region in regions {
            if started.elapsed() >= budget {
                return Err("Regex timeout".into());
            }
            for found in re.find_iter(&region.original) {
                if started.elapsed() >= budget {
                    return Err("Regex timeout".into());
                }
                found.map_err(|_| "Regex timeout".to_owned())?;
                // find_iter advances after empty matches, so pure lookarounds
                // count as matches and x* still terminates (INTEG-028).
                total = total.saturating_add(1);
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
        for region in regions {
            let text = region.text(filter.case_sensitive);
            total = total
                .saturating_add(u32::try_from(text.matches(&needle).count()).unwrap_or(u32::MAX));
        }
    }
    Ok(total)
}

/// Validate before starting a crawl, including XPath syntax.
pub fn validate(cfg: &CustomSearchConfig) -> Vec<ConfigIssue> {
    let mut issues = Vec::new();
    let mut add = |code: &str, path: String, message: String| {
        issues.push(ConfigIssue {
            code: format!("invalid_custom_search.{code}"),
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
            "limit",
            "custom_search.filters".into(),
            "Maximal 100 Suchfilter".into(),
        );
    }
    let mut names = HashSet::new();
    for (i, filter) in cfg.filters.iter().enumerate() {
        let path = format!("custom_search.filters[{i}]");
        if !names.insert(filter_name(filter, i).to_lowercase()) {
            add(
                "duplicate_name",
                format!("{path}.name"),
                "Name ist nicht eindeutig".into(),
            );
        }
        if filter.expression.trim().is_empty() {
            add(
                "empty_expression",
                format!("{path}.expression"),
                "Suchausdruck darf nicht leer sein".into(),
            );
        } else if filter.kind == SearchKind::Regex {
            if let Err(e) = regex(&filter.expression, 0) {
                add("invalid_regex", format!("{path}.expression"), e.to_string());
            }
        }
        if filter.scope == SearchScope::XPath {
            let xpath = filter.xpath.as_deref().unwrap_or("");
            if xpath.trim().is_empty() {
                add(
                    "invalid_xpath",
                    format!("{path}.xpath"),
                    "XPath erforderlich".into(),
                );
            } else if let Err(e) = Queries::default().sequence(xpath) {
                add("invalid_xpath", format!("{path}.xpath"), e.to_string());
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
