//! The safety rules of the cleanup service.
//!
//! These are the most important tests in the project. Everything else being
//! wrong costs a wrong number on a screen; this being wrong costs somebody
//! their files. The removal path is exercised through a remover that moves
//! files into a temporary directory, so a bug in a test can never destroy
//! anything outside it.

mod common;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use common::{file, Harness};
use diskmate_core::db;
use diskmate_core::model::{
    CandidateKind, CandidateState, CleanupCandidate, Confirmation, SafetyClass,
};
use diskmate_core::services::{CleanupRules, CleanupService, DryRunRemover, Remover};

/// Moves files into a temporary directory instead of the real Trash, so the
/// success path is genuinely exercised without touching the host's Trash.
struct TempTrash {
    root: PathBuf,
    moved: Mutex<Vec<PathBuf>>,
}

impl TempTrash {
    fn new(root: PathBuf) -> Self {
        Self {
            root,
            moved: Mutex::new(Vec::new()),
        }
    }
}

impl Remover for TempTrash {
    fn move_to_trash(&self, path: &Path) -> Result<(), String> {
        let name = path.file_name().ok_or("no file name")?;
        let target = self.root.join(name);
        std::fs::rename(path, &target).map_err(|e| e.to_string())?;
        self.moved.lock().unwrap().push(target);
        Ok(())
    }

    fn name(&self) -> &'static str {
        "temp-trash"
    }
}

fn candidate(path: &Path, bytes: i64, safety: SafetyClass) -> CleanupCandidate {
    CleanupCandidate {
        id: 0,
        scan_id: 1,
        entry_id: None,
        path: path.to_string_lossy().into_owned(),
        name: path.file_name().unwrap().to_string_lossy().into_owned(),
        kind: CandidateKind::Cache,
        safety,
        rule: "test".into(),
        reason: "a test candidate".into(),
        detail: None,
        bytes,
        modified_at: None,
        state: CandidateState::Proposed,
        group_key: None,
    }
}

/// A service, an index, and a scan row the candidates can hang off.
fn fixture() -> (tempfile::TempDir, rusqlite::Connection) {
    let dir = tempfile::tempdir().unwrap();
    let conn = db::open(&dir.path().join("index.sqlite3")).unwrap();
    conn.execute(
        "INSERT INTO scans (id, root_path, status, started_at) VALUES (1, '/tmp', 'completed', 0)",
        [],
    )
    .unwrap();
    (dir, conn)
}

#[test]
fn a_protected_candidate_is_never_removed() {
    let (dir, conn) = fixture();
    let target = file(dir.path(), "precious.txt", 1234);

    // Staged by hand, bypassing the staging guard, to prove the refusal is
    // enforced at removal and not only at staging.
    CleanupService::record_candidates(&conn, &[candidate(&target, 1234, SafetyClass::Review)])
        .unwrap();
    conn.execute(
        "UPDATE cleanup_candidates SET state = 'staged', safety = ?1",
        [SafetyClass::Protected.as_i64()],
    )
    .unwrap();

    let staged = CleanupService::staged(&conn).unwrap();
    let service = CleanupService::with_remover(Arc::new(DryRunRemover::default()));
    let report = service
        .remove_approved(
            &conn,
            &Confirmation {
                candidate_ids: staged.iter().map(|c| c.id).collect(),
                expected_total_bytes: staged.iter().map(|c| c.bytes).sum(),
            },
        )
        .unwrap();

    assert_eq!(report.removed_count, 0);
    assert_eq!(report.refused_count, 1);
    assert!(target.exists(), "the file must still be there");
    assert!(report.outcomes[0]
        .detail
        .as_ref()
        .unwrap()
        .contains("protects"));
}

