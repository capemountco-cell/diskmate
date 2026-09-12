# Portable mode

DiskMate should be able to live on a USB drive and run from it, so a technician
can plug into a Mac, understand its storage, and leave nothing behind.

The part that has to be right from the start is already done: **nothing in the
engine assumes its state lives in a fixed directory under the user's home.**
Every consumer asks `paths::AppPaths`, the cleanup audit log included.

## The layout

```
DiskMate USB/
├── DiskMate.app
├── DiskMate Data/          index.sqlite3 and any future state
└── DiskMate Logs/
```

`DiskMate Data` is what marks the drive as portable. A `.diskmate-portable`
marker file next to the application does the same thing for layouts that do not
use the folder.

## How the location is chosen

`AppPaths::resolve` runs once at startup, in this order:

1. **`DISKMATE_DATA_DIR`** — an explicit override. Used by the tests, the
   benchmark harness, and anyone scripting DiskMate.
2. **Portable** — walk up from the running executable looking for a
   `DiskMate Data` directory or a `.diskmate-portable` marker. On macOS the
   binary sits at `<root>/DiskMate.app/Contents/MacOS/DiskMate`, so the drive
   root is four levels up; elsewhere it is usually one. Rather than hard-coding
   either shape, each ancestor is checked, bounded to six levels so a stray
   folder far up the tree cannot capture an installed build.
3. **Installed** — `~/Library/Application Support/DiskMate` on macOS, the XDG
   data directory elsewhere.

A portable root that turns out to be **not writable** — a locked disk image, a
write-protected stick — falls back to installed mode and records why. The reason
is surfaced in the UI, so a technician is never silently writing to the host Mac
when they meant not to.

This is unit tested: an app bundle beside a data folder, a marker file alone, an
ordinary executable, and the environment override.

## Using it today

```sh
# Build the app on a Mac
./scripts/build-macos.sh

# Lay out a drive
cp -R target/release/bundle/macos/DiskMate.app /Volumes/DISKMATE/
mkdir -p "/Volumes/DISKMATE/DiskMate Data" "/Volumes/DISKMATE/DiskMate Logs"

# Run it from the drive
open /Volumes/DISKMATE/DiskMate.app
```

DiskMate detects the layout and keeps its index on the drive. Verify in the
sidebar, which reads **Running portably**.

Format the drive as APFS or HFS+ if it will hold the index. SQLite on exFAT
works but exFAT has no real file locking, so an interrupted scan is more likely
to need the index rebuilt.

## What macOS does not allow

**There is no Windows-style autorun on macOS, and DiskMate will not fake one.**
Every trick for launching code from removable media without the user asking is
either blocked by the operating system or a security problem. None will be
implemented.

Launching DiskMate from a drive means opening it, the same as any other
application.

A future *optional* installed helper could notice a DiskMate drive being mounted
and offer to open the copy on it. That is a user-installed, user-consented
convenience, and it is not built.

## Permissions on a borrowed Mac

Portable mode does not change what macOS allows. On an unfamiliar machine:

- Volumes, capacity, memory and launch items are readable straight away.
- Full Disk Access must be granted to *that copy of the app on that Mac*, and
  granting it needs an administrator. It persists for the drive's copy until
  removed.
- Without it, **Choose a folder…** is the quickest route: macOS grants access to
  a folder the user picks in the panel, with no setting to change.

See [permissions.md](permissions.md).

## Still to come

- Writing the log files that `DiskMate Logs` is reserved for.
- A generated report a technician can leave with the machine's owner.
- Carrying an index between machines, keyed by volume identity rather than by
  mount point.
- A first-run explanation that the drive is being used for state.

The path resolution is the part that had to exist now. Everything above is
additive on top of it.
