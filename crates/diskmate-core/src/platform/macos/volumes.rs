//! Volume discovery on macOS via `getmntinfo`.
//!
//! ## Why the mount table, and why firmlinks matter
//!
//! Since Catalina a Mac's "Macintosh HD" is two APFS volumes: a sealed,
//! read-only system volume mounted at `/`, and a data volume mounted at
//! `/System/Volumes/Data` that holds everything the user owns. Firmlinks make
//! `/Users` at the root resolve into the data volume, so the two look like one
//! disk in Finder but are separate devices to `stat`.
//!
//! DiskMate therefore presents them as one volume, and a scan of `/` must be
//! allowed to cross onto the data volume (`ScanOptions::cross_device`) while
//! still skipping every *other* mount. That is what `skip_mount_points` is for:
//! it lists the mounts nested under the scan root that are genuinely separate
//! storage, so the walker can refuse those precisely instead of refusing every
//! device change.

use std::path::{Path, PathBuf};

use crate::error::{CoreError, Result};
use crate::model::{VolumeInfo, VolumeKind};
use crate::platform::unix_util;

/// The data half of the boot volume. Presented as part of the boot volume
/// rather than as a separate disk, because that is what it is to a user.
pub const DATA_VOLUME: &str = "/System/Volumes/Data";

/// Synthetic filesystems that hold no user storage.
const SYNTHETIC_FS: &[&str] = &["devfs", "autofs", "lifs", "fdesc", "procfs", "nullfs"];
const NETWORK_FS: &[&str] = &["nfs", "smbfs", "afpfs", "webdav", "ftp"];

struct Mount {
    mount_point: String,
    device: String,
    fs_type: String,
    total_bytes: Option<i64>,
    available_bytes: Option<i64>,
    is_local: bool,
    is_read_only: bool,
    is_removable: bool,
    is_browsable: bool,
}

/// Read the kernel's mount table.
///
/// `getmntinfo` returns a pointer to a static buffer owned by libc. It is not
/// thread safe, which is why every caller goes through this function and
/// immediately copies the fields out.
fn read_mounts() -> Result<Vec<Mount>> {
    let mut raw: *mut libc::statfs = std::ptr::null_mut();
    // SAFETY: `getmntinfo` fills `raw` with a pointer to libc's static array and
    // returns the element count. We only read within that count and copy out.
    let count = unsafe { libc::getmntinfo(&mut raw, libc::MNT_NOWAIT) };
    if count <= 0 || raw.is_null() {
        return Err(CoreError::invalid(
            "getmntinfo returned no mounted filesystems",
        ));
    }
    // SAFETY: bounded by the count libc just reported.
    let slice = unsafe { std::slice::from_raw_parts(raw, count as usize) };

    Ok(slice
        .iter()
        .map(|fs| {
            let block = fs.f_bsize as i64;
            Mount {
                mount_point: unix_util::c_string(&fs.f_mntonname),
                device: unix_util::c_string(&fs.f_mntfromname),
                fs_type: unix_util::c_string(&fs.f_fstypename),
                total_bytes: Some(fs.f_blocks as i64 * block),
                // `f_bavail` is what an unprivileged process may actually use,
                // which is the number a person recognises as "available".
                available_bytes: Some(fs.f_bavail as i64 * block),
                is_local: fs.f_flags & libc::MNT_LOCAL as u32 != 0,
                is_read_only: fs.f_flags & libc::MNT_RDONLY as u32 != 0,
                is_removable: fs.f_flags & MNT_REMOVABLE != 0,
                is_browsable: fs.f_flags & MNT_DONTBROWSE == 0,
            }
        })
        .collect())
}

/// `MNT_DONTBROWSE` and `MNT_REMOVABLE` from `<sys/mount.h>`. Declared here
/// because the `libc` crate does not expose them for every Apple target.
const MNT_DONTBROWSE: u32 = 0x0010_0000;
const MNT_REMOVABLE: u32 = 0x0020_0000;

