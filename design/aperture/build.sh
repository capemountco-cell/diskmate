#!/bin/sh
# Reassembles the six artboards from parts/. The sidebar is generated so that
# the nav selection is the only thing that differs between screens.
set -e
cd "$(dirname "$0")"

build() {
  out="$1"; theme="$2"; active="$3"; main="$4"
  cat parts/head.html "parts/base-$theme.css" parts/base-common.css \
      parts/mid.html parts/wrap-open.html > "$out"
  node parts/mksidebar.mjs "$active" >> "$out"
  printf '\n' >> "$out"
  cat "parts/$main" parts/wrap-close.html parts/tail.html >> "$out"
  echo "  $out"
}

build Main.dc.html       light storage Storage.main.html
build Scanning.dc.html   light storage Scanning.main.html
build Cleanup.dc.html    light cleanup Cleanup.main.html
build Duplicates.dc.html light cleanup Duplicates.main.html
build Memory.dc.html     light memory  Memory.main.html
build Dark.dc.html       dark  storage Storage.main.html
