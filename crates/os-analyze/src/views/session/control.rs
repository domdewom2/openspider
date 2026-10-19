//! Direct cancellation bridge, without a polling thread per build.
use os_model::{cancel::CancelRegistration, CancelToken};
use os_store::CrawlFile;
pub(super) struct InterruptGuard {
    _registration: CancelRegistration,
}
impl InterruptGuard {
    pub fn new(file: &CrawlFile, cancel: CancelToken) -> Self {
        let interrupt = file.conn().get_interrupt_handle();
        Self {
            _registration: cancel.on_cancel(move || interrupt.interrupt()),
        }
    }
}

/// Connection-local build settings. Restore the caller's settings on every exit.
pub(super) struct BuildPragmas<'a> {
    conn: &'a rusqlite::Connection,
    temp_store: i64,
    cache_size: i64,
    change_temp: bool,
}
impl<'a> BuildPragmas<'a> {
    pub fn new(conn: &'a rusqlite::Connection) -> Result<Self, rusqlite::Error> {
        let guard = Self {
            conn,
            temp_store: conn.pragma_query_value(None, "temp_store", |r| r.get(0))?,
            cache_size: conn.pragma_query_value(None, "cache_size", |r| r.get(0))?,
            change_temp: conn.is_autocommit(),
        };
        if guard.change_temp && guard.temp_store != 2 {
            conn.pragma_update(None, "temp_store", "MEMORY")?;
        }
        conn.pragma_update(None, "cache_size", -131072)?;
        Ok(guard)
    }
}
impl Drop for BuildPragmas<'_> {
    fn drop(&mut self) {
        let _ = self.conn.pragma_update(None, "cache_size", self.cache_size);
        if self.change_temp && self.temp_store != 2 {
            let _ = self.conn.pragma_update(None, "temp_store", self.temp_store);
        }
    }
}
