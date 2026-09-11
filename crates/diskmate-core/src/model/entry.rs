use serde::{Deserialize, Serialize};

use super::Category;

/// What a filesystem entry is. Stored as a small integer so the index stays
/// compact and so partial indexes can filter on it cheaply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[repr(i64)]
pub enum EntryKind {
    File = 0,
    Directory = 1,
    Symlink = 2,
    /// Sockets, FIFOs, block/character devices and anything else we can stat
    /// but should not treat as user data.
    Other = 3,
}

impl EntryKind {
    pub fn as_i64(self) -> i64 {
        self as i64
    }

    pub fn from_i64(value: i64) -> Self {
        match value {
            0 => Self::File,
            1 => Self::Directory,
            2 => Self::Symlink,
            _ => Self::Other,
        }
    }
}

/// A filesystem entry as produced by the scanner, before it reaches SQLite.
///
/// `path` is populated for directories only. File paths are reconstructed at
/// query time by joining onto the parent directory row, which removes roughly
/// 60-100 bytes of duplicated string per file from the index. On a five million
/// file volume that is the difference between a ~400 MB and a ~120 MB database.
#[derive(Debug, Clone)]
pub struct EntryRecord {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub name: String,
    /// Absolute path. `Some` for directories, `None` for every other kind.
    pub path: Option<String>,
    pub depth: i32,
    pub kind: EntryKind,
    pub logical_size: i64,
    pub allocated_size: i64,
    /// Populated by the aggregator for directories; zero for other kinds.
    pub subtree_logical_size: i64,
    pub subtree_allocated_size: i64,
    pub subtree_file_count: i64,
    pub subtree_dir_count: i64,
    pub extension: Option<String>,
    pub modified_at: Option<i64>,
    pub created_at: Option<i64>,
    pub accessed_at: Option<i64>,
    pub mode: i64,
    pub uid: i64,
    pub gid: i64,
    pub inode: i64,
    pub device: i64,
    pub nlink: i64,
    pub is_hidden: bool,
    /// True when this entry shares an inode with an entry already counted in
    /// this scan. Its bytes are excluded from aggregates so hard links are not
    /// double counted.
    pub hardlink_dup: bool,
    /// False when the entry exists but DiskMate could not read into it. Such a
    /// directory is *not* reported as zero bytes; the UI distinguishes "empty"
    /// from "not accessible".
    pub accessible: bool,
    pub category: Category,
}

/// A row as returned to the presentation layer. Paths are fully resolved here
/// because the UI only ever asks for a bounded page of rows.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryRow {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub name: String,
    pub path: String,
    pub kind: EntryKind,
    pub depth: i32,
    /// Bytes attributed to this row: subtree total for directories, logical
    /// size for everything else.
    pub size: i64,
    pub allocated_size: i64,
    pub file_count: i64,
    pub dir_count: i64,
    pub extension: Option<String>,
    pub modified_at: Option<i64>,
    pub category: Category,
    pub is_hidden: bool,
    pub accessible: bool,
    /// Share of the parent directory's subtree size, in the range 0.0..=1.0.
    pub share_of_parent: f64,
}

/// One row of the file-type breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionBucket {
    /// Lowercased extension without the dot, or `None` for extensionless files.
    pub extension: Option<String>,
    pub file_count: i64,
    pub total_size: i64,
}
