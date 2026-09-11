//! Read-only inspection of macOS launch configuration.
//!
//! DiskMate enumerates the standard `LaunchAgents` and `LaunchDaemons`
//! directories and reads each property list to recover the label and the
//! program it would run. It does not call `launchctl`, does not load or unload
//! anything, and does not write to any of these locations.
//!
//! Login Items are not enumerated here. Modern macOS keeps them in a private,
//! SMAppService-managed store that has no supported read interface, and
//! guessing from legacy locations would produce a list that is wrong on any
//! recent system. Reporting an incomplete list honestly is better than
//! reporting a fabricated complete one; see `docs/permissions.md`.

use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::model::{StartupImpact, StartupItem, StartupScope, StartupSource};
use crate::platform::startup_locations::{standard_locations, LocationRole};

pub fn items() -> Result<Vec<StartupItem>> {
    let mut items = Vec::new();
    for location in standard_locations(std::env::var_os("HOME").map(PathBuf::from).as_deref()) {
        collect(
            &location.path,
            location.source,
            location.scope,
            location.role,
            &mut items,
        );
    }
    items.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(items)
}

fn collect(
    dir: &Path,
    source: StartupSource,
    scope: StartupScope,
    role: LocationRole,
    out: &mut Vec<StartupItem>,
) {
    // A missing or unreadable directory is normal: `~/Library/LaunchAgents`
    // does not exist on a clean install, and system locations may need Full
    // Disk Access. Neither is an error.
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("plist") {
            continue;
        }
        let file_name = path
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let parsed = read_job(&path);
        out.push(StartupItem {
            id: parsed.label.clone().unwrap_or_else(|| file_name.clone()),
            name: parsed.label.clone().unwrap_or(file_name),
            source,
            scope,
            config_path: path.to_string_lossy().into_owned(),
            program: parsed.program,
            runs_at_load: parsed.run_at_load,
            disabled: parsed.disabled,
            // Never guessed. Until DiskMate can measure launch cost, it says so.
            impact: StartupImpact::Unmeasured,
            is_os_provided: role == LocationRole::Os,
        });
    }
}

#[derive(Default)]
struct LaunchJob {
    label: Option<String>,
    program: Option<String>,
    run_at_load: Option<bool>,
    disabled: Option<bool>,
}

/// Read the few keys DiskMate shows. A malformed or binary-but-unreadable
/// plist yields an item with the filename and no detail, which is still worth
/// showing.
fn read_job(path: &Path) -> LaunchJob {
    let Ok(plist::Value::Dictionary(dict)) = plist::Value::from_file(path) else {
        return LaunchJob::default();
    };
    let program = dict
        .get("Program")
        .and_then(|value| value.as_string())
        .map(str::to_owned)
        .or_else(|| {
            dict.get("ProgramArguments")
                .and_then(|value| value.as_array())
                .and_then(|args| args.first())
                .and_then(|value| value.as_string())
                .map(str::to_owned)
        });
    LaunchJob {
        label: dict
            .get("Label")
            .and_then(|value| value.as_string())
            .map(str::to_owned),
        program,
        run_at_load: dict.get("RunAtLoad").and_then(|value| value.as_boolean()),
        disabled: dict.get("Disabled").and_then(|value| value.as_boolean()),
    }
}
