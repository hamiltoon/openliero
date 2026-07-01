#!/usr/bin/env bash
# Regenerates golden/sim_slice5prime_nobj.txt by building a REAL C++ Game and driving
# 2 worms N ticks of the FULL ProcessFrame subset (bonuses Process loop + object loops
# BEFORE worms, then each worm->Process, then ++cycles + the gated bonus-drop roll)
# under SCRIPTED input, dumping per-tick master + component hashes (see
# sim_physics_dump.cpp).
# Slice 5'a T7 makes the per-pixel in-flight NOBJECT worm-hit arm (nobject.cpp:166-203)
# LIVE with NO C++ dumper change: the dumper already drives the unmodified
# `NObject::Process`, which reads `common.WormSprite(...)` + `common.materials[...]`
# (fully loaded). The scenario (max_bonuses 0) has worm0 fire the CANNON so its shell
# clears worm1's head, strikes the left wall, and scatters 5 splinters
# (`particle__small_damage`); one splinter flies back EAST and its single pixel crosses
# worm1's SOLID silhouette on exactly one tick — the per-pixel `CheckForSpecWormHit`
# opens the NOBJECT arm (DoDamage 2 + the rand(3) hit-sound gate FIRST + the 1-blood
# fan SECOND, then worm_destroy frees the splinter), wounding worm1 50->48 WITHOUT
# killing; the transparent-corner ticks around it fire NOTHING (the anti-false-positive
# near-miss witness). This is the MIRROR of T6's wobject arm (blood-then-sound) — here
# the order is sound-then-blood (LJUD FÖRE BLOD). Needs the full C++ build (links the
# `game` target), so this is a LOCAL/MANUAL step — it is NOT run in the lightweight
# rust.yml CI. Override PRESET for other platforms (e.g. linux-x64).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
# cd to ROOT up front so `cmake --preset` finds CMakePresets.json, and so
# Common::load("data/TC/openliero") + the scenario's relative level path resolve.
cd "$ROOT"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_sim_physics
"build/$PRESET/Release/oracle_dump_sim_physics" \
  "rust/oracle-tests/golden/sim_slice5prime_nobj_scenario.txt" \
  "rust/oracle-tests/golden/sim_slice5prime_nobj.txt"
echo "wrote rust/oracle-tests/golden/sim_slice5prime_nobj.txt"
