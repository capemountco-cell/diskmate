//! Folding processes into the applications they belong to.
//!
//! Platform-independent, so every probe shares it and it is tested on Linux.

use crate::model::ProcessMemory;

/// Group processes into applications by the outermost `.app` bundle their
/// executable lives in. Shared by every platform implementation and tested
/// directly.
///
/// Input rows are `(pid, executable path, process name, footprint bytes)`.
pub(crate) fn group_by_app(rows: Vec<(i32, String, String, i64)>) -> Vec<ProcessMemory> {
    use std::collections::HashMap;

    let mut groups: HashMap<String, ProcessMemory> = HashMap::new();
    for (pid, path, process_name, bytes) in rows {
        let bundle = outermost_bundle(&path);
        let key = bundle.clone().unwrap_or_else(|| format!("pid:{pid}"));
        let name = bundle
            .as_deref()
            .and_then(|b| b.rsplit('/').next())
            .map(|b| b.trim_end_matches(".app").to_string())
            .unwrap_or(process_name);
        // The main process runs the bundle's own executable directly, not one
        // nested inside a helper bundle.
        let is_main = bundle
            .as_deref()
            .map(|b| path.starts_with(&format!("{b}/Contents/MacOS/")))
            .unwrap_or(true);

        let entry = groups.entry(key).or_insert_with(|| ProcessMemory {
            pid,
            name,
            resident_bytes: 0,
            instance_count: 0,
            bundle_path: bundle.clone(),
            can_quit: false,
        });
        entry.resident_bytes += bytes.max(0);
        entry.instance_count += 1;
        if is_main {
            entry.pid = pid;
        }
    }

    let mut apps: Vec<ProcessMemory> = groups.into_values().collect();
    apps.sort_by(|a, b| {
        b.resident_bytes
            .cmp(&a.resident_bytes)
            .then(a.name.cmp(&b.name))
    });
    apps
}

/// `/Applications/Foo.app/Contents/Frameworks/Bar.app/...` -> `/Applications/Foo.app`.
fn outermost_bundle(path: &str) -> Option<String> {
    let cut = path.find(".app/")?;
    Some(path[..cut + 4].to_string())
}

#[cfg(test)]
mod tests {
    use super::group_by_app;

    #[test]
    fn helpers_fold_into_their_app_and_the_main_process_is_the_quit_target() {
        let rows = vec![
            (
                11,
                "/Applications/Browser.app/Contents/Frameworks/Browser Helper.app/Contents/MacOS/Browser Helper".to_string(),
                "Browser Helper".to_string(),
                300,
            ),
            (
                10,
                "/Applications/Browser.app/Contents/MacOS/Browser".to_string(),
                "Browser".to_string(),
                200,
            ),
            (20, "/usr/libexec/someagent".to_string(), "someagent".to_string(), 900),
            (
                30,
                "/Applications/Notes.app/Contents/MacOS/Notes".to_string(),
                "Notes".to_string(),
                50,
            ),
        ];
        let apps = group_by_app(rows);
        assert_eq!(apps.len(), 3);
        assert_eq!(apps[0].name, "someagent");
        assert_eq!(apps[0].bundle_path, None);
        assert_eq!(apps[1].name, "Browser");
        assert_eq!(apps[1].pid, 10);
        assert_eq!(apps[1].resident_bytes, 500);
        assert_eq!(apps[1].instance_count, 2);
        assert_eq!(
            apps[1].bundle_path.as_deref(),
            Some("/Applications/Browser.app")
        );
        assert!(apps.iter().all(|app| !app.can_quit));
    }
}
