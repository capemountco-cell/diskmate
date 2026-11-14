//! macOS system probe.
//!
//! Everything here except `processes::request_quit` is read-only, and all of
//! it uses documented, non-private interfaces:
//! `getmntinfo` and `statfs` for volumes, `sysctl` and `host_statistics64` for
//! memory, and ordinary file reads for launch configuration. DiskMate does not
//! use private frameworks, does not try to work around TCC, and degrades to
//! partial information when macOS withholds access.

mod icons;
mod memory;
mod processes;
mod startup;
mod volumes;

use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::model::{MemorySnapshot, ProcessMemory, StartupItem, VolumeInfo};

use super::SystemProbe;

pub(crate) use processes::request_quit;

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

    fn top_apps(&self, limit: usize) -> Result<Vec<ProcessMemory>> {
        processes::top_apps(limit)
    }

    fn icon_png(&self, path: &Path, size: u32) -> Option<Vec<u8>> {
        icons::icon_png(path, size)
    }

    fn startup_items(&self) -> Result<Vec<StartupItem>> {
        startup::items()
    }

    fn skip_mount_points(&self, root: &Path) -> Vec<PathBuf> {
        volumes::skip_mount_points(root)
    }
}
