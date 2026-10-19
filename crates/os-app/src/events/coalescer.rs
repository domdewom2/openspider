//! One clock and one budget per window, including operations and hidden windows.
use crate::{runtime::hooks::TickPolicy, App, WindowId};
use std::{collections::HashMap, sync::Mutex, time::Duration};

/// Tick budget for counters and status, capped at one second (K2-39).
/// Unlike `App::window_period`, this deadline does not follow table intervals
/// above one second. The session feed independently debounces row updates at
/// the configured live interval; hidden windows receive at most one tick/s.
#[derive(Default)]
pub struct Coalescer {
    last: Mutex<HashMap<WindowId, Duration>>,
}
impl TickPolicy for Coalescer {
    fn due_windows(&self, app: &App) -> Vec<WindowId> {
        let now = app.config().clock.monotonic();
        let windows = app.subscribed_windows();
        let mut last = self.last.lock().unwrap_or_else(|p| p.into_inner());
        last.retain(|w, _| windows.contains(w));
        windows
            .into_iter()
            .filter(|w| {
                let period = if app.is_visible(w) {
                    Duration::from_millis(u64::from(app.live_interval_ms().clamp(100, 1000)))
                } else {
                    Duration::from_secs(1)
                };
                let previous = last.entry(w.clone()).or_insert(now);
                if now.saturating_sub(*previous) >= period {
                    *previous = now;
                    true
                } else {
                    false
                }
            })
            .collect()
    }
}
