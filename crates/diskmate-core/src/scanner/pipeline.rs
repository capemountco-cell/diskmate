//! Scan orchestration.
//!
//! ```text
//!   traversal threads  ->  bounded channel  ->  writer thread
//!   (read_dir + lstat)     (backpressure)       (aggregate + batch insert)
//!                                                      |
//!                                              progress sampler -> UI
//! ```
//!
//! The channel is bounded in *directories*, and each directory carries its
//! whole listing, so peak memory is `queue_capacity x average directory size`
//! plus the aggregator. Nothing in the pipeline grows with the size of the
//! volume, which is the property that makes a ten-million-file scan behave the
//! same as a ten-thousand-file one.
//!
//! There is exactly one writer thread. SQLite serialises writers anyway, so a
//! second one would add lock contention and no throughput; the writer is also
//! where aggregation happens, which lets the aggregator be a plain `Vec` with
//! no atomics.

use std::collections::HashMap;
use std::time::Instant;

use rusqlite::Connection;

use crate::db::writer::IndexWriter;
use crate::db::{index_size_bytes, StorageQueries};
use crate::error::Result;
use crate::model::{Category, EntryKind, ScanOptions, ScanProgress, ScanStatus, ScanSummary};
use crate::ENGINE_VERSION;

use super::aggregate::DirAggregator;
use super::cancel::CancellationToken;
use super::progress::{ProgressCounters, ProgressThrottle};
use super::walk;

/// Called with coalesced progress. Must not block: it runs on the writer
/// thread, between database batches.
pub type ProgressSink<'a> = &'a mut dyn FnMut(ScanProgress);

/// Everything a scan needs beyond its options.
pub struct ScanRequest {
    pub options: ScanOptions,
    pub volume_id: Option<i64>,
    pub cancel: CancellationToken,
}

impl ScanRequest {
    pub fn new(options: ScanOptions) -> Self {
        Self {
            options,
            volume_id: None,
            cancel: CancellationToken::new(),
        }
    }
}

/// Run one scan to completion, cancellation, or failure.
///
/// Takes the connection by value and hands it back, so the caller's pool never
/// has two writers open on the index at once.
pub fn run_scan(
    conn: Connection,
    request: &ScanRequest,
    database_path: Option<&std::path::Path>,
    progress: ProgressSink<'_>,
) -> Result<(Connection, ScanSummary)> {
    let options = &request.options;
    let started = Instant::now();

    let options_json = serde_json::to_string(options).unwrap_or_else(|_| "{}".into());
    let root_display = options.root.to_string_lossy().into_owned();
    let scan_id = IndexWriter::begin_scan(
        &conn,
        request.volume_id,
        &root_display,
        &options_json,
        ENGINE_VERSION,
    )?;
    let first_entry_id = IndexWriter::max_entry_id(&conn)? + 1;

    let counters = ProgressCounters::new();
    let (sender, receiver) = crossbeam_channel::bounded::<walk::DirBatch>(options.queue_capacity);

    let handle = match walk::start(
        options,
        first_entry_id,
        sender,
        request.cancel.clone(),
        counters.clone(),
    ) {
        Ok(handle) => handle,
        Err(err) => {
            let writer = IndexWriter::new(conn, scan_id);
            let conn = writer.abort(ScanStatus::Failed);
            let _ = conn;
            return Err(err);
        }
    };

    let mut writer = IndexWriter::new(conn, scan_id);
    let mut aggregator = DirAggregator::new();
    let mut categories: HashMap<Category, (i64, i64)> = HashMap::new();
    let mut extensions: HashMap<String, (i64, i64)> = HashMap::new();
    let mut pending = Vec::with_capacity(options.batch_size);
    let mut unreadable: Vec<i64> = Vec::new();
    let mut throttle = ProgressThrottle::new(options.progress_interval_ms);

    // The root row is written first so that navigation has an anchor even if
    // the scan is cancelled a moment later.
    let root_entry_id = handle.root_record.id;
    let root_index = handle.root_index;
    let mut fatal: Option<crate::CoreError> = None;
    if let Err(err) = writer.write_entries(std::slice::from_ref(&handle.root_record)) {
        fatal = Some(err);
        request.cancel.cancel();
    }

    while let Ok(batch) = receiver.recv() {
        if fatal.is_some() {
            continue; // drain so the walkers can finish and exit cleanly
        }
        aggregator.record_listing(batch.listing);
        if let Some(id) = batch.unreadable_entry_id {
            unreadable.push(id);
        }
        accumulate_breakdowns(&batch.entries, &mut categories, &mut extensions);

        if let Err(err) = writer.write_errors(&batch.errors) {
            fatal = Some(err);
            request.cancel.cancel();
            continue;
        }

        pending.extend(batch.entries);
        if pending.len() >= options.batch_size {
            if let Err(err) = writer.write_entries(&pending) {
                fatal = Some(err);
                request.cancel.cancel();
                continue;
            }
            pending.clear();
        }

        let now = Instant::now();
        if throttle.should_emit(now) {
            progress(sample(scan_id, &counters, started, ScanStatus::Running));
        }
    }

    handle.join();

    if let Some(err) = fatal {
        let conn = writer.abort(ScanStatus::Failed);
        drop(conn);
        return Err(err);
    }

    if !pending.is_empty() {
        writer.write_entries(&pending)?;
        pending.clear();
    }

    let status = if request.cancel.is_cancelled() {
        ScanStatus::Cancelled
    } else {
        ScanStatus::Completed
    };

    writer.apply_directory_aggregates(&aggregator.drain_aggregates())?;
    writer.mark_inaccessible(&unreadable)?;
    writer.write_breakdowns(&categories, &extensions)?;
    writer.create_query_indexes()?;

    let totals = aggregator.root_totals(root_index);
    let duration_ms = started.elapsed().as_millis() as i64;
    let files = counters.files.load(std::sync::atomic::Ordering::Relaxed) as i64;
    let dirs = counters
        .directories
        .load(std::sync::atomic::Ordering::Relaxed) as i64;
    let symlinks = counters.symlinks.load(std::sync::atomic::Ordering::Relaxed) as i64;
    let others = counters.others.load(std::sync::atomic::Ordering::Relaxed) as i64;
    let errors = counters.errors.load(std::sync::atomic::Ordering::Relaxed) as i64;

    // Prefer the aggregator's exact totals; fall back to the running counters
    // when the root never finalised because the scan was cancelled.
    let (logical, allocated) = match totals {
        Some(agg) => (agg.logical, agg.allocated),
        None => (
            counters.bytes.load(std::sync::atomic::Ordering::Relaxed) as i64,
            0,
        ),
    };

    let conn = writer.finish_scan(
        status,
        Some(root_entry_id),
        duration_ms,
        files,
        dirs,
        symlinks,
        others,
        errors,
        logical,
        allocated,
    )?;

    let mut summary = StorageQueries::scan_summary(&conn, scan_id)?;
    summary.index_bytes = database_path.map(index_size_bytes).unwrap_or(0);
    progress(sample(scan_id, &counters, started, status));
    Ok((conn, summary))
}

