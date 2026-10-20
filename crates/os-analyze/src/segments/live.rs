//! Headless change-feed observer. The app uses its existing read pool/write queue instead.
use super::{live_snapshot, reassign, reassign_urls, SegmentError};
use os_model::CancelToken;
use os_store::{CrawlFile, OpenMode};
use std::{
    path::PathBuf,
    thread::{self, JoinHandle},
    time::Duration,
};

/// Joins its worker at the end of a crawl; a final pass observes completed analysis.
pub struct Observer {
    stop: CancelToken,
    worker: Option<JoinHandle<()>>,
}
impl Drop for Observer {
    fn drop(&mut self) {
        self.stop.cancel();
        if let Some(worker) = self.worker.take() {
            worker.thread().unpark();
            let _ = worker.join();
        }
    }
}
pub fn observe(path: PathBuf, cancel: CancelToken) -> std::io::Result<Observer> {
    let stop = CancelToken::new();
    let stopped = stop.clone();
    let worker = thread::Builder::new()
        .name("os-segments".into())
        .spawn(move || {
            let mut previous = None;
            let mut file = None;
            loop {
                if cancel.is_cancelled() {
                    break;
                }
                if file.is_none() {
                    match CrawlFile::open(&path, OpenMode::ReadWrite) {
                        Ok(opened) => {
                            let _ = opened.conn().busy_timeout(Duration::from_secs(5));
                            file = Some(opened);
                        }
                        Err(error) => tracing::warn!(%error,"Cannot open segment observer"),
                    }
                }
                if let Some(file) = &file {
                    match refresh(file, previous.as_ref(), &cancel) {
                        Ok(Some(state)) => previous = Some(state),
                        Ok(None) => {}
                        Err(error) => tracing::warn!(%error,"Segment observer update failed"),
                    }
                }
                if stopped.is_cancelled() {
                    break;
                }
                thread::park_timeout(Duration::from_millis(250));
            }
        })?;
    Ok(Observer {
        stop,
        worker: Some(worker),
    })
}
fn refresh(
    file: &CrawlFile,
    previous: Option<&(String, u64)>,
    cancel: &CancelToken,
) -> Result<Option<(String, u64)>, SegmentError> {
    let since = previous.map_or(0, |(_, seq)| *seq);
    let Some(snapshot) = live_snapshot(file, since)? else {
        return Ok(None);
    };
    if previous == Some(&(snapshot.fingerprint.clone(), snapshot.sequence)) {
        return Ok(None);
    }
    let cfg = file.config()?;
    let incremental = previous.is_some_and(|(fingerprint, seq)| {
        fingerprint == &snapshot.fingerprint && snapshot.sequence >= *seq
    }) && !snapshot.changed.is_empty()
        && snapshot.changed.len() <= 4096;
    if incremental && os_store::queries::change_seq(file.conn())? == snapshot.sequence {
        reassign_urls(file, &cfg, &snapshot.changed, cancel)?;
    } else {
        reassign(file, &cfg, cancel)?;
    }
    let sequence = file
        .get_meta("segments.assignment_seq")?
        .and_then(|s| s.parse().ok())
        .unwrap_or(snapshot.sequence);
    Ok(Some((snapshot.fingerprint, sequence)))
}
