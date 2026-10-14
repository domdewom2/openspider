//! Kindprozess für PERF-001/003: crawlt eine Site in eine Crawl-Datei (`os-loadtest crawl-child ...`).
//!
//! Der Elternprozess ([`crate::engine::EngineCrawler`]) misst von außen den Spitzen-RSS dieses Prozesses. Das
//! Protokoll auf stdout ist dasselbe wie bei `examples/crawl_child.rs` von os-engine (P19), damit ein Parser
//! beide Kinder liest:
//!
//! * `READY pid=<pid>` – die Crawl-Datei existiert, der Crawl läuft.
//! * `PROGRESS crawled=<n> queued=<n> discovered=<n>` – bei jeder Änderung von `crawled`, höchstens alle 20 ms.
//! * `DONE state=<…> end_reason=<…|-> crawled=<n>` – Abschluss oder Pause.
//!
//! Gegenüber `crawl_child` kann dieses Kind die Senke tauschen (`--slow-sink-ms`, PERF-003) und das
//! Speicherprofil wählen (`--profile`, CRAWL-124).

use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use os_engine::{start_crawl, CrawlHandle, CrawlRequest, CrawlSource, CrawlSummary, EngineDeps};
use os_fetch::{FetchConfig, HttpFetcher, MemorySecretStore};
use os_model::{CrawlConfig, CrawlState, RandomIds, SystemClock};
use os_store::StorageProfile;

use crate::sink::tuned_sink_factory;

/// Parameter des Kindes.
#[derive(Debug, Clone)]
pub struct ChildArgs {
    /// Zu erzeugende Crawl-Datei.
    pub file: PathBuf,
    /// Start-URL.
    pub start: String,
    /// `network.resolve`-Einträge (`host=ip:port`).
    pub resolve: Vec<String>,
    /// Parallele Abrufe.
    pub threads: u32,
    /// Speicherprofil.
    pub profile: StorageProfile,
    /// Verzögerung je Batch der Senke in Millisekunden (`None` = normale `SqliteSink`).
    pub slow_sink_ms: Option<u64>,
    /// `PRAGMA wal_autocheckpoint` in Seiten (`None` = Standard von SQLite, 1000).
    pub wal_autocheckpoint: Option<u32>,
}

/// Name eines Profils auf der Kommandozeile.
pub fn profile_name(p: StorageProfile) -> &'static str {
    match p {
        StorageProfile::Balanced => "balanced",
        StorageProfile::MaxSpeed => "max-speed",
        StorageProfile::MinMemory => "min-memory",
    }
}

/// Profil aus dem Namen auf der Kommandozeile.
pub fn parse_profile(s: &str) -> Result<StorageProfile, String> {
    match s {
        "balanced" => Ok(StorageProfile::Balanced),
        "max-speed" => Ok(StorageProfile::MaxSpeed),
        "min-memory" => Ok(StorageProfile::MinMemory),
        other => Err(format!(
            "unbekanntes Profil `{other}` (balanced, max-speed, min-memory)"
        )),
    }
}

fn say(line: &str) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

fn report(summary: &CrawlSummary) {
    say(&format!(
        "DONE state={} end_reason={} crawled={}",
        summary.state.as_str(),
        summary.end_reason.map_or("-", |r| r.as_str()),
        summary.crawled
    ));
}

/// Beendet das Warten, sobald der Crawl einen Endzustand meldet (das Handle wird erst danach verbraucht).
async fn wait_for_end(handle: &CrawlHandle) {
    let mut watch = handle.watch();
    loop {
        let state = watch.borrow().state;
        if matches!(
            state,
            CrawlState::Completed | CrawlState::Paused | CrawlState::Failed
        ) {
            return;
        }
        if watch.changed().await.is_err() {
            return;
        }
    }
}

/// Führt den Crawl aus; Rückgabe ist der Exit-Code (0 bei Abschluss).
pub async fn run(args: ChildArgs) -> Result<i32, String> {
    let mut cfg = CrawlConfig::default();
    cfg.network.resolve.extend(args.resolve.iter().cloned());
    cfg.speed.threads = args.threads;
    let clock = Arc::new(SystemClock);
    let mut deps = EngineDeps {
        clock: clock.clone(),
        ids: Arc::new(RandomIds),
        profile: args.profile,
        secrets: Arc::new(MemorySecretStore::new()),
        sink_factory: None,
        on_finished: None,
        owner_kind: "cli",
    };
    if args.slow_sink_ms.is_some() || args.wal_autocheckpoint.is_some() {
        deps.sink_factory = Some(tuned_sink_factory(
            args.slow_sink_ms.map(Duration::from_millis),
            args.wal_autocheckpoint,
            clock,
        ));
    }
    let fetch_cfg = FetchConfig::from_crawl_config(&cfg, &*deps.secrets, deps.clock.clone())
        .map_err(|e| e.to_string())?;
    let fetcher = Arc::new(HttpFetcher::new(fetch_cfg).map_err(|e| e.to_string())?);
    let handle = start_crawl(
        fetcher,
        CrawlRequest {
            config: cfg,
            source: CrawlSource::Spider {
                start_url: args.start.clone(),
            },
            file: args.file.clone(),
            project: "loadtest".to_string(),
            name: "loadtest".to_string(),
        },
        deps,
    )
    .await
    .map_err(|e| e.to_string())?;
    say(&format!("READY pid={}", std::process::id()));

    let mut watch = handle.watch();
    tokio::spawn(async move {
        let mut last = u64::MAX;
        while watch.changed().await.is_ok() {
            let p = watch.borrow_and_update().clone();
            if p.crawled != last {
                last = p.crawled;
                say(&format!(
                    "PROGRESS crawled={} queued={} discovered={}",
                    p.crawled, p.queued, p.discovered
                ));
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    });
    wait_for_end(&handle).await;
    let summary = handle.wait().await.map_err(|e| e.to_string())?;
    report(&summary);
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_names_round_trip() {
        for p in [
            StorageProfile::Balanced,
            StorageProfile::MaxSpeed,
            StorageProfile::MinMemory,
        ] {
            assert_eq!(parse_profile(profile_name(p)).unwrap(), p);
        }
        assert!(parse_profile("memory").is_err());
    }
}
