//! Token-Bucket-Rate-Limit für alle Abrufe (CRAWL-113).
//!
//! Der Eimer fasst genau ein Token (kein Burst über 1): Zwei Abrufbeginne liegen nie näher als
//! `1 / Rate` beieinander, unabhängig von Thread-Zahl und Abrufzweck. Das Limit gilt für Seiten,
//! Ressourcen, externe URLs, robots.txt und Sitemaps, weil es im Fetcher sitzt.
//!
//! Zeitquelle ist `Clock::monotonic`; gewartet wird mit `tokio::time::sleep`. Unter
//! `#[tokio::test(start_paused = true)]` genügt eine Uhr, die auf `tokio::time::Instant` beruht,
//! um Raten ohne Wanduhr zu prüfen (M1-Plan 2.8).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use os_model::Clock;

/// Begrenzt die Abrufbeginne pro Sekunde.
pub struct RateLimiter {
    interval: Duration,
    clock: Arc<dyn Clock>,
    /// Frühester Zeitpunkt (Uhr-Zeit), zu dem der nächste Abruf beginnen darf.
    next_free: Mutex<Duration>,
}

impl std::fmt::Debug for RateLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RateLimiter")
            .field("interval", &self.interval)
            .finish()
    }
}

impl RateLimiter {
    /// Begrenzt auf `per_sec` Abrufe pro Sekunde (z. B. `0.5` = einer alle 2 s).
    ///
    /// Fehler bei Werten ≤ 0, `NaN`, unendlich oder so klein, dass das Intervall nicht darstellbar ist.
    pub fn new(per_sec: f64, clock: Arc<dyn Clock>) -> Result<Self, String> {
        if !per_sec.is_finite() || per_sec <= 0.0 {
            return Err(format!(
                "speed.max_urls_per_sec muss größer als 0 sein (ist {per_sec})"
            ));
        }
        let interval = Duration::try_from_secs_f64(1.0 / per_sec)
            .map_err(|_| format!("speed.max_urls_per_sec {per_sec} ist nicht darstellbar"))?;
        Ok(RateLimiter {
            interval,
            clock,
            next_free: Mutex::new(Duration::ZERO),
        })
    }

    /// Mindestabstand zwischen zwei Abrufbeginnen.
    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// Wartet, bis der Abruf beginnen darf. Reihenfolge der Aufrufer bleibt erhalten.
    pub async fn acquire(&self) {
        let wait = self.reserve();
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
    }

    /// Reserviert den nächsten freien Zeitpunkt und liefert die Wartezeit bis dahin.
    fn reserve(&self) -> Duration {
        let now = self.clock.monotonic();
        let mut next = self.next_free.lock().unwrap_or_else(|e| e.into_inner());
        let slot = (*next).max(now);
        *next = slot + self.interval;
        slot - now
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant as StdInstant;

    /// Uhr auf Basis der tokio-Zeit: läuft unter `start_paused` mit der simulierten Zeit.
    struct TokioClock {
        start: tokio::time::Instant,
    }

    impl TokioClock {
        fn new() -> Arc<Self> {
            Arc::new(TokioClock {
                start: tokio::time::Instant::now(),
            })
        }
    }

    impl Clock for TokioClock {
        fn now(&self) -> os_model::Timestamp {
            os_model::Timestamp::now_utc()
        }
        fn monotonic(&self) -> Duration {
            self.start.elapsed()
        }
    }

    // spec: CRAWL-113
    #[tokio::test(start_paused = true)]
    async fn two_per_second_over_twenty_requests_takes_at_least_9_5_seconds() {
        let clock = TokioClock::new();
        let limiter = RateLimiter::new(2.0, clock.clone()).unwrap();
        let mut starts = Vec::new();
        for _ in 0..20 {
            limiter.acquire().await;
            starts.push(clock.monotonic());
        }
        let total = *starts.last().unwrap() - starts[0];
        assert!(
            total >= Duration::from_millis(9500),
            "Gesamtdauer {total:?}"
        );
        // in keinem 1-s-Fenster mehr als 2 Beginne (Burst höchstens 1 über die Rate)
        for (i, s) in starts.iter().enumerate() {
            let in_window = starts[i..]
                .iter()
                .take_while(|t| **t < *s + Duration::from_secs(1))
                .count();
            assert!(in_window <= 2, "{in_window} Abrufe im Fenster ab {s:?}");
        }
    }

    // spec: CRAWL-113
    #[tokio::test(start_paused = true)]
    async fn half_per_second_spaces_requests_by_two_seconds() {
        let clock = TokioClock::new();
        let limiter = RateLimiter::new(0.5, clock.clone()).unwrap();
        assert_eq!(limiter.interval(), Duration::from_secs(2));
        let mut prev: Option<Duration> = None;
        for _ in 0..5 {
            limiter.acquire().await;
            let t = clock.monotonic();
            if let Some(p) = prev {
                assert!(t - p >= Duration::from_secs(2), "Abstand {:?}", t - p);
            }
            prev = Some(t);
        }
    }

    // spec: CRAWL-113
    #[tokio::test(start_paused = true)]
    async fn concurrent_callers_share_one_budget() {
        let clock = TokioClock::new();
        let limiter = Arc::new(RateLimiter::new(4.0, clock.clone()).unwrap());
        let mut tasks = Vec::new();
        for _ in 0..8 {
            let l = limiter.clone();
            let c = clock.clone();
            tasks.push(tokio::spawn(async move {
                l.acquire().await;
                c.monotonic()
            }));
        }
        let mut times = Vec::new();
        for t in tasks {
            times.push(t.await.unwrap());
        }
        times.sort();
        for w in times.windows(2) {
            assert!(w[1] - w[0] >= Duration::from_millis(250), "{w:?}");
        }
    }

    // spec: CRAWL-113
    #[tokio::test(start_paused = true)]
    async fn idle_time_does_not_build_up_a_burst() {
        let clock = TokioClock::new();
        let limiter = RateLimiter::new(1.0, clock.clone()).unwrap();
        limiter.acquire().await;
        tokio::time::sleep(Duration::from_secs(60)).await;
        let before = clock.monotonic();
        limiter.acquire().await; // sofort frei
        limiter.acquire().await; // aber nur ein Token: eine Sekunde Wartezeit
        assert!(clock.monotonic() - before >= Duration::from_secs(1));
    }

    // spec: CRAWL-113
    #[test]
    fn invalid_rates_are_rejected() {
        let clock: Arc<dyn Clock> = Arc::new(os_model::SystemClock);
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY, 1e-300] {
            assert!(RateLimiter::new(bad, clock.clone()).is_err(), "{bad}");
        }
        assert!(RateLimiter::new(1000.0, clock).is_ok());
    }

    // spec: CRAWL-113
    #[tokio::test]
    async fn first_request_is_not_delayed() {
        let limiter = RateLimiter::new(0.5, Arc::new(os_model::SystemClock)).unwrap();
        let t = StdInstant::now();
        limiter.acquire().await;
        assert!(t.elapsed() < Duration::from_millis(500));
    }
}
