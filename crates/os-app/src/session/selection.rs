//! Resolve explicit IDs or a versioned view through its public command contract.
use crate::{dto::*, runtime::error::parse_args, App, CallCtx, CmdError};
use std::collections::BTreeSet;
pub async fn resolve(
    app: &App,
    ctx: &CallCtx,
    selection: Selection,
) -> Result<Vec<os_model::UrlId>, CmdError> {
    let ids = match selection {
        Selection::Ids { ids } => ids,
        Selection::All {
            view_id,
            version,
            except,
        } => {
            let except: BTreeSet<_> = except.into_iter().collect();
            let mut ids = Vec::new();
            let mut offset = 0;
            loop {
                if ctx.cancel.is_cancelled() {
                    return Err(CmdError::new(ErrorCode::Cancelled, "err-cancelled"));
                }
                let page: RowWindow = parse_args(app.dispatch(ctx.clone(),"view.rows",serde_json::json!({"viewId":view_id,"version":version,"offset":offset,"limit":500,"columns":[]})).await?)?;
                if page.version != version {
                    return Err(CmdError::new(ErrorCode::Stale, "err-view-stale"));
                }
                let n = page.row_keys.len();
                ids.extend(page.row_keys.into_iter().filter(|id| !except.contains(id)));
                if n < 500 {
                    break;
                }
                offset += n as u64;
            }
            ids
        }
    };
    if ids.iter().any(|id| *id <= 0) {
        return Err(CmdError::invalid_input("err-url-id", "selection"));
    }
    Ok(ids
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(os_model::UrlId)
        .collect())
}
