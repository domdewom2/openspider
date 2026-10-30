//! Repair moved CLI paths without re-enabling intentionally unregistered tasks.
use super::{OsPlanner, PlannerError, PlannerSpec};
/// Stable bundle paths need no rewrite; moved installations are replaced idempotently.
pub fn repair(planner: &dyn OsPlanner, spec: &PlannerSpec) -> Result<bool, PlannerError> {
    if !planner.is_registered(&spec.task_id)
        || planner.registered_binary(&spec.task_id).as_deref() == Some(spec.binary.as_path())
    {
        return Ok(false);
    }
    planner.install(spec)?;
    Ok(true)
}
