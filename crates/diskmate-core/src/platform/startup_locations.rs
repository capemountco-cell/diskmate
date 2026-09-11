//! The standard locations that hold launch configuration, and what each means.
//!
//! Pure path tables and classification, deliberately free of any system call so
//! that the rules can be unit tested on any host. The probes supply the file
//! reading; this module supplies the meaning.

use std::path::{Path, PathBuf};

use crate::model::{StartupScope, StartupSource};

/// Who owns a location: the operating system, the machine's administrator, or
/// the signed-in user. DiskMate never suggests touching an OS-owned item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocationRole {
    Os,
    Admin,
    User,
}

#[derive(Debug, Clone)]
pub struct StartupLocation {
    pub path: PathBuf,
    pub source: StartupSource,
    pub scope: StartupScope,
    pub role: LocationRole,
}

/// The launch locations DiskMate reads on macOS, in the order it presents them.
pub fn standard_locations(home: Option<&Path>) -> Vec<StartupLocation> {
    let mut locations = vec![
        StartupLocation {
            path: PathBuf::from("/Library/LaunchAgents"),
            source: StartupSource::LaunchAgent,
            scope: StartupScope::System,
            role: LocationRole::Admin,
        },
        StartupLocation {
            path: PathBuf::from("/Library/LaunchDaemons"),
            source: StartupSource::LaunchDaemon,
            scope: StartupScope::System,
            role: LocationRole::Admin,
        },
        StartupLocation {
            path: PathBuf::from("/System/Library/LaunchAgents"),
            source: StartupSource::LaunchAgent,
            scope: StartupScope::Os,
            role: LocationRole::Os,
        },
        StartupLocation {
            path: PathBuf::from("/System/Library/LaunchDaemons"),
            source: StartupSource::LaunchDaemon,
            scope: StartupScope::Os,
            role: LocationRole::Os,
        },
    ];
    if let Some(home) = home {
        locations.insert(
            0,
            StartupLocation {
                path: home.join("Library/LaunchAgents"),
                source: StartupSource::LaunchAgent,
                scope: StartupScope::User,
                role: LocationRole::User,
            },
        );
    }
    locations
}

impl LocationRole {
    /// Classify a configuration file by where it lives.
    pub fn from_path(path: &Path) -> Self {
        let text = path.to_string_lossy();
        if text.starts_with("/System/") || text.starts_with("/usr/lib/") || text.starts_with("/lib/")
        {
            Self::Os
        } else if text.starts_with("/Library/") || text.starts_with("/etc/") {
            Self::Admin
        } else {
            Self::User
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_home_adds_the_user_agent_location_first() {
        let home = PathBuf::from("/Users/ada");
        let locations = standard_locations(Some(&home));
        assert_eq!(locations[0].path, home.join("Library/LaunchAgents"));
        assert_eq!(locations[0].scope, StartupScope::User);
    }

    #[test]
    fn without_a_home_only_machine_locations_are_listed() {
        let locations = standard_locations(None);
        assert_eq!(locations.len(), 4);
        assert!(locations.iter().all(|l| l.scope != StartupScope::User));
    }

    #[test]
    fn system_locations_are_recognised_as_os_owned() {
        assert_eq!(
            LocationRole::from_path(Path::new("/System/Library/LaunchDaemons/com.apple.x.plist")),
            LocationRole::Os
        );
        assert_eq!(
            LocationRole::from_path(Path::new("/Library/LaunchAgents/com.vendor.helper.plist")),
            LocationRole::Admin
        );
        assert_eq!(
            LocationRole::from_path(Path::new("/Users/ada/Library/LaunchAgents/x.plist")),
            LocationRole::User
        );
    }
}
