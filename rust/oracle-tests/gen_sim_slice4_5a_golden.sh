#!/usr/bin/env bash
# Regenerates golden/sim_slice4_5a_<variant>.txt — 12 columns: the 11 oracle_dump_sim_physics
# hash columns + Game::IsGameOver() — for the Step-4½ slice-4½a-1 SETTINGS-DRIVEN scenarios.
# Each scenario's `settings <file>` sidecar is read by the REAL C++ Settings::FromToml and the
# worms start in the C++ LocalController state (sim_physics_dump.cpp `settings` path).
# Needs the full C++ build (links the `game` target), so this is a LOCAL/MANUAL step — NOT run
# in the lightweight rust.yml CI. Override PRESET for other platforms (e.g. linux-x64).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cd "$ROOT"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_sim_physics
for v in defaults killemall scales gametag; do
  "build/$PRESET/Release/oracle_dump_sim_physics" \
    "rust/oracle-tests/golden/sim_slice4_5a_${v}_scenario.txt" \
    "rust/oracle-tests/golden/sim_slice4_5a_${v}.txt"
  echo "wrote rust/oracle-tests/golden/sim_slice4_5a_${v}.txt"
done
