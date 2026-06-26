#!/usr/bin/env bash
# Regenerates golden/level.txt by running the REAL C++ Level::load.
# Needs the full C++ build (links the `game` target), so this is a LOCAL/MANUAL
# step — it is NOT run in the lightweight rust.yml CI. Override PRESET for other
# platforms (e.g. linux-x64).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "build/$PRESET" --config Release --target oracle_dump_level
"build/$PRESET/Release/oracle_dump_level" \
  "$ROOT/data/TC/openliero/Levels/modern_test.lev" \
  "$ROOT/rust/oracle-tests/golden/level.txt"
echo "wrote rust/oracle-tests/golden/level.txt"
