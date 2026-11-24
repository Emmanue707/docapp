use std::any::Any;
use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::cancel::CancelToken;
use crate::progress::{Progress, ProgressSink, SharedThrottle};
use crate::{Error, Result};

static NEXT_JOB_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Priority {
    High,
    Normal,
    Low,
}

impl Priority {
    fn rank(self) -> u8 {
        match self {
            Self::High => 0,
            Self::Normal => 1,
            Self::Low => 2,
        }
    }
}

pub struct JobCtx {
    token: CancelToken,
    sink: Option<ProgressSink>,
    throttle: Arc<SharedThrottle>,
}

impl JobCtx {
    pub fn cancel_token(&self) -> &CancelToken {
        &self.token
    }

    pub fn report(&self, done: u64, total: u64, stage: impl Into<String>) {
        self.throttle
            .report(&self.sink, Progress::new(done, total, stage));
    }
}

enum JobOutcome {
    Value(Box<dyn Any + Send>),
    Err(Error),
}

type JobWork = Box<dyn FnOnce(JobCtx) -> Result<Box<dyn Any + Send>> + Send>;

struct JobRecord {
    id: u64,
    priority: Priority,
    token: CancelToken,
    sink: Option<ProgressSink>,
    throttle: Arc<SharedThrottle>,
    run: Mutex<Option<JobWork>>,
    result: Mutex<Option<JobOutcome>>,
}

impl JobRecord {
    fn set_result(&self, outcome: JobOutcome) {
        if let Ok(mut slot) = self.result.lock() {
            *slot = Some(outcome);
        }
    }
}

type JobBox = Arc<JobRecord>;

struct JobQueue {
    jobs: VecDeque<JobBox>,
}

impl JobQueue {
    fn push(&mut self, job: JobBox) {
        let rank = job.priority.rank();
        let idx = self
            .jobs
            .iter()
            .position(|existing| existing.priority.rank() > rank)
            .unwrap_or(self.jobs.len());
        self.jobs.insert(idx, job);
    }

    fn pop_highest(&mut self) -> Option<JobBox> {
        self.jobs.pop_front()
    }
}

struct RunnerState {
    queue: Mutex<JobQueue>,
    queue_cap: usize,
    permits: Mutex<usize>,
    waiters: Condvar,
    progress: Option<ProgressSink>,
    shutdown: Mutex<bool>,
}

impl RunnerState {
    fn grant_permit(&self) -> Result<()> {
        let mut permits = self
            .permits
            .lock()
            .map_err(|_| Error::Backend("job queue poisoned".to_owned()))?;
        if *permits >= self.queue_cap {
            let (guard, _) = self
                .waiters
                .wait_timeout_while(permits, Duration::from_millis(50), |p| *p >= self.queue_cap)
                .map_err(|_| Error::Backend("job queue poisoned".to_owned()))?;
            permits = guard;
            if self.shutdown_locked()? {
                return Err(Error::Cancelled);
            }
        }
        *permits += 1;
        Ok(())
    }

    fn release_permit(&self) {
        if let Ok(mut permits) = self.permits.lock() {
            if *permits > 0 {
                *permits -= 1;
            }
            self.waiters.notify_one();
        }
    }

    fn shutdown_locked(&self) -> Result<bool> {
        let shutdown = self
            .shutdown
            .lock()
            .map_err(|_| Error::Backend("job runner poisoned".to_owned()))?;
        Ok(*shutdown)
    }

    fn is_shutdown(&self) -> bool {
        self.shutdown.lock().map(|g| *g).unwrap_or(true)
    }

    fn enqueue(&self, job: JobBox) -> Result<()> {
        if job.token.is_cancelled() {
            job.set_result(JobOutcome::Err(Error::Cancelled));
            return Ok(());
        }
        self.grant_permit()?;
        if job.token.is_cancelled() {
            job.set_result(JobOutcome::Err(Error::Cancelled));
            self.release_permit();
            return Ok(());
        }
        let mut queue = self
            .queue
            .lock()
            .map_err(|_| Error::Backend("job queue poisoned".to_owned()))?;
        queue.push(job);
        Ok(())
    }

    fn take_job(&self) -> Option<JobBox> {
        let mut queue = self.queue.lock().ok()?;
        queue.pop_highest()
    }
}

enum WorkerMsg {
    Job,
    Shutdown,
}

pub struct JobRunner {
    state: Arc<RunnerState>,
    workers: Vec<JoinHandle<()>>,
    job_tx: SyncSender<WorkerMsg>,
    #[expect(dead_code)]
    job_rx: Arc<Mutex<Receiver<WorkerMsg>>>,
}

pub struct JobRunnerBuilder {
    workers: usize,
    queue_cap: usize,
    progress: Option<ProgressSink>,
}

impl JobRunnerBuilder {
    pub fn workers(mut self, count: usize) -> Self {
        self.workers = count.max(1);
        self
    }

    pub fn queue_capacity(mut self, cap: usize) -> Self {
        self.queue_cap = cap.max(1);
        self
    }

