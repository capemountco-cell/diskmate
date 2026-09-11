# Architecture

This document records the decisions that shape DiskMate, and why each was made.
It is meant to be read before changing anything structural.

## The shape

```
  React + TypeScript            Presentation. No product decisions.
        │
        │  Tauri commands (request/response) and events (scan progress)
        ▼
  src-tauri/src/commands        Adapters. Deserialise, call one service, serialise.
        │
        ▼
  diskmate-core::services       Application services. Every decision lives here.
        │
        ├── scanner              Traversal, aggregation, cancellation, progress
        ├── db                   Migrations, batched writer, indexed queries
        ├── platform             Volumes, memory, startup items, protected paths
        └── model                Domain types shared across the whole stack
        │
        ▼
  SQLite index                  The source of truth for filesystem knowledge
```

### Two crates, and why

`diskmate-core` contains the entire product's logic and **does not depend on
Tauri**. This is not tidiness for its own sake; it buys three things:

1. The engine is testable without a window, a webview, or a display server. The
   26 integration tests build real directory trees and scan them.
2. The engine can be benchmarked as a plain binary, which is how
   `scripts/bench.sh` compares scanner revisions.
3. When DiskMate grows a command-line mode, a technician's report generator, or
   a second front end, none of them route through Tauri.

`src-tauri` is thin on purpose. A command handler that contains a decision is a
decision that cannot be tested, so handlers only marshal.

### Services, not command handlers

`services::StorageService` owns the index connection and mediates every storage
operation. It is the only thing that knows that a scan takes the connection for
its duration, that a second scan must be refused while one runs, and that a
query during a scan opens a second read view instead of blocking the UI.

`MemoryService` and `StartupService` exist with almost nothing in them. They are
there so that when memory and startup become real features, the boundary is
already drawn and the work does not accrete inside a command handler.

## The index is the source of truth

The UI must never load a filesystem tree. A Mac can hold ten million entries;
serialising even a fraction of that across IPC would make the app feel broken
regardless of how fast the scanner is.

So the contract is: **the scanner writes to SQLite, and the UI asks SQLite
bounded questions.** `list_children` takes a parent id, a sort, a limit and an
offset. `largest_files` takes a limit. Nothing returns an unbounded result.

Three consequences worth knowing:

- **Directories store their absolute path; files do not.** A file's path is
  `parent.path || '/' || name`, resolved by a single join. Storing it per file
  would add 60–100 bytes to every row; on a five-million-file volume that is the
  difference between roughly 120 MB and 400 MB of index.
- **Aggregation happens before the UI sees anything.** Folder totals are
  computed while walking and written once. React never sums a column.
- **Query indexes are created after the bulk load**, not maintained during it.

See [docs/database.md](docs/database.md) for the schema and the trade-offs.

## The scanner

The design and its measurements are in [docs/scanner.md](docs/scanner.md). The
three properties that drove it:

- **Bounded memory.** Nothing in the pipeline grows with the size of the volume.
  A ten-million-file scan and a ten-thousand-file scan have the same shape.
- **Backpressure.** Worker threads block when the writer falls behind, rather
  than buffering the filesystem in RAM.
- **Survivability.** One unreadable directory, one vanished file, or one
  hard-linked directory loop never fails a scan.

## Platform abstraction

`platform::SystemProbe` is the only place that talks to the operating system
about volumes, memory and startup items. `macos`, `linux` and `unsupported`
implementations sit behind it, selected at compile time.

This is not speculative portability. It is what lets the whole engine be built
and tested on Linux CI with a real platform implementation rather than a stub,
and it means a future Windows port replaces one module instead of rewriting the
core.

The scanner itself is already platform-neutral: it uses `std::fs` plus
`std::os::unix::fs::MetadataExt`, with a non-Unix fallback.

## Read-only, on purpose

There is no code path in either crate that deletes, moves or modifies a file,
changes a launch item, or signals a process. `SafetyClass` and the
`cleanup_candidates` table exist so the vocabulary and the storage for cleanup
are fixed before any cleanup code is written, and `platform::protected` exists
so that the protected-path check is something cleanup is built *around* rather
than something bolted on afterwards.

See [docs/security.md](docs/security.md).

## Decisions worth recording

**Plain CSS, no Tailwind, no component library.** DiskMate should read like a
system utility. That is a matter of a narrow neutral ramp, one accent, hairline
rules and restraint with surfaces — all of which are easier to hold consistent
in one stylesheet of design tokens than across utility classes. It also removes
roughly fifty dependencies the Lovable scaffold carried.

**A reducer, not a state library.** The Storage screen has one lifecycle and a
handful of transitions. Writing them out is what keeps the components free of
logic.

**`rusqlite` with `bundled`.** SQLite is compiled into the binary, so DiskMate
never depends on the host's version and behaves identically on every Mac.

**No async runtime for scanning.** A scan is a long syscall-bound job. Putting
it on an executor would occupy a thread for its whole duration anyway, so it
runs on a plain OS thread and reports through events.

**Decimal size units.** 1 KB is 1000 bytes, because that is what Finder has
reported since Snow Leopard. A storage tool that disagrees with Finder about the
size of the same folder is one people stop trusting.

## Repository layout

```
crates/diskmate-core/
  src/model/          Domain types, serialised straight across IPC
  src/db/             Migrations, batched writer, indexed read queries
  src/scanner/        walk, meta, aggregate, pipeline, progress, cancel
  src/platform/       macos/, linux/, protected paths, startup locations
  src/services/       StorageService, MemoryService, StartupService
  src/paths.rs        Portable-mode-aware data directories
  benches/            Reproducible scanner benchmark
  tests/              Integration tests over real directory trees
src-tauri/
  src/commands/       IPC adapters, grouped by area
  src/state.rs        Application state
  tauri.conf.json     Window, bundle and CSP configuration
  capabilities/       The permission set the window is granted
src/
  app/                Shell and navigation
  features/           storage/, cleanup/, startup/, memory/
  components/         Shared primitives, icons, the meter
  lib/                IPC wrappers, model types, formatting
  styles/             Design tokens, base, application
scripts/              build-macos.sh, build-dmg.sh, bench.sh
docs/                 The documents referenced above
```
