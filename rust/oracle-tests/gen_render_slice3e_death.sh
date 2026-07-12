#!/usr/bin/env bash
# Regenerates the Slice-3e DEATH render golden (the dying life-bar countdown arm).
# Builds the REAL C++ Game + a headless Renderer, drives the scenario N ticks, and for
# each tick dumps (a) the normal 11-column sim record (render_slice3e_death_sim.txt, the
# isolation source) and (b) a sidecar frame golden (render_slice3e_death.txt: <tick>
# <frame_hash16> <state_hash8> + a final `total`). worm0's EXPLOSIVES kills worm1 (5d
# geometry); while worm1 is dead the HUD draws the DYING life bar (viewport.cpp:88-93,
# 100-(killed_timer*25)/37) which grows as killed_timer counts down — behind the
# scenario's `render_hud` directive. Banners are NOT drawn (banner_y state is stepped in
# unrun viewport-processing; Step-4 deferral). LOCAL/MANUAL — needs the full C++ build
# (links `game`); NOT in the lightweight rust.yml CI. Override PRESET.
#
# The 3rd dumper arg is the seed passed EXPLICITLY (42, matching the scenario); the 4th
# arg is the sidecar frame-golden path. Adds NOTHING to any prior golden — only writes
# the new render_slice3e_death_* files (re-diff discipline: existing goldens untouched).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cd "$ROOT"

cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_sim_physics

BIN="$ROOT/build/$PRESET/Release/oracle_dump_sim_physics"
GOLD="$ROOT/rust/oracle-tests/golden"
cd "$ROOT"
"$BIN" \
  "$GOLD/render_slice3e_death_scenario.txt" \
  "$GOLD/render_slice3e_death_sim.txt" \
  "42" \
  "$GOLD/render_slice3e_death.txt"
echo "wrote render_slice3e_death{_sim,}.txt"
