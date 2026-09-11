//! Application state shared by every command handler.

use std::sync::Arc;

use diskmate_core::paths::AppPaths;
use diskmate_core::services::{MemoryService, StartupService, StorageService};

pub struct AppState {
    pub storage: Arc<StorageService>,
    pub memory: MemoryService,
    pub startup: StartupService,
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
        })
    }
}
