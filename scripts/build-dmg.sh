#!/usr/bin/env bash
#
# Build DiskMate.dmg.
#
# Wraps build-macos.sh and asks Tauri's bundler for the disk image, which lays
# out the drag-to-Applications window described in src-tauri/tauri.conf.json.
#
#   ./scripts/build-dmg.sh
#   ./scripts/build-dmg.sh --universal
#
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "error: a .dmg can only be built on macOS (this is $(uname -s))." >&2
  exit 1
fi

TARGET_ARGS=()
for arg in "$@"; do
  case "$arg" in
    --universal)
      rustup target add aarch64-apple-darwin x86_64-apple-darwin
      TARGET_ARGS=(--target universal-apple-darwin)
      ;;
    *) echo "error: unknown option $arg" >&2; exit 2 ;;
  esac
done

npm ci --no-audit --no-fund 2>/dev/null || npm install --no-audit --no-fund
npm run typecheck
cargo test --workspace --release

echo "==> Building the disk image"
npm run tauri -- build --bundles dmg "${TARGET_ARGS[@]}"

DMG="$(find src-tauri/target -name 'DiskMate_*.dmg' -maxdepth 5 -print -quit)"
if [[ -z "$DMG" ]]; then
  echo "error: the build finished but no .dmg was found." >&2
  exit 1
fi

FINAL="$(dirname "$DMG")/DiskMate.dmg"
mv -f "$DMG" "$FINAL"

echo
echo "Built $FINAL"
echo "  size   $(du -sh "$FINAL" | cut -f1)"
echo "  sha256 $(shasum -a 256 "$FINAL" | cut -d' ' -f1)"
echo
echo "Unsigned and un-notarized. Anyone opening it will need Finder's"
echo "right-click > Open on first launch. See docs/permissions.md."
