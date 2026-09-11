//! End-to-end scanner behaviour against real directory trees.

mod common;

use std::path::PathBuf;

use common::{dir, file, running_as_root, symlink, Harness};
use diskmate_core::db::{ChildQuery, SortDirection, SortKey};
use diskmate_core::model::{EntryKind, ScanErrorKind, ScanStatus};

#[test]
fn a_simple_tree_is_counted_exactly() {
    let harness = Harness::new();
    file(&harness.tree, "a.txt", 100);
    file(&harness.tree, "b.txt", 250);
    dir(&harness.tree, "nested");
    file(&harness.tree, "nested/c.bin", 1_000);

    let summary = harness.scan();

    assert_eq!(summary.status, ScanStatus::Completed);
    assert_eq!(summary.file_count, 3);
    assert_eq!(summary.dir_count, 2, "the root counts itself plus `nested`");
    assert_eq!(summary.logical_bytes, 1_350);
    assert_eq!(summary.error_count, 0);
}

#[test]
fn an_empty_directory_is_zero_bytes_and_still_accessible() {
    let harness = Harness::new();
    dir(&harness.tree, "empty");
    file(&harness.tree, "data.bin", 512);

    let summary = harness.scan();
    let children = harness.root_children(summary.scan_id);
    let empty = children
        .iter()
        .find(|row| row.name == "empty")
        .expect("empty dir listed");

    assert_eq!(empty.kind, EntryKind::Directory);
    assert_eq!(empty.size, 0);
    assert_eq!(empty.file_count, 0);
    assert!(empty.accessible, "an empty folder is readable, just empty");
}

#[test]
fn nested_totals_roll_all_the_way_up() {
    let harness = Harness::new();
    file(&harness.tree, "one/two/three/deep.bin", 4_096);
    file(&harness.tree, "one/two/sibling.bin", 1_024);
    file(&harness.tree, "one/top.bin", 16);

    let summary = harness.scan();
    let root = harness.service.snapshot(summary.scan_id).unwrap().root;
    assert_eq!(root.size, 5_136);

    let one = harness.child_named(summary.scan_id, root.id, "one");
    assert_eq!(one.size, 5_136);
    assert_eq!(one.file_count, 3);
    assert_eq!(one.dir_count, 2, "`two` and `three`");

    let two = harness.child_named(summary.scan_id, one.id, "two");
    assert_eq!(two.size, 5_120);
    assert_eq!(two.dir_count, 1);
}

#[test]
fn symlinks_are_recorded_but_never_followed() {
    let harness = Harness::new();
    let target = dir(&harness.tree, "real");
    file(&harness.tree, "real/payload.bin", 10_000);
    symlink(&target, &harness.path("link-to-real"));
    symlink(
        &harness.path("real/payload.bin"),
        &harness.path("link-to-file"),
    );

    let summary = harness.scan();

    assert_eq!(summary.symlink_count, 2);
    assert_eq!(
        summary.file_count, 1,
        "the link target must not be counted twice"
    );
    assert_eq!(
        summary.logical_bytes, 10_000,
        "following the directory link would double the total"
    );

    let children = harness.root_children(summary.scan_id);
    let link = children
        .iter()
        .find(|row| row.name == "link-to-real")
        .unwrap();
    assert_eq!(link.kind, EntryKind::Symlink);
    assert_eq!(link.dir_count, 0);
}

#[test]
fn a_symlink_loop_terminates_instead_of_recursing() {
    let harness = Harness::new();
    let inner = dir(&harness.tree, "outer/inner");
    // The classic trap: a link inside the tree pointing back at its ancestor.
    symlink(&harness.path("outer"), &inner.join("back"));
    symlink(&inner.join("back"), &harness.path("outer/side"));
    file(&harness.tree, "outer/payload.bin", 64);

    let summary = harness.scan();

    assert_eq!(summary.status, ScanStatus::Completed);
    assert_eq!(summary.file_count, 1);
    assert_eq!(summary.symlink_count, 2);
    assert_eq!(summary.logical_bytes, 64);
}