#[test]
fn a_candidate_in_a_protected_location_is_refused_even_if_marked_safe() {
    let (_dir, conn) = fixture();
    // Marked Safe by a hypothetical buggy rule, but living somewhere DiskMate
    // protects. The location wins.
    let sneaky = candidate(
        Path::new("/Users/ada/Documents/thesis.pdf"),
        99,
        SafetyClass::Safe,
    );
    conn.execute(
        "INSERT INTO cleanup_candidates
             (scan_id, rule, safety, reason, reclaimable_bytes, state, created_at, path, kind)
         VALUES (1, 'test', 0, 'r', 99, 'staged', 0, ?1, 'cache')",
        [&sneaky.path],
    )
    .unwrap();

    let staged = CleanupService::staged(&conn).unwrap();
    let service = CleanupService::with_remover(Arc::new(DryRunRemover::default()));
    let report = service
        .remove_approved(
            &conn,
            &Confirmation {
                candidate_ids: vec![staged[0].id],
                expected_total_bytes: 99,
            },
        )
        .unwrap();

    assert_eq!(report.refused_count, 1);
    assert!(report.outcomes[0]
        .detail
        .as_ref()
        .unwrap()
        .contains("protects"));
}

#[test]
fn recording_a_protected_path_is_dropped_before_it_reaches_the_queue() {
    let (_dir, conn) = fixture();
    let written = CleanupService::record_candidates(
        &conn,
        &[
            candidate(Path::new("/System/Library/Thing"), 10, SafetyClass::Safe),
            candidate(
                Path::new("/Users/ada/Library/Caches/ok"),
                20,
                SafetyClass::Safe,
            ),
        ],
    )
    .unwrap();
    assert_eq!(written, 1, "the protected path is never even proposed");
}

#[test]
fn staging_refuses_protected_candidates() {
    let (_dir, conn) = fixture();
    conn.execute(
        "INSERT INTO cleanup_candidates
             (id, scan_id, rule, safety, reason, reclaimable_bytes, state, created_at, path, kind)
         VALUES (7, 1, 'test', 2, 'r', 10, 'proposed', 0, '/tmp/x/y/z', 'cache')",
        [],
    )
    .unwrap();
    assert_eq!(CleanupService::stage(&conn, &[7]).unwrap(), 0);
    assert!(CleanupService::staged(&conn).unwrap().is_empty());
}

#[test]
fn an_approval_whose_total_does_not_match_refuses_everything() {
    let (dir, conn) = fixture();
    let a = file(dir.path(), "a.bin", 1000);
    let b = file(dir.path(), "b.bin", 2000);
    CleanupService::record_candidates(
        &conn,
        &[
            candidate(&a, 1000, SafetyClass::Safe),
            candidate(&b, 2000, SafetyClass::Safe),
        ],
    )
    .unwrap();
    let ids: Vec<i64> = CleanupService::candidates(&conn, 1, None, 100)
        .unwrap()
        .iter()
        .map(|c| c.id)
        .collect();
    CleanupService::stage(&conn, &ids).unwrap();

    let trash = tempfile::tempdir().unwrap();
    let service = CleanupService::with_remover(Arc::new(TempTrash::new(trash.path().into())));

    // The user saw 3000 bytes; claim they approved 2500.
    let error = service
        .remove_approved(
            &conn,
            &Confirmation {
                candidate_ids: ids.clone(),
                expected_total_bytes: 2500,
            },
        )
        .expect_err("a mismatched total must refuse");
    assert!(error.to_string().contains("review it again"));
    assert!(
        a.exists() && b.exists(),
        "nothing may be removed on a mismatch"
    );
}

