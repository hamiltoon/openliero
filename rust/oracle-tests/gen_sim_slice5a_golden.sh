#!/usr/bin/env bash
# Regenerates golden/sim_slice5a.txt by building a REAL C++ Game and driving 2 worms
# N ticks of the FULL ProcessFrame subset (object loops BEFORE worms, then each
# worm->Process: control/aim/Fire/weapon-timer) under SCRIPTED input, dumping per-tick
# master + component hashes (see sim_physics_dump.cpp). Slice 5a adds only a new
# scenario (worm0 FIRES the CANNON, which arcs into the dirt and EXPLODES into a
# medium_explosion sobject -> dirt-debris + 5 splinter nobjects + carved level) and this
# driver; the dumper is UNCHANGED from slice 4b (object loops + `weapon <slot> <name>`
# directive + settings->shadow=false). The cannon's explosion reaches SObject::Create /
# NObject automatically — no dumper edit. Needs the full C++ build (links the `game`
# target), so this is a LOCAL/MANUAL step — it is NOT run in the lightweight rust.yml
# CI. Override PRESET for other platforms (e.g. linux-x64).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_sim_physics
# Run from ROOT so Common::load("data/TC/openliero") and the scenario's relative
# level path resolve.
cd "$ROOT"
"build/$PRESET/Release/oracle_dump_sim_physics" \
  "rust/oracle-tests/golden/sim_slice5a_scenario.txt" \
  "rust/oracle-tests/golden/sim_slice5a.txt"
echo "wrote rust/oracle-tests/golden/sim_slice5a.txt"
