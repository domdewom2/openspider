//! Link-Entscheidung: Speichern/Crawlen-Matrix je Linktyp und Nofollow (CRAWL-038/039/044/045/046/056).
//!
//! [`decide`] ist eine reine Funktion. Sie beantwortet für einen aus einer Seite extrahierten Link, ob das
//! Ziel in der Crawl-Datei geführt wird (`store`, Tabellen sowie Inlinks/Outlinks) und ob es abgerufen wird
//! (`crawl`). Include/Exclude, Tiefe, Limits und robots.txt entscheidet die Engine (P15), nicht diese Datei.
//!
//! # Matrix (CRAWL-038 … 051)
//!
//! Jeder Linktyp hat einen eigenen Schalter `spider.*` mit `store` und `crawl`:
//!
//! | Linktyp | Schalter |
//! |---|---|
//! | `Hyperlink`, `PdfLink` | `internal` (Ziel intern) bzw. `external` (Ziel extern) |
//! | `Image` | `images` |
//! | `Css` | `css` (auch `Other`: Schriftarten aus `@font-face`, `object`/`embed`, `preload` ohne bekanntes `as`) |
//! | `JavaScript` | `javascript` |
//! | `Media` | `media` |
//! | `Iframe` | `iframes` |
//! | `Canonical`, `HttpCanonical` | `canonicals` |
//! | `RelNext`, `RelPrev` | `pagination` |
//! | `Hreflang`, `HttpHreflang` | `hreflang` |
//! | `Amp` | `amp` |
//! | `MobileAlternate` | `mobile_alternate` |
//! | `MetaRefresh`, `HttpRefresh` | `meta_refresh` |
//! | `HttpRedirect`, `JavaScriptRedirect` | immer speichern und crawlen (die Weiterleitungskette gehört P15) |
//! | `HstsPolicy`, `XmlSitemap` | speichern, nie über den Link abrufen (HSTS ist kein Abruf; Sitemaps holt `sitemaps::ingest`) |
//!
//! Ist das Ziel **extern**, gilt zusätzlich der Schalter `external`: `store` und `crawl` des Typs werden mit
//! denen von `external` verknüpft (CRAWL-045: „gilt auch für externe Bilder, CSS, JS, Hreflang- und
//! Canonical-Ziele“; CRAWL-039: externe Bilder nur bei „Externe Links › Crawlen“).
//!
//! # Nicht crawlbare Links (K-35)
//!
//! `ExtractedLink.resolved == None` heißt: kein abrufbares Ziel (`mailto:`, `tel:`, `javascript:`, `data:`,
//! `about:`, `file:`, nicht http(s)). Es gibt kein Crawlability-Feld (CRAWL-053 ist M2); solche Links ergeben
//! `store = false, crawl = false` ohne Grund.
//!
//! # Nofollow (CRAWL-056)
//!
//! Gilt für die Navigationslinks `Hyperlink` und `PdfLink`. Ein Link wird nicht verfolgt, wenn sein `rel`
//! `nofollow`, `sponsored` oder `ugc` enthält (`ExtractedLink::nofollow_rel`) oder die Seite meta-/header-seitig
//! nofollow ist (`page_follow == false`). Die Optionen `follow_internal_nofollow` / `follow_external_nofollow`
//! heben beides für interne bzw. externe Ziele auf. Der Link bleibt in jedem Fall mit `follow = false`
//! gespeichert ([`link_follows`]); verfolgt wird er trotzdem, wenn die Option es verlangt. Andere Linktypen
//! (Bilder, CSS, Canonical …) sind von nofollow nicht betroffen.
//!
//! # Gründe (`LinkDecision::reason`)
//!
//! Nur gesetzt, wenn das Ziel gespeichert, aber nicht abgerufen wird:
//! `ExternalNotChecked` (externes Ziel, Schalter aus), `StoreOnly` (internes Ziel, Crawlen des Typs aus),
//! `Nofollow` (Crawlen wäre erlaubt, aber der Link wird nicht verfolgt). Ist der Schalter des Typs aus, gewinnt er
//! gegenüber `Nofollow`.

