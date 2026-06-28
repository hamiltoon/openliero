#!/usr/bin/env bash
# Regenerates golden/sim_slice1.txt by building a REAL C++ Game to its tick-0
# state and dumping HashGameState + HashGameComponents (see sim_dump.cpp).
# Needs the full C++ build (links the `game` target), so this is a LOCAL/MANUAL
# step — it is NOT run in the lightweight rust.yml CI. Override PRESET for other
# platforms (e.g. linux-x64).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
SEED="${SEED:-42}"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_sim
# Run from ROOT so Common::load("data/TC/openliero") resolves.
cd "$ROOT"
"build/$PRESET/Release/oracle_dump_sim" \
  "data/TC/openliero/Levels/modern_test.lev" \
  "rust/oracle-tests/golden/sim_slice1.txt" \
  "$SEED"
echo "wrote rust/oracle-tests/golden/sim_slice1.txt"