#[test]
fn a_directory_that_cannot_be_read_is_marked_inaccessible_not_empty() {
    use std::os::unix::fs::PermissionsExt;

    let harness = Harness::new();
    file(&harness.tree, "visible.bin", 128);
    let locked = dir(&harness.tree, "locked");
    file(&harness.tree, "locked/hidden.bin", 4_096);
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();

    let can_still_read = std::fs::read_dir(&locked).is_ok();
    let summary = harness.scan();
    // Restore so the temp dir can be cleaned up.
    let _ = std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755));

    let children = harness.root_children(summary.scan_id);
    let row = children
        .iter()
        .find(|row| row.name == "locked")
        .expect("locked dir listed");

    if can_still_read {
        // Running as root: permissions do not apply, so assert the scan is
        // simply correct rather than skipping the test silently.
        assert!(
            running_as_root(),
            "a non-root process should not have read a 0o000 directory"
        );
        assert!(row.accessible);
        assert_eq!(row.size, 4_096);
        assert_eq!(summary.error_count, 0);
    } else {
        assert!(
            !row.accessible,
            "an unreadable folder must not look like an empty one"
        );
        assert_eq!(row.size, 0);
        assert_eq!(summary.file_count, 1, "only the readable file is counted");
        assert!(summary.error_count >= 1);

        let errors = harness.service.scan_errors(summary.scan_id, 100).unwrap();
        assert!(errors
            .iter()
            .any(|e| e.kind == ScanErrorKind::PermissionDenied));
    }
}

#[test]
fn one_unreadable_directory_does_not_abort_the_scan() {
    use std::os::unix::fs::PermissionsExt;

    let harness = Harness::new();
    for index in 0..20 {
        file(&harness.tree, &format!("ok-{index}/data.bin"), 100);
    }
    let locked = dir(&harness.tree, "locked");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();

    let summary = harness.scan();
    let _ = std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755));

    assert_eq!(summary.status, ScanStatus::Completed);
    assert_eq!(summary.file_count, 20);
    assert_eq!(summary.logical_bytes, 2_000);
}

#[test]
fn scanning_a_file_instead_of_a_directory_is_a_clean_error() {
    let harness = Harness::new();
    let target = file(&harness.tree, "not-a-dir.txt", 10);
    let options = diskmate_core::model::ScanOptions::new(&target);

    let error = harness
        .service
        .scan(options, |_| {})
        .expect_err("should refuse");
    assert!(error.to_string().contains("not a directory"));
}

#[test]
fn unusual_and_unicode_filenames_survive_indexing() {
    let harness = Harness::new();
    let long_name = format!("{}.dat", "very-long-".repeat(20));
    let names = [
        "spaces in name.txt",
        "emoji-🎉-party.bin",
        "日本語のファイル.txt",
        "Ünïcödé-ÅÄÖ.dat",
        "trailing.dots...",
        "quote'and\"quote.txt",
        "semi;colon&amp.bin",
        "-leading-dash.txt",
        ".hidden-file",
        long_name.as_str(),
    ];
    for (index, name) in names.iter().enumerate() {
        file(&harness.tree, name, index + 1);
    }

    let summary = harness.scan();
    assert_eq!(summary.file_count, names.len() as i64);

    let children = harness.root_children(summary.scan_id);
    for name in names {
        assert!(
            children.iter().any(|row| row.name == name),
            "{name:?} should be in the index"
        );
    }

    let hidden = children
        .iter()
        .find(|row| row.name == ".hidden-file")
        .unwrap();
    assert!(hidden.is_hidden);
}

#[cfg(unix)]
#[test]
fn a_filename_that_is_not_valid_utf8_is_still_indexed() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let harness = Harness::new();
    // 0xFF is never valid UTF-8. The file is real and takes real space, so
    // dropping it would under-report the volume.
    let raw = OsStr::from_bytes(b"broken-\xFF-name.bin");
    std::fs::write(harness.tree.join(raw), vec![b'x'; 321]).unwrap();

    let summary = harness.scan();

    assert_eq!(summary.file_count, 1);
    assert_eq!(summary.logical_bytes, 321);
    let children = harness.root_children(summary.scan_id);
    assert!(
        children[0].name.contains('\u{FFFD}'),
        "name is stored lossily, not dropped"
    );
}

#[test]
fn very_deep_paths_are_traversed_without_stack_growth() {
    const DEPTH: usize = 128;
    let harness = Harness::new();
    let mut relative = PathBuf::new();
    for level in 0..DEPTH {
        relative.push(format!("d{level}"));
    }
    file(
        &harness.tree,
        &format!("{}/leaf.bin", relative.display()),
        777,
    );

    let summary = harness.scan();

    assert_eq!(summary.status, ScanStatus::Completed);
    assert_eq!(summary.dir_count, DEPTH as i64 + 1);
    assert_eq!(summary.logical_bytes, 777);

    let root = harness.service.snapshot(summary.scan_id).unwrap().root;
    assert_eq!(root.size, 777, "the total survives a 128-level rollup");
}

