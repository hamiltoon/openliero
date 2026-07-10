#!/usr/bin/env bash
# Regenerates golden/sim_slice5c.txt by building a REAL C++ Game and driving 2 worms
# N ticks of the FULL ProcessFrame subset (bonuses Process loop + object loops BEFORE
# worms, then each worm->Process, then ++cycles + the per-tick bonus-drop roll) under
# SCRIPTED input, dumping per-tick master + component hashes (see sim_physics_dump.cpp).
# Slice 5c makes the `bonuses` pool LIVE: the scenario sets `max_bonuses 4`, so the
# per-tick roll `rand(CBonusDropChance)==0` eventually fires and `Game::CreateBonus`
# (T2) drops a weapon/health bonus, which then falls/bounces (`Bonus::Process`, T3).
# NO weapon is fired and both worms are kept clear of the bonus, so the only sobjects
# are the bonus spawn flash (`sobject_types[7]` = teleport_flash, detectRange=0 => the
# chain-loop is inert) and (if reached) the expiry sobject. The dumper is the slice-5c
# T0+T3 build (the bonuses Process loop + the gated bonus-drop roll). Needs the full C++
# build (links the `game` target), so this is a LOCAL/MANUAL step — it is NOT run in the
# lightweight rust.yml CI. Override PRESET for other platforms (e.g. linux-x64).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
# cd to ROOT up front so `cmake --preset` finds CMakePresets.json, and so
# Common::load("data/TC/openliero") + the scenario's relative level path resolve.
cd "$ROOT"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_sim_physics
"build/$PRESET/Release/oracle_dump_sim_physics" \
  "rust/oracle-tests/golden/sim_slice5c_scenario.txt" \
  "rust/oracle-tests/golden/sim_slice5c.txt"
echo "wrote rust/oracle-tests/golden/sim_slice5c.txt"
