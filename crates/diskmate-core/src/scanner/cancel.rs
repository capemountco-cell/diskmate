//! Cancellation.
//!
//! A scan of a full volume can run for minutes, so cancellation has to be both
//! prompt and clean. The token is checked between directories and between
//! entries within a directory, which bounds the response time to one `lstat`
//! rather than one directory.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Cheap, cloneable cancellation flag shared by every worker and the writer.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    flag: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        // Release so that anything the canceller wrote before this is visible
        // to a worker that observes the flag.
        self.flag.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clone_observes_cancellation() {
        let token = CancellationToken::new();
        let clone = token.clone();
        assert!(!clone.is_cancelled());
        token.cancel();
        assert!(clone.is_cancelled());
    }
}
