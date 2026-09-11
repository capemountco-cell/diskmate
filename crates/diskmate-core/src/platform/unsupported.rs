//! Fallback probe for platforms DiskMate has not been ported to.
//!
//! It reports its limitations rather than guessing, so a future Windows or BSD
//! build starts from an honest baseline instead of fabricated numbers.

use std::path::{Path, PathBuf};

use crate::error::{CoreError, Result};
use crate::model::{MemorySnapshot, StartupItem, VolumeInfo};

use super::SystemProbe;

/// Constructed only on platforms without a dedicated probe, so it is dead code
/// on macOS and Linux builds by design.
#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct UnsupportedProbe;

#[allow(dead_code)]
impl SystemProbe for UnsupportedProbe {
    fn name(&self) -> &'static str {
        "unsupported"
    }

    fn volumes(&self) -> Result<Vec<VolumeInfo>> {
        Err(CoreError::Unsupported {
            feature: "volume discovery",
        })
    }

    fn memory(&self) -> Result<MemorySnapshot> {
        Err(CoreError::Unsupported {
            feature: "memory inspection",
        })
    }

    fn startup_items(&self) -> Result<Vec<StartupItem>> {
        Err(CoreError::Unsupported {
            feature: "startup inspection",
        })
    }

    fn skip_mount_points(&self, _root: &Path) -> Vec<PathBuf> {
        Vec::new()
    }
}
