use serde::{Deserialize, Serialize};

/// How hard the operating system is working to satisfy memory demand.
///
/// This mirrors the signal macOS itself publishes. DiskMate reports it rather
/// than inventing a "memory health score", because free RAM is not a goal:
/// a healthy Mac uses spare memory for caching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MemoryPressure {
    Normal,
    Warning,
    Critical,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySnapshot {
    pub total_bytes: i64,
    /// Memory that is genuinely committed: anonymous, wired and compressed.
    pub used_bytes: i64,
    /// Memory the OS is holding for caches and can reclaim on demand. Not a
    /// problem, and never presented as one.
    pub cached_bytes: i64,
    pub free_bytes: i64,
    pub wired_bytes: Option<i64>,
    pub compressed_bytes: Option<i64>,
    pub swap_used_bytes: Option<i64>,
    pub swap_total_bytes: Option<i64>,
    pub pressure: MemoryPressure,
    pub sampled_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessMemory {
    pub pid: i32,
    pub name: String,
    pub resident_bytes: i64,
    /// Number of processes folded into this row when grouping by application.
    pub instance_count: u32,
}
