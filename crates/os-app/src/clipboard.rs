//! Clipboard TSV, with a hard cell budget before writing to the desktop.
use crate::{
    commands::view,
    dto::*,
    runtime::{App, CallCtx},
};
use std::collections::HashSet;
/// Maximum number of data cells accepted by view.copy.
pub const MAX_COPY_CELLS: u64 = 1_000_000;
pub fn too_large() -> CmdError {
    CmdError::new(ErrorCode::Unsupported, "err-copy-too-large")
}
fn clean(value: &serde_json::Value) -> String {
    let s = match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(s) => s.clone(),
        v => v.to_string(),
    };
    s.replace(['\t', '\r', '\n'], " ")
}
/// Append one TSV row. This is independent of locale and platform in tests.
pub fn tsv_row(values: &[serde_json::Value], windows: bool) -> String {
    let mut text = values.iter().map(clean).collect::<Vec<_>>().join("\t");
    text.push_str(if windows { "\r\n" } else { "\n" });
    text
}
pub(crate) async fn copy(app: &App, ctx: &CallCtx, args: CopyArgs) -> Result<CopyResult, CmdError> {
    let v = view::get(app, ctx, &args.view_id)?;
    let (ids, except) = match &args.selection {
        Selection::Ids { ids } => (
            Some(ids.iter().copied().collect::<HashSet<_>>()),
            HashSet::new(),
        ),
        Selection::All {
            view_id,
            version,
            except,
        } => {
            if view_id != &args.view_id || *version != v.version {
                return Err(view::stale());
            }
            (None, except.iter().copied().collect())
        }
    };
    let targeted = ids.is_some() && view::detail_spec(&v.spec).is_none();
    let selected = ids
        .as_ref()
        .map(|ids| ids.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default();
    let probe = if targeted {
        view::selected_page(app, &v, args.columns.clone(), selected.clone(), 0, 500).await?
    } else {
        view::page(
            app,
            &v.session,
            v.spec.clone(),
            args.columns.clone(),
            0,
            1,
            Some(v.version),
        )
        .await?
    };
    let width = probe.columns.len() as u64;
    if width == 0 {
        return Err(CmdError::invalid_input("err-copy-columns", "columns"));
    }
    if ids.is_none() && probe.total.saturating_sub(except.len() as u64) > MAX_COPY_CELLS / width {
        return Err(too_large());
    }
    let mut text = String::new();
    let windows = cfg!(windows);
    if args.with_header {
        text.push_str(&tsv_row(
            &probe
                .columns
                .iter()
                .map(|c| serde_json::Value::String(c.key.clone()))
                .collect::<Vec<_>>(),
            windows,
        ));
    }
    let mut count = 0u64;
    let mut append = |p: &view::DataPage| -> Result<(), CmdError> {
        for (key, row) in p.keys.iter().zip(&p.cells) {
            if ids.as_ref().is_none_or(|ids| ids.contains(key)) && !except.contains(key) {
                count += 1;
                if count > MAX_COPY_CELLS / width {
                    return Err(too_large());
                }
                text.push_str(&tsv_row(row, windows));
            }
        }
        Ok(())
    };
    if targeted {
        // The first restricted page doubles as the schema probe.
        append(&probe)?;
        let mut offset = probe.keys.len() as u64;
        while offset < probe.total {
            let p =
                view::selected_page(app, &v, args.columns.clone(), selected.clone(), offset, 500)
                    .await?;
            append(&p)?;
            if p.keys.is_empty() {
                break;
            }
            offset += p.keys.len() as u64;
        }
    } else if let Some(ids) = &ids {
        // Detail sources expose ordinal row keys. Read only the requested contiguous ranges
        // through DetailReq.offset/limit, preserving the original source URL selection.
        let mut keys = ids.iter().copied().filter(|id| *id > 0).collect::<Vec<_>>();
        keys.sort_unstable();
        let mut start = 0;
        while start < keys.len() {
            let mut end = start + 1;
            while end < keys.len() && end - start < 500 && keys[end] == keys[end - 1] + 1 {
                end += 1;
            }
            let p = view::page(
                app,
                &v.session,
                v.spec.clone(),
                args.columns.clone(),
                (keys[start] - 1) as u64,
                (end - start) as u32,
                Some(v.version),
            )
            .await?;
            append(&p)?;
            start = end;
        }
    } else {
        let mut offset = 0;
        loop {
            let p = view::page(
                app,
                &v.session,
                v.spec.clone(),
                args.columns.clone(),
                offset,
                500,
                Some(v.version),
            )
            .await?;
            append(&p)?;
            offset += p.keys.len() as u64;
            if p.keys.is_empty() || offset >= p.total {
                break;
            }
        }
    }
    let desktop = app
        .desktop_actions()
        .ok_or_else(|| CmdError::new(ErrorCode::Unsupported, "err-desktop-unavailable"))?;
    let bytes = text.len() as u64;
    tokio::task::spawn_blocking(move || desktop.write_clipboard(&text))
        .await
        .map_err(|e| crate::runtime::error::internal("view.copy", e.to_string()))??;
    Ok(CopyResult { rows: count, bytes })
}
