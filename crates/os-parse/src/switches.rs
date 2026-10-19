//! Erfassungsschalter: bestehende gespeicherte Fakten bleiben unberührt.
use crate::ParseConfig;
use os_model::{Directive, DirectiveSource, ParsedPage};

pub fn filter_directives(directives: &mut Vec<Directive>, cfg: &ParseConfig) {
    directives.retain(|d| match d.source {
        DirectiveSource::MetaRobots => cfg.extraction.meta_robots,
        DirectiveSource::XRobotsTag => cfg.extraction.x_robots_tag,
    });
}

pub fn apply(page: &mut ParsedPage, cfg: &ParseConfig) {
    // Pseudo-Hyperlinks ersetzen die regulär extrahierte Instanz desselben Elements.
    let replaced: std::collections::HashSet<_> = page
        .links
        .iter()
        .filter(|l| !l.crawlable)
        .map(|l| l.doc_order)
        .collect();
    page.links.retain(|l| {
        !l.crawlable
            || l.link_type != os_model::LinkType::Hyperlink
            || !replaced.contains(&l.doc_order)
    });
    // Leere/Fragment-Links sind ohne Erfassungsschalter weiterhin die bisherigen Selbstlinks.
    let e = &cfg.extraction;
    for (enabled, texts) in [
        (e.page_titles, &mut page.titles),
        (e.meta_description, &mut page.meta_descriptions),
        (e.meta_keywords, &mut page.meta_keywords),
        (e.h1, &mut page.h1),
        (e.h2, &mut page.h2),
    ] {
        if !enabled {
            texts.clear();
        }
    }
    if !e.word_count {
        page.word_count = None;
    }
    if !e.text_ratio {
        page.text_ratio = None;
    }
    if !e.forms {
        page.forms.clear();
    }
    if !e.readability {
        page.ext.retain(|(k, _)| k.namespace() != "readability");
    }
}

/// Begrenzt alle entdeckbaren Verweise gemeinsam, einschließlich Ressourcen und Metadaten.
/// Der zweite Baum ist nur bei tatsächlicher Überschreitung nötig und lebt nur für diese Seite.
pub fn limit(page: &mut ParsedPage, html: &str, max: u32) {
    let total = page.links.len()
        + page.canonicals.len()
        + page.hreflang.len()
        + page.pagination.len()
        + page.refresh.len();
    let capped = max != 0 && total > max as usize;
    page.ext.push((
        os_model::ExtKey::from_static("crawl.links_truncated"),
        os_model::Value::Bool(capped),
    ));
    if !capped {
        return;
    }
    let doc = crate::dom::Document::parse(html);
    let mut positions = std::collections::HashMap::new();
    crate::links::walk(&doc, |el, order, _| {
        positions.insert(crate::dom::xpath(el), order);
    });
    let mut entries: Vec<(u32, usize, usize)> = page
        .links
        .iter()
        .enumerate()
        .map(|(i, l)| (l.doc_order, 0, i))
        .collect();
    for (category, paths) in [
        page.canonicals
            .iter()
            .map(|r| r.xpath.as_deref())
            .collect::<Vec<_>>(),
        page.hreflang.iter().map(|r| r.xpath.as_deref()).collect(),
        page.pagination.iter().map(|r| r.xpath.as_deref()).collect(),
        page.refresh.iter().map(|r| r.xpath.as_deref()).collect(),
    ]
    .into_iter()
    .enumerate()
    {
        for (i, path) in paths.into_iter().enumerate() {
            entries.push((
                path.and_then(|p| positions.get(p).copied())
                    .unwrap_or(u32::MAX),
                category + 1,
                i,
            ));
        }
    }
    entries.sort_by_key(|e| e.0);
    let keep: std::collections::HashSet<_> = entries
        .into_iter()
        .take(max as usize)
        .map(|(_, category, i)| (category, i))
        .collect();
    macro_rules! retain {
        ($list:expr, $category:expr) => {{
            let mut i = 0;
            $list.retain(|_| {
                let yes = keep.contains(&($category, i));
                i += 1;
                yes
            });
        }};
    }
    retain!(page.links, 0);
    retain!(page.canonicals, 1);
    retain!(page.hreflang, 2);
    retain!(page.pagination, 3);
    retain!(page.refresh, 4);
    page.images.retain(|im| {
        page.links.iter().any(|l| {
            l.link_type == os_model::LinkType::Image
                && l.doc_order == im.doc_order
                && l.resolved == im.resolved
        })
    });
}
