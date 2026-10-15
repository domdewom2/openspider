//! `EngineCrawler` – os-engine als [`CrawlerUnderTest`] (PERF-001, PERF-003, PERF-005).
//!
//! Der Crawl läuft immer in einem **Kindprozess**, damit der Spitzen-RSS des Kernprozesses (ohne Testserver und
//! Runner) gemessen werden kann (`rss`) und ein harter Abbruch (PERF-005) den Elternprozess nicht trifft.
//!
//! | Szenario | Ablauf | Kriterien (Spec) |
//! |---|---|---|
//! | `perf-001` | Referenzcrawl mit 1/10 der Seiten, dann der Hauptcrawl (je `os-loadtest crawl-child`) | RSS < 1,5 GB; RSS(Haupt)/RSS(Referenz) < 2 (beides nicht im Profil „Maximale Geschwindigkeit“, K-16); Cache-Grenze des Profils „Maximale Geschwindigkeit“ ≤ 25 % RAM und ≤ 4 GiB; kein Stillstand > 30 s (PERF-003, K-73); ab 1 000 000 Seiten PERF-002: Mittel ≥ 500 URLs/s, zehn Abschnitte zu 70 % bis 130 % des Mittels (K-72), darunter nur Bericht |
//! | `perf-003` | ein Crawl mit [`SlowSink`](crate::sink::SlowSink) (100 ms je Batch) | RSS < 500 MB; der Crawl läuft weiter (kein Stillstand > 30 s) und endet; die zehn Abschnitte nach PERF-002 nur im Bericht |
//! | `perf-005` | Referenzcrawl; Crawl mit gesäten harten Abbrüchen (`examples/crawl_child.rs`), nach jedem Abbruch `integrity_check` und Fortsetzen | `integrity_check = ok`; URL-Menge und Status je URL wie der Referenzcrawl; keine URL doppelt; wiederholte Requests je Abbruch höchstens Threads + Kanal + größter Batch (K-49), einzeln geprüft |
//!
//! Alle übrigen Szenarien liefern [`CrawlerError::NotImplemented`] (Rendering, Nachlauf-Analysen und Exporte
//! gehören anderen Paketen).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use os_model::time::IdSource;
use os_model::{Clock, CrawlConfig, CrawlMode, RandomIds, SystemClock, Timestamp};
use os_store::{profile, CrawlFile, CrawlInit, OpenMode, StorageProfile};
use os_testsite::{SiteOptions, SyntheticConfig, TestSite};
use serde_json::{json, Value};

use crate::child::profile_name;
use crate::crawler::{CrawlFuture, CrawlJob, CrawlOutcome, CrawlerError, CrawlerUnderTest};
use crate::proc::{example_binary, Proc, RunResult};
use crate::scenario::Size;
use crate::throughput::{self, segment_rates};

/// PERF-001: Spitzen-RSS des Kernprozesses (1,5 GB).
pub const MAX_RSS_BYTES: u64 = 1_500_000_000;
/// PERF-001: Verhältnis der Spitzen-RSS bei zehnfacher Seitenzahl (kleiner als Faktor 2).
pub const MAX_RSS_RATIO: f64 = 2.0;
/// PERF-003: Spitzen-RSS bei verlangsamtem Store (500 MB).
pub const BACKPRESSURE_RSS_BYTES: u64 = 500_000_000;
/// Längste erlaubte Zeit ohne Fortschritt des Crawls: `docs/spec/07-performance.md` PERF-003, zweites Kriterium
/// („nie mehr als 30 s zwischen zwei Fortschrittsmeldungen mit steigender Zahl abgerufener URLs“, K-73). Gilt für
/// `perf-003` und für den Crawl von `perf-001`.
pub const MAX_STALL_SECS: f64 = 30.0;
/// Text des Berichtsfelds `rss_limits` im Profil „Maximale Geschwindigkeit“ (K-16): die RSS-Grenzen von PERF-001 gelten
/// dort nicht, weil der Cache bis zu 25 % des RAM beträgt.
pub const RSS_LIMITS_NOT_JUDGED: &str = "nicht bewertet (Profil max-speed, K-16)";
/// PERF-001/K-16: Obergrenze des Caches im Profil „Maximale Geschwindigkeit“.
pub const CACHE_CAP_BYTES: u64 = 4 * 1024 * 1024 * 1024;
/// PERF-005/K-49: Was je Abbruch ungeschrieben bleiben darf, wenn der Writer dem Crawl nicht folgt, zusätzlich zu den
/// Abrufen in Bearbeitung (Threads): der volle Kanal zum Writer-Thread plus der größte Batch der Writer-Stufe
/// (`writer_thread::CHANNEL_CAPACITY` + `writer_thread::BATCH_PAGES_BACKLOG`). Mehr bedeutet unbegrenztes Puffern.
/// Die frühere „Spec-Grenze“ Threads + 200 entfällt (Entscheidung K-49): die Spec verlangt „keine Duplikate, keine
/// Verluste“; wiederholt werden dürfen genau die URLs, die beim Abbruch in Bearbeitung oder abgerufen, aber noch nicht
/// persistiert waren.
pub const REPEAT_BUFFER_ALLOWANCE: u64 = (os_engine::writer_thread::CHANNEL_CAPACITY
    + os_engine::writer_thread::BATCH_PAGES_BACKLOG)
    as u64;
/// Standard für `threads`.
pub const DEFAULT_THREADS: u32 = 8;
/// Host, unter dem PERF-005 die Testsite anspricht (Auflösung über `network.resolve`), damit die URLs beider Crawls
/// gleich sind.
const HOST: &str = "site.test";

/// os-engine als getesteter Crawler.
#[derive(Debug, Clone)]
pub struct EngineCrawler {
    /// Programm, das `crawl-child` kennt (`os-loadtest`); meist `std::env::current_exe()`.
    pub child_exe: PathBuf,
    /// Parallele Abrufe.
    pub threads: u32,
    /// Speicherprofil der Crawl-Dateien.
    pub profile: StorageProfile,
    /// Seed für die Abbruchzeitpunkte (PERF-005).
    pub seed: u64,
    /// Anzahl harter Abbrüche (`None` = 3 bei `small`, 5 bei `large`).
    pub kills: Option<usize>,
    /// Ordner für Crawl-Dateien (`None` = temporärer Ordner); die Dateien werden nach dem Lauf gelöscht.
    pub work_dir: Option<PathBuf>,
    /// Kein Fortschritt länger als dies gilt als Stillstand.
    pub stall_timeout: Duration,
}

