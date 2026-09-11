//! macOS system probe.
//!
//! Everything here is read-only and uses documented, non-private interfaces:
//! `getmntinfo` and `statfs` for volumes, `sysctl` and `host_statistics64` for
//! memory, and ordinary file reads for launch configuration. DiskMate does not
//! use private frameworks, does not try to work around TCC, and degrades to
//! partial information when macOS withholds access.

mod memory;
mod startup;
mod volumes;

use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::model::{MemorySnapshot, StartupItem, VolumeInfo};

use super::SystemProbe;

#[derive(Debug, Default)]
pub struct MacOsProbe;

impl SystemProbe for MacOsProbe {
    fn name(&self) -> &'static str {
        "macos"
    }

    fn volumes(&self) -> Result<Vec<VolumeInfo>> {
        volumes::volumes()
    }

    fn memory(&self) -> Result<MemorySnapshot> {
        memory::snapshot()
    }

    fn startup_items(&self) -> Result<Vec<StartupItem>> {
        startup::items()
    }

    fn skip_mount_points(&self, root: &Path) -> Vec<PathBuf> {
        volumes::skip_mount_points(root)
    }
}
