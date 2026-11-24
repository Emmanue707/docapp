use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct Progress {
    pub done: u64,
    pub total: u64,
    pub stage: String,
}

impl Progress {
    pub fn new(done: u64, total: u64, stage: impl Into<String>) -> Self {
        Self {
            done,
            total,
            stage: stage.into(),
        }
    }

    pub fn stage_static(done: u64, total: u64, stage: &'static str) -> Self {
        Self {
            done,
            total,
            stage: stage.to_owned(),
        }
    }
}

pub type ProgressSink = Arc<dyn Fn(Progress) + Send + Sync>;

const THROTTLE: Duration = Duration::from_millis(16);

pub fn throttle_progress(sink: &Option<ProgressSink>, state: &mut ThrottleState, update: Progress) {
    let done = update.done;
    let total = update.total;
    let stage = update.stage;
    let complete = total > 0 && done >= total;
    let now = Instant::now();
    let stage_changed = state.last_stage.as_deref() != Some(stage.as_str());
    let elapsed = now.duration_since(state.last_emit);
    if complete || stage_changed || elapsed >= THROTTLE {
        state.last_emit = now;
        state.last_stage = Some(stage.clone());
        if let Some(sink) = sink {
            sink(Progress { done, total, stage });
        }
    }
}

pub struct ThrottleState {
    last_emit: Instant,
    last_stage: Option<String>,
}

impl ThrottleState {
    pub fn new() -> Self {
        Self {
            last_emit: Instant::now() - THROTTLE,
            last_stage: None,
        }
    }
}

pub(crate) struct SharedThrottle {
    inner: Mutex<ThrottleState>,
}

impl SharedThrottle {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(ThrottleState::new()),
        }
    }

    pub fn report(&self, sink: &Option<ProgressSink>, update: Progress) {
        let Ok(mut state) = self.inner.lock() else {
            return;
        };
        throttle_progress(sink, &mut state, update);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn throttles_rapid_updates() {
        let count = Arc::new(AtomicUsize::new(0));
        let sink: ProgressSink = {
            let count = Arc::clone(&count);
            Arc::new(move |_| {
                count.fetch_add(1, Ordering::Relaxed);
            })
        };
        let sink = Some(sink);
        let mut state = ThrottleState::new();
        for i in 0..5u64 {
            throttle_progress(&sink, &mut state, Progress::new(i, 10, "stage"));
        }
        assert_eq!(count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn stage_change_emits() {
        let count = Arc::new(AtomicUsize::new(0));
        let sink: ProgressSink = {
            let count = Arc::clone(&count);
            Arc::new(move |_| {
                count.fetch_add(1, Ordering::Relaxed);
            })
        };
        let sink = Some(sink);
        let mut state = ThrottleState::new();
        throttle_progress(&sink, &mut state, Progress::new(0, 10, "a"));
        throttle_progress(&sink, &mut state, Progress::new(1, 10, "b"));
        assert_eq!(count.load(Ordering::Relaxed), 2);
    }
}
