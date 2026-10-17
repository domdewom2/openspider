#[path = "aggregates_fixture.rs"]
mod fixture;
use os_analyze::aggregates::status::{analysis_status, on_config_changed, try_analysis_status};
use os_analyze::analysis::persist::{self, RunState};
use os_analyze::state::step_config_hash;
use os_model::{AnalysisStep, CrawlConfig, RenderingMode, StepState};

fn done(file: &os_store::CrawlFile, cfg: &CrawlConfig) {
    for &step in AnalysisStep::ALL {
        persist::write_step(
            file,
            step,
            StepState::Done,
            None,
            Some(&step_config_hash(step, cfg)),
        )
        .unwrap();
    }
    persist::write_run_state(file, RunState::Completed, None).unwrap();
}

// spec: UI-082 [M2]
#[test]
fn thresholds_invalidate_only_near_duplicates_and_segments_invalidate_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture::create(dir.path());
    fixture::depth_fixture(&file);
    let mut cfg = CrawlConfig::default();
    cfg.analysis.near_duplicates = true;
    done(&file, &cfg);
    let mut new = cfg.clone();
    new.duplicates.near_threshold_pct = 83;
    on_config_changed(&file, &cfg, &new).unwrap();
    let status = analysis_status(&file, &new);
    assert!(status.error.is_none());
    assert_eq!(status.stale, vec![AnalysisStep::NearDuplicates]);
    assert!(status.needs_recrawl.is_empty());
    assert_eq!(persist::read_run_state(&file).unwrap(), RunState::Stale);
    assert_eq!(
        persist::read_steps(&file).unwrap()[&AnalysisStep::Content].state,
        StepState::Done
    );
    persist::clear_stale(&file, AnalysisStep::NearDuplicates).unwrap();
    done(&file, &cfg);
    let mut json = serde_json::to_value(&cfg).unwrap();
    json["segments"] =
        serde_json::json!({"rules":[{"name":"New","color":"#abcdef","pattern":"/a/"}]});
    // Die konkrete Regelvalidierung gehört K14; jede Änderung im Abschnitt ist
    // unabhängig von sämtlichen schrittspezifischen Konfigurationsprojektionen.
    for &step in AnalysisStep::ALL {
        assert_eq!(
            step_config_hash(step, &cfg),
            os_analyze::state::step_config_hash_json(step, &json)
        );
    }
    on_config_changed(&file, &cfg, &cfg).unwrap();
    assert!(analysis_status(&file, &cfg).stale.is_empty());
}

// spec: ONPAGE-140 [M2]
#[test]
fn content_area_requires_recrawl_only_when_stored_html_is_missing() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture::create(dir.path());
    fixture::depth_fixture(&file);
    let mut cfg = CrawlConfig::default();
    cfg.analysis.near_duplicates = true;
    done(&file, &cfg);
    let mut new = cfg.clone();
    new.content.area.include_elements = vec!["main".into()];
    // Auch wenn config.set die Konfiguration zuerst speichert, bleibt die
    // Inhaltsbereichs-Provenienz des bisherigen Ergebnisses erhalten.
    file.set_config(&new).unwrap();
    on_config_changed(&file, &cfg, &new).unwrap();
    let status = analysis_status(&file, &new);
    assert_eq!(
        status.needs_recrawl,
        vec![AnalysisStep::Content, AnalysisStep::NearDuplicates]
    );
    assert_eq!(
        status.stale,
        vec![AnalysisStep::Content, AnalysisStep::NearDuplicates]
    );
    file.conn().execute("INSERT INTO page_html(url_id,body) SELECT id,CAST('<main>fixture</main>' AS BLOB) FROM urls WHERE status_code BETWEEN 200 AND 299",[]).unwrap();
    assert!(analysis_status(&file, &new).needs_recrawl.is_empty());
    // Eine einzige nicht gespeicherte HTML-Seite reicht aus; das 404 dagegen
    // gehört nicht zur Content-Grundmenge.
    file.conn()
        .execute("DELETE FROM page_html WHERE url_id=2", [])
        .unwrap();
    assert_eq!(analysis_status(&file, &new).needs_recrawl.len(), 2);
    done(&file, &new);
    persist::clear_stale(&file, AnalysisStep::Content).unwrap();
    persist::clear_stale(&file, AnalysisStep::NearDuplicates).unwrap();
    assert!(analysis_status(&file, &new).needs_recrawl.is_empty());
    assert!(analysis_status(&file, &new).stale.is_empty());
}

// spec: ONPAGE-140 [M2]
#[test]
fn rendering_uses_rendered_html_and_a_threshold_alone_never_requires_recrawl() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture::create(dir.path());
    fixture::depth_fixture(&file);
    let mut cfg = CrawlConfig::default();
    cfg.rendering.mode = RenderingMode::JavaScript;
    cfg.analysis.near_duplicates = true;
    file.set_config(&cfg).unwrap();
    done(&file, &cfg);
    let mut new = cfg.clone();
    new.content.area.exclude_elements.push("aside".into());
    file.conn()
        .execute(
            "INSERT INTO page_html(url_id,body) SELECT id,CAST('raw' AS BLOB) FROM urls",
            [],
        )
        .unwrap();
    assert_eq!(analysis_status(&file, &new).needs_recrawl.len(), 2);
    file.conn().execute("INSERT INTO render_pages(url_id,status,html) SELECT id,'ok',CAST('rendered' AS BLOB) FROM urls",[]).unwrap();
    assert!(analysis_status(&file, &new).needs_recrawl.is_empty());
    new = cfg.clone();
    new.duplicates.near_threshold_pct = 80;
    assert!(analysis_status(&file, &new).needs_recrawl.is_empty());
    assert_eq!(
        analysis_status(&file, &new).stale,
        vec![AnalysisStep::NearDuplicates]
    );
}

// spec: UI-082 [M2]
#[test]
fn explicit_data_staleness_preserves_results_and_disabled_steps_are_not_stale() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture::create(dir.path());
    let cfg = CrawlConfig::default();
    done(&file, &cfg);
    persist::mark_stale(&file, &[AnalysisStep::Content]).unwrap();
    let status = analysis_status(&file, &cfg);
    assert_eq!(status.stale, vec![AnalysisStep::Content]);
    assert_eq!(
        status
            .steps
            .iter()
            .find(|(s, _)| *s == AnalysisStep::Content)
            .unwrap()
            .1,
        StepState::Done
    );
    let mut cfg = cfg;
    cfg.analysis.content = false;
    assert!(!analysis_status(&file, &cfg)
        .stale
        .contains(&AnalysisStep::Content));
    file.conn()
        .execute("DROP TABLE analysis_steps", [])
        .unwrap();
    assert!(try_analysis_status(&file, &cfg).is_err());
    assert!(analysis_status(&file, &cfg).error.is_some());
}

// spec: ONPAGE-140 [M2]
#[test]
fn content_area_recrawl_hint_does_not_require_a_previous_postcrawl_analysis() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture::create(dir.path());
    fixture::depth_fixture(&file);
    let old = CrawlConfig::default();
    let mut new = old.clone();
    new.content.area.include_elements = vec!["main".into()];
    file.set_config(&new).unwrap();
    on_config_changed(&file, &old, &new).unwrap();
    let status = analysis_status(&file, &new);
    assert!(status.needs_recrawl.contains(&AnalysisStep::Content));
    assert_eq!(
        status
            .steps
            .iter()
            .find(|(s, _)| *s == AnalysisStep::Content)
            .unwrap()
            .1,
        StepState::Pending
    );
}