use os_model::{CrawlConfig, ExtractedLink, LinkType, NotCrawledReason, StoreCrawl};
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

impl LinkDecision {
    /// Weder speichern noch abrufen, kein Grund (Ziel existiert für die Tabellen nicht).
    const DROPPED: LinkDecision = LinkDecision {
        store: false,
        crawl: false,
        reason: None,
    };
}

/// hreflang-Ziele bei „Speichern an, Crawlen aus“ (TECH-071, K-62, Paritätsbefund N-08): Eine URL, die nur per
/// hreflang bekannt ist, hat keine Tabellen-Sichtbarkeit und ihre hreflang-Linkzeile fehlt in den Link-Exporten.
/// Die Zeile der URL entsteht trotzdem (die hreflang-Spalten der Quellseite verweisen auf sie); ist das Ziel
/// anderweitig bekannt und in den Tabellen, bleibt alles wie bisher. Gilt nur für `Hreflang` und `HttpHreflang`;
/// alle anderen Typen behalten „in den Tabellen, nicht angefragt“.
pub fn is_hreflang_store_only(link_type: LinkType, d: &LinkDecision) -> bool {
    matches!(link_type, LinkType::Hreflang | LinkType::HttpHreflang) && d.store && !d.crawl
}

/// `Folgen`-Wert des Links (Spalte `follow`): `false`, wenn `rel` nofollow/sponsored/ugc enthält oder die Seite
/// nofollow ist. Unabhängig von den Folgen-Optionen (CRAWL-056, TECH-067: die Anzeige bleibt `false`).
pub fn link_follows(link: &ExtractedLink, page_follow: bool) -> bool {
    page_follow && !link.nofollow_rel
}

/// Entscheidet, ob das Ziel eines Links gespeichert und/oder gecrawlt wird.
///
/// `page_follow` ist `false`, wenn die Quellseite per Meta-Robots oder `X-Robots-Tag` nofollow ist
/// (`ParsedPage::effective_follow`).
pub fn decide(
    link: &ExtractedLink,
    scope: ScopeClass,
    cfg: &CrawlConfig,
    page_follow: bool,
) -> LinkDecision {
    if !has_http_target(link) {
        return LinkDecision::DROPPED;
    }

    let sw = switch(link.link_type, scope, cfg);
    if !sw.store && !sw.crawl {
        return LinkDecision::DROPPED;
    }
    if !sw.crawl {
        return LinkDecision {
            store: sw.store,
            crawl: false,
            reason: sw.store.then(|| switch_off_reason(scope)),
        };
    }

    if is_navigation(link.link_type) && !link_follows(link, page_follow) {
        let lifted = match scope {
            ScopeClass::Internal => cfg.spider.follow_internal_nofollow,
            ScopeClass::External => cfg.spider.follow_external_nofollow,
        };
        if !lifted {
            return LinkDecision {
                store: sw.store,
                crawl: false,
                reason: sw.store.then_some(NotCrawledReason::Nofollow),
            };
        }
    }

    LinkDecision {
        store: sw.store,
        crawl: true,
        reason: None,
    }
}

/// Der Link hat ein abrufbares http(s)-Ziel.
fn has_http_target(link: &ExtractedLink) -> bool {
    link.resolved.as_deref().is_some_and(|r| {
        let lower = |n: usize| r.get(..n).map(str::to_ascii_lowercase);
        lower(7).as_deref() == Some("http://") || lower(8).as_deref() == Some("https://")
    })
}

/// Navigationslinks, auf die nofollow wirkt.
fn is_navigation(t: LinkType) -> bool {
    matches!(t, LinkType::Hyperlink | LinkType::PdfLink)
}

