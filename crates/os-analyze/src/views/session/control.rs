//! Bridges the transport-independent token to SQLite's interrupt handle during builds.
use os_model::CancelToken;
use os_store::CrawlFile;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::Duration,
};

pub(super) struct InterruptGuard {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl InterruptGuard {
    pub fn new(file: &CrawlFile, cancel: CancelToken) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let interrupt = file.conn().get_interrupt_handle();
        let thread = std::thread::spawn(move || {
            while !worker_stop.load(Ordering::Acquire) {
                if cancel.is_cancelled() {
                    interrupt.interrupt();
                    break;
                }
                std::thread::park_timeout(Duration::from_millis(2));
            }
        });
        Self {
            stop,
            thread: Some(thread),
        }
    }
}
impl Drop for InterruptGuard {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            let _ = thread.join();
        }
    }
}
