//! Canonical-Ziele, die sich nur in der Schreibweise unterscheiden (TECH-012, Fable-Gate M1, Flake in
//! `os-analyze` `tab_canonicals::contains_canonical_counts_html_and_header_canonicals`): `multi-case.html` nennt
//! `/case/a` und `/case/A`, jede der beiden Seiten trägt ein Selbst-Canonical. Die Tests stellen den Wettlauf
//! ohne Zufall nach: simulierte Zeit (`start_paused`), jede Reihenfolge der Antwortzeiten und jede Zahl von
//! Threads. Erwartet ist immer dasselbe Ergebnis: beide Ziele sind verschiedene URLs mit eigener Zeile, werden je
//! genau einmal abgerufen und tragen je genau ein Canonical (auf sich selbst); die Kennungen im Canonical-Ziel
//! der Ausgangsseite stimmen mit den Zeilen überein.

use std::collections::BTreeMap;
use std::time::Duration;

use os_engine::testing::{sim_deps, MemorySink, SimFetcher, SimPage, SimSite};
use os_engine::{start_crawl, CrawlRequest, CrawlSource};
use os_model::{CrawlConfig, CrawlState, PageWrite};

const BASE: &str = "https://site.test";

fn html(title: &str, head: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{title}</title>{head}</head><body><h1>{title}</h1></body></html>"
    )
}

fn page(body: String, delay_ms: u64) -> SimPage {
    SimPage {
        status: 200,
        headers: vec![(
            "content-type".to_string(),
            "text/html; charset=utf-8".to_string(),
        )],
        body,
        delay: Some(Duration::from_millis(delay_ms)),
    }
}

/// Die Fixture `tech/canonical-basic` in klein: Start, `multi-case.html` mit beiden Schreibweisen, beide Ziele mit
/// Selbst-Canonical. `delay_a`/`delay_b` sind die Antwortzeiten von `/case/a` und `/case/A`.
fn site(delay_a: u64, delay_b: u64) -> SimSite {
    let self_canonical = |p: &str| format!("<link rel=\"canonical\" href=\"{p}\">");
    SimSite::new()
        .page(
            &format!("{BASE}/"),
            &html("start", "<!-- -->").replace(
                "<h1>start</h1>",
                "<h1>start</h1><a href=\"/multi-case.html\">m</a>",
            ),
        )
        .page(
            &format!("{BASE}/multi-case.html"),
            &html(
                "multi-case",
                "<link rel=\"canonical\" href=\"/case/a\"><link rel=\"canonical\" href=\"/case/A\">",
            ),
        )
        .with(
            &format!("{BASE}/case/a"),
            page(html("a", &self_canonical("/case/a")), delay_a),
        )
        .with(
            &format!("{BASE}/case/A"),
            page(html("A", &self_canonical("/case/A")), delay_b),
        )
}

fn by_url(pages: &[PageWrite], urls: &BTreeMap<i64, String>) -> BTreeMap<String, PageWrite> {
    pages
        .iter()
        .map(|p| (urls[&p.url_id.0].clone(), p.clone()))
        .collect()
}

async fn crawl(
    delay_a: u64,
    delay_b: u64,
    threads: u32,
) -> (MemorySink, std::sync::Arc<SimFetcher>) {
    let sink = MemorySink::new();
    let fetcher = SimFetcher::new(site(delay_a, delay_b));
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = threads;
    let handle = start_crawl(
        fetcher.clone(),
        CrawlRequest {
            config: cfg,
            source: CrawlSource::Spider {
                start_url: format!("{BASE}/"),
            },
            file: dir.path().join("crawl.ospider"),
            project: "sim".to_string(),
            name: "sim".to_string(),
        },
        sim_deps(&sink),
    )
    .await
    .expect("Crawl startet");
    let summary = handle.wait().await.unwrap();
    assert_eq!(summary.state, CrawlState::Completed);
    (sink, fetcher)
}

