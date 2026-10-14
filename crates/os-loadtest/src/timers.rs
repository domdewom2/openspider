//! Timer-Messung (M1-P30): Wie genau schlafen Betriebssystem-Thread und Tokio-Timer?
//!
//! Unter Windows beträgt die Auflösung der Systemuhr standardmäßig ~15,6 ms: `sleep(1 ms)`, `interval(1 ms)` und
//! `timeout` kehren dann erst nach einem ganzen Tick zurück, jede Schleife mit kurzem Schlafen verliert dort pro
//! Runde bis zu 14 ms. [`Monitors`] schläft während eines Crawls laufend 1 ms und verbucht, wie spät der Aufruf
//! zurückkehrt; blockiert ein Aufruf einen Runtime-Thread, zeigt sich das als große Verspätung des Tokio-Timers.
//! Aktiv nur mit `OS_LOADTEST_DIAG=1` (Kindprozess) und im Befehl `os-loadtest diag`.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Verspätungen von Schlafaufrufen.
#[derive(Debug, Default)]
pub struct Lateness {
    ticks: AtomicU64,
    sum_us: AtomicU64,
    max_us: AtomicU64,
    gt5ms: AtomicU64,
    gt20ms: AtomicU64,
    gt100ms: AtomicU64,
}

impl Lateness {
    /// Verbucht, dass ein Aufruf `late` nach dem gewünschten Zeitpunkt zurückkehrte.
    pub fn record(&self, late: Duration) {
        let us = u64::try_from(late.as_micros()).unwrap_or(u64::MAX);
        self.ticks.fetch_add(1, Ordering::Relaxed);
        self.sum_us.fetch_add(us, Ordering::Relaxed);
        self.max_us.fetch_max(us, Ordering::Relaxed);
        if us > 5_000 {
            self.gt5ms.fetch_add(1, Ordering::Relaxed);
        }
        if us > 20_000 {
            self.gt20ms.fetch_add(1, Ordering::Relaxed);
        }
        if us > 100_000 {
            self.gt100ms.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Die Zähler als `(Name, Wert)`; `mean_us` ist der Mittelwert der Verspätung.
    pub fn values(&self) -> Vec<(&'static str, u64)> {
        let ticks = self.ticks.load(Ordering::Relaxed);
        let sum = self.sum_us.load(Ordering::Relaxed);
        vec![
            ("ticks", ticks),
            ("mean_us", sum.checked_div(ticks).unwrap_or(0)),
            ("max_us", self.max_us.load(Ordering::Relaxed)),
            ("gt5ms", self.gt5ms.load(Ordering::Relaxed)),
            ("gt20ms", self.gt20ms.load(Ordering::Relaxed)),
            ("gt100ms", self.gt100ms.load(Ordering::Relaxed)),
        ]
    }
}

/// Wunschdauer der Messschläfe.
pub const TICK: Duration = Duration::from_millis(1);

/// Misst `n` Schläfe von `TICK` auf dem Betriebssystem-Thread.
pub fn measure_thread_sleep(n: usize) -> Lateness {
    let l = Lateness::default();
    for _ in 0..n {
        let t = Instant::now();
        std::thread::sleep(TICK);
        l.record(t.elapsed().saturating_sub(TICK));
    }
    l
}

/// Misst `n` Schläfe von `TICK` mit dem Tokio-Timer.
pub async fn measure_tokio_sleep(n: usize) -> Lateness {
    let l = Lateness::default();
    for _ in 0..n {
        let t = Instant::now();
        tokio::time::sleep(TICK).await;
        l.record(t.elapsed().saturating_sub(TICK));
    }
    l
}

/// Misst `n` Takte eines `tokio::time::interval(TICK)`: Verspätung gegen den Sollzeitpunkt `k * TICK`.
pub async fn measure_tokio_interval(n: usize) -> (Lateness, Duration) {
    let l = Lateness::default();
    let mut tick = tokio::time::interval(TICK);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    tick.tick().await;
    let start = Instant::now();
    let mut last = start;
    for _ in 0..n {
        tick.tick().await;
        let now = Instant::now();
        l.record(now.duration_since(last).saturating_sub(TICK));
        last = now;
    }
    (l, start.elapsed())
}

/// Laufende Messung während eines Crawls.
pub struct Monitors {
    stop: Arc<AtomicBool>,
    tokio_task: tokio::task::JoinHandle<()>,
    os_thread: std::thread::JoinHandle<()>,
    rt: Arc<Lateness>,
    os: Arc<Lateness>,
}

impl Monitors {
    /// Startet beide Messschleifen.
    pub fn start() -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let (rt, os) = (Arc::new(Lateness::default()), Arc::new(Lateness::default()));
        let tokio_task = {
            let (stop, rt) = (Arc::clone(&stop), Arc::clone(&rt));
            tokio::spawn(async move {
                while !stop.load(Ordering::Relaxed) {
                    let t = Instant::now();
                    tokio::time::sleep(TICK).await;
                    rt.record(t.elapsed().saturating_sub(TICK));
                }
            })
        };
        let os_thread = {
            let (stop, os) = (Arc::clone(&stop), Arc::clone(&os));
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let t = Instant::now();
                    std::thread::sleep(TICK);
                    os.record(t.elapsed().saturating_sub(TICK));
                }
            })
        };
        Self {
            stop,
            tokio_task,
            os_thread,
            rt,
            os,
        }
    }

    /// Beendet die Messung und liefert `(Gruppe, Werte)`: `rt` (Tokio-Timer) und `os_sleep` (Thread).
    pub async fn stop(self) -> Vec<(&'static str, Vec<(&'static str, u64)>)> {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.tokio_task.await;
        let _ = self.os_thread.join();
        vec![("rt", self.rt.values()), ("os_sleep", self.os.values())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Infrastruktur (M1-P30), kein Spec-Tag.
    #[test]
    fn lateness_buckets_and_mean() {
        let l = Lateness::default();
        l.record(Duration::from_millis(2));
        l.record(Duration::from_millis(30));
        l.record(Duration::from_millis(200));
        let v: std::collections::BTreeMap<_, _> = l.values().into_iter().collect();
        assert_eq!(v["ticks"], 3);
        assert_eq!(v["gt5ms"], 2);
        assert_eq!(v["gt20ms"], 2);
        assert_eq!(v["gt100ms"], 1);
        assert_eq!(v["max_us"], 200_000);
        assert_eq!(v["mean_us"], 77_333);
    }

    // Infrastruktur (M1-P30), kein Spec-Tag.
    #[tokio::test]
    async fn monitors_start_and_stop_and_see_ticks() {
        let m = Monitors::start();
        tokio::time::sleep(Duration::from_millis(50)).await;
        let groups = m.stop().await;
        assert_eq!(groups.len(), 2);
        for (_, values) in groups {
            assert!(values.iter().any(|(n, v)| *n == "ticks" && *v > 0));
        }
    }
}
