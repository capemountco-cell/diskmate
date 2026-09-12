# The local index

DiskMate stores what it learns about a filesystem in a single SQLite database.
It is a cache in the sense that it can always be rebuilt by scanning again, and
a source of truth in the sense that nothing else in the product holds
filesystem knowledge.

Location depends on how DiskMate is running; see
[portable-mode.md](portable-mode.md). By default:

```
~/Library/Application Support/DiskMate/index.sqlite3
```

SQLite is compiled into the binary (`rusqlite` with `bundled`), so DiskMate
never depends on the host's version.

## Connection settings

| Pragma | Value | Why |
| --- | --- | --- |
| `journal_mode` | `WAL` | The UI can read while a scan writes |
| `synchronous` | `NORMAL` | The worst case is losing the tail of one scan, and a scan is always repeatable |
| `temp_store` | `MEMORY` | Keeps aggregation's sort scratch off disk |
| `cache_size` | 64 MB | Navigation queries stay in cache |
| `mmap_size` | 256 MB | Read-heavy navigation without heap growth |
| `foreign_keys` | `ON` | Cascades on scan deletion |

## Schema

Migrations are forward-only, embedded in the binary, applied in a transaction
and recorded in `schema_migrations`. A migration is never edited after release;
a change is always a new file.

### Core tables

| Table | Holds |
| --- | --- |
| `volumes` | Mounted volumes seen, with capacity and kind |
| `scans` | One row per scan: status, timings, totals, options, engine version |
| `fs_entries` | One row per filesystem entry |
| `scan_errors` | Every path the scan could not read, and why |
| `scan_categories` | Per-scan category totals, accumulated while walking |
| `scan_extensions` | Per-scan extension totals, accumulated while walking |
| `scan_benchmarks` | The benchmark ledger, for comparing scanner revisions |

### Three decisions in `fs_entries`

**Directories store their absolute path; files do not.**

A file's path is `parent.path || '/' || name`, resolved by a single join at
query time. Every listing query therefore joins `fs_entries` to itself once,
which is an O(1) primary-key lookup per row.

The alternative — a `path` column on every row — adds 60–100 bytes to each file.
On a five-million-file volume that is roughly 400 MB of index instead of 120 MB,
paid on every write, to save a join that costs nothing.

**`subtree_logical_size` carries attributed bytes for every kind, not just
directories.**

For a directory it is the subtree total. For a file it is the file's length. For
a hard-link repeat or a symlink it is zero. One column means one index can order
a mixed listing of files and folders by the bytes attributed to each, which is
exactly the query the explorer makes.

The UI is shown a *different* number for a file — its real length — so a
hard-linked copy reads as the size it is, marked *shared copy*. The attributed
value is what shares within a folder are computed from, which is what keeps them
summing to one.

**`accessible` distinguishes empty from unreadable.**

A directory DiskMate could not open has `accessible = 0` and a zero subtree
total, and its reason is a row in `scan_errors`. Reporting it as zero bytes with
no further comment would be a quiet lie about how much of the disk is accounted
for.

### Indexes, and when they are built

Query indexes are created **after** the bulk load, not maintained during it.
During a scan only the rowid primary key is maintained, which turns four
incremental B-tree updates per row into four bulk sorts at the end.

| Index | Serves |
| --- | --- |
| `(scan_id, parent_id, subtree_logical_size DESC)` | Largest immediate children — the hot path |
| `(scan_id, logical_size DESC) WHERE kind = 0` | Largest files |
| `(scan_id, subtree_logical_size DESC) WHERE kind = 1` | Largest folders |
| `(scan_id, path) WHERE path IS NOT NULL` | Navigation by path, rescans, deep links |

There is deliberately **no index on `extension`**. Indexing a string column
across millions of rows would cost more in write throughput than the file-type
breakdown is worth, so category and extension totals are accumulated in Rust
while walking — a few thousand distinct extensions at most — and written once
into `scan_categories` and `scan_extensions`. The `extension` column is still
stored, so a future "show me every `.mov`" can scan for it.

## Write path

`db::writer::IndexWriter` is driven by exactly one thread. SQLite serialises
writers anyway, so a second would add lock contention and no throughput.

- Rows are inserted with multi-row `VALUES` tuples: one prepared statement
  covers 128 entries instead of 128 trips through the VDBE.
- A transaction spans 50,000 entries, not one. A transaction per entry would be
  an fsync per entry.
- Directory subtree totals arrive at the end and are applied by loading the
  aggregator's results into a temporary table and running one `UPDATE ... FROM`.
- `PRAGMA optimize` runs once at the end so the planner's statistics match the
  indexes just built.

