//! Filterzustände und Analyse-Runner (M1-P14): ONPAGE-007, TECH-005, ONPAGE-190 (Zustand statt Zahl).

#[path = "framework_common.rs"]
mod common;

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use common::*;
use os_analyze::analysis::persist::{self, RunState};
use os_analyze::analysis::runner::{run_only, run_with, RunOptions, StepFn};
use os_analyze::analysis::{self, CancelToken};
use os_analyze::state::{step_config_hash, AnalysisSnapshot, Freshness};
use os_analyze::views::{filter_counts_in, query_view_in, state_of_in, ViewQuery};
use os_analyze::AnalyzeError;
use os_model::time::ManualClock;
use os_model::{AnalysisStep, CrawlConfig, FilterState, RenderingMode, StepState};
use os_store::{CrawlFile, OpenMode};

fn crawl(dir: &std::path::Path, name: &str) -> (std::path::PathBuf, CrawlFile) {
    let (path, file) = create(dir, name, 5);
    insert(
        &file,
        &[
            row(1, "a.html").int("canonical_count", 1),
            row(2, "b.html").int("canonical_count", 0),
        ],
    );
    (path, file)
}

fn done(_: &CrawlFile, _: &CrawlConfig, _: &CancelToken) -> Result<StepState, AnalyzeError> {
    Ok(StepState::Done)
}

fn na(_: &CrawlFile, _: &CrawlConfig, _: &CancelToken) -> Result<StepState, AnalyzeError> {
    Ok(StepState::NotApplicable)
}

fn canonicals_and_sitemaps_done(step: AnalysisStep) -> StepFn {
    match step {
        AnalysisStep::Canonicals | AnalysisStep::Sitemaps => done,
        _ => na,
    }
}

fn state(file: &CrawlFile, cfg: &CrawlConfig, filter: &str) -> FilterState {
    state_of_in(&[test_tab()], file, cfg, "Test", filter).unwrap()
}

fn run_done(file: &CrawlFile, cfg: &CrawlConfig) -> analysis::AnalysisOutcome {
    let opts = RunOptions {
        registry: canonicals_and_sitemaps_done,
        ..RunOptions::default()
    };
    run_with(file, cfg, &CancelToken::new(), &|_, _| {}, &opts).unwrap()
}

fn page(file: &CrawlFile, cfg: &CrawlConfig, filter: &str) -> os_analyze::views::ViewPage {
    let mut q = ViewQuery::new("Test", filter);
    q.limit = 100;
    query_view_in(&[test_tab()], file, cfg, &q).unwrap()
}

// spec: ONPAGE-007, TECH-005
#[test]
fn post_crawl_filter_shows_a_state_until_its_step_has_run() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = crawl(dir.path(), "a.ospider");
    let cfg = CrawlConfig::default();

    // Nach Crawl-Ende ohne Analyse: Zustand statt Zahl, keine Zeilen.
    assert_eq!(
        state(&file, &cfg, "Needs Analysis"),
        FilterState::NeedsCrawlAnalysis
    );
    let counts = filter_counts_in(&[test_tab()], &file, &cfg).unwrap();
    let c = counts
        .iter()
        .find(|c| c.filter == "Needs Analysis")
        .unwrap();
    assert_eq!(
        (c.count, c.pct),
        (None, None),
        "nie 0 an Stelle des Zustands"
    );
    assert_eq!(c.state, FilterState::NeedsCrawlAnalysis);
    let p = page(&file, &cfg, "Needs Analysis");
    assert_eq!((p.rows.len(), p.total), (0, 0));

    // Live-Filter derselben Ansicht sind bereit.
    assert_eq!(state(&file, &cfg, "All"), FilterState::Ready);

    // Nach dem Teilschritt: die Zahl.
    let outcome = run_done(&file, &cfg);
    assert!(outcome.complete);
    assert_eq!(state(&file, &cfg, "Needs Analysis"), FilterState::Ready);
    let counts = filter_counts_in(&[test_tab()], &file, &cfg).unwrap();
    let c = counts
        .iter()
        .find(|c| c.filter == "Needs Analysis")
        .unwrap();
    assert_eq!(c.count, Some(1));
    assert_eq!(page(&file, &cfg, "Needs Analysis").total, 1);
}

