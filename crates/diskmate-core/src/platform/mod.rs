//! Platform abstraction for system inspection.
//!
//! Volume discovery, memory statistics and startup-item enumeration are the
//! three places where DiskMate has to talk to the operating system directly.
//! Each one sits behind [`SystemProbe`] so that the scanner, the index and the
//! services above them stay portable, and so that the macOS implementation can
//! be replaced wholesale without touching anything else.
//!
//! Every method of [`SystemProbe`] returns data or an explicit "not supported"
//! error and modifies nothing. The one action in this module is separate, in
//! [`AppQuitter`]: asking an ordinary app to quit, exactly as its Quit menu
//! item would. It is only ever called by `services::ProcessService`, after the
//! person clicked Quit on that app.

mod grouping;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
mod protected;
pub mod startup_locations;
mod unsupported;

pub(crate) use grouping::group_by_app;
pub use protected::{is_protected_path, is_uninstallable_app_bundle, protected_prefixes};

use crate::error::Result;
use crate::model::{MemorySnapshot, ProcessMemory, StartupItem, VolumeInfo};

/// Read-only view of the host operating system.
pub trait SystemProbe: Send + Sync {
    /// Short identifier for the implementation, recorded in benchmarks and
    /// shown in diagnostics.
    fn name(&self) -> &'static str;

    /// Mounted volumes with whatever metadata the platform can supply.
    fn volumes(&self) -> Result<Vec<VolumeInfo>>;

    /// A point-in-time memory snapshot.
    fn memory(&self) -> Result<MemorySnapshot>;

    /// Applications using the most memory, largest first, helpers folded into
    /// their app. Processes DiskMate is not allowed to inspect are left out
    /// rather than estimated.
    fn top_apps(&self, limit: usize) -> Result<Vec<ProcessMemory>>;

    /// Items configured to run automatically. Enumeration only; DiskMate never
    /// enables, disables or removes one.
    fn startup_items(&self) -> Result<Vec<StartupItem>>;

    /// Mount points a volume scan should not descend into: other volumes,
    /// network shares and synthetic filesystems.
    fn skip_mount_points(&self, root: &std::path::Path) -> Vec<std::path::PathBuf>;

    /// The icon Finder shows for `path`, `size` pixels square, as PNG bytes.
    /// `None` where the platform has no such thing or it cannot be read.
    fn icon_png(&self, _path: &std::path::Path, _size: u32) -> Option<Vec<u8>> {
        None
    }
}

/// The one process action DiskMate has: asking an app to quit.
///
/// A quit request is what the app's own Quit menu item sends, so the app can
/// save, ask about unsaved work, or decline. It is never a kill signal.
pub trait AppQuitter: Send + Sync {
    /// Ask the app whose main process is `pid` to quit. Returns once the
    /// request is delivered, not when the app has exited.
    fn request_quit(&self, pid: i32) -> std::result::Result<(), String>;

    fn name(&self) -> &'static str;
}

/// The real quitter on macOS; refuses everywhere else.
#[derive(Debug, Default)]
pub struct SystemQuitter;

impl AppQuitter for SystemQuitter {
    fn request_quit(&self, pid: i32) -> std::result::Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            macos::request_quit(pid)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = pid;
            Err("quitting apps is only supported on macOS".to_string())
        }
    }

    fn name(&self) -> &'static str {
        "system"
    }
}

#[cfg(target_os = "macos")]
type Active = macos::MacOsProbe;
#[cfg(target_os = "linux")]
type Active = linux::LinuxProbe;
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
type Active = unsupported::UnsupportedProbe;

/// The probe for the host this build runs on.
pub fn probe() -> &'static dyn SystemProbe {
    static PROBE: std::sync::OnceLock<Active> = std::sync::OnceLock::new();
    PROBE.get_or_init(Active::default)
}

/// Human-readable host description for diagnostics and benchmark records.
/// The per-user cache and temporary folders macOS keeps outside the home
/// folder (under /private/var/folders). Apps leave files there in folders
/// named after their bundle identifier. Empty on other systems.
pub fn per_user_dirs() -> Vec<std::path::PathBuf> {
    #[cfg(target_os = "macos")]
    {
        // From <unistd.h>: _CS_DARWIN_USER_TEMP_DIR and _CS_DARWIN_USER_CACHE_DIR.
        const TEMP: libc::c_int = 65537;
        const CACHE: libc::c_int = 65538;
        [TEMP, CACHE]
            .into_iter()
            .filter_map(|name| {
                let mut buffer = vec![0u8; 1024];
                // SAFETY: confstr writes at most `buffer.len()` bytes, including
                // the terminating NUL, and returns the length it needed.
                let needed =
                    unsafe { libc::confstr(name, buffer.as_mut_ptr().cast(), buffer.len()) };
                if needed == 0 || needed > buffer.len() {
                    return None;
                }
                let text = std::ffi::CStr::from_bytes_until_nul(&buffer)
                    .ok()?
                    .to_str()
                    .ok()?;
                let path = std::path::PathBuf::from(text.trim_end_matches('/'));
                path.is_dir().then_some(path)
            })
            .collect()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Vec::new()
    }
}

pub fn host_description() -> String {
    format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
}

/// Shared helpers for the Unix implementations.
#[cfg(unix)]
pub(crate) mod unix_util {
    #[cfg(target_os = "macos")]
    use std::ffi::CStr;
    use std::path::Path;

    /// Copy a NUL-terminated C string out of a `statfs`-style field.
    #[cfg(target_os = "macos")]
    pub(crate) fn c_string(raw: &[libc::c_char]) -> String {
        // SAFETY: the kernel guarantees these fields are NUL-terminated within
        // their fixed-size buffer.
        unsafe { CStr::from_ptr(raw.as_ptr()) }
            .to_string_lossy()
            .into_owned()
    }

    /// Name a volume the way a person would: the last path component of the
    /// mount point.
    ///
    /// The root has no such component. macOS resolves its real name separately
    /// (see `platform::macos::volumes`); elsewhere "System" is accurate and
    /// does not pretend to be a Mac.
    pub(crate) fn volume_name(mount_point: &str) -> String {
        if mount_point == "/" {
            return "System".to_string();
        }
        Path::new(mount_point)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| mount_point.to_string())
    }

    /// Whether DiskMate can list the mount point at all right now.
    ///
    /// A volume can appear in the mount table and still be unreadable, most
    /// often because macOS has not granted Full Disk Access yet. Checking once
    /// per volume is cheap and lets the UI explain the situation instead of
    /// showing an empty scan.
    pub(crate) fn is_readable(mount_point: &str) -> bool {
        std::fs::read_dir(mount_point).is_ok()
    }
}
