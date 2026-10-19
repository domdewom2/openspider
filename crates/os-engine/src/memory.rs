//! Sekündlicher Ressourcenschutz: Engine-RSS plus Renderer, RAM-Grenze und freier RAM.
use os_model::{settings::StorageSettings, MemoryStage};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

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

#[derive(Clone, Copy, Debug, Default)]
pub struct Sample {
    pub engine_bytes: u64,
    pub renderer_bytes: u64,
    pub physical_bytes: u64,
    pub available_bytes: u64,
    /// Nur für deterministische Tests; App-Einstellungen behalten mindestens 1 GiB.
    pub injected_limit: Option<u64>,
}
pub type Probe = Arc<dyn Fn() -> Sample + Send + Sync>;
static PROBE: Mutex<Option<Probe>> = Mutex::new(None);
static TEST_LIMIT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// Injiziert nur die Grenze, ohne die echte Prozessmessung zu ersetzen.
#[cfg(feature = "testing")]
pub fn set_limit_for_test(bytes: Option<u64>) {
    TEST_LIMIT.store(bytes.unwrap_or(0), std::sync::atomic::Ordering::Relaxed);
}
static SETTINGS: OnceLock<Mutex<StorageSettings>> = OnceLock::new();

pub fn set_probe(probe: Option<Probe>) {
    *PROBE.lock().unwrap_or_else(|p| p.into_inner()) = probe;
}
pub fn apply_settings(settings: &StorageSettings) {
    if let Some(gb) = settings.ram_limit_gb {
        let requested = (gb * (1_u64 << 30) as f64) as u64;
        let ram = limits(settings, sample().physical_bytes, 1).0;
        if requested != ram {
            tracing::warn!(
                requested,
                ram,
                "RAM-Obergrenze an physischen Speicher angepasst"
            );
        }
    }
    os_store::profile::set_cache_limit(settings.cache_limit_mb.map(|mb| u64::from(mb) << 20));
    *SETTINGS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|p| p.into_inner()) = settings.clone();
}
pub fn limits(settings: &StorageSettings, physical: u64, cores: u32) -> (u64, u64, u32) {
    let gib = 1_u64 << 30;
    let upper = physical.saturating_sub(2 * gib).max(gib);
    let requested = settings
        .ram_limit_gb
        .map(|gb| (gb * gib as f64) as u64)
        .unwrap_or(physical / 2);
    let ram = requested.clamp(gib, upper);
    let cache = settings
        .cache_limit_mb
        .map(|mb| u64::from(mb) << 20)
        .unwrap_or((physical / 4).max(512 << 20));
    let renderers = settings
        .render_process_limit
        .unwrap_or((cores / 2).clamp(1, 8))
        .clamp(1, 32);
    (ram, cache, renderers)
}
/// Ressourcenvertrag für den Renderer und die Diagnose; Änderungen wirken beim nächsten Abruf.
pub fn resource_limits() -> (u64, u64, u32) {
    let settings = SETTINGS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get() as u32);
    limits(&settings, sample().physical_bytes, cores)
}

#[derive(Debug)]
pub struct Diagnostics {
    pub memory: Sample,
    pub limit_bytes: u64,
    pub cache_limit_bytes: u64,
    pub render_process_limit: u32,
    pub file_bytes: u64,
    pub free_disk_bytes: Option<u64>,
    pub profile: os_store::StorageProfile,
}
pub fn diagnostics(file: &std::path::Path, profile: os_store::StorageProfile) -> Diagnostics {
    let (limit_bytes, cache_limit_bytes, render_process_limit) = resource_limits();
    Diagnostics {
        memory: sample(),
        limit_bytes,
        cache_limit_bytes,
        render_process_limit,
        file_bytes: std::fs::metadata(file).map_or(0, |m| m.len()),
        free_disk_bytes: crate::disk_guard::status(file).ok().map(|s| s.free_bytes),
        profile,
    }
}

impl Sample {
    pub fn used(self) -> u64 {
        self.engine_bytes.saturating_add(self.renderer_bytes)
    }
    pub fn limit(self) -> u64 {
        self.injected_limit
            .or_else(|| {
                let limit = TEST_LIMIT.load(std::sync::atomic::Ordering::Relaxed);
                (limit > 0).then_some(limit)
            })
            .unwrap_or_else(|| {
                limits(
                    &SETTINGS
                        .get_or_init(Default::default)
                        .lock()
                        .unwrap_or_else(|p| p.into_inner()),
                    self.physical_bytes,
                    1,
                )
                .0
            })
    }
    pub fn stage(self) -> MemoryStage {
        stage(self.used(), self.limit())
    }
}
pub fn stage(used: u64, limit: u64) -> MemoryStage {
    if limit == 0 {
        return MemoryStage::Normal;
    }
    if u128::from(used) * 100 >= u128::from(limit) * 90 {
        MemoryStage::Paused
    } else if u128::from(used) * 100 >= u128::from(limit) * 80 {
        MemoryStage::Warning
    } else {
        MemoryStage::Normal
    }
}

