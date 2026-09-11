//! Progress counters and event throttling.
//!
//! Workers bump lock-free counters; a sampler turns them into events at a fixed
//! cadence. The UI therefore sees a smooth ten-or-so updates a second no matter
//! whether the scanner is grinding through one huge file or a million tiny
//! ones, and the IPC channel never becomes the bottleneck.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Shared, monotonically increasing scan counters.
#[derive(Debug, Default)]
pub struct ProgressCounters {
    pub files: AtomicU64,
    pub directories: AtomicU64,
    pub symlinks: AtomicU64,
    pub others: AtomicU64,
    pub bytes: AtomicU64,
    pub errors: AtomicU64,
    /// Sampled, not authoritative: updated once per directory so the UI has
    /// something to show without paying for a lock per file.
    current_path: Mutex<Option<String>>,
}

impl ProgressCounters {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn entries(&self) -> u64 {
        self.files.load(Ordering::Relaxed)
            + self.directories.load(Ordering::Relaxed)
            + self.symlinks.load(Ordering::Relaxed)
            + self.others.load(Ordering::Relaxed)
    }

    pub fn set_current_path(&self, path: &str) {
        if let Ok(mut slot) = self.current_path.lock() {
            match slot.as_mut() {
                // Reuse the allocation; this runs once per directory.
                Some(existing) => {
                    existing.clear();
                    existing.push_str(path);
                }
                None => *slot = Some(path.to_owned()),
            }
        }
    }

    pub fn current_path(&self) -> Option<String> {
        self.current_path.lock().ok().and_then(|slot| slot.clone())
    }
}

/// Rate limiter for progress events.
pub struct ProgressThrottle {
    interval: std::time::Duration,
    last_emit: std::time::Instant,
}

impl ProgressThrottle {
    pub fn new(interval_ms: u64) -> Self {
        // Never emit faster than ~60 Hz even if a caller asks for 0.
        let interval = std::time::Duration::from_millis(interval_ms.max(16));
        Self {
            interval,
            last_emit: std::time::Instant::now() - interval,
        }
    }

    /// `true` when enough time has passed to emit another event.
    pub fn should_emit(&mut self, now: std::time::Instant) -> bool {
        if now.duration_since(self.last_emit) >= self.interval {
            self.last_emit = now;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn throttle_suppresses_events_inside_the_interval() {
        let mut throttle = ProgressThrottle::new(100);
        let start = Instant::now();
        assert!(throttle.should_emit(start));
        assert!(!throttle.should_emit(start + Duration::from_millis(50)));
        assert!(throttle.should_emit(start + Duration::from_millis(150)));
    }

    #[test]
    fn counters_sum_every_entry_kind() {
        let counters = ProgressCounters::new();
        counters.files.fetch_add(3, Ordering::Relaxed);
        counters.directories.fetch_add(2, Ordering::Relaxed);
        counters.symlinks.fetch_add(1, Ordering::Relaxed);
        assert_eq!(counters.entries(), 6);
    }
}
