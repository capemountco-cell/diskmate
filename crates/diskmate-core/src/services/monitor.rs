//! The live monitor.
//!
//! A scanner answers "what is on my disk right now". The monitor answers "what
//! is happening", which is a different question and the one people have when a
//! machine feels slow rather than full.
//!
//! It samples on a timer and keeps a bounded rolling history in memory. Bounded
//! matters: a monitor left running for a week must not itself become the
//! memory problem it is reporting on.
//!
//! Strictly read-only, and deliberately so. There is no operation here that
//! frees memory, ends a process, or writes anything. On modern macOS a
//! "free RAM" button makes a Mac slower until its caches refill, and DiskMate
//! explains memory rather than pretending to fix it.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::model::{MemoryPressure, MemorySnapshot};
use crate::platform::{self, SystemProbe};

/// Samples kept. At the default one-second cadence this is fifteen minutes of
/// history, which is enough to see a trend and small enough to ignore: roughly
/// 900 samples of a few dozen bytes each.
const HISTORY_CAPACITY: usize = 900;
const DEFAULT_INTERVAL_MS: u64 = 1_000;

/// One moment in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    pub at: i64,
    pub memory_used_bytes: i64,
    pub memory_cached_bytes: i64,
    pub memory_total_bytes: i64,
    pub swap_used_bytes: Option<i64>,
    pub pressure: MemoryPressure,
    /// Free space on the volume being watched, when one is.
    pub disk_available_bytes: Option<i64>,
}

/// What the monitor has seen, and what it means.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorReport {
    pub running: bool,
    pub interval_ms: u64,
    pub samples: Vec<Sample>,
    pub latest: Option<Sample>,
    /// Highest pressure seen across the retained history, so a spike that has
    /// already passed is not silently lost between glances at the screen.
    pub peak_pressure: MemoryPressure,
    pub headline: String,
    pub detail: String,
}

pub struct MonitorService {
    probe: &'static dyn SystemProbe,
    state: Arc<MonitorState>,
}

struct MonitorState {
    running: AtomicBool,
    interval_ms: Mutex<u64>,
    history: Mutex<VecDeque<Sample>>,
    watch_path: Mutex<Option<std::path::PathBuf>>,
}

impl Default for MonitorService {
    fn default() -> Self {
        Self::new()
    }
}

impl MonitorService {
    pub fn new() -> Self {
        Self {
            probe: platform::probe(),
            state: Arc::new(MonitorState {
                running: AtomicBool::new(false),
                interval_ms: Mutex::new(DEFAULT_INTERVAL_MS),
                history: Mutex::new(VecDeque::with_capacity(HISTORY_CAPACITY)),
                watch_path: Mutex::new(None),
            }),
        }
    }

    /// Take one reading without starting anything.
    pub fn sample_once(&self) -> Result<Sample> {
        let memory = self.probe.memory()?;
        Ok(self.build_sample(&memory))
    }

    fn build_sample(&self, memory: &MemorySnapshot) -> Sample {
        let disk_available_bytes = self
            .state
            .watch_path
            .lock()
            .ok()
            .and_then(|path| path.clone())
            .and_then(|path| {
                self.probe
                    .volumes()
                    .ok()?
                    .into_iter()
                    .find(|v| path.starts_with(&v.mount_point))
                    .and_then(|v| v.available_bytes)
            });
        Sample {
            at: crate::db::now_unix(),
            memory_used_bytes: memory.used_bytes,
            memory_cached_bytes: memory.cached_bytes,
            memory_total_bytes: memory.total_bytes,
            swap_used_bytes: memory.swap_used_bytes,
            pressure: memory.pressure,
            disk_available_bytes,
        }
    }

    /// Start sampling on a background thread. Idempotent: starting an already
    /// running monitor changes the interval and nothing else.
    pub fn start(&self, interval_ms: u64, watch: Option<std::path::PathBuf>) {
        *self.state.interval_ms.lock().expect("monitor poisoned") = interval_ms.clamp(250, 60_000);
        *self.state.watch_path.lock().expect("monitor poisoned") = watch;
        if self.state.running.swap(true, Ordering::SeqCst) {
            return;
        }

        let state = Arc::clone(&self.state);
        let probe = self.probe;
        // A plain thread rather than an async task: this sleeps almost all of
        // the time, and occupying an executor thread to do that would be waste.
        std::thread::Builder::new()
            .name("diskmate-monitor".into())
            .spawn(move || {
                while state.running.load(Ordering::SeqCst) {
                    if let Ok(memory) = probe.memory() {
                        let watch = state.watch_path.lock().ok().and_then(|p| p.clone());
                        let disk_available_bytes = watch.and_then(|path| {
                            probe
                                .volumes()
                                .ok()?
                                .into_iter()
                                .find(|v| path.starts_with(&v.mount_point))
                                .and_then(|v| v.available_bytes)
                        });
                        let sample = Sample {
                            at: crate::db::now_unix(),
                            memory_used_bytes: memory.used_bytes,
                            memory_cached_bytes: memory.cached_bytes,
                            memory_total_bytes: memory.total_bytes,
                            swap_used_bytes: memory.swap_used_bytes,
                            pressure: memory.pressure,
                            disk_available_bytes,
                        };
                        if let Ok(mut history) = state.history.lock() {
                            if history.len() == HISTORY_CAPACITY {
                                history.pop_front();
                            }
                            history.push_back(sample);
                        }
                    }
                    let interval = *state.interval_ms.lock().expect("monitor poisoned");
                    std::thread::sleep(Duration::from_millis(interval));
                }
            })
            .ok();
    }

    pub fn stop(&self) {
        self.state.running.store(false, Ordering::SeqCst);
    }

