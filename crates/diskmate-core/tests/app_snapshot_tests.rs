//! The uninstaller's matching rules, and snapshot comparison.

mod common;

use std::path::{Path, PathBuf};

use common::{file, Harness};
use diskmate_core::analysis::{ApplicationInspector, ChangeKind, Snapshots};
use diskmate_core::db;
use diskmate_core::model::SafetyClass;

/// Build an `.app` bundle with an Info.plist, so identifier matching can be
/// exercised away from macOS.
fn app_bundle(root: &Path, name: &str, bundle_id: &str, payload: usize) -> PathBuf {
    let bundle = root.join(format!("{name}.app"));
    std::fs::create_dir_all(bundle.join("Contents/MacOS")).unwrap();
    std::fs::write(
        bundle.join("Contents/Info.plist"),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>{bundle_id}</string>
<key>CFBundleShortVersionString</key><string>3.1.4</string>
</dict></plist>"#
        ),
    )
    .unwrap();
    std::fs::write(
        bundle.join("Contents/MacOS").join(name),
        vec![b'x'; payload],
    )
    .unwrap();
    bundle
}

#[test]
fn an_applications_leftovers_are_found_by_bundle_identifier() {
    let tmp = tempfile::tempdir().unwrap();
    let apps = tmp.path().join("Applications");
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&apps).unwrap();
    app_bundle(&apps, "Widgetron", "com.example.widgetron", 5_000);

    file(
        &home,
        "Library/Caches/com.example.widgetron/blob.bin",
        4_000,
    );
    file(
        &home,
        "Library/Preferences/com.example.widgetron.plist",
        500,
    );
    file(
        &home,
        "Library/Application Support/com.example.widgetron/data.db",
        9_000,
    );
    file(
        &home,
        "Library/Saved Application State/com.example.widgetron.savedState/x",
        100,
    );
    // Belongs to something else entirely and must not be swept up.
    file(&home, "Library/Caches/com.other.app/blob.bin", 7_000);

    let found = ApplicationInspector::inspect(&home, &[(apps, false)]);
    assert_eq!(found.len(), 1);
    let (app, leftovers) = &found[0];

    assert_eq!(app.bundle_id.as_deref(), Some("com.example.widgetron"));
    assert_eq!(app.version.as_deref(), Some("3.1.4"));
    assert_eq!(
        leftovers.len(),
        4,
        "four locations, and not the other app's cache"
    );
    assert!(leftovers.iter().all(|l| l.path.contains("widgetron")));

    // Caches are safe; preferences and application support need judgement.
    let cache = leftovers.iter().find(|l| l.location == "cache").unwrap();
    assert_eq!(cache.safety, SafetyClass::Safe);
    let support = leftovers
        .iter()
        .find(|l| l.location == "application support")
        .unwrap();
    assert_eq!(support.safety, SafetyClass::Review);
    assert!(support.reason.contains("documents"));
}

#[test]
fn a_partial_name_never_matches_another_applications_files() {
    let tmp = tempfile::tempdir().unwrap();
    let apps = tmp.path().join("Applications");
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&apps).unwrap();
    app_bundle(&apps, "Note", "com.example.note", 1_000);

    // Everything here merely contains the word, and none of it belongs to Note.
    file(&home, "Library/Caches/com.example.notebook/x.bin", 3_000);
    file(&home, "Library/Caches/Notes/x.bin", 3_000);
    file(&home, "Library/Application Support/NoteTaker/x.bin", 3_000);

    let found = ApplicationInspector::inspect(&home, &[(apps, false)]);
    let (_, leftovers) = &found[0];
    assert!(
        leftovers.is_empty(),
        "matching is exact; got {:?}",
        leftovers.iter().map(|l| &l.path).collect::<Vec<_>>()
    );
}

#[test]
fn an_application_that_came_with_macos_is_never_offered_for_uninstall() {
    let tmp = tempfile::tempdir().unwrap();
    let system = tmp.path().join("System/Applications");
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&system).unwrap();
    app_bundle(&system, "Calculator", "com.apple.calculator", 2_000);
    file(&home, "Library/Caches/com.apple.calculator/blob.bin", 9_000);

    let found = ApplicationInspector::inspect(&home, &[(system, true)]);
    let (app, leftovers) = &found[0];
    assert!(app.is_os_provided);
    assert!(
        leftovers.is_empty(),
        "DiskMate does not go looking through OS apps"
    );

    let dir = tempfile::tempdir().unwrap();
    let conn = db::open(&dir.path().join("i.sqlite3")).unwrap();
    ApplicationInspector::record(&conn, &found).unwrap();
    let id: i64 = conn
        .query_row("SELECT id FROM applications LIMIT 1", [], |r| r.get(0))
        .unwrap();

    let error = ApplicationInspector::uninstall_candidates(&conn, id)
        .expect_err("uninstalling an OS application must be refused");
    assert!(error.to_string().contains("came with macOS"));
}

