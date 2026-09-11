use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VolumeKind {
    Internal,
    External,
    Network,
    Virtual,
    Unknown,
}

/// A mounted volume as reported by the platform layer.
///
/// Every capacity field is optional: network mounts, synthetic volumes and
/// read-only system snapshots do not all answer `statfs` the same way, and
/// reporting a confident `0` for an unknown value would be a lie.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeInfo {
    /// Stable identifier for the volume where the platform can supply one,
    /// otherwise the mount point. Used to correlate scans across sessions.
    pub id: String,
    pub name: String,
    pub mount_point: String,
    /// Device or remote share backing the mount, e.g. `/dev/disk3s5`.
    pub device: Option<String>,
    pub fs_type: String,
    pub kind: VolumeKind,
    pub total_bytes: Option<i64>,
    pub available_bytes: Option<i64>,
    /// `total - available`, or `None` when either side is unknown.
    pub used_bytes: Option<i64>,
    pub is_removable: bool,
    pub is_read_only: bool,
    /// False for volumes macOS hides from Finder (`MNT_DONTBROWSE`), such as
    /// the Preboot and VM helper volumes.
    pub is_browsable: bool,
    /// Whether DiskMate is able to read the mount point at all right now. A
    /// volume can be listed and still be unreadable without Full Disk Access.
    pub is_readable: bool,
}

impl VolumeInfo {
    /// Fill in `used_bytes` from the other two fields.
    pub fn with_derived_usage(mut self) -> Self {
        self.used_bytes = match (self.total_bytes, self.available_bytes) {
            (Some(total), Some(available)) => Some((total - available).max(0)),
            _ => None,
        };
        self
    }
}
