# The scanner

The scanner is the heart of DiskMate. Everything else in the product is a
consequence of how well it works, so this document records its design, the
reasoning behind each choice, and the numbers actually measured.

## Shape

```
  worker threads              bounded channel            writer thread
  ───────────────             ───────────────            ─────────────
  pop a directory   ──────►   one message per   ──────►  aggregate totals
  read_dir + lstat            directory, with            batch INSERT
  push child dirs             its whole listing          accumulate breakdowns
       ▲                                                       │
       └──────────── backpressure when the writer lags ◄────────┘
                                                               │
                                        progress sampler ──────┴──► UI events
```

Three properties drove this, in order:

1. **Bounded memory.** Nothing grows with the size of the volume. A
   ten-million-file scan has the same memory profile as a ten-thousand-file one.
2. **Backpressure.** When SQLite falls behind, the walkers block. They never
   buffer the filesystem in RAM to keep busy.
3. **Survivability.** One unreadable directory, one file that vanishes
   mid-scan, or one hard-linked directory loop must not fail a scan.

## Traversal

A fixed pool of worker threads shares a **LIFO stack** of directories
(`scanner/walk.rs`). LIFO is the important word. Popping the most recently
discovered directory keeps traversal depth-first-ish, which bounds the pending
frontier to roughly `depth × branching`. A FIFO queue over a real volume grows to
hold hundreds of thousands of paths before it starts shrinking.

Termination is detected with an active-worker count held under the same lock as
the stack: the walk is over when the stack is empty *and* nobody is still
producing work. Children are pushed **before** the producing task is marked
finished, because otherwise the count can reach zero in the window between the
two and the pool shuts down with work pending. That was a real bug, caught by
the integration tests.

Each worker reads one directory completely, stats its children, and emits a
single message carrying the whole listing. Entry ids and aggregator indices are
handed out in one block per directory, so a scan costs two atomic increments per
*directory* rather than one per entry.

### Safety rules

- **Symlinks are never followed.** Every stat is an `lstat`
  (`DirEntry::metadata` is `lstat` on Unix), so a symlink is recorded as a
  symlink and traversal stops. Symlink cycles are therefore *structurally*
  impossible rather than defended against.
- **Directory identity is tracked.** Hard-linked directories, firmlinks and bind
  mounts can produce a cycle with no symlink involved, so each directory's
  `(device, inode)` is claimed before descending. The second path to the same
  directory records a `traversalLoop` error and stops.
- **Mount points are skipped precisely.** The platform layer supplies the mount
  table, so a scan refuses exactly the mounts nested under its root rather than
  refusing every device change. This matters on macOS, where `/` and
  `/System/Volumes/Data` are two APFS volumes joined by firmlinks and treating a
  device change as a boundary would report a Mac as using ten gigabytes.
- **An unreadable directory is marked inaccessible, not empty.** Its subtree
  total stays zero, `accessible` is false, and the UI says "Not readable". The
  scan summary reports how many there were and why.

### What is counted

| Entry | Counted as a name | Bytes attributed |
| --- | --- | --- |
| File | yes | its length |
| Hard-link repeat | yes | **none** — the bytes belong to the first name |
| Directory | yes | its subtree total |
| Symlink | yes | **none** — `lstat` reports the target path's length, which is not storage |
| Socket, FIFO, device | yes | none |

Only files with `nlink > 1` are entered into the hard-link set, so on a typical
volume that set stays near empty. A hard-linked copy still displays its real
size in the UI, labelled *shared copy*; it is the folder roll-up that excludes
it, which is what keeps a folder's total from exceeding the disk.

Allocated size comes from `st_blocks × 512`, which is defined in 512-byte units
on both macOS and Linux regardless of the filesystem's own block size. That is
what makes sparse files and APFS clones report honestly.

## Aggregation

Folder sizes are the one part of scanning that is not embarrassingly parallel: a
folder's total is final only once every descendant has been seen, and
descendants are visited on several threads in no particular order.

`scanner/aggregate.rs` keeps a **completion count per directory**. Each node
records how many child directories it is waiting for; when a node's own listing
has arrived and all its child directories have finalised, its totals roll into
its parent. Rolling up may finalise the parent, so the cascade runs on an
explicit stack rather than by recursion — a 10,000-level chain is a test case.

The aggregator runs on the writer thread, which means nodes need no atomics and
no locks. Contended atomics across eight walker threads measured worse, and the
writer is not the bottleneck.

