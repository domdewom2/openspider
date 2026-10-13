//! Link-Entscheidung gegen die Fixtures (CRAWL-038/039/044/045/046/056) – ohne Engine.
//!
//! Jede Fixture-Seite wird von der Testsite geholt (`HttpFetcher`), mit `os_parse::parse_html` ausgewertet, und
//! aus den Ergebnissen (Links, Canonicals, Paginierung, hreflang, Refresh) entstehen die `ExtractedLink`, die
//! `decide::decide` bekommt – so wie P15 sie bildet. Die Crawl-Läufe selbst stehen in `decide_crawl.rs`.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use os_engine::decide::{decide, link_follows, LinkDecision};
use os_fetch::{
    Fetch, FetchConfig, FetchOutcome, FetchPurpose, FetchRequest, HttpFetcher, MemorySecretStore,
};
use os_model::{
    CrawlConfig, ExtractedLink, FixedClock, HintSource, LinkPosition, LinkType, NotCrawledReason,
    ParsedPage, PathType, RefreshSource, SpiderConfig, StoreCrawl, Timestamp,
};
use os_parse::{parse_html, ParseConfig, ParseInput};
use os_testsite::TestSite;
use os_url::{normalize, parse_start_url, Scope, ScopeClass};
use url::Url;

struct Fixture {
    site: TestSite,
    fetcher: HttpFetcher,
    cfg: CrawlConfig,
    scope: Scope,
    base: String,
}

impl Fixture {
    async fn start(scenario: &str) -> Fixture {
        let site = TestSite::start(scenario)
            .await
            .unwrap_or_else(|e| panic!("Testsite {scenario}: {e}"));
        let mut cfg = CrawlConfig::default();
        cfg.network.resolve.push(format!("*.test={}", site.addr()));
        let clock = Arc::new(FixedClock::new(
            Timestamp::from_db("2026-01-01T00:00:00.000Z").unwrap(),
            Duration::from_millis(1),
        ));
        let fetcher = HttpFetcher::new(
            FetchConfig::from_crawl_config(&cfg, &MemorySecretStore::new(), clock).unwrap(),
        )
        .unwrap();
        let start = parse_start_url(&site.url("/")).unwrap();
        let scope = Scope::new(&start, &cfg.scope);
        let base = site.url("");
        Fixture {
            site,
            fetcher,
            cfg,
            scope,
            base,
        }
    }

    /// Holt eine Seite und wertet sie aus; der Statuscode muss 200 sein.
    async fn page(&self, path: &str) -> ParsedPage {
        let url = Url::parse(&self.site.url(path)).unwrap();
        let result = self
            .fetcher
            .fetch(FetchRequest {
                url: url.clone(),
                purpose: FetchPurpose::Page,
                cookies: None,
                max_body_bytes: 0,
            })
            .await;
        let FetchOutcome::Response(r) = result.outcome else {
            panic!("keine Antwort für {path}");
        };
        assert_eq!(r.status, 200, "{path}");
        let content_type = r
            .headers
            .iter()
            .find(|(n, _)| n == "content-type")
            .map(|(_, v)| v.clone());
        let cfg = ParseConfig::from_crawl_config(&self.cfg);
        parse_html(&ParseInput {
            url: &url,
            body: &r.body,
            content_type: content_type.as_deref(),
            headers: &r.headers,
            cfg: &cfg,
        })
    }

    fn class(&self, resolved: &str) -> ScopeClass {
        self.scope
            .classify(&normalize(&Url::parse(resolved).unwrap()))
    }

    /// Pfad (intern) bzw. volle URL (extern) ohne Fragment.
    fn short(&self, resolved: &str) -> String {
        let n = normalize(&Url::parse(resolved).unwrap()).encoded;
        match n.strip_prefix(self.base.trim_end_matches('/')) {
            Some(path) if !path.is_empty() => path.to_string(),
            Some(_) => "/".to_string(),
            None => n,
        }
    }

