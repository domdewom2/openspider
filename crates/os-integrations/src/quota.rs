//! Lokaler Verbrauch je Quelle, Property und Pazifiktag; unabhängig von Crawl-Dateien.
use crate::{queue::pacific_day, ApiError};
use os_model::Timestamp;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Mutex,
};
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Ledger {
    counts: BTreeMap<String, u64>,
}
pub struct QuotaLedger {
    path: PathBuf,
    lock: Mutex<()>,
}
impl QuotaLedger {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            lock: Mutex::new(()),
        }
    }
    fn lock_file(&self) -> Result<std::fs::File, ApiError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| failure())?;
        }
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.path.with_extension("lock"))
            .map_err(|_| failure())?;
        file.lock().map_err(|_| failure())?;
        Ok(file)
    }
    fn key(source: &str, property: &str, now: Timestamp) -> String {
        serde_json::to_string(&(source, property, pacific_day(now))).expect("string tuple")
    }
    fn read(&self) -> Result<Ledger, ApiError> {
        match std::fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| failure()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Ledger::default()),
            Err(_) => Err(failure()),
        }
    }
    fn write(&self, ledger: &Ledger) -> Result<(), ApiError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| failure())?;
        }
        let temp = self
            .path
            .with_extension(format!("{}.tmp", oauth2::CsrfToken::new_random().secret()));
        let result = (|| {
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)
                .map_err(|_| failure())?;
            file.write_all(&serde_json::to_vec(ledger).map_err(|_| failure())?)
                .map_err(|_| failure())?;
            file.sync_all().map_err(|_| failure())?;
            std::fs::rename(&temp, &self.path).map_err(|_| failure())
        })(); // rename ersetzt atomar auf den unterstützten Plattformen.
        if result.is_err() {
            let _ = std::fs::remove_file(temp);
        }
        result
    }
    pub fn reserve(
        &self,
        source: &str,
        property: &str,
        now: Timestamp,
        limit: u64,
    ) -> Result<bool, ApiError> {
        let _lock = self.lock.lock().unwrap_or_else(|p| p.into_inner());
        let _file = self.lock_file()?;
        let mut ledger = self.read()?;
        let used = ledger
            .counts
            .entry(Self::key(source, property, now))
            .or_default();
        if *used >= limit {
            return Ok(false);
        }
        *used += 1;
        self.write(&ledger)?;
        Ok(true)
    }
    pub fn remaining(
        &self,
        source: &str,
        property: &str,
        now: Timestamp,
        limit: u64,
    ) -> Result<u64, ApiError> {
        let _lock = self.lock.lock().unwrap_or_else(|p| p.into_inner());
        let _file = self.lock_file()?;
        Ok(limit.saturating_sub(
            *self
                .read()?
                .counts
                .get(&Self::key(source, property, now))
                .unwrap_or(&0),
        ))
    }
    pub fn exhaust(
        &self,
        source: &str,
        property: &str,
        now: Timestamp,
        limit: u64,
    ) -> Result<(), ApiError> {
        let _lock = self.lock.lock().unwrap_or_else(|p| p.into_inner());
        let _file = self.lock_file()?;
        let mut ledger = self.read()?;
        ledger
            .counts
            .insert(Self::key(source, property, now), limit);
        self.write(&ledger)
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
}
fn failure() -> ApiError {
    ApiError::Transport("API-Kontingentdatei nicht lesbar oder schreibbar".into())
}
/// Nächste Tagesgrenze, auch an Tagen mit 23 bzw. 25 Stunden.
pub fn next_reset(now: Timestamp) -> Timestamp {
    let day = pacific_day(now);
    let mut lo = now.unix_millis();
    let mut hi = lo + 27 * 60 * 60 * 1000;
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if pacific_day(Timestamp::from_unix_millis(mid)) == day {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Timestamp::from_unix_millis(hi)
}
