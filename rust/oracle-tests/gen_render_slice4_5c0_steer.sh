#!/usr/bin/env bash
# Regenerates the Step-4½c-0 STEERABLE-CAMERA live render golden (design §5.4). Builds the REAL
# C++ Game + a headless Renderer and drives render_slice4_5c0_steer_scenario.txt through the
# opt-in `render_live` path (real Game::ProcessFrame — inputs applied first — plus the two
# viewports wired into ProcessViewports, whose alive arm centres a steering worm's viewport on
# its missiles' centroid, viewport.cpp:30-32). Writes the 11-column sim record
# (render_slice4_5c0_steer_sim.txt) and the frame sidecar (render_slice4_5c0_steer.txt:
# <tick> <frame_hash16> <state_hash8> + `total`). LOCAL/MANUAL — needs the full C++ build.
# The dumper's 3rd argument is the seed, read from the scenario (the generator scanned it).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cd "$ROOT"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_sim_physics
BIN="$ROOT/build/$PRESET/Release/oracle_dump_sim_physics"
GOLD="$ROOT/rust/oracle-tests/golden"
SCN="$GOLD/render_slice4_5c0_steer_scenario.txt"
SEED="$(awk '$1 == "seed" { print $2 }' "$SCN")"
"$BIN" "$SCN" "$GOLD/render_slice4_5c0_steer_sim.txt" "$SEED" "$GOLD/render_slice4_5c0_steer.txt"
echo "wrote render_slice4_5c0_steer{_sim,}.txt (seed $SEED)"