impl EngineCrawler {
    /// Mit Standardwerten; `child_exe` ist das `os-loadtest`-Programm.
    pub fn new(child_exe: impl Into<PathBuf>) -> Self {
        Self {
            child_exe: child_exe.into(),
            threads: DEFAULT_THREADS,
            profile: StorageProfile::Balanced,
            seed: 20_261_014,
            kills: None,
            work_dir: None,
            stall_timeout: Duration::from_secs(600),
        }
    }

    fn scratch(&self) -> Result<tempfile::TempDir, String> {
        match &self.work_dir {
            Some(dir) => {
                std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                tempfile::Builder::new()
                    .prefix("os-loadtest-")
                    .tempdir_in(dir)
            }
            None => tempfile::Builder::new().prefix("os-loadtest-").tempdir(),
        }
        .map_err(|e| format!("Arbeitsordner: {e}"))
    }

    fn own_child(&self, file: &Path, start: &str, slow_sink_ms: Option<u64>) -> Command {
        let mut cmd = Command::new(&self.child_exe);
        cmd.arg("crawl-child")
            .arg("--file")
            .arg(file)
            .args(["--start", start])
            .args(["--threads", &self.threads.to_string()])
            .args(["--profile", profile_name(self.profile)]);
        if let Some(ms) = slow_sink_ms {
            cmd.args(["--slow-sink-ms", &ms.to_string()]);
        }
        cmd
    }

