//! Additive desktop adapters for UI-124 / OUT-230. No registry or existing command is changed.
use std::path::PathBuf;

use crate::{
    dto::{CmdError, CrawlConfigDto, ErrorCode, OpHandle},
    runtime::error::{internal, store_error, to_json},
    App,
};

/// Read-only integrity verification, including older versions without migration.
pub async fn check(path: String) -> Result<(), CmdError> {
    tokio::task::spawn_blocking(move || {
        os_store::files::verify(&PathBuf::from(path)).map_err(|e| store_error(&e))
    })
    .await
    .map_err(|e| internal("crawl integrity", e.to_string()))?
}

/// Read the embedded configuration without changing either the file or the active draft.
pub async fn configuration(app: &App, path: String) -> Result<CrawlConfigDto, CmdError> {
    // A separate short-lived slot keeps the active session and its cached connections untouched.
    struct Registration(crate::pool::ReadPool, crate::pool::CrawlKey);
    impl Drop for Registration {
        fn drop(&mut self) {
            self.0.unregister(&self.1);
        }
    }
    let key = crate::pool::CrawlKey(format!(
        "crawl-file-config-{}",
        app.config().ids.new_crawl_id(app.config().clock.now())
    ));
    let registration = Registration(app.read_pool().clone(), key.clone());
    registration.0.register(key.clone(), PathBuf::from(path));
    let config = registration.0.run(key, |file, _| file.config()).await?;
    super::config::dto(&config)
}

/// Best-effort recovery into a new file. SQLite recovery itself cannot be interrupted safely.
pub fn repair(app: &App, path: String) -> OpHandle {
    let app = app.clone();
    let ops = app.ops().clone();
    ops.spawn("crawls-repair", false, move |cancel, progress| {
        Box::pin(async move {
            if cancel.is_cancelled() {
                return Err(CmdError::new(ErrorCode::Cancelled, "err-cancelled"));
            }
            progress.report(0, None);
            let result = app
                .write_queue()
                .submit_admin(move || {
                    os_store::files::repair_copy(&PathBuf::from(path)).map_err(|e| store_error(&e))
                })
                .await?;
            progress.report(1, Some(1));
            to_json(&serde_json::json!({
                "path": result.repaired.to_string_lossy(),
                "integrityOk": result.integrity_ok,
                "usable": result.usable,
                "lostRows": result.lost_rows(),
            }))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: OUT-230 [M2]
    #[tokio::test]
    async fn invalid_fixture_is_rejected_without_changes() {
        let fixture =
            include_bytes!("../../../../apps/desktop/src/features/crawls/fixtures/fake.ospider");
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("fake.ospider");
        std::fs::write(&path, fixture).unwrap();
        let error = check(path.to_string_lossy().into_owned())
            .await
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::Store);
        assert_eq!(std::fs::read(path).unwrap(), fixture);
    }
    // spec: OUT-230 [M2]
    #[tokio::test]
    async fn configuration_and_repair_use_real_fixture_file_without_mutating_source() {
        let temp = tempfile::tempdir().unwrap();
        let app = App::new(crate::AppConfig::new(temp.path())).unwrap();
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../apps/desktop/src/features/crawls/fixtures/management.json"
        ))
        .unwrap();
        let now = app.config().clock.now();
        let id = app.config().ids.new_crawl_id(now);
        let path = temp.path().join("Alpha.ospider");
        let key = crate::pool::CrawlKey(id.to_string());
        app.write_queue()
            .create_file(
                key,
                path.clone(),
                os_store::CrawlInit {
                    crawl_id: id,
                    mode: os_model::CrawlMode::Spider,
                    start_url: Some(fixture["rows"][0]["startUrl"].as_str().unwrap().into()),
                    list_source: None,
                    project: "Kunde A".into(),
                    name: "Alpha".into(),
                    config: os_model::CrawlConfig::default(),
                    created_at: now,
                },
                os_model::WriteBatch::default(),
            )
            .await
            .unwrap();
        let input = path.to_string_lossy().into_owned();
        let before = std::fs::read(&path).unwrap();
        check(input.clone()).await.unwrap();
        let dto = configuration(&app, input.clone()).await.unwrap();
        assert_eq!(
            dto.config,
            serde_json::to_value(os_model::CrawlConfig::default()).unwrap()
        );
        let handle = repair(&app, input.clone());
        assert!(!handle.cancellable);
        let result = app.ops().wait(&handle.op_id).await.unwrap();
        assert_eq!(
            result.state,
            crate::dto::OpState::Done,
            "{:?}",
            result.error
        );
        let result = result.result.unwrap();
        assert_eq!(result["integrityOk"], true);
        assert_eq!(result["usable"], true);
        assert_eq!(result["lostRows"], 0);
        let repaired = result["path"].as_str().unwrap().to_owned();
        assert!(repaired.ends_with("Alpha.repaired.ospider"));
        check(repaired.clone()).await.unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let copy_before = std::fs::read(&repaired).unwrap();
        let second = repair(&app, input);
        let failure = app.ops().wait(&second.op_id).await.unwrap();
        assert_eq!(failure.state, crate::dto::OpState::Failed);
        assert_eq!(std::fs::read(&repaired).unwrap(), copy_before);
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
}
