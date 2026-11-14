//! Application state shared by every command handler.

use std::sync::Arc;

use diskmate_core::paths::AppPaths;
use diskmate_core::scanner::CancellationToken;
use diskmate_core::services::{
    IconService, MemoryService, MonitorService, ProcessService, StartupService, StorageService,
};

pub struct AppState {
    pub storage: Arc<StorageService>,
    pub memory: MemoryService,
    pub startup: StartupService,
    pub monitor: MonitorService,
    pub process: ProcessService,
    pub icons: IconService,
    /// Cancels a running duplicate search. Held here rather than inside the
    /// service so the command layer can reach it while the search owns a
    /// thread.
    pub duplicate_cancel: CancellationToken,
}

impl AppState {
    pub fn initialise() -> Result<Self, Box<dyn std::error::Error>> {
        // Portable mode is decided here, once, from the running executable's
        // location. Nothing downstream hard-codes a data directory.
        let paths = AppPaths::discover()?;
        Ok(Self {
            storage: StorageService::new(paths)?,
            memory: MemoryService::new(),
            startup: StartupService::new(),
            monitor: MonitorService::new(),
            process: ProcessService::new(),
            icons: IconService::new(),
            duplicate_cancel: CancellationToken::new(),
        })
    }
}
