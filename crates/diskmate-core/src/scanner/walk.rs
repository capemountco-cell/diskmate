//! Parallel filesystem traversal.
//!
//! ## Shape
//!
//! A fixed pool of worker threads shares a LIFO stack of directories. LIFO is
//! deliberate: popping the most recently discovered directory keeps traversal
//! depth-first-ish, which bounds the pending frontier to roughly
//! `depth x branching` instead of the whole tree. A FIFO queue on a real volume
//! grows to hold hundreds of thousands of paths before it shrinks.
//!
//! Each worker reads one directory completely, stats its children, and emits a
//! single [`DirBatch`] into a bounded channel. Bounded is what gives the
//! pipeline backpressure: if SQLite falls behind, the walkers block instead of
//! buffering the filesystem in RAM.
//!
//! ## Safety rules
//!
//! * **Symlinks are never followed.** Every `stat` is an `lstat`, so a symlink
//!   is recorded as a symlink and traversal stops there. Symlink cycles are
//!   therefore structurally impossible rather than defended against.
//! * **Directory identity is tracked.** Hard-linked directories, firmlinks and
//!   bind mounts can still produce a cycle without any symlink, so each
//!   directory's `(device, inode)` is recorded before descending.
//! * **One unreadable directory never fails a scan.** The error is recorded and
//!   the directory is marked inaccessible, which is not the same as empty.

use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use crossbeam_channel::Sender;

use crate::model::{Category, EntryKind, EntryRecord, ScanError, ScanErrorKind, ScanOptions};

use super::aggregate::{DirIndex, DirListing};
use super::cancel::CancellationToken;
use super::meta::{build_record, entry_kind, raw_meta, RawMeta};
use super::progress::ProgressCounters;

/// Everything one directory contributes to the index, delivered as one message.
#[derive(Debug)]
pub struct DirBatch {
    pub listing: DirListing,
    pub entries: Vec<EntryRecord>,
    pub errors: Vec<ScanError>,
    /// Set when this directory was enqueued as readable but `read_dir` then
    /// failed. The parent already wrote its row optimistically, so the writer
    /// corrects `accessible` for this id at the end of the scan.
    pub unreadable_entry_id: Option<i64>,
}

/// A directory waiting to be read.
#[derive(Debug)]
struct DirTask {
    path: PathBuf,
    entry_id: i64,
    index: DirIndex,
    /// Aggregator index of the directory that discovered this one.
    parent_index: Option<DirIndex>,
    depth: i32,
    category: Category,
}

/// The shared work stack plus the bookkeeping needed to detect completion.
struct WorkQueue {
    inner: Mutex<QueueState>,
    ready: Condvar,
}

struct QueueState {
    stack: Vec<DirTask>,
    /// Workers currently reading a directory. Traversal is finished when the
    /// stack is empty *and* nobody is still producing new work.
    active: usize,
    shutdown: bool,
}

impl WorkQueue {
    fn new() -> Self {
        Self {
            inner: Mutex::new(QueueState {
                stack: Vec::new(),
                active: 0,
                shutdown: false,
            }),
            ready: Condvar::new(),
        }
    }

    fn push_many(&self, tasks: Vec<DirTask>) {
        if tasks.is_empty() {
            return;
        }
        let mut state = self.inner.lock().expect("work queue poisoned");
        let woken = tasks.len();
        state.stack.extend(tasks);
        drop(state);
        if woken > 1 {
            self.ready.notify_all();
        } else {
            self.ready.notify_one();
        }
    }

    /// Take the next directory, or `None` once traversal is complete.
    ///
    /// `active` is incremented while still holding the lock so that a worker
    /// cannot observe an empty stack in the window between another worker
    /// popping its task and pushing that task's children.
    fn pop(&self) -> Option<DirTask> {
        let mut state = self.inner.lock().expect("work queue poisoned");
        loop {
            if state.shutdown {
                return None;
            }
            if let Some(task) = state.stack.pop() {
                state.active += 1;
                return Some(task);
            }
            if state.active == 0 {
                state.shutdown = true;
                self.ready.notify_all();
                return None;
            }
            state = self.ready.wait(state).expect("work queue poisoned");
        }
    }

    fn finish_task(&self) {
        let mut state = self.inner.lock().expect("work queue poisoned");
        state.active -= 1;
        if state.active == 0 && state.stack.is_empty() {
            state.shutdown = true;
            self.ready.notify_all();
        }
    }

    /// Stop the pool early. Workers already inside a directory finish that
    /// directory, so the index is left at a directory boundary.
    fn shutdown(&self) {
        let mut state = self.inner.lock().expect("work queue poisoned");
        state.shutdown = true;
        state.stack.clear();
        self.ready.notify_all();
    }
}