#[test]
fn max_depth_stops_traversal_without_losing_the_rows_already_seen() {
    let harness = Harness::new();
    file(&harness.tree, "a/b/c/deep.bin", 500);
    file(&harness.tree, "a/shallow.bin", 100);

    let mut options = harness.options();
    options.max_depth = Some(2);
    let summary = harness.scan_with(options);

    assert_eq!(
        summary.logical_bytes, 100,
        "only entries above the cut are counted"
    );
    let root = harness.service.snapshot(summary.scan_id).unwrap().root;
    let a = harness.child_named(summary.scan_id, root.id, "a");
    let b = harness.child_named(summary.scan_id, a.id, "b");
    assert!(
        !b.accessible,
        "a directory below the depth limit is not reported as empty"
    );
}

#[test]
fn a_large_synthetic_tree_is_indexed_correctly() {
    const DIRS: usize = 100;
    const FILES_PER_DIR: usize = 100;

    let harness = Harness::new();
    for d in 0..DIRS {
        for f in 0..FILES_PER_DIR {
            file(&harness.tree, &format!("bucket-{d:03}/file-{f:03}.dat"), 10);
        }
    }

    let summary = harness.scan();

    assert_eq!(summary.file_count, (DIRS * FILES_PER_DIR) as i64);
    assert_eq!(summary.dir_count, DIRS as i64 + 1);
    assert_eq!(summary.logical_bytes, (DIRS * FILES_PER_DIR * 10) as i64);
    assert!(summary.entries_per_second > 0.0);

    // Batching must not have dropped or duplicated anything.
    let root = harness.service.snapshot(summary.scan_id).unwrap().root;
    let count = harness
        .service
        .child_count(summary.scan_id, root.id, false)
        .unwrap();
    assert_eq!(count, DIRS as i64);
}

#[test]
fn cancellation_stops_quickly_and_leaves_a_labelled_partial_index() {
    const DIRS: usize = 400;
    let harness = Harness::new();
    for d in 0..DIRS {
        for f in 0..50 {
            file(&harness.tree, &format!("b-{d:03}/f-{f:02}.dat"), 64);
        }
    }

    let service = harness.service.clone();
    let options = harness.options();
    let started = std::time::Instant::now();
    let summary = service
        .scan(options, {
            let service = service.clone();
            move |progress| {
                if progress.entries_scanned > 500 {
                    service.cancel_active_scan();
                }
            }
        })
        .expect("a cancelled scan still returns a summary");

    assert_eq!(summary.status, ScanStatus::Cancelled);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(20),
        "cancellation should not wait for the whole tree"
    );
    assert!(
        summary.file_count < (DIRS * 50) as i64,
        "the scan really did stop early"
    );

    // The partial index must still be navigable.
    let snapshot = harness
        .service
        .snapshot(summary.scan_id)
        .expect("partial snapshot");
    assert_eq!(snapshot.scan.status, ScanStatus::Cancelled);
    assert!(snapshot.root.id > 0);
}

#[test]
fn progress_events_are_throttled_but_still_arrive() {
    let harness = Harness::new();
    for d in 0..40 {
        for f in 0..50 {
            file(&harness.tree, &format!("p-{d:02}/f-{f:02}.dat"), 8);
        }
    }

    let mut options = harness.options();
    options.progress_interval_ms = 20;
    let (summary, events) = harness.scan_collecting_progress(options);

    assert!(!events.is_empty(), "the UI must receive progress");
    assert!(
        events.len() < summary.file_count as usize,
        "progress is coalesced, not one event per entry"
    );
    let last = events.last().unwrap();
    assert_eq!(last.status, ScanStatus::Completed);
    assert_eq!(last.files_scanned as i64, summary.file_count);
}

#[cfg(unix)]
#[test]
fn hard_links_are_counted_once() {
    let harness = Harness::new();
    let original = file(&harness.tree, "original.bin", 5_000);
    std::fs::hard_link(&original, harness.path("link-a.bin")).unwrap();
    std::fs::hard_link(&original, harness.path("link-b.bin")).unwrap();

    let summary = harness.scan();

    assert_eq!(summary.file_count, 3, "all three names are real entries");
    assert_eq!(
        summary.logical_bytes, 5_000,
        "but only one file's worth of storage"
    );

    let children = harness.root_children(summary.scan_id);
    assert_eq!(children.len(), 3, "every name is still listed");
    for row in &children {
        assert_eq!(
            row.size, 5_000,
            "each name reports the size of the file it points at"
        );
    }

    // The folder total is the invariant that must not double count: three names
    // for one 5 KB file is 5 KB of storage, not 15 KB.
    let root = harness.service.snapshot(summary.scan_id).unwrap().root;
    assert_eq!(
        root.size, 5_000,
        "attributed bytes never exceed what is on disk"
    );
}

