#!/usr/bin/env bash
# Regenerates the Slice-3e RELOAD render golden (loading bar + blinking "Reloading"
# text). Builds the REAL C++ Game + a headless Renderer, drives the scenario N ticks,
# and for each tick dumps (a) the normal 11-column sim record
# (render_slice3e_reload_sim.txt, the isolation source) and (b) a sidecar frame golden
# (render_slice3e_reload.txt: <tick> <frame_hash16> <state_hash8> + a final `total`).
# worm0's single-ammo RIFLE fires once and reloads for the rest of the window, so the
# HUD reload arm (viewport.cpp:110-128) draws the growing loading bar + the blinking
# Reloading text behind the scenario's `render_hud` directive. LOCAL/MANUAL — needs the
# full C++ build (links `game`); NOT in the lightweight rust.yml CI. Override PRESET.
#
# The 3rd dumper arg is the seed passed EXPLICITLY (42, matching the scenario); the 4th
# arg is the sidecar frame-golden path. Adds NOTHING to any prior golden — only writes
# the new render_slice3e_reload_* files (re-diff discipline: existing goldens untouched).
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
  "$GOLD/render_slice3e_reload_scenario.txt" \
  "$GOLD/render_slice3e_reload_sim.txt" \
  "42" \
  "$GOLD/render_slice3e_reload.txt"
echo "wrote render_slice3e_reload{_sim,}.txt"
