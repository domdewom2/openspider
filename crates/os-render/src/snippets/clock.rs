//! The same injected time source drives deadlines, waits and trace timestamps.
use std::{future::Future, time::Duration};

pub(super) trait Clock: Sync {
    fn now(&self) -> Duration;
    fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send;
}

pub(super) struct TokioClock(tokio::time::Instant);
impl Default for TokioClock {
    fn default() -> Self {
        Self(tokio::time::Instant::now())
    }
}
impl Clock for TokioClock {
    fn now(&self) -> Duration {
        self.0.elapsed()
    }
    async fn sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}