/// State every worker shares for the duration of a scan.
struct WalkShared {
    queue: WorkQueue,
    sender: Sender<DirBatch>,
    cancel: CancellationToken,
    counters: Arc<ProgressCounters>,
    next_entry_id: AtomicI64,
    next_dir_index: AtomicU32,
    /// Directories already descended into, by `(device, inode)`.
    visited_dirs: Mutex<HashSet<(i64, i64)>>,
    /// Inodes of multi-link files already counted. Only files with `nlink > 1`
    /// are inserted, so on a typical volume this set stays near empty.
    seen_hardlinks: Mutex<HashSet<(i64, i64)>>,
    skip_paths: HashSet<PathBuf>,
    root_device: i64,
    cross_device: bool,
    max_depth: Option<i32>,
}

/// Result of starting a walk: the root row, plus the threads to join.
///
/// Deliberately does **not** hold the shared state. The channel's sender lives
/// inside that state, so a handle keeping one alive would mean the consumer's
/// `recv` loop never observed the end of the stream, and the scan would hang
/// once the last directory had been read.
pub struct WalkHandle {
    pub root_record: EntryRecord,
    pub root_index: DirIndex,
    workers: Vec<std::thread::JoinHandle<()>>,
}

impl WalkHandle {
    /// Block until every worker has exited.
    pub fn join(self) {
        for worker in self.workers {
            let _ = worker.join();
        }
    }
}

/// Prepare the root row and start the worker pool.
///
/// Returns an error only when the root itself cannot be used; every failure
/// below the root is reported through the batch stream instead.
pub fn start(
    options: &ScanOptions,
    root_entry_id: i64,
    sender: Sender<DirBatch>,
    cancel: CancellationToken,
    counters: Arc<ProgressCounters>,
) -> crate::Result<WalkHandle> {
    let root = options
        .root
        .canonicalize()
        .map_err(|e| crate::CoreError::io(&options.root, e))?;
    let metadata = std::fs::symlink_metadata(&root).map_err(|e| crate::CoreError::io(&root, e))?;
    if !metadata.is_dir() {
        return Err(crate::CoreError::NotADirectory(root));
    }

    let meta = raw_meta(&metadata);
    let root_path = root.to_string_lossy().into_owned();
    let root_name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root_path.clone());
    let root_category = crate::model::classify_directory(&root_name, &root_path, Category::Unknown);

    let root_record = build_record(
        root_entry_id,
        None,
        root_name,
        Some(root_path),
        0,
        EntryKind::Directory,
        meta,
        root_category,
        false,
        true,
    );

    let shared = Arc::new(WalkShared {
        queue: WorkQueue::new(),
        sender,
        cancel,
        counters,
        next_entry_id: AtomicI64::new(root_entry_id + 1),
        next_dir_index: AtomicU32::new(1),
        visited_dirs: Mutex::new(HashSet::from([(meta.device, meta.inode)])),
        seen_hardlinks: Mutex::new(HashSet::new()),
        skip_paths: options.skip_paths.iter().cloned().collect(),
        root_device: meta.device,
        cross_device: options.cross_device,
        max_depth: options.max_depth,
    });

    shared.queue.push_many(vec![DirTask {
        path: root,
        entry_id: root_entry_id,
        index: 0,
        parent_index: None,
        depth: 0,
        category: root_category,
    }]);
    shared.counters.directories.fetch_add(1, Ordering::Relaxed);

    let thread_count = options.resolved_threads();
    let mut workers = Vec::with_capacity(thread_count);
    for id in 0..thread_count {
        let shared = Arc::clone(&shared);
        workers.push(
            std::thread::Builder::new()
                .name(format!("diskmate-walk-{id}"))
                .spawn(move || worker_loop(shared))
                .map_err(crate::CoreError::PlainIo)?,
        );
    }

    // Dropping this last reference is what lets the sender die with the final
    // worker, which is how the consumer learns the walk is over.
    drop(shared);

    Ok(WalkHandle {
        root_record,
        root_index: 0,
        workers,
    })
}

fn worker_loop(shared: Arc<WalkShared>) {
    while let Some(task) = shared.queue.pop() {
        if shared.cancel.is_cancelled() {
            shared.queue.finish_task();
            shared.queue.shutdown();
            break;
        }
        let children = read_directory(&shared, &task);
        // Children must be queued *before* this task is marked finished.
        // Otherwise `active` can reach zero between the two calls and the queue
        // would declare the walk complete while children are still pending.
        shared.queue.push_many(children);
        shared.queue.finish_task();
    }
}

