//! Duplicate detection, and the things that must never count as a duplicate.

mod common;

use common::{file, Harness};
use diskmate_core::scanner::CancellationToken;

/// Write a file of `size` bytes whose contents are determined by `seed`, so two
/// files can be made same-size-different-content on purpose.
fn seeded(root: &std::path::Path, relative: &str, size: usize, seed: u8) -> std::path::PathBuf {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut bytes = vec![seed; size];
    // Vary the tail too, so a partial digest of both ends can tell them apart.
    if let Some(last) = bytes.last_mut() {
        *last = seed.wrapping_add(1);
    }
    std::fs::write(&path, bytes).unwrap();
    path
}

const BIG: usize = 2_000_000;

#[test]
fn identical_files_are_found_and_the_waste_is_counted() {
    let harness = Harness::new();
    seeded(&harness.tree, "a/report.bin", BIG, 7);
    seeded(&harness.tree, "b/report-copy.bin", BIG, 7);
    seeded(&harness.tree, "c/nested/another.bin", BIG, 7);

    let summary = harness.scan();
    let stats = harness
        .service
        .find_duplicates(summary.scan_id, &CancellationToken::new(), |_, _, _| {})
        .unwrap();

    assert_eq!(stats.groups, 1);
    // Three copies of one file: two of them are waste.
    assert_eq!(stats.reclaimable_bytes, (BIG * 2) as i64);

    let groups = harness
        .service
        .duplicate_groups(summary.scan_id, 10)
        .unwrap();
    assert_eq!(groups[0].member_count, 3);
    assert_eq!(groups[0].members.iter().filter(|m| m.is_keeper).count(), 1);
}

#[test]
fn files_of_the_same_size_with_different_contents_are_not_duplicates() {
    let harness = Harness::new();
    seeded(&harness.tree, "one.bin", BIG, 1);
    seeded(&harness.tree, "two.bin", BIG, 2);

    let summary = harness.scan();
    let stats = harness
        .service
        .find_duplicates(summary.scan_id, &CancellationToken::new(), |_, _, _| {})
        .unwrap();

    assert_eq!(stats.size_candidates, 2, "both were size candidates");
    assert_eq!(stats.groups, 0, "but neither is a duplicate");
    assert_eq!(stats.reclaimable_bytes, 0);
}

#[test]
fn an_identical_name_never_makes_two_files_duplicates() {
    let harness = Harness::new();
    seeded(&harness.tree, "2023/invoice.pdf", BIG, 10);
    seeded(&harness.tree, "2024/invoice.pdf", BIG, 20);

    let summary = harness.scan();
    let stats = harness
        .service
        .find_duplicates(summary.scan_id, &CancellationToken::new(), |_, _, _| {})
        .unwrap();

    assert_eq!(stats.groups, 0, "same name, same size, different bytes");
}

#[cfg(unix)]
#[test]
fn hard_links_are_not_reported_as_duplicates() {
    let harness = Harness::new();
    let original = seeded(&harness.tree, "original.bin", BIG, 3);
    std::fs::hard_link(&original, harness.tree.join("same-file.bin")).unwrap();

    let summary = harness.scan();
    let stats = harness
        .service
        .find_duplicates(summary.scan_id, &CancellationToken::new(), |_, _, _| {})
        .unwrap();

    assert_eq!(
        stats.groups, 0,
        "two names for one inode already share their bytes; removing one frees nothing"
    );
}

#[test]
fn small_files_are_ignored_however_many_copies_exist() {
    let harness = Harness::new();
    for index in 0..20 {
        file(&harness.tree, &format!("icons/icon-{index}.png"), 2_000);
    }
    let summary = harness.scan();
    let stats = harness
        .service
        .find_duplicates(summary.scan_id, &CancellationToken::new(), |_, _, _| {})
        .unwrap();

    assert_eq!(
        stats.groups, 0,
        "a thousand identical icons is true and useless"
    );
}

#[test]
fn the_oldest_copy_is_the_one_kept() {
    let harness = Harness::new();
    let first = seeded(&harness.tree, "first.bin", BIG, 9);
    let second = seeded(&harness.tree, "second.bin", BIG, 9);

    // Make `first` clearly older.
    let old = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
    filetime::set_file_mtime(&first, filetime::FileTime::from_system_time(old)).ok();

    let summary = harness.scan();
    harness
        .service
        .find_duplicates(summary.scan_id, &CancellationToken::new(), |_, _, _| {})
        .unwrap();
    let groups = harness
        .service
        .duplicate_groups(summary.scan_id, 10)
        .unwrap();

    let keeper = groups[0].members.iter().find(|m| m.is_keeper).unwrap();
    assert!(
        keeper.path.ends_with("first.bin"),
        "the oldest copy is kept, got {}",
        keeper.path
    );
    let _ = second;
}

#[test]
fn cleanup_candidates_never_include_the_copy_being_kept() {
    let harness = Harness::new();
    seeded(&harness.tree, "keep/data.bin", BIG, 5);
    seeded(&harness.tree, "extra/data.bin", BIG, 5);
    seeded(&harness.tree, "more/data.bin", BIG, 5);

    let summary = harness.scan();
    harness
        .service
        .find_duplicates(summary.scan_id, &CancellationToken::new(), |_, _, _| {})
        .unwrap();
    let written = harness
        .service
        .propose_duplicate_cleanup(summary.scan_id)
        .unwrap();

    assert_eq!(
        written, 2,
        "three copies produce two candidates, never three"
    );

    let groups = harness
        .service
        .duplicate_groups(summary.scan_id, 10)
        .unwrap();
    let keeper = groups[0].members.iter().find(|m| m.is_keeper).unwrap();
    let candidates = harness
        .service
        .cleanup_candidates(summary.scan_id, None, 100)
        .unwrap();
    assert!(
        candidates.iter().all(|c| c.path != keeper.path),
        "the kept copy is never proposed for removal"
    );
    for candidate in &candidates {
        assert!(candidate.detail.as_ref().unwrap().contains("being kept"));
    }
}

#[test]
fn cancellation_stops_the_search() {
    let harness = Harness::new();
    for index in 0..40 {
        seeded(&harness.tree, &format!("copy-{index}.bin",), BIG, 4);
    }
    let summary = harness.scan();

    let cancel = CancellationToken::new();
    cancel.cancel();
    let stats = harness
        .service
        .find_duplicates(summary.scan_id, &cancel, |_, _, _| {})
        .unwrap();

    assert_eq!(
        stats.groups, 0,
        "an already-cancelled search records nothing"
    );
}
