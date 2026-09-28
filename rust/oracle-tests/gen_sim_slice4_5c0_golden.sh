#!/usr/bin/env bash
# Regenerates golden/sim_slice4_5c0_<variant>.txt — 12 columns: the 11 oracle_dump_sim_physics
# hash columns + Game::IsGameOver() — for the Step-4½ slice-4½c-0 WEAPON-BRANCH scenarios.
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
for v in laser missile trails booby; do
  out="rust/oracle-tests/golden/sim_slice4_5c0_${v}.txt"
  "build/$PRESET/Release/oracle_dump_sim_physics" \
    "rust/oracle-tests/golden/sim_slice4_5c0_${v}_scenario.txt" \
    "$out"
  echo "wrote $out"
  # C++-SIDE GATE: 12 columns on every row, and column 12 (IsGameOver) constant 0 — lives 99,
  # these matches never end (design §5.2). A failure aborts the script (set -e + awk exit 1).
  awk -v v="$v" '
    NF != 12 { printf "FAIL %s: %d columns at tick %s (want 12)\n", v, NF, $1; bad = 1; exit 1 }
    $12 != "0" { printf "FAIL %s: IsGameOver=%s at tick %s (lives 99: want 0)\n", v, $12, $1; bad = 1; exit 1 }
    END { if (bad) { exit 1 } printf "  gate %s: %d rows, 12 columns, IsGameOver constant 0\n", v, NR }
  ' "$out"
done
