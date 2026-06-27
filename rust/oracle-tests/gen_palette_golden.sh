#!/usr/bin/env bash
# Regenerates golden/palette.txt by running the REAL C++ Palette ops.
# Needs the full C++ build (links the `game` target), so this is a LOCAL/MANUAL
# step — it is NOT run in the lightweight rust.yml CI. Override PRESET for other
# platforms (e.g. linux-x64).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "build/$PRESET" --config Release --target oracle_dump_palette
"build/$PRESET/Release/oracle_dump_palette" \
  "$ROOT/rust/oracle-tests/golden/palette.txt"
echo "wrote rust/oracle-tests/golden/palette.txt"
