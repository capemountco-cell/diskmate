# DiskMate

A storage and system-resource manager for macOS.

DiskMate answers one question: **what is using my Mac's resources, and what can I
safely do about it?** It reads your disk, explains what it found, and keeps
everything it learns on your machine.

It is not a "Mac cleaner". There are no invented problem counts, no fabricated
performance claims, and no button that promises to make your Mac faster.

## Status: Milestone 01

Milestone 01 is **read-only**. Nothing in this codebase deletes, moves, or
modifies a file, changes a launch item, or signals a process.

| Area    | State                                                                     |
| ------- | ------------------------------------------------------------------------- |
| Storage | Working. Volumes, scanning, breakdowns, largest folders and files, browsing |
| Cleanup | Not started. The page says so and shows no numbers                         |
| Startup | Read-only. Launch agents and daemons are listed; impact is not measured    |
| Memory  | Read-only. Pressure, usage, cache and swap, with an explanation            |

## Requirements

| Tool                | Version    | Why                                        |
| ------------------- | ---------- | ------------------------------------------ |
| macOS               | 12 or later| The app's deployment target                |
| Xcode CLT           | any        | Linker and SDK (`xcode-select --install`)  |
| Rust                | 1.82+      | The engine (`rustup` recommended)          |
| Node.js             | 20+        | The frontend build                         |

SQLite is compiled into the binary, so there is nothing to install for it.

## Commands

```sh
# Install JavaScript dependencies
npm install

# Run the app in development (starts Vite, then the Tauri window)
npm run dev

# Rust: tests, lints, formatting
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check

# Frontend: types, tests, lint
npm run typecheck
npm run test
npm run lint

# Scanner benchmark, recorded under bench-results/
./scripts/bench.sh
./scripts/bench.sh --files 500000 --dirs 8000
./scripts/bench.sh --path ~/Library --label library

# Production build
npm run build                 # DiskMate.app and DiskMate.dmg
./scripts/build-macos.sh      # DiskMate.app only
./scripts/build-dmg.sh        # DiskMate.dmg
./scripts/build-macos.sh --universal   # Apple silicon and Intel in one bundle
```

The built artefacts land under `target/release/bundle/`.
See [INSTALL.md](INSTALL.md) for a step-by-step walkthrough.

## How it is put together

```
React + TypeScript          presentation only
        |  Tauri commands and events
Application services        one decision-making layer
        |
Rust core                   scanner, aggregation, platform abstraction
        |
SQLite index                the source of truth for filesystem knowledge
        |
macOS filesystem and system APIs
```

Two crates:

- **`crates/diskmate-core`** — everything that is not presentation. It has no
  dependency on Tauri, so it builds and its tests run on any Unix host.
- **`src-tauri`** — the adapter. Each command deserialises its arguments, calls
  one service method, and serialises the result.

The UI never receives a filesystem tree. A Mac can hold millions of entries, so
the index is the source of truth and the UI asks bounded questions of it, such
as "the hundred largest immediate children of this folder".

Further reading:

- [INSTALL.md](INSTALL.md) — installing it on a Mac, step by step
- [ARCHITECTURE.md](ARCHITECTURE.md) — the layers and why they are separated
- [docs/scanner.md](docs/scanner.md) — traversal, aggregation, and measurements
- [docs/database.md](docs/database.md) — schema and the reasoning behind it
- [docs/security.md](docs/security.md) — the safety model for future cleanup
- [docs/permissions.md](docs/permissions.md) — what macOS does and does not allow
- [docs/portable-mode.md](docs/portable-mode.md) — running from a USB drive

## Privacy

DiskMate's filesystem intelligence runs entirely on your Mac. It does not
upload file names, paths, folder structures, file contents, hashes, application
lists, scan databases, or process information, and it does not need an internet
connection. The app declares no network entitlement.

Your files are none of our business.

## Known limitations

- **Unsigned builds.** No Apple Developer Program membership is used in this
  phase, so the app is not signed or notarized. A build you made yourself opens
  normally; one you downloaded needs a one-time approval in System Settings.
  See [INSTALL.md](INSTALL.md).
- **Full Disk Access.** Without it macOS hides parts of the disk from every app,
  DiskMate included. Folders it cannot read are reported as *not readable*
  rather than counted as empty, and the totals say what was left out. Choosing a
  folder in the picker grants access to that folder without any setting.
- **Login Items are not listed.** Modern macOS keeps them in a private store
  with no supported read interface. DiskMate lists launch agents and daemons and
  says that the list is partial, rather than guessing from legacy locations.
- **Startup impact is not measured.** Every item reports *unmeasured*. There is
  no code path that invents a launch-time figure.
- **One scan at a time**, per application instance.
- **No treemap yet.** The engine was the priority for Milestone 01.
