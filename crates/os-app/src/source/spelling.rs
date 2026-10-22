//! K09 IPC adapters for the spelling service supplied by D19.
use crate::{
    commands::{catalog::view_error, detail_spelling},
    dto::*,
    registry::CmdResult,
    runtime::{
        error::{parse_args, to_json},
        sessions::SessionInfo,
        App, CallCtx,
    },
};
use os_analyze::tabs::content;
use os_parse::spelling::dictionary;

/// Add the U21 actions to D19's existing command registration.
pub fn register(r: &mut crate::registry::Registry) {
    macro_rules! action {
        ($name:literal, $args:literal) => {
            r.register(
                crate::registry::CommandMeta {
                    name: $name,
                    args: $args,
                    ret: "void",
                    long_running: true,
                },
                |app, ctx, args| {
                    Box::pin(async move {
                        let session = crate::commands::catalog::session(app, &ctx)?;
                        spelling_action(app, ctx, session, args, $name).await
                    })
                },
            );
        };
    }
    action!("detail.spellingIgnore", "SpellingIgnoreArgs");
    action!("detail.spellingDictionary", "SpellingDictionaryArgs");
    action!("detail.spellingRecheck", "void");
}

pub async fn spelling_action(
    app: &App,
    ctx: CallCtx,
    session: SessionInfo,
    args: serde_json::Value,
    action: &'static str,
) -> CmdResult {
    if !session.writable {
        return Err(CmdError::new(ErrorCode::Permission, "err-db-read-only"));
    }
    if session.state == os_model::CrawlState::Running {
        return Err(CmdError::new(
            ErrorCode::CrawlRunning,
            "err-config-pause-required",
        ));
    }
    let mut word = None;
    let mut rule = None;
    let mut dictionary_word = None;
    match action {
        "detail.spellingIgnore" => {
            let a: SpellingIgnoreArgs = parse_args(args)?;
            if let Some(url_id) = a.url_id {
                let exists = app
                    .read_pool()
                    .run_snapshot(session.key.clone(), move |file, _| {
                        Ok(
                            os_store::queries::url_by_id(file.conn(), os_model::UrlId(url_id))?
                                .is_some(),
                        )
                    })
                    .await?;
                if !exists {
                    return Err(CmdError::new(ErrorCode::NotFound, "err-url-not-found"));
                }
            }
            word = a.word.filter(|s| !s.trim().is_empty());
            rule = a.rule_id.filter(|s| !s.trim().is_empty());
            if word.is_some() == rule.is_some() {
                return Err(CmdError::invalid_input("err-spelling-action", "word"));
            }
        }
        "detail.spellingDictionary" => {
            let a: SpellingDictionaryArgs = parse_args(args)?;
            if a.word.trim().is_empty() || a.word.len() > 1000 || a.word.contains(['\n', '\r']) {
                return Err(CmdError::invalid_input("err-spelling-action", "word"));
            }
            let language = os_parse::spelling::language_code(&a.language);
            if language.is_empty()
                || language.len() > 64
                || !language
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            {
                return Err(CmdError::invalid_input("err-spelling-action", "language"));
            }
            dictionary_word = Some(a);
        }
        _ => {}
    }
    let worker = app.clone();
    to_json(
        &app.ops()
            .spawn("op-spelling-recheck", true, move |cancel, progress| {
                Box::pin(async move {
                    let mut cfg = (*session.config).clone();
                    if let Some(rule) = rule {
                        detail_spelling::ignore_rule(&mut cfg, &rule);
                        let config_args = serde_json::json!({
                            "schemaVersion": cfg.schema_version,
                            "generator": cfg.generator,
                            "config": cfg,
                        });
                        worker.dispatch(ctx, "config.set", config_args).await?;
                    }
                    let resources = dictionary::under(&worker.config().paths.settings);
                    let work_cancel = cancel.clone();
                    worker
                        .write_queue()
                        .submit_file(&session.key, move |file| {
                            if work_cancel.is_cancelled() {
                                return Err(CmdError::new(ErrorCode::Cancelled, "err-cancelled"));
                            }
                            if let Some(word) = word {
                                content::ignore_spelling_word(file, &word).map_err(view_error)?;
                            }
                            if let Some(a) = dictionary_word {
                                detail_spelling::add_to_dictionary(
                                    &a.language,
                                    &a.word,
                                    &resources,
                                )
                                .map_err(|e| {
                                    CmdError::new(ErrorCode::Io, "err-dictionary-write")
                                        .with_detail(e.to_string())
                                })?;
                            }
                            progress.report(0, None);
                            let changed = content::recheck_spelling(
                                file,
                                &cfg,
                                &detail_spelling::checker(&cfg, &resources),
                                &work_cancel,
                            )
                            .map_err(view_error)?;
                            progress.report(changed.len() as u64, Some(changed.len() as u64));
                            Ok(serde_json::Value::Null)
                        })
                        .await
                })
            }),
    )
}