    pub fn progress_sink(mut self, sink: ProgressSink) -> Self {
        self.progress = Some(sink);
        self
    }

    pub fn build(self) -> JobRunner {
        JobRunner::new(self.workers, self.queue_cap, self.progress)
    }
}

impl JobRunner {
    pub fn builder() -> JobRunnerBuilder {
        JobRunnerBuilder {
            workers: default_workers(),
            queue_cap: 64,
            progress: None,
        }
    }

    pub fn new(workers: usize, queue_cap: usize, progress: Option<ProgressSink>) -> Self {
        let workers = workers.max(1);
        let queue_cap = queue_cap.max(1);
        let (job_tx, job_rx) =
            std::sync::mpsc::sync_channel::<WorkerMsg>(workers.saturating_mul(4).max(4));
        let state = Arc::new(RunnerState {
            queue: Mutex::new(JobQueue {
                jobs: VecDeque::new(),
            }),
            queue_cap,
            permits: Mutex::new(0),
            waiters: Condvar::new(),
            progress: progress.clone(),
            shutdown: Mutex::new(false),
        });
        let job_rx = Arc::new(Mutex::new(job_rx));
        let mut handles = Vec::with_capacity(workers);
        for _ in 0..workers {
            let state = Arc::clone(&state);
            let job_tx = job_tx.clone();
            let job_rx_worker = Arc::clone(&job_rx);
            handles.push(thread::spawn(move || {
                worker_loop(state, job_tx, job_rx_worker)
            }));
        }
        Self {
            state,
            workers: handles,
            job_tx,
            job_rx,
        }
    }

    pub fn submit<T, F>(&self, priority: Priority, work: F) -> JobHandle<T>
    where
        T: Send + 'static,
        F: FnOnce(JobCtx) -> Result<T> + Send + 'static,
    {
        let id = NEXT_JOB_ID.fetch_add(1, AtomicOrdering::Relaxed);
        let token = CancelToken::new();
        let throttle = Arc::new(SharedThrottle::new());
        let sink = self.state.progress.clone();
        let job: JobBox = Arc::new(JobRecord {
            id,
            priority,
            token: token.clone(),
            sink: sink.clone(),
            throttle,
            run: Mutex::new(Some(Box::new(move |ctx| {
                let out = work(ctx)?;
                Ok(Box::new(out) as Box<dyn Any + Send>)
            }))),
            result: Mutex::new(None),
        });
        let _ = self.state.enqueue(Arc::clone(&job));
        if job.token.is_cancelled() {
            let _ = self.job_tx.try_send(WorkerMsg::Job);
        } else if !self.state.is_shutdown() {
            let _ = self.job_tx.send(WorkerMsg::Job);
        }
        JobHandle {
            job,
            cancel_token: token,
            _marker: std::marker::PhantomData,
        }
    }
}

impl Drop for JobRunner {
    fn drop(&mut self) {
        if let Ok(mut shutdown) = self.state.shutdown.lock() {
            *shutdown = true;
        }
        self.state.waiters.notify_all();
        for _ in &self.workers {
            let _ = self.job_tx.send(WorkerMsg::Shutdown);
        }
        while let Some(handle) = self.workers.pop() {
            let _ = handle.join();
        }
    }
}

fn default_workers() -> usize {
    thread::available_parallelism()
        .map(|n| n.get().min(4))
        .unwrap_or(1)
}

fn worker_loop(
    state: Arc<RunnerState>,
    job_tx: SyncSender<WorkerMsg>,
    job_rx: Arc<Mutex<Receiver<WorkerMsg>>>,
) {
    loop {
        let msg = {
            let rx = match job_rx.lock() {
                Ok(g) => g,
                Err(_) => break,
            };
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(msg) => msg,
                Err(RecvTimeoutError::Timeout) => {
                    if state.is_shutdown() {
                        break;
                    }
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
        };
        match msg {
            WorkerMsg::Shutdown => break,
            WorkerMsg::Job => {
                let Some(job) = state.take_job() else {
                    continue;
                };
                run_job(&state, &job_tx, job);
            }
        }
    }
}

fn run_job(state: &RunnerState, job_tx: &SyncSender<WorkerMsg>, job: JobBox) {
    if job.token.is_cancelled() {
        job.set_result(JobOutcome::Err(Error::Cancelled));
        state.release_permit();
        return;
    }
    let run = job.run.lock().ok().and_then(|mut slot| slot.take());
    let Some(run) = run else {
        job.set_result(JobOutcome::Err(Error::Backend(
            "job already ran".to_owned(),
        )));
        state.release_permit();
        return;
    };
    let ctx = JobCtx {
        token: job.token.clone(),
        sink: job.sink.clone(),
        throttle: Arc::clone(&job.throttle),
    };
    let caught = catch_unwind(AssertUnwindSafe(|| run(ctx)));
    let outcome = match caught {
        Ok(Ok(value)) => JobOutcome::Value(value),
        Ok(Err(err)) => JobOutcome::Err(err),
        Err(payload) => {
            let detail = panic_message(payload);
            JobOutcome::Err(Error::Backend(detail))
        }
    };
    job.set_result(outcome);
    state.release_permit();
    let _ = job_tx.try_send(WorkerMsg::Job);
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        return (*s).to_owned();
    }
    if let Some(s) = payload.downcast_ref::<String>() {
        return s.clone();
    }
    "job panicked".to_owned()
}

