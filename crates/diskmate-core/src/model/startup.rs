use serde::{Deserialize, Serialize};

/// Where a startup item was found. DiskMate reads these locations; it never
/// writes to them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StartupSource {
    LaunchAgent,
    LaunchDaemon,
    LoginItem,
    /// Non-macOS equivalents, kept so the model is not macOS-shaped.
    UserService,
    SystemService,
    Autostart,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StartupScope {
    /// Runs for the signed-in user only.
    User,
    /// Runs for every user on the machine.
    System,
    /// Shipped by the operating system vendor.
    Os,
}

/// Startup impact.
///
/// DiskMate will not guess. Until it can measure launch cost defensibly, every
/// item reports [`StartupImpact::Unmeasured`]; there is deliberately no code
/// path that fabricates a "saves 4.2 seconds" claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StartupImpact {
    Unmeasured,
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupItem {
    /// Reverse-DNS label where the platform provides one, otherwise the file
    /// name.
    pub id: String,
    pub name: String,
    pub source: StartupSource,
    pub scope: StartupScope,
    pub config_path: String,
    /// Executable the item launches, when it can be determined without running
    /// anything.
    pub program: Option<String>,
    /// Whether the item is configured to run automatically. `None` when the
    /// configuration could not be read.
    pub runs_at_load: Option<bool>,
    pub disabled: Option<bool>,
    pub impact: StartupImpact,
    /// True when the item ships with the operating system. DiskMate never
    /// suggests touching these.
    pub is_os_provided: bool,
}
