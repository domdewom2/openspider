//! Release update contracts, independent of Tauri. Implementations: M3-U01/U02.
pub mod check;
pub mod download;
pub mod install;
pub mod manifest;
pub mod verify;

pub use crate::dto::update::{
    AvailableUpdate, DisableReason, InstallWhen, PendingInstall, UpdateState,
};
pub use check::check;
pub use manifest::{platform_key, PlatformEntry, UpdateManifest};
pub use verify::{verify, VerifyError};

/// STUB(M3-U01): context will carry app-wide proxy, settings and update state.
#[derive(Debug, Default)]
pub struct UpdateContext;

/// Fixed user agent: no machine, user or installation identifier (OUT-250).
pub const USER_AGENT: &str = "OpenSpider-Updater";
