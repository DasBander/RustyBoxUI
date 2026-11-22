//! Cooperative background work. Only ordinary Send data crosses the UI thread boundary.
use crate::Result;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread,
    time::Duration,
};

pub struct JobContext<P> {
    cancelled: Arc<AtomicBool>,
    progress: SyncSender<P>,
}
impl<P> JobContext<P> {
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
    pub fn checkpoint(&self) -> Result<()> {
        if self.is_cancelled() {
            Err("Job cancelled".into())
        } else {
            Ok(())
        }
    }
    /// Bound pending progress without blocking the UI or losing a final item when the queue fills.
    pub fn report(&self, mut item: P) -> Result<()> {
        loop {
            self.checkpoint()?;
            match self.progress.try_send(item) {
                Ok(()) => return Ok(()),
                Err(TrySendError::Disconnected(_)) => return Err("Job receiver closed".into()),
                Err(TrySendError::Full(value)) => {
                    item = value;
                    thread::sleep(Duration::from_millis(4));
                }
            }
        }
    }
}
pub struct Job<P, R> {
    cancelled: Arc<AtomicBool>,
    progress: Receiver<P>,
    result: Receiver<Result<R>>,
}
impl<P: Send + 'static, R: Send + 'static> Job<P, R> {
    pub fn spawn(work: impl FnOnce(JobContext<P>) -> Result<R> + Send + 'static) -> Result<Self> {
        let cancelled = Arc::new(AtomicBool::new(false));
        let (progress, updates) = mpsc::sync_channel(32);
        let (done, result) = mpsc::channel();
        let context = JobContext {
            cancelled: cancelled.clone(),
            progress,
        };
        thread::Builder::new()
            .name("rustybox-job".into())
            .spawn(move || {
                let answer =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(context)))
                        .unwrap_or_else(|_| Err("Background job panicked".into()));
                let _ = done.send(answer);
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            cancelled,
            progress: updates,
            result,
        })
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
    pub fn progress(&self) -> impl Iterator<Item = P> + '_ {
        self.progress.try_iter()
    }
    pub fn try_finish(&self) -> Option<Result<R>> {
        self.result.try_recv().ok()
    }
}
impl<P, R> Drop for Job<P, R> {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}