    async fn blocking<T: Send + 'static>(
        f: impl FnOnce() -> Result<T, String> + Send + 'static,
    ) -> Result<T, CrawlerError> {
        tokio::task::spawn_blocking(f)
            .await
            .map_err(|e| CrawlerError::Failed(format!("Lasttest-Thread: {e}")))?
            .map_err(CrawlerError::Failed)
    }

    /// Ein vollständiger Crawl in einem eigenen Kind (`crawl-child`).
    async fn run_plain(
        &self,
        start: String,
        file: PathBuf,
        slow_sink_ms: Option<u64>,
    ) -> Result<RunResult, CrawlerError> {
        let cmd = self.own_child(&file, &start, slow_sink_ms);
        let stall = self.stall_timeout;
        Self::blocking(move || {
            let result = Proc::spawn(cmd, stall)?.finish()?;
            if result.state != "Completed" {
                return Err(format!(
                    "der Crawl endete mit Zustand {} ({})",
                    result.state, result.end_reason
                ));
            }
            Ok(result)
        })
        .await
    }

    // ---- PERF-001 ----------------------------------------------------------------------------------------

    async fn perf001(&self, job: CrawlJob) -> Result<CrawlOutcome, CrawlerError> {
        let scratch = self.scratch().map_err(CrawlerError::Failed)?;
        let ref_pages = (job.site.pages / 10).max(100);
        let ref_cfg = SyntheticConfig {
            pages: ref_pages,
            ..job.site.clone()
        };
        let ref_site = TestSite::start_synthetic(ref_cfg, SiteOptions::default())
            .await
            .map_err(|e| CrawlerError::Failed(format!("Referenz-Site: {e}")))?;

        let ref_file = scratch.path().join("ref.ospider");
        let main_file = scratch.path().join("main.ospider");
        let reference = self
            .run_plain(ref_site.url("/"), ref_file.clone(), None)
            .await?;
        let ref_file_bytes = std::fs::metadata(&ref_file).map(|m| m.len()).unwrap_or(0);
        let _ = std::fs::remove_file(&ref_file);
        let main = self
            .run_plain(job.start_url.clone(), main_file.clone(), None)
            .await?;
        let file_bytes = std::fs::metadata(&main_file).map(|m| m.len()).unwrap_or(0);

        let mut violations = Vec::new();
        let (rss, ref_rss) = (main.peak_rss_bytes, reference.peak_rss_bytes);
        let cache_bytes = self
            .profile
            .cache_size_kib()
            .unsigned_abs()
            .saturating_mul(1024);
        let judged = judge_rss_scaling(RssScaling {
            pages: job.site.pages,
            reference_pages: ref_pages,
            rss: rss.unwrap_or(0),
            reference_rss: ref_rss.unwrap_or(0),
            file_bytes,
            reference_file_bytes: ref_file_bytes,
            cache_bytes,
        });
        violations.extend(rss_limit_violations(
            self.profile,
            job.site.pages,
            rss,
            &judged,
        ));

        // PERF-003 (K-73): auch der Crawl von PERF-001 steht nie länger als 30 s still
        violations.extend(stall_violation(
            "PERF-003 (Crawl aus PERF-001)",
            main.max_stall_s,
        ));

        // PERF-002 (K-72): Durchsatz in zehn Abschnitten, Urteil erst ab 1 000 000 Seiten
        let mean_urls_per_s = main.crawled as f64 / main.seconds.max(1e-9);
        let rates = segment_rates(&main.samples, main.crawled);
        violations.extend(throughput::judge(
            job.site.pages,
            mean_urls_per_s,
            rates.as_deref().unwrap_or(&[]),
        ));

        // Cache-Grenze (K-16) über die PRAGMA-Werte des Profils „Maximale Geschwindigkeit“
        let cache = max_speed_cache(scratch.path());
        match &cache {
            Ok(c) => {
                if let Err(msg) = check_cache_limit(c.cache_size_kib, c.ram_bytes) {
                    violations.push(msg);
                }
            }
            Err(e) => violations.push(format!("PERF-001: Cache-Grenze nicht prüfbar: {e}")),
        }

        let mut extra = BTreeMap::new();
        extra.insert("profile".into(), json!(profile_name(self.profile)));
        extra.insert("threads".into(), json!(self.threads));
        extra.insert("reference_pages".into(), json!(ref_pages));
        extra.insert("reference_peak_rss_bytes".into(), json!(ref_rss));
        extra.insert("rss_ratio".into(), json!(judged.ratio.map(|q| round(q, 3))));
        extra.insert("rss_scaling_rule".into(), json!(judged.rule));
        extra.insert("file_bytes".into(), json!(file_bytes));
        extra.insert("reference_file_bytes".into(), json!(ref_file_bytes));
        extra.insert("profile_cache_bytes".into(), json!(cache_bytes));
        extra.insert("urls_per_s".into(), json!(round(mean_urls_per_s, 1)));
        extra.insert("diag".into(), json!(main.diag));
        extra.insert("max_stall_s".into(), json!(round(main.max_stall_s, 3)));
        for (k, v) in throughput::report_fields(rates.as_deref(), mean_urls_per_s) {
            extra.insert(k.into(), v);
        }
        extra.insert(
            "rss_limits".into(),
            json!(if rss_limits_apply(self.profile) {
                "bewertet"
            } else {
                RSS_LIMITS_NOT_JUDGED
            }),
        );
        extra.insert("limit_peak_rss_bytes".into(), json!(MAX_RSS_BYTES));
        extra.insert("limit_rss_ratio".into(), json!(MAX_RSS_RATIO));
        if let Ok(c) = &cache {
            extra.insert("max_speed_cache_kib".into(), json!(c.cache_size_kib));
            extra.insert("physical_ram_bytes".into(), json!(c.ram_bytes));
        }
        Ok(CrawlOutcome {
            urls_crawled: main.crawled,
            duration: Duration::from_secs_f64(main.seconds),
            peak_rss_bytes: main.peak_rss_bytes,
            extra,
            violations,
            warnings: Vec::new(),
        })
    }

    // ---- PERF-003 ----------------------------------------------------------------------------------------

    async fn perf003(&self, job: CrawlJob) -> Result<CrawlOutcome, CrawlerError> {
        let scratch = self.scratch().map_err(CrawlerError::Failed)?;
        let delay_ms = crate::sink::SPEC_DELAY.as_millis() as u64;
        let file = scratch.path().join("slow.ospider");
        let run = self
            .run_plain(job.start_url.clone(), file, Some(delay_ms))
            .await?;

        let mut violations = Vec::new();
        match run.peak_rss_bytes {
            Some(r) if r >= BACKPRESSURE_RSS_BYTES => violations.push(format!(
                "PERF-003: Spitzen-RSS {} MB >= 500 MB bei verlangsamtem Store",
                r / 1_000_000
            )),
            None => violations.push("PERF-003: Spitzen-RSS nicht messbar".to_string()),
            _ => {}
        }
        violations.extend(stall_violation("PERF-003", run.max_stall_s));
        // PERF-002 nur im Bericht (K-72: das Urteil gehört zum Crawl von perf-001)
        let mean_urls_per_s = run.crawled as f64 / run.seconds.max(1e-9);
        let rates = segment_rates(&run.samples, run.crawled);

        let mut extra = BTreeMap::new();
        extra.insert("profile".into(), json!(profile_name(self.profile)));
        extra.insert("threads".into(), json!(self.threads));
        extra.insert("sink_delay_ms".into(), json!(delay_ms));
        extra.insert("end_reason".into(), json!(run.end_reason));
        extra.insert("max_stall_s".into(), json!(round(run.max_stall_s, 3)));
        extra.insert("urls_per_s".into(), json!(round(mean_urls_per_s, 1)));
        for (k, v) in throughput::report_fields(rates.as_deref(), mean_urls_per_s) {
            extra.insert(k.into(), v);
        }
        extra.insert("diag".into(), json!(run.diag));
        extra.insert("limit_peak_rss_bytes".into(), json!(BACKPRESSURE_RSS_BYTES));
        Ok(CrawlOutcome {
            urls_crawled: run.crawled,
            duration: Duration::from_secs_f64(run.seconds),
            peak_rss_bytes: run.peak_rss_bytes,
            extra,
            violations,
            warnings: Vec::new(),
        })
    }

    // ---- PERF-005 ----------------------------------------------------------------------------------------

    async fn perf005(&self, job: CrawlJob) -> Result<CrawlOutcome, CrawlerError> {
        let scratch = self.scratch().map_err(CrawlerError::Failed)?;
        let ref_site = TestSite::start_synthetic(job.site.clone(), SiteOptions::default())
            .await
            .map_err(|e| CrawlerError::Failed(format!("Referenz-Site: {e}")))?;
        let test_site = TestSite::start_synthetic(
            job.site.clone(),
            SiteOptions {
                port: 0,
                log_requests: true,
            },
        )
        .await
        .map_err(|e| CrawlerError::Failed(format!("Site: {e}")))?;
        let example = example_binary(&self.child_exe, "os-engine", "crawl_child")
            .map_err(CrawlerError::Failed)?;
        let kills = self.kills.unwrap_or(match job.size {
            Size::Small => 3,
            Size::Large => 5,
        });
        let plan = Plan005 {
            example,
            dir: scratch.path().to_path_buf(),
            ref_addr: ref_site.addr().to_string(),
            test_addr: test_site.addr().to_string(),
            threads: self.threads,
            pages: job.site.pages,
            kill_points: kill_points(self.seed, kills, job.site.pages),
            seed: self.seed,
            stall: self.stall_timeout,
        };
        let test_site = Arc::new(test_site);
        let snapshot_site = Arc::clone(&test_site);
        let mut out =
            Self::blocking(move || run005(&plan, &|| snapshot_site.request_counts())).await?;

        // wiederholte Requests am Server der abgebrochenen Läufe (robots.txt gehört zu jedem Start), je Abbruch
        let counts = test_site.request_counts();
        let robots = counts.get("/robots.txt").copied().unwrap_or(0);
        let phases = repeated_per_phase(&out.phase_counts);
        let repeated: u64 = phases.iter().sum();
        // Phase 0 liegt vor dem ersten Abbruch; Wiederholungen darin hat kein Abbruch verursacht
        let repeated_before_first_kill = phases.first().copied().unwrap_or(0);
        let per_kill_list: Vec<u64> = phases.iter().skip(1).copied().collect();
        let kills = out.kills_done as u64;
        let per_kill = u64::from(self.threads) + REPEAT_BUFFER_ALLOWANCE;
        let bound = kills * per_kill;
        out.violations.extend(repeat_violations(
            &per_kill_list,
            repeated_before_first_kill,
            per_kill,
            self.threads,
        ));
        let stats = RepeatStats::of(&per_kill_list);
        out.extra.insert("threads".into(), json!(self.threads));
        out.extra.insert("seed".into(), json!(self.seed));
        out.extra
            .insert("repeated_requests".into(), json!(repeated));
        out.extra
            .insert("repeated_requests_bound".into(), json!(bound));
        out.extra
            .insert("repeated_bound_per_kill".into(), json!(per_kill));
        // je Abbruch einzeln, in Abbruchreihenfolge (Abbruch 1 = erste Fortsetzung)
        out.extra
            .insert("repeated_per_kill".into(), json!(per_kill_list));
        out.extra
            .insert("repeated_per_kill_mean".into(), json!(round(stats.mean, 1)));
        out.extra
            .insert("repeated_per_kill_max".into(), json!(stats.max));
        out.extra.insert(
            "repeated_before_first_kill".into(),
            json!(repeated_before_first_kill),
        );
        out.extra.insert("robots_fetches".into(), json!(robots));
        out.extra
            .insert("requests_total".into(), json!(test_site.total_requests()));
        out.extra.insert(
            "requests_reference".into(),
            json!(ref_site.total_requests()),
        );
        Ok(CrawlOutcome {
            urls_crawled: out.crawled,
            duration: Duration::from_secs_f64(out.seconds),
            peak_rss_bytes: None,
            extra: out.extra,
            violations: out.violations,
            warnings: out.warnings,
        })
    }
}

