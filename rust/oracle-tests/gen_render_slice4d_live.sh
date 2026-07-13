#!/usr/bin/env bash
# Regenerates the Slice-4d LIVE render golden (the flash/shake/banner/centering MILESTONE).
# Builds the REAL C++ Game + a headless Renderer and drives the scenario N ticks through the
# opt-in `render_live` path (real Game::ProcessFrame + the two viewports wired into
# ProcessViewports), so the top-of-frame screen_flash/shake decrements, the (cycles&1)
# banner_y walk, the sobject-create screen_flash + viewport-shake writes, and centering all
# evolve from REAL explosions / a real death + respawn. For each tick it dumps (a) the normal
# 11-column sim record (render_slice4d_live_sim.txt, the isolation source) and (b) a sidecar
# frame golden (render_slice4d_live.txt: <tick> <frame_hash16> <state_hash8> + a final
# `total`). LOCAL/MANUAL — needs the full C++ build (links `game`); NOT in the lightweight
# rust.yml CI. Override PRESET for other platforms.
#
# The 3rd dumper arg is the seed passed EXPLICITLY (42, matching the scenario); the 4th arg
# is the sidecar frame-golden path. Because `render_live` is opt-in (only this scenario sets
# it), the dumper binary change adds NOTHING to any prior golden — only writes the new
# render_slice4d_live_* files (the re-diff gate: existing goldens stay byte-identical).
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
  "$GOLD/render_slice4d_live_scenario.txt" \
  "$GOLD/render_slice4d_live_sim.txt" \
  "42" \
  "$GOLD/render_slice4d_live.txt"
echo "wrote render_slice4d_live{_sim,}.txt"
