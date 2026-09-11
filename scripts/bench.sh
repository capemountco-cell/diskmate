#!/usr/bin/env bash
#
# Run the scanner benchmark and record the result.
#
#   ./scripts/bench.sh                       # default synthetic tree
#   ./scripts/bench.sh --files 500000        # a bigger one
#   ./scripts/bench.sh --path ~/Library      # a real directory
#
# Results are appended to bench-results/ with the machine and the commit, so two
# revisions can be compared on the same input rather than on recollection.
#
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."
mkdir -p bench-results

STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
COMMIT="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
OUT="bench-results/${STAMP}-${COMMIT}.txt"

{
  echo "commit   $COMMIT"
  echo "date     $STAMP"
  echo "host     $(uname -s) $(uname -m)"
  echo "rustc    $(rustc --version)"
  echo
} | tee "$OUT"

cargo bench -p diskmate-core -- "$@" | tee -a "$OUT"

echo
echo "Recorded in $OUT"
