use serde::{Deserialize, Serialize};

/// Coarse storage category used for the breakdown on the Storage screen.
///
/// Classification is deliberately cheap and path-driven: it runs inline in the
/// scanner with no extra syscalls. Richer, rule-driven classification lands in
/// the `file_classifications` table (see `docs/database.md`) so it can be
/// recomputed without rescanning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[repr(i64)]
pub enum Category {
    Unknown = 0,
    Applications = 1,
    Documents = 2,
    Media = 3,
    Downloads = 4,
    Developer = 5,
    Caches = 6,
    System = 7,
    Other = 8,
}

impl Category {
    pub fn as_i64(self) -> i64 {
        self as i64
    }

    pub fn from_i64(value: i64) -> Self {
        match value {
            1 => Self::Applications,
            2 => Self::Documents,
            3 => Self::Media,
            4 => Self::Downloads,
            5 => Self::Developer,
            6 => Self::Caches,
            7 => Self::System,
            8 => Self::Other,
            _ => Self::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::Applications => "Applications",
            Self::Documents => "Documents",
            Self::Media => "Media",
            Self::Downloads => "Downloads",
            Self::Developer => "Developer",
            Self::Caches => "Caches",
            Self::System => "System",
            Self::Other => "Other",
        }
    }
}

/// How safe DiskMate considers a future cleanup candidate.
///
/// Nothing in Milestone 01 deletes anything; this type exists so that the
/// vocabulary is fixed before any cleanup code is written. `Reclaimable` is
/// never a synonym for `Safe`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[repr(i64)]
pub enum SafetyClass {
    /// Regeneratable or extremely low risk.
    Safe = 0,
    /// Possibly removable, requires human judgement.
    Review = 1,
    /// DiskMate will never recommend removing this.
    Protected = 2,
}

impl SafetyClass {
    pub fn as_i64(self) -> i64 {
        self as i64
    }

    pub fn from_i64(value: i64) -> Self {
        match value {
            0 => Self::Safe,
            2 => Self::Protected,
            _ => Self::Review,
        }
    }
}

/// Classify a directory by the *segments of its absolute path*.
///
/// The scanner classifies directories only; files inherit their parent's
/// category. That keeps classification to one pass over a few path segments
/// per directory rather than a per-file string scan, and it matches how users
/// think about storage ("my Downloads folder", not "this one file in it").
///
/// `parent` is the category already assigned to the containing directory, so a
/// nested folder inside `Downloads` stays `Downloads` unless something more
/// specific applies.
pub fn classify_directory(name: &str, absolute_path: &str, parent: Category) -> Category {
    // Application bundles are the strongest signal on macOS and are cheap to
    // detect from the name alone.
    if name.ends_with(".app") {
        return Category::Applications;
    }

    if let Some(explicit) = classify_absolute(absolute_path) {
        return explicit;
    }

    match name {
        "Caches" | "cache" | "Cache" | ".cache" => return Category::Caches,
        "node_modules" | "target" | ".gradle" | ".cargo" | "DerivedData" | "Pods"
        | ".venv" | "venv" | "__pycache__" | ".next" | "build" | "dist" => {
            return Category::Developer
        }
        _ => {}
    }

    if parent == Category::Unknown {
        Category::Other
    } else {
        parent
    }
}

/// Match well-known absolute locations. Kept separate from name matching so it
/// can be unit tested against fabricated paths on any host.
fn classify_absolute(path: &str) -> Option<Category> {
    const RULES: &[(&str, Category)] = &[
        ("/Applications", Category::Applications),
        ("/System/Applications", Category::Applications),
        ("/System/Library", Category::System),
        ("/System/Volumes/Preboot", Category::System),
        ("/System/Volumes/VM", Category::System),
        ("/Library/Caches", Category::Caches),
        ("/private/var/folders", Category::Caches),
        ("/private/var/db", Category::System),
        ("/usr", Category::System),
        ("/bin", Category::System),
        ("/sbin", Category::System),
        ("/opt/homebrew", Category::Developer),
        ("/usr/local/Cellar", Category::Developer),
    ];
    for (prefix, category) in RULES {
        if path_has_prefix(path, prefix) {
            return Some(*category);
        }
    }

    // Home-relative locations. `/Users/<name>/...` on macOS, `/home/<name>/...`
    // elsewhere; matching the tail keeps one rule table for both.
    const HOME_RULES: &[(&str, Category)] = &[
        ("/Downloads", Category::Downloads),
        ("/Documents", Category::Documents),
        ("/Desktop", Category::Documents),
        ("/Movies", Category::Media),
        ("/Music", Category::Media),
        ("/Pictures", Category::Media),
        ("/Videos", Category::Media),
        ("/Library/Caches", Category::Caches),
        ("/Library/Containers", Category::Other),
        ("/Library/Developer", Category::Developer),
        ("/Library", Category::System),
    ];
    let home = home_relative(path)?;
    for (suffix, category) in HOME_RULES {
        if path_has_prefix(home, suffix) {
            return Some(*category);
        }
    }
    None
}

/// Return the portion of `path` after `/Users/<name>` or `/home/<name>`.
fn home_relative(path: &str) -> Option<&str> {
    for root in ["/Users/", "/home/"] {
        if let Some(rest) = path.strip_prefix(root) {
            let cut = rest.find('/')?;
            return Some(&rest[cut..]);
        }
    }
    None
}

/// `true` when `path` equals `prefix` or begins with `prefix` followed by a
/// separator. Plain `starts_with` would match `/Users/x/Downloadsomething`.
fn path_has_prefix(path: &str, prefix: &str) -> bool {
    if !path.starts_with(prefix) {
        return false;
    }
    matches!(path.as_bytes().get(prefix.len()), None | Some(b'/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_bundles_are_applications() {
        assert_eq!(
            classify_directory("Xcode.app", "/Applications/Xcode.app", Category::Unknown),
            Category::Applications
        );
    }

    #[test]
    fn downloads_is_matched_under_any_home() {
        for path in ["/Users/ada/Downloads", "/home/ada/Downloads"] {
            assert_eq!(
                classify_directory("Downloads", path, Category::Unknown),
                Category::Downloads
            );
        }
    }

    #[test]
    fn nested_folders_inherit_the_parent_category() {
        assert_eq!(
            classify_directory("invoices", "/Users/ada/Downloads/invoices", Category::Downloads),
            Category::Downloads
        );
    }

    #[test]
    fn prefix_match_respects_segment_boundaries() {
        assert!(path_has_prefix("/Applications", "/Applications"));
        assert!(path_has_prefix("/Applications/Xcode.app", "/Applications"));
        assert!(!path_has_prefix("/ApplicationsOld", "/Applications"));
    }

    #[test]
    fn developer_artifacts_are_recognised_by_name() {
        assert_eq!(
            classify_directory("node_modules", "/Users/ada/code/app/node_modules", Category::Other),
            Category::Developer
        );
        assert_eq!(
            classify_directory("DerivedData", "/Users/ada/Library/Developer/Xcode/DerivedData", Category::Developer),
            Category::Developer
        );
    }

    #[test]
    fn library_beats_generic_inheritance_but_caches_beats_library() {
        assert_eq!(
            classify_directory("Library", "/Users/ada/Library", Category::Unknown),
            Category::System
        );
        assert_eq!(
            classify_directory("Caches", "/Users/ada/Library/Caches", Category::System),
            Category::Caches
        );
    }

    #[test]
    fn unknown_paths_fall_back_to_other() {
        assert_eq!(
            classify_directory("stuff", "/Volumes/Backup/stuff", Category::Unknown),
            Category::Other
        );
    }
}