impl CrawlerUnderTest for EngineCrawler {
    fn name(&self) -> &str {
        "os-engine"
    }

    fn crawl(&self, job: CrawlJob) -> CrawlFuture<'_> {
        Box::pin(async move {
            match job.scenario.as_str() {
                "perf-001" => self.perf001(job).await,
                "perf-003" => self.perf003(job).await,
                "perf-005" => self.perf005(job).await,
                _ => Err(CrawlerError::NotImplemented),
            }
        })
    }
}

/// Gilt die RSS-Grenze von PERF-001 (1,5 GB, Verhältnis) im Profil? Nein im Profil „Maximale Geschwindigkeit“
/// (K-16): der Cache beträgt dort bis zu 25 % des RAM (höchstens 4 GiB); die Grenze des Caches wird weiter geprüft.
pub fn rss_limits_apply(profile: StorageProfile) -> bool {
    profile != StorageProfile::MaxSpeed
}

/// PERF-001: Verletzungen der RSS-Grenzen (1,5 GB und Verhältnis zum Referenzlauf); im Profil
/// „Maximale Geschwindigkeit“ keine ([`rss_limits_apply`]).
pub fn rss_limit_violations(
    profile: StorageProfile,
    pages: u64,
    rss: Option<u64>,
    scaling: &ScalingVerdict,
) -> Vec<String> {
    let mut out = Vec::new();
    if !rss_limits_apply(profile) {
        return out;
    }
    match rss {
        Some(r) if r >= MAX_RSS_BYTES => out.push(format!(
            "PERF-001: Spitzen-RSS {} MB >= 1,5 GB bei {pages} Seiten",
            r / 1_000_000
        )),
        None => out.push("PERF-001: Spitzen-RSS nicht messbar".to_string()),
        _ => {}
    }
    if let Err(msg) = &scaling.verdict {
        out.push(msg.clone());
    }
    out
}

/// PERF-003 (K-73): Verletzung, wenn der Crawl länger als [`MAX_STALL_SECS`] ohne steigende Zahl abgerufener URLs
/// stand. `who` ist die Kennung im Text (PERF-003, auch für den Crawl von PERF-001).
pub fn stall_violation(who: &str, max_stall_s: f64) -> Option<String> {
    (max_stall_s > MAX_STALL_SECS).then(|| {
        format!("{who}: der Crawl stand {max_stall_s:.1} s still (Grenze {MAX_STALL_SECS} s)")
    })
}

// ---- PERF-005: Ablauf im blockierenden Thread ------------------------------------------------------------------

struct Plan005 {
    example: PathBuf,
    dir: PathBuf,
    ref_addr: String,
    test_addr: String,
    threads: u32,
    pages: u64,
    kill_points: Vec<u64>,
    seed: u64,
    stall: Duration,
}

struct Out005 {
    crawled: u64,
    seconds: f64,
    kills_done: usize,
    violations: Vec<String>,
    warnings: Vec<String>,
    extra: BTreeMap<String, Value>,
    /// Requests je Pfad (ohne `/robots.txt`), kumuliert am Ende jeder Phase: Phase 0 bis zum ersten Abbruch, Phase i
    /// von der Fortsetzung nach Abbruch i bis zum nächsten Abbruch bzw. zum Ende.
    phase_counts: Vec<BTreeMap<String, u64>>,
}

/// Request-Zähler je Pfad der Testsite (kumuliert seit Start).
type Counts = BTreeMap<String, u64>;

/// Wiederholte Requests je Phase aus den kumulierten Zählern am Ende jeder Phase ([`Out005::phase_counts`]): ein
/// Request ist eine Wiederholung, wenn derselbe Pfad schon in einer früheren Phase oder früher in derselben Phase
/// abgerufen wurde. Die Summe über alle Phasen ist `Summe(Anzahl - 1)` über alle Pfade, wie die bisherige Gesamtzahl.
/// Phase i >= 1 gehört zu Abbruch i.
pub fn repeated_per_phase(cumulative: &[BTreeMap<String, u64>]) -> Vec<u64> {
    let empty = Counts::new();
    cumulative
        .iter()
        .enumerate()
        .map(|(i, now)| {
            let before = if i == 0 { &empty } else { &cumulative[i - 1] };
            now.iter()
                .filter(|(path, _)| path.as_str() != "/robots.txt")
                .map(|(path, &n)| {
                    let seen = before.get(path).copied().unwrap_or(0);
                    let fresh = n.saturating_sub(seen);
                    // war der Pfad noch nie abgerufen, ist der erste Abruf keine Wiederholung
                    if seen == 0 {
                        fresh.saturating_sub(1)
                    } else {
                        fresh
                    }
                })
                .sum()
        })
        .collect()
}

/// Mittel und Maximum der Wiederholungen je Abbruch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RepeatStats {
    /// Mittelwert (0 ohne Abbrüche).
    pub mean: f64,
    /// Größter Wert (0 ohne Abbrüche).
    pub max: u64,
}

impl RepeatStats {
    /// Aus der Liste je Abbruch.
    pub fn of(per_kill: &[u64]) -> Self {
        Self {
            mean: if per_kill.is_empty() {
                0.0
            } else {
                per_kill.iter().sum::<u64>() as f64 / per_kill.len() as f64
            },
            max: per_kill.iter().copied().max().unwrap_or(0),
        }
    }
}

/// PERF-005/K-49: Verletzungen der Grenze für wiederholte Requests, **je Abbruch** geprüft (jeder Wert <= `bound`),
/// nicht nur die Summe. Wiederholungen vor dem ersten Abbruch (`before_first_kill`) darf es nicht geben, sie zählen
/// gegen keinen Abbruch, sind aber in der Gesamtzahl des Berichts enthalten und werden hier gemeldet, wenn sie allein
/// die Grenze eines Abbruchs überschreiten.
pub fn repeat_violations(
    per_kill: &[u64],
    before_first_kill: u64,
    bound: u64,
    threads: u32,
) -> Vec<String> {
    let mut out = Vec::new();
    for (i, &n) in per_kill.iter().enumerate() {
        if n > bound {
            out.push(format!(
                "PERF-005: {n} wiederholte Requests nach Abbruch {} > {bound} (Threads {threads} + Kanal und größter Batch {REPEAT_BUFFER_ALLOWANCE}, K-49): die Engine puffert unbegrenzt viel",
                i + 1
            ));
        }
    }
    if before_first_kill > bound {
        out.push(format!(
            "PERF-005: {before_first_kill} wiederholte Requests vor dem ersten Abbruch > {bound} (K-49)"
        ));
    }
    out
}

/// SplitMix64: kleiner, gesäter Zufallsgenerator (Regel 2.8: keine Zufallszahlen ohne Seed).
pub struct SplitMix64(pub u64);