// spec: ONPAGE-007
#[test]
fn missing_config_switch_and_rendering_are_reported_as_states() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = crawl(dir.path(), "a.ospider");
    let mut cfg = CrawlConfig::default();
    assert!(!cfg.duplicates.near_enabled);
    assert_eq!(
        state(&file, &cfg, "Needs Config"),
        FilterState::NeedsConfig("duplicates.near_enabled".into())
    );
    assert_eq!(
        state(&file, &cfg, "Needs Rendering"),
        FilterState::NeedsRendering
    );
    let p = page(&file, &cfg, "Needs Rendering");
    assert_eq!(p.total, 0);

    cfg.duplicates.near_enabled = true;
    cfg.rendering.mode = RenderingMode::JavaScript;
    // Mit eingeschalteter Funktion bleibt der Nachlauf-Filter „Crawl-Analyse erforderlich“.
    assert_eq!(
        state(&file, &cfg, "Needs Config"),
        FilterState::NeedsCrawlAnalysis
    );
    assert_eq!(state(&file, &cfg, "Needs Rendering"), FilterState::Ready);
    assert_eq!(page(&file, &cfg, "Needs Rendering").total, 2);
}

// spec: ONPAGE-007, TECH-005
#[test]
fn config_change_or_mark_stale_turns_the_state_to_stale() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = crawl(dir.path(), "a.ospider");
    let mut cfg = CrawlConfig::default();
    run_done(&file, &cfg);
    assert_eq!(state(&file, &cfg, "Needs Analysis"), FilterState::Ready);

    // Änderung, die den Teilschritt betrifft (Abschnitt `spider`) → veraltet, ohne HTTP-Anfragen.
    cfg.spider.follow_internal_nofollow = true;
    assert_eq!(state(&file, &cfg, "Needs Analysis"), FilterState::Stale);
    let c = filter_counts_in(&[test_tab()], &file, &cfg).unwrap();
    let c = c.iter().find(|c| c.filter == "Needs Analysis").unwrap();
    assert_eq!((c.state.clone(), c.count), (FilterState::Stale, None));
    // Erneute Analyse mit der neuen Konfiguration: wieder bereit.
    run_done(&file, &cfg);
    assert_eq!(state(&file, &cfg, "Needs Analysis"), FilterState::Ready);

    // Explizite Markierung (Neu crawlen / Entfernen): veraltet, Analysezustand `stale`.
    assert_eq!(persist::read_run_state(&file).unwrap(), RunState::Completed);
    persist::mark_stale(&file, &[AnalysisStep::Canonicals]).unwrap();
    assert_eq!(state(&file, &cfg, "Needs Analysis"), FilterState::Stale);
    assert_eq!(persist::read_run_state(&file).unwrap(), RunState::Stale);
    // Ein anderer Schritt bleibt davon unberührt.
    assert!(!persist::read_stale(&file)
        .unwrap()
        .contains(AnalysisStep::Sitemaps));
    run_done(&file, &cfg);
    assert_eq!(state(&file, &cfg, "Needs Analysis"), FilterState::Ready);
    assert!(persist::read_stale(&file).unwrap().is_empty());
}