#[test]
fn an_approval_that_misses_part_of_the_queue_refuses_everything() {
    let (dir, conn) = fixture();
    let a = file(dir.path(), "a.bin", 1000);
    let b = file(dir.path(), "b.bin", 2000);
    CleanupService::record_candidates(
        &conn,
        &[
            candidate(&a, 1000, SafetyClass::Safe),
            candidate(&b, 2000, SafetyClass::Safe),
        ],
    )
    .unwrap();
    let ids: Vec<i64> = CleanupService::candidates(&conn, 1, None, 100)
        .unwrap()
        .iter()
        .map(|c| c.id)
        .collect();
    CleanupService::stage(&conn, &ids).unwrap();

    let trash = tempfile::tempdir().unwrap();
    let service = CleanupService::with_remover(Arc::new(TempTrash::new(trash.path().into())));

    // Approving only one of the two staged items: the queue the user reviewed
    // is not the queue that exists.
    let error = service
        .remove_approved(
            &conn,
            &Confirmation {
                candidate_ids: vec![ids[0]],
                expected_total_bytes: 1000,
            },
        )
        .expect_err("a partial approval must refuse");
    assert!(error.to_string().contains("changed since you reviewed"));
    assert!(a.exists() && b.exists());
}

#[test]
fn an_approval_naming_something_not_staged_is_refused() {
    let (_dir, conn) = fixture();
    let service = CleanupService::with_remover(Arc::new(DryRunRemover::default()));
    let error = service
        .remove_approved(
            &conn,
            &Confirmation {
                candidate_ids: vec![4242],
                expected_total_bytes: 0,
            },
        )
        .expect_err("an unknown id must refuse");
    assert!(error.to_string().contains("no longer in the cleanup queue"));
}

#[test]
fn a_file_that_changed_after_staging_is_refused() {
    let (dir, conn) = fixture();
    let target = file(dir.path(), "growing.log", 1000);
    CleanupService::record_candidates(&conn, &[candidate(&target, 1000, SafetyClass::Safe)])
        .unwrap();
    let ids: Vec<i64> = CleanupService::candidates(&conn, 1, None, 10)
        .unwrap()
        .iter()
        .map(|c| c.id)
        .collect();
    CleanupService::stage(&conn, &ids).unwrap();

    // Something appended to it between review and approval.
    std::fs::write(&target, vec![b'x'; 5000]).unwrap();

    let trash = tempfile::tempdir().unwrap();
    let service = CleanupService::with_remover(Arc::new(TempTrash::new(trash.path().into())));
    let report = service
        .remove_approved(
            &conn,
            &Confirmation {
                candidate_ids: ids,
                expected_total_bytes: 1000,
            },
        )
        .unwrap();

    assert_eq!(report.refused_count, 1);
    assert!(target.exists());
    assert!(report.outcomes[0]
        .detail
        .as_ref()
        .unwrap()
        .contains("changed since you reviewed"));
}

#[test]
fn a_file_that_vanished_is_recorded_rather_than_reported_as_removed() {
    let (dir, conn) = fixture();
    let target = file(dir.path(), "gone.bin", 1000);
    CleanupService::record_candidates(&conn, &[candidate(&target, 1000, SafetyClass::Safe)])
        .unwrap();
    let ids: Vec<i64> = CleanupService::candidates(&conn, 1, None, 10)
        .unwrap()
        .iter()
        .map(|c| c.id)
        .collect();
    CleanupService::stage(&conn, &ids).unwrap();
    std::fs::remove_file(&target).unwrap();

    let trash = tempfile::tempdir().unwrap();
    let service = CleanupService::with_remover(Arc::new(TempTrash::new(trash.path().into())));
    let report = service
        .remove_approved(
            &conn,
            &Confirmation {
                candidate_ids: ids,
                expected_total_bytes: 1000,
            },
        )
        .unwrap();

    assert_eq!(report.removed_count, 0);
    assert_eq!(report.outcomes[0].state, CandidateState::Vanished);
    assert_eq!(
        report.removed_bytes, 0,
        "vanished bytes are never claimed as reclaimed"
    );
}

