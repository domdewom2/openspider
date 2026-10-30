use super::{DisableReason, UpdateContext, UpdateState};
/// STUB(M3-U01): never performs network access or reports a successful check.
pub async fn check(_ctx: &UpdateContext, _manual: bool) -> UpdateState {
    UpdateState::Disabled(DisableReason::NotImplemented)
}
