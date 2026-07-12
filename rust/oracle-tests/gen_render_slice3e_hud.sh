#!/usr/bin/env bash
# Regenerates the Slice-3e HUD headline render golden (the full player view: world +
# HUD bars/text + 52x36 minimap). Builds the REAL C++ Game + a headless Renderer,
# drives the scenario N ticks, and for each tick dumps (a) the normal 11-column sim
# record (render_slice3e_hud_sim.txt, the isolation source) and (b) a sidecar frame
# golden (render_slice3e_hud.txt: <tick> <frame_hash16> <state_hash8> + a final
# `total`). The HUD/minimap are drawn behind the scenario's opt-in `render_hud`
# directive (viewport.cpp:84-153 pre-block + :593-613 minimap), 1:1 with the Rust
# frame::draw HUD path. LOCAL/MANUAL — needs the full C++ build (links `game`); NOT in
# the lightweight rust.yml CI. Override PRESET for other platforms.
#
# The 3rd dumper arg is the seed passed EXPLICITLY (42, matching the scenario); the 4th
# arg is the sidecar frame-golden path. Adds NOTHING to any prior golden — only writes
# the new render_slice3e_hud_* files (re-diff discipline: existing goldens untouched).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cd "$ROOT"

# Fixture level (idempotent; reads material indices from the live tc.cfg).
python3 "$ROOT/rust/oracle-tests/gen_render_stage_level.py"

cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_sim_physics

BIN="$ROOT/build/$PRESET/Release/oracle_dump_sim_physics"
GOLD="$ROOT/rust/oracle-tests/golden"
cd "$ROOT"
"$BIN" \
  "$GOLD/render_slice3e_hud_scenario.txt" \
  "$GOLD/render_slice3e_hud_sim.txt" \
  "42" \
  "$GOLD/render_slice3e_hud.txt"
echo "wrote render_slice3e_hud{_sim,}.txt"
