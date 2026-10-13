//! Indexierbarkeitsmodell (TECH-001): Gründe, Prüfreihenfolge und Randfälle von `evaluate`.
//!
//! Die URL-Gleichheit (`same_url`) ersetzt hier ein kleiner Nachbau von `os_url::eq_for_compare` (TECH-002):
//! Fragment entfernt, ungültige URLs (`http://[bad`) sind nie gleich. Der Abgleich mit dem echten Vergleich
//! läuft in den Engine- und Tab-Tests.

use os_model::*;

const PAGE: &str = "https://site.test/p";

fn same(a: &str, b: &str) -> bool {
    fn key(u: &str) -> Option<&str> {
        let u = u.split('#').next().unwrap_or(u);
        ((u.starts_with("https://") || u.starts_with("http://")) && !u.contains("[bad"))
            .then_some(u)
    }
    matches!((key(a), key(b)), (Some(x), Some(y)) if x == y)
}

fn fetch(status: u16) -> FetchFacts {
    FetchFacts {
        status_code: status,
        ..FetchFacts::default()
    }
}

fn page() -> PageFacts {
    PageFacts {
        title: ElementPair::default(),
        meta_description: ElementPair::default(),
        meta_keywords: ElementPair::default(),
        h1: ElementPair::default(),
        h2: ElementPair::default(),
        word_count: None,
        text_ratio: None,
        canonical_count: 0,
        canonical_url: None,
        canonical_target: None,
        canonical_self: None,
        effective_index: true,
        effective_follow: true,
        has_rel_next: false,
        has_rel_prev: false,
        hreflang_count: 0,
    }
}

fn refresh(source: RefreshSource, target: Option<&str>) -> RefreshRef {
    RefreshRef {
        source,
        raw: format!("5; url={}", target.unwrap_or("")),
        delay: Some(5),
        target_raw: target.map(str::to_string),
        target: target.map(str::to_string),
        in_head: true,
        order: 0,
    }
}

fn eval_with(
    f: &FetchFacts,
    p: Option<&PageFacts>,
    refresh: &[RefreshRef],
    cfg: &CrawlConfig,
) -> (Indexability, Option<IndexabilityStatus>) {
    evaluate(&IndexabilityInput {
        fetch: f,
        page: p,
        page_url: PAGE,
        refresh,
        cfg,
        same_url: same,
    })
}

fn eval(
    f: &FetchFacts,
    p: Option<&PageFacts>,
    refresh: &[RefreshRef],
) -> Option<IndexabilityStatus> {
    let (indexability, status) = eval_with(f, p, refresh, &CrawlConfig::default());
    assert_eq!(
        indexability == Indexability::Indexable,
        status.is_none(),
        "Indexability und Status passen zusammen"
    );
    status
}

// spec: TECH-001
#[test]
fn a_clean_200_page_is_indexable() {
    let (i, s) = eval_with(&fetch(200), Some(&page()), &[], &CrawlConfig::default());
    assert_eq!((i, s), (Indexability::Indexable, None));
}

// spec: TECH-001
#[test]
fn every_reason_is_reported_on_its_own() {
    use IndexabilityStatus::*;
    let mut blocked = fetch(0);
    blocked.robots_blocked = true;
    assert_eq!(eval(&blocked, None, &[]), Some(BlockedByRobots));
    assert_eq!(eval(&fetch(0), None, &[]), Some(NoResponse));
    assert_eq!(eval(&fetch(301), None, &[]), Some(Redirected));
    assert_eq!(eval(&fetch(404), None, &[]), Some(ClientError));
    assert_eq!(eval(&fetch(503), None, &[]), Some(ServerError));

    let mut noindex = page();
    noindex.effective_index = false;
    assert_eq!(eval(&fetch(200), Some(&noindex), &[]), Some(Noindex));

    let mut canon = page();
    canon.canonical_url = Some("https://site.test/other".into());
    assert_eq!(eval(&fetch(200), Some(&canon), &[]), Some(Canonicalised));

    let selfref = [refresh(RefreshSource::Meta, None)];
    assert_eq!(
        eval(&fetch(200), Some(&page()), &selfref),
        Some(SelfReferencingMetaRefresh)
    );
}

