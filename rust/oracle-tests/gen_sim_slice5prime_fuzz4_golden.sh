#!/usr/bin/env bash
# Regenerates golden/sim_slice5prime_fuzz4.txt by building a REAL C++ Game and driving
# 2 worms N ticks of the FULL ProcessFrame subset (object loops BEFORE worms, then each
# worm->Process, then ++cycles + the gated bonus-drop roll) under SCRIPTED input, dumping
# per-tick master + component hashes (see sim_physics_dump.cpp).
# Slice 5' T10 makes the per-pixel in-flight worm-hit arm (weapon.cpp:287-326) land on a
# MOVING worm with NO C++ dumper change: the dumper already drives the unmodified
# WObject::Process worm-hit loop, which reads common.WormSprite(current_frame, direction)
# + common.materials. Variant 4 of 4 of the moving-worms fuzz: BOTH worms walk
# (animate=true) and fire flat DARTs at each other; the crossing darts per-pixel-hit the
# opposing MOVING worm at a walk-animation current_frame/direction (the coverage the
# idle-victim milestone cannot give). Needs the full C++ build (links the `game` target),
# so this is a LOCAL/MANUAL step — it is NOT run in the lightweight rust.yml CI. Override
# PRESET for other platforms (e.g. linux-x64).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
# cd to ROOT up front so `cmake --preset` finds CMakePresets.json, and so
# Common::load("data/TC/openliero") + the scenario's relative level path resolve.
cd "$ROOT"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_sim_physics
"build/$PRESET/Release/oracle_dump_sim_physics" \
  "rust/oracle-tests/golden/sim_slice5prime_fuzz4_scenario.txt" \
  "rust/oracle-tests/golden/sim_slice5prime_fuzz4.txt"
echo "wrote rust/oracle-tests/golden/sim_slice5prime_fuzz4.txt"
