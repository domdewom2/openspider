//! Local MCP controls; these commands are never exposed as MCP tools.
use crate::mcp::{self, Service};
use crate::{
    dto::*,
    registry::{CmdResult, CommandMeta, Registry},
    runtime::error::{parse_args, to_json},
    App,
};
use serde_json::Value;
pub fn register(r: &mut Registry) {
    r.register(
        CommandMeta {
            name: "mcp.status",
            args: "void",
            ret: "McpStatus",
            long_running: false,
        },
        |app, _, args| Box::pin(run(app, "mcp.status", args)),
    );
    r.register(
        CommandMeta {
            name: "mcp.start",
            args: "McpStartArgs",
            ret: "McpStatus",
            long_running: false,
        },
        |app, _, args| Box::pin(run(app, "mcp.start", args)),
    );
    r.register(
        CommandMeta {
            name: "mcp.stop",
            args: "void",
            ret: "McpStatus",
            long_running: false,
        },
        |app, _, args| Box::pin(run(app, "mcp.stop", args)),
    );
    r.register(
        CommandMeta {
            name: "mcp.tools",
            args: "void",
            ret: "ToolInfo[]",
            long_running: false,
        },
        |app, _, args| Box::pin(run(app, "mcp.tools", args)),
    );
    r.register(
        CommandMeta {
            name: "mcp.setToolEnabled",
            args: "SetToolEnabledArgs",
            ret: "void",
            long_running: false,
        },
        |app, _, args| Box::pin(run(app, "mcp.setToolEnabled", args)),
    );
    r.register(
        CommandMeta {
            name: "mcp.setClasses",
            args: "SetClassesArgs",
            ret: "void",
            long_running: false,
        },
        |app, _, args| Box::pin(run(app, "mcp.setClasses", args)),
    );
    r.register(
        CommandMeta {
            name: "mcp.token",
            args: "McpTokenArgs",
            ret: "McpToken",
            long_running: false,
        },
        |app, _, args| Box::pin(run(app, "mcp.token", args)),
    );
    r.register(
        CommandMeta {
            name: "mcp.regenerateToken",
            args: "McpTokenArgs",
            ret: "McpToken",
            long_running: false,
        },
        |app, _, args| Box::pin(run(app, "mcp.regenerateToken", args)),
    );
    r.register(
        CommandMeta {
            name: "mcp.setAutostart",
            args: "BoolArgs",
            ret: "void",
            long_running: false,
        },
        |app, _, args| Box::pin(run(app, "mcp.setAutostart", args)),
    );
    r.register(
        CommandMeta {
            name: "mcp.saveToolsMarkdown",
            args: "PathArgs",
            ret: "void",
            long_running: false,
        },
        |app, _, args| Box::pin(run(app, "mcp.saveToolsMarkdown", args)),
    );
    r.register(
        CommandMeta {
            name: "mcp.clientConfigPreview",
            args: "McpClientArgs",
            ret: "McpClientPreview",
            long_running: false,
        },
        |app, _, args| Box::pin(run(app, "mcp.clientConfigPreview", args)),
    );
    r.register(
        CommandMeta {
            name: "mcp.clientConfigWrite",
            args: "McpClientArgs",
            ret: "McpClientWritten",
            long_running: false,
        },
        |app, _, args| Box::pin(run(app, "mcp.clientConfigWrite", args)),
    );
    r.register(
        CommandMeta {
            name: "mcp.callLog",
            args: "PageArgs",
            ret: "CallLogRow[]",
            long_running: false,
        },
        |app, _, args| Box::pin(run(app, "mcp.callLog", args)),
    );
}
async fn run(app: &App, name: &str, args: Value) -> CmdResult {
    let service = app.ext::<Service>();
    let mut state = service.state.lock().await;
    match name {
        "mcp.status" => to_json(&mcp::status(&state).await),
        "mcp.start" => {
            let args: McpStartArgs = parse_args(args)?;
            to_json(&mcp::start(app, &mut state, args.port).await?)
        }
        "mcp.stop" => {
            if let Some(http) = state.http.take() {
                http.stop().await;
            }
            to_json(&McpStatus::Off)
        }
        "mcp.token" | "mcp.regenerateToken" => {
            let args: McpTokenArgs = parse_args(args)?;
            let token = mcp::token(app, &mut state)?;
            let value = if name == "mcp.regenerateToken" {
                token.rotate()
            } else {
                token.copy_token()
            }
            .map_err(mcp::error)?;
            to_json(&McpToken {
                masked: "••••••••".into(),
                value: args.reveal.then_some(value),
            })
        }
        "mcp.tools" => {
            let server = mcp::server(app, mcp::load(app)?)?;
            to_json(
                &server
                    .catalog()
                    .into_iter()
                    .map(|(t, class, enabled)| ToolInfo {
                        class: format!("{class:?}").to_ascii_lowercase(),
                        name: t.name.to_string(),
                        enabled,
                        description: t.description.unwrap_or_default().to_string(),
                        schema: serde_json::json!(t.input_schema),
                    })
                    .collect::<Vec<_>>(),
            )
        }

        "mcp.setClasses" => {
            let args: SetClassesArgs = parse_args(args)?;
            let mut settings = mcp::load(app)?;
            settings.classes = os_mcp::permissions::Permissions::resolve(
                &args.classes,
                false,
                &os_mcp::settings::McpSettings::default(),
            )
            .map_err(mcp::error)?
            .classes()
            .to_vec();
            mcp::save(app, &state, &settings)?;
            Ok(Value::Null)
        }
        "mcp.setToolEnabled" => {
            let args: SetToolEnabledArgs = parse_args(args)?;
            if !os_mcp::tools::all().iter().any(|t| t.name() == args.name) {
                return Err(mcp::error("Unknown MCP tool"));
            }
            let mut settings = mcp::load(app)?;
            settings.tools.insert(args.name, args.enabled);
            mcp::save(app, &state, &settings)?;
            Ok(Value::Null)
        }
        "mcp.setAutostart" => {
            let args: BoolArgs = parse_args(args)?;
            let mut settings = mcp::load(app)?;
            settings.http.autostart = args.value;
            if let Some(http) = &state.http {
                settings.http.port = Some(http.port());
            }
            mcp::save(app, &state, &settings)?;
            Ok(Value::Null)
        }
        "mcp.saveToolsMarkdown" => {
            let args: PathArgs = parse_args(args)?;
            std::fs::write(args.path, mcp::server(app, mcp::load(app)?)?.markdown())
                .map_err(mcp::error)?;
            Ok(Value::Null)
        }
        "mcp.clientConfigPreview" => {
            let args: McpClientArgs = parse_args(args)?;
            to_json(&mcp::clients::preview(app, &mut state, args.client)?)
        }
        "mcp.clientConfigWrite" => {
            let args: McpClientArgs = parse_args(args)?;
            to_json(&mcp::clients::write(app, &mut state, args.client)?)
        }
        "mcp.callLog" => {
            let args: PageArgs = parse_args(args)?;
            let rows = state
                .server
                .as_ref()
                .map(|s| s.call_log().rows())
                .unwrap_or_default();
            to_json(
                &rows
                    .into_iter()
                    .skip(args.offset as usize)
                    .take(args.limit.min(500) as usize)
                    .map(|r| CallLogRow {
                        at: r
                            .at
                            .0
                            .format(&time::format_description::well_known::Rfc3339)
                            .unwrap_or_default(),
                        tool: r.tool,
                        class: r.class,
                        client: r.client,
                        duration_ms: r.duration_ms,
                        ok: r.ok,
                        args_summary: r.args.to_string(),
                        result_bytes: Some(r.result_bytes),
                    })
                    .collect::<Vec<_>>(),
            )
        }
        _ => unreachable!(),
    }
}
