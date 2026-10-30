//! Installer cleanup: remove OS registrations, retaining all user files.
use crate::exit::CliError;
use os_sched::planner::{CommandRunner, OsPlanner, SystemRunner};

/// Remove every registered task and propagate failures to the installer.
pub fn cleanup_with(planner: &dyn OsPlanner) -> Result<(), CliError> {
    for id in planner.registered_ids() {
        planner.uninstall(&id).map_err(|e| {
            CliError::internal(format!("Aufgabe {id} konnte nicht entfernt werden: {e}"))
        })?;
    }
    Ok(())
}

/// Enumerate the OS namespace, independent of possibly missing local task XML files.
pub fn cleanup_windows_with(runner: &dyn CommandRunner) -> Result<(), CliError> {
    let out = runner
        .run(
            "schtasks",
            &["/Query".into(), "/FO".into(), "CSV".into(), "/NH".into()],
        )
        .map_err(|e| CliError::internal(format!("Aufgaben konnten nicht gelesen werden: {e}")))?;
    if out.status != 0 {
        return Err(CliError::internal("Aufgaben konnten nicht gelesen werden"));
    }
    for line in out.stdout.lines() {
        // schtasks CSV always quotes the first field; doubled quotes escape a quote.
        let Some(rest) = line.strip_prefix('"') else {
            continue;
        };
        let mut name = String::new();
        let mut chars = rest.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                } else {
                    break;
                }
            }
            name.push(c);
        }
        if !name.to_ascii_lowercase().starts_with("\\openspider\\") {
            continue;
        }
        let result = runner
            .run(
                "schtasks",
                &["/Delete".into(), "/TN".into(), name, "/F".into()],
            )
            .map_err(|e| {
                CliError::internal(format!("Aufgabe konnte nicht entfernt werden: {e}"))
            })?;
        if result.status != 0 {
            return Err(CliError::internal("Aufgabe konnte nicht entfernt werden"));
        }
    }
    Ok(())
}

pub fn cleanup() -> Result<(), CliError> {
    #[cfg(windows)]
    {
        cleanup_windows_with(&SystemRunner)
    }
    #[cfg(not(windows))]
    {
        let dir = os_store::paths::app_paths()
            .settings
            .join("schedules")
            .join("planner");
        if let Some(planner) = os_sched::planner::system(std::sync::Arc::new(SystemRunner), &dir) {
            cleanup_with(planner.as_ref())?;
        }
        Ok(())
    }
}
