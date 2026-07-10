#!/usr/bin/env bash
# Regenerates golden/sim_slice6_gametag.txt by building a REAL C++ Game with
# Settings::game_mode = kGmGameOfTag (1) and driving 2 worms N ticks of the full
# ProcessFrame subset (incl. the game-mode switch, game.cpp:372-461, already ported
# in the dumper). worm0 kills worm1 with EXPLOSIVES; worm1 respawns; once BOTH worms
# are visible the frame-tail GameOfTag gate bumps the "it"-worm's timer at each
# 70-cycle boundary. No C++ dumper change (the switch is threaded off the scenario's
# `game_mode`). Needs the full C++ build (links the `game` target), so this is a
# LOCAL/MANUAL step — NOT run in the lightweight rust.yml CI. Override PRESET for
# other platforms (e.g. linux-x64).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cd "$ROOT"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_sim_physics
"build/$PRESET/Release/oracle_dump_sim_physics" \
  "rust/oracle-tests/golden/sim_slice6_gametag_scenario.txt" \
  "rust/oracle-tests/golden/sim_slice6_gametag.txt"
echo "wrote rust/oracle-tests/golden/sim_slice6_gametag.txt"
