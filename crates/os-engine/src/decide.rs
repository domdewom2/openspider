//! Link-Entscheidung: Speichern/Crawlen-Matrix je Linktyp und Nofollow (CRAWL-038/039/044/045/046/056).
//!
//! STUB(M1-P15b)

use os_model::{CrawlConfig, ExtractedLink, NotCrawledReason};
use os_url::ScopeClass;

/// Entscheidung für einen Link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkDecision {
    /// Ziel in der Crawl-Datei speichern.
    pub store: bool,
    /// Ziel abrufen.
    pub crawl: bool,
    /// Grund, falls nicht abgerufen.
    pub reason: Option<NotCrawledReason>,
}

/// Entscheidet, ob das Ziel eines Links gespeichert und/oder gecrawlt wird.
// STUB(M1-P15b): interner Hyperlink mit follow → speichern und crawlen; alles andere → nur speichern.
pub fn decide(
    link: &ExtractedLink,
    scope: ScopeClass,
    cfg: &CrawlConfig,
    page_follow: bool,
) -> LinkDecision {
    let follow = page_follow && (!link.nofollow_rel || cfg.spider.follow_internal_nofollow);
    if link.link_type.is_hyperlink() && scope == ScopeClass::Internal && follow {
        LinkDecision {
            store: true,
            crawl: true,
            reason: None,
        }
    } else {
        LinkDecision {
            store: true,
            crawl: false,
            reason: Some(NotCrawledReason::CrawlDisabled),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_model::{LinkPosition, LinkType, PathType};

    fn link(link_type: LinkType, nofollow: bool) -> ExtractedLink {
        ExtractedLink {
            link_type,
            href_raw: "/a".to_string(),
            resolved: Some("https://site.test/a".to_string()),
            anchor: None,
            alt_text: None,
            rel: None,
            nofollow_rel: nofollow,
            target: None,
            path_type: PathType::RootRelative,
            xpath: "/html[1]/body[1]/a[1]".to_string(),
            position: LinkPosition::Content,
            in_head: false,
            doc_order: 0,
        }
    }

    #[test]
    fn internal_followed_hyperlink_is_stored_and_crawled() {
        let cfg = CrawlConfig::default();
        let d = decide(
            &link(LinkType::Hyperlink, false),
            ScopeClass::Internal,
            &cfg,
            true,
        );
        assert_eq!(
            d,
            LinkDecision {
                store: true,
                crawl: true,
                reason: None
            }
        );
    }

    #[test]
    fn everything_else_is_stored_only() {
        let cfg = CrawlConfig::default();
        for d in [
            decide(
                &link(LinkType::Hyperlink, false),
                ScopeClass::External,
                &cfg,
                true,
            ),
            decide(
                &link(LinkType::Image, false),
                ScopeClass::Internal,
                &cfg,
                true,
            ),
            decide(
                &link(LinkType::Hyperlink, true),
                ScopeClass::Internal,
                &cfg,
                true,
            ),
            decide(
                &link(LinkType::Hyperlink, false),
                ScopeClass::Internal,
                &cfg,
                false,
            ),
        ] {
            assert!(d.store);
            assert!(!d.crawl);
            assert_eq!(d.reason, Some(NotCrawledReason::CrawlDisabled));
        }
    }
}
