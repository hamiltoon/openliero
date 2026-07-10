#!/usr/bin/env bash
# Regenerates golden/sim_slice4d.txt by building a REAL C++ Game and driving 2 worms
# N ticks of the FULL ProcessFrame subset (object loops BEFORE worms, then each
# worm->Process: control/aim/ProcessWeapons/Fire/ProcessSight/ProcessWeaponChange/
# ProcessMovement) under SCRIPTED input, dumping per-tick master + component hashes
# (see sim_physics_dump.cpp). Slice 4d adds only a new scenario (worm0 wields the
# HANDGUN with low ammo, FIRES TWICE to empty the magazine -> reload; each fire leaves
# a SHELL nobject; a weapon-change is held DURING the reload -> load_change cycle; then
# a dig window carves the dirt floor) and this driver; the dumper is UNCHANGED from
# slice 4c except for the optional `[ammo]` 3rd token on the `weapon` directive (added
# in slice-4d task 6). The handgun reaches reload/shell/dig/load_change/laser-sight
# through real game code -- no further dumper edit. Needs the full C++ build (links the
# `game` target), so this is a LOCAL/MANUAL step -- it is NOT run in the lightweight
# rust.yml CI. Override PRESET for other platforms (e.g. linux-x64).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_sim_physics
# Run from ROOT so Common::load("data/TC/openliero") and the scenario's relative
# level path resolve.
cd "$ROOT"
"build/$PRESET/Release/oracle_dump_sim_physics" \
  "rust/oracle-tests/golden/sim_slice4d_scenario.txt" \
  "rust/oracle-tests/golden/sim_slice4d.txt"
echo "wrote rust/oracle-tests/golden/sim_slice4d.txt"
