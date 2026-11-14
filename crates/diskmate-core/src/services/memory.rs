//! The Memory service.
//!
//! Explaining memory correctly matters more than acting on it, so this service
//! only reads. There is no "purge the cache" operation: macOS hands cached
//! memory to any app that asks for it, and emptying the cache makes a Mac
//! briefly slower, not faster.
//!
//! What does free memory is quitting the app that is using it. That lives in
//! `ProcessService`, behind an explicit click on that one app.

use crate::error::Result;
use crate::model::{MemoryPressure, MemorySnapshot};
use crate::platform::{self, SystemProbe};

/// A memory reading together with the sentence DiskMate would say about it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryReport {
    pub snapshot: MemorySnapshot,
    pub headline: String,
    pub detail: String,
}

pub struct MemoryService {
    probe: &'static dyn SystemProbe,
}

impl Default for MemoryService {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryService {
    pub fn new() -> Self {
        Self {
            probe: platform::probe(),
        }
    }

    pub fn snapshot(&self) -> Result<MemorySnapshot> {
        self.probe.memory()
    }

    pub fn report(&self) -> Result<MemoryReport> {
        let snapshot = self.probe.memory()?;
        let (headline, detail) = describe(&snapshot);
        Ok(MemoryReport {
            snapshot,
            headline,
            detail,
        })
    }
}

/// Turn a snapshot into language a person can act on, or not act on.
pub(crate) fn describe(snapshot: &MemorySnapshot) -> (String, String) {
    match snapshot.pressure {
        MemoryPressure::Normal => (
            "Your memory is healthy".to_string(),
            "Your Mac is using spare memory for caching, which makes things faster. \
             No action is necessary."
                .to_string(),
        ),
        MemoryPressure::Warning => (
            "Memory is under pressure".to_string(),
            "Your Mac is compressing memory and may start using swap. Closing apps you \
             are not using will help."
                .to_string(),
        ),
        MemoryPressure::Critical => (
            "Memory pressure is high".to_string(),
            "Your Mac is swapping to disk, which slows everything down. Quitting the \
             largest apps will help most."
                .to_string(),
        ),
        MemoryPressure::Unknown => (
            "Memory status unavailable".to_string(),
            "DiskMate could not read the memory pressure level on this system.".to_string(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(pressure: MemoryPressure) -> MemorySnapshot {
        MemorySnapshot {
            total_bytes: 16 << 30,
            used_bytes: 8 << 30,
            cached_bytes: 6 << 30,
            free_bytes: 2 << 30,
            wired_bytes: None,
            compressed_bytes: None,
            swap_used_bytes: None,
            swap_total_bytes: None,
            pressure,
            sampled_at: 0,
        }
    }

    #[test]
    fn normal_pressure_recommends_no_action() {
        let (headline, detail) = describe(&snapshot(MemoryPressure::Normal));
        assert!(headline.contains("healthy"));
        assert!(detail.contains("No action is necessary"));
    }

    #[test]
    fn no_wording_ever_offers_to_free_memory() {
        for pressure in [
            MemoryPressure::Normal,
            MemoryPressure::Warning,
            MemoryPressure::Critical,
            MemoryPressure::Unknown,
        ] {
            let (headline, detail) = describe(&snapshot(pressure));
            let text = format!("{headline} {detail}").to_lowercase();
            for forbidden in [
                "free ram",
                "free up memory",
                "purge",
                "optimize",
                "boost",
                "clean",
            ] {
                assert!(!text.contains(forbidden), "{pressure:?} says {forbidden:?}");
            }
        }
    }
}
