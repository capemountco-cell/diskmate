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

    /// Clear the flag so the token can be used for a new run.
    ///
    /// Only correct between runs: resetting while workers are still checking
    /// it would un-cancel work that is already stopping. The one caller is the
    /// command layer, which holds a single token across successive searches.
    pub fn reset(&self) {
        self.flag.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reset_token_can_be_used_again() {
        let token = CancellationToken::new();
        token.cancel();
        assert!(token.is_cancelled());
        token.reset();
        assert!(!token.is_cancelled());
    }

    #[test]
    fn a_clone_observes_cancellation() {
        let token = CancellationToken::new();
        let clone = token.clone();
        assert!(!clone.is_cancelled());
        token.cancel();
        assert!(clone.is_cancelled());
    }
}
