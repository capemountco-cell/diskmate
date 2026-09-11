//! Read-only enumeration of things that start automatically on Linux.
//!
//! This exists so that the startup service has a real implementation to develop
//! against outside macOS. Like the macOS probe it only reads: nothing here
//! enables, disables, masks or removes a unit.

use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::model::{StartupImpact, StartupItem, StartupScope, StartupSource};

use crate::platform::startup_locations::LocationRole;

pub fn items() -> Result<Vec<StartupItem>> {
    let mut items = Vec::new();
    let home = std::env::var_os("HOME").map(PathBuf::from);

    let mut locations: Vec<(PathBuf, StartupSource, StartupScope, bool)> = vec![
        (
            PathBuf::from("/etc/systemd/system"),
            StartupSource::SystemService,
            StartupScope::System,
            false,
        ),
        (
            PathBuf::from("/etc/xdg/autostart"),
            StartupSource::Autostart,
            StartupScope::System,
            false,
        ),
        (
            PathBuf::from("/lib/systemd/system"),
            StartupSource::SystemService,
            StartupScope::Os,
            true,
        ),
    ];
    if let Some(home) = home {
        locations.push((
            home.join(".config/systemd/user"),
            StartupSource::UserService,
            StartupScope::User,
            false,
        ));
        locations.push((
            home.join(".config/autostart"),
            StartupSource::Autostart,
            StartupScope::User,
            false,
        ));
    }

    for (dir, source, scope, os_provided) in locations {
        collect(&dir, source, scope, os_provided, &mut items);
    }
    items.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(items)
}

fn collect(
    dir: &Path,
    source: StartupSource,
    scope: StartupScope,
    os_provided: bool,
    out: &mut Vec<StartupItem>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        if !(name.ends_with(".service") || name.ends_with(".desktop")) {
            continue;
        }
        out.push(StartupItem {
            id: name.clone(),
            name: name
                .trim_end_matches(".service")
                .trim_end_matches(".desktop")
                .to_string(),
            source,
            scope,
            config_path: path.to_string_lossy().into_owned(),
            program: exec_line(&path),
            runs_at_load: None,
            disabled: None,
            // Never guessed. See `model::StartupImpact`.
            impact: StartupImpact::Unmeasured,
            is_os_provided: os_provided || LocationRole::from_path(&path) == LocationRole::Os,
        });
    }
}

/// Pull the command out of a unit or desktop file without executing anything.
fn exec_line(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines()
        .find_map(|line| {
            line.strip_prefix("ExecStart=")
                .or_else(|| line.strip_prefix("Exec="))
        })
        .map(|value| value.trim().to_string())
}
