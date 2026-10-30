use super::{InstallWhen, UpdateContext, UpdateState};
/// STUB(M3-U02): never modifies an installed application.
pub async fn install(_ctx: &UpdateContext, _when: InstallWhen) -> UpdateState {
    UpdateState::Failed("STUB(M3-U02): install unavailable".into())
}