#[test]
fn rescanning_produces_an_independent_scan() {
    let harness = Harness::new();
    file(&harness.tree, "first.bin", 100);
    let first = harness.scan();

    file(&harness.tree, "second.bin", 300);
    let second = harness.scan();

    assert_ne!(first.scan_id, second.scan_id);
    assert_eq!(first.logical_bytes, 100, "the earlier scan is unchanged");
    assert_eq!(second.logical_bytes, 400);

    // Entry ids must not collide across scans.
    let first_root = harness.service.snapshot(first.scan_id).unwrap().root;
    let second_root = harness.service.snapshot(second.scan_id).unwrap().root;
    assert_ne!(first_root.id, second_root.id);
    assert_eq!(
        harness
            .service
            .children(&ChildQuery::new(first.scan_id, first_root.id))
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn a_scan_left_running_is_reaped_as_interrupted_on_restart() {
    use diskmate_core::db::{self, StorageQueries};

    let data = tempfile::tempdir().unwrap();
    let db_path = data.path().join("index.sqlite3");
    let conn = db::open(&db_path).unwrap();
    conn.execute(
        "INSERT INTO scans (root_path, status, started_at) VALUES ('/tmp/x', 'running', 0)",
        [],
    )
    .unwrap();
    let scan_id = conn.last_insert_rowid();
    drop(conn);

    // Reopening the index is what a restart looks like.
    let conn = db::open(&db_path).unwrap();
    let reaped = StorageQueries::reap_interrupted_scans(&conn).unwrap();

    assert_eq!(reaped, 1);
    let summary = StorageQueries::scan_summary(&conn, scan_id).unwrap();
    assert_eq!(summary.status, ScanStatus::Interrupted);
}

#[test]
fn navigation_sorts_and_paginates_over_the_index() {
    let harness = Harness::new();
    for (name, size) in [("small.bin", 10), ("medium.bin", 500), ("large.bin", 9_000)] {
        file(&harness.tree, name, size);
    }
    let summary = harness.scan();
    let root = harness.service.snapshot(summary.scan_id).unwrap().root;

    let by_size = harness
        .service
        .children(&ChildQuery::new(summary.scan_id, root.id))
        .unwrap();
    assert_eq!(
        by_size.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
        ["large.bin", "medium.bin", "small.bin"]
    );

    let by_name = harness
        .service
        .children(&ChildQuery {
            sort: SortKey::Name,
            direction: SortDirection::Asc,
            ..ChildQuery::new(summary.scan_id, root.id)
        })
        .unwrap();
    assert_eq!(by_name[0].name, "large.bin");

    let page = harness
        .service
        .children(&ChildQuery {
            limit: 1,
            offset: 1,
            ..ChildQuery::new(summary.scan_id, root.id)
        })
        .unwrap();
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].name, "medium.bin");
}

#[test]
fn file_paths_are_reconstructed_from_the_parent_directory() {
    let harness = Harness::new();
    file(&harness.tree, "deep/folder/target.bin", 42);
    let summary = harness.scan();

    let largest = harness
        .service
        .snapshot(summary.scan_id)
        .unwrap()
        .largest_files;
    let target = largest.iter().find(|row| row.name == "target.bin").unwrap();

    assert_eq!(
        target.path,
        harness.path("deep/folder/target.bin").to_string_lossy()
    );
    let ancestors = harness
        .service
        .ancestors(summary.scan_id, target.id)
        .unwrap();
    assert_eq!(ancestors.len(), 4, "root, deep, folder, target.bin");
    assert_eq!(ancestors.first().unwrap().parent_id, None);
    assert_eq!(ancestors.last().unwrap().name, "target.bin");
}

#[test]
fn share_of_parent_never_exceeds_one() {
    let harness = Harness::new();
    file(&harness.tree, "big/a.bin", 900);
    file(&harness.tree, "big/b.bin", 100);
    let summary = harness.scan();

    let root = harness.service.snapshot(summary.scan_id).unwrap().root;
    let big = harness.child_named(summary.scan_id, root.id, "big");
    let children = harness
        .service
        .children(&ChildQuery::new(summary.scan_id, big.id))
        .unwrap();

    let shares: Vec<f64> = children.iter().map(|row| row.share_of_parent).collect();
    assert!((shares.iter().sum::<f64>() - 1.0).abs() < 1e-9);
    assert!(shares.iter().all(|share| (0.0..=1.0).contains(share)));
}

