//! Paths DiskMate protects.
//!
//! Milestone 01 deletes nothing, so nothing consults this list yet. It exists
//! now because the cleanup service must be built *around* a protected-path
//! check rather than having one bolted on, and because the Storage UI already
//! uses it to mark rows that will never be offered as cleanup candidates.
//!
//! The list is intentionally conservative. Anything the operating system, the
//! boot process, or a user's irreplaceable data depends on belongs here.

use std::path::Path;

/// Absolute path prefixes DiskMate will never recommend removing.
pub fn protected_prefixes() -> &'static [&'static str] {
    &[
        // Operating system.
        "/System",
        "/Library/Apple",
        "/usr",
        "/bin",
        "/sbin",
        "/etc",
        "/var/db",
        "/private/var/db",
        "/private/etc",
        "/cores",
        "/dev",
        "/Volumes",
        // Applications: removing one is an uninstall, not a cleanup.
        "/Applications",
        "/System/Applications",
        // Launch configuration. Never touched automatically, ever.
        "/Library/LaunchAgents",
        "/Library/LaunchDaemons",
        "/System/Library/LaunchAgents",
        "/System/Library/LaunchDaemons",
        // Keys, credentials and mail stores.
        "/Library/Keychains",
        // Linux equivalents, so the rule set is not macOS-shaped.
        "/boot",
        "/proc",
        "/sys",
        "/lib",
        "/lib64",
    ]
}

/// Home-relative paths that are protected for every user.
fn protected_home_suffixes() -> &'static [&'static str] {
    &[
        "/Documents",
        "/Desktop",
        "/Pictures",
        "/Movies",
        "/Music",
        "/Library/Keychains",
        "/Library/Mail",
        "/Library/Messages",
        "/Library/Photos",
        "/Library/Application Support/MobileSync",
        "/.ssh",
        "/.gnupg",
    ]
}

/// Whether a path is protected from any future cleanup action.
pub fn is_protected_path(path: &Path) -> bool {
    let Some(text) = path.to_str() else {
        // A path DiskMate cannot even render as text is not a path it should
        // ever act on.
        return true;
    };
    if protected_prefixes()
        .iter()
        .any(|prefix| has_prefix(text, prefix))
    {
        return true;
    }
    if let Some(relative) = home_relative(text) {
        if protected_home_suffixes()
            .iter()
            .any(|suffix| has_prefix(relative, suffix))
        {
            return true;
        }
    }
    false
}

fn home_relative(path: &str) -> Option<&str> {
    for root in ["/Users/", "/home/"] {
        if let Some(rest) = path.strip_prefix(root) {
            let cut = rest.find('/')?;
            return Some(&rest[cut..]);
        }
    }
    None
}

fn has_prefix(path: &str, prefix: &str) -> bool {
    path.starts_with(prefix) && matches!(path.as_bytes().get(prefix.len()), None | Some(b'/'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn system_locations_are_protected() {
        for path in [
            "/System",
            "/System/Library/CoreServices",
            "/usr/bin",
            "/Applications/Mail.app",
        ] {
            assert!(
                is_protected_path(&PathBuf::from(path)),
                "{path} should be protected"
            );
        }
    }

    #[test]
    fn user_data_is_protected() {
        for path in [
            "/Users/ada/Documents",
            "/Users/ada/Documents/taxes/2024.pdf",
            "/Users/ada/.ssh/id_ed25519",
            "/home/ada/Pictures",
        ] {
            assert!(
                is_protected_path(&PathBuf::from(path)),
                "{path} should be protected"
            );
        }
    }

    #[test]
    fn ordinary_caches_are_not_protected() {
        for path in [
            "/Users/ada/Library/Caches/Chromium",
            "/Users/ada/Downloads/installer.dmg",
        ] {
            assert!(
                !is_protected_path(&PathBuf::from(path)),
                "{path} should not be protected"
            );
        }
    }

    #[test]
    fn similar_names_do_not_match_a_protected_prefix() {
        assert!(!is_protected_path(&PathBuf::from(
            "/Users/ada/Documents-old"
        )));
        assert!(!is_protected_path(&PathBuf::from("/usrdata")));
    }
}
