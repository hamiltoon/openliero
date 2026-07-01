#!/usr/bin/env bash
# Regenerates golden/sim_slice5d_fuzz4.txt by building a REAL C++ Game and driving 2 worms
# N ticks of the FULL ProcessFrame subset (bonuses Process loop + object loops BEFORE
# worms, then each worm->Process, then ++cycles + the gated bonus-drop roll) under
# SCRIPTED input, dumping per-tick master + component hashes (see sim_physics_dump.cpp).
# Slice 5d makes the worm-loop DEATH + RESPAWN path LIVE with NO C++ dumper change: the
# dumper already drives the unmodified `Worm::Process` with `quick_sim==false` and
# `blood==100`. The scenario (max_bonuses 0) has worm0 fire EXPLOSIVES to air-burst
# worm1, which starts at health 8 (< settings->health/4) so it DRIPS pre-death then DIES
# from the blast (death block: --lives + worm0 kills++ + 120-blood/8-gib spray), and after
# a 150-tick killed_timer countdown RESPAWNS (BeginRespawn spawn search + DoRespawning
# convergence + `rand()&1`). Needs the full C++ build (links the `game` target), so this
# is a LOCAL/MANUAL step — it is NOT run in the lightweight rust.yml CI. Override PRESET
# for other platforms (e.g. linux-x64).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
# cd to ROOT up front so `cmake --preset` finds CMakePresets.json, and so
# Common::load("data/TC/openliero") + the scenario's relative level path resolve.
cd "$ROOT"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_sim_physics
"build/$PRESET/Release/oracle_dump_sim_physics" \
  "rust/oracle-tests/golden/sim_slice5d_fuzz4_scenario.txt" \
  "rust/oracle-tests/golden/sim_slice5d_fuzz4.txt"
echo "wrote rust/oracle-tests/golden/sim_slice5d_fuzz4.txt"