impl SplitMix64 {
    /// Nächster Wert.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Gleichverteilt in `[lo, hi)`.
    pub fn range(&mut self, lo: u64, hi: u64) -> u64 {
        if hi <= lo {
            return lo;
        }
        lo + self.next_u64() % (hi - lo)
    }
}

/// Abbruchpunkte als Stand von `crawled`: der Bereich 8 % … 85 % der Seiten wird in `kills` gleiche Abschnitte
/// geteilt, in jedem liegt ein gesäter Zufallspunkt (aufsteigend, mit Abstand).
pub fn kill_points(seed: u64, kills: usize, pages: u64) -> Vec<u64> {
    let mut rng = SplitMix64(seed);
    let (lo, hi) = (pages * 8 / 100, pages * 85 / 100);
    let span = (hi - lo) / kills.max(1) as u64;
    (0..kills as u64)
        .map(|i| rng.range(lo + i * span, lo + (i + 1) * span).max(1))
        .collect()
}

fn child_cmd(plan: &Plan005, file: &Path, addr: &str, resume: bool, threads: u32) -> Command {
    let mut cmd = Command::new(&plan.example);
    cmd.arg("--file")
        .arg(file)
        .args(["--start", &format!("http://{HOST}/")])
        .args(["--resolve", &format!("*.test={addr}")])
        .args(["--threads", &threads.to_string()]);
    if resume {
        cmd.arg("--resume");
    }
    cmd
}

/// Ein Zeilenkopf der Tabelle `urls`: Zustand und Statuscode je URL.
type UrlRows = BTreeMap<String, (String, i64)>;

fn url_rows(file: &CrawlFile) -> Result<UrlRows, String> {
    let mut stmt = file
        .conn()
        .prepare("SELECT url, state, COALESCE(status_code, -1) FROM urls")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                (r.get::<_, String>(1)?, r.get::<_, i64>(2)?),
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<UrlRows, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

fn scalar(file: &CrawlFile, sql: &str) -> Result<i64, String> {
    file.conn()
        .query_row(sql, [], |r| r.get(0))
        .map_err(|e| format!("{sql}: {e}"))
}

/// Setzt den Heartbeat der Datei 60 s zurück: das Fortsetzen läuft „eine Minute später“ (die Engine erkennt einen
/// Absturz erst, wenn der Heartbeat älter als 10 s ist; so muss der Lasttest nicht in Echtzeit warten).
fn age_heartbeat(path: &Path) -> Result<(), String> {
    let file = CrawlFile::open(path, OpenMode::ReadWrite).map_err(|e| e.to_string())?;
    let heartbeat = file
        .get_meta("heartbeat_at")
        .map_err(|e| e.to_string())?
        .ok_or("kein Heartbeat in der Datei")?;
    let at = Timestamp::from_db(&heartbeat).map_err(|e| e.to_string())?;
    let older = Timestamp::from_unix_millis(at.unix_millis() - 60_000);
    file.set_meta("heartbeat_at", &older.to_db())
        .map_err(|e| e.to_string())
}

fn run005(plan: &Plan005, snapshot: &dyn Fn() -> Counts) -> Result<Out005, String> {
    let mut violations = Vec::new();
    let warnings = Vec::new();
    let mut extra: BTreeMap<String, Value> = BTreeMap::new();

    // Referenz: ununterbrochener Crawl
    let ref_path = plan.dir.join("ref.ospider");
    let reference = Proc::spawn(
        child_cmd(plan, &ref_path, &plan.ref_addr, false, plan.threads),
        plan.stall,
    )?
    .finish()?;
    if reference.state != "Completed" {
        return Err(format!("Referenzcrawl endete mit {}", reference.state));
    }
    let ref_rows =
        url_rows(&CrawlFile::open(&ref_path, OpenMode::ReadOnly).map_err(|e| e.to_string())?)?;

    // Crawl mit harten Abbrüchen
    let path = plan.dir.join("crawl.ospider");
    let mut rng = SplitMix64(plan.seed ^ 0xA5A5_5A5A);
    let mut kills_done = 0usize;
    let mut kill_log = Vec::new();
    let started = std::time::Instant::now();
    let mut finished: Option<RunResult> = None;
    let mut phase_counts: Vec<Counts> = Vec::new();
    for (i, &point) in plan.kill_points.iter().enumerate() {
        if i > 0 {
            age_heartbeat(&path)?;
        }
        let mut child = Proc::spawn(
            child_cmd(plan, &path, &plan.test_addr, i > 0, plan.threads),
            plan.stall,
        )?;
        match child.wait_until_crawled(point)? {
            Some(at) => {
                // gesäte Verzögerung: der Abbruch trifft verschiedene Stellen des Batch-Zyklus
                std::thread::sleep(Duration::from_millis(rng.range(0, 40)));
                let last_reported = child.kill().map_err(|e| format!("Abbruch: {e}"))?;
                // Requests, die der Server beim Abbruch schon angenommen hatte, zählen noch zur beendeten Phase
                std::thread::sleep(Duration::from_millis(50));
                phase_counts.push(snapshot());
                kills_done += 1;
                let file = CrawlFile::open(&path, OpenMode::ReadOnly).map_err(|e| e.to_string())?;
                let ok = file.integrity_check().map_err(|e| e.to_string())?;
                let dup = scalar(&file, "SELECT COUNT(*) - COUNT(DISTINCT url) FROM urls")?;
                let done = scalar(&file, "SELECT COUNT(*) FROM urls WHERE state = 'done'")?;
                if !ok {
                    violations.push(format!(
                        "PERF-005: integrity_check nach Abbruch {} nicht ok",
                        i + 1
                    ));
                }
                if dup != 0 {
                    violations.push(format!(
                        "PERF-005: {dup} doppelte URLs nach Abbruch {}",
                        i + 1
                    ));
                }
                kill_log.push(json!({
                    "kill": i + 1,
                    "target_crawled": point,
                    "reported_crawled_at_target": at,
                    "last_reported_crawled": last_reported,
                    "done_in_file": done,
                    "lost_in_pipeline": last_reported.map(|n| i64::try_from(n).unwrap_or(i64::MAX) - done),
                    "integrity_ok": ok,
                    "duplicate_urls": dup,
                }));
            }
            None => {
                // der Crawl war vor dem Abbruchpunkt fertig (zu schneller Lauf): kein Abbruch möglich
                finished = Some(child.finish()?);
                break;
            }
        }
    }
    let result = match finished {
        Some(r) => r,
        None => {
            if kills_done > 0 {
                age_heartbeat(&path)?;
            }
            Proc::spawn(
                child_cmd(plan, &path, &plan.test_addr, kills_done > 0, plan.threads),
                plan.stall,
            )?
            .finish()?
        }
    };
    let seconds = started.elapsed().as_secs_f64();
    phase_counts.push(snapshot());
    if kills_done == 0 {
        violations.push(
            "PERF-005: es gab keinen harten Abbruch (Crawl zu schnell für den Abbruchpunkt)"
                .to_string(),
        );
    }
    if result.state != "Completed" {
        violations.push(format!(
            "PERF-005: fortgesetzter Crawl endete mit {}",
            result.state
        ));
    }

    // Endergebnis gegen den Referenzcrawl
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).map_err(|e| e.to_string())?;
    if !file.integrity_check().map_err(|e| e.to_string())? {
        violations.push("PERF-005: integrity_check am Ende nicht ok".to_string());
    }
    let dup = scalar(&file, "SELECT COUNT(*) - COUNT(DISTINCT url) FROM urls")?;
    if dup != 0 {
        violations.push(format!("PERF-005: {dup} doppelte URLs am Ende"));
    }
    let queued = scalar(&file, "SELECT COUNT(*) FROM queue")?;
    if queued != 0 {
        violations.push(format!(
            "PERF-005: {queued} Einträge bleiben in der Warteschlange"
        ));
    }
    let rows = url_rows(&file)?;
    let missing = ref_rows.keys().filter(|u| !rows.contains_key(*u)).count();
    let extra_urls = rows.keys().filter(|u| !ref_rows.contains_key(*u)).count();
    let differing: Vec<&String> = ref_rows
        .iter()
        .filter(|(u, v)| rows.get(*u).is_some_and(|w| w != *v))
        .map(|(u, _)| u)
        .collect();
    if missing > 0 || extra_urls > 0 || !differing.is_empty() {
        violations.push(format!(
            "PERF-005: URL-Menge/Status weichen vom Referenzcrawl ab ({missing} fehlen, {extra_urls} zusätzlich, {} mit anderem Status/Zustand; z. B. {:?})",
            differing.len(),
            differing.iter().take(3).collect::<Vec<_>>()
        ));
    }
    let mut statuses: BTreeMap<String, u64> = BTreeMap::new();
    for (_, status) in rows.values() {
        *statuses.entry(status.to_string()).or_default() += 1;
    }
    extra.insert("reference_urls".into(), json!(ref_rows.len()));
    extra.insert("urls".into(), json!(rows.len()));
    extra.insert("status_counts".into(), json!(statuses));
    extra.insert("urls_missing".into(), json!(missing));
    extra.insert("urls_extra".into(), json!(extra_urls));
    extra.insert("urls_with_other_status".into(), json!(differing.len()));
    extra.insert("duplicate_urls".into(), json!(dup));
    extra.insert(
        "status_identical".into(),
        json!(missing == 0 && extra_urls == 0 && differing.is_empty()),
    );
    extra.insert("kills_requested".into(), json!(plan.kill_points.len()));
    extra.insert("kills_done".into(), json!(kills_done));
    extra.insert("kill_points".into(), json!(plan.kill_points));
    extra.insert("kills".into(), json!(kill_log));
    extra.insert("pages".into(), json!(plan.pages));
    Ok(Out005 {
        crawled: result.crawled,
        seconds,
        kills_done,
        violations,
        warnings,
        extra,
        phase_counts,
    })
}