#[test]
fn an_approved_removal_moves_the_file_and_is_audited() {
    let (dir, conn) = fixture();
    let target = file(dir.path(), "cache.bin", 4096);
    CleanupService::record_candidates(&conn, &[candidate(&target, 4096, SafetyClass::Safe)])
        .unwrap();
    let ids: Vec<i64> = CleanupService::candidates(&conn, 1, None, 10)
        .unwrap()
        .iter()
        .map(|c| c.id)
        .collect();
    CleanupService::stage(&conn, &ids).unwrap();

    let trash = tempfile::tempdir().unwrap();
    let remover = Arc::new(TempTrash::new(trash.path().into()));
    let service = CleanupService::with_remover(remover.clone());
    let report = service
        .remove_approved(
            &conn,
            &Confirmation {
                candidate_ids: ids,
                expected_total_bytes: 4096,
            },
        )
        .unwrap();

    assert_eq!(report.removed_count, 1);
    assert_eq!(report.removed_bytes, 4096);
    assert!(!target.exists(), "it left the original location");
    assert!(
        trash.path().join("cache.bin").exists(),
        "it is recoverable, not destroyed"
    );

    let history = CleanupService::history(&conn, 10).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].state, CandidateState::Removed);
}

#[test]
fn paths_too_close_to_the_root_are_refused() {
    let (_dir, conn) = fixture();
    for path in ["/", "/Volumes", "/tmp"] {
        conn.execute("DELETE FROM cleanup_candidates", []).unwrap();
        conn.execute(
            "INSERT INTO cleanup_candidates
                 (scan_id, rule, safety, reason, reclaimable_bytes, state, created_at, path, kind)
             VALUES (1, 'test', 0, 'r', 1, 'staged', 0, ?1, 'cache')",
            [path],
        )
        .unwrap();
        let staged = CleanupService::staged(&conn).unwrap();
        let service = CleanupService::with_remover(Arc::new(DryRunRemover::default()));
        let report = service
            .remove_approved(
                &conn,
                &Confirmation {
                    candidate_ids: vec![staged[0].id],
                    expected_total_bytes: 1,
                },
            )
            .unwrap();
        assert_eq!(report.refused_count, 1, "{path} must be refused");
    }
}

#[test]
fn rules_never_propose_anything_protected() {
    let harness = Harness::new();
    file(&harness.tree, "Library/Caches/SomeApp/blob.bin", 4_000_000);
    file(
        &harness.tree,
        "code/project/node_modules/pkg/index.js",
        3_000_000,
    );
    let summary = harness.scan();

    let dir = tempfile::tempdir().unwrap();
    let conn = db::open(&dir.path().join("i.sqlite3")).unwrap();
    let _ = conn;

    // Run the rules against the real index the harness just built.
    let proposed = harness
        .service
        .propose_cleanup(summary.scan_id)
        .expect("rules should run against a completed scan");

    for candidate in &proposed {
        assert!(
            !diskmate_core::platform::is_protected_path(std::path::Path::new(&candidate.path)),
            "{} is protected and must never be proposed",
            candidate.path
        );
        assert!(
            !candidate.reason.is_empty(),
            "every candidate carries a reason"
        );
        assert_ne!(candidate.safety, SafetyClass::Protected);
    }
    let _ = CleanupRules::propose;
}