/// Fold a directory's file rows into the category and extension totals.
///
/// Done here rather than in SQL because an index on `fs_entries.extension`
/// would cost more write throughput on a multi-million-file volume than the
/// breakdown is worth, and these two maps stay small: a few thousand distinct
/// extensions at most.
fn accumulate_breakdowns(
    entries: &[crate::model::EntryRecord],
    categories: &mut HashMap<Category, (i64, i64)>,
    extensions: &mut HashMap<String, (i64, i64)>,
) {
    for entry in entries {
        if entry.kind != EntryKind::File {
            continue;
        }
        // Counted either way; a hard-link repeat contributes no bytes, matching
        // how the directory roll-up treats it.
        let bytes = if entry.hardlink_dup {
            0
        } else {
            entry.logical_size
        };

        let bucket = categories.entry(entry.category).or_insert((0, 0));
        bucket.0 += 1;
        bucket.1 += bytes;

        let key = entry.extension.as_deref().unwrap_or("");
        if let Some(bucket) = extensions.get_mut(key) {
            bucket.0 += 1;
            bucket.1 += bytes;
        } else {
            extensions.insert(key.to_owned(), (1, bytes));
        }
    }
}

fn sample(
    scan_id: i64,
    counters: &ProgressCounters,
    started: Instant,
    status: ScanStatus,
) -> ScanProgress {
    use std::sync::atomic::Ordering::Relaxed;
    let elapsed_ms = started.elapsed().as_millis() as u64;
    let entries = counters.entries();
    ScanProgress {
        scan_id,
        status,
        entries_scanned: entries,
        files_scanned: counters.files.load(Relaxed),
        directories_scanned: counters.directories.load(Relaxed),
        symlinks_scanned: counters.symlinks.load(Relaxed),
        bytes_indexed: counters.bytes.load(Relaxed),
        errors: counters.errors.load(Relaxed),
        elapsed_ms,
        entries_per_second: if elapsed_ms > 0 {
            entries as f64 / (elapsed_ms as f64 / 1000.0)
        } else {
            0.0
        },
        current_path: counters.current_path(),
    }
}
