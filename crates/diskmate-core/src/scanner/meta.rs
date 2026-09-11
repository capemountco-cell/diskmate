//! Turning one `stat` result into one index row.
//!
//! Kept separate from traversal because it is the single hottest function in
//! DiskMate: it runs once per filesystem entry, so every allocation and every
//! extra syscall here is multiplied by several million.

use std::fs::Metadata;

use crate::model::{Category, EntryKind, EntryRecord};

/// Metadata fields DiskMate indexes, flattened out of the platform's `stat`.
#[derive(Debug, Clone, Copy, Default)]
pub struct RawMeta {
    pub logical_size: i64,
    pub allocated_size: i64,
    pub modified_at: Option<i64>,
    pub created_at: Option<i64>,
    pub accessed_at: Option<i64>,
    pub mode: i64,
    pub uid: i64,
    pub gid: i64,
    pub inode: i64,
    pub device: i64,
    pub nlink: i64,
}

#[cfg(unix)]
pub fn raw_meta(metadata: &Metadata) -> RawMeta {
    use std::os::unix::fs::MetadataExt;
    RawMeta {
        logical_size: metadata.len() as i64,
        // `st_blocks` is defined in 512-byte units on both macOS and Linux
        // regardless of the filesystem's own block size. This is what makes
        // sparse files and APFS clones report honestly: a 10 GB sparse file
        // with nothing written to it allocates nothing.
        allocated_size: metadata.blocks() as i64 * 512,
        modified_at: Some(metadata.mtime()),
        created_at: birth_time(metadata),
        accessed_at: Some(metadata.atime()),
        mode: metadata.mode() as i64,
        uid: metadata.uid() as i64,
        gid: metadata.gid() as i64,
        inode: metadata.ino() as i64,
        device: metadata.dev() as i64,
        nlink: metadata.nlink() as i64,
    }
}

#[cfg(not(unix))]
pub fn raw_meta(metadata: &Metadata) -> RawMeta {
    let to_unix = |t: std::io::Result<std::time::SystemTime>| -> Option<i64> {
        t.ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
    };
    RawMeta {
        logical_size: metadata.len() as i64,
        allocated_size: metadata.len() as i64,
        modified_at: to_unix(metadata.modified()),
        created_at: to_unix(metadata.created()),
        accessed_at: to_unix(metadata.accessed()),
        ..RawMeta::default()
    }
}

/// Creation time, where the platform records one.
///
/// `Metadata::created` reads a field already present in the `stat` result
/// (`st_birthtime` on macOS, `statx` on Linux), so this costs arithmetic rather
/// than a syscall. Filesystems that do not track it return an error, which
/// becomes `None` rather than a fabricated timestamp.
#[cfg(unix)]
fn birth_time(metadata: &Metadata) -> Option<i64> {
    metadata
        .created()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs() as i64)
}

pub fn entry_kind(file_type: std::fs::FileType) -> EntryKind {
    if file_type.is_symlink() {
        EntryKind::Symlink
    } else if file_type.is_dir() {
        EntryKind::Directory
    } else if file_type.is_file() {
        EntryKind::File
    } else {
        EntryKind::Other
    }
}

/// Lowercased extension, or `None`.
///
/// Deliberately strict, because the extension feeds a user-visible breakdown:
/// a leading dot does not make a hidden file an extension (`.zshrc` has none),
/// and an implausibly long or space-bearing tail is a filename quirk rather
/// than a type (`Backup 2024.1.2.3 final copy`).
pub fn extension_of(name: &str) -> Option<&str> {
    let stem = name.strip_prefix('.').unwrap_or(name);
    let (_, ext) = stem.rsplit_once('.')?;
    if ext.is_empty() || ext.len() > 16 {
        return None;
    }
    if !ext.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    Some(ext)
}

