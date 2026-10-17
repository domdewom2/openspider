//! Fortschritt nach CRAWL-130 (Rate, Restzeit, Prozent) – reine Rechnung ohne eigene Uhr.
//!
//! Der [`ProgressTracker`] bekommt die monotone Zeit (`Clock::monotonic`) bei jedem Aufruf übergeben und
//! rechnet daraus die Kennzahlen. Dadurch lassen sich alle Zeitkriterien mit `ManualClock` bzw. der
//! simulierten tokio-Zeit prüfen (M1-Plan, Regel 2.8).
//!
//! Definitionen (CRAWL-130):
//! * `crawled` = abgeschlossene Abrufe mit Ergebnis (auch Statuscode 0), ohne robots.txt.
//! * `queued` = eingeplante, noch nicht gestartete URLs.
//! * `rate` = gleitender Mittelwert der Abschlüsse über die letzten 10 s (in den ersten 10 s Laufzeit
//!   geteilt durch die bisherige Laufzeit, damit der Anfang nicht unterschätzt wird).
//! * `avg_rate` = abgeschlossene Abrufe ÷ Laufzeit ohne Pausen.
//! * `eta_s` = `queued ÷ rate`, erst nach 30 s Laufzeit **und** mindestens 100 gecrawlten URLs, nie bei Rate 0.
//! * `percent` = `crawled ÷ (crawled + queued) × 100` abgerundet; bei gesetztem URL-Limit gilt der größere
//!   Wert aus dieser Formel und `crawled ÷ Limit`; nach Abschluss immer 100.

use std::collections::VecDeque;
use std::time::Duration;

use os_model::{CrawlState, EndReason, Progress, Timestamp};

/// Fenster der Rate.
pub const RATE_WINDOW: Duration = Duration::from_secs(10);
/// Mindestlaufzeit, ab der eine Restzeit angezeigt wird.
pub const ETA_MIN_RUNTIME: Duration = Duration::from_secs(30);
/// Mindestzahl gecrawlter URLs, ab der eine Restzeit angezeigt wird.
pub const ETA_MIN_CRAWLED: u64 = 100;

/// Rechnet den Fortschritt eines Crawls aus.
#[derive(Debug, Clone)]
pub struct ProgressTracker {
    started_at: Timestamp,
    start_mono: Duration,
    state: CrawlState,
    end_reason: Option<EndReason>,
    crawled: u64,
    response_ms_sum: u64,
    response_count: u64,
    window: VecDeque<Duration>,
    paused_total: Duration,
    paused_since: Option<Duration>,
    /// Gesetztes URL-Limit (`None` = unbegrenzt).
    limit: Option<u64>,
    /// Zeitpunkt (monoton), zu dem der Crawl beendet wurde; danach läuft die Uhr nicht weiter.
    finished_at: Option<Duration>,
    /// Laufzeit vor dem aktuellen Lauf (Fortsetzen): Pausen eingeschlossen, die Wartezeit bis zum Fortsetzen
    /// gehört zu `paused_total`.
    elapsed_before: Duration,
}

impl ProgressTracker {
    /// Beginnt bei `now_mono` (monotone Zeit des Starts).
    pub fn new(started_at: Timestamp, now_mono: Duration, limit: Option<u64>) -> Self {
        ProgressTracker {
            started_at,
            start_mono: now_mono,
            state: CrawlState::Running,
            end_reason: None,
            crawled: 0,
            response_ms_sum: 0,
            response_count: 0,
            window: VecDeque::new(),
            paused_total: Duration::ZERO,
            paused_since: None,
            limit,
            finished_at: None,
            elapsed_before: Duration::ZERO,
        }
    }

    /// Setzt den Zählerstand, mit dem ein fortgesetzter Crawl weiterrechnet (P19).
    pub fn resume_from(&mut self, crawled: u64, paused_total: Duration) {
        self.crawled = crawled;
        self.paused_total = paused_total;
    }

