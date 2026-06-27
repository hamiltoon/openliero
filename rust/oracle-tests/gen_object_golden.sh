#!/usr/bin/env bash
# Regenerates golden/object.txt by running the REAL C++ Common::load (which reads
# weapons/nobjects/sobjects .cfg and resolves cross-references). Needs the full
# C++ build (links the `game` target), so this is a LOCAL/MANUAL step — NOT run in
# the lightweight rust.yml CI. Override PRESET for other platforms (e.g.
# linux-x64). Run from the repo root so the TC dir resolves like the in-tree
# tests do.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "build/$PRESET" --config Release --target oracle_dump_object
(
  cd "$ROOT"
  "build/$PRESET/Release/oracle_dump_object" \
    "data/TC/openliero" \
    "rust/oracle-tests/golden/object.txt"
)
echo "wrote rust/oracle-tests/golden/object.txt"