pub fn sample() -> Sample {
    let probe = PROBE.lock().unwrap_or_else(|p| p.into_inner()).clone();
    if let Some(probe) = probe {
        return probe();
    }
    static SYSTEM: OnceLock<Mutex<(sysinfo::System, Option<Instant>, Sample)>> = OnceLock::new();
    let mut state = SYSTEM
        .get_or_init(|| Mutex::new((sysinfo::System::new(), None, Sample::default())))
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if state
        .1
        .is_some_and(|at| at.elapsed() < Duration::from_secs(1))
    {
        return state.2;
    }
    let sys = &mut state.0;
    sys.refresh_memory();
    sys.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::All,
        true,
        sysinfo::ProcessRefreshKind::nothing().with_memory(),
    );
    let pid = sysinfo::Pid::from_u32(std::process::id());
    let mut descendants = std::collections::HashSet::from([pid]);
    loop {
        let before = descendants.len();
        for (id, process) in sys.processes() {
            if process.parent().is_some_and(|p| descendants.contains(&p)) {
                descendants.insert(*id);
            }
        }
        if before == descendants.len() {
            break;
        }
    }
    let result = Sample {
        engine_bytes: sys.process(pid).map_or(0, |p| p.memory()),
        renderer_bytes: sys
            .processes()
            .iter()
            .filter(|(id, _)| **id != pid && descendants.contains(id))
            .map(|(_, p)| p.memory())
            .sum(),
        physical_bytes: sys.total_memory(),
        available_bytes: sys.available_memory(),
        injected_limit: None,
    };
    state.1 = Some(Instant::now());
    state.2 = result;
    result
}

pub(crate) fn guard_start<F: os_fetch::Fetch>(sh: &crate::worker::Shared<F>) {
    let measured = sample();
    if measured.stage() == MemoryStage::Paused {
        *sh.ctl.reason.lock().unwrap_or_else(|p| p.into_inner()) = Some(("memory", "Wenig Arbeitsspeicher – Crawl pausiert; Threads reduzieren, Rendering oder HTML-Speicherung abschalten".into()));
        sh.ctl.stop.store(
            crate::worker::STOP_PAUSE,
            std::sync::atomic::Ordering::SeqCst,
        );
    }
}

pub fn tick(input: &MemoryInputs<'_>) -> MemoryAction {
    #[cfg(test)]
    TICKS.with(|ticks| ticks.borrow_mut().push(input.progress.elapsed_s));
    let measured = sample();
    let used = input.progress.memory_used_bytes.unwrap_or(measured.used());
    let limit = input
        .progress
        .memory_limit_bytes
        .unwrap_or(measured.limit());
    if stage(used, limit) == MemoryStage::Paused {
        let message = format!("Wenig Arbeitsspeicher – Crawl pausiert ({used}/{limit} Bytes). Threads reduzieren; Rendering abschalten; HTML-Speicherung abschalten.");
        tracing::warn!(used, limit, "{message}");
        MemoryAction::Pause(message)
    } else if measured.physical_bytes > 0
        && u128::from(measured.available_bytes) * 10 < u128::from(measured.physical_bytes)
    {
        tracing::warn!("Wenig verfügbarer RAM – Crawl gedrosselt");
        MemoryAction::Throttle((input.config.speed.threads / 2).max(1))
    } else {
        MemoryAction::None
    }
}

/// Pausiert lange App-Operationen an ihren vorhandenen Checkpoints, ohne sie abzubrechen.
pub fn watch_operation(token: os_model::CancelToken) -> OperationGuard {
    let task = tokio::spawn(async move {
        let mut paused = false;
        loop {
            if token.is_cancelled() {
                break;
            }
            match sample().stage() {
                MemoryStage::Paused => {
                    token.pause();
                    paused = true;
                }
                MemoryStage::Normal if paused => {
                    token.resume();
                    paused = false;
                }
                _ => {}
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
    OperationGuard(task)
}
pub struct OperationGuard(tokio::task::JoinHandle<()>);
impl Drop for OperationGuard {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[cfg(test)]
thread_local! {
    static TICKS: std::cell::RefCell<Vec<u64>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Wendet die app-weite Parallelitätsgrenze auf den Renderer des Crawls an.
/// Der Permit bleibt während des gesamten Renderaufrufs belegt, auch bei Abbruch/Fehlern.
pub fn resource_deps(mut deps: crate::EngineDeps) -> crate::EngineDeps {
    if let Some(renderer) = deps.renderer.take() {
        deps.renderer = Some(Arc::new(LimitedRenderer(renderer)));
    }
    deps
}
struct LimitedRenderer(Arc<dyn crate::render_glue::Renderer>);
static RENDER_ACTIVE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
struct RenderPermit;
impl Drop for RenderPermit {
    fn drop(&mut self) {
        RENDER_ACTIVE.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}
impl crate::render_glue::Renderer for LimitedRenderer {
    fn render<'a>(
        &'a self,
        url: &'a str,
        opts: &'a os_render::RenderOpts,
    ) -> futures_util::future::BoxFuture<'a, Result<os_render::RenderedPage, os_render::RenderError>>
    {
        Box::pin(async move {
            loop {
                let limit = resource_limits().2;
                let active = RENDER_ACTIVE.load(std::sync::atomic::Ordering::SeqCst);
                if active < limit
                    && RENDER_ACTIVE
                        .compare_exchange(
                            active,
                            active + 1,
                            std::sync::atomic::Ordering::SeqCst,
                            std::sync::atomic::Ordering::SeqCst,
                        )
                        .is_ok()
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            let _permit = RenderPermit;
            self.0.render(url, opts).await
        })
    }
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
