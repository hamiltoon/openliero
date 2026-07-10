#!/usr/bin/env bash
# Regenerates golden/sim_slice5prime.txt by building a REAL C++ Game and driving 2 worms
# N ticks of the FULL ProcessFrame subset (bonuses Process loop + object loops BEFORE
# worms, then each worm->Process, then ++cycles + the gated bonus-drop roll) under
# SCRIPTED input, dumping per-tick master + component hashes (see sim_physics_dump.cpp).
# Slice 5'a makes the per-pixel in-flight wobject worm-hit arm (weapon.cpp:287-326) LIVE
# with NO C++ dumper change: the dumper already drives the unmodified `WObject::Process`,
# which reads `common.WormSprite(...)` + `common.materials[...]` (fully loaded). The
# scenario (max_bonuses 0) has worm0 fire the open-gate DART so its single descending
# projectile crosses worm1's SOLID silhouette on exactly one tick — the per-pixel
# `CheckForSpecWormHit` opens the arm (DoDamage 5 + the 10-blood fan + rand(3) gate),
# wounding worm1 50->45 WITHOUT killing; the transparent-corner ticks around it fire
# NOTHING (the anti-false-positive near-miss witness). Needs the full C++ build (links the
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
  "rust/oracle-tests/golden/sim_slice5prime_scenario.txt" \
  "rust/oracle-tests/golden/sim_slice5prime.txt"
echo "wrote rust/oracle-tests/golden/sim_slice5prime.txt"
