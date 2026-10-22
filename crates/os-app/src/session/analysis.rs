//! Analysis operations use the writer queue and wait for integration completion.
use super::SessionService;
use crate::{
    dto::*,
    ops::ProgressSink,
    pool::CrawlKey,
    runtime::{
        error::{internal, to_json},
        sessions::SessionInfo,
    },
    App, CmdError,
};
use os_analyze::analysis::{persist, runner, AnalysisOutcome};
use os_model::{AnalysisStep, CancelToken, StepState};

fn step_state(s: StepState) -> StepStateDto {
    match s {
        StepState::Pending => StepStateDto::Pending,
        StepState::Done => StepStateDto::Done,
        StepState::Skipped => StepStateDto::Skipped,
        StepState::NotApplicable => StepStateDto::NotApplicable,
    }
}
pub async fn status(app: &App, s: &SessionInfo) -> Result<AnalysisStatus, CmdError> {
    if let Some(st) = app
        .ext::<SessionService>()
        .analysis
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(&s.key)
        .filter(|s| s.running)
        .cloned()
    {
        return Ok(st);
    }
    let cfg = s.config.clone();
    app.read_pool()
        .run(s.key.clone(), move |f, _| {
            let data = os_analyze::aggregates::status::analysis_status(f, &cfg);
            let snapshot = os_analyze::state::AnalysisSnapshot::load(f)
                .map_err(|e| os_store::StoreError::InvalidInput(e.to_string()))?;
            let cfg_json = serde_json::to_value(cfg.as_ref())
                .map_err(|e| os_store::StoreError::InvalidInput(e.to_string()))?;
            Ok(AnalysisStatus {
                steps: data
                    .steps
                    .into_iter()
                    .map(|(step, state)| AnalysisStepStatus {
                        step: step.as_str().into(),
                        state: step_state(state),
                        stale: snapshot.freshness(step, &cfg_json)
                            == os_analyze::state::Freshness::Stale,
                    })
                    .collect(),
                running: f.get_meta("analysis_state")?.as_deref() == Some("running"),
                current: None,
                percent: None,
            })
        })
        .await
}
fn publish(app: &App, key: &CrawlKey, status: AnalysisStatus) {
    let svc = app.ext::<SessionService>();
    svc.analysis
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(key.clone(), status.clone());
    svc.parts.lock().unwrap_or_else(|p| p.into_inner()).analysis = Some(status);
}

pub fn start_operation(
    app: &App,
    s: SessionInfo,
    steps: Option<Vec<String>>,
    automatic: bool,
) -> Result<OpHandle, CmdError> {
    super::writable(&s)?;
    let only = steps
        .map(|steps| {
            steps
                .into_iter()
                .map(|s| {
                    s.parse::<AnalysisStep>().map_err(|_| {
                        CmdError::invalid_input("err-analysis-step", "steps").with_arg("step", s)
                    })
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;
    let initial = AnalysisStatus {
        steps: Vec::new(),
        running: true,
        current: None,
        percent: Some(0),
    };
    {
        let svc = app.ext::<SessionService>();
        let mut map = svc.analysis.lock().unwrap_or_else(|p| p.into_inner());
        if map.get(&s.key).is_some_and(|s| s.running) {
            return Err(super::busy());
        }
        map.insert(s.key.clone(), initial.clone());
    }
    publish(app, &s.key, initial);
    let app = app.clone();
    let ops = app.ops().clone();
    Ok(ops.spawn("op-analysis", true, move |cancel, progress| {
        Box::pin(finish_operation(app, s, only, automatic, cancel, progress))
    }))
}

async fn finish_operation(
    app: App,
    s: SessionInfo,
    only: Option<Vec<AnalysisStep>>,
    automatic: bool,
    cancel: CancelToken,
    progress: ProgressSink,
) -> Result<serde_json::Value, CmdError> {
    let result = execute(&app, &s, only, automatic, cancel.clone(), progress).await;
    let final_status = match &result {
        Ok(outcome) => AnalysisStatus {
            steps: outcome
                .steps
                .iter()
                .map(|(s, st)| AnalysisStepStatus {
                    step: s.as_str().into(),
                    state: step_state(*st),
                    stale: false,
                })
                .collect(),
            running: false,
            current: None,
            percent: Some(if outcome.complete { 100 } else { 0 }),
        },
        Err(_) => AnalysisStatus {
            steps: Vec::new(),
            running: false,
            current: None,
            percent: None,
        },
    };
    publish(&app, &s.key, final_status.clone());
    if automatic {
        crate::commands::mail::after_analysis(&app, &s.key);
    }
    super::feed::invalidate(&app, &s.key);
    if cancel.is_cancelled() {
        return Err(CmdError::new(ErrorCode::Cancelled, "err-cancelled"));
    }
    result?;
    to_json(&final_status)
}

async fn execute(
    app: &App,
    s: &SessionInfo,
    only: Option<Vec<AnalysisStep>>,
    automatic: bool,
    cancel: CancelToken,
    progress: ProgressSink,
) -> Result<AnalysisOutcome, CmdError> {
    if automatic {
        tokio::select! {
            _ = app.crawl_events().wait_barriers(&s.key) => {},
            _ = wait_cancel(&cancel) => return Err(CmdError::new(ErrorCode::Cancelled, "err-cancelled")),
        }
    }
    let svc = app.ext::<SessionService>();
    let _guard = svc.mutation.lock().await;
    if cancel.is_cancelled() {
        return Err(CmdError::new(ErrorCode::Cancelled, "err-cancelled"));
    }
    super::control::ensure_idle(app, s).await?;
    let cfg = app
        .read_pool()
        .run(s.key.clone(), |f, _| f.config())
        .await?;
    let a = app.clone();
    let key = s.key.clone();
    let clock = app.config().clock.clone();
    app.write_queue()
        .submit_file(&s.key, move |f| {
            let options = runner::RunOptions {
                clock: clock.as_ref(),
                only: only.as_deref(),
                ..Default::default()
            };
            let total = only.as_ref().map_or(AnalysisStep::ALL.len(), Vec::len) as u64;
            let outcome = runner::run_with(
                f,
                &cfg,
                &cancel,
                &|step, pct| {
                    let mut st = a
                        .ext::<SessionService>()
                        .analysis
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .get(&key)
                        .cloned()
                        .expect("registered analysis");
                    st.current = Some(step.as_str().into());
                    st.percent = Some((pct * 100.0) as u8);
                    let entry = AnalysisStepStatus {
                        step: step.as_str().into(),
                        state: if pct < 1.0 {
                            StepStateDto::Running
                        } else {
                            StepStateDto::Done
                        },
                        stale: false,
                    };
                    if let Some(old) = st.steps.iter_mut().find(|s| s.step == entry.step) {
                        *old = entry;
                    } else {
                        st.steps.push(entry);
                    }
                    progress.report(
                        st.steps
                            .iter()
                            .filter(|s| s.state == StepStateDto::Done)
                            .count() as u64,
                        Some(total),
                    );
                    publish(&a, &key, st);
                },
                &options,
            )
            .map_err(|e| internal("analysis", e.to_string()))?;
            // Same incomplete-crawl rule as M1's run_manual, also for selected steps.
            if f.meta()
                .map_err(|e| crate::runtime::error::store_error(&e))?
                .state
                != os_model::CrawlState::Completed
            {
                persist::write_run_state(f, persist::RunState::Incomplete, None)
                    .map_err(|e| internal("analysis", e.to_string()))?;
            }
            Ok(outcome)
        })
        .await
}
async fn wait_cancel(cancel: &CancelToken) {
    while !cancel.is_cancelled() {
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
}
