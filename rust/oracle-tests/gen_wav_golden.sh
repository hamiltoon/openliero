#!/usr/bin/env bash
# Regenerates golden/wav.txt by running the REAL C++ Common::load (which decodes
# each sounds/<name>.wav into original_data and calls CreateSound). Needs the
# full C++ build (links the `game` target), so this is a LOCAL/MANUAL step —
# NOT run in the lightweight rust.yml CI. Override PRESET for other platforms
# (e.g. linux-x64). Run from the repo root so the TC dir resolves the same way
# the in-tree tests do.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "build/$PRESET" --config Release --target oracle_dump_wav
(
  cd "$ROOT"
  "build/$PRESET/Release/oracle_dump_wav" \
    "data/TC/openliero" \
    "rust/oracle-tests/golden/wav.txt"
)
echo "wrote rust/oracle-tests/golden/wav.txt"
