//! Applications, and the files they scatter.
//!
//! Dragging an app to the Trash removes the bundle and leaves everything else:
//! caches, preferences, containers, saved state, logs, web storage, login
//! items. A "complete uninstall" means finding those, and the only reliable
//! thread connecting them is the bundle identifier.
//!
//! What this module will not do:
//!
//! * **Guess by name.** An application called "Notes" does not own every file
//!   with "notes" in it. Matching is on the bundle identifier, or on an exact
//!   directory name, and nothing looser.
//! * **Touch anything.** It produces candidates. Removal goes through the
//!   cleanup service like everything else, and an application shipped with
//!   macOS is never proposed at all.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};

use crate::db;
use crate::error::Result;
use crate::model::{CandidateKind, CandidateState, CleanupCandidate, SafetyClass};

/// An installed application.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Application {
    pub id: i64,
    pub name: String,
    pub bundle_id: Option<String>,
    pub path: String,
    pub version: Option<String>,
    pub bundle_bytes: i64,
    pub leftover_bytes: i64,
    pub leftover_count: i64,
    pub last_used_at: Option<i64>,
    /// Shipped with the operating system. Never proposed for removal.
    pub is_os_provided: bool,
}

impl Application {
    /// Bundle plus everything it left elsewhere.
    pub fn total_bytes(&self) -> i64 {
        self.bundle_bytes + self.leftover_bytes
    }
}

/// A file an application put somewhere other than its own bundle.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Leftover {
    pub path: String,
    /// Which of the standard locations this came from, in plain words.
    pub location: String,
    pub bytes: i64,
    pub safety: SafetyClass,
    pub reason: String,
}

/// Where applications scatter files, and what each location means.
///
/// The tuple is: path relative to a root, whether it is a file named after the
/// identifier rather than a directory, a human name, how risky removing it is,
/// and what removing it costs.
struct Location {
    relative: &'static str,
    /// `true` when the entry is `<identifier>.<ext>` rather than a directory.
    suffix: Option<&'static str>,
    label: &'static str,
    safety: SafetyClass,
    consequence: &'static str,
}

const HOME_LOCATIONS: &[Location] = &[
    Location {
        relative: "Library/Caches",
        suffix: None,
        label: "cache",
        safety: SafetyClass::Safe,
        consequence: "it would be rebuilt if the app were reinstalled",
    },
    Location {
        relative: "Library/HTTPStorages",
        suffix: None,
        label: "web storage",
        safety: SafetyClass::Safe,
        consequence: "cookies and web caches this app kept",
    },
    Location {
        relative: "Library/WebKit",
        suffix: None,
        label: "web data",
        safety: SafetyClass::Safe,
        consequence: "web content this app cached",
    },
    Location {
        relative: "Library/Saved Application State",
        suffix: Some("savedState"),
        label: "saved window state",
        safety: SafetyClass::Safe,
        consequence: "only which windows were open last time",
    },
    Location {
        relative: "Library/Logs",
        suffix: None,
        label: "logs",
        safety: SafetyClass::Safe,
        consequence: "diagnostic output, not your data",
    },
    Location {
        relative: "Library/Preferences",
        suffix: Some("plist"),
        label: "preferences",
        safety: SafetyClass::Review,
        consequence: "your settings for this app; reinstalling would start fresh",
    },
    Location {
        relative: "Library/Application Support",
        suffix: None,
        label: "application support",
        safety: SafetyClass::Review,
        consequence: "this can hold documents and libraries the app created",
    },
    Location {
        relative: "Library/Containers",
        suffix: None,
        label: "sandbox container",
        safety: SafetyClass::Review,
        consequence: "a sandboxed app keeps all of its data here",
    },
    Location {
        relative: "Library/Group Containers",
        suffix: None,
        label: "shared container",
        safety: SafetyClass::Review,
        consequence: "data shared between this app and its extensions",
    },
    Location {
        relative: "Library/LaunchAgents",
        suffix: Some("plist"),
        label: "launch agent",
        safety: SafetyClass::Review,
        consequence: "what made this app start automatically",
    },
];

pub struct ApplicationInspector;

impl ApplicationInspector {
    /// Directories that hold applications, and whether they are the operating
    /// system's.
    pub fn application_roots(home: Option<&Path>) -> Vec<(PathBuf, bool)> {
        let mut roots = vec![
            (PathBuf::from("/Applications"), false),
            (PathBuf::from("/Applications/Utilities"), false),
            (PathBuf::from("/System/Applications"), true),
            (PathBuf::from("/System/Applications/Utilities"), true),
        ];
        if let Some(home) = home {
            roots.push((home.join("Applications"), false));
        }
        roots
    }