A node holds counters only, no strings: **about 56 bytes per directory**,
independent of how many files each contains. A volume with ten million files and
two hundred thousand folders costs roughly 11 MB to aggregate. This is why
directory rows are streamed to disk with zero totals and corrected at the end by
one `UPDATE ... FROM` against a temporary table, rather than buffered in memory
until complete.

A cancelled scan still emits partial totals. Reporting a partial number for an
explicitly incomplete scan is honest; dropping the row would make the folder look
empty.

## Cancellation

`CancellationToken` is an `Arc<AtomicBool>` checked between entries within a
directory and between directories. Response time is bounded by one `lstat`, not
by one directory.

On cancel, the walkers stop at a directory boundary, the writer flushes what it
has, aggregates and indexes are still built, and the scan row is marked
`cancelled`. The partial index remains fully navigable and the UI labels it.

A scan left `running` by a process that died is reaped as `interrupted` the next
time the index is opened, so the index never claims a scan is in flight.

## Progress

Workers bump relaxed atomics. A sampler turns them into events at a fixed
cadence (default 150 ms, floor 16 ms), so the UI sees a smooth six-to-ten updates
a second whether the scanner is grinding through one huge file or a million tiny
ones. The current path is sampled once per directory under a mutex, which is
microseconds against the cost of `read_dir`.

Progress is never one event per entry. That would make IPC the bottleneck.

## Measurements

Run `./scripts/bench.sh` to reproduce; results are recorded under
`bench-results/` with the commit and the machine.

**Every number below was measured on a 4-core x86_64 Linux VM**, because that is
the machine this milestone was built on. They are useful for comparing scanner
revisions against each other. They are *not* a claim about macOS performance and
must not be quoted as one until the same benchmark has been run on a Mac.

Synthetic tree: 120,000 files across 3,000 directories, depth 5, 8.3 GB, a long
tail of small files with occasional large ones.

### Threads (batch 2048)

| Threads | Duration | Entries/second |
| ------: | -------: | -------------: |
| 1 | 963 ms | 127,726 |
| 2 | 635 ms | 193,701 |
| 4 | 535 ms | 229,907 |
| 8 | 586 ms | 209,898 |

Scaling is good to the core count and then reverses: eight threads on four cores
adds contention for the directory-identity lock and the channel without adding
throughput. The default is `min(available_parallelism, 8)`.

### Batch size (4 threads)

| Batch | Duration | Entries/second |
| ----: | -------: | -------------: |
| 256 | 1,213 ms | 101,401 |
| 1,024 | 672 ms | 183,036 |
| 2,048 | 636 ms | 193,396 |
| 8,192 | 696 ms | 176,724 |

Small batches pay statement overhead; very large ones hold rows in memory longer
than they help. 2,048 is the default. Run-to-run variance at four threads is
around 15% on this machine, so treat differences smaller than that as noise.

### A real filesystem

Scanning `/` on the same machine through the built application:

| | |
| --- | --- |
| Entries | 217,950 (188,355 files, 29,595 folders) |
| Indexed | 16.2 GB |
| Duration | 3.0 s |
| Rate | 77,053 entries/second |
| Index size | 47.2 MB (~217 bytes per entry) |

A real tree is slower than the synthetic one, which is expected: colder caches,
deeper nesting, far more small files, live progress events going to a webview,
and index creation at the end.

## Where the time goes

In rough order, for a volume of small files:

1. `lstat` per entry. Unavoidable — sizes and timestamps are needed for every
   entry, and `d_type` from `readdir` does not supply them.
2. SQLite insert throughput. Addressed by multi-row `VALUES`, transactions
   spanning many batches, and building query indexes after the load.
3. Path string construction. Directories store a path; files do not, which
   removes most of it.
4. Channel and lock traffic. Reduced by one message per directory rather than
   per entry, and by block allocation of ids.

## Things deliberately not done

- **No content hashing during a scan.** The initial scan is metadata-driven.
  Duplicate detection escalates from size grouping to a partial probe to a full
  digest, and only when asked. See [database.md](database.md).
- **No in-memory tree.** The index is the tree.
- **No unbounded task spawning.** A fixed pool, a bounded channel, one writer.
- **No `walkdir` or `jwalk` dependency.** Neither gives the control this needs
  over per-directory batching, cancellation granularity, aggregation hooks and
  error recording, and the traversal is about 250 lines.
