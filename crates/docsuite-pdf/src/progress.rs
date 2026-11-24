use docsuite_core::{
    CancelToken, Progress, ProgressSink, Result, ThrottleState, throttle_progress,
};

pub(crate) fn report(
    sink: &Option<ProgressSink>,
    state: &mut ThrottleState,
    done: u64,
    total: u64,
    stage: &str,
) {
    throttle_progress(sink, state, Progress::new(done, total, stage));
}

pub(crate) fn step(
    cancel: &CancelToken,
    sink: &Option<ProgressSink>,
    state: &mut ThrottleState,
    index: u64,
    total: u64,
    stage: &str,
) -> Result<()> {
    cancel.check()?;
    report(sink, state, index + 1, total, stage);
    Ok(())
}