    /// Entscheidung je Link (`resolved` vorhanden oder nicht).
    fn decisions(
        &self,
        links: &[ExtractedLink],
        cfg: &CrawlConfig,
        page_follow: bool,
    ) -> Vec<(ExtractedLink, LinkDecision)> {
        links
            .iter()
            .map(|l| {
                let scope = l
                    .resolved
                    .as_deref()
                    .map_or(ScopeClass::External, |r| self.class(r));
                (l.clone(), decide(l, scope, cfg, page_follow))
            })
            .collect()
    }

    /// Abzurufende Ziele (ohne die Seite selbst).
    fn crawl_targets(
        &self,
        links: &[ExtractedLink],
        cfg: &CrawlConfig,
        page_follow: bool,
        page: &str,
    ) -> BTreeSet<String> {
        self.decisions(links, cfg, page_follow)
            .into_iter()
            .filter(|(_, d)| d.crawl)
            .map(|(l, _)| self.short(l.resolved.as_deref().unwrap()))
            .filter(|t| t != page)
            .collect()
    }
}

fn synth(link_type: LinkType, raw: &str, resolved: Option<&str>, order: u32) -> ExtractedLink {
    ExtractedLink {
        link_type,
        href_raw: raw.to_string(),
        resolved: resolved.map(str::to_string),
        anchor: None,
        alt_text: None,
        rel: None,
        nofollow_rel: false,
        target: None,
        path_type: PathType::Absolute,
        xpath: String::new(),
        position: LinkPosition::Content,
        in_head: true,
        doc_order: order,
    }
}

/// Alle Verweise einer ausgewerteten Seite als `ExtractedLink`; Canonicals, Paginierung, hreflang und Refresh
/// kommen aus eigenen Listen von `ParsedPage` und werden hier wie in der Engine zu Links.
fn all_links(page: &ParsedPage) -> Vec<ExtractedLink> {
    let mut out = page.links.clone();
    for c in &page.canonicals {
        let t = if c.source == HintSource::Http {
            LinkType::HttpCanonical
        } else {
            LinkType::Canonical
        };
        out.push(synth(t, &c.raw, c.resolved.as_deref(), c.order));
    }
    for p in &page.pagination {
        let t = if p.rel_next {
            LinkType::RelNext
        } else {
            LinkType::RelPrev
        };
        out.push(synth(t, &p.raw, p.resolved.as_deref(), p.order));
    }
    for h in &page.hreflang {
        let t = if h.source == HintSource::Http {
            LinkType::HttpHreflang
        } else {
            LinkType::Hreflang
        };
        out.push(synth(t, &h.href_raw, h.resolved.as_deref(), h.order));
    }
    for r in &page.refresh {
        if let Some(target) = &r.target {
            let t = if r.source == RefreshSource::Header {
                LinkType::HttpRefresh
            } else {
                LinkType::MetaRefresh
            };
            out.push(synth(
                t,
                r.target_raw.as_deref().unwrap_or(target),
                Some(target),
                r.order,
            ));
        }
    }
    out
}

type Slot = fn(&mut SpiderConfig) -> &mut StoreCrawl;

fn cfg_with(slot: Slot, store: bool, crawl: bool) -> CrawlConfig {
    let mut cfg = CrawlConfig::default();
    *slot(&mut cfg.spider) = StoreCrawl { store, crawl };
    cfg
}

// spec: CRAWL-038
#[tokio::test]
async fn matrix_fixture_page_yields_one_link_of_every_type() {
    let fx = Fixture::start("crawl-store-crawl-matrix").await;
    let page = fx.page("/").await;
    let links = all_links(&page);
    let mut found: Vec<(String, String)> = links
        .iter()
        .map(|l| {
            (
                l.link_type.as_str().to_string(),
                fx.short(l.resolved.as_deref().unwrap()),
            )
        })
        .collect();
    found.sort();
    let mut expected: Vec<(&str, &str)> = vec![
        ("Hyperlink", "/m/intern.html"),
        ("Hyperlink", "http://external.test/extern.html"),
        ("Image", "/m/bild.png"),
        ("CSS", "/m/style.css"),
        ("JavaScript", "/m/app.js"),
        ("Media", "/m/film.mp4"),
        ("Iframe", "/m/frame.html"),
        ("Canonical", "/m/canonical.html"),
        ("Rel Next", "/m/next.html"),
        ("Rel Prev", "/m/prev.html"),
        ("Hreflang", "/m/hreflang-fr.html"),
        ("Meta Refresh", "/m/refresh.html"),
    ];
    expected.sort();
    let expected: Vec<(String, String)> = expected
        .into_iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
    assert_eq!(found, expected);
}

