use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::{Error, Result};

#[derive(Clone)]
pub struct CancelToken {
    local: Arc<AtomicBool>,
    parent: Option<Arc<CancelToken>>,
}

impl CancelToken {
    pub fn new() -> Self {
        Self {
            local: Arc::new(AtomicBool::new(false)),
            parent: None,
        }
    }

    pub fn child(&self) -> Self {
        Self {
            local: Arc::new(AtomicBool::new(false)),
            parent: Some(Arc::new(self.clone())),
        }
    }

    pub fn cancel(&self) {
        self.local.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.local.load(Ordering::Relaxed)
            || self
                .parent
                .as_ref()
                .is_some_and(|parent| parent.is_cancelled())
    }

    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            return Err(Error::Cancelled);
        }
        Ok(())
    }
}

impl Default for CancelToken {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_cancel_propagates_to_child() {
        let parent = CancelToken::new();
        let child = parent.child();
        parent.cancel();
        assert!(child.is_cancelled());
    }
}
