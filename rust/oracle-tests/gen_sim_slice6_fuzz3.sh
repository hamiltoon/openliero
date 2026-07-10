#!/usr/bin/env bash
# Regenerates golden/sim_slice6_fuzz3.txt from the C++ oracle by driving the SAME
# scenario (golden/sim_slice6_fuzz3_scenario.txt) the Rust difftest reads. Slice-6 T7
# >1000-tick FUZZ variant 3 of 5: 2 worms seeded DEAD (visible 0, pos 0 0,
# killed_timer 150 via ResetWorms, high lives) that respawn in-sim, driven 1500 ticks
# by per-tick random inputs `input_rng() & 0x7f` PRE-EXPANDED into the scenario's
# `input` lines (mirror of src/tests/test_determinism.cpp; CONTROLLER DECISION: NO
# dumper change — the scenario is the single source of truth). game seed 175, input
# seed 54321, max_bonuses 4, real arena Levels/modern_test.lev. The reproducible
# generator that expanded the inputs is rust/oracle-tests/examples/gen_slice6_fuzz.rs.
# Needs the full C++ build (links the `game` target), so this is a LOCAL/MANUAL step —
# NOT run in the lightweight rust.yml CI. Override PRESET for other platforms.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cd "$ROOT"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_sim_physics
"build/$PRESET/Release/oracle_dump_sim_physics" \
  "rust/oracle-tests/golden/sim_slice6_fuzz3_scenario.txt" \
  "rust/oracle-tests/golden/sim_slice6_fuzz3.txt"
echo "wrote rust/oracle-tests/golden/sim_slice6_fuzz3.txt"