// spec: TECH-001
#[test]
fn the_first_matching_reason_wins() {
    use IndexabilityStatus::*;
    let other = [refresh(RefreshSource::Meta, Some("https://site.test/x"))];
    let selfref = [refresh(RefreshSource::Meta, None)];
    let mut noindex_canon = page();
    noindex_canon.effective_index = false;
    noindex_canon.canonical_url = Some("https://site.test/other".into());

    // robots vor „keine Antwort“ und vor allem anderen
    let mut blocked = fetch(404);
    blocked.robots_blocked = true;
    assert_eq!(eval(&blocked, None, &other), Some(BlockedByRobots));
    let mut unreachable = fetch(0);
    unreachable.no_response_reason = Some(NoResponseReason::BlockedByRobotsUnreachable);
    assert_eq!(eval(&unreachable, None, &[]), Some(BlockedByRobots));

    // keine Antwort vor Weiterleitung
    assert_eq!(eval(&fetch(0), None, &other), Some(NoResponse));
    // Weiterleitung (3xx und Refresh) vor Client-/Serverfehler
    assert_eq!(eval(&fetch(302), None, &selfref), Some(Redirected));
    assert_eq!(eval(&fetch(404), None, &other), Some(Redirected));
    assert_eq!(eval(&fetch(503), None, &other), Some(Redirected));
    // Fehler vor Selbst-Refresh
    assert_eq!(eval(&fetch(404), None, &selfref), Some(ClientError));
    assert_eq!(eval(&fetch(500), None, &selfref), Some(ServerError));
    // Selbst-Refresh vor noindex, noindex vor Canonical
    assert_eq!(
        eval(&fetch(200), Some(&noindex_canon), &selfref),
        Some(SelfReferencingMetaRefresh)
    );
    assert_eq!(eval(&fetch(200), Some(&noindex_canon), &[]), Some(Noindex));
}

// spec: TECH-001
#[test]
fn meta_refresh_to_another_url_is_a_redirect_with_any_delay() {
    use IndexabilityStatus::*;
    let mut r = refresh(RefreshSource::Meta, Some("https://site.test/x"));
    for delay in [0, 5, 300] {
        r.delay = Some(delay);
        assert_eq!(
            eval(&fetch(200), Some(&page()), std::slice::from_ref(&r)),
            Some(Redirected)
        );
    }
    // Header-Variante
    let header = refresh(RefreshSource::Header, Some("https://site.test/x"));
    assert_eq!(
        eval(&fetch(200), Some(&page()), &[header]),
        Some(Redirected)
    );
    // andere Query = andere URL (TECH-068)
    let query = refresh(RefreshSource::Meta, Some("https://site.test/p?x=1"));
    assert_eq!(eval(&fetch(200), Some(&page()), &[query]), Some(Redirected));
}

// spec: TECH-001
#[test]
fn self_referencing_refresh_depends_on_the_option() {
    use IndexabilityStatus::*;
    let bare = refresh(RefreshSource::Meta, None);
    let own = refresh(RefreshSource::Meta, Some(PAGE));
    let fragment = refresh(RefreshSource::Meta, Some("https://site.test/p#top"));
    for r in [bare, own, fragment] {
        assert_eq!(
            eval(&fetch(200), Some(&page()), std::slice::from_ref(&r)),
            Some(SelfReferencingMetaRefresh),
            "Standard: Selbst-Refresh ist nicht indexierbar ({r:?})"
        );
        let mut cfg = CrawlConfig::default();
        cfg.advanced.respect_self_ref_meta_refresh = false;
        assert_eq!(
            eval_with(&fetch(200), Some(&page()), &[r], &cfg),
            (Indexability::Indexable, None),
            "ohne Option ändert ein Selbst-Refresh nichts"
        );
    }
    // ohne Option bleiben die übrigen Gründe wirksam
    let mut cfg = CrawlConfig::default();
    cfg.advanced.respect_self_ref_meta_refresh = false;
    let mut noindex = page();
    noindex.effective_index = false;
    let bare = refresh(RefreshSource::Meta, None);
    assert_eq!(
        eval_with(&fetch(200), Some(&noindex), &[bare], &cfg),
        (Indexability::NonIndexable, Some(Noindex))
    );
}

// spec: TECH-001
#[test]
fn the_header_refresh_is_the_one_that_counts() {
    use IndexabilityStatus::*;
    // Wie beim Abruf: zuerst der Header, sonst das erste Meta-Element.
    let meta_other = refresh(RefreshSource::Meta, Some("https://site.test/x"));
    let header_self = refresh(RefreshSource::Header, Some(PAGE));
    assert_eq!(
        eval(
            &fetch(200),
            Some(&page()),
            &[meta_other.clone(), header_self]
        ),
        Some(SelfReferencingMetaRefresh)
    );
    let meta_self = refresh(RefreshSource::Meta, None);
    assert_eq!(
        eval(&fetch(200), Some(&page()), &[meta_self, meta_other]),
        Some(SelfReferencingMetaRefresh),
        "das erste Meta-Element entscheidet"
    );
}

