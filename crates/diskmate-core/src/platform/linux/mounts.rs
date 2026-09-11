//! Volume discovery from `/proc/self/mountinfo`.

use std::path::{Path, PathBuf};

use crate::error::{CoreError, Result};
use crate::model::{VolumeInfo, VolumeKind};
use crate::platform::unix_util;

/// Filesystems that are kernel interfaces rather than storage. They are never
/// offered as scan targets and never descended into.
const SYNTHETIC: &[&str] = &[
    "autofs", "bpf", "cgroup", "cgroup2", "configfs", "debugfs", "devpts", "devtmpfs",
    "efivarfs", "fuse.gvfsd-fuse", "fusectl", "hugetlbfs", "mqueue", "proc", "pstore",
    "ramfs", "securityfs", "sysfs", "tracefs",
];

const NETWORK: &[&str] = &["nfs", "nfs4", "cifs", "smbfs", "sshfs", "afs", "ceph"];

struct Mount {
    mount_point: String,
    fs_type: String,
    source: String,
    read_only: bool,
}

fn read_mounts() -> Result<Vec<Mount>> {
    let text = std::fs::read_to_string("/proc/self/mountinfo")
        .map_err(|e| CoreError::io("/proc/self/mountinfo", e))?;
    let mut mounts = Vec::new();
    for line in text.lines() {
        // mountinfo: id parent major:minor root mount-point options - fstype source super-opts
        let Some((left, right)) = line.split_once(" - ") else { continue };
        let left_fields: Vec<&str> = left.split(' ').collect();
        let right_fields: Vec<&str> = right.split(' ').collect();
        if left_fields.len() < 6 || right_fields.len() < 2 {
            continue;
        }
        mounts.push(Mount {
            mount_point: unescape(left_fields[4]),
            fs_type: right_fields[0].to_string(),
            source: unescape(right_fields[1]),
            read_only: left_fields[5].split(',').any(|opt| opt == "ro"),
        });
    }
    Ok(mounts)
}

/// mountinfo escapes space, tab, newline and backslash as octal.
fn unescape(raw: &str) -> String {
    if !raw.contains('\\') {
        return raw.to_string();
    }
    let bytes = raw.as_bytes();
    let mut out = String::with_capacity(raw.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 3 < bytes.len() {
            if let Some(value) = std::str::from_utf8(&bytes[i + 1..i + 4])
                .ok()
                .and_then(|digits| u8::from_str_radix(digits, 8).ok())
            {
                out.push(value as char);
                i += 4;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

pub fn volumes() -> Result<Vec<VolumeInfo>> {
    let mut volumes = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for mount in read_mounts()? {
        if SYNTHETIC.contains(&mount.fs_type.as_str()) || mount.fs_type.starts_with("fuse.") {
            continue;
        }
        if !seen.insert(mount.mount_point.clone()) {
            continue;
        }
        let (total, available) = capacity(&mount.mount_point);
        let kind = if NETWORK.contains(&mount.fs_type.as_str()) {
            VolumeKind::Network
        } else if mount.fs_type == "tmpfs" || mount.fs_type == "overlay" {
            VolumeKind::Virtual
        } else if mount.mount_point.starts_with("/media") || mount.mount_point.starts_with("/mnt") {
            VolumeKind::External
        } else {
            VolumeKind::Internal
        };
        volumes.push(
            VolumeInfo {
                id: mount.mount_point.clone(),
                name: unix_util::volume_name(&mount.mount_point),
                mount_point: mount.mount_point.clone(),
                device: Some(mount.source),
                fs_type: mount.fs_type,
                kind,
                total_bytes: total,
                available_bytes: available,
                used_bytes: None,
                is_removable: kind == VolumeKind::External,
                is_read_only: mount.read_only,
                is_browsable: true,
                is_readable: unix_util::is_readable(&mount.mount_point),
            }
            .with_derived_usage(),
        );
    }
    volumes.sort_by(|a, b| a.mount_point.cmp(&b.mount_point));
    Ok(volumes)
}

fn capacity(mount_point: &str) -> (Option<i64>, Option<i64>) {
    let Ok(path) = std::ffi::CString::new(mount_point) else { return (None, None) };
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `path` is a valid NUL-terminated string and `stat` is a correctly
    // sized, writable `statvfs`.
    if unsafe { libc::statvfs(path.as_ptr(), &mut stat) } != 0 {
        return (None, None);
    }
    let block = if stat.f_frsize > 0 { stat.f_frsize } else { stat.f_bsize } as i64;
    (
        Some(stat.f_blocks as i64 * block),
        Some(stat.f_bavail as i64 * block),
    )
}

/// Mount points nested under `root` that a scan should not descend into.
pub fn skip_mount_points(root: &Path) -> Vec<PathBuf> {
    let Ok(mounts) = read_mounts() else { return Vec::new() };
    let root_text = root.to_string_lossy().into_owned();
    mounts
        .into_iter()
        .filter(|mount| mount.mount_point != root_text && mount.mount_point.starts_with(&root_text))
        .map(|mount| PathBuf::from(mount.mount_point))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn octal_escapes_are_decoded() {
        assert_eq!(unescape("/mnt/My\\040Drive"), "/mnt/My Drive");
        assert_eq!(unescape("/plain/path"), "/plain/path");
    }

    #[test]
    fn the_host_reports_at_least_one_volume() {
        let volumes = volumes().expect("mountinfo should be readable");
        assert!(volumes.iter().any(|v| v.mount_point == "/"));
    }
}