    /// Wie [`resume_from`](Self::resume_from), zusätzlich mit der Laufzeit der früheren Läufe samt Pausen
    /// (`elapsed_s` zählt über Pause und Neustart hinweg weiter, `paused_s` enthält die Wartezeit).
    pub fn resume_with_elapsed(
        &mut self,
        crawled: u64,
        paused_total: Duration,
        elapsed_before: Duration,
    ) {
        self.resume_from(crawled, paused_total);
        self.elapsed_before = elapsed_before;
    }

    /// Ein Abruf ist abgeschlossen. `response_ms` nur bei echter Antwort.
    pub fn record_completion(&mut self, now: Duration, response_ms: Option<u32>) {
        self.crawled += 1;
        if let Some(ms) = response_ms {
            self.response_ms_sum += u64::from(ms);
            self.response_count += 1;
        }
        self.window.push_back(now);
        self.trim(now);
    }

    fn trim(&mut self, now: Duration) {
        while let Some(front) = self.window.front() {
            if now.saturating_sub(*front) >= RATE_WINDOW {
                self.window.pop_front();
            } else {
                break;
            }
        }
    }

    /// Beginn einer Pause.
    pub fn pause(&mut self, now: Duration) {
        if self.paused_since.is_none() {
            self.paused_since = Some(now);
            self.state = CrawlState::Paused;
        }
    }

    /// Ende einer Pause.
    pub fn resume(&mut self, now: Duration) {
        if let Some(since) = self.paused_since.take() {
            self.paused_total += now.saturating_sub(since);
            self.state = CrawlState::Running;
            // Abschlüsse vor der Pause sollen die Rate danach nicht verfälschen.
            self.window.clear();
        }
    }

    /// Der Crawl ist beendet (Zustand und Grund).
    pub fn finish(&mut self, now: Duration, state: CrawlState, reason: Option<EndReason>) {
        if let Some(since) = self.paused_since.take() {
            self.paused_total += now.saturating_sub(since);
        }
        self.state = state;
        self.end_reason = reason;
        self.finished_at = Some(now);
    }

    /// Anzahl abgeschlossener Abrufe.
    pub fn crawled(&self) -> u64 {
        self.crawled
    }

    /// Summe der Pausen bis `now`.
    pub fn paused_total(&self, now: Duration) -> Duration {
        let running = self
            .paused_since
            .map_or(Duration::ZERO, |s| now.saturating_sub(s));
        self.paused_total + running
    }

    /// Bildet den Stand zum Zeitpunkt `now`.
    pub fn snapshot(
        &mut self,
        now: Duration,
        queued: u64,
        discovered: u64,
        active_threads: u32,
    ) -> Progress {
        let now = self.finished_at.unwrap_or(now);
        self.trim(now);
        let elapsed = self.elapsed_before + now.saturating_sub(self.start_mono);
        let paused = self.paused_total(now);
        let active_elapsed = elapsed.saturating_sub(paused);

        let rate = if self.paused_since.is_some() || self.window.is_empty() {
            0.0
        } else {
            let span = active_elapsed.min(RATE_WINDOW);
            if span.is_zero() {
                0.0
            } else {
                self.window.len() as f64 / span.as_secs_f64()
            }
        };
        let avg_rate = if active_elapsed.is_zero() {
            0.0
        } else {
            self.crawled as f64 / active_elapsed.as_secs_f64()
        };
        let eta_s = if self.state == CrawlState::Running
            && rate > 0.0
            && active_elapsed >= ETA_MIN_RUNTIME
            && self.crawled >= ETA_MIN_CRAWLED
        {
            Some((queued as f64 / rate).ceil() as u64)
        } else {
            None
        };
        let avg_response_time = (self.response_count > 0)
            .then(|| self.response_ms_sum as f64 / self.response_count as f64 / 1000.0);

        Progress {
            state: self.state,
            end_reason: self.end_reason,
            crawled: self.crawled,
            queued,
            percent: self.percent(queued),
            rate,
            avg_response_time,
            elapsed_s: elapsed.as_secs(),
            eta_s,
            active_threads,
            started_at: self.started_at,
            avg_rate,
            paused_s: paused.as_secs(),
            discovered,
            // M2-E02 (CRAWL-129): Speicherstufe und Dateigröße
            memory_stage: None,
            file_bytes: None,
            memory_used_bytes: None,
            memory_limit_bytes: None,
        }
    }

