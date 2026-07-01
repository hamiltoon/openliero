#!/usr/bin/env bash
# Regenerates golden/sim_slice3.txt by building a REAL C++ Game and driving 2 worms
# N ticks of the FULL worm->Process (control/aim/jump/weapon-change/weapon-timer/
# ninjarope) under SCRIPTED input, dumping per-tick master + component hashes (see
# sim_physics_dump.cpp). Uses the SAME unmodified dumper as slice 2 — slice 3 adds
# only a new scenario (with `input` lines) + this driver. Needs the full C++ build
# (links the `game` target), so this is a LOCAL/MANUAL step — it is NOT run in the
# lightweight rust.yml CI. Override PRESET for other platforms (e.g. linux-x64).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_sim_physics
# Run from ROOT so Common::load("data/TC/openliero") and the scenario's relative
# level path resolve.
cd "$ROOT"
"build/$PRESET/Release/oracle_dump_sim_physics" \
  "rust/oracle-tests/golden/sim_slice3_scenario.txt" \
  "rust/oracle-tests/golden/sim_slice3.txt"
echo "wrote rust/oracle-tests/golden/sim_slice3.txt"
