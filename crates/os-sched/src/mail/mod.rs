//! Accounts, SMTP delivery and completion notifications (OUT-280 … OUT-282).
mod accounts;
mod content;
mod transport;
pub use accounts::{Account, AccountStore, Auth, Tls};
pub use content::{Change, Completion, Issue};
pub use transport::{send, send_with_context, DeliveryContext};

use os_fetch::secrets::SecretStoreMut;
use os_model::settings::{AppSettings, NetworkSettings};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct MailError {
    pub message: String,
    pub smtp_code: Option<String>,
}
impl MailError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            smtp_code: None,
        }
    }
}
impl From<std::io::Error> for MailError {
    fn from(_: std::io::Error) -> Self {
        Self::new("Mail-Verbindung oder Kontodatei nicht verfügbar")
    }
}

/// Global notification policy. Empty recipients always suppress delivery.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Policy {
    pub on_success: bool,
    pub on_failure: bool,
    pub account: Option<String>,
    pub to: Vec<String>,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            on_success: false,
            on_failure: true,
            account: None,
            to: vec![],
        }
    }
}
impl Policy {
    pub fn enabled(&self, failed: bool) -> bool {
        self.account.is_some()
            && self.to.iter().any(|s| !s.trim().is_empty())
            && if failed {
                self.on_failure
            } else {
                self.on_success
            }
    }
}

pub async fn notify(
    dir: &Path,
    secrets: &dyn SecretStoreMut,
    network: &NetworkSettings,
    policy: &Policy,
    completion: &Completion,
) -> Result<bool, MailError> {
    notify_with_context(
        dir,
        secrets,
        network,
        policy,
        completion,
        &DeliveryContext::default(),
    )
    .await
}

pub async fn notify_with_context(
    dir: &Path,
    secrets: &dyn SecretStoreMut,
    network: &NetworkSettings,
    policy: &Policy,
    completion: &Completion,
    context: &DeliveryContext<'_>,
) -> Result<bool, MailError> {
    if !policy.enabled(completion.error.is_some()) {
        return Ok(false);
    }
    let account = AccountStore::new(dir).get(policy.account.as_deref().unwrap())?;
    send_with_context(&account, secrets, network, &policy.to, completion, context).await?;
    Ok(true)
}

/// Scheduled runs use the same delivery path as UI runs, including failures before crawling.
pub async fn notify_task(
    dir: &Path,
    _crawls: &Path,
    secrets: &dyn SecretStoreMut,
    task: &crate::tasks::Task,
    record: &crate::history::RunRecord,
    completion: Completion,
) -> Result<bool, MailError> {
    notify_task_with_context(
        dir,
        _crawls,
        secrets,
        task,
        record,
        completion,
        &DeliveryContext::default(),
    )
    .await
}

pub async fn notify_task_with_context(
    dir: &Path,
    _crawls: &Path,
    secrets: &dyn SecretStoreMut,
    task: &crate::tasks::Task,
    record: &crate::history::RunRecord,
    mut completion: Completion,
    context: &DeliveryContext<'_>,
) -> Result<bool, MailError> {
    use crate::history::RunResult;
    if matches!(record.result, RunResult::Skipped | RunResult::Running) {
        return Ok(false);
    }
    let settings =
        AppSettings::load(dir).map_err(|_| MailError::new("Einstellungen nicht lesbar"))?;
    completion.task = Some(task.name.clone());
    if completion.start_url.is_empty() {
        completion.start_url = match &task.crawl.source {
            crate::tasks::model::Source::Spider { url } => url.clone(),
            crate::tasks::model::Source::List { path } => path.clone(),
        };
    }
    completion.project = task.project_name().into();
    completion.started = record.started_at.clone();
    completion.ended = record.ended_at.clone().unwrap_or_default();
    completion.duration_ms = duration(&completion.started, &completion.ended);
    if matches!(record.result, RunResult::Failed | RunResult::Aborted) {
        completion.error = Some(
            record
                .message
                .clone()
                .unwrap_or_else(|| record.result.label().into()),
        );
    }
    let policy = Policy {
        on_success: task.notify.on_success,
        on_failure: task.notify.on_failure,
        account: task.notify.account.clone(),
        to: task.notify.to.clone(),
    };
    // Per-task recipients override the global policy, including an explicitly empty list.
    notify_with_context(
        dir,
        secrets,
        &settings.network,
        &policy,
        &completion,
        context,
    )
    .await
}
pub fn duration(start: &str, end: &str) -> i64 {
    match (
        os_model::Timestamp::from_db(start),
        os_model::Timestamp::from_db(end),
    ) {
        (Ok(a), Ok(b)) => (b.unix_millis() - a.unix_millis()).max(0),
        _ => 0,
    }
}

/// Failure notifications default to enabled (OUT-281).
pub fn default_on_failure() -> bool {
    true
}

/// Versandadapter für C02-Abschlussmails.
pub mod completion;
