//! OAuth reuses I01's one-shot loopback callback and SecretStoreMut-backed accounts.
use crate::{credentials::Credentials, http, oauth, ApiCtx, ApiError};
use os_model::{ApiKind, Clock};
use std::sync::Arc;

/// The host opens the authorization URL in its system browser (tests use a callback).
#[allow(clippy::too_many_arguments)]
pub async fn connect(
    ctx: &ApiCtx,
    credentials: &Credentials,
    client_id: &str,
    client_secret: Option<&str>,
    endpoints: &oauth::Endpoints,
    clock: Arc<dyn Clock>,
    remember: bool,
    open: impl FnOnce(&str) -> Result<(), ApiError>,
) -> Result<crate::credentials::Credential, ApiError> {
    let authorization = oauth::Authorization::begin_drive(client_id, endpoints).await?;
    open(authorization.url.as_str())?;
    let credential = authorization
        .finish(
            &http::client_for(ctx)?,
            client_id,
            client_secret,
            endpoints,
            clock,
        )
        .await?;
    if !credential.scopes.iter().any(|s| s == super::SCOPE) {
        return Err(super::reconnect());
    }
    credentials.put(ApiKind::Drive, credential.clone(), remember)?;
    Ok(credential)
}
pub async fn refresh_account(
    ctx: &ApiCtx,
    credentials: &Credentials,
    account: &str,
    client_id: &str,
    endpoints: &oauth::Endpoints,
    clock: &dyn Clock,
) -> Result<(), ApiError> {
    let mut credential =
        crate::credentials::read_account(ctx.secrets.as_ref(), ApiKind::Drive, Some(account))?;
    oauth::refresh(
        &mut credential,
        &http::client_for(ctx)?,
        client_id,
        endpoints,
        clock,
    )
    .await
    .map_err(|_| super::reconnect())?;
    let remember = credentials.is_persistent(ApiKind::Drive, account);
    credentials.put(ApiKind::Drive, credential, remember)?;
    Ok(())
}

/// Standard desktop entry point; the callback variant above is used by headless hosts/tests.
pub async fn connect_in_browser(
    ctx: &ApiCtx,
    credentials: &Credentials,
    client_id: &str,
    client_secret: Option<&str>,
    endpoints: &oauth::Endpoints,
    clock: Arc<dyn Clock>,
    remember: bool,
) -> Result<crate::credentials::Credential, ApiError> {
    connect(
        ctx,
        credentials,
        client_id,
        client_secret,
        endpoints,
        clock,
        remember,
        |url| {
            webbrowser::open(url).map(|_| ()).map_err(|_| {
                ApiError::Transport("Systembrowser konnte nicht geöffnet werden".into())
            })
        },
    )
    .await
}
