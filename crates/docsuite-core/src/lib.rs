mod cancel;
mod error;
mod job;
mod progress;

pub use cancel::CancelToken;
pub use error::{Error, Result};
pub use job::{JobCtx, JobHandle, JobRunner, Priority};
pub use progress::{Progress, ProgressSink, throttle_progress};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn version_cstr() -> &'static [u8] {
    static BYTES: &[u8] = concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes();
    BYTES
}
