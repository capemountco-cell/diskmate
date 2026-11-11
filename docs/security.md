# Safety model

DiskMate's job is to tell people the truth about their disk and then help them
act on it. Both halves need trust, and trust is easier to keep than to rebuild.
This document states the rules the product is built to.

## What DiskMate can and cannot do

DiskMate can do two things that change anything: move a file or folder to the
Trash, and ask an ordinary app to quit. It cannot delete anything permanently,
empty the Trash, modify a file's contents, change a launch item, kill a
process, or purge memory, because no such code exists.

This is verifiable rather than aspirational:

- Outside the cleanup service, the engine's only filesystem calls are
  `read_dir`, `symlink_metadata`, `DirEntry::metadata`, `canonicalize`,
  `read_to_string` on configuration files, `File::read` for content hashing, and
  `create_dir_all` for DiskMate's own data directory.
- The cleanup service's only mutating call is a move into the Trash, behind the
  `Remover` trait. Nothing else in the workspace calls `remove_file`,
  `remove_dir_all`, or `rename` on a user's path.
- The Tauri capability set grants `core:default`, `dialog:allow-open` and
  `opener:allow-reveal-item-in-dir`. No filesystem, shell, HTTP or process
  permission is granted to the window.
- The app declares no network entitlement.

## Quitting an app

`services::ProcessService::quit_app(pid, name)` is the only process action, and
the menu bar item and the Memory screen both go through it. It:

- sends `NSRunningApplication -terminate`, the request an app's own Quit menu
  item sends, so the app can save, ask about unsaved work, or decline. It never
  sends a signal;
- refuses anything without the regular activation policy (a Dock icon), so
  background agents, daemons and command-line processes are listed for
  information only;
- re-reads the running apps and refuses unless `pid` still belongs to the app
  named `name`, so a stale menu or a reused pid cannot quit the wrong thing;
- refuses DiskMate itself.

The action sits behind the `platform::AppQuitter` trait, whose one method is
`request_quit`, for the same reason removal sits behind `Remover`.

There is no "free memory" or purge operation. macOS lends idle memory to the
file cache and takes it back on demand, so purging makes the number smaller and
the Mac briefly slower. Quitting the app that holds the memory is what frees it.

## One cleanup service

Every destructive operation flows through `services::CleanupService`. No feature
implements its own deletion: the duplicate finder, the uninstaller and the
detection rules all produce *candidates*, and a candidate is a row in a table,
not an action.

The mechanism is a trait with exactly one destructive method:

```rust
pub trait Remover: Send + Sync {
    fn move_to_trash(&self, path: &Path) -> Result<(), String>;
    fn name(&self) -> &'static str;
}
```

Anything that wants to remove something has to hold one of these, and the only
implementations are `TrashRemover` (the real one) and `DryRunRemover` (which
refuses). Adding a second deletion path means adding a second implementation of
a trait whose name says what it does, which is the kind of change a reviewer
notices.

The flow, as implemented:

```
  CleanupCandidate
        ↓  safety classification      SAFE | REVIEW | PROTECTED
        ↓  protected-path validation  platform::is_protected_path
        ↓  shape validation           absolute, not a volume root, ≥ 3 components
        ↓  staging                    the user puts it in the queue
        ↓  confirmation               ids *and* total bytes must match the queue
        ↓  move to Trash              not unlink
        ↓  verify                     confirm the path is actually gone
        ↓  audit                      cleanup_operations records what happened
```

## Approval is by identity and by total

`remove_approved` takes a `Confirmation`:

```rust
pub struct Confirmation {
    pub candidate_ids: Vec<i64>,
    pub expected_total_bytes: i64,
}
```

If the set of ids does not exactly equal what is staged, or the total does not
match, **nothing is removed** — not the matching part, the whole operation. The
purpose is to make the number on the screen a promise. A user who approves
"414 MB, 3 items" cannot get anything else, including if the queue changed
underneath them between rendering and clicking.

A staged file is also re-checked at removal time: if its size or modification
time changed since staging, it is refused. What you approved is what existed
when you looked at it.

## Safety classes

`model::SafetyClass` is the vocabulary every rule, candidate and refusal uses.

| Class | Meaning |
| --- | --- |
| `Safe` | Regeneratable or extremely low risk. A build cache, a thumbnail cache. |
| `Review` | Possibly removable, needs human judgement. An old download, a large file nobody has opened in two years. |
| `Protected` | DiskMate will never recommend removing it. |

