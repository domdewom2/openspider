//! TECH-008: End-to-End-Benchmark, nur auf ruhiger Maschine ausführen.
use std::time::{Duration, Instant};

use os_engine::testing::crawl_fixture_from;
use os_model::CrawlConfig;

async fn sample(enabled: bool) -> Duration {
    let mut cfg = CrawlConfig::default();
    cfg.advanced.html_validation = enabled;
    let start = Instant::now();
    let fx = crawl_fixture_from("crawl-site-100", "http://site.test/", cfg).await;
    let elapsed = start.elapsed();
    assert!(fx.summary.crawled >= 100);
    elapsed
}

// spec: TECH-008
#[tokio::test]
#[ignore = "Benchmark: crawl-site-100, ruhige Maschine, höchstens 5 % Mehrkosten"]
async fn html_validation_adds_at_most_five_percent_to_a_text_crawl() {
    // Warm-up beider Pfade; wechselnde Reihenfolge begrenzt Cache- und Temperatureffekte.
    sample(false).await;
    sample(true).await;
    let mut off = Vec::new();
    let mut on = Vec::new();
    for i in 0..9 {
        if i % 2 == 0 {
            off.push(sample(false).await);
            on.push(sample(true).await);
        } else {
            on.push(sample(true).await);
            off.push(sample(false).await);
        }
    }
    off.sort();
    on.sort();
    let ratio = on[4].as_secs_f64() / off[4].as_secs_f64();
    println!(
        "TECH-008 crawl-site-100: off={:?}, on={:?}, ratio={ratio:.4}",
        off[4], on[4]
    );
    assert!(
        ratio <= 1.05,
        "Validierung: {:.2} % Mehrkosten",
        (ratio - 1.0) * 100.0
    );
}
