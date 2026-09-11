# Installing DiskMate on your Mac

Two routes. Pick one.

- **[Route A: download the disk image](#route-a-download-the-disk-image)** —
  nothing to install, about five minutes. Use this unless you want to change
  the code.
- **[Route B: build it yourself](#route-b-build-it-yourself)** — you get
  `DiskMate.dmg` and `DiskMate.app` from source, and can rebuild after edits.

Both produce the same unsigned application. See
[Why macOS complains](#why-macos-complains-and-what-to-click) for what that
means on first launch.

---

## Route A: download the disk image

Continuous integration builds `DiskMate.dmg` on every push. You are downloading
the exact artefact the build produced.

### 1. Open the build

Go to the pull request, click the **Checks** tab, and choose the most recent
**CI** run on the `claude/diskmate-macos-app-6hhmk5` branch. Or go straight to:

<https://github.com/capemountco-cell/diskmate/actions>

Open the newest run with a green tick.

### 2. Download the artefact

Scroll to the bottom of the run summary, to the **Artifacts** section. Click
**DiskMate-dmg-unsigned-arm64**. You must be signed in to GitHub; artifacts are
not available to signed-out visitors.

It downloads as a `.zip`, because GitHub always zips artifacts.

### 3. Unzip and open

Double-click the downloaded zip. You get `DiskMate.dmg`. Double-click that.

A window opens showing the DiskMate icon and an Applications shortcut.

### 4. Install

Drag **DiskMate** onto the **Applications** folder in that window.

Eject the disk image: click the eject arrow next to `DiskMate` in the Finder
sidebar, then delete the `.dmg` and the `.zip` if you like.

### 5. First launch

Open Applications and double-click DiskMate. macOS will refuse it the first
time. That is expected — see
[Why macOS complains](#why-macos-complains-and-what-to-click).

> **On an Intel Mac?** The CI artefact is built for Apple silicon and will not
> run. Use Route B with `--universal`.

---

## Route B: build it yourself

About 20 minutes the first time, most of it Rust compiling. Roughly 5 GB of
disk for the toolchains and build output.

### 1. Check what you already have

Open Terminal (press ⌘-Space, type "Terminal", press Return) and run:

```sh
sw_vers -productVersion    # macOS version; needs 12 or later
uname -m                   # arm64 = Apple silicon, x86_64 = Intel
rustc --version            # needs 1.82 or later
node --version             # needs 20 or later
xcode-select -p            # should print a path, not an error
```

Install whatever is missing, from the next step. Skip what is already there.

### 2. Xcode Command Line Tools

```sh
xcode-select --install
```

A dialog appears. Click **Install** and wait. This is the compiler and the macOS
SDK. It is a few gigabytes.

If it prints `command line tools are already installed`, you are done.

### 3. Rust

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Press Return to accept the default installation. Then, **in the same terminal**:

```sh
source "$HOME/.cargo/env"
rustc --version
```

New terminal windows pick it up automatically.

### 4. Node.js

If you have Homebrew:

```sh
brew install node
```

If not, download the LTS installer from <https://nodejs.org> and run it.

Check it:

```sh
node --version
```

### 5. Get the code

```sh
cd ~/Developer 2>/dev/null || mkdir -p ~/Developer && cd ~/Developer
git clone https://github.com/capemountco-cell/diskmate.git
cd diskmate
git checkout claude/diskmate-macos-app-6hhmk5
```

### 6. Install the JavaScript dependencies

```sh
npm install
```

### 7. Build the disk image

```sh
./scripts/build-dmg.sh
```

On an Intel Mac, or to make one image that runs on both architectures:

```sh
./scripts/build-dmg.sh --universal
```

The script runs the tests first, then builds. Expect 10 to 20 minutes on the
first run; later builds take a minute or two because Rust caches its work.

When it finishes it prints the path and a checksum:

```
Built ./target/release/bundle/dmg/DiskMate.dmg
  size   8.4M
  sha256 …
```

### 8. Install it

```sh
open target/release/bundle/dmg/DiskMate.dmg
```

Drag **DiskMate** onto **Applications**, then eject the disk image.

Because you built it on this Mac, macOS did not quarantine it. It opens on a
double-click with no warning.

### Want just the app, no disk image?

```sh
./scripts/build-macos.sh
open target/release/bundle/macos/
```

Drag `DiskMate.app` to Applications yourself.

### Want to run it without installing?

```sh
npm run dev
```

This opens DiskMate with live reload, for development.

---

## Why macOS complains, and what to click

DiskMate is not signed with an Apple Developer ID, because this phase
deliberately does not require a paid Apple Developer Program membership.

macOS only objects to files that arrived from somewhere else. So:

**You built it yourself** → nothing happens. It opens normally.

**You downloaded it** → the first launch is refused with *"Apple could not
verify DiskMate is free of malware"*.

Click **Done**, then:

**macOS 15 Sequoia or later**

1. Open **System Settings**.
2. Go to **Privacy & Security**.
3. Scroll to the bottom, to **Security**. There is a line saying DiskMate was
   blocked.
4. Click **Open Anyway**.
5. Authenticate with Touch ID or your password.
6. Confirm **Open Anyway**.

The older right-click → Open trick was removed in Sequoia; it will not work.

**macOS 14 Sonoma or earlier**

1. Right-click (or Control-click) DiskMate in Applications.
2. Choose **Open**.
3. Click **Open** in the dialog.

**Either version, from the terminal**

```sh
xattr -dr com.apple.quarantine /Applications/DiskMate.app
```

Whichever you use, it is once per copy. After that DiskMate opens normally.

---

## Letting DiskMate see your whole disk

DiskMate works immediately, but macOS hides some folders from every application
until you allow it. Without permission DiskMate cannot see your Mail store,
Messages history, Safari data, or iOS device backups — and iOS backups alone are
often tens of gigabytes.

DiskMate never reports a folder it cannot read as empty. It marks it **Not
readable** and tells you how much was left out.

To grant access:

1. **System Settings** → **Privacy & Security** → **Full Disk Access**.
2. Click **+**, choose DiskMate from Applications, and turn it on.
3. Quit DiskMate completely (⌘-Q) and open it again. macOS requires the restart.

**You do not have to do this.** The **Choose a folder…** button next to the
volume list scans any folder you pick, with no setting to change, because macOS
grants access to whatever you select in an open panel.

---

## Using it the first time

1. Open DiskMate. It lists your volumes with capacity.
2. Your startup disk is selected. Click **Scan**.
3. Watch the live count. **Cancel** stops it at once and keeps what it found.
4. When it finishes you get the category breakdown, the largest folders and
   files, and a browser. Double-click a folder to look inside.

DiskMate is read-only in this milestone. It does not delete, move or change
anything, and it never sends anything off your Mac.

---

## If something goes wrong

**`xcrun: error: invalid active developer path`**
The Command Line Tools are missing or broken. Run `xcode-select --install`.

**`cargo: command not found`**
Rust is installed but not on your PATH in this terminal. Run
`source "$HOME/.cargo/env"`, or open a new terminal window.

**The build fails on `libsqlite3-sys`**
Almost always the Command Line Tools. Confirm `xcode-select -p` prints a path.

**"DiskMate is damaged and can't be opened"**
This is the quarantine attribute, not damage. Run
`xattr -dr com.apple.quarantine /Applications/DiskMate.app`.

**The app opens but the volume list is empty**
Report it — that is a real bug. Include your macOS version and `uname -m`.

**The scan finds much less than Finder says is used**
Expected without Full Disk Access. The capacity bar separates *indexed by
DiskMate* from *used, not indexed*, and the scan summary says how many folders
were skipped and why.