**"Reclaimable" is never a synonym for "safe to delete."** A duplicate group's
`reclaimable` figure is a measurement of how many bytes are held twice. It is
not a recommendation, and it is never presented as one.

## Protected paths

`platform::protected` holds the list, and it is deliberately conservative:
operating-system directories, application bundles, launch configuration,
keychains, and the user's own documents, desktop, pictures, movies, music, mail,
messages, device backups and SSH and GPG keys.

Two details that matter:

- Prefix matching respects segment boundaries. `/Users/ada/Documents-old` does
  not match `/Users/ada/Documents`.
- A path DiskMate cannot render as text is treated as protected. A path it
  cannot read is not a path it should act on.

The list is consulted three times on the way to a removal: when a rule proposes
a candidate, when a candidate is staged, and again inside `remove_approved`. A
rule that proposes a protected path is a bug in that rule, and the redundancy
means such a bug cannot reach a file.

## Never automatic

No future version will automatically remove:

- Documents, Desktop, Downloads contents
- Photos, videos, music
- User projects
- Anything DiskMate does not recognise

An unrecognised file is not a candidate. It is a file DiskMate does not
understand, and the honest response is to say nothing about it.

## Trash, not unlink

Removal moves items to the Trash. That is not belt and braces — it is the
difference between a mistake a person can undo and one they cannot. There is no
permanent-deletion path in the codebase. If one is ever offered it will be a
separate, explicit, per-operation choice, and it will not share a code path with
this one.

`TrashRemover` delegates to the `trash` crate, whose macOS backend calls
`NSFileManager trashItemAtURL:` — the same operation the Finder performs — so
items land in the Trash with their "Put Back" information intact.

## Duplicates and the uninstaller

Two features are capable of proposing a lot of items at once, so both are
deliberately narrow.

**Duplicates** are established byte for byte and never by name. Files are
grouped by size from the index, then by a digest of the first and last 16 KB,
and only survivors of both are read in full. Hard links to one inode are
excluded: they are one file with two names, and "reclaiming" one reclaims
nothing. The oldest copy in a group is the keeper and is never proposed.

**The uninstaller** matches leftovers by exact bundle identifier or exact
directory name, in a fixed table of ten locations. Prefix and substring matching
are not used, because `com.example.app` must never match `com.example.apple`.
Applications that came with macOS are refused outright.

## Launch items and processes

DiskMate reads launch agents and daemons. It does not load, unload, enable,
disable or delete them, and it does not call `launchctl`. Modifying launch
configuration by hand is a well-known way to break an application in a manner
that is hard to diagnose, so it will only ever happen with an explicit,
per-item choice.

The same applies to processes: DiskMate does not signal anything, and there is
no "free memory" operation. See [permissions.md](permissions.md) for what macOS
allows an unprivileged app to see.

## Presentation rules

These are product rules, and the code is tested against some of them.

- **No fabricated counts.** If a feature is not implemented, its page says so
  and shows nothing. The Startup page shows no impact figure because impact is
  not measured.
- **Reasons, not verdicts.** Every candidate carries a sentence saying what it
  is and what removing it costs — "cargo rebuilds this", "rebuilding takes time,
  not data" — and a test asserts that no candidate ships without one.
- **No invented measurements.** Startup impact reports `Unmeasured` because it
  has not been measured. There is no code path that produces a launch-time
  figure.
- **No invented progress.** The scan bar is indeterminate because the scanner
  cannot know a total until it has walked the tree. A percentage that jumps
  around is worse than none.
- **No alarm styling.** DiskMate's only device for "you should know this" is a
  line of text with a coloured rule. There is no full-bleed red banner, because
  nothing this product reports is an emergency.
- **No language that offers to optimise, boost, clean or free.** The memory
  service has a unit test asserting that none of its wording does.

## Privacy

DiskMate's filesystem intelligence operates entirely locally. It does not upload
file names, paths, directory structures, file contents, hashes, application
lists, scan databases, or process information. The product works with no
internet connection.

If licensing or anonymous product analytics are ever added, they will be
architecturally separate from filesystem and system analysis — a different
module, with no access to the index. That separation is a design constraint, not
a promise about intent.

Your files are none of our business.

## Reporting a problem

If you find a way to make DiskMate delete, move, or modify something without an
explicit user choice, that is a serious bug. Open an issue on the repository
with the steps to reproduce it.
