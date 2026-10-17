//! Shared shutdown path for window close, menu and keyboard shortcuts (F-11).
pub use crate::dto::QuitDecision;
use crate::{
    dto::{ErrorCode, McpStatus, QuitReason},
    runtime::{error::parse_args, hooks::QuitHook},
    session::SessionService,
    App, CallCtx, CmdError,
};
use futures_util::future::BoxFuture;
use std::sync::atomic::Ordering;

pub struct SessionQuit;
impl QuitHook for SessionQuit {
    fn request_quit(&self, app: App) -> BoxFuture<'static, QuitDecision> {
        Box::pin(async move {
            let mut reasons = Vec::new();
            if app.sessions().all().iter().any(|s| {
                s.engine
                    .as_ref()
                    .is_some_and(|e| e.handle.lock().unwrap_or_else(|p| p.into_inner()).is_some())
            }) {
                reasons.push(QuitReason::Crawl);
            }
            let active = app.ops().active_statuses();
            if active.iter().any(|s| s.label_key.contains("analysis")) {
                reasons.push(QuitReason::Analysis);
            }
            if active.iter().any(|s| !s.label_key.contains("analysis")) {
                reasons.push(QuitReason::Export);
            }
            if mcp_connected(&app).await {
                reasons.push(QuitReason::McpSessions);
            }
            if reasons.is_empty() {
                QuitDecision::Quit
            } else {
                QuitDecision::Confirm { reasons }
            }
        })
    }
}
async fn mcp_connected(app: &App) -> bool {
    app.dispatch(
        CallCtx::for_window("main"),
        "mcp.status",
        serde_json::Value::Null,
    )
    .await
    .ok()
    .and_then(|v| parse_args::<McpStatus>(v).ok())
    .is_some_and(|s| matches!(s,McpStatus::Connected { clients,.. } if clients > 0))
}

pub async fn confirm(app: &App, accepted: bool) -> Result<(), CmdError> {
    if !accepted {
        return Ok(());
    }
    let service = app.ext::<SessionService>();
    if service.quitting.swap(true, Ordering::SeqCst) {
        return Err(crate::session::busy());
    }
    let result = async {
        let active = app.ops().active_statuses();
        app.ops().cancel_all();
        // Do not exit while an exporter/analysis is still writing. Fail safely if it
        // cannot cooperate; the user can retry. Request drain is handled below.
        for op in active {
            tokio::time::timeout(std::time::Duration::from_secs(3), app.ops().wait(&op.op_id))
                .await
                .map_err(|_| CmdError::new(ErrorCode::Busy, "err-quit-operation-running"))?;
        }
        let _guard = service.mutation.lock().await;
        for s in app.sessions().all() {
            if s.engine
                .as_ref()
                .is_some_and(|e| e.handle.lock().unwrap_or_else(|p| p.into_inner()).is_some())
            {
                crate::session::control::settle(app, &s, Some(false)).await?;
            }
        }
        if mcp_connected(app).await {
            app.dispatch(
                CallCtx::for_window("main"),
                "mcp.stop",
                serde_json::Value::Null,
            )
            .await?;
        }
        app.write_queue().flush().await;
        Ok(())
    }
    .await;
    if result.is_ok() {
        if let Some(host) = app.window_host() {
            host.exit();
        }
    }
    service.quitting.store(false, Ordering::SeqCst);
    result
}
