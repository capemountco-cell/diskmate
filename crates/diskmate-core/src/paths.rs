//! Where DiskMate keeps its local state.
//!
//! DiskMate must be able to run from a technician's USB drive without
//! installing anything, so no part of the engine may assume that state lives in
//! a fixed directory under the user's home. Every consumer asks [`AppPaths`].
//!
//! Resolution order:
//!
//! 1. `DISKMATE_DATA_DIR` — an explicit override, used by tests, benchmarks and
//!    anyone scripting DiskMate.
//! 2. Portable mode — a writable `DiskMate Data` directory beside the
//!    executable's bundle, or a `.diskmate-portable` marker file.
//! 3. Installed mode — the platform's per-user application-support directory.
//!
//! A portable root that turns out to be read-only (a locked disk image, a
//! write-protected stick) falls back to installed mode and records why, rather
//! than failing to start.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};

/// Directory name looked for next to the application when probing for a
/// portable installation.
pub const PORTABLE_DATA_DIR: &str = "DiskMate Data";
pub const PORTABLE_LOGS_DIR: &str = "DiskMate Logs";
/// Marker file that forces portable mode even without the directory layout.
pub const PORTABLE_MARKER: &str = ".diskmate-portable";
pub const DATA_DIR_ENV: &str = "DISKMATE_DATA_DIR";

const APP_DIR_NAME: &str = "DiskMate";
const DB_FILE_NAME: &str = "index.sqlite3";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataRootKind {
    /// Per-user application support directory.
    Installed,
    /// Running from removable media, state alongside the app.
    Portable,
    /// Location forced by `DISKMATE_DATA_DIR`.
    Explicit,
}

impl fmt::Display for DataRootKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Installed => "installed",
            Self::Portable => "portable",
            Self::Explicit => "explicit",
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPaths {
    pub kind: DataRootKind,
    pub data_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub database: PathBuf,
    /// Set when portable mode was detected but could not be used. Surfaced in
    /// the UI so a technician is never silently writing to the host Mac.
    pub portable_fallback_reason: Option<String>,
}

impl AppPaths {
    /// Resolve paths using the running executable's location.
    pub fn discover() -> Result<Self> {
        let exe = std::env::current_exe().ok();
        Self::resolve(exe.as_deref(), std::env::var_os(DATA_DIR_ENV).map(PathBuf::from))
    }

    /// Resolve paths from explicit inputs. Split out from [`Self::discover`] so
    /// the rules can be tested without relocating the test binary.
    pub fn resolve(executable: Option<&Path>, env_override: Option<PathBuf>) -> Result<Self> {
        if let Some(dir) = env_override {
            return Self::materialise(DataRootKind::Explicit, dir.clone(), dir.join("logs"), None);
        }

        let mut fallback_reason = None;
        if let Some(root) = executable.and_then(portable_root) {
            let data = root.join(PORTABLE_DATA_DIR);
            let logs = root.join(PORTABLE_LOGS_DIR);
            match Self::materialise(DataRootKind::Portable, data, logs, None) {
                Ok(paths) => return Ok(paths),
                Err(err) => {
                    fallback_reason = Some(format!(
                        "portable location {} is not writable: {err}",
                        root.display()
                    ));
                }
            }
        }

        let installed = installed_data_dir()?;
        let logs = installed.join("logs");
        Self::materialise(DataRootKind::Installed, installed, logs, fallback_reason)
    }

    fn materialise(
        kind: DataRootKind,
        data_dir: PathBuf,
        logs_dir: PathBuf,
        portable_fallback_reason: Option<String>,
    ) -> Result<Self> {
        std::fs::create_dir_all(&data_dir).map_err(|e| CoreError::io(&data_dir, e))?;
        std::fs::create_dir_all(&logs_dir).map_err(|e| CoreError::io(&logs_dir, e))?;
        let database = data_dir.join(DB_FILE_NAME);
        Ok(Self { kind, data_dir, logs_dir, database, portable_fallback_reason })
    }

    pub fn is_portable(&self) -> bool {
        self.kind == DataRootKind::Portable
    }
}

/// Walk up from the executable looking for a portable layout.
///
/// On macOS the binary sits at `<root>/DiskMate.app/Contents/MacOS/DiskMate`,
/// so the drive root is four levels up. Elsewhere it is usually one. Rather
/// than hard-coding either shape, check each ancestor for the marker directory
/// or file and stop at the first hit, bounded to a handful of levels so a
/// stray `DiskMate Data` folder far up the tree cannot capture an installed
/// build.
fn portable_root(executable: &Path) -> Option<PathBuf> {
    executable
        .ancestors()
        .skip(1)
        .take(6)
        .find(|dir| dir.join(PORTABLE_DATA_DIR).is_dir() || dir.join(PORTABLE_MARKER).is_file())
        .map(Path::to_path_buf)
}

#[cfg(target_os = "macos")]
fn installed_data_dir() -> Result<PathBuf> {
    let home = home_dir()?;
    Ok(home.join("Library").join("Application Support").join(APP_DIR_NAME))
}

#[cfg(not(target_os = "macos"))]
fn installed_data_dir() -> Result<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(xdg).join(APP_DIR_NAME));
    }
    Ok(home_dir()?.join(".local").join("share").join(APP_DIR_NAME))
}

fn home_dir() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| CoreError::invalid("HOME is not set, cannot locate application data"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_override_wins() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = AppPaths::resolve(None, Some(tmp.path().join("custom"))).unwrap();
        assert_eq!(paths.kind, DataRootKind::Explicit);
        assert!(paths.database.starts_with(tmp.path().join("custom")));
        assert!(paths.logs_dir.is_dir());
    }

    #[test]
    fn portable_layout_is_detected_beside_an_app_bundle() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("DISKMATE_USB");
        let exe = root.join("DiskMate.app/Contents/MacOS/DiskMate");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::create_dir_all(root.join(PORTABLE_DATA_DIR)).unwrap();

        let paths = AppPaths::resolve(Some(&exe), None).unwrap();
        assert_eq!(paths.kind, DataRootKind::Portable);
        assert_eq!(paths.data_dir, root.join(PORTABLE_DATA_DIR));
        assert_eq!(paths.logs_dir, root.join(PORTABLE_LOGS_DIR));
    }

    #[test]
    fn marker_file_alone_enables_portable_mode() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stick");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join(PORTABLE_MARKER), b"").unwrap();
        let exe = root.join("diskmate");

        let paths = AppPaths::resolve(Some(&exe), None).unwrap();
        assert_eq!(paths.kind, DataRootKind::Portable);
    }

    #[test]
    fn an_ordinary_executable_resolves_to_installed_mode() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = tmp.path().join("bin/diskmate");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();

        let paths = AppPaths::resolve(Some(&exe), None).unwrap();
        assert_eq!(paths.kind, DataRootKind::Installed);
        assert!(paths.portable_fallback_reason.is_none());
    }
}
