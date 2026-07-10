#!/usr/bin/env bash
# Regenerates the Slice-3a render goldens: builds the REAL C++ Game + a headless
# Renderer, drives 2 static worms N ticks, and for each tick dumps (a) the normal
# 11-column sim record (render_slice3a_sim.txt, the isolation source) and (b) a
# sidecar frame golden (render_slice3a.txt: <tick> <frame_hash16> <state_hash8> +
# total). Terrain-only draw (no HUD/shadow/sprite/minimap). LOCAL/MANUAL — needs
# the full C++ build (links `game`); NOT in the lightweight rust.yml CI. Override
# PRESET for other platforms.
#
# The 3rd arg is the seed passed EXPLICITLY (42, matching the scenario) so the
# sidecar 4th-arg slot is unambiguous; the dumper enables its opt-in render path
# from the scenario's `render player` directive and writes the sidecar to argv[4].
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_sim_physics
cd "$ROOT"
"build/$PRESET/Release/oracle_dump_sim_physics" \
  "rust/oracle-tests/golden/render_slice3a_scenario.txt" \
  "rust/oracle-tests/golden/render_slice3a_sim.txt" \
  "42" \
  "rust/oracle-tests/golden/render_slice3a.txt"
echo "wrote rust/oracle-tests/golden/render_slice3a{_sim,}.txt"
