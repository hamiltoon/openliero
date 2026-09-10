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
MARGIN=200  # ticks recorded after the game-over tick (>= the 180-frame C++ post-mortem)
for v in defaults killemall scales gametag; do
  out="rust/oracle-tests/golden/sim_slice4_5a_${v}.txt"
  "build/$PRESET/Release/oracle_dump_sim_physics" \
    "rust/oracle-tests/golden/sim_slice4_5a_${v}_scenario.txt" \
    "$out"
  echo "wrote $out"
  # C++-SIDE GATE on column 12 (Game::IsGameOver). `defaults` is the T7 smoke: a match
  # that never ends, so its column must stay 0 throughout. Every settings MATRIX variant
  # must instead flip 0 -> 1 exactly once and stay 1 for at least MARGIN rows — that is
  # what makes the golden prove the game-over seam and gives T9 a "stays true" window.
  # A failure aborts the script (set -e + a non-zero awk exit), so a bad golden is never
  # left looking regenerated-and-fine.
  if [ "$v" = defaults ]; then
    awk '
      $12 != "0" { printf "FAIL defaults: IsGameOver=%s at tick %s (must stay 0)\n", $12, $1; bad = 1; exit 1 }
      END { if (bad) { exit 1 } printf "  gate %s: %d rows, IsGameOver constant 0\n", "defaults", NR }
    ' "$out"
  else
    awk -v v="$v" -v margin="$MARGIN" '
      NR == 1 { first = $12; prev = $12 }
      $12 != prev { flips++; flip_tick = $1; flip_row = NR; prev = $12 }
      END {
        if (first != "0") { printf "FAIL %s: first row IsGameOver=%s (want 0)\n", v, first; exit 1 }
        if (flips != 1) { printf "FAIL %s: IsGameOver changed %d times (want exactly 1)\n", v, flips + 0; exit 1 }
        if (prev != "1") { printf "FAIL %s: last row IsGameOver=%s (want 1)\n", v, prev; exit 1 }
        trail = NR - flip_row + 1
        if (trail < margin) { printf "FAIL %s: %d trailing 1 rows (want >= %d)\n", v, trail, margin; exit 1 }
        printf "  gate %s: %d rows, IsGameOver 0->1 at tick %s, %d trailing 1s\n", v, NR, flip_tick, trail
      }
    ' "$out"
  fi
done
