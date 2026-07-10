#!/usr/bin/env bash
# Regenerates golden/sim_slice6_scales.txt by building a REAL C++ Game with
# Settings::game_mode = kGmScalesOfJustice (3) and driving 2 worms N ticks of the
# full ProcessFrame subset. Scales lives inside the unmodified `Game::DoDamage`
# (game.cpp:567-589): worm0's EXPLOSIVES air-bursts WOUND worm1, and the damage is
# redistributed as HEALING to the attacker worm0 via `DoHealingDirect`. No C++
# dumper change (the Scales branch fires off `settings->game_mode`, set from the
# scenario's `game_mode`). Needs the full C++ build (links the `game` target), so
# this is a LOCAL/MANUAL step — NOT run in the lightweight rust.yml CI. Override
# PRESET for other platforms (e.g. linux-x64).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cd "$ROOT"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_sim_physics
"build/$PRESET/Release/oracle_dump_sim_physics" \
  "rust/oracle-tests/golden/sim_slice6_scales_scenario.txt" \
  "rust/oracle-tests/golden/sim_slice6_scales.txt"
echo "wrote rust/oracle-tests/golden/sim_slice6_scales.txt"