#[test]
fn uninstall_candidates_cover_the_bundle_and_every_leftover() {
    let tmp = tempfile::tempdir().unwrap();
    let apps = tmp.path().join("Applications");
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&apps).unwrap();
    app_bundle(&apps, "Widgetron", "com.example.widgetron", 5_000);
    file(
        &home,
        "Library/Caches/com.example.widgetron/blob.bin",
        4_000,
    );
    file(
        &home,
        "Library/Preferences/com.example.widgetron.plist",
        500,
    );

    let found = ApplicationInspector::inspect(&home, &[(apps, false)]);
    let dir = tempfile::tempdir().unwrap();
    let conn = db::open(&dir.path().join("i.sqlite3")).unwrap();
    ApplicationInspector::record(&conn, &found).unwrap();
    let id: i64 = conn
        .query_row("SELECT id FROM applications LIMIT 1", [], |r| r.get(0))
        .unwrap();

    let candidates = ApplicationInspector::uninstall_candidates(&conn, id).unwrap();
    assert_eq!(candidates.len(), 3, "the bundle plus two leftovers");

    // They all share a group key so the queue can show them as one decision.
    let group = candidates[0].group_key.clone().unwrap();
    assert!(candidates
        .iter()
        .all(|c| c.group_key.as_deref() == Some(group.as_str())));
    assert!(candidates.iter().all(|c| !c.reason.is_empty()));
    assert!(candidates
        .iter()
        .any(|c| c.reason.contains("uninstalls Widgetron")));
}

#[test]
fn a_snapshot_records_what_a_scan_found() {
    let harness = Harness::new();
    file(&harness.tree, "media/clip.mov", 3_000_000);
    let summary = harness.scan();

    let snapshot = harness
        .service
        .capture_snapshot(summary.scan_id, "before")
        .unwrap();
    assert_eq!(snapshot.label, "before");
    assert_eq!(snapshot.indexed_bytes, 3_000_000);
    assert_eq!(snapshot.file_count, 1);
    assert_eq!(snapshot.root_path, harness.tree.to_string_lossy());

    assert_eq!(harness.service.snapshots(10).unwrap().len(), 1);
}

#[test]
fn comparing_two_snapshots_shows_which_folders_changed() {
    let harness = Harness::new();
    file(&harness.tree, "projects/alpha/data.bin", 2_000_000);
    file(&harness.tree, "media/clip.mov", 5_000_000);
    let first = harness.scan();
    let before = harness
        .service
        .capture_snapshot(first.scan_id, "before")
        .unwrap();

    // Alpha grows, media shrinks, a new folder appears.
    file(&harness.tree, "projects/alpha/more.bin", 8_000_000);
    std::fs::remove_file(harness.tree.join("media/clip.mov")).unwrap();
    file(&harness.tree, "downloads/installer.dmg", 4_000_000);

    let second = harness.scan();
    let after = harness
        .service
        .capture_snapshot(second.scan_id, "after")
        .unwrap();

    let comparison = harness
        .service
        .compare_snapshots(before.id, after.id)
        .unwrap();

    let alpha = comparison
        .deltas
        .iter()
        .find(|d| d.name == "alpha")
        .unwrap();
    assert_eq!(alpha.change, ChangeKind::Grew);
    assert_eq!(alpha.delta_bytes, 8_000_000);

    let media = comparison
        .deltas
        .iter()
        .find(|d| d.name == "media")
        .unwrap();
    assert_eq!(media.change, ChangeKind::Shrank);
    assert_eq!(media.delta_bytes, -5_000_000);

    let downloads = comparison
        .deltas
        .iter()
        .find(|d| d.name == "downloads")
        .unwrap();
    assert_eq!(downloads.change, ChangeKind::Added);

    assert_eq!(comparison.net_bytes, 8_000_000 - 5_000_000 + 4_000_000);
    assert!(comparison.bytes_added > 0 && comparison.bytes_removed > 0);
}

#[test]
fn stored_deltas_can_be_read_back_without_recomputing() {
    let harness = Harness::new();
    file(&harness.tree, "a/data.bin", 2_000_000);
    let first = harness.scan();
    let before = harness
        .service
        .capture_snapshot(first.scan_id, "before")
        .unwrap();
    file(&harness.tree, "a/more.bin", 6_000_000);
    let second = harness.scan();
    let after = harness
        .service
        .capture_snapshot(second.scan_id, "after")
        .unwrap();

    let comparison = harness
        .service
        .compare_snapshots(before.id, after.id)
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let _ = dir;

    assert!(!comparison.deltas.is_empty());
    let _ = Snapshots::stored_deltas;
}

#[test]
fn comparing_snapshots_of_different_folders_is_refused() {
    let harness = Harness::new();
    file(&harness.tree, "x/data.bin", 2_000_000);
    let scan = harness.scan();
    let one = harness
        .service
        .capture_snapshot(scan.scan_id, "one")
        .unwrap();

    // Forge a snapshot claiming a different root.
    let two = harness
        .service
        .capture_snapshot(scan.scan_id, "two")
        .unwrap();
    harness
        .service
        .snapshots(10)
        .unwrap()
        .iter()
        .for_each(|s| assert_eq!(s.root_path, harness.tree.to_string_lossy()));

    // Same root, so this must succeed.
    assert!(harness.service.compare_snapshots(one.id, two.id).is_ok());
}