// spec: TECH-001
#[test]
fn an_unparseable_refresh_target_has_no_effect() {
    let broken = refresh(RefreshSource::Meta, Some("http://[bad"));
    assert_eq!(eval(&fetch(200), Some(&page()), &[broken]), None);
    let mut unresolved = refresh(RefreshSource::Meta, None);
    unresolved.target_raw = Some("ht!tp://".into());
    assert_eq!(eval(&fetch(200), Some(&page()), &[unresolved]), None);
}

// spec: TECH-001
#[test]
fn canonical_is_compared_with_the_url_equality_of_the_engine() {
    use IndexabilityStatus::*;
    let mut p = page();
    p.canonical_url = Some(PAGE.into());
    assert_eq!(eval(&fetch(200), Some(&p), &[]), None, "gleiche URL");
    p.canonical_url = Some("https://site.test/p#top".into());
    assert_eq!(eval(&fetch(200), Some(&p), &[]), None, "Fragment ignoriert");
    p.canonical_url = Some("https://site.test/p/".into());
    assert_eq!(
        eval(&fetch(200), Some(&p), &[]),
        Some(Canonicalised),
        "Slash"
    );
    p.canonical_url = Some("http://site.test/p".into());
    assert_eq!(
        eval(&fetch(200), Some(&p), &[]),
        Some(Canonicalised),
        "Schema"
    );
    // ungültige Canonical wird ignoriert (TECH-014)
    p.canonical_url = Some("http://[bad".into());
    assert_eq!(eval(&fetch(200), Some(&p), &[]), None);
}

// spec: TECH-001
#[test]
fn without_stored_canonicals_nothing_is_canonicalised() {
    // CRAWL-046: spider.canonicals.store = false
    let mut p = page();
    p.canonical_url = Some("https://site.test/other".into());
    let mut cfg = CrawlConfig::default();
    cfg.spider.canonicals.store = false;
    assert_eq!(
        eval_with(&fetch(200), Some(&p), &[], &cfg),
        (Indexability::Indexable, None)
    );
    let mut with_crawl_off = CrawlConfig::default();
    with_crawl_off.spider.canonicals.crawl = false;
    assert_eq!(
        eval_with(&fetch(200), Some(&p), &[], &with_crawl_off).1,
        Some(IndexabilityStatus::Canonicalised),
        "Canonicals crawlen aus ändert die Bewertung nicht"
    );
}

// spec: TECH-001
#[test]
fn non_html_resources_and_external_urls() {
    use IndexabilityStatus::*;
    // PDF: noindex per X-Robots-Tag oder HTTP-Canonical (Seitenfakten aus den Header-Hinweisen)
    let mut pdf = page();
    pdf.effective_index = false;
    assert_eq!(eval(&fetch(200), Some(&pdf), &[]), Some(Noindex));
    let mut pdf = page();
    pdf.canonical_url = Some("https://site.test/doc".into());
    assert_eq!(eval(&fetch(200), Some(&pdf), &[]), Some(Canonicalised));
    // Extern: keine Seitenfakten, nur der Status zählt – ein 200 ist indexierbar
    assert_eq!(eval(&fetch(200), None, &[]), None);
    assert_eq!(eval(&fetch(404), None, &[]), Some(ClientError));
    assert_eq!(eval(&fetch(500), None, &[]), Some(ServerError));
}

// spec: TECH-001
#[test]
fn robots_blocked_with_a_real_status_is_still_blocked() {
    // „Ignorieren, aber melden“: echter Statuscode mit robots_blocked = 1. TECH-001 nennt Blocked by Robots.txt
    // als ersten Grund; ob eine tatsächlich abgerufene Seite so zu führen ist, ist in der Spec nicht
    // ausdrücklich geregelt (Spec-Unklarheit, im Paketbericht gemeldet) – hier gilt die Prüfreihenfolge.
    let mut f = fetch(200);
    f.robots_blocked = true;
    assert_eq!(
        eval(&f, Some(&page()), &[]),
        Some(IndexabilityStatus::BlockedByRobots)
    );
}
