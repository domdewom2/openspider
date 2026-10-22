//! In os-cli::run_task eingebundener Mail-Hook; kein Abhängigkeitszyklus zu os-sched.
use super::{RunRecord, RunResult, Task, TaskHooks};
use os_sched::mail::completion::CompletionSender;
use std::{path::PathBuf, sync::Arc};

/// Abschlussmail-Anbindung für `run_in` / `run_with`; Account-Auflösung und Transport liefert T02.
pub struct ComparisonMailHooks {
    pub crawls: PathBuf,
    pub sender: Arc<dyn CompletionSender>,
}

impl TaskHooks for ComparisonMailHooks {
    fn notify(&self, task: &Task, record: &RunRecord) -> Result<(), String> {
        let send = match record.result {
            RunResult::Completed | RunResult::CompletedWithWarnings => task.notify.on_success,
            RunResult::Failed | RunResult::Aborted => task.notify.on_failure,
            _ => false,
        };
        if !send {
            return Ok(());
        }
        let id = record
            .crawl_id
            .as_deref()
            .ok_or("Completion mail: no crawl available")?;
        let crawl = os_store::registry::list_crawls(&self.crawls)
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|c| c.meta.crawl_id.to_string() == id)
            .ok_or("Completion mail: crawl not found")?;
        let file = os_store::CrawlFile::open(&crawl.path, os_store::OpenMode::ReadOnly)
            .map_err(|e| e.to_string())?;
        let content = os_analyze::compare::change::notification::content(
            &file,
            &task.name,
            task.execution.auto_compare,
            task.effective_exports().skip_empty,
        )
        .map_err(|e| e.to_string())?;
        self.sender.send(
            &task.notify.to,
            &format!("{}: {}", task.name, record.result.label()),
            &content.text,
            &content.html,
        )
    }
}
