//! Optionaler Katalog nicht crawlbarer Links (CRAWL-053, K2-27).
use os_model::{ExtractedLink, LinkType, UncrawlableReason};
use scraper::ElementRef;

use crate::{base, dom, links, text, ParseConfig};

fn navigates(handler: &str) -> bool {
    let handler = handler.to_ascii_lowercase();
    handler.contains("location") || handler.contains("window.open")
}

/// Die Heuristik extrahiert ausschließlich String-Literale, führt niemals JavaScript aus.
fn literal(handler: &str) -> Option<&str> {
    let start = handler.find(['\'', '"'])?;
    let quote = handler.as_bytes()[start] as char;
    let rest = &handler[start + 1..];
    let end = rest.find(quote)?;
    Some(&rest[..end])
}

fn navigation_literal(handler: &str) -> Option<&str> {
    let lower = handler.to_ascii_lowercase();
    let start = [lower.find("location"), lower.find("window.open")]
        .into_iter()
        .flatten()
        .min()?;
    literal(&handler[start..])
}

pub fn inspect(doc: &dom::Document, base: &url::Url, cfg: &ParseConfig) -> Vec<ExtractedLink> {
    if !cfg.crawl.spider.store_uncrawlable_links {
        return Vec::new();
    }
    let mut out = Vec::new();
    links::walk(doc, |el: ElementRef<'_>, doc_order, position| {
        let name = el.value().name();
        let href = el.value().attr("href");
        let onclick = el.value().attr("onclick").unwrap_or_default();
        if name == "area" {
            return;
        }
        let reason = if name != "a" {
            href.map(|_| UncrawlableReason::NonAnchorHref).or_else(|| {
                navigates(onclick).then_some(UncrawlableReason::AnchorOnclickNavigation)
            })
        } else {
            match href.map(str::trim) {
                Some(h) if h.to_ascii_lowercase().starts_with("javascript:") => {
                    Some(UncrawlableReason::JavascriptHref)
                }
                _ if onclick.to_ascii_lowercase().contains("return false") => {
                    Some(UncrawlableReason::AnchorReturnFalse)
                }
                _ if navigates(onclick) => Some(UncrawlableReason::AnchorOnclickNavigation),
                None => Some(UncrawlableReason::AnchorMissingHref),
                Some("") => Some(UncrawlableReason::AnchorEmptyHref),
                Some(h) if h.starts_with('#') && !cfg.crawl.advanced.crawl_fragments => {
                    Some(UncrawlableReason::AnchorFragmentOnly)
                }
                _ => None,
            }
        };
        let Some(reason) = reason else {
            return;
        };
        let raw = href.unwrap_or(onclick);
        let target = match reason {
            UncrawlableReason::JavascriptHref => literal(raw),
            UncrawlableReason::AnchorOnclickNavigation if href.is_none() => {
                navigation_literal(onclick)
            }
            UncrawlableReason::AnchorMissingHref => None,
            _ => href,
        };
        let resolved = target.and_then(|t| base::resolve_string(base, &base::clean_url_text(t)));
        out.push(ExtractedLink {
            link_type: LinkType::Hyperlink,
            href_raw: raw.to_string(),
            xpath: if resolved.is_some() {
                crate::link_position::element_path(doc, el)
            } else {
                raw.to_string()
            },
            resolved,
            anchor: Some(text::normalize_ws(&dom::text_content(el, true)))
                .filter(|s| !s.is_empty()),
            rel: el.value().attr("rel").map(str::to_ascii_lowercase),
            nofollow_rel: el.value().attr("rel").is_some_and(|r| {
                r.split_ascii_whitespace().any(|t| {
                    matches!(
                        t.to_ascii_lowercase().as_str(),
                        "nofollow" | "ugc" | "sponsored"
                    )
                })
            }),
            path_type: base::path_type(raw),
            position,
            in_head: dom::in_head(el),
            doc_order,
            crawlable: false,
            uncrawlable_reason: Some(reason),
            ..Default::default()
        });
    });
    out
}
