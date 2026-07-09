#!/usr/bin/env bash
# Regenerates golden/sim_slice5prime_pickup_health.txt (Slice 5'b T9, variant a) by
# building a REAL C++ Game and driving it under the scripted walk-on scenario: worm0,
# wounded to health 50, walks RIGHT onto a dropped FRAME-1 (health) bonus and HEALS. The
# dumper (oracle_dump_sim_physics) is UNCHANGED — it already drives the full
# `Worm::Process` incl. the pickup block (`worm.cpp:287-322`). Needs the full C++ build
# (links the `game` target), so this is a LOCAL/MANUAL step — NOT run in the lightweight
# rust.yml CI. Override PRESET for other platforms (e.g. linux-x64).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cd "$ROOT"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_sim_physics
"build/$PRESET/Release/oracle_dump_sim_physics" \
  "rust/oracle-tests/golden/sim_slice5prime_pickup_health_scenario.txt" \
  "rust/oracle-tests/golden/sim_slice5prime_pickup_health.txt"
echo "wrote rust/oracle-tests/golden/sim_slice5prime_pickup_health.txt"
