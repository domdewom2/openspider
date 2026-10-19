//! Hostbezogene Parallelität, Crawl-delay, adaptive Drosselung und AIMD.
use os_fetch::{FetchOutcome, FetchResult};
use os_model::{Clock, CrawlConfig, NoResponseReason};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::Notify;
use url::Url;

struct State {
    auto_mode: bool,
    responses: u64,
    last_overload: Option<(Duration, u64)>,
    adapter_pause: Option<tokio::time::Instant>,
    active: usize,
    limit: usize,
    last_start: Option<Duration>,
    paused_until: Duration,
    spacing: Duration,
    throttle_until: Duration,
    successes: u32,
    fast: u32,
    samples: VecDeque<u32>,
    delay_cap_reported: bool,
    timer_anchor: Option<(Duration, tokio::time::Instant)>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            auto_mode: false,
            responses: 0,
            last_overload: None,
            adapter_pause: None,
            active: 0,
            limit: 1,
            last_start: None,
            paused_until: Duration::ZERO,
            spacing: Duration::ZERO,
            throttle_until: Duration::ZERO,
            successes: 0,
            fast: 0,
            samples: VecDeque::new(),
            delay_cap_reported: false,
            timer_anchor: None,
        }
    }
}
impl State {
    // Review decision: an isolated 503 without Retry-After is handled by retry.
    // A burst needs two overload responses within BOTH 10 s and 20 responses.
    fn overload(&mut self, status: u16, retry: Option<Duration>, now: Duration) -> bool {
        self.responses = self.responses.saturating_add(1);
        if !matches!(status, 429 | 503) {
            return false;
        }
        let repeated = self.last_overload.is_some_and(|(at, response)| {
            now.saturating_sub(at) <= Duration::from_secs(10)
                && self.responses.saturating_sub(response) <= 20
        });
        self.last_overload = Some((now, self.responses));
        status == 429 || retry.is_some() || repeated
    }
    fn increase_spacing(&mut self) {
        self.spacing = if self.spacing.is_zero() {
            Duration::from_secs(1)
        } else {
            (self.spacing * 2).min(Duration::from_secs(60))
        };
        self.successes = 0;
    }
    // Timer waits also count when a deterministic clock advances only on queries.
    // Tokio time remains virtual under start_paused.
    fn now(&mut self, clock: &dyn Clock) -> Duration {
        let clock_now = clock.monotonic();
        let timer_now = self
            .timer_anchor
            .map(|(base, start)| base.saturating_add(start.elapsed()));
        if timer_now.is_none_or(|t| clock_now > t) {
            self.timer_anchor = Some((clock_now, tokio::time::Instant::now()));
            clock_now
        } else {
            timer_now.unwrap()
        }
    }
}
#[derive(Default)]
struct Host {
    state: Mutex<State>,
    changed: Notify,
}
pub struct HostPermit {
    host: Option<Arc<Host>>,
}
impl Drop for HostPermit {
    fn drop(&mut self) {
        let Some(host) = &self.host else {
            return;
        };
        host.state.lock().unwrap_or_else(|p| p.into_inner()).active -= 1;
        host.changed.notify_waiters();
    }
}
#[derive(Default)]
pub struct PaceCancellation {
    cancelled: std::sync::atomic::AtomicBool,
    changed: Notify,
}
impl PaceCancellation {
    pub fn cancel(&self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.changed.notify_waiters();
    }
    async fn cancelled(&self) {
        let wait = self.changed.notified();
        tokio::pin!(wait);
        wait.as_mut().enable();
        if !self.cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            wait.await;
        }
    }
}
pub struct HostPacer {
    cancellation: Arc<PaceCancellation>,
    notices: Arc<Mutex<Vec<String>>>,
    auto_start: std::sync::atomic::AtomicBool,
    cap: Option<u32>,
    hosts: Mutex<HashMap<String, Arc<Host>>>,
}
impl HostPacer {
    pub fn cancellation(&self) -> Arc<PaceCancellation> {
        self.cancellation.clone()
    }
    pub fn notices(&self) -> Arc<Mutex<Vec<String>>> {
        self.notices.clone()
    }
    pub fn new(cap: Option<u32>) -> Self {
        Self {
            notices: Arc::default(),
            cancellation: Arc::default(),
            auto_start: std::sync::atomic::AtomicBool::new(false),
            cap: cap.filter(|n| *n > 0),
            hosts: Mutex::new(HashMap::new()),
        }
    }
    /// Configure the initial concurrency before workers (including robots) start.
    pub fn configure(&self, cfg: &CrawlConfig) {
        self.auto_start
            .store(auto(cfg), std::sync::atomic::Ordering::SeqCst);
    }
    fn host(&self, url: &Url) -> Arc<Host> {
        let key = format!(
            "{}:{}",
            url.host_str().unwrap_or(""),
            url.port_or_known_default().unwrap_or(0)
        );
        self.hosts
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .entry(key)
            .or_insert_with(|| {
                let auto_mode = self.auto_start.load(std::sync::atomic::Ordering::SeqCst);
                Arc::new(Host {
                    state: Mutex::new(State {
                        auto_mode,
                        limit: if auto_mode {
                            1
                        } else {
                            self.cap.map_or(usize::MAX, |n| n as usize)
                        },
                        ..State::default()
                    }),
                    changed: Notify::new(),
                })
            })
            .clone()
    }
    pub async fn acquire(&self, url: &Url) -> HostPermit {
        let host = self.host(url);
        loop {
            let notified = host.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            {
                let mut s = host.state.lock().unwrap_or_else(|p| p.into_inner());
                if s.active < s.limit {
                    s.active += 1;
                    return HostPermit {
                        host: Some(host.clone()),
                    };
                }
            }
            tokio::select! { _ = notified => {}, _ = self.cancellation.cancelled() => return HostPermit { host: None } }
        }
    }
    pub async fn before_fetch(
        &self,
        url: &Url,
        crawl_delay: Option<f64>,
        cfg: &CrawlConfig,
        clock: &dyn Clock,
    ) {
        let host = self.host(url);
        let capped = cfg.robots.respect_crawl_delay && crawl_delay.is_some_and(|n| n > 60.0);
        let crawl_delay = if cfg.robots.respect_crawl_delay {
            crawl_delay
                .filter(|n| !n.is_nan())
                .unwrap_or(0.0)
                .clamp(0.0, 60.0)
        } else {
            0.0
        };
        loop {
            let wait = {
                let mut s = host.state.lock().unwrap_or_else(|p| p.into_inner());
                if capped && !s.delay_cap_reported {
                    s.delay_cap_reported = true;
                    tracing::info!(host = url.host_str(), "Crawl-delay auf 60 s begrenzt");
                    let mut notices = self.notices.lock().unwrap_or_else(|p| p.into_inner());
                    if notices.len() < 256 {
                        notices.push(format!(
                            "Crawl-delay auf 60 s begrenzt: {}",
                            url.host_str().unwrap_or("")
                        ));
                    }
                }
                if auto(cfg) && !s.auto_mode {
                    s.limit = 1;
                }
                s.auto_mode = auto(cfg);
                if !auto(cfg) {
                    s.limit = self.cap.map_or(usize::MAX, |n| n as usize);
                    host.changed.notify_waiters();
                }
                let rate = cfg
                    .speed
                    .max_urls_per_sec
                    .and_then(|r| Duration::try_from_secs_f64(1.0 / r).ok())
                    .unwrap_or_default();
                let now = s.now(clock);
                let rate = if s.throttle_until > now {
                    (rate * 2).max(Duration::from_secs(1))
                } else {
                    rate
                };
                let spacing = Duration::from_secs_f64(crawl_delay)
                    .max(s.spacing)
                    .max(rate);
                let due = s
                    .last_start
                    .map_or(Duration::ZERO, |t| t.saturating_add(spacing))
                    .max(s.paused_until);
                let adapter_wait = s
                    .adapter_pause
                    .map(|t| t.saturating_duration_since(tokio::time::Instant::now()))
                    .unwrap_or_default();
                if due <= now && adapter_wait.is_zero() {
                    s.last_start = Some(now);
                    Duration::ZERO
                } else {
                    due.saturating_sub(now).max(adapter_wait)
                }
            };
            if wait.is_zero() {
                return;
            }
            tokio::select! { _ = tokio::time::sleep(wait) => {}, _ = self.cancellation.cancelled() => return }
        }
    }
    pub fn on_result(&self, url: &Url, result: &FetchResult, cfg: &CrawlConfig, clock: &dyn Clock) {
        let host = self.host(url);
        let mut s = host.state.lock().unwrap_or_else(|p| p.into_inner());
        let now = s.now(clock);
        let was_throttled = !s.spacing.is_zero() || s.paused_until.max(s.throttle_until) > now;
        let (status, elapsed, timeout, retry) = match &result.outcome {
            FetchOutcome::Response(r) => (
                r.status,
                r.response_time_ms,
                false,
                r.header("retry-after")
                    .and_then(|v| crate::retry::retry_after(v, clock.now(), 300)),
            ),
            FetchOutcome::NoResponse {
                reason, elapsed_ms, ..
            } => (
                0,
                *elapsed_ms,
                matches!(
                    reason,
                    NoResponseReason::ConnectionTimeout | NoResponseReason::ResponseTimeout
                ),
                None,
            ),
        };
        if status == 429 && cfg.http.throttle_on_429 {
            s.throttle_until = now.saturating_add(Duration::from_secs(60));
            if let Some(delay) = retry {
                s.paused_until = s.paused_until.max(now.saturating_add(delay));
            }
        }
        let overload_status = if status == 429 && !cfg.http.throttle_on_429 {
            0
        } else {
            status
        };
        let overloaded = s.overload(overload_status, retry, now);
        if cfg.speed.adaptive_throttle && overloaded {
            if let Some(delay) = retry {
                s.paused_until = s.paused_until.max(now.saturating_add(delay));
            } else {
                s.increase_spacing();
            }
            s.successes = 0;
            tracing::info!(host = url.host_str(), "Gedrosselt");
        } else if (200..400).contains(&status) {
            s.successes += 1;
            if s.successes >= 20 {
                s.spacing = if s.spacing <= Duration::from_secs(1) {
                    Duration::ZERO
                } else {
                    s.spacing / 2
                };
                s.successes = 0;
            }
        }
        if auto(cfg) {
            let mut sorted: Vec<_> = s.samples.iter().copied().collect();
            sorted.sort_unstable();
            let median = sorted
                .get(sorted.len() / 2)
                .copied()
                .unwrap_or(elapsed)
                .max(1);
            if matches!(status, 429 | 503) || timeout || u64::from(elapsed) > u64::from(median) * 3
            {
                s.limit = (s.limit / 2).max(1);
                s.fast = 0;
            } else if (200..400).contains(&status) && u64::from(elapsed) * 2 < u64::from(median) * 3
            {
                s.fast += 1;
                if s.fast >= 10 {
                    s.limit = (s.limit + 1).min(
                        self.cap
                            .unwrap_or(cfg.speed.threads)
                            .min(cfg.speed.threads)
                            .max(1) as usize,
                    );
                    s.fast = 0;
                }
            } else {
                s.fast = 0;
            }
            if status != 0 {
                if s.samples.len() == 31 {
                    s.samples.pop_front();
                }
                s.samples.push_back(elapsed);
            }
        }
        if !was_throttled && (!s.spacing.is_zero() || s.paused_until.max(s.throttle_until) > now) {
            let mut notices = self.notices.lock().unwrap_or_else(|p| p.into_inner());
            let message = format!("Gedrosselt: {}", url.host_str().unwrap_or(""));
            if notices.len() < 256 && !notices.contains(&message) {
                notices.push(message);
            }
        }
        host.changed.notify_waiters();
    }
    /// Clockless adapter for overload responses. Retry-After is a one-shot Tokio
    /// pause (capped at 300 s), never permanent spacing. Without Retry-After only
    /// 429 doubles spacing; 503 burst detection and recovery require on_result's
    /// complete response stream and configuration.
    pub fn on_response(&self, url: &Url, status: u16, retry_after: Option<Duration>) {
        if !matches!(status, 429 | 503) {
            return;
        }
        let host = self.host(url);
        let mut s = host.state.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(delay) = retry_after {
            let until = tokio::time::Instant::now() + delay.min(Duration::from_secs(300));
            s.adapter_pause = Some(s.adapter_pause.map_or(until, |old| old.max(until)));
        } else if status == 429 {
            s.increase_spacing();
        }
        host.changed.notify_waiters();
    }
    /// Momentaufnahme für Fortschrittsanzeigen und deterministische Tests.
    pub fn status(&self, url: &Url, clock: &dyn Clock) -> (usize, bool) {
        let host = self.host(url);
        let mut s = host.state.lock().unwrap_or_else(|p| p.into_inner());
        let now = s.now(clock);
        (
            s.limit,
            !s.spacing.is_zero()
                || s.paused_until.max(s.throttle_until) > now
                || s.adapter_pause
                    .is_some_and(|t| t > tokio::time::Instant::now()),
        )
    }
}
fn auto(cfg: &CrawlConfig) -> bool {
    cfg.speed.auto && cfg.speed.max_urls_per_sec.is_none()
}