    pub fn is_running(&self) -> bool {
        self.state.running.load(Ordering::SeqCst)
    }

    pub fn clear(&self) {
        if let Ok(mut history) = self.state.history.lock() {
            history.clear();
        }
    }

    /// The history, plus what it means in words.
    pub fn report(&self) -> MonitorReport {
        let samples: Vec<Sample> = self
            .state
            .history
            .lock()
            .map(|history| history.iter().cloned().collect())
            .unwrap_or_default();
        let latest = samples.last().cloned();
        let peak_pressure = peak_pressure(&samples);
        let (headline, detail) = describe(&samples, peak_pressure);
        MonitorReport {
            running: self.is_running(),
            interval_ms: *self.state.interval_ms.lock().expect("monitor poisoned"),
            latest,
            peak_pressure,
            headline,
            detail,
            samples,
        }
    }
}

fn peak_pressure(samples: &[Sample]) -> MemoryPressure {
    let rank = |pressure: MemoryPressure| match pressure {
        MemoryPressure::Critical => 3,
        MemoryPressure::Warning => 2,
        MemoryPressure::Normal => 1,
        MemoryPressure::Unknown => 0,
    };
    samples
        .iter()
        .map(|s| s.pressure)
        .max_by_key(|p| rank(*p))
        .unwrap_or(MemoryPressure::Unknown)
}

/// What the history means, stated without drama.
pub(crate) fn describe(samples: &[Sample], peak: MemoryPressure) -> (String, String) {
    if samples.is_empty() {
        return (
            "Not watching yet".to_string(),
            "Start the monitor to see how memory and free space change over time.".to_string(),
        );
    }
    let latest = samples.last().expect("checked non-empty");
    let swapping = latest.swap_used_bytes.unwrap_or(0) > 0;

    match (latest.pressure, peak) {
        (MemoryPressure::Critical, _) => (
            "Memory pressure is high".to_string(),
            "Your Mac is swapping to disk, which slows everything down. Quitting the \
             largest apps will help most."
                .to_string(),
        ),
        (MemoryPressure::Warning, _) => (
            "Memory is under pressure".to_string(),
            "Your Mac is compressing memory to keep up. Closing apps you are not using \
             will ease it."
                .to_string(),
        ),
        (MemoryPressure::Normal, MemoryPressure::Critical | MemoryPressure::Warning) => (
            "Steady now, but it spiked".to_string(),
            "Memory is comfortable at the moment, though it came under pressure earlier \
             in this window. Something asked for a lot and gave it back."
                .to_string(),
        ),
        (MemoryPressure::Normal, _) if swapping => (
            "Steady, with some swap in use".to_string(),
            "Your Mac is keeping up. The swap already on disk is left over from earlier \
             and is not being added to."
                .to_string(),
        ),
        (MemoryPressure::Normal, _) => (
            "Everything is steady".to_string(),
            "Memory is comfortable and your Mac is using the spare for caching, which \
             is what makes things feel fast."
                .to_string(),
        ),
        (MemoryPressure::Unknown, _) => (
            "Memory status unavailable".to_string(),
            "DiskMate could not read the memory pressure level on this system.".to_string(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(pressure: MemoryPressure, swap: i64) -> Sample {
        Sample {
            at: 0,
            memory_used_bytes: 8 << 30,
            memory_cached_bytes: 4 << 30,
            memory_total_bytes: 16 << 30,
            swap_used_bytes: Some(swap),
            pressure,
            disk_available_bytes: None,
        }
    }

    #[test]
    fn an_empty_history_says_so_rather_than_guessing() {
        let (headline, _) = describe(&[], MemoryPressure::Unknown);
        assert!(headline.contains("Not watching"));
    }

    #[test]
    fn a_spike_that_already_passed_is_still_reported() {
        let samples = vec![
            sample(MemoryPressure::Critical, 0),
            sample(MemoryPressure::Normal, 0),
        ];
        let peak = peak_pressure(&samples);
        assert_eq!(peak, MemoryPressure::Critical);
        let (headline, detail) = describe(&samples, peak);
        assert!(headline.contains("spiked"));
        assert!(detail.contains("earlier"));
    }

    #[test]
    fn existing_swap_is_not_reported_as_a_problem_while_pressure_is_normal() {
        let samples = vec![sample(MemoryPressure::Normal, 2 << 30)];
        let (headline, detail) = describe(&samples, MemoryPressure::Normal);
        assert!(headline.contains("Steady"));
        assert!(detail.contains("left over"));
    }

    #[test]
    fn no_wording_ever_offers_to_free_or_optimise_memory() {
        for pressure in [
            MemoryPressure::Normal,
            MemoryPressure::Warning,
            MemoryPressure::Critical,
            MemoryPressure::Unknown,
        ] {
            let samples = vec![sample(pressure, 0)];
            let (headline, detail) = describe(&samples, pressure);
            let text = format!("{headline} {detail}").to_lowercase();
            for forbidden in [
                "free ram",
                "free up",
                "purge",
                "optimi",
                "boost",
                "speed up your",
            ] {
                assert!(!text.contains(forbidden), "{pressure:?} says {forbidden:?}");
            }
        }
    }

    #[test]
    fn history_is_bounded_so_a_long_watch_is_not_itself_a_leak() {
        let service = MonitorService::new();
        {
            let mut history = service.state.history.lock().unwrap();
            for _ in 0..(HISTORY_CAPACITY + 250) {
                if history.len() == HISTORY_CAPACITY {
                    history.pop_front();
                }
                history.push_back(sample(MemoryPressure::Normal, 0));
            }
        }
        assert_eq!(service.report().samples.len(), HISTORY_CAPACITY);
    }
}