#[test]
fn a_removed_candidate_stops_being_offered() {
    let (dir, conn) = fixture();
    let trash = tempfile::tempdir().unwrap();
    let going = file(dir.path(), "going.bin", 2_000_000);
    let staying = file(dir.path(), "staying.bin", 3_000_000);

    CleanupService::record_candidates(
        &conn,
        &[
            candidate(&going, 2_000_000, SafetyClass::Safe),
            candidate(&staying, 3_000_000, SafetyClass::Safe),
        ],
    )
    .unwrap();

    let all = CleanupService::candidates(&conn, 1, None, 100).unwrap();
    assert_eq!(all.len(), 2);
    let doomed = all
        .iter()
        .find(|c| c.path == going.to_string_lossy())
        .expect("the candidate we are about to remove");

    CleanupService::stage(&conn, &[doomed.id]).unwrap();
    let service =
        CleanupService::with_remover(Arc::new(TempTrash::new(trash.path().to_path_buf())));
    let report = service
        .remove_approved(
            &conn,
            &Confirmation {
                candidate_ids: vec![doomed.id],
                expected_total_bytes: doomed.bytes,
            },
        )
        .unwrap();
    assert_eq!(report.removed_count, 1);

    // The point of the test: what is gone is not still on offer, and the
    // listing agrees with the group counts rather than contradicting them.
    let remaining = CleanupService::candidates(&conn, 1, None, 100).unwrap();
    assert_eq!(remaining.len(), 1, "the removed candidate must be gone");
    assert_eq!(remaining[0].path, staying.to_string_lossy());

    let groups = CleanupService::groups(&conn, 1).unwrap();
    let counted: i64 = groups.iter().map(|g| g.count).sum();
    assert_eq!(
        counted,
        remaining.len() as i64,
        "the group counts and the listing must describe the same set"
    );

    // Re-proposing must not resurrect it: the decision has been acted on.
    CleanupService::record_candidates(&conn, &[candidate(&going, 2_000_000, SafetyClass::Safe)])
        .unwrap();
    let after = CleanupService::candidates(&conn, 1, None, 100).unwrap();
    assert_eq!(after.len(), 1, "a removed candidate is never re-proposed");
}

/// Moves in batches like the real Trash, but its batch request always fails
/// and one named file never moves, so the per-item fallback is exercised.
struct StubbornTrash {
    inner: TempTrash,
    batches: Mutex<usize>,
}

impl Remover for StubbornTrash {
    fn move_to_trash(&self, path: &Path) -> Result<(), String> {
        if path.file_name().is_some_and(|n| n == "stuck.bin") {
            return Err("it is in use".into());
        }
        self.inner.move_to_trash(path)
    }

    fn move_all_to_trash(&self, _paths: &[PathBuf]) -> Result<(), String> {
        *self.batches.lock().unwrap() += 1;
        Err("one of these could not be moved".into())
    }

    fn name(&self) -> &'static str {
        "stubborn-trash"
    }
}

#[test]
fn a_batch_that_fails_falls_back_to_one_at_a_time_and_reports_progress() {
    let (dir, conn) = fixture();
    let mut candidates = Vec::new();
    for index in 0..29 {
        let path = file(dir.path(), &format!("item-{index}.bin"), 100);
        candidates.push(candidate(&path, 100, SafetyClass::Safe));
    }
    let stuck = file(dir.path(), "stuck.bin", 100);
    candidates.push(candidate(&stuck, 100, SafetyClass::Safe));
    CleanupService::record_candidates(&conn, &candidates).unwrap();
    let ids: Vec<i64> = CleanupService::candidates(&conn, 1, None, 100)
        .unwrap()
        .iter()
        .map(|c| c.id)
        .collect();
    CleanupService::stage(&conn, &ids).unwrap();

    let trash = tempfile::tempdir().unwrap();
    let remover = Arc::new(StubbornTrash {
        inner: TempTrash::new(trash.path().into()),
        batches: Mutex::new(0),
    });
    let service = CleanupService::with_remover(remover.clone());
    let mut seen = Vec::new();
    let report = service
        .remove_approved_with_progress(
            &conn,
            &Confirmation {
                candidate_ids: ids.clone(),
                expected_total_bytes: 3000,
            },
            |progress| seen.push((progress.done, progress.done_bytes)),
        )
        .unwrap();

    assert_eq!(report.removed_count, 29);
    assert_eq!(report.failed_count, 1);
    assert!(
        stuck.exists(),
        "the file that would not move is still there"
    );
    assert_eq!(
        *remover.batches.lock().unwrap(),
        2,
        "30 items go in two batches"
    );
    assert_eq!(seen.first(), Some(&(0, 0)));
    assert_eq!(seen.last(), Some(&(30, 3000)));
    assert!(
        seen.windows(2).all(|pair| pair[0].0 <= pair[1].0),
        "progress only moves forward"
    );
}
