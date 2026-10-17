//! Ressourcenschutz (CRAWL-129, K-3): Speicherstufen `normal`/`warning`/`paused` (80 % bzw. 90 % der RAM-Obergrenze) und
//! Messung der Dateigröße für `Progress.memory_stage`/`Progress.file_bytes`.
//!
//! M2-F02 legt nur das Modul an; die Umsetzung gehört M2-E02.

// STUB(M2-E02)

/// Eingaben des sekündlichen Ressourcenwächters (CRAWL-129).
pub struct MemoryInputs<'a> {
    pub progress: &'a os_model::Progress,
    pub config: &'a os_model::CrawlConfig,
}
#[derive(Debug, PartialEq, Eq)]
pub enum MemoryAction {
    None,
    Throttle(u32),
    Pause(String),
}
// STUB(M2-E02)
pub fn tick(input: &MemoryInputs<'_>) -> MemoryAction {
    #[cfg(test)]
    TICKS.with(|ticks| ticks.borrow_mut().push(input.progress.elapsed_s));
    let _ = input;
    MemoryAction::None
}

#[cfg(test)]
thread_local! {
    static TICKS: std::cell::RefCell<Vec<u64>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{SimFetcher, SimSite};
    use crate::{start_crawl, CrawlRequest, CrawlSource};
    use os_model::{CrawlConfig, ManualClock, Timestamp};
    use std::{sync::Arc, time::Duration};

    // spec: CRAWL-129
    // Infrastruktur (M2-F05a) für CRAWL-129; Fachprüfung M2-E02.
    #[tokio::test(start_paused = true)]
    async fn control_loop_calls_memory_tick_once_per_manual_clock_second() {
        TICKS.with(|ticks| ticks.borrow_mut().clear());
        let clock = Arc::new(ManualClock::new(Timestamp::UNIX_EPOCH));
        let mut deps = crate::testing::test_deps();
        deps.clock = clock.clone();
        let dir = tempfile::tempdir().unwrap();
        let fetcher = SimFetcher::new(
            SimSite::new()
                .page("https://site.test/", "<html>fixture</html>")
                .delay(Duration::from_secs(60)),
        );
        let handle = start_crawl(
            fetcher.clone(),
            CrawlRequest {
                config: CrawlConfig::default(),
                source: CrawlSource::Spider {
                    start_url: "https://site.test/".into(),
                },
                file: dir.path().join("memory.ospider"),
                project: "fixture".into(),
                name: "tick".into(),
            },
            deps,
        )
        .await
        .unwrap();
        // Die Steuerschleife muss vor dem ersten manuellen Zeitschritt laufen.
        while fetcher.max_concurrency() == 0 {
            tokio::task::yield_now().await;
        }
        for second in 1..=3 {
            clock.advance(Duration::from_secs(1));
            for _ in 0..10 {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            TICKS.with(|ticks| assert_eq!(*ticks.borrow(), (1..=second).collect::<Vec<_>>()));
        }
        handle.stop();
        handle.wait().await.unwrap();
    }
}