// spec: TECH-005, CRAWL-141
#[test]
fn sitemaps_step_turns_stale_with_changes_in_spider_and_sitemaps_only() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = crawl(dir.path(), "a.ospider");
    let mut cfg = CrawlConfig::default();
    run_done(&file, &cfg);

    let freshness = |cfg: &CrawlConfig| {
        let json = serde_json::to_value(cfg).unwrap();
        AnalysisSnapshot::load(&file)
            .unwrap()
            .freshness(AnalysisStep::Sitemaps, &json)
    };
    assert_eq!(freshness(&cfg), Freshness::Fresh);

    // Ein fremder Abschnitt (Geschwindigkeit) lässt den Schritt unberührt.
    let mut other = cfg.clone();
    other.speed.threads += 1;
    assert_eq!(freshness(&other), Freshness::Fresh);

    // `spider.*` (Linkspeicherung, hreflang, Canonical) beeinflusst die Orphans → veraltet, ohne HTTP-Anfragen.
    let mut spider = cfg.clone();
    spider.spider.follow_internal_nofollow = !spider.spider.follow_internal_nofollow;
    assert_eq!(freshness(&spider), Freshness::Stale);
    let mut canonicals = cfg.clone();
    canonicals.spider.canonicals.store = !canonicals.spider.canonicals.store;
    assert_eq!(freshness(&canonicals), Freshness::Stale);

    // Weiterhin: der eigene Abschnitt `sitemaps`.
    cfg.sitemaps.crawl_linked = !cfg.sitemaps.crawl_linked;
    assert_eq!(freshness(&cfg), Freshness::Stale);

    // Der Konfigurationshash unterscheidet sich entsprechend.
    let base = CrawlConfig::default();
    assert_ne!(
        step_config_hash(AnalysisStep::Sitemaps, &base),
        step_config_hash(AnalysisStep::Sitemaps, &spider)
    );
    assert_eq!(
        step_config_hash(AnalysisStep::Sitemaps, &base),
        step_config_hash(AnalysisStep::Sitemaps, &other)
    );
}

// spec: TECH-005
#[test]
fn deselected_step_keeps_its_filters_pending() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = crawl(dir.path(), "a.ospider");
    let mut cfg = CrawlConfig::default();
    cfg.analysis.canonicals = false;
    let outcome = run_done(&file, &cfg);
    let canon = outcome
        .steps
        .iter()
        .find(|(s, _)| *s == AnalysisStep::Canonicals);
    assert_eq!(canon, Some(&(AnalysisStep::Canonicals, StepState::Skipped)));
    assert_eq!(
        state(&file, &cfg, "Needs Analysis"),
        FilterState::NeedsCrawlAnalysis
    );
    assert_eq!(page(&file, &cfg, "Needs Analysis").total, 0);
}

