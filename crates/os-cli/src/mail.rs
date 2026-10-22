//! Global completion delivery for ordinary CLI crawls (OUT-281, additive K2-61).
use crate::{exec::Outcome, reporter::Reporter};
use std::path::Path;
pub async fn notify_global(dir: &Path, outcome: &Outcome, rep: &Reporter) {
    if outcome.exit == crate::exit::Exit::Interrupted {
        return;
    }
    let Ok(settings) = os_model::settings::AppSettings::load(dir) else {
        return;
    };
    let policy = os_sched::mail::Policy {
        on_success: settings.accounts.notify_on_success,
        on_failure: settings.accounts.notify_on_failure,
        account: settings.accounts.notify_account,
        to: settings.accounts.notify_to,
    };
    let failed = outcome.exit != crate::exit::Exit::Ok;
    if !policy.enabled(failed) {
        return;
    }
    let completion = outcome
        .mail_completion
        .clone()
        .or_else(|| {
            outcome
                .crawl_path
                .as_ref()
                .and_then(|p| os_analyze::mail_completion(p, None, false, false).ok())
        })
        .and_then(|v| serde_json::from_value::<os_sched::mail::Completion>(v).ok());
    let mut completion = completion.unwrap_or_default();
    if failed {
        completion.error = Some(outcome.summary.clone());
    }
    if let Err(e) = os_sched::mail::notify(
        dir,
        os_fetch::secret_store_from_env().as_ref(),
        &settings.network,
        &policy,
        &completion,
    )
    .await
    {
        rep.warn(format!("Abschlussmail fehlgeschlagen: {e}"));
    }
}

/// Preserve summary facts before a temporary crawl file is removed; disabled mail does no analysis work.
pub fn capture_before_discard(dir: &Path, path: &Path) -> Option<serde_json::Value> {
    let settings = os_model::settings::AppSettings::load(dir).ok()?;
    let a = &settings.accounts;
    if a.notify_account.is_none()
        || a.notify_to.iter().all(|s| s.trim().is_empty())
        || !(a.notify_on_success || a.notify_on_failure)
    {
        return None;
    }
    os_analyze::mail_completion(path, None, false, false).ok()
}