/// Grund, wenn das Crawlen des Typs (bzw. bei externen Zielen von „Extern“) ausgeschaltet ist.
fn switch_off_reason(scope: ScopeClass) -> NotCrawledReason {
    match scope {
        ScopeClass::Internal => NotCrawledReason::StoreOnly,
        ScopeClass::External => NotCrawledReason::ExternalNotChecked,
    }
}

/// Wirksamer Schalter eines Linktyps für das Ziel im Bereich `scope`.
fn switch(t: LinkType, scope: ScopeClass, cfg: &CrawlConfig) -> StoreCrawl {
    let s = &cfg.spider;
    let own = match t {
        LinkType::Hyperlink | LinkType::PdfLink => match scope {
            ScopeClass::Internal => return s.internal,
            ScopeClass::External => return s.external,
        },
        LinkType::Image => s.images,
        // Schriftarten (`@font-face`), `object`/`embed` und `preload` kommen als `Other`; sie hängen am CSS-Schalter.
        LinkType::Css | LinkType::Other => s.css,
        LinkType::JavaScript => s.javascript,
        LinkType::Media => s.media,
        LinkType::Iframe => s.iframes,
        LinkType::Canonical | LinkType::HttpCanonical => s.canonicals,
        LinkType::RelNext | LinkType::RelPrev => s.pagination,
        LinkType::Hreflang | LinkType::HttpHreflang => s.hreflang,
        LinkType::Amp => s.amp,
        LinkType::MobileAlternate => s.mobile_alternate,
        LinkType::MetaRefresh | LinkType::HttpRefresh => s.meta_refresh,
        LinkType::HttpRedirect | LinkType::JavaScriptRedirect => StoreCrawl::ON,
        LinkType::HstsPolicy | LinkType::XmlSitemap => return StoreCrawl::STORE_ONLY,
    };
    match scope {
        ScopeClass::Internal => own,
        ScopeClass::External => StoreCrawl {
            store: own.store && s.external.store,
            crawl: own.crawl && s.external.crawl,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_model::{LinkPosition, PathType};

    fn link(link_type: LinkType, nofollow: bool) -> ExtractedLink {
        ExtractedLink {
            link_type,
            href_raw: "/a".to_string(),
            resolved: Some("https://site.test/a".to_string()),
            anchor: None,
            alt_text: None,
            rel: nofollow.then(|| "nofollow".to_string()),
            nofollow_rel: nofollow,
            target: None,
            path_type: PathType::RootRelative,
            xpath: "/html[1]/body[1]/a[1]".to_string(),
            position: LinkPosition::Content,
            in_head: false,
            doc_order: 0,
        }
    }

    const STORE_AND_CRAWL: LinkDecision = LinkDecision {
        store: true,
        crawl: true,
        reason: None,
    };

    /// Alle Linktypen mit eigenem Schalter und dem Zugriff darauf.
    type Slot = fn(&mut os_model::SpiderConfig) -> &mut StoreCrawl;
    fn slots() -> Vec<(Vec<LinkType>, Slot)> {
        vec![
            (vec![LinkType::Hyperlink, LinkType::PdfLink], |s| {
                &mut s.internal
            }),
            (vec![LinkType::Image], |s| &mut s.images),
            (vec![LinkType::Css, LinkType::Other], |s| &mut s.css),
            (vec![LinkType::JavaScript], |s| &mut s.javascript),
            (vec![LinkType::Media], |s| &mut s.media),
            (vec![LinkType::Iframe], |s| &mut s.iframes),
            (vec![LinkType::Canonical, LinkType::HttpCanonical], |s| {
                &mut s.canonicals
            }),
            (vec![LinkType::RelNext, LinkType::RelPrev], |s| {
                &mut s.pagination
            }),
            (vec![LinkType::Hreflang, LinkType::HttpHreflang], |s| {
                &mut s.hreflang
            }),
            (vec![LinkType::Amp], |s| &mut s.amp),
            (vec![LinkType::MobileAlternate], |s| &mut s.mobile_alternate),
            (vec![LinkType::MetaRefresh, LinkType::HttpRefresh], |s| {
                &mut s.meta_refresh
            }),
        ]
    }

    fn sc(store: bool, crawl: bool) -> StoreCrawl {
        StoreCrawl { store, crawl }
    }

    // spec: CRAWL-038
    #[test]
    fn matrix_covers_all_four_switch_combinations_for_every_type() {
        for (types, slot) in slots() {
            for t in types {
                for (store, crawl) in [(true, true), (true, false), (false, true), (false, false)] {
                    let mut cfg = CrawlConfig::default();
                    *slot(&mut cfg.spider) = sc(store, crawl);
                    let d = decide(&link(t, false), ScopeClass::Internal, &cfg, true);
                    let expected = match (store, crawl) {
                        (true, true) => STORE_AND_CRAWL,
                        // Speichern an, Crawlen aus: in den Tabellen, nicht angefragt.
                        (true, false) => LinkDecision {
                            store: true,
                            crawl: false,
                            reason: Some(NotCrawledReason::StoreOnly),
                        },
                        // Speichern aus, Crawlen an: angefragt, nicht in den Tabellen.
                        (false, true) => LinkDecision {
                            store: false,
                            crawl: true,
                            reason: None,
                        },
                        // Beide aus: weder angefragt noch angezeigt.
                        (false, false) => LinkDecision::DROPPED,
                    };
                    assert_eq!(d, expected, "{t:?} store={store} crawl={crawl}");
                }
            }
        }
    }

    // spec: TECH-071
    #[test]
    fn only_hreflang_types_with_store_on_and_crawl_off_are_hidden_from_the_tables() {
        // K-62: die Matrix-Entscheidung bleibt „speichern, nicht abrufen“; der Worker legt für diese Fälle eine
        // Zeile ohne Tabellen-Sichtbarkeit und keine Linkzeile an.
        let store_only = LinkDecision {
            store: true,
            crawl: false,
            reason: Some(NotCrawledReason::StoreOnly),
        };
        assert!(is_hreflang_store_only(LinkType::Hreflang, &store_only));
        assert!(is_hreflang_store_only(LinkType::HttpHreflang, &store_only));
        for t in [
            LinkType::Canonical,
            LinkType::RelNext,
            LinkType::Amp,
            LinkType::MobileAlternate,
            LinkType::MetaRefresh,
            LinkType::Hyperlink,
        ] {
            assert!(!is_hreflang_store_only(t, &store_only), "{t:?}");
        }
        assert!(!is_hreflang_store_only(
            LinkType::Hreflang,
            &STORE_AND_CRAWL
        ));
        let mut cfg = CrawlConfig::default();
        cfg.spider.hreflang = StoreCrawl::STORE_ONLY;
        let d = decide(
            &link(LinkType::Hreflang, false),
            ScopeClass::Internal,
            &cfg,
            true,
        );
        assert!(is_hreflang_store_only(LinkType::Hreflang, &d));
    }

    // spec: CRAWL-038
    #[test]
    fn switch_changes_apply_to_every_later_decision() {
        // Änderungen an der Konfiguration (auch während einer Pause) wirken auf jede spätere Entscheidung,
        // weil `decide` keinen Zustand hält.
        let l = link(LinkType::Image, false);
        let mut cfg = CrawlConfig::default();
        assert_eq!(
            decide(&l, ScopeClass::Internal, &cfg, true),
            STORE_AND_CRAWL
        );
        cfg.spider.images = StoreCrawl::STORE_ONLY;
        assert!(!decide(&l, ScopeClass::Internal, &cfg, true).crawl);
        cfg.spider.images = StoreCrawl::ON;
        assert_eq!(
            decide(&l, ScopeClass::Internal, &cfg, true),
            STORE_AND_CRAWL
        );
    }

    // spec: CRAWL-039
    #[test]
    fn images_follow_the_image_switch_and_external_images_need_external_crawl() {
        let cfg = CrawlConfig::default();
        let img = link(LinkType::Image, false);
        assert_eq!(
            decide(&img, ScopeClass::Internal, &cfg, true),
            STORE_AND_CRAWL
        );
        assert_eq!(
            decide(&img, ScopeClass::External, &cfg, true),
            STORE_AND_CRAWL
        );

        let mut cfg = CrawlConfig::default();
        cfg.spider.external = StoreCrawl::STORE_ONLY;
        assert_eq!(
            decide(&img, ScopeClass::External, &cfg, true),
            LinkDecision {
                store: true,
                crawl: false,
                reason: Some(NotCrawledReason::ExternalNotChecked)
            }
        );
        // Interne Bilder bleiben unberührt.
        assert_eq!(
            decide(&img, ScopeClass::Internal, &cfg, true),
            STORE_AND_CRAWL
        );
        // Ein per <a href> verlinktes Bild ist ein Hyperlink und folgt dem Hyperlink-Schalter.
        let mut cfg = CrawlConfig::default();
        cfg.spider.images = StoreCrawl::OFF;
        assert_eq!(
            decide(
                &link(LinkType::Hyperlink, false),
                ScopeClass::Internal,
                &cfg,
                true
            ),
            STORE_AND_CRAWL
        );
    }

    // spec: CRAWL-044
    #[test]
    fn internal_hyperlinks_follow_the_internal_switch() {
        let l = link(LinkType::Hyperlink, false);
        let mut cfg = CrawlConfig::default();
        assert_eq!(
            decide(&l, ScopeClass::Internal, &cfg, true),
            STORE_AND_CRAWL
        );
        cfg.spider.internal = StoreCrawl::STORE_ONLY;
        // „Crawlen aus: nur die Start-URL wird angefragt“ – kein Interner Link wird abgerufen.
        assert_eq!(
            decide(&l, ScopeClass::Internal, &cfg, true),
            LinkDecision {
                store: true,
                crawl: false,
                reason: Some(NotCrawledReason::StoreOnly)
            }
        );
        // Der externe Schalter ist davon unabhängig.
        assert_eq!(
            decide(&l, ScopeClass::External, &cfg, true),
            STORE_AND_CRAWL
        );
    }

    // spec: CRAWL-044, CRAWL-038
    #[test]
    fn links_without_http_target_are_dropped() {
        let cfg = CrawlConfig::default();
        let mut l = link(LinkType::Hyperlink, false);
        l.resolved = None; // mailto:, tel:, javascript:, data:, about:, file: (K-35)
        for scope in [ScopeClass::Internal, ScopeClass::External] {
            assert_eq!(decide(&l, scope, &cfg, true), LinkDecision::DROPPED);
        }
        // Auch ein (unerwartet) aufgelöster Nicht-http(s)-Wert wird nicht abgerufen.
        for odd in [
            "mailto:a@site.test",
            "ftp://site.test/x",
            "javascript:void(0)",
        ] {
            let mut l = link(LinkType::Image, false);
            l.resolved = Some(odd.to_string());
            assert_eq!(
                decide(&l, ScopeClass::Internal, &cfg, true),
                LinkDecision::DROPPED,
                "{odd}"
            );
        }
        // http(s) in beliebiger Schreibweise zählt.
        let mut l = link(LinkType::Hyperlink, false);
        l.resolved = Some("HTTPS://site.test/x".to_string());
        assert!(decide(&l, ScopeClass::Internal, &cfg, true).crawl);
    }

    // spec: CRAWL-045
    #[test]
    fn external_links_follow_the_external_switch_for_all_target_types() {
        let mut cfg = CrawlConfig::default();
        cfg.spider.external = StoreCrawl::STORE_ONLY;
        // „Crawlen aus: keine einzige Anfrage an externe Hosts; externe URLs erscheinen ohne Statuscode.“
        for t in [
            LinkType::Hyperlink,
            LinkType::Image,
            LinkType::Css,
            LinkType::JavaScript,
            LinkType::Hreflang,
            LinkType::Canonical,
            LinkType::Media,
            LinkType::Iframe,
        ] {
            assert_eq!(
                decide(&link(t, false), ScopeClass::External, &cfg, true),
                LinkDecision {
                    store: true,
                    crawl: false,
                    reason: Some(NotCrawledReason::ExternalNotChecked)
                },
                "{t:?}"
            );
        }
        // Speichern extern aus, Crawlen an: angefragt, nicht in den Tabellen.
        cfg.spider.external = sc(false, true);
        assert_eq!(
            decide(
                &link(LinkType::Hyperlink, false),
                ScopeClass::External,
                &cfg,
                true
            ),
            LinkDecision {
                store: false,
                crawl: true,
                reason: None
            }
        );
        // Beide aus.
        cfg.spider.external = StoreCrawl::OFF;
        assert_eq!(
            decide(
                &link(LinkType::Image, false),
                ScopeClass::External,
                &cfg,
                true
            ),
            LinkDecision::DROPPED
        );
    }

    // spec: CRAWL-045
    #[test]
    fn external_resource_needs_both_the_type_switch_and_the_external_switch() {
        let mut cfg = CrawlConfig::default();
        cfg.spider.images = StoreCrawl::STORE_ONLY;
        let img = link(LinkType::Image, false);
        // Typ-Schalter aus, extern an: nicht abgerufen.
        assert_eq!(
            decide(&img, ScopeClass::External, &cfg, true),
            LinkDecision {
                store: true,
                crawl: false,
                reason: Some(NotCrawledReason::ExternalNotChecked)
            }
        );
        // Standard (hreflang nur speichern): externes Hreflang-Ziel wird nicht abgerufen.
        let cfg = CrawlConfig::default();
        assert!(
            !decide(
                &link(LinkType::Hreflang, false),
                ScopeClass::External,
                &cfg,
                true
            )
            .crawl
        );
    }

    // spec: CRAWL-046
    #[test]
    fn canonicals_follow_the_canonical_switch() {
        for t in [LinkType::Canonical, LinkType::HttpCanonical] {
            let l = link(t, false);
            let mut cfg = CrawlConfig::default();
            assert_eq!(
                decide(&l, ScopeClass::Internal, &cfg, true),
                STORE_AND_CRAWL
            );
            // Crawlen aus: nicht angefragt, aber in den Spalten geführt.
            cfg.spider.canonicals = StoreCrawl::STORE_ONLY;
            let d = decide(&l, ScopeClass::Internal, &cfg, true);
            assert!(d.store && !d.crawl, "{t:?}");
            assert_eq!(d.reason, Some(NotCrawledReason::StoreOnly));
            // Speichern aus: Spalten bleiben leer (kein Canonical-Eintrag).
            cfg.spider.canonicals = sc(false, false);
            assert!(!decide(&l, ScopeClass::Internal, &cfg, true).store);
        }
    }

    // spec: CRAWL-046
    #[test]
    fn canonical_with_nofollow_rel_token_is_still_crawled() {
        // `rel="canonical nofollow"` wird als Canonical erkannt; nofollow betrifft nur Navigationslinks.
        let l = link(LinkType::Canonical, true);
        assert_eq!(
            decide(&l, ScopeClass::Internal, &CrawlConfig::default(), true),
            STORE_AND_CRAWL
        );
    }

    // spec: CRAWL-038
    #[test]
    fn default_switches_match_the_spec_defaults() {
        let cfg = CrawlConfig::default();
        let on = |t| decide(&link(t, false), ScopeClass::Internal, &cfg, true);
        for t in [
            LinkType::Hyperlink,
            LinkType::Image,
            LinkType::Css,
            LinkType::JavaScript,
            LinkType::Media,
            LinkType::Iframe,
            LinkType::Canonical,
            LinkType::MetaRefresh,
        ] {
            assert_eq!(on(t), STORE_AND_CRAWL, "{t:?}");
        }
        // Speichern an, Crawlen aus: Hreflang, Paginierung, Mobile Alternate.
        for t in [
            LinkType::Hreflang,
            LinkType::HttpHreflang,
            LinkType::RelNext,
            LinkType::RelPrev,
            LinkType::MobileAlternate,
        ] {
            let d = on(t);
            assert!(d.store && !d.crawl, "{t:?}");
        }
        // AMP: standardmäßig weder gespeichert noch angefragt.
        assert_eq!(on(LinkType::Amp), LinkDecision::DROPPED);
    }

    // spec: CRAWL-038
    #[test]
    fn redirect_hsts_and_sitemap_types_are_not_switchable() {
        let mut cfg = CrawlConfig::default();
        cfg.spider.internal = StoreCrawl::OFF;
        cfg.spider.meta_refresh = StoreCrawl::OFF;
        assert_eq!(
            decide(
                &link(LinkType::HttpRedirect, false),
                ScopeClass::Internal,
                &cfg,
                true
            ),
            STORE_AND_CRAWL
        );
        for t in [LinkType::HstsPolicy, LinkType::XmlSitemap] {
            assert_eq!(
                decide(&link(t, false), ScopeClass::Internal, &cfg, true),
                LinkDecision {
                    store: true,
                    crawl: false,
                    reason: Some(NotCrawledReason::StoreOnly)
                },
                "{t:?}"
            );
        }
        // Externe Weiterleitungsziele folgen dem Schalter „Extern“.
        cfg.spider.external = StoreCrawl::STORE_ONLY;
        assert!(
            !decide(
                &link(LinkType::HttpRedirect, false),
                ScopeClass::External,
                &cfg,
                true
            )
            .crawl
        );
    }

    // spec: CRAWL-056
    #[test]
    fn nofollow_rel_is_stored_but_not_followed_by_default() {
        let cfg = CrawlConfig::default();
        let want = LinkDecision {
            store: true,
            crawl: false,
            reason: Some(NotCrawledReason::Nofollow),
        };
        for scope in [ScopeClass::Internal, ScopeClass::External] {
            assert_eq!(
                decide(&link(LinkType::Hyperlink, true), scope, &cfg, true),
                want,
                "{scope:?}"
            );
            assert_eq!(
                decide(&link(LinkType::PdfLink, true), scope, &cfg, true),
                want,
                "{scope:?}"
            );
        }
    }

    // spec: CRAWL-056
    #[test]
    fn page_level_nofollow_stops_all_hyperlinks_of_the_page() {
        let cfg = CrawlConfig::default();
        let d = decide(
            &link(LinkType::Hyperlink, false),
            ScopeClass::Internal,
            &cfg,
            false,
        );
        assert_eq!(
            d,
            LinkDecision {
                store: true,
                crawl: false,
                reason: Some(NotCrawledReason::Nofollow)
            }
        );
        // Ressourcen derselben Seite werden weiter abgerufen.
        assert_eq!(
            decide(
                &link(LinkType::Image, false),
                ScopeClass::Internal,
                &cfg,
                false
            ),
            STORE_AND_CRAWL
        );
        assert_eq!(
            decide(
                &link(LinkType::Css, false),
                ScopeClass::Internal,
                &cfg,
                false
            ),
            STORE_AND_CRAWL
        );
    }

    // spec: CRAWL-056
    #[test]
    fn follow_options_are_separate_for_internal_and_external() {
        let mut cfg = CrawlConfig::default();
        cfg.spider.follow_internal_nofollow = true;
        let nf = link(LinkType::Hyperlink, true);
        assert_eq!(
            decide(&nf, ScopeClass::Internal, &cfg, true),
            STORE_AND_CRAWL
        );
        assert!(!decide(&nf, ScopeClass::External, &cfg, true).crawl);
        // Die Option hebt auch Seiten-nofollow für interne Ziele auf.
        assert_eq!(
            decide(
                &link(LinkType::Hyperlink, false),
                ScopeClass::Internal,
                &cfg,
                false
            ),
            STORE_AND_CRAWL
        );

        let mut cfg = CrawlConfig::default();
        cfg.spider.follow_external_nofollow = true;
        assert_eq!(
            decide(&nf, ScopeClass::External, &cfg, true),
            STORE_AND_CRAWL
        );
        assert!(!decide(&nf, ScopeClass::Internal, &cfg, true).crawl);
    }

    // spec: CRAWL-056
    #[test]
    fn follow_option_does_not_override_a_disabled_crawl_switch() {
        let mut cfg = CrawlConfig::default();
        cfg.spider.follow_internal_nofollow = true;
        cfg.spider.internal = StoreCrawl::STORE_ONLY;
        let d = decide(
            &link(LinkType::Hyperlink, true),
            ScopeClass::Internal,
            &cfg,
            true,
        );
        assert_eq!(
            d,
            LinkDecision {
                store: true,
                crawl: false,
                reason: Some(NotCrawledReason::StoreOnly)
            }
        );
    }

    // spec: CRAWL-056
    #[test]
    fn nofollow_without_store_drops_the_link() {
        let mut cfg = CrawlConfig::default();
        cfg.spider.internal = sc(false, true);
        let d = decide(
            &link(LinkType::Hyperlink, true),
            ScopeClass::Internal,
            &cfg,
            true,
        );
        assert_eq!(d, LinkDecision::DROPPED);
    }

    // spec: CRAWL-056
    #[test]
    fn follow_column_stays_false_even_when_nofollow_is_followed() {
        let mut cfg = CrawlConfig::default();
        cfg.spider.follow_internal_nofollow = true;
        let nf = link(LinkType::Hyperlink, true);
        assert!(decide(&nf, ScopeClass::Internal, &cfg, true).crawl);
        assert!(!link_follows(&nf, true));
        assert!(link_follows(&link(LinkType::Hyperlink, false), true));
        assert!(!link_follows(&link(LinkType::Hyperlink, false), false));
    }

    // spec: CRAWL-038
    #[test]
    fn decide_is_total_over_all_types_scopes_and_flags() {
        // Kein Panic, und ein Grund steht nur bei „gespeichert, nicht abgerufen“.
        let all = [
            LinkType::Hyperlink,
            LinkType::Image,
            LinkType::Css,
            LinkType::JavaScript,
            LinkType::Media,
            LinkType::Iframe,
            LinkType::Canonical,
            LinkType::HttpCanonical,
            LinkType::RelNext,
            LinkType::RelPrev,
            LinkType::Hreflang,
            LinkType::HttpHreflang,
            LinkType::Amp,
            LinkType::MobileAlternate,
            LinkType::MetaRefresh,
            LinkType::HttpRefresh,
            LinkType::HttpRedirect,
            LinkType::HstsPolicy,
            LinkType::JavaScriptRedirect,
            LinkType::XmlSitemap,
            LinkType::PdfLink,
            LinkType::Other,
        ];
        let cfg = CrawlConfig::default();
        for t in all {
            for scope in [ScopeClass::Internal, ScopeClass::External] {
                for nf in [false, true] {
                    for pf in [false, true] {
                        let d = decide(&link(t, nf), scope, &cfg, pf);
                        assert_eq!(
                            d.reason.is_some(),
                            d.store && !d.crawl,
                            "{t:?} {scope:?} {nf} {pf}"
                        );
                    }
                }
            }
        }
    }
}