// spec: CRAWL-038
#[tokio::test]
async fn matrix_all_four_switch_combinations_per_link_type() {
    let fx = Fixture::start("crawl-store-crawl-matrix").await;
    let page = fx.page("/").await;
    let links = all_links(&page);

    // (Linktyp, Zielpfad, Schalter, extern?)
    let cases: Vec<(LinkType, &str, Slot, bool)> = vec![
        (
            LinkType::Hyperlink,
            "/m/intern.html",
            |s| &mut s.internal,
            false,
        ),
        (
            LinkType::Hyperlink,
            "http://external.test/extern.html",
            |s| &mut s.external,
            true,
        ),
        (LinkType::Image, "/m/bild.png", |s| &mut s.images, false),
        (LinkType::Css, "/m/style.css", |s| &mut s.css, false),
        (
            LinkType::JavaScript,
            "/m/app.js",
            |s| &mut s.javascript,
            false,
        ),
        (LinkType::Media, "/m/film.mp4", |s| &mut s.media, false),
        (LinkType::Iframe, "/m/frame.html", |s| &mut s.iframes, false),
        (
            LinkType::Canonical,
            "/m/canonical.html",
            |s| &mut s.canonicals,
            false,
        ),
        (
            LinkType::RelNext,
            "/m/next.html",
            |s| &mut s.pagination,
            false,
        ),
        (
            LinkType::RelPrev,
            "/m/prev.html",
            |s| &mut s.pagination,
            false,
        ),
        (
            LinkType::Hreflang,
            "/m/hreflang-fr.html",
            |s| &mut s.hreflang,
            false,
        ),
        (
            LinkType::MetaRefresh,
            "/m/refresh.html",
            |s| &mut s.meta_refresh,
            false,
        ),
    ];
    for (link_type, target, slot, external) in cases {
        let link = links
            .iter()
            .find(|l| {
                l.link_type == link_type && fx.short(l.resolved.as_deref().unwrap()) == target
            })
            .unwrap_or_else(|| panic!("{link_type:?} {target} fehlt"));
        let scope = fx.class(link.resolved.as_deref().unwrap());
        assert_eq!(scope == ScopeClass::External, external, "{target}");
        for (store, crawl) in [(true, true), (true, false), (false, true), (false, false)] {
            let d = decide(link, scope, &cfg_with(slot, store, crawl), true);
            assert_eq!(
                (d.store, d.crawl),
                (store, crawl),
                "{link_type:?} {target} store={store} crawl={crawl}"
            );
            // Grund nur bei „gespeichert, nicht abgerufen“.
            assert_eq!(d.reason.is_some(), store && !crawl, "{target}");
        }
    }
}