/// Build the row for one filesystem entry.
#[allow(clippy::too_many_arguments)]
pub fn build_record(
    id: i64,
    parent_id: Option<i64>,
    name: String,
    path: Option<String>,
    depth: i32,
    kind: EntryKind,
    meta: RawMeta,
    category: Category,
    hardlink_dup: bool,
    accessible: bool,
) -> EntryRecord {
    let is_hidden = name.starts_with('.');
    let extension = if kind == EntryKind::File {
        extension_of(&name).map(|ext| ext.to_ascii_lowercase())
    } else {
        None
    };

    // A directory's subtree totals are filled in by the aggregator. Everything
    // else carries its own size here, so a single indexed column can order a
    // mixed listing of files and folders by the bytes attributed to each.
    //
    // Two cases get zero attributed bytes:
    //
    //  * A hard-link repeat. The data is already counted against the first path
    //    that referenced this inode, and counting it again would let a folder's
    //    total exceed the size of the disk.
    //  * A symlink, socket, FIFO or device node. `lstat` reports a symlink's
    //    size as the length of its target path, which is true and completely
    //    meaningless as storage: attributing it would make a folder full of
    //    aliases look like it held real data.
    //
    // The row still records the real `logical_size` in both cases, so nothing
    // is hidden; it is only the roll-up that excludes them.
    let counts_towards_totals = kind == EntryKind::File && !hardlink_dup;
    let attributed = if counts_towards_totals {
        meta.logical_size
    } else {
        0
    };
    // Allocated blocks are real disk cost whatever the entry is, so a symlink
    // that occupies a block is counted. Fast symlinks occupy none.
    let attributed_alloc = if kind == EntryKind::Directory || hardlink_dup {
        0
    } else {
        meta.allocated_size
    };

    EntryRecord {
        id,
        parent_id,
        name,
        path,
        depth,
        kind,
        logical_size: meta.logical_size,
        allocated_size: meta.allocated_size,
        subtree_logical_size: attributed,
        subtree_allocated_size: attributed_alloc,
        // Counts are counts of names. A hard link is a real entry a person can
        // see, so it is counted; only its bytes are excluded above.
        subtree_file_count: i64::from(kind == EntryKind::File),
        subtree_dir_count: 0,
        extension,
        modified_at: meta.modified_at,
        created_at: meta.created_at,
        accessed_at: meta.accessed_at,
        mode: meta.mode,
        uid: meta.uid,
        gid: meta.gid,
        inode: meta.inode,
        device: meta.device,
        nlink: meta.nlink,
        is_hidden,
        hardlink_dup,
        accessible,
        category,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_extensions_are_extracted() {
        assert_eq!(extension_of("photo.JPG"), Some("JPG"));
        assert_eq!(extension_of("archive.tar.gz"), Some("gz"));
    }

    #[test]
    fn a_leading_dot_is_not_an_extension() {
        assert_eq!(extension_of(".zshrc"), None);
        assert_eq!(extension_of(".config.json"), Some("json"));
    }

    #[test]
    fn implausible_tails_are_rejected() {
        assert_eq!(extension_of("Report final copy"), None);
        assert_eq!(extension_of("backup.2024 final"), None);
        assert_eq!(extension_of("file.averyverylongextension"), None);
        assert_eq!(extension_of("trailing."), None);
    }

    #[test]
    fn unicode_names_are_handled_without_panicking() {
        assert_eq!(extension_of("写真.jpeg"), Some("jpeg"));
        assert_eq!(extension_of("Ünïcödé"), None);
        assert_eq!(extension_of("emoji🎉.png"), Some("png"));
    }

    #[test]
    fn directories_defer_their_size_to_the_aggregator() {
        let record = build_record(
            1,
            None,
            "Movies".into(),
            Some("/Users/ada/Movies".into()),
            1,
            EntryKind::Directory,
            RawMeta {
                logical_size: 4096,
                allocated_size: 4096,
                ..RawMeta::default()
            },
            Category::Media,
            false,
            true,
        );
        assert_eq!(
            record.logical_size, 4096,
            "the directory inode's own size is kept"
        );
        assert_eq!(
            record.subtree_logical_size, 0,
            "totals come from the aggregator"
        );
    }

    #[test]
    fn symlinks_contribute_no_bytes_to_a_folder_total() {
        // `lstat` reports a symlink's size as its target path length.
        let meta = RawMeta {
            logical_size: 42,
            allocated_size: 0,
            ..RawMeta::default()
        };
        let link = build_record(
            1,
            Some(0),
            "alias".into(),
            None,
            1,
            EntryKind::Symlink,
            meta,
            Category::Other,
            false,
            true,
        );
        assert_eq!(
            link.logical_size, 42,
            "the row keeps what the filesystem said"
        );
        assert_eq!(link.subtree_logical_size, 0, "but it is not storage");
        assert_eq!(link.subtree_file_count, 0);
    }

    #[test]
    fn hard_link_repeats_are_attributed_no_bytes() {
        let meta = RawMeta {
            logical_size: 5_000,
            allocated_size: 5_120,
            nlink: 2,
            ..RawMeta::default()
        };
        let first = build_record(
            1,
            Some(0),
            "a.bin".into(),
            None,
            1,
            EntryKind::File,
            meta,
            Category::Other,
            false,
            true,
        );
        let repeat = build_record(
            2,
            Some(0),
            "b.bin".into(),
            None,
            1,
            EntryKind::File,
            meta,
            Category::Other,
            true,
            true,
        );
        assert_eq!(first.subtree_logical_size, 5_000);
        assert_eq!(repeat.subtree_logical_size, 0);
        assert_eq!(
            repeat.logical_size, 5_000,
            "the real size is still reported"
        );
        assert_eq!(
            repeat.subtree_file_count, 1,
            "the name is still a real entry"
        );
    }
}