/// Read one directory and emit its batch. Returns the child directories to
/// enqueue.
fn read_directory(shared: &Arc<WalkShared>, task: &DirTask) -> Vec<DirTask> {
    shared
        .counters
        .set_current_path(&task.path.to_string_lossy());

    let read_dir = match std::fs::read_dir(&task.path) {
        Ok(iter) => iter,
        Err(err) => {
            // The directory exists but we cannot look inside. Report it as
            // inaccessible: its subtree total stays zero and the UI says so,
            // rather than claiming the folder is empty.
            let error = scan_error(&task.path, &err);
            shared.counters.errors.fetch_add(1, Ordering::Relaxed);
            send(
                shared,
                DirBatch {
                    listing: DirListing {
                        index: task.index,
                        entry_id: task.entry_id,
                        parent: task.parent_index,
                        ..DirListing::default()
                    },
                    entries: Vec::new(),
                    errors: vec![error],
                    unreadable_entry_id: Some(task.entry_id),
                },
            );
            return Vec::new();
        }
    };

    let mut staged: Vec<StagedEntry> = Vec::new();
    let mut errors: Vec<ScanError> = Vec::new();
    let mut child_dirs = 0u32;
    let mut own_logical = 0i64;
    let mut own_allocated = 0i64;
    let mut own_files = 0i64;

    for dirent in read_dir {
        if shared.cancel.is_cancelled() {
            break;
        }
        let dirent = match dirent {
            Ok(entry) => entry,
            Err(err) => {
                errors.push(scan_error(&task.path, &err));
                continue;
            }
        };
        let name_os = dirent.file_name();
        let child_path = task.path.join(&name_os);
        let name = os_to_string(&name_os);

        // `DirEntry::metadata` is an `lstat` on Unix: it never follows a
        // symlink, which is the whole basis of the loop-safety story.
        let metadata = match dirent.metadata() {
            Ok(metadata) => metadata,
            Err(err) => {
                // Entries vanish between `read_dir` and `stat` on a live
                // system. Record the name so it is not silently lost.
                let mut error = scan_error(&child_path, &err);
                if error.kind == ScanErrorKind::NotFound {
                    error.kind = ScanErrorKind::Vanished;
                }
                errors.push(error);
                staged.push(StagedEntry {
                    name,
                    kind: EntryKind::Other,
                    meta: RawMeta::default(),
                    category: task.category,
                    hardlink_dup: false,
                    accessible: false,
                    descend: false,
                });
                continue;
            }
        };

        let kind = entry_kind(metadata.file_type());
        let meta = raw_meta(&metadata);

        match kind {
            EntryKind::Directory => {
                let descend = should_descend(shared, &child_path, meta, task.depth, &mut errors);
                let category = crate::model::classify_directory(
                    &name,
                    &child_path.to_string_lossy(),
                    task.category,
                );
                if descend {
                    child_dirs += 1;
                }
                staged.push(StagedEntry {
                    name,
                    kind,
                    meta,
                    category,
                    hardlink_dup: false,
                    accessible: descend,
                    descend,
                });
            }
            _ => {
                let hardlink_dup = kind == EntryKind::File
                    && meta.nlink > 1
                    && !claim_hardlink(shared, meta.device, meta.inode);
                if kind == EntryKind::File {
                    // Every name is counted. Only the bytes of a hard-link
                    // repeat are excluded, so counts stay consistent with what
                    // the user can see in Finder.
                    own_files += 1;
                    if !hardlink_dup {
                        own_logical += meta.logical_size;
                        own_allocated += meta.allocated_size;
                    }
                } else {
                    // A symlink's `lstat` size is its target path length, which
                    // is not storage. Its allocated blocks, if any, are.
                    own_allocated += meta.allocated_size;
                }
                staged.push(StagedEntry {
                    name,
                    kind,
                    meta,
                    category: task.category,
                    hardlink_dup,
                    accessible: true,
                    descend: false,
                });
            }
        }
    }

    // Ids are handed out in one block per directory rather than one at a time,
    // so a scan costs two atomic increments per directory instead of one per
    // entry.
    let first_id = shared
        .next_entry_id
        .fetch_add(staged.len() as i64, Ordering::Relaxed);
    let first_dir_index = shared
        .next_dir_index
        .fetch_add(child_dirs, Ordering::Relaxed);

    let mut entries = Vec::with_capacity(staged.len());
    let mut next_tasks = Vec::with_capacity(child_dirs as usize);
    let mut dir_cursor = first_dir_index;
    let mut files = 0u64;
    let mut dirs = 0u64;
    let mut symlinks = 0u64;
    let mut others = 0u64;

    for (offset, item) in staged.into_iter().enumerate() {
        let id = first_id + offset as i64;
        let child_path = task.path.join(&item.name);
        let path =
            (item.kind == EntryKind::Directory).then(|| child_path.to_string_lossy().into_owned());

        match item.kind {
            EntryKind::File => files += 1,
            EntryKind::Directory => dirs += 1,
            EntryKind::Symlink => symlinks += 1,
            EntryKind::Other => others += 1,
        }

        if item.descend {
            next_tasks.push(DirTask {
                path: child_path,
                entry_id: id,
                index: dir_cursor,
                parent_index: Some(task.index),
                depth: task.depth + 1,
                category: item.category,
            });
            dir_cursor += 1;
        }

        entries.push(build_record(
            id,
            Some(task.entry_id),
            item.name,
            path,
            task.depth + 1,
            item.kind,
            item.meta,
            item.category,
            item.hardlink_dup,
            item.accessible,
        ));
    }

    shared.counters.files.fetch_add(files, Ordering::Relaxed);
    // The root counted itself at start-up; here we count directories we will
    // actually descend into plus the ones we recorded but skipped.
    shared
        .counters
        .directories
        .fetch_add(dirs, Ordering::Relaxed);
    shared
        .counters
        .symlinks
        .fetch_add(symlinks, Ordering::Relaxed);
    shared.counters.others.fetch_add(others, Ordering::Relaxed);
    shared
        .counters
        .bytes
        .fetch_add(own_logical.max(0) as u64, Ordering::Relaxed);
    if !errors.is_empty() {
        shared
            .counters
            .errors
            .fetch_add(errors.len() as u64, Ordering::Relaxed);
    }

    send(
        shared,
        DirBatch {
            listing: DirListing {
                index: task.index,
                entry_id: task.entry_id,
                parent: task.parent_index,
                own_logical,
                own_allocated,
                own_files,
                child_dirs,
            },
            entries,
            errors,
            unreadable_entry_id: None,
        },
    );

    next_tasks
}

