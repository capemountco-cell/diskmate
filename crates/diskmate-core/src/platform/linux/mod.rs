//! Linux system probe.
//!
//! DiskMate targets macOS first, but the engine is deliberately portable and
//! this implementation is what lets the scanner, index and services be built
//! and tested on Linux CI without stubbing the platform layer out.

mod memory;
mod mounts;
mod startup;

use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::model::{MemorySnapshot, StartupItem, VolumeInfo};

use super::SystemProbe;

#[derive(Debug, Default)]
pub struct LinuxProbe;

impl SystemProbe for LinuxProbe {
    fn name(&self) -> &'static str {
        "linux"
    }

    fn volumes(&self) -> Result<Vec<VolumeInfo>> {
        mounts::volumes()
    }

    fn memory(&self) -> Result<MemorySnapshot> {
        memory::snapshot()
    }

    fn startup_items(&self) -> Result<Vec<StartupItem>> {
        startup::items()
    }

    fn skip_mount_points(&self, root: &Path) -> Vec<PathBuf> {
        mounts::skip_mount_points(root)
    }
}
