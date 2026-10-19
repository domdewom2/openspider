//! Mobile Alternates aus HTML und HTTP-Link-Headern (CRAWL-051).
use crate::{base, canonical::has_rel_token, dom, headers, links, ParseConfig};
use os_model::{ExtractedLink, LinkType};

fn mobile(media: &str) -> bool {
    let media = media.trim().to_ascii_lowercase();
    media == "handheld" || media.contains("max-width") || media.contains("max-device-width")
}

pub fn extract(doc: &dom::Document, base: &url::Url, _cfg: &ParseConfig) -> Vec<ExtractedLink> {
    let mut out = Vec::new();
    links::walk(doc, |el, doc_order, position| {
        let v = el.value();
        if v.name() != "link"
            || !dom::in_head(el)
            || !has_rel_token(v.attr("rel").unwrap_or_default(), "alternate")
            || !mobile(v.attr("media").unwrap_or_default())
            || v.attr("hreflang").is_some()
            || v.attr("type").is_some()
        {
            return;
        }
        let Some(raw) = v.attr("href") else {
            return;
        };
        out.push(ExtractedLink {
            link_type: LinkType::MobileAlternate,
            href_raw: raw.to_string(),
            resolved: base::resolve_string(base, &base::clean_url_text(raw)),
            path_type: base::path_type(raw),
            xpath: crate::link_position::element_path(doc, el),
            position,
            doc_order,
            in_head: true,
            ..Default::default()
        });
    });
    out
}

pub fn from_headers(
    url: &url::Url,
    headers: &[(String, String)],
    _cfg: &ParseConfig,
) -> Vec<ExtractedLink> {
    headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("link"))
        .flat_map(|(_, value)| headers::parse_link_header(value))
        .filter(|e| {
            has_rel_token(e.param("rel").unwrap_or_default(), "alternate")
                && mobile(e.param("media").unwrap_or_default())
                && e.param("hreflang").is_none()
                && e.param("type").is_none()
        })
        .map(|e| ExtractedLink {
            link_type: LinkType::MobileAlternate,
            resolved: base::resolve_string(url, &base::clean_url_text(&e.target)),
            path_type: base::path_type(&e.target),
            href_raw: e.target,
            doc_order: u32::MAX,
            ..Default::default()
        })
        .collect()
}