    fn percent(&self, queued: u64) -> u8 {
        if self.state == CrawlState::Completed {
            return 100;
        }
        let total = self.crawled + queued;
        let mut percent = if total == 0 {
            0
        } else {
            (u128::from(self.crawled) * 100 / u128::from(total)) as u64
        };
        if let Some(limit) = self.limit.filter(|l| *l > 0) {
            let by_limit = (u128::from(self.crawled) * 100 / u128::from(limit)) as u64;
            percent = percent.max(by_limit);
        }
        percent.min(100) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_model::{Clock, ManualClock};

    fn start() -> Timestamp {
        Timestamp::from_db("2026-01-01T00:00:00.000Z").unwrap()
    }

    fn tracker(clock: &ManualClock, limit: Option<u64>) -> ProgressTracker {
        ProgressTracker::new(start(), clock.monotonic(), limit)
    }

    /// `n` Abschlüsse im Abstand `step`.
    fn run(clock: &ManualClock, t: &mut ProgressTracker, n: u32, step: Duration) {
        for _ in 0..n {
            clock.advance(step);
            t.record_completion(clock.monotonic(), Some(100));
        }
    }

    // spec: CRAWL-130
    #[test]
    fn rate_is_the_ten_second_window_average() {
        let clock = ManualClock::new(start());
        let mut t = tracker(&clock, None);
        // 1 Thread, 100 ms je Abruf: 10 Abschlüsse pro Sekunde, 20 s lang
        run(&clock, &mut t, 200, Duration::from_millis(100));
        clock.advance(Duration::from_millis(50));
        let p = t.snapshot(clock.monotonic(), 50, 250, 1);
        assert!(
            (7.0..=10.0).contains(&p.rate),
            "Rate nach 20 s: {} (erwartet 7–10)",
            p.rate
        );
        assert_eq!(p.crawled, 200);
        assert_eq!(p.avg_response_time, Some(0.1));
    }

    // spec: CRAWL-130
    #[test]
    fn rate_in_the_first_seconds_uses_the_elapsed_time() {
        let clock = ManualClock::new(start());
        let mut t = tracker(&clock, None);
        run(&clock, &mut t, 20, Duration::from_millis(100));
        clock.advance(Duration::from_millis(50));
        let p = t.snapshot(clock.monotonic(), 0, 20, 1);
        assert!((9.0..=10.0).contains(&p.rate), "{}", p.rate);
    }

    // spec: CRAWL-130
    #[test]
    fn eta_needs_thirty_seconds_and_one_hundred_urls() {
        let clock = ManualClock::new(start());
        let mut t = tracker(&clock, None);
        // 100 URLs in 20 s: genug URLs, zu wenig Zeit
        run(&clock, &mut t, 100, Duration::from_millis(200));
        let p = t.snapshot(clock.monotonic(), 400, 500, 5);
        assert_eq!(p.eta_s, None, "vor 30 s keine Restzeit");
        // 40 weitere URLs bis 30 s: Zeit erreicht, URLs reichen
        run(&clock, &mut t, 60, Duration::from_millis(200));
        let p = t.snapshot(clock.monotonic(), 400, 500, 5);
        assert!(p.eta_s.is_some(), "nach 32 s und 160 URLs: {p:?}");
        // Warteschlange / Rate: 400 URLs bei 5 pro Sekunde = 80 s
        assert_eq!(p.eta_s, Some(80));

        // viel Zeit, aber weniger als 100 URLs: keine Restzeit
        let clock = ManualClock::new(start());
        let mut t = tracker(&clock, None);
        run(&clock, &mut t, 50, Duration::from_millis(900));
        let p = t.snapshot(clock.monotonic(), 400, 500, 1);
        assert!(clock.monotonic() >= Duration::from_secs(30));
        assert_eq!(p.eta_s, None, "weniger als 100 URLs");
    }

    // spec: CRAWL-130
    #[test]
    fn eta_is_none_at_rate_zero() {
        let clock = ManualClock::new(start());
        let mut t = tracker(&clock, None);
        run(&clock, &mut t, 150, Duration::from_millis(250));
        assert!(t.snapshot(clock.monotonic(), 100, 250, 5).eta_s.is_some());
        // 11 s ohne Abschluss: Fenster leer, Rate 0
        clock.advance(Duration::from_secs(11));
        let p = t.snapshot(clock.monotonic(), 100, 250, 0);
        assert_eq!(p.rate, 0.0);
        assert_eq!(p.eta_s, None);
        // Pause: ebenfalls keine Restzeit
        let mut t = tracker(&clock, None);
        run(&clock, &mut t, 150, Duration::from_millis(250));
        t.pause(clock.monotonic());
        let p = t.snapshot(clock.monotonic(), 100, 250, 0);
        assert_eq!(p.state, CrawlState::Paused);
        assert_eq!((p.rate, p.eta_s), (0.0, None));
    }

    // spec: CRAWL-130
    #[test]
    fn percent_rounds_down_and_respects_the_limit() {
        let clock = ManualClock::new(start());
        let mut t = tracker(&clock, None);
        assert_eq!(t.snapshot(clock.monotonic(), 0, 0, 0).percent, 0);
        run(&clock, &mut t, 1, Duration::from_millis(10));
        // 1 / (1 + 2) = 33,33 → 33
        assert_eq!(t.snapshot(clock.monotonic(), 2, 3, 1).percent, 33);

        // Limit 10, 5 gecrawlt, 100 in der Warteschlange: 5/105 = 4 %, 5/10 = 50 % → 50
        let mut t = tracker(&clock, Some(10));
        run(&clock, &mut t, 5, Duration::from_millis(10));
        assert_eq!(t.snapshot(clock.monotonic(), 100, 105, 1).percent, 50);

        // nach Abschluss immer 100
        t.finish(
            clock.monotonic(),
            CrawlState::Completed,
            Some(EndReason::LimitReached),
        );
        let p = t.snapshot(clock.monotonic(), 100, 105, 0);
        assert_eq!(p.percent, 100);
        assert_eq!(p.end_reason, Some(EndReason::LimitReached));
    }

    // spec: CRAWL-130
    #[test]
    fn pause_of_ten_seconds_does_not_lower_the_average_rate() {
        let clock = ManualClock::new(start());
        let mut t = tracker(&clock, None);
        run(&clock, &mut t, 100, Duration::from_millis(100)); // 10 s, 10 pro Sekunde
        let before = t.snapshot(clock.monotonic(), 0, 100, 1);
        assert!((before.avg_rate - 10.0).abs() < 0.01, "{}", before.avg_rate);

        t.pause(clock.monotonic());
        clock.advance(Duration::from_secs(10));
        let paused = t.snapshot(clock.monotonic(), 0, 100, 0);
        assert_eq!(paused.paused_s, 10);
        assert_eq!(paused.elapsed_s, 20);
        assert!((paused.avg_rate - 10.0).abs() < 0.01, "{}", paused.avg_rate);

        t.resume(clock.monotonic());
        run(&clock, &mut t, 100, Duration::from_millis(100));
        let after = t.snapshot(clock.monotonic(), 0, 200, 1);
        assert_eq!(after.state, CrawlState::Running);
        assert_eq!(after.paused_s, 10);
        assert_eq!(after.elapsed_s, 30);
        assert!((after.avg_rate - 10.0).abs() < 0.01, "{}", after.avg_rate);
    }

    // spec: CRAWL-130
    #[test]
    fn finished_tracker_keeps_its_final_values() {
        let clock = ManualClock::new(start());
        let mut t = tracker(&clock, None);
        run(&clock, &mut t, 10, Duration::from_millis(100));
        t.finish(
            clock.monotonic(),
            CrawlState::Completed,
            Some(EndReason::QueueEmpty),
        );
        let a = t.snapshot(clock.monotonic(), 0, 10, 0);
        clock.advance(Duration::from_secs(60));
        let b = t.snapshot(clock.monotonic(), 0, 10, 0);
        assert_eq!(a.elapsed_s, b.elapsed_s);
        assert_eq!(a.avg_rate, b.avg_rate);
        assert_eq!(b.state, CrawlState::Completed);
    }
}
