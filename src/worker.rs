//! One running and one replaceable pending job per lane. Obsolete work cooperates
//! through checkpoints; dropping a receiver alone is not cancellation.
use std::sync::{
    atomic::{AtomicU64, Ordering},
    mpsc, Arc, Condvar, Mutex,
};

type Job = Box<dyn FnOnce() + Send>;
#[derive(Clone)]
pub struct Cancellation {
    generation: Arc<AtomicU64>,
    expected: u64,
}
impl Cancellation {
    pub fn cancelled(&self) -> bool {
        self.generation.load(Ordering::Acquire) != self.expected
    }
}
thread_local! { static ACTIVE: std::cell::RefCell<Option<Cancellation>> = const { std::cell::RefCell::new(None) }; }
pub fn checkpoint() -> anyhow::Result<()> {
    anyhow::ensure!(
        !ACTIVE.with(|active| active
            .borrow()
            .as_ref()
            .is_some_and(Cancellation::cancelled)),
        "Loading cancelled"
    );
    Ok(())
}
pub fn active_cancellation() -> Option<Cancellation> {
    ACTIVE.with(|active| active.borrow().clone())
}

struct State {
    pending: Option<Job>,
    shutdown: bool,
}
pub struct LatestWorker {
    state: Arc<(Mutex<State>, Condvar)>,
    generation: Arc<AtomicU64>,
}
impl LatestWorker {
    pub fn new() -> Self {
        let state = Arc::new((
            Mutex::new(State {
                pending: None,
                shutdown: false,
            }),
            Condvar::new(),
        ));
        let shared = state.clone();
        std::thread::spawn(move || loop {
            let job = {
                let (mutex, wake) = &*shared;
                let mut state = mutex.lock().unwrap();
                while state.pending.is_none() && !state.shutdown {
                    state = wake.wait(state).unwrap();
                }
                if state.shutdown {
                    break;
                }
                state.pending.take().unwrap()
            };
            // One malformed decoder/job must not permanently kill this lane.
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
            ACTIVE.with(|active| *active.borrow_mut() = None);
        });
        Self {
            state,
            generation: Arc::new(AtomicU64::new(0)),
        }
    }
    pub fn submit<R: Send + 'static>(
        &self,
        task: impl FnOnce() -> R + Send + 'static,
    ) -> mpsc::Receiver<R> {
        let expected = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        let cancellation = Cancellation {
            generation: self.generation.clone(),
            expected,
        };
        let (tx, rx) = mpsc::channel();
        let job = Box::new(move || {
            if cancellation.cancelled() {
                return;
            }
            ACTIVE.with(|active| *active.borrow_mut() = Some(cancellation.clone()));
            let result = task();
            if !cancellation.cancelled() {
                let _ = tx.send(result);
            }
        });
        let (mutex, wake) = &*self.state;
        mutex.lock().unwrap().pending = Some(job);
        wake.notify_one();
        rx
    }
    pub fn cancel(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.state.0.lock().unwrap().pending = None;
    }
}
impl Drop for LatestWorker {
    fn drop(&mut self) {
        self.cancel();
        self.state.0.lock().unwrap().shutdown = true;
        self.state.1.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rapid_requests_keep_only_latest_pending_job() {
        let worker = LatestWorker::new();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let old = worker.submit(move || {
            started_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            1
        });
        started_rx.recv().unwrap();
        let discarded = worker.submit(|| 2);
        let latest = worker.submit(|| 3);
        release_tx.send(()).unwrap();
        assert!(old.recv_timeout(std::time::Duration::from_secs(2)).is_err());
        assert!(discarded
            .recv_timeout(std::time::Duration::from_secs(2))
            .is_err());
        assert_eq!(
            latest
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap(),
            3
        );
    }
}
