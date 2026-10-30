use super::{UpdateContext, UpdateState};
/// STUB(M3-U02): no download before the verification pipeline is implemented.
pub async fn download(_ctx: &UpdateContext) -> UpdateState {
    UpdateState::Failed("STUB(M3-U02): download unavailable".into())
}