pub fn volumes() -> Result<Vec<VolumeInfo>> {
    let mounts = read_mounts()?;
    let mut volumes = Vec::new();

    for mount in &mounts {
        if SYNTHETIC_FS.contains(&mount.fs_type.as_str()) {
            continue;
        }
        // The data volume is reported as part of the boot volume below.
        if mount.mount_point == DATA_VOLUME {
            continue;
        }
        // Preboot, Recovery, VM and Update are implementation detail. They are
        // hidden from Finder for the same reason.
        if !mount.is_browsable && mount.mount_point != "/" {
            continue;
        }

        let kind = if NETWORK_FS.contains(&mount.fs_type.as_str()) {
            VolumeKind::Network
        } else if mount.is_removable || !mount.is_local {
            VolumeKind::External
        } else if mount.mount_point.starts_with("/Volumes/") {
            VolumeKind::External
        } else {
            VolumeKind::Internal
        };

        let name = if mount.mount_point == "/" {
            boot_volume_name()
        } else {
            unix_util::volume_name(&mount.mount_point)
        };

        // For the boot volume, capacity comes from the data volume: the sealed
        // system volume reports the shared APFS container's free space but a
        // misleadingly small size of its own.
        let (total, available) = if mount.mount_point == "/" {
            mounts
                .iter()
                .find(|m| m.mount_point == DATA_VOLUME)
                .map(|m| (m.total_bytes, m.available_bytes))
                .unwrap_or((mount.total_bytes, mount.available_bytes))
        } else {
            (mount.total_bytes, mount.available_bytes)
        };

        volumes.push(
            VolumeInfo {
                id: mount.device.clone(),
                name,
                mount_point: mount.mount_point.clone(),
                device: Some(mount.device.clone()),
                fs_type: mount.fs_type.clone(),
                kind,
                total_bytes: total,
                available_bytes: available,
                used_bytes: None,
                is_removable: mount.is_removable,
                is_read_only: mount.is_read_only,
                is_browsable: mount.is_browsable,
                is_readable: unix_util::is_readable(&mount.mount_point),
            }
            .with_derived_usage(),
        );
    }

    // Boot volume first, then external disks, then the rest.
    volumes.sort_by_key(|volume| {
        (
            volume.mount_point != "/",
            volume.kind != VolumeKind::External,
            volume.mount_point.clone(),
        )
    });
    Ok(volumes)
}

/// The user-visible name of the boot volume.
///
/// macOS keeps a symlink in `/Volumes` named after the boot volume and pointing
/// at `/`. Reading it gets the real name ("Macintosh HD", or whatever the user
/// renamed it to) without linking Foundation or touching a private interface.
/// If the symlink is missing, fall back to the default name rather than showing
/// a device node.
fn boot_volume_name() -> String {
    const DEFAULT: &str = "Macintosh HD";
    let Ok(entries) = std::fs::read_dir("/Volumes") else {
        return DEFAULT.to_string();
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_symlink() {
            continue;
        }
        if std::fs::read_link(&path).is_ok_and(|target| target == Path::new("/")) {
            return entry.file_name().to_string_lossy().into_owned();
        }
    }
    DEFAULT.to_string()
}

/// Mounts nested under `root` that a scan must not descend into.
///
/// The data volume is deliberately *not* in this list when scanning `/`: it is
/// the same logical disk, reached through firmlinks, and excluding it would
/// report a Mac as using ten gigabytes.
pub fn skip_mount_points(root: &Path) -> Vec<PathBuf> {
    let Ok(mounts) = read_mounts() else {
        return Vec::new();
    };
    let root_text = root.to_string_lossy().into_owned();
    let scanning_boot_volume = root_text == "/";

    mounts
        .into_iter()
        .filter(|mount| mount.mount_point != root_text)
        .filter(|mount| mount.mount_point.starts_with(&root_text))
        .filter(|mount| !(scanning_boot_volume && mount.mount_point == DATA_VOLUME))
        .map(|mount| PathBuf::from(mount.mount_point))
        .collect()
}
