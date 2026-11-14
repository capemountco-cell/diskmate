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

/// Memory held by one application, with its helper processes folded in.
///
/// A browser is one main process and dozens of helpers living inside its own
/// bundle; listing them separately would bury the answer to "what is using my
/// memory". Everything whose executable sits inside the same `.app` is one row.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProcessMemory {
    /// The main process. This is the one a quit request is sent to.
    pub pid: i32,
    pub name: String,
    /// Physical memory footprint, summed across the app's processes. On macOS
    /// this is the figure Activity Monitor shows in its Memory column.
    pub resident_bytes: i64,
    /// Number of processes folded into this row.
    pub instance_count: u32,
    /// The `.app` bundle, when the process belongs to one.
    pub bundle_path: Option<String>,
    /// True only for ordinary apps with a Dock presence, which can be asked to
    /// quit the way the Quit menu item would. Background agents, daemons and
    /// command-line processes are listed but never offered a Quit.
    pub can_quit: bool,
}