#[test]
fn breakdowns_reflect_what_was_scanned() {
    let harness = Harness::new();
    file(&harness.tree, "movie.mov", 5_000);
    file(&harness.tree, "clip.mov", 3_000);
    file(&harness.tree, "notes.txt", 100);
    file(&harness.tree, "no-extension", 7);

    let summary = harness.scan();
    let snapshot = harness.service.snapshot(summary.scan_id).unwrap();

    let mov = snapshot
        .extensions
        .iter()
        .find(|b| b.extension.as_deref() == Some("mov"))
        .unwrap();
    assert_eq!(mov.file_count, 2);
    assert_eq!(mov.total_size, 8_000);

    let none = snapshot
        .extensions
        .iter()
        .find(|b| b.extension.is_none())
        .unwrap();
    assert_eq!(none.file_count, 1);

    let categorised: i64 = snapshot.categories.iter().map(|c| c.file_count).sum();
    assert_eq!(categorised, 4, "every file lands in exactly one category");
}

#[test]
fn a_second_scan_cannot_start_while_one_is_running() {
    let harness = Harness::new();
    for index in 0..200 {
        file(&harness.tree, &format!("d{index:03}/f.bin"), 32);
    }

    let service = harness.service.clone();
    let rejected = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = rejected.clone();
    let options = harness.options();

    service
        .scan(options.clone(), {
            let service = service.clone();
            move |_| {
                if service.scan(options.clone(), |_| {}).is_err() {
                    flag.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            }
        })
        .unwrap();

    assert!(rejected.load(std::sync::atomic::Ordering::SeqCst));
    assert!(
        harness.service.active_scan().is_none(),
        "the slot is released afterwards"
    );
}

#[cfg(unix)]
#[test]
fn a_hard_link_reports_its_real_size_and_says_the_bytes_are_shared() {
    let harness = Harness::new();
    let original = file(&harness.tree, "original.bin", 5_000);
    std::fs::hard_link(&original, harness.path("copy.bin")).unwrap();

    let summary = harness.scan();
    let children = harness.root_children(summary.scan_id);
    let shared: Vec<_> = children.iter().filter(|row| row.shares_storage).collect();

    assert_eq!(
        shared.len(),
        1,
        "exactly one of the two names is the repeat"
    );
    assert_eq!(shared[0].size, 5_000, "it still reports what the file is");
    assert_eq!(
        shared[0].share_of_parent, 0.0,
        "but it claims none of the folder"
    );

    let total_share: f64 = children.iter().map(|row| row.share_of_parent).sum();
    assert!((total_share - 1.0).abs() < 1e-9, "shares still sum to one");
}

#[test]
fn largest_folders_skips_folders_that_only_contain_one_big_folder() {
    let harness = Harness::new();
    // A chain of containers ending in a folder where the space spreads out.
    file(&harness.tree, "a/b/c/part-one.bin", 600_000);
    file(&harness.tree, "a/b/c/part-two.bin", 500_000);
    // An unrelated folder, so the list has something to compare against.
    file(&harness.tree, "other/single.bin", 100_000);

    let summary = harness.scan();
    let folders = harness
        .service
        .snapshot(summary.scan_id)
        .unwrap()
        .largest_folders;
    let names: Vec<&str> = folders.iter().map(|row| row.name.as_str()).collect();

    assert!(
        names.contains(&"c"),
        "the folder where the space spreads out is listed"
    );
    assert!(!names.contains(&"a"), "a pass-through container is not");
    assert!(!names.contains(&"b"), "nor is the next one down");
    assert!(names.contains(&"other"));
}

#[test]
fn a_directory_reports_its_subtree_total_while_a_file_reports_itself() {
    let harness = Harness::new();
    file(&harness.tree, "folder/inner.bin", 900);
    file(&harness.tree, "loose.bin", 300);

    let summary = harness.scan();
    let root = harness.service.snapshot(summary.scan_id).unwrap().root;

    assert_eq!(
        harness.child_named(summary.scan_id, root.id, "folder").size,
        900
    );
    assert_eq!(
        harness
            .child_named(summary.scan_id, root.id, "loose.bin")
            .size,
        300
    );
}
