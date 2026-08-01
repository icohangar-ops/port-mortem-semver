#!/usr/bin/env bash
#
# Differential fuzzer: hammers the original npm/node-semver and the Rust port
# with the same randomly generated inputs and fails if they ever disagree.
#
#   ./scripts/diff_fuzz.sh [durationSeconds] [seed]
#
# Writes fuzz/log.txt and exits non-zero if there was any divergence.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

DURATION="${1:-60}"
SEED="${2:-$(date +%s)}"

if [ ! -x "target/release/semver-rpc" ]; then
  echo "building semver-rpc..." >&2
  CARGO_TARGET_DIR="$ROOT/target" cargo build --release >&2
fi

mkdir -p fuzz

echo "fuzzing for ${DURATION}s (seed ${SEED})..." >&2

set +e
node scripts/diff_fuzz.js "$SEED" "$DURATION" > fuzz/log.txt 2>&1
status=$?
set -e

cat fuzz/log.txt

if [ "$status" -ne 0 ]; then
  echo "" >&2
  echo "FAIL: divergences found, see fuzz/log.txt" >&2
  exit 1
fi

echo "" >&2
echo "OK: no divergences" >&2
