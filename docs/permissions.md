# macOS permissions

What DiskMate can see, what it cannot, and what it does about the difference.

DiskMate does not attempt to bypass any macOS security control. Where access is
withheld, it says so and keeps working with what it has.

## What works with no permission at all

- Enumerating mounted volumes and reading their capacity (`getmntinfo`,
  `statfs`). Names, mount points, filesystem types, free space.
- The user's own home directory, minus the protected subfolders below.
- `/Applications`, `/System`, `/usr`, `/Library` — readable by any process.
- Memory statistics (`sysctl`, `host_statistics64`).
- `/Library/LaunchAgents`, `/Library/LaunchDaemons` and their `/System`
  counterparts.
- Any folder the user picks in the open panel, and everything inside it. macOS
  grants access to what a person explicitly chose, which is why DiskMate offers
  **Choose a folder…** next to the volume list.

## What needs Full Disk Access

macOS protects some locations behind TCC even inside the user's own home. Until
Full Disk Access is granted, these are invisible to *every* application:

| Location | Contains |
| --- | --- |
| `~/Library/Mail` | Mail storage |
| `~/Library/Messages` | iMessage history |
| `~/Library/Safari` | Browsing data |
| `~/Library/Application Support/MobileSync` | iOS device backups, often tens of GB |
| `~/Library/Containers/<app>` (some) | Sandboxed app data |
| `~/Library/Cookies` | Cookies |
| `/Library/Application Support/com.apple.TCC` | The permission database itself |
| Time Machine local snapshots | Snapshot storage |

Some of these are large. A Mac can easily have 60 GB of iOS backups that no scan
sees without Full Disk Access.

**Granting it:** System Settings → Privacy & Security → Full Disk Access → add
DiskMate. macOS requires the app to be relaunched afterwards.

### How DiskMate behaves without it

- Each unreadable directory is recorded in `scan_errors` with
  `permissionDenied`.
- Its row is marked `accessible = 0`, with a zero subtree total. The explorer
  shows **Not readable**. It is never presented as an empty folder.
- The scan summary reports the count and explains it in plain language, with
  where to change the setting.
- The volume's capacity bar distinguishes *indexed by DiskMate* from *used, not
  indexed*, so the gap is visible rather than implied.

This is deliberate. Under-reporting a disk silently is how a storage tool loses
the user's trust the first time its number disagrees with Finder's.

## What is not available to any unprivileged app

- **Login Items.** Modern macOS keeps them in a private, SMAppService-managed
  store with no supported read interface. DiskMate lists launch agents and
  daemons and states that the list is partial, rather than guessing from legacy
  locations and producing a list that is wrong on any recent system.
- **Other users' home directories**, without administrator authorisation.
- **Per-process memory for processes owned by other users**, without elevation.
- **A total for APFS snapshot storage**, without privileged interfaces.

DiskMate reports these as unavailable. It does not estimate them.

## The macOS volume layout

Since Catalina a Mac's boot disk is two APFS volumes: a sealed, read-only system
volume at `/`, and a data volume at `/System/Volumes/Data` holding everything the
user owns. Firmlinks make `/Users` at the root resolve into the data volume, so
Finder shows one disk while `stat` sees two devices.

DiskMate presents them as one volume, takes the boot volume's capacity from the
data volume (the sealed volume reports the shared container's free space but a
misleadingly small size of its own), and allows a scan of `/` to cross onto the
data volume while skipping every *other* mount. Treating a device change as a
hard boundary would report a Mac as using ten gigabytes.

## Opening an unsigned build

Builds in this phase are **unsigned and un-notarized**. No Apple Developer
Program membership is used, which is a deliberate decision for the initial
product rather than an oversight.

What happens on first launch depends on where the app came from, because macOS
only applies Gatekeeper's quarantine to files that arrived from elsewhere.

**Built on the machine you are running it on.** Nothing special happens. There
is no quarantine attribute, so DiskMate opens on a double-click like any other
application.

**Downloaded** — a `.dmg` from a browser, or a CI artifact zip. The file carries
`com.apple.quarantine`, and the first launch is refused.

The bundle is **ad-hoc signed** (`bundle.macOS.signingIdentity` is `-`), and
that matters more than it sounds. A bundle with no signature at all is reported
by Gatekeeper as *"DiskMate is damaged and can't be opened. You should move it
to the Trash"* — wording about a missing signature that reads as a corrupt
download, and which **Open Anyway** often cannot clear. An ad-hoc signature
produces the ordinary *"Apple could not verify..."* prompt instead, which the
user can approve.

It is not notarization and does not make the app verified; only a Developer ID
does that. It makes the refusal honest and recoverable.

| macOS | How to open it |
| --- | --- |
| 15 Sequoia and later | Let it be refused, then System Settings → Privacy & Security → scroll to Security → **Open Anyway**, and authenticate. The older right-click → Open bypass no longer works. |
| 14 Sonoma and earlier | Right-click the app → **Open** → **Open**. |
| Any version | `xattr -dr com.apple.quarantine /Applications/DiskMate.app` removes the attribute directly. |

Either way it is a once-per-copy step. After that DiskMate opens normally.

Full Disk Access can still be granted to an unsigned app.

When a Developer ID is available, `scripts/build-macos.sh` and
`scripts/build-dmg.sh` are where signing and notarization slot in. Nothing else
in the project assumes an unsigned build.

## Not sandboxed, and not a Mac App Store app

`src-tauri/entitlements.plist` sets `com.apple.security.app-sandbox` to false.

A sandboxed application cannot read the whole disk even with Full Disk Access
granted, which would leave DiskMate unable to do its one job. Mac App Store
distribution requires the sandbox, so DiskMate is not an App Store application.
That is a consequence of the product, not a preference.

No network entitlement is requested, because the product does not use the
network.

## Permission onboarding

DiskMate explains the situation where it arises: the volume list marks a volume
it cannot read, the scan summary explains what was left out, and the folder
picker is offered as the alternative that needs no setting.

A proper onboarding flow — explaining Full Disk Access before the first scan,
with a direct link to the right System Settings pane and detection of when it has
been granted — is still to come. Being correct about what was missed matters more
than being smooth about asking.