// spec: TECH-012
#[tokio::test(start_paused = true)]
async fn case_variants_of_a_canonical_target_are_two_urls_in_every_response_order() {
    // (Antwortzeit /case/a, Antwortzeit /case/A) in Millisekunden: a zuerst, A zuerst, gleichzeitig, A sehr spät
    for (delay_a, delay_b) in [(10, 10), (5, 500), (500, 5), (0, 0), (1, 2_000)] {
        for threads in [1, 2, 4, 8] {
            let what = format!("a={delay_a} ms, A={delay_b} ms, {threads} Threads");
            let (sink, fetcher) = crawl(delay_a, delay_b, threads).await;

            // jede der beiden URLs genau einmal abgerufen
            assert_eq!(fetcher.count(&format!("{BASE}/case/a")), 1, "{what}");
            assert_eq!(fetcher.count(&format!("{BASE}/case/A")), 1, "{what}");

            // beide haben eine eigene, verschiedene Zeile
            let urls: BTreeMap<i64, String> = sink
                .new_urls()
                .into_iter()
                .map(|n| (n.id.0, n.url))
                .collect();
            let mut case_urls: Vec<&String> =
                urls.values().filter(|u| u.contains("/case/")).collect();
            case_urls.sort();
            assert_eq!(
                case_urls,
                [&format!("{BASE}/case/A"), &format!("{BASE}/case/a")],
                "{what}"
            );
            let ids: Vec<i64> = urls
                .iter()
                .filter(|(_, u)| u.contains("/case/"))
                .map(|(id, _)| *id)
                .collect();
            assert_ne!(ids[0], ids[1], "{what}");

            // beide Seiten sind geschrieben, mit genau einem Selbst-Canonical
            let pages = by_url(&sink.pages(), &urls);
            for p in ["a", "A"] {
                let w = pages
                    .get(&format!("{BASE}/case/{p}"))
                    .unwrap_or_else(|| panic!("{what}: Seite /case/{p} fehlt"));
                assert_eq!(w.fetch.status_code, 200, "{what} {p}");
                let facts = w.page.as_ref().expect("Seitenfakten");
                assert_eq!(facts.canonical_count, 1, "{what} {p}");
                assert_eq!(facts.canonical_self, Some(true), "{what} {p}");
                assert_eq!(w.canonicals.len(), 1, "{what} {p}");
                assert_eq!(
                    w.canonicals[0].target_id,
                    Some(w.url_id),
                    "{what} {p}: Ziel = eigene Zeile"
                );
            }

            // die Ausgangsseite verweist mit ihren beiden Canonical-Zeilen auf die beiden verschiedenen Zeilen
            let multi = pages
                .get(&format!("{BASE}/multi-case.html"))
                .unwrap_or_else(|| panic!("{what}: multi-case.html fehlt"));
            let targets: Vec<i64> = multi
                .canonicals
                .iter()
                .map(|c| c.target_id.expect("Ziel-ID").0)
                .collect();
            assert_eq!(targets.len(), 2, "{what}");
            assert_ne!(targets[0], targets[1], "{what}: zwei verschiedene Ziele");
            assert_eq!(urls[&targets[0]], format!("{BASE}/case/a"), "{what}");
            assert_eq!(urls[&targets[1]], format!("{BASE}/case/A"), "{what}");
        }
    }
}

// spec: TECH-012
#[tokio::test(start_paused = true)]
async fn both_case_variants_are_discovered_before_either_is_fetched() {
    // Mit einem Thread wird erst /case/a abgerufen (Entdeckungsreihenfolge); /case/A steht dann schon in der
    // Warteschlange und geht nicht verloren, auch wenn /case/a lange braucht.
    let (_sink, fetcher) = crawl(2_000, 1, 1).await;
    let order: Vec<String> = fetcher
        .page_requests()
        .into_iter()
        .map(|r| r.url)
        .filter(|u| u.contains("/case/"))
        .collect();
    assert_eq!(order, [format!("{BASE}/case/a"), format!("{BASE}/case/A")]);
}
