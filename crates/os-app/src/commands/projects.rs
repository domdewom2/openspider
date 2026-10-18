//! Projekte der Crawl-Verwaltung (M2-O02, OUT-228/229).
use super::crawls::{admin, management_guard};
use crate::dto::*;
use crate::registry::{CmdResult, CommandMeta, Registry};
use crate::runtime::error::{parse_args, store_error, to_json};
use crate::runtime::{App, CallCtx};
use futures_util::future::BoxFuture;

fn info(p: os_store::registry::Project) -> ProjectInfo {
    ProjectInfo {
        id: p.id,
        name: p.name,
        locked: p.locked,
        crawls: p.crawls,
    }
}
fn dispatch<'a>(
    app: &'a App,
    _ctx: CallCtx,
    args: serde_json::Value,
    action: &'static str,
) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let _guard = management_guard(app).await;
        let now = app.config().clock.now();
        match action {
            "projects.list" => to_json(
                &admin(app, |m| {
                    m.projects()
                        .map(|p| p.into_iter().map(info).collect::<Vec<_>>())
                        .map_err(|e| store_error(&e))
                })
                .await?,
            ),
            "projects.create" => {
                let ProjectCreateArgs { name } = parse_args(args)?;
                let id = app.config().ids.new_crawl_id(now).to_string();
                to_json(
                    &admin(app, move |m| {
                        m.create_project(&id, &name)
                            .map(info)
                            .map_err(|e| store_error(&e))
                    })
                    .await?,
                )
            }
            "projects.rename" => {
                let ProjectRenameArgs { id, name } = parse_args(args)?;
                admin(app, move |m| {
                    m.rename_project(&id, &name, now)
                        .map_err(|e| store_error(&e))
                })
                .await?;
                Ok(serde_json::Value::Null)
            }
            _ => {
                let ProjectLockArgs { id, locked } = parse_args(args)?;
                admin(app, move |m| {
                    m.lock_project(&id, locked).map_err(|e| store_error(&e))
                })
                .await?;
                Ok(serde_json::Value::Null)
            }
        }
    })
}
pub fn register(r: &mut Registry) {
    macro_rules! command {
        ($name:literal,$args:literal,$ret:literal) => {
            r.register(
                CommandMeta {
                    name: $name,
                    args: $args,
                    ret: $ret,
                    long_running: false,
                },
                |app, ctx, args| dispatch(app, ctx, args, $name),
            );
        };
    }
    command!("projects.list", "void", "ProjectInfo[]");
    command!("projects.create", "ProjectCreateArgs", "ProjectInfo");
    command!("projects.rename", "ProjectRenameArgs", "void");
    command!("projects.lock", "ProjectLockArgs", "void");
}