// ---- PERF-001: Skalierung des RSS --------------------------------------------------------------------------

/// Eingaben für [`judge_rss_scaling`].
#[derive(Debug, Clone, Copy)]
pub struct RssScaling {
    /// Seiten des Hauptlaufs.
    pub pages: u64,
    /// Seiten des Referenzlaufs (ein Zehntel).
    pub reference_pages: u64,
    /// Spitzen-RSS des Hauptlaufs.
    pub rss: u64,
    /// Spitzen-RSS des Referenzlaufs.
    pub reference_rss: u64,
    /// Größe der Crawl-Datei des Hauptlaufs.
    pub file_bytes: u64,
    /// Größe der Crawl-Datei des Referenzlaufs.
    pub reference_file_bytes: u64,
    /// SQLite-Cache des Profils in Bytes.
    pub cache_bytes: u64,
}

/// Ergebnis von [`judge_rss_scaling`].
#[derive(Debug, Clone, PartialEq)]
pub struct ScalingVerdict {
    /// `rss / reference_rss`.
    pub ratio: Option<f64>,
    /// Welche Regel gilt: `ratio` oder `cache-bounded-growth`.
    pub rule: &'static str,
    /// `Err` mit der Meldung, wenn das Kriterium verletzt ist.
    pub verdict: Result<(), String>,
}

/// Slack der Wachstumsregel: Arbeitsstrukturen der Engine neben dem SQLite-Cache (MiB).
const GROWTH_SLACK_BYTES: u64 = 64 * 1024 * 1024;

/// PERF-001: „RSS bei 100 000 und 1 000 000 URLs unterscheidet sich um weniger als Faktor 2.“
///
/// Das Verhältnis gilt, sobald der Referenzlauf den SQLite-Cache des Profils schon gefüllt hat (Crawl-Datei des
/// Referenzlaufs mindestens so groß wie der Cache; bei 100 000 Seiten mit 30 Links und dem Profil „Ausgewogen“
/// der Fall). Bei den kleinen Varianten (10 000 gegen 1 000 Seiten) wächst der RSS dagegen mit dem Cache, der die
/// kleine Datei komplett aufnimmt – das Verhältnis wäre kein Maß für unbegrenztes Wachstum. Dort gilt:
/// der RSS-Zuwachs darf höchstens 125 % des Dateizuwachses plus 64 MiB betragen (`cache-bounded-growth`; der
/// Seiten-Cache von SQLite hält Seiten samt Verwaltungsdaten, im Profil „Maximale Geschwindigkeit“ mit bis zu 4 GiB
/// die ganze Datei).
pub fn judge_rss_scaling(s: RssScaling) -> ScalingVerdict {
    if s.rss == 0 || s.reference_rss == 0 {
        return ScalingVerdict {
            ratio: None,
            rule: "none",
            verdict: Err("PERF-001: RSS-Verhältnis nicht berechenbar (kein RSS gemessen)".into()),
        };
    }
    let ratio = s.rss as f64 / s.reference_rss as f64;
    if s.reference_file_bytes >= s.cache_bytes {
        let verdict = if ratio < MAX_RSS_RATIO {
            Ok(())
        } else {
            Err(format!(
                "PERF-001: RSS({}) / RSS({}) = {ratio:.2} >= 2",
                s.pages, s.reference_pages
            ))
        };
        return ScalingVerdict {
            ratio: Some(ratio),
            rule: "ratio",
            verdict,
        };
    }
    let growth = s.rss.saturating_sub(s.reference_rss);
    let file_growth = s.file_bytes.saturating_sub(s.reference_file_bytes);
    let allowed = file_growth + file_growth / 4 + GROWTH_SLACK_BYTES;
    let verdict = if growth <= allowed {
        Ok(())
    } else {
        Err(format!(
            "PERF-001: RSS wächst von {} auf {} MB (+{} MB), mehr als 125 % des Dateizuwachses + 64 MiB ({} MB) bei {} gegen {} Seiten",
            s.reference_rss / 1_000_000,
            s.rss / 1_000_000,
            growth / 1_000_000,
            allowed / 1_000_000,
            s.pages,
            s.reference_pages
        ))
    };
    ScalingVerdict {
        ratio: Some(ratio),
        rule: "cache-bounded-growth",
        verdict,
    }
}