// spec: CRAWL-038
#[tokio::test]
async fn matrix_defaults_crawl_everything_except_store_only_types() {
    let fx = Fixture::start("crawl-store-crawl-matrix").await;
    let links = all_links(&fx.page("/").await);
    let targets = fx.crawl_targets(&links, &fx.cfg, true, "/");
    let expected: BTreeSet<String> = [
        "/m/intern.html",
        "http://external.test/extern.html",
        "/m/bild.png",
        "/m/style.css",
        "/m/app.js",
        "/m/film.mp4",
        "/m/frame.html",
        "/m/canonical.html",
        "/m/refresh.html",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    // Paginierung und hreflang sind standardmäßig nur gespeichert (CRAWL-047/048).
    assert_eq!(targets, expected);
    let stored_only: Vec<_> = fx
        .decisions(&links, &fx.cfg, true)
        .into_iter()
        .filter(|(_, d)| d.store && !d.crawl)
        .map(|(l, d)| (l.link_type, d.reason))
        .collect();
    assert_eq!(stored_only.len(), 3);
    assert!(stored_only
        .iter()
        .all(|(_, r)| *r == Some(NotCrawledReason::StoreOnly)));
}

// spec: CRAWL-039
#[tokio::test]
async fn images_follow_the_image_switch_and_the_external_switch() {
    let fx = Fixture::start("crawl-store-crawl-matrix").await;
    let links = all_links(&fx.page("/").await);
    let img = links
        .iter()
        .find(|l| l.link_type == LinkType::Image)
        .unwrap();
    assert_eq!(fx.short(img.resolved.as_deref().unwrap()), "/m/bild.png");
    let d = decide(img, ScopeClass::Internal, &fx.cfg, true);
    assert_eq!((d.store, d.crawl), (true, true));

    // Dasselbe Bild auf einem externen Host wird nur bei „Externe Links › Crawlen“ geprüft.
    let mut external_img = img.clone();
    external_img.resolved = Some("http://cdn.other.test/x.png".to_string());
    assert_eq!(
        fx.class("http://cdn.other.test/x.png"),
        ScopeClass::External
    );
    let d = decide(&external_img, ScopeClass::External, &fx.cfg, true);
    assert_eq!((d.store, d.crawl), (true, true));
    let mut cfg = fx.cfg.clone();
    cfg.spider.external = StoreCrawl::STORE_ONLY;
    let d = decide(&external_img, ScopeClass::External, &cfg, true);
    assert_eq!((d.store, d.crawl), (true, false));
    assert_eq!(d.reason, Some(NotCrawledReason::ExternalNotChecked));
}

// spec: CRAWL-044
#[tokio::test]
async fn links_basic_internal_targets_and_multiple_links() {
    let fx = Fixture::start("crawl-links-basic").await;
    let page = fx.page("/").await;
    let links = all_links(&page);

    // Drei Links auf /a mit verschiedenen Ankertexten; jeder Link ist eine eigene Instanz.
    let to_a: Vec<_> = links
        .iter()
        .filter(|l| fx.short(l.resolved.as_deref().unwrap_or("http://x.test/")) == "/a")
        .collect();
    assert_eq!(to_a.len(), 3);
    let anchors: BTreeSet<_> = to_a.iter().filter_map(|l| l.anchor.clone()).collect();
    assert_eq!(anchors.len(), 3);

    // Abzurufen: /a (einmal als Ziel), /b, das <area>-Ziel und der externe Link; kein Ziel für <a> ohne href,
    // `#`, mailto:, tel: und javascript:.
    let targets = fx.crawl_targets(&links, &fx.cfg, true, "/");
    let expected: BTreeSet<String> = ["/a", "/b", "/area-ziel", "http://external.test/ext"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(targets, expected);

    // Nicht abrufbare Ziele (mailto, tel, javascript) werden weder gespeichert noch abgerufen.
    let dropped = fx
        .decisions(&links, &fx.cfg, true)
        .into_iter()
        .filter(|(l, _)| l.resolved.is_none())
        .count();
    assert!(
        dropped >= 3,
        "mailto, tel und javascript ergeben keinen Verweis"
    );
    assert!(fx
        .decisions(&links, &fx.cfg, true)
        .into_iter()
        .filter(|(l, _)| l.resolved.is_none())
        .all(|(_, d)| !d.store && !d.crawl));
}

// spec: CRAWL-044
#[tokio::test]
async fn links_basic_internal_crawl_off_requests_nothing_internal() {
    let fx = Fixture::start("crawl-links-basic").await;
    let links = all_links(&fx.page("/").await);
    let cfg = cfg_with(|s| &mut s.internal, true, false);
    let targets = fx.crawl_targets(&links, &cfg, true, "/");
    // Nur der externe Link bleibt übrig; intern wird nichts angefragt, aber alles bleibt gespeichert.
    assert_eq!(
        targets,
        ["http://external.test/ext".to_string()]
            .into_iter()
            .collect()
    );
    let internal_stored = fx
        .decisions(&links, &cfg, true)
        .into_iter()
        .filter(|(l, _)| {
            l.resolved
                .as_deref()
                .is_some_and(|r| fx.class(r) == ScopeClass::Internal)
        })
        .all(|(_, d)| d.store && !d.crawl && d.reason == Some(NotCrawledReason::StoreOnly));
    assert!(internal_stored);
}

// spec: CRAWL-045
#[tokio::test]
async fn links_basic_external_crawl_off_requests_nothing_external() {
    let fx = Fixture::start("crawl-links-basic").await;
    let links = all_links(&fx.page("/").await);
    let cfg = cfg_with(|s| &mut s.external, true, false);
    let targets = fx.crawl_targets(&links, &cfg, true, "/");
    assert!(
        targets
            .iter()
            .all(|t| !t.starts_with("http://external.test")),
        "{targets:?}"
    );
    assert!(targets.contains("/a"));
    let ext = fx
        .decisions(&links, &cfg, true)
        .into_iter()
        .find(|(l, _)| {
            l.resolved
                .as_deref()
                .is_some_and(|r| r.contains("external.test"))
        })
        .unwrap()
        .1;
    // Die externe URL bleibt in den Tabellen (ohne Statuscode), wird aber nicht angefragt.
    assert_eq!(
        ext,
        LinkDecision {
            store: true,
            crawl: false,
            reason: Some(NotCrawledReason::ExternalNotChecked)
        }
    );
}

// spec: CRAWL-056
#[tokio::test]
async fn nofollow_rel_links_are_stored_but_not_followed_by_default() {
    let fx = Fixture::start("crawl-nofollow").await;
    let page = fx.page("/").await;
    let links = all_links(&page);
    assert!(page.effective_follow);

    let targets = fx.crawl_targets(&links, &fx.cfg, page.effective_follow, "/");
    // Verfolgt: Follow-Links und Seiten mit Nofollow-Direktive (die Direktive wirkt erst auf ihre eigenen Links).
    let expected: BTreeSet<String> = [
        "/auch-follow",
        "/frei",
        "http://external.test/ext-follow",
        "/meta-nofollow",
        "/header-nofollow",
        "/googlebot-nofollow",
        "/header-googlebot-nofollow",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    assert_eq!(targets, expected);

    // Alle nofollow/sponsored/ugc-Ziele bleiben gespeichert (Folgen = false), werden aber nicht angefragt.
    for t in [
        "/nur-nofollow",
        "/nur-sponsored",
        "/nur-ugc",
        "/nofollow-noopener",
        "http://external.test/ext-nofollow",
        "http://external.test/ext-sponsored",
        "http://external.test/ext-ugc",
    ] {
        let (link, d) = fx
            .decisions(&links, &fx.cfg, true)
            .into_iter()
            .find(|(l, _)| fx.short(l.resolved.as_deref().unwrap()) == t)
            .unwrap_or_else(|| panic!("{t}"));
        assert!(link.nofollow_rel, "{t}");
        assert!(!link_follows(&link, true), "{t}");
        assert_eq!(
            d,
            LinkDecision {
                store: true,
                crawl: false,
                reason: Some(NotCrawledReason::Nofollow)
            },
            "{t}"
        );
    }
}

// spec: CRAWL-056
#[tokio::test]
async fn url_with_nofollow_and_follow_link_is_requested() {
    let fx = Fixture::start("crawl-nofollow").await;
    let links = all_links(&fx.page("/").await);
    let to_auch: Vec<_> = fx
        .decisions(&links, &fx.cfg, true)
        .into_iter()
        .filter(|(l, _)| fx.short(l.resolved.as_deref().unwrap()) == "/auch-follow")
        .collect();
    assert_eq!(to_auch.len(), 2);
    assert_eq!(to_auch.iter().filter(|(_, d)| d.crawl).count(), 1);
    assert_eq!(to_auch.iter().filter(|(l, _)| l.nofollow_rel).count(), 1);
}

// spec: CRAWL-056
#[tokio::test]
async fn follow_options_lift_nofollow_separately_for_internal_and_external() {
    let fx = Fixture::start("crawl-nofollow").await;
    let links = all_links(&fx.page("/").await);
    let internal_nf = [
        "/nur-nofollow",
        "/nur-sponsored",
        "/nur-ugc",
        "/nofollow-noopener",
    ];
    let external_nf = [
        "http://external.test/ext-nofollow",
        "http://external.test/ext-sponsored",
        "http://external.test/ext-ugc",
    ];

    let mut cfg = fx.cfg.clone();
    cfg.spider.follow_internal_nofollow = true;
    let t = fx.crawl_targets(&links, &cfg, true, "/");
    assert!(internal_nf.iter().all(|p| t.contains(*p)), "{t:?}");
    assert!(external_nf.iter().all(|p| !t.contains(*p)), "{t:?}");

    let mut cfg = fx.cfg.clone();
    cfg.spider.follow_external_nofollow = true;
    let t = fx.crawl_targets(&links, &cfg, true, "/");
    assert!(external_nf.iter().all(|p| t.contains(*p)), "{t:?}");
    assert!(internal_nf.iter().all(|p| !t.contains(*p)), "{t:?}");

    // Auch bei verfolgten nofollow-Links bleibt `Folgen` = false.
    let any_nf = links.iter().find(|l| l.nofollow_rel).unwrap();
    assert!(!link_follows(any_nf, true));
}

// spec: CRAWL-056
#[tokio::test]
async fn page_level_nofollow_from_meta_header_and_googlebot_stops_every_link() {
    let fx = Fixture::start("crawl-nofollow").await;
    for (path, behind) in [
        ("/meta-nofollow", "/hinter-meta"),
        ("/header-nofollow", "/hinter-header"),
        ("/googlebot-nofollow", "/hinter-googlebot"),
        ("/header-googlebot-nofollow", "/hinter-header-googlebot"),
    ] {
        let page = fx.page(path).await;
        assert!(!page.effective_follow, "{path}: Seite ist nofollow");
        let links = all_links(&page);

        // Mit der Seiten-Direktive wird der einzige Link nicht verfolgt, bleibt aber gespeichert.
        let d = fx
            .decisions(&links, &fx.cfg, page.effective_follow)
            .into_iter()
            .find(|(l, _)| fx.short(l.resolved.as_deref().unwrap()) == behind)
            .unwrap_or_else(|| panic!("{path}: Link {behind}"))
            .1;
        assert_eq!(
            d,
            LinkDecision {
                store: true,
                crawl: false,
                reason: Some(NotCrawledReason::Nofollow)
            },
            "{path}"
        );

        // „Internem nofollow folgen“ hebt das auf.
        let mut cfg = fx.cfg.clone();
        cfg.spider.follow_internal_nofollow = true;
        assert!(
            fx.crawl_targets(&links, &cfg, page.effective_follow, path)
                .contains(behind),
            "{path}"
        );
    }
}

// spec: CRAWL-046
#[tokio::test]
async fn canonical_targets_follow_the_canonical_switch() {
    let fx = Fixture::start("crawl-canonicals").await;
    let cases = [
        ("/c1", 1, vec![("/nur-canonical", LinkType::Canonical)]),
        (
            "/c2",
            1,
            vec![("/nur-header-canonical", LinkType::HttpCanonical)],
        ),
        (
            "/c3",
            2,
            vec![
                ("/nur-canonical-a", LinkType::Canonical),
                ("/nur-canonical-b", LinkType::Canonical),
            ],
        ),
        ("/c4", 1, vec![("/nur-body-canonical", LinkType::Canonical)]),
        // rel="Canonical" in Großbuchstaben und rel="canonical nofollow" (mehrere Tokens) werden erkannt.
        (
            "/c5",
            1,
            vec![("/nur-canonical-gross", LinkType::Canonical)],
        ),
        (
            "/c6",
            1,
            vec![("/nur-canonical-multi", LinkType::Canonical)],
        ),
    ];
    for (path, count, expected) in cases {
        let page = fx.page(path).await;
        let links = all_links(&page);
        let canon: Vec<_> = links
            .iter()
            .filter(|l| matches!(l.link_type, LinkType::Canonical | LinkType::HttpCanonical))
            .collect();
        assert_eq!(canon.len(), count, "{path}");
        let got: Vec<_> = canon
            .iter()
            .map(|l| (fx.short(l.resolved.as_deref().unwrap()), l.link_type))
            .collect();
        let want: Vec<_> = expected.iter().map(|(p, t)| (p.to_string(), *t)).collect();
        assert_eq!(got, want, "{path}");

        // Standard (an/an): die Canonical-Ziele werden angefragt.
        let defaults = fx.crawl_targets(&links, &fx.cfg, page.effective_follow, path);
        for (target, _) in &expected {
            assert!(
                defaults.contains(*target),
                "{path}: {target} wird angefragt"
            );
        }
        // Crawlen aus: nicht angefragt, aber gespeichert (in der Spalte geführt).
        let cfg = cfg_with(|s| &mut s.canonicals, true, false);
        let off = fx.crawl_targets(&links, &cfg, page.effective_follow, path);
        for (target, _) in &expected {
            assert!(!off.contains(*target), "{path}: {target} nicht angefragt");
        }
        for (l, d) in fx.decisions(
            &canon.iter().map(|l| (*l).clone()).collect::<Vec<_>>(),
            &cfg,
            true,
        ) {
            assert!(d.store && !d.crawl, "{path} {:?}", l.link_type);
        }
        // Speichern aus: Canonical-Spalten bleiben leer.
        let cfg = cfg_with(|s| &mut s.canonicals, false, false);
        assert!(fx
            .decisions(
                &canon.iter().map(|l| (*l).clone()).collect::<Vec<_>>(),
                &cfg,
                true
            )
            .iter()
            .all(|(_, d)| !d.store && !d.crawl));
    }
}

// spec: CRAWL-046
#[tokio::test]
async fn canonical_in_body_is_marked_outside_head() {
    let fx = Fixture::start("crawl-canonicals").await;
    let page = fx.page("/c4").await;
    assert_eq!(page.canonicals.len(), 1);
    assert!(!page.canonicals[0].in_head);
    let page = fx.page("/c1").await;
    assert!(page.canonicals[0].in_head);
}

// spec: CRAWL-046
#[tokio::test]
async fn canonical_of_a_variant_points_to_the_base_url() {
    let fx = Fixture::start("crawl-canonicals").await;
    let page = fx.page("/variante").await;
    assert_eq!(page.canonicals.len(), 1);
    assert_eq!(
        fx.short(page.canonicals[0].resolved.as_deref().unwrap()),
        "/variante"
    );
}

/// Muster, das die Basis-URL der Testsite wörtlich enthält (`.` maskiert).
fn base_pattern(base: &str, rest: &str) -> String {
    format!("{}{rest}", base.trim_end_matches('/').replace('.', r"\."))
}

/// Verlinkte Ziele der Startseite als URL-kodierte Adressen, in Dokumentreihenfolge.
fn page_targets(links: &[ExtractedLink]) -> Vec<String> {
    links
        .iter()
        .filter(|l| l.link_type == LinkType::Hyperlink)
        .map(|l| normalize(&Url::parse(l.resolved.as_deref().unwrap()).unwrap()).encoded)
        .collect()
}

// spec: CRAWL-109
#[tokio::test]
async fn include_fixture_matches_the_spec_examples() {
    use os_url::{RuleDecision, RuleSet};
    let fx = Fixture::start("crawl-include-exclude").await;
    let targets = page_targets(&all_links(&fx.page("/").await));
    let allowed = |include: &[String], partial: bool| -> Vec<String> {
        let rules = RuleSet::compile(include, &[], partial).unwrap();
        targets
            .iter()
            .filter(|t| rules.decide(t) == RuleDecision::Allowed)
            .map(|t| fx.short(t))
            .collect()
    };
    let blog = [".*/blog/.*".to_string()];
    assert_eq!(
        allowed(&blog, false),
        ["/blog/a", "/blog/a?x=1", "/blog/b"],
        "/shop/x und /Blog/c passen nicht (Groß-/Kleinschreibung zählt)"
    );
    // `(?i)` schaltet die Groß-/Kleinschreibung ab.
    assert!(allowed(&["(?i).*/blog/.*".to_string()], false).contains(&"/Blog/c".to_string()));
    // Vollübereinstimmung: ein Teilmuster ohne `.*` passt auf nichts, mit Teilübereinstimmung auf alle /blog/-URLs.
    assert!(allowed(&["/blog/".to_string()], false).is_empty());
    assert_eq!(
        allowed(&["/blog/".to_string()], true),
        ["/blog/a", "/blog/a?x=1", "/blog/b"]
    );
    // Ein Muster mit der vollständigen URL passt nur auf genau diese URL, nicht auf `?x=1`.
    assert_eq!(
        allowed(&[base_pattern(&fx.base, "/blog/a")], false),
        ["/blog/a"]
    );
    // Die Seite /blog/a bindet das Bild /img/a.png ein; es passt nicht auf `.*/blog/.*`.
    let blog_a = all_links(&fx.page("/blog/a").await);
    let img = blog_a
        .iter()
        .find(|l| l.link_type == LinkType::Image)
        .unwrap();
    assert_eq!(fx.short(img.resolved.as_deref().unwrap()), "/img/a.png");
    let rules = RuleSet::compile(&blog, &[], false).unwrap();
    assert_eq!(
        rules.decide(&normalize(&Url::parse(img.resolved.as_deref().unwrap()).unwrap()).encoded),
        RuleDecision::NotIncluded
    );
}

// spec: CRAWL-110
#[tokio::test]
async fn exclude_fixture_matches_the_spec_examples() {
    use os_url::{RuleDecision, RuleSet};
    let fx = Fixture::start("crawl-include-exclude").await;
    let targets = page_targets(&all_links(&fx.page("/").await));
    let excluded = |pattern: String| -> Vec<String> {
        let rules = RuleSet::compile(&[], &[pattern], false).unwrap();
        targets
            .iter()
            .filter(|t| matches!(rules.decide(t), RuleDecision::Excluded(_)))
            .map(|t| fx.short(t))
            .collect()
    };
    assert_eq!(excluded(r".*\.pdf".to_string()), ["/handbuch.pdf"]);
    assert_eq!(
        excluded(r".*[?&]sortierung=.*".to_string()),
        ["/liste?sortierung=preis", "/liste?seite=2&sortierung=preis"]
    );
    assert_eq!(
        excluded(r".*/artikel/[0-9]{4}/.*".to_string()),
        ["/artikel/2024/titel"]
    );
    assert_eq!(
        excluded(base_pattern(&fx.base, "/kalender/.*")),
        ["/kalender/2025-01"],
        "blog.site.test/kalender/x liegt auf einem anderen Host"
    );
    // Abgeglichen wird gegen die kodierte URL.
    assert_eq!(excluded(".*%20.*".to_string()), ["/a%20b"]);
    assert_eq!(excluded(".* .*".to_string()), Vec::<String>::new());
    assert_eq!(excluded(".*%C3%BC.*".to_string()), ["/%C3%BCber"]);
    // Ohne `.*` wirkt ein Teilmuster nicht.
    assert!(excluded(r"\.pdf".to_string()).is_empty());
    // URL, die Include und Exclude erfüllt: Exclude gewinnt.
    let rules = RuleSet::compile(
        &[".*/blog/.*".to_string()],
        &[r".*\?x=1".to_string()],
        false,
    )
    .unwrap();
    let blog_x = targets.iter().find(|t| t.ends_with("/blog/a?x=1")).unwrap();
    assert!(matches!(rules.decide(blog_x), RuleDecision::Excluded(_)));
}

// spec: TECH-210
#[tokio::test]
async fn sitemap_fixture_index_is_read_down_to_the_orphan_url() {
    use os_engine::sitemaps::{ingest, SitemapSeed};
    use os_model::SitemapKind;
    let fx = Fixture::start("crawl-links-basic").await;
    let out = ingest(
        &fx.fetcher,
        vec![SitemapSeed {
            url: Url::parse(&fx.site.url("/sitemap-index.xml")).unwrap(),
            found_via: "config".to_string(),
        }],
        3,
    )
    .await;
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].doc.as_ref().unwrap().kind, SitemapKind::Index);
    assert_eq!((out[1].parent, out[1].depth), (Some(0), 1));
    let urls: Vec<_> = out[1]
        .doc
        .as_ref()
        .unwrap()
        .locs
        .iter()
        .map(|l| fx.short(l.url.as_deref().unwrap()))
        .collect();
    assert_eq!(urls, ["/a", "/nur-in-sitemap"]);
    // Die Seite `/nur-in-sitemap` ist von `/` aus nicht verlinkt.
    let page = all_links(&fx.page("/").await);
    assert!(fx
        .crawl_targets(&page, &fx.cfg, true, "/")
        .iter()
        .all(|t| t != "/nur-in-sitemap"));
}
