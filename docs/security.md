# Safety model

DiskMate's job is to tell people the truth about their disk and then help them
act on it. Both halves need trust, and trust is easier to keep than to rebuild.
This document states the rules the product is built to, most of which constrain
code that does not exist yet — which is the point.

## Milestone 01 is read-only

There is no code path in `diskmate-core` or `src-tauri` that deletes, moves, or
modifies a file, changes a launch item, or signals a process.

This is verifiable rather than aspirational:

- The engine's only filesystem calls are `read_dir`, `symlink_metadata`,
  `DirEntry::metadata`, `canonicalize`, `read_to_string` on configuration files,
  and `create_dir_all` for DiskMate's own data directory.
- The Tauri capability set grants `core:default`, `dialog:allow-open` and
  `opener:allow-reveal-item-in-dir`. No filesystem, shell, HTTP or process
  permission is granted to the window.
- The app declares no network entitlement.

## One cleanup service, when there is one

Every destructive operation will flow through a single service. No feature will
implement its own deletion. A second deletion path is a second set of safety
checks that will drift from the first.

The flow that service will implement:

```
  CleanupCandidate
        ↓  safety classification      SAFE | REVIEW | PROTECTED
        ↓  protected-path validation  platform::protected
        ↓  permission validation      can this actually be moved?
        ↓  explicit human review      per item, never a blanket approval
        ↓  move to Trash              not unlink
        ↓  verify                     confirm it went where it should
        ↓  update the index
```

## Safety classes

`model::SafetyClass` exists now so the vocabulary is fixed before any code uses
it.

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

Nothing consults this list yet. It is here so that cleanup is built *around* a
protected-path check rather than having one added afterwards.

## Never automatic

No future version will automatically remove:

- Documents, Desktop, Downloads contents
- Photos, videos, music
- User projects
- Anything DiskMate does not recognise

An unrecognised file is not a candidate. It is a file DiskMate does not
understand, and the honest response is to say nothing about it.

## Trash, not unlink

Removal will move items to the Trash. That is not belt and braces — it is the
difference between a mistake a person can undo and one they cannot. Permanent
deletion, if it is ever offered, will be a separate, explicit, per-operation
choice.

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
  and shows nothing. The Cleanup page has no number and no button.
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