Measured index size is around 187–217 bytes per entry, depending on name lengths
and directory ratio.

## Reading

Every read query is bounded. `ChildQuery` clamps its limit to 1,000 regardless
of what the caller asks for; a request for a million rows is a bug, not a
feature.

Two queries are worth calling out:

**`ancestors`** walks the parent chain iteratively rather than with a recursive
CTE. The chain is at most a few dozen rows and each hop is a primary-key lookup,
so this is both faster and easier to bound against a corrupt chain.

**`largest_directories`** excludes pass-through folders. Every ancestor of a big
folder is also big, so the naive query returns the same bytes five times over as
a parent chain — `home`, `user`, `project`, `target`, `debug`. A folder is
dropped when a single child folder accounts for 85% or more of it, leaving the
folder where the space actually spreads out. The `NOT EXISTS` is an index seek
on the children index per candidate, and candidates are visited in size order,
so it typically touches a few dozen rows before the limit is met.

## Retention

`prune_scans(keep_per_root)` keeps the newest N scans per root and deletes the
rest with everything derived from them. Without it the index grows without bound
across rescans. A scan referenced by a snapshot is never pruned.

## Analysis tables

Created by migrations 002 and 003, and written by the analysis modules rather
than the scanner. Keeping them out of `fs_entries` means a rescan does not have
to reproduce work that is still valid, and a duplicate search can be cancelled
without leaving the index inconsistent.

| Table | Holds |
| --- | --- |
| `file_classifications` | Rule-driven categories, recomputable without rescanning |
| `content_hashes` | Partial and full digests, keyed by entry and kind |
| `duplicate_groups`, `duplicate_group_members` | Confirmed duplicate sets |
| `cleanup_candidates` | Candidates with a rule, a reason and a safety class |
| `cleanup_operations` | The audit log: what was removed, when, by which remover |
| `applications` | Installed applications, with bundle identifier and version |
| `application_leftovers` | What an application scattered, and where |
| `snapshots` | Point-in-time storage records |
| `scan_comparisons`, `snapshot_deltas` | Change between two scans, per folder |

### The cleanup tables

`cleanup_candidates` is unique on `(scan_id, path, rule)`, so re-running the
rules updates a candidate rather than duplicating it. Its `state` column carries
the whole lifecycle — `proposed`, `staged`, `removed`, `refused`, `failed` — and
the upsert only touches rows still in `proposed` or `staged`. That is what stops
a removed candidate being resurrected by the next proposal pass.

`entry_id` is nullable, which matters: an application's leftovers are found by
inspecting the filesystem directly, so a candidate can exist for a path the scan
never indexed.

`cleanup_operations` is written after every removal attempt, successful or not,
and records the remover's name. It is the only place that knows a file left, and
it is never consulted to decide anything — it exists so a person can answer "what
did this thing do to my disk" without trusting the UI.

### Snapshots are small on purpose

A snapshot stores a scan's totals and its identity, not a copy of the tree. The
comparison reads both scans' directory rows and joins them on path, so the detail
lives in the index exactly once. The cost is that a snapshot is only as good as
the scan it points at, which is why `prune_scans` never removes a scan a
snapshot references.

A comparison collapses ancestor chains before returning: if one folder inside
another accounts for at least 90% of the outer folder's change, only the inner
one is reported. Without that, a single 400 MB file appearing deep in a tree is
reported once per level above it, and the folder that actually changed is buried
among its own parents.

### Duplicate detection, and why the schema looks like this

Hashing every file is the obvious approach and the wrong one: it reads the whole
volume to find that almost nothing matches. The escalation, as implemented:

```
all files
   ↓  group by logical size          (an index-only pass, no reads)
   ↓  discard unique sizes           (removes the large majority)
   ↓  partial digest of a sample     (content_hashes, kind = 'partial')
   ↓  discard mismatches
   ↓  full digest                    (content_hashes, kind = 'full')
   ↓  confirmed duplicate_groups
```

`content_hashes` is keyed by `(entry_id, kind)` so both stages live in one table,
and it carries the offset and length of the partial probe so a later revision can
tell which sampling strategy produced a row. The partial probe is 16 KB from each
end of the file; the full digest is BLAKE3.

Hard-linked repeats are excluded before any of this. Two names for one inode are
not two copies, and removing one reclaims nothing.

**Filenames never establish that two files are duplicates.** Two files with the
same name are two files with the same name.

Note what `duplicate_groups.reclaimable` means: the bytes recoverable if all but
one copy were removed. That is a *measurement*, not a recommendation. Nothing in
DiskMate treats "reclaimable" as a synonym for "safe to delete".
