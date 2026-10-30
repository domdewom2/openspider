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
pub use verify::{verify, verify_release, VerifyError};

pub mod policy;
pub use policy::{UpdateContext, USER_AGENT};
