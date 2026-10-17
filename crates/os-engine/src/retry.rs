//! Wiederholungsentscheidung (CRAWL-064), Fachumsetzung M2-E03.
use std::time::Duration;
pub enum RetryDecision {
    Done,
    RetryAfter(Duration),
}
// STUB(M2-E03)
pub fn decide(
    _res: &os_fetch::FetchResult,
    _attempt: u32,
    _cfg: &os_model::CrawlConfig,
) -> RetryDecision {
    RetryDecision::Done
}