pub struct JobHandle<T> {
    job: JobBox,
    cancel_token: CancelToken,
    _marker: std::marker::PhantomData<T>,
}

impl<T> JobHandle<T>
where
    T: Send + 'static,
{
    pub fn id(&self) -> u64 {
        self.job.id
    }

    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }

    pub fn try_result(&self) -> Option<Result<T>>
    where
        T: Clone,
    {
        let guard = self.job.result.lock().ok()?;
        let outcome = guard.as_ref()?;
        match outcome {
            JobOutcome::Err(err) => Some(Err(crate::error::copy_error(err))),
            JobOutcome::Value(boxed) => boxed
                .downcast_ref::<T>()
                .map(|value| Ok(value.clone()))
                .or(Some(Err(Error::Backend(
                    "job result type mismatch".to_owned(),
                )))),
        }
    }

    pub fn wait(self) -> Result<T> {
        loop {
            if let Ok(mut guard) = self.job.result.lock()
                && let Some(outcome) = guard.take()
            {
                return extract_result_owned(outcome);
            }
            thread::sleep(Duration::from_millis(1));
        }
    }
}

fn extract_result_owned<T: Send + 'static>(outcome: JobOutcome) -> Result<T> {
    match outcome {
        JobOutcome::Err(err) => Err(crate::error::copy_error(&err)),
        JobOutcome::Value(boxed) => boxed
            .downcast::<T>()
            .map(|value| *value)
            .map_err(|_| Error::Backend("job result type mismatch".to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn cancel_before_start() {
        let runner = JobRunner::builder().workers(1).build();
        let handle = runner.submit(Priority::Normal, |_ctx| Ok(7u32));
        handle.cancel();
        let out = handle.wait();
        assert!(matches!(out, Err(Error::Cancelled)));
    }

    #[test]
    fn cancel_during_run() {
        let runner = JobRunner::builder().workers(1).build();
        let handle = runner.submit(Priority::Normal, |ctx| {
            for _ in 0..50 {
                ctx.cancel_token().check()?;
                thread::sleep(Duration::from_millis(5));
            }
            Ok(1u32)
        });
        thread::sleep(Duration::from_millis(20));
        handle.cancel();
        let out = handle.wait();
        assert!(matches!(out, Err(Error::Cancelled)));
    }

    #[test]
    fn priority_order_single_worker() {
        let runner = JobRunner::builder().workers(1).queue_capacity(8).build();
        let order = Arc::new(Mutex::new(Vec::<&'static str>::new()));
        let release = Arc::new(Barrier::new(2));
        let release_hold = Arc::clone(&release);
        let _hold = runner.submit(Priority::Normal, move |_| {
            release_hold.wait();
            thread::sleep(Duration::from_millis(30));
            Ok(())
        });
        thread::sleep(Duration::from_millis(20));
        let order_low = Arc::clone(&order);
        let low = runner.submit(Priority::Low, move |_| {
            order_low.lock().expect("lock").push("low");
            Ok(())
        });
        let order_high = Arc::clone(&order);
        let high = runner.submit(Priority::High, move |_| {
            order_high.lock().expect("lock").push("high");
            Ok(())
        });
        release.wait();
        assert!(_hold.wait().is_ok());
        assert!(high.wait().is_ok());
        assert!(low.wait().is_ok());
        let finished = order.lock().expect("lock");
        assert_eq!(&finished[..], &["high", "low"]);
    }

    #[test]
    fn progress_throttling_in_jobs() {
        let hits = Arc::new(AtomicUsize::new(0));
        let sink: ProgressSink = {
            let hits = Arc::clone(&hits);
            Arc::new(move |_| {
                hits.fetch_add(1, Ordering::Relaxed);
            })
        };
        let runner = JobRunner::builder().workers(1).progress_sink(sink).build();
        let handle = runner.submit(Priority::Normal, |ctx| {
            for i in 0..20u64 {
                ctx.report(i, 20, "work");
                thread::sleep(Duration::from_millis(1));
            }
            Ok(())
        });
        assert!(handle.wait().is_ok());
        assert!(hits.load(Ordering::Relaxed) < 10);
    }

    #[test]
    fn panic_becomes_backend_error() {
        let runner = JobRunner::builder().workers(1).build();
        let handle = runner.submit(Priority::Normal, |_ctx| -> Result<()> {
            panic!("boom");
        });
        let out = handle.wait();
        assert!(matches!(out, Err(Error::Backend(_))));
    }

    #[test]
    fn shutdown_joins_workers() {
        let runner = JobRunner::builder().workers(2).build();
        let _handle = runner.submit(Priority::Normal, |_| {
            thread::sleep(Duration::from_millis(80));
            Ok(())
        });
        drop(runner);
    }
}