// spec: TECH-005, ONPAGE-007
#[test]
fn cancelled_step_never_shows_partial_or_old_results() {
    fn cancelling(
        _: &CrawlFile,
        _: &CrawlConfig,
        cancel: &CancelToken,
    ) -> Result<StepState, AnalyzeError> {
        cancel.cancel();
        Ok(StepState::Pending)
    }
    fn registry(step: AnalysisStep) -> StepFn {
        match step {
            AnalysisStep::Canonicals => cancelling,
            _ => na,
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = crawl(dir.path(), "a.ospider");
    let cfg = CrawlConfig::default();
    run_done(&file, &cfg);
    assert_eq!(state(&file, &cfg, "Needs Analysis"), FilterState::Ready);

    // Erneuter, abgebrochener Lauf: das alte Ergebnis gilt nicht weiter, der Schritt steht aus.
    let opts = RunOptions {
        registry,
        max_parallel: 1,
        ..RunOptions::default()
    };
    let outcome = run_with(&file, &cfg, &CancelToken::new(), &|_, _| {}, &opts).unwrap();
    assert!(!outcome.complete);
    assert_eq!(
        state(&file, &cfg, "Needs Analysis"),
        FilterState::NeedsCrawlAnalysis
    );
    assert_eq!(
        persist::read_run_state(&file).unwrap(),
        RunState::Incomplete
    );
}

// spec: TECH-005
#[test]
fn default_run_skips_deselected_and_marks_steps_without_data_basis() {
    let dir = tempfile::tempdir().unwrap();
    let (path, file) = crawl(dir.path(), "a.ospider");
    let mut cfg = CrawlConfig::default();
    cfg.analysis.links = false;
    let clock = ManualClock::new(created_at());
    let opts = RunOptions {
        clock: &clock,
        ..RunOptions::default()
    };
    let outcome = run_with(&file, &cfg, &CancelToken::new(), &|_, _| {}, &opts).unwrap();
    assert!(outcome.complete);
    let states: std::collections::BTreeMap<_, _> = outcome.steps.iter().cloned().collect();
    assert_eq!(
        states[&AnalysisStep::Links],
        StepState::Skipped,
        "Schalter aus"
    );
    assert_eq!(
        states[&AnalysisStep::Images],
        StepState::NotApplicable,
        "ohne JS-Rendering"
    );
    assert_eq!(
        states[&AnalysisStep::Analytics],
        StepState::NotApplicable,
        "ohne Anbindung"
    );
    assert_eq!(
        states[&AnalysisStep::SearchConsole],
        StepState::NotApplicable
    );
    assert_eq!(states[&AnalysisStep::LinkMetrics], StepState::NotApplicable);
    // Die Schritte „Kanonisierungen“ und „Sitemaps“ sind umgesetzt und enden fertig (`Done`); Sitemaps auch
    // ohne Sitemap (ohne Einträge keine Marken). Alle übrigen Schritte ohne Datenbasis sind in M1 nicht anwendbar.
    assert_eq!(states[&AnalysisStep::Canonicals], StepState::Done);
    assert_eq!(states[&AnalysisStep::Sitemaps], StepState::Done);
    assert_eq!(states[&AnalysisStep::LinkScore], StepState::NotApplicable);
    // Reihenfolge der Ausgabe = AnalysisStep::ALL, unabhängig von der Abschlussreihenfolge.
    let order: Vec<_> = outcome.steps.iter().map(|(s, _)| *s).collect();
    assert_eq!(order, AnalysisStep::ALL);

    // Ergebnisse stehen in der Datei: erneutes Öffnen zeigt sie ohne Neuberechnung.
    drop(file);
    let reopened = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    let rows = persist::read_steps(&reopened).unwrap();
    assert_eq!(rows.len(), AnalysisStep::ALL.len());
    assert_eq!(rows[&AnalysisStep::Links].state, StepState::Skipped);
    assert_eq!(
        rows[&AnalysisStep::Links].finished_at.as_deref(),
        Some("2026-10-13T07:35:00.000Z")
    );
    assert!(rows[&AnalysisStep::Links].config_hash.is_some());
    assert_eq!(
        persist::read_run_state(&reopened).unwrap(),
        RunState::Completed
    );
    assert_eq!(
        reopened.get_meta("analysis_at").unwrap().as_deref(),
        Some("2026-10-13T07:35:00.000Z")
    );

    // Images mit JS-Rendering ist anwendbar (Schritt läuft), ohne Umsetzung in M1 aber nicht anwendbar.
    cfg.rendering.mode = RenderingMode::JavaScript;
    assert_eq!(
        os_analyze::analysis::runner::predetermined_state(AnalysisStep::Images, &cfg),
        None
    );
}

// spec: TECH-005
#[test]
fn independent_steps_run_in_parallel() {
    static ARRIVED: AtomicUsize = AtomicUsize::new(0);
    fn rendezvous(
        _: &CrawlFile,
        _: &CrawlConfig,
        _: &CancelToken,
    ) -> Result<StepState, AnalyzeError> {
        ARRIVED.fetch_add(1, Ordering::SeqCst);
        let start = Instant::now();
        while ARRIVED.load(Ordering::SeqCst) < 2 {
            if start.elapsed() > Duration::from_secs(10) {
                return Err(AnalyzeError::NotImplemented(
                    "Teilschritte liefen nicht parallel",
                ));
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        Ok(StepState::Done)
    }
    fn registry(step: AnalysisStep) -> StepFn {
        match step {
            AnalysisStep::Canonicals | AnalysisStep::Sitemaps => rendezvous,
            _ => na,
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = crawl(dir.path(), "a.ospider");
    let opts = RunOptions {
        registry,
        max_parallel: 2,
        ..RunOptions::default()
    };
    let outcome = run_with(
        &file,
        &CrawlConfig::default(),
        &CancelToken::new(),
        &|_, _| {},
        &opts,
    )
    .unwrap();
    assert!(outcome.complete);
    let states: std::collections::BTreeMap<_, _> = outcome.steps.iter().cloned().collect();
    assert_eq!(states[&AnalysisStep::Canonicals], StepState::Done);
    assert_eq!(states[&AnalysisStep::Sitemaps], StepState::Done);
}

// spec: TECH-005
#[test]
fn cancel_keeps_finished_steps_and_leaves_the_rest_pending() {
    static SITEMAPS_RAN: AtomicBool = AtomicBool::new(false);
    fn finishing_and_cancelling(
        _: &CrawlFile,
        _: &CrawlConfig,
        cancel: &CancelToken,
    ) -> Result<StepState, AnalyzeError> {
        cancel.cancel();
        Ok(StepState::Done)
    }
    fn sitemaps(
        _: &CrawlFile,
        _: &CrawlConfig,
        _: &CancelToken,
    ) -> Result<StepState, AnalyzeError> {
        SITEMAPS_RAN.store(true, Ordering::SeqCst);
        Ok(StepState::Done)
    }
    fn registry(step: AnalysisStep) -> StepFn {
        match step {
            AnalysisStep::Canonicals => finishing_and_cancelling,
            AnalysisStep::Sitemaps => sitemaps,
            _ => na,
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = crawl(dir.path(), "a.ospider");
    let cfg = CrawlConfig::default();
    let opts = RunOptions {
        registry,
        max_parallel: 1,
        ..RunOptions::default()
    };
    let outcome = run_with(&file, &cfg, &CancelToken::new(), &|_, _| {}, &opts).unwrap();
    assert!(!outcome.complete);
    let states: std::collections::BTreeMap<_, _> = outcome.steps.iter().cloned().collect();
    assert_eq!(
        states[&AnalysisStep::Canonicals],
        StepState::Done,
        "fertiger Schritt bleibt gültig"
    );
    assert_eq!(
        states[&AnalysisStep::Sitemaps],
        StepState::Pending,
        "bleibt ausstehend"
    );
    assert!(!SITEMAPS_RAN.load(Ordering::SeqCst));
    // In der Datei genauso; der fertige Schritt liefert Zahlen, der ausstehende nicht.
    let rows = persist::read_steps(&file).unwrap();
    assert_eq!(rows[&AnalysisStep::Canonicals].state, StepState::Done);
    assert_eq!(rows[&AnalysisStep::Sitemaps].state, StepState::Pending);
    assert_eq!(state(&file, &cfg, "Needs Analysis"), FilterState::Ready);
    assert_eq!(
        persist::read_run_state(&file).unwrap(),
        RunState::Incomplete
    );
}

// spec: TECH-005
#[test]
fn failing_step_is_reported_but_finished_steps_are_kept() {
    fn failing(_: &CrawlFile, _: &CrawlConfig, _: &CancelToken) -> Result<StepState, AnalyzeError> {
        Err(AnalyzeError::NotImplemented("kaputt"))
    }
    fn registry(step: AnalysisStep) -> StepFn {
        match step {
            AnalysisStep::Canonicals => done,
            AnalysisStep::Sitemaps => failing,
            _ => na,
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = crawl(dir.path(), "a.ospider");
    let opts = RunOptions {
        registry,
        ..RunOptions::default()
    };
    let err = run_with(
        &file,
        &CrawlConfig::default(),
        &CancelToken::new(),
        &|_, _| {},
        &opts,
    );
    assert!(matches!(err, Err(AnalyzeError::NotImplemented("kaputt"))));
    let rows = persist::read_steps(&file).unwrap();
    assert_eq!(rows[&AnalysisStep::Canonicals].state, StepState::Done);
    assert_eq!(rows[&AnalysisStep::Sitemaps].state, StepState::Pending);
    assert_eq!(
        persist::read_run_state(&file).unwrap(),
        RunState::Incomplete
    );
}

// spec: TECH-005
#[test]
fn progress_is_reported_per_step_and_run_only_limits_the_scope() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = crawl(dir.path(), "a.ospider");
    let cfg = CrawlConfig::default();
    let seen: RefCell<Vec<(AnalysisStep, f32)>> = RefCell::new(Vec::new());
    let outcome = run_only(
        &file,
        &cfg,
        &CancelToken::new(),
        &|s, p| seen.borrow_mut().push((s, p)),
        &[AnalysisStep::Sitemaps, AnalysisStep::Analytics],
    )
    .unwrap();
    let steps: Vec<_> = outcome.steps.iter().map(|(s, _)| *s).collect();
    assert_eq!(steps, vec![AnalysisStep::Sitemaps, AnalysisStep::Analytics]);
    let seen = seen.into_inner();
    for step in [AnalysisStep::Sitemaps, AnalysisStep::Analytics] {
        assert!(seen.contains(&(step, 1.0)), "{step:?} meldet 1.0");
    }
    assert!(
        seen.contains(&(AnalysisStep::Sitemaps, 0.0)),
        "gestartete Schritte melden 0.0"
    );
    // Ein Teillauf entscheidet nicht alle Schritte → Analyse insgesamt unvollständig.
    assert_eq!(
        persist::read_run_state(&file).unwrap(),
        RunState::Incomplete
    );
}

// spec: TECH-005
#[test]
fn already_cancelled_token_runs_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = crawl(dir.path(), "a.ospider");
    let cancel = CancelToken::new();
    cancel.cancel();
    let outcome = analysis::run(&file, &CrawlConfig::default(), &cancel, &|_, _| {}).unwrap();
    assert!(!outcome.complete);
    let rows = persist::read_steps(&file).unwrap();
    assert_eq!(rows[&AnalysisStep::Canonicals].state, StepState::Pending);
}

// Infrastruktur (M2-F02, K2-19): `NearDuplicates` ist ein Teilschritt der Crawl-Analyse mit eigenem Schalter und
// eigenem Konfigurations-Hash; als Stub ist er angemeldet und ohne Umsetzung `NotApplicable`.
#[test]
fn near_duplicates_is_a_registered_step_with_its_own_switch_and_config_hash() {
    assert!(AnalysisStep::ALL.contains(&AnalysisStep::NearDuplicates));
    assert_eq!(AnalysisStep::NearDuplicates.as_str(), "near_duplicates");
    assert_eq!(
        "near_duplicates".parse::<AnalysisStep>().unwrap(),
        AnalysisStep::NearDuplicates
    );

    let dir = tempfile::tempdir().unwrap();
    let (_path, file) = crawl(dir.path(), "nd.ospider");
    let outcome = run_with(
        &file,
        &CrawlConfig::default(),
        &CancelToken::new(),
        &|_, _| {},
        &RunOptions::default(),
    )
    .unwrap();
    let states: std::collections::BTreeMap<_, _> = outcome.steps.iter().cloned().collect();
    assert_eq!(
        states[&AnalysisStep::NearDuplicates],
        StepState::NotApplicable
    );
    for step in [
        AnalysisStep::LinkScore,
        AnalysisStep::Pagination,
        AnalysisStep::Hreflang,
        AnalysisStep::Links,
    ] {
        assert_eq!(states[&step], StepState::NotApplicable, "{step:?}");
    }

    let mut off = CrawlConfig::default();
    off.analysis.near_duplicates = false;
    let outcome = run_with(
        &file,
        &off,
        &CancelToken::new(),
        &|_, _| {},
        &RunOptions::default(),
    )
    .unwrap();
    let states: std::collections::BTreeMap<_, _> = outcome.steps.iter().cloned().collect();
    assert_eq!(states[&AnalysisStep::NearDuplicates], StepState::Skipped);

    // Der Hash hängt an `duplicates.*` und `content.*`, nicht an fremden Abschnitten.
    let base = CrawlConfig::default();
    let mut threshold = base.clone();
    threshold.duplicates.near_threshold_pct = 80;
    let mut unrelated = base.clone();
    unrelated.speed.threads = 9;
    assert_ne!(
        step_config_hash(AnalysisStep::NearDuplicates, &base),
        step_config_hash(AnalysisStep::NearDuplicates, &threshold)
    );
    assert_eq!(
        step_config_hash(AnalysisStep::NearDuplicates, &base),
        step_config_hash(AnalysisStep::NearDuplicates, &unrelated)
    );
}
