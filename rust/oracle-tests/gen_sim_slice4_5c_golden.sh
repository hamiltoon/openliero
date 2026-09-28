#!/usr/bin/env bash
# Regenerates golden/sim_slice4_5c_match_{humans,bot}.txt — 12 columns (the 11
# oracle_dump_sim_physics hashes + Game::IsGameOver) for the Step-4½ slice-4½c CONTINUATION
# matches (design §6.5): the `settings` path runs the REAL weapon-selection phase (the scenario's
# `weapsel` lines, via weapsel_drive.hpp) in place of InitWeapons, then 600 fuzz ticks. Needs the
# full C++ build, so this is a LOCAL/MANUAL step. Override PRESET (e.g. linux-x64).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cd "$ROOT"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON -DCMAKE_EXPORT_COMPILE_COMMANDS=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_sim_physics
for v in humans bot; do
  scn="rust/oracle-tests/golden/weapsel_match_${v}_scenario.txt"
  out="rust/oracle-tests/golden/sim_slice4_5c_match_${v}.txt"
  "build/$PRESET/Release/oracle_dump_sim_physics" "$scn" "$out"
  echo "wrote $out"
  ticks=$(awk '$1 == "ticks" { print $2 }' "$scn")
  # C++-SIDE GATE: 12 columns, IsGameOver constant 0 (lives 99), ticks+1 rows, and a NON-ZERO
  # tick-0 rng (column 3): the selection's draws reached the match (design §6.5).
  awk -v v="$v" -v ticks="$ticks" '
    NF != 12 { printf "FAIL %s: %d columns at tick %s\n", v, NF, $1; bad = 1; exit 1 }
    $12 != "0" { printf "FAIL %s: IsGameOver=%s at tick %s (lives 99)\n", v, $12, $1; bad = 1; exit 1 }
    NR == 1 && $3 == "00000000" { printf "FAIL %s: tick-0 rng is 0 (no selection draw)\n", v; bad = 1; exit 1 }
    END {
      if (bad) { exit 1 }
      if (NR != ticks + 1) { printf "FAIL %s: %d rows (want %d)\n", v, NR, ticks + 1; exit 1 }
      printf "  gate %s: %d rows, 12 columns, IsGameOver 0, tick-0 rng non-zero\n", v, NR
    }
  ' "$out"
done