// ---- Cache-Grenze (K-16) ------------------------------------------------------------------------------------

/// Gemessene PRAGMA-Werte des Profils „Maximale Geschwindigkeit“.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheSetting {
    /// `PRAGMA cache_size` (negativ = KiB).
    pub cache_size_kib: i64,
    /// Physischer RAM dieses Rechners.
    pub ram_bytes: Option<u64>,
}

/// Legt eine leere Crawl-Datei mit dem Profil `profile` an (Hilfe für Prüfungen und Tests).
pub fn create_crawl_file(path: &Path, profile: StorageProfile) -> Result<CrawlFile, String> {
    let clock = SystemClock;
    let init = CrawlInit {
        crawl_id: RandomIds.new_crawl_id(clock.now()),
        mode: CrawlMode::Spider,
        start_url: Some("http://site.test/".to_string()),
        list_source: None,
        project: "loadtest".to_string(),
        name: "loadtest".to_string(),
        config: CrawlConfig::default(),
        created_at: clock.now(),
    };
    CrawlFile::create(path, &init, profile).map_err(|e| e.to_string())
}

/// Legt eine Crawl-Datei mit dem Profil „Maximale Geschwindigkeit“ an und liest `cache_size` zurück.
pub fn max_speed_cache(dir: &Path) -> Result<CacheSetting, String> {
    let path = dir.join("cache-check.ospider");
    let file = create_crawl_file(&path, StorageProfile::MaxSpeed)?;
    let applied = profile::read(file.conn()).map_err(|e| e.to_string())?;
    drop(file);
    let _ = std::fs::remove_file(&path);
    Ok(CacheSetting {
        cache_size_kib: applied.cache_size,
        ram_bytes: profile::physical_ram_bytes(),
    })
}

/// PERF-001/K-16: Der Cache des Profils „Maximale Geschwindigkeit“ ist höchstens 25 % des physischen RAM und
/// höchstens 4 GiB groß (Untergrenze: der Wert von „Ausgewogen“, 256 MiB, auf sehr kleinen Rechnern).
pub fn check_cache_limit(cache_size_kib: i64, ram_bytes: Option<u64>) -> Result<(), String> {
    if cache_size_kib >= 0 {
        return Err(format!(
            "PERF-001: cache_size {cache_size_kib} ist positiv (Seiten statt KiB), die Grenze lässt sich nicht prüfen"
        ));
    }
    let bytes = cache_size_kib.unsigned_abs().saturating_mul(1024);
    if bytes > CACHE_CAP_BYTES {
        return Err(format!(
            "PERF-001: Cache {} MiB > 4 GiB (K-16)",
            bytes / (1024 * 1024)
        ));
    }
    const FLOOR_BYTES: u64 = 256 * 1024 * 1024;
    if let Some(ram) = ram_bytes {
        let limit = (ram / 4).max(FLOOR_BYTES);
        if bytes > limit {
            return Err(format!(
                "PERF-001: Cache {} MiB > 25 % des RAM ({} MiB)",
                bytes / (1024 * 1024),
                limit / (1024 * 1024)
            ));
        }
    }
    Ok(())
}

