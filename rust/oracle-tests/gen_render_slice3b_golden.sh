#!/usr/bin/env bash
# Regenerates the Slice-3b render goldens (the sprite + shadow world view): fan, dart,
# blood, shadow, laser, shake, dart_water. Builds the REAL C++ Game + a headless Renderer, drives each
# scenario's worms N ticks, and for each tick dumps (a) the normal 11-column sim record
# (render_slice3b_<name>_sim.txt, the isolation source) and (b) a sidecar frame golden
# (render_slice3b_<name>.txt: <tick> <frame_hash16> <state_hash8> + a final `total`). Full
# world draw (shadow pass gated by `render_shadow`; shake/flash by `render_shake`/
# `render_flash`). LOCAL/MANUAL — needs the full C++ build (links `game`); NOT in the
# lightweight rust.yml CI. Override PRESET for other platforms.
#
# Also (re)generates the two purpose-built fixture levels this corpus needs — render_stage.lev
# (high floor so grounded worms are inside the fixed [0,158) framehash window; the reduced
# dumper never re-centres its viewports, so the camera stays at world origin) and
# see_shadow_test.lev (a kSeeShadow background band so the shadow pass paints visible pixels)
# and water_stage.lev (render_stage geometry with a water-range [160,168) sky band so the
# dart's sobject explosion sprite lands over water and BlitImageR paints — the positive proof).
#
# The 3rd dumper arg is the seed passed EXPLICITLY (42, matching each scenario); the 4th arg
# is the sidecar frame-golden path. The dumper enables its opt-in render path from the
# scenario's `render player` directive. Adds NOTHING to any prior golden — only writes the
# new render_slice3b_* files (re-diff discipline: existing goldens are untouched).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"

# Fixture levels (idempotent; read material indices from the live tc.cfg).
python3 "$ROOT/rust/oracle-tests/gen_render_stage_level.py"
python3 "$ROOT/rust/oracle-tests/gen_see_shadow_level.py"
python3 "$ROOT/rust/oracle-tests/gen_water_stage_level.py"

cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_sim_physics

BIN="$ROOT/build/$PRESET/Release/oracle_dump_sim_physics"
GOLD="$ROOT/rust/oracle-tests/golden"
cd "$ROOT"
for name in fan dart blood shadow laser shake dart_water; do
  "$BIN" \
    "$GOLD/render_slice3b_${name}_scenario.txt" \
    "$GOLD/render_slice3b_${name}_sim.txt" \
    "42" \
    "$GOLD/render_slice3b_${name}.txt"
  echo "wrote render_slice3b_${name}{_sim,}.txt"
done
