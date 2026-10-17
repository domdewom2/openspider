//! Wiederholungen werden von der Frontier terminiert; kein Backoff hält einen Worker.
use os_fetch::{FetchOutcome, FetchResult};
use os_model::{CrawlConfig, NoResponseReason, Timestamp};
use std::time::Duration;

#[derive(Debug, PartialEq, Eq)]
pub enum RetryDecision {
    Done,
    RetryAfter(Duration),
}

/// Retry-After relativ zur injizierten Uhr; ungültige Werte werden ignoriert.
pub fn retry_after(value: &str, now: Timestamp, cap: u64) -> Option<Duration> {
    let value = value.trim();
    if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) {
        return Some(Duration::from_secs(
            value.parse::<u64>().unwrap_or(u64::MAX).min(cap),
        ));
    }
    let date =
        time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc2822).ok()?;
    let ms = (date.unix_timestamp_nanos() / 1_000_000 - i128::from(now.unix_millis())).max(0);
    Some(Duration::from_millis(u64::try_from(ms).unwrap_or(u64::MAX)).min(Duration::from_secs(cap)))
}

pub fn decide(res: &FetchResult, attempt: u32, cfg: &CrawlConfig) -> RetryDecision {
    let (limit, delay) = match &res.outcome {
        FetchOutcome::Response(r) if (500..600).contains(&r.status) || r.status == 429 => {
            let limit = if r.status == 429 {
                cfg.http.retries_network
            } else {
                cfg.http.retries_5xx
            };
            let delay = if matches!(r.status, 429 | 503) {
                r.header("retry-after").and_then(|v| {
                    retry_after(
                        v,
                        res.started_at
                            .plus(Duration::from_millis(u64::from(r.response_time_ms))),
                        60,
                    )
                })
            } else {
                None
            };
            (limit, delay)
        }
        FetchOutcome::NoResponse {
            reason:
                NoResponseReason::ConnectionError
                | NoResponseReason::ConnectionTimeout
                | NoResponseReason::ResponseTimeout,
            ..
        } => (cfg.http.retries_network, None),
        _ => return RetryDecision::Done,
    };
    if attempt >= limit {
        return RetryDecision::Done;
    }
    let backoff = Duration::from_secs(1u64.checked_shl(attempt).unwrap_or(u64::MAX));
    RetryDecision::RetryAfter(delay.unwrap_or(backoff))
}