fn round(v: f64, digits: i32) -> f64 {
    let f = 10f64.powi(digits);
    (v * f).round() / f
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIB: u64 = 1024 * 1024 * 1024;

    #[test]
    fn kill_points_are_seeded_ascending_and_inside_the_range() {
        let a = kill_points(7, 4, 10_000);
        assert_eq!(
            a,
            kill_points(7, 4, 10_000),
            "gleicher Seed, gleiche Punkte"
        );
        assert_ne!(a, kill_points(8, 4, 10_000));
        assert_eq!(a.len(), 4);
        assert!(a.windows(2).all(|w| w[0] < w[1]), "{a:?}");
        assert!(a[0] >= 800 && *a.last().unwrap() < 8_500, "{a:?}");
        assert!(kill_points(1, 0, 1_000).is_empty());
    }

    fn scaling(rss_mb: u64, ref_rss_mb: u64, file_mb: u64, ref_file_mb: u64) -> ScalingVerdict {
        const MB: u64 = 1_000_000;
        judge_rss_scaling(RssScaling {
            pages: 1_000_000,
            reference_pages: 100_000,
            rss: rss_mb * MB,
            reference_rss: ref_rss_mb * MB,
            file_bytes: file_mb * MB,
            reference_file_bytes: ref_file_mb * MB,
            cache_bytes: 268_435_456,
        })
    }

    // spec: PERF-001
    #[test]
    fn rss_scaling_uses_the_ratio_once_the_cache_is_full() {
        // 100 000 Seiten: Datei 600 MB > Cache 256 MiB
        let ok = scaling(450, 300, 6_000, 600);
        assert_eq!(ok.rule, "ratio");
        assert!(ok.verdict.is_ok(), "{ok:?}");
        let bad = scaling(700, 300, 6_000, 600);
        assert!(bad.verdict.unwrap_err().contains(">= 2"));
    }

    // spec: PERF-001
    #[test]
    fn rss_scaling_of_small_runs_may_grow_with_the_cache_but_not_beyond_the_data() {
        // gemessen: 29 MB -> 96 MB bei Dateien von 6 und 59 MB
        let ok = scaling(96, 29, 59, 6);
        assert_eq!(ok.rule, "cache-bounded-growth");
        assert!(ok.ratio.unwrap() > 3.0);
        assert!(ok.verdict.is_ok(), "{ok:?}");
        // Wachstum um 400 MB bei 53 MB mehr Daten: Leck
        let bad = scaling(430, 30, 59, 6);
        assert!(bad.verdict.is_err());
        // gemessen im Profil „Maximale Geschwindigkeit“: 63 -> 416 MB bei 5 und 302 MB Dateigröße
        assert!(scaling(416, 63, 302, 30).verdict.is_ok());
        assert!(scaling(0, 30, 59, 6).verdict.is_err());
    }

    fn counts(pairs: &[(&str, u64)]) -> BTreeMap<String, u64> {
        pairs.iter().map(|(p, n)| (p.to_string(), *n)).collect()
    }

    // spec: PERF-005
    #[test]
    fn repeated_requests_are_counted_per_kill_in_the_order_of_the_kills() {
        // Phase 0: /a, /b, /c je einmal. Abbruch 1. Phase 1: /b und /c erneut, /d neu. Abbruch 2. Phase 2: /c zum
        // dritten Mal, /d erneut, /e neu. robots.txt gehört zu jedem Start und zählt nie.
        let cumulative = vec![
            counts(&[("/a", 1), ("/b", 1), ("/c", 1), ("/robots.txt", 1)]),
            counts(&[
                ("/a", 1),
                ("/b", 2),
                ("/c", 2),
                ("/d", 1),
                ("/robots.txt", 2),
            ]),
            counts(&[
                ("/a", 1),
                ("/b", 2),
                ("/c", 3),
                ("/d", 2),
                ("/e", 1),
                ("/robots.txt", 3),
            ]),
        ];
        let phases = repeated_per_phase(&cumulative);
        assert_eq!(phases, vec![0, 2, 2]);
        // die Summe entspricht der bisherigen Gesamtzahl Summe(Anzahl - 1) ohne robots.txt
        let last = cumulative.last().unwrap();
        let total: u64 = last
            .iter()
            .filter(|(p, _)| p.as_str() != "/robots.txt")
            .map(|(_, n)| n - 1)
            .sum();
        assert_eq!(phases.iter().sum::<u64>(), total);
        // doppelter Abruf innerhalb einer Phase ist ebenfalls eine Wiederholung
        let dup = vec![counts(&[("/a", 3)]), counts(&[("/a", 3), ("/b", 1)])];
        assert_eq!(repeated_per_phase(&dup), vec![2, 0]);
        assert!(repeated_per_phase(&[]).is_empty());
    }

    // spec: PERF-005
    #[test]
    fn repeat_stats_report_mean_and_maximum_and_the_check_is_per_kill() {
        let stats = RepeatStats::of(&[10, 40, 25]);
        assert_eq!(stats.max, 40);
        assert!((stats.mean - 25.0).abs() < 1e-9);
        assert_eq!(RepeatStats::of(&[]), RepeatStats { mean: 0.0, max: 0 });
        // Summe 75 liegt unter 3 x 30, aber ein einzelner Abbruch (40) überschreitet die Grenze
        let v = repeat_violations(&[10, 40, 25], 0, 30, 8);
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(
            v[0].contains("Abbruch 2") && v[0].contains("PERF-005"),
            "{v:?}"
        );
        assert!(repeat_violations(&[30, 30], 0, 30, 8).is_empty());
        assert_eq!(repeat_violations(&[1], 31, 30, 8).len(), 1);
    }

    // spec: PERF-001
    #[test]
    fn max_speed_does_not_judge_the_rss_limits_but_the_other_profiles_do() {
        let bad_ratio = ScalingVerdict {
            ratio: Some(3.0),
            rule: "ratio",
            verdict: Err("PERF-001: RSS(1000000) / RSS(100000) = 3.00 >= 2".into()),
        };
        let huge = Some(2_000_000_000);
        assert!(
            rss_limit_violations(StorageProfile::MaxSpeed, 1_000_000, huge, &bad_ratio).is_empty()
        );
        assert!(!rss_limits_apply(StorageProfile::MaxSpeed));
        for p in [StorageProfile::Balanced, StorageProfile::MinMemory] {
            assert!(rss_limits_apply(p));
            let v = rss_limit_violations(p, 1_000_000, huge, &bad_ratio);
            assert_eq!(v.len(), 2, "{v:?}");
            assert!(v[0].contains("1,5 GB") && v[1].contains(">= 2"));
        }
        // ohne Messwert ist nur ein bewertetes Profil verletzt
        let ok = ScalingVerdict {
            ratio: Some(1.0),
            rule: "ratio",
            verdict: Ok(()),
        };
        assert_eq!(
            rss_limit_violations(StorageProfile::Balanced, 10, None, &ok).len(),
            1
        );
        assert!(rss_limit_violations(StorageProfile::MaxSpeed, 10, None, &ok).is_empty());
    }

    // spec: PERF-003
    #[test]
    fn the_stall_limit_is_the_spec_value_and_applies_with_the_given_label() {
        assert_eq!(MAX_STALL_SECS, 30.0);
        assert!(stall_violation("PERF-003", 30.0).is_none());
        let v = stall_violation("PERF-003 (Crawl aus PERF-001)", 31.3).unwrap();
        assert!(
            v.contains("31.3 s") && v.contains("PERF-001") && v.contains("30"),
            "{v}"
        );
    }

    #[test]
    fn splitmix_is_deterministic() {
        let mut a = SplitMix64(1);
        let mut b = SplitMix64(1);
        assert_eq!(a.next_u64(), b.next_u64());
        assert_eq!(SplitMix64(5).range(10, 10), 10);
        for _ in 0..100 {
            assert!((3..7).contains(&a.range(3, 7)));
        }
    }

    // spec: PERF-001
    #[test]
    fn cache_limit_accepts_the_k16_formula_and_rejects_larger_caches() {
        let kib = |ram| StorageProfile::MaxSpeed.cache_size_kib_for_ram(Some(ram));
        for ram in [GIB / 2, 4 * GIB, 8 * GIB, 16 * GIB, 64 * GIB] {
            assert_eq!(check_cache_limit(kib(ram), Some(ram)), Ok(()), "{ram}");
        }
        // 8 GiB RAM: 25 % = 2 GiB; 3 GiB wäre zu viel
        assert!(check_cache_limit(-3 * 1024 * 1024, Some(8 * GIB)).is_err());
        // 64 GiB RAM: höchstens 4 GiB
        assert!(check_cache_limit(-5 * 1024 * 1024, Some(64 * GIB)).is_err());
        assert!(check_cache_limit(-4 * 1024 * 1024, Some(64 * GIB)).is_ok());
        assert!(
            check_cache_limit(2000, Some(8 * GIB)).is_err(),
            "positiv = Seiten"
        );
        // ohne bekannten RAM zählt nur die 4-GiB-Grenze
        assert!(check_cache_limit(-262_144, None).is_ok());
    }

    // spec: PERF-001
    #[test]
    fn this_machines_max_speed_profile_respects_the_cache_limit() {
        let dir = tempfile::tempdir().unwrap();
        let c = max_speed_cache(dir.path()).unwrap();
        assert_eq!(check_cache_limit(c.cache_size_kib, c.ram_bytes), Ok(()));
        assert!(c.ram_bytes.is_some());
    }
}