    /// Find installed applications and everything they left behind.
    ///
    /// `home` and `roots` are parameters rather than being read from the
    /// environment so the whole search can be pointed at a fixture tree and
    /// tested on any host.
    pub fn inspect(home: &Path, roots: &[(PathBuf, bool)]) -> Vec<(Application, Vec<Leftover>)> {
        let mut found = Vec::new();
        for (root, os_provided) in roots {
            let Ok(entries) = std::fs::read_dir(root) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("app") {
                    continue;
                }
                let name = path
                    .file_stem()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let (bundle_id, version) = read_bundle_metadata(&path);
                let bundle_bytes = directory_size(&path);

                let leftovers = if *os_provided {
                    // An application that came with macOS is not something
                    // DiskMate offers to uninstall, so there is no reason to go
                    // looking for what it stores.
                    Vec::new()
                } else {
                    Self::leftovers(home, bundle_id.as_deref(), &name)
                };

                let app = Application {
                    id: 0,
                    name,
                    bundle_id,
                    path: path.to_string_lossy().into_owned(),
                    version,
                    bundle_bytes,
                    leftover_bytes: leftovers.iter().map(|l| l.bytes).sum(),
                    leftover_count: leftovers.len() as i64,
                    last_used_at: last_used(&path),
                    is_os_provided: *os_provided,
                };
                found.push((app, leftovers));
            }
        }
        // Largest first: the whole point is finding what is worth removing.
        found.sort_by_key(|(app, _)| std::cmp::Reverse(app.total_bytes()));
        found
    }

    /// Everything an application put outside its own bundle.
    ///
    /// Identified by bundle identifier where one exists, and by an exact
    /// directory name otherwise. Never by a partial or fuzzy name match: that
    /// is how an uninstaller deletes the wrong thing.
    pub fn leftovers(home: &Path, bundle_id: Option<&str>, name: &str) -> Vec<Leftover> {
        let mut keys: Vec<String> = Vec::new();
        if let Some(id) = bundle_id {
            keys.push(id.to_string());
        }
        keys.push(name.to_string());

        let mut out: BTreeMap<String, Leftover> = BTreeMap::new();
        for location in HOME_LOCATIONS {
            let base = home.join(location.relative);
            for key in &keys {
                let candidate = match location.suffix {
                    Some(ext) => base.join(format!("{key}.{ext}")),
                    None => base.join(key),
                };
                let Ok(metadata) = std::fs::symlink_metadata(&candidate) else {
                    continue;
                };
                let bytes = if metadata.is_dir() {
                    directory_size(&candidate)
                } else {
                    metadata.len() as i64
                };
                let path = candidate.to_string_lossy().into_owned();
                out.entry(path.clone()).or_insert(Leftover {
                    path,
                    location: location.label.to_string(),
                    bytes,
                    safety: location.safety,
                    reason: format!(
                        "{name} left this behind in {}: {}.",
                        location.label, location.consequence
                    ),
                });
            }
        }
        out.into_values().collect()
    }

    /// Store what was found, replacing the previous inspection.
    pub fn record(conn: &Connection, found: &[(Application, Vec<Leftover>)]) -> Result<usize> {
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM applications", [])?;
        let now = db::now_unix();
        for (app, leftovers) in found {
            tx.execute(
                "INSERT INTO applications
                     (bundle_id, name, path, version, bundle_bytes, leftover_bytes,
                      leftover_count, last_used_at, is_os_provided, scanned_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    app.bundle_id,
                    app.name,
                    app.path,
                    app.version,
                    app.bundle_bytes,
                    app.leftover_bytes,
                    app.leftover_count,
                    app.last_used_at,
                    i64::from(app.is_os_provided),
                    now
                ],
            )?;
            let app_id = tx.last_insert_rowid();
            for leftover in leftovers {
                tx.execute(
                    "INSERT OR REPLACE INTO application_leftovers
                         (application_id, path, location, bytes, safety, reason)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        app_id,
                        leftover.path,
                        leftover.location,
                        leftover.bytes,
                        leftover.safety.as_i64(),
                        leftover.reason
                    ],
                )?;
            }
        }
        tx.commit()?;
        Ok(found.len())
    }

    pub fn applications(conn: &Connection, include_os: bool) -> Result<Vec<Application>> {
        let sql = if include_os {
            "SELECT id, name, bundle_id, path, version, bundle_bytes, leftover_bytes,
                    leftover_count, last_used_at, is_os_provided
               FROM applications ORDER BY bundle_bytes + leftover_bytes DESC"
        } else {
            "SELECT id, name, bundle_id, path, version, bundle_bytes, leftover_bytes,
                    leftover_count, last_used_at, is_os_provided
               FROM applications WHERE is_os_provided = 0
              ORDER BY bundle_bytes + leftover_bytes DESC"
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(Application {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    bundle_id: row.get(2)?,
                    path: row.get(3)?,
                    version: row.get(4)?,
                    bundle_bytes: row.get(5)?,
                    leftover_bytes: row.get(6)?,
                    leftover_count: row.get(7)?,
                    last_used_at: row.get(8)?,
                    is_os_provided: row.get::<_, i64>(9)? != 0,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn leftovers_of(conn: &Connection, application_id: i64) -> Result<Vec<Leftover>> {
        let mut stmt = conn.prepare(
            "SELECT path, location, bytes, safety, reason
               FROM application_leftovers WHERE application_id = ?1 ORDER BY bytes DESC",
        )?;
        let rows = stmt
            .query_map(params![application_id], |row| {
                Ok(Leftover {
                    path: row.get(0)?,
                    location: row.get(1)?,
                    bytes: row.get(2)?,
                    safety: SafetyClass::from_i64(row.get(3)?),
                    reason: row.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Turn one application into cleanup candidates: the bundle plus every
    /// leftover, sharing a group key so the queue shows them together.
    ///
    /// Refuses outright for an application that shipped with macOS.
    pub fn uninstall_candidates(
        conn: &Connection,
        application_id: i64,
    ) -> Result<Vec<CleanupCandidate>> {
        let app = Self::applications(conn, true)?
            .into_iter()
            .find(|a| a.id == application_id)
            .ok_or_else(|| crate::CoreError::invalid("that application is not in the list"))?;
        if app.is_os_provided {
            return Err(crate::CoreError::invalid(format!(
                "{} came with macOS and Mr Clean will not uninstall it",
                app.name
            )));
        }

        let group_key = format!("app:{}", app.id);
        let mut out = vec![CleanupCandidate {
            id: 0,
            scan_id: 0,
            entry_id: None,
            path: app.path.clone(),
            name: format!("{}.app", app.name),
            kind: CandidateKind::AppBundle,
            safety: SafetyClass::Review,
            rule: crate::services::UNINSTALL_BUNDLE_RULE.into(),
            reason: format!("Removing this uninstalls {}.", app.name),
            detail: app
                .version
                .as_ref()
                .map(|v| format!("Version {v}."))
                .or_else(|| Some("The application itself.".into())),
            bytes: app.bundle_bytes,
            modified_at: app.last_used_at,
            state: CandidateState::Proposed,
            group_key: Some(group_key.clone()),
        }];

        for leftover in Self::leftovers_of(conn, application_id)? {
            out.push(CleanupCandidate {
                id: 0,
                scan_id: 0,
                entry_id: None,
                name: Path::new(&leftover.path)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| leftover.path.clone()),
                path: leftover.path,
                kind: CandidateKind::AppLeftover,
                safety: leftover.safety,
                rule: format!("uninstall:{}", leftover.location.replace(' ', "-")),
                reason: leftover.reason,
                detail: Some(format!("Left behind by {}.", app.name)),
                bytes: leftover.bytes,
                modified_at: None,
                state: CandidateState::Proposed,
                group_key: Some(group_key.clone()),
            });
        }
        Ok(out)
    }
}

/// Read `CFBundleIdentifier` and the version from an app's `Info.plist`.
#[cfg(target_os = "macos")]
fn read_bundle_metadata(bundle: &Path) -> (Option<String>, Option<String>) {
    let Ok(plist::Value::Dictionary(dict)) =
        plist::Value::from_file(bundle.join("Contents/Info.plist"))
    else {
        return (None, None);
    };
    let string = |key: &str| dict.get(key).and_then(|v| v.as_string()).map(str::to_owned);
    (
        string("CFBundleIdentifier"),
        string("CFBundleShortVersionString").or_else(|| string("CFBundleVersion")),
    )
}

/// Off macOS there is no `Info.plist` to read. Tests supply one anyway so the
/// matching rules can be exercised; anything else has no identifier, and the
/// search falls back to an exact directory-name match.
#[cfg(not(target_os = "macos"))]
fn read_bundle_metadata(bundle: &Path) -> (Option<String>, Option<String>) {
    let Ok(text) = std::fs::read_to_string(bundle.join("Contents/Info.plist")) else {
        return (None, None);
    };
    let extract = |key: &str| -> Option<String> {
        let marker = format!("<key>{key}</key>");
        let start = text.find(&marker)? + marker.len();
        let rest = &text[start..];
        let open = rest.find("<string>")? + "<string>".len();
        let close = rest[open..].find("</string>")?;
        Some(rest[open..open + close].trim().to_string())
    };
    (
        extract("CFBundleIdentifier"),
        extract("CFBundleShortVersionString").or_else(|| extract("CFBundleVersion")),
    )
}

/// When the bundle was last opened, as far as the filesystem knows.
fn last_used(bundle: &Path) -> Option<i64> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // Access time on the executable is the closest thing to "last
        // launched" available without private interfaces. It is indicative,
        // and the UI says so rather than presenting it as a launch record.
        std::fs::metadata(bundle).ok().map(|m| m.atime())
    }
    #[cfg(not(unix))]
    {
        let _ = bundle;
        None
    }
}

/// Recursive size of a directory, following nothing.
fn directory_size(path: &Path) -> i64 {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return 0;
    };
    if !metadata.is_dir() {
        return metadata.len() as i64;
    }
    let mut total = 0i64;
    let mut stack = vec![path.to_path_buf()];
    // Bounded so a pathological tree cannot make listing applications hang.
    let mut visited = 0usize;
    while let Some(dir) = stack.pop() {
        if visited > 200_000 {
            break;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            visited += 1;
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                stack.push(entry.path());
            } else if !meta.is_symlink() {
                total += meta.len() as i64;
            }
        }
    }
    total
}
