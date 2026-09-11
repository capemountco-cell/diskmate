#!/usr/bin/env bash
#
# Build DiskMate.app.
#
# Produces an unsigned, un-notarized bundle. That is deliberate for this phase:
# DiskMate does not require an Apple Developer Program membership to build or to
# run, and development is not blocked waiting for a certificate. See
# docs/permissions.md for what an unsigned build means for the person running it.
#
#   ./scripts/build-macos.sh                 # build for this Mac's architecture
#   ./scripts/build-macos.sh --universal     # one bundle for Apple silicon and Intel
#
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "error: DiskMate.app can only be built on macOS (this is $(uname -s))." >&2
  echo "       The engine itself builds and tests anywhere: cargo test -p diskmate-core" >&2
  exit 1
fi

UNIVERSAL=0
for arg in "$@"; do
  case "$arg" in
    --universal) UNIVERSAL=1 ;;
    *) echo "error: unknown option $arg" >&2; exit 2 ;;
  esac
done

echo "==> Environment"
echo "    macOS        $(sw_vers -productVersion)"
echo "    Architecture $(uname -m)"
echo "    Rust         $(rustc --version)"
echo "    Node         $(node --version)"

if ! xcode-select -p >/dev/null 2>&1; then
  echo "error: the Xcode Command Line Tools are not installed." >&2
  echo "       Run: xcode-select --install" >&2
  exit 1
fi

echo "==> Installing JavaScript dependencies"
npm ci --no-audit --no-fund 2>/dev/null || npm install --no-audit --no-fund

echo "==> Checking"
npm run typecheck
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --release

TARGET_ARGS=()
if [[ "$UNIVERSAL" == "1" ]]; then
  echo "==> Adding both Apple targets"
  rustup target add aarch64-apple-darwin x86_64-apple-darwin
  TARGET_ARGS=(--target universal-apple-darwin)
fi

echo "==> Building the bundle"
npm run tauri -- build --bundles app "${TARGET_ARGS[@]}"

# Searched rather than assumed: this is a Cargo workspace, so the bundle lands
# in the workspace target directory, not under src-tauri as a single-crate Tauri
# project would put it.
APP="$(find . -type d -path '*/release/bundle/macos/DiskMate.app' -print -quit)"
if [[ -z "$APP" ]]; then
  echo "error: the build finished but DiskMate.app was not found." >&2
  echo "       looked for */release/bundle/macos/DiskMate.app under $PWD" >&2
  exit 1
fi

echo
echo "Built $APP"
echo "  size $(du -sh "$APP" | cut -f1)"
echo
echo "This bundle is unsigned, but you built it here, so macOS did not"
echo "quarantine it: drag it to /Applications and open it normally."
echo "A copy you send elsewhere will need the steps in INSTALL.md."