struct StagedEntry {
    name: String,
    kind: EntryKind,
    meta: RawMeta,
    category: Category,
    hardlink_dup: bool,
    accessible: bool,
    descend: bool,
}

/// Decide whether to descend into a child directory, recording why not.
fn should_descend(
    shared: &Arc<WalkShared>,
    path: &Path,
    meta: RawMeta,
    depth: i32,
    errors: &mut Vec<ScanError>,
) -> bool {
    if let Some(max) = shared.max_depth {
        if depth + 1 >= max {
            return false;
        }
    }
    if shared.skip_paths.contains(path) {
        return false;
    }
    if !shared.cross_device && meta.device != shared.root_device {
        errors.push(ScanError {
            path: path.to_string_lossy().into_owned(),
            kind: ScanErrorKind::CrossDevice,
            code: None,
            message: "another volume is mounted here; scan it separately".into(),
        });
        return false;
    }
    // Hard-linked directories and bind mounts can reach the same directory by
    // two paths. Claim the identity before descending so the second path stops.
    let mut visited = shared.visited_dirs.lock().expect("visited set poisoned");
    if !visited.insert((meta.device, meta.inode)) {
        drop(visited);
        errors.push(ScanError {
            path: path.to_string_lossy().into_owned(),
            kind: ScanErrorKind::TraversalLoop,
            code: None,
            message: "already visited through another path".into(),
        });
        return false;
    }
    true
}

/// `true` when this is the first path seen for a multi-link inode.
fn claim_hardlink(shared: &Arc<WalkShared>, device: i64, inode: i64) -> bool {
    shared
        .seen_hardlinks
        .lock()
        .expect("hardlink set poisoned")
        .insert((device, inode))
}

fn send(shared: &Arc<WalkShared>, batch: DirBatch) {
    // A send failure means the writer is gone, which only happens when the scan
    // is being torn down; stopping the pool is the correct response.
    if shared.sender.send(batch).is_err() {
        shared.queue.shutdown();
    }
}

fn scan_error(path: &Path, err: &std::io::Error) -> ScanError {
    ScanError {
        path: path.to_string_lossy().into_owned(),
        kind: ScanErrorKind::from_io(err),
        code: err.raw_os_error(),
        message: err.to_string(),
    }
}

/// Filenames are bytes on Unix and need not be valid UTF-8.
///
/// DiskMate stores a lossy rendering rather than dropping the entry: an
/// undecodable name is still a real file taking real space, and the user needs
/// to see it. The original bytes are recoverable from the parent path when an
/// operation ever needs them.
fn os_to_string(name: &OsStr) -> String {
    name.to_string_lossy().into_owned()
}
