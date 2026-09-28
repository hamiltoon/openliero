#!/usr/bin/env bash
# Regenerates golden/sim_slice4_5e_<case>.txt — 12 columns: the 11 oracle_dump_sim_physics hash
# columns + Game::IsGameOver() — for the Step-4½e-1 G3 GENERATED-LEVEL scenarios (plan T7). Each
# scenario's `generate <level_seed>` builds its level with the REAL Level::GenerateFromSettings over
# a Rand seeded with the level seed, and its `settings <file>` sidecar is read by the REAL C++
# Settings::FromToml; the worms start in the C++ LocalController state (sim_physics_dump.cpp).
# Needs the full C++ build (links the `game` target), so this is a LOCAL/MANUAL step — NOT run
# in the lightweight rust.yml CI. Override PRESET for other platforms (e.g. linux-x64).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cd "$ROOT"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_sim_physics
MARGIN=200  # rows recorded after a game-over tick (>= the 180-frame C++ post-mortem)
for c in small odd tall banned; do
  scn="rust/oracle-tests/golden/sim_slice4_5e_${c}_scenario.txt"
  out="rust/oracle-tests/golden/sim_slice4_5e_${c}.txt"
  "build/$PRESET/Release/oracle_dump_sim_physics" "$scn" "$out"
  echo "wrote $out"
  # Optional UB guard: a spawn candidate on a level that is not 504x350 can read past the level's
  # `materials[]` (see examples/gen_slice4_5e1_sim.rs), which the Release dumper survives silently.
  # CHECKED_DUMPER names an oracle_dump_sim_physics built with -D_GLIBCXX_ASSERTIONS and
  # -fsanitize=address (a scratch build dir); it must run clean and write the same bytes.
  if [ -n "${CHECKED_DUMPER:-}" ]; then
    chk="$(mktemp)"
    "$CHECKED_DUMPER" "$scn" "$chk"
    cmp "$chk" "$out"
    rm -f "$chk"
    echo "  checked $c: clean under the checked dumper, byte-identical"
  fi
  ticks="$(awk '$1 == "ticks" { print $2 }' "$scn")"
  # The ledger's expectation for column 12: "never over", or "game over at tick N" (then it
  # must flip 0 -> 1 exactly once, at N, and stay 1 for at least MARGIN rows). `tall` must flip.
  over="$(sed -n 's/^# LEDGER (Rust, driven state): game over at tick \([0-9]*\),.*/\1/p' "$scn")"
  if [ -z "$over" ] && ! grep -q '^# LEDGER (Rust, driven state): never over$' "$scn"; then
    echo "FAIL $c: the scenario header states no game-over expectation"
    exit 1
  fi
  if [ "$c" = tall ] && [ -z "$over" ]; then
    echo "FAIL tall: the ledger must record a game over"
    exit 1
  fi
  # C++-SIDE GATE. A failure aborts the script (set -e + a non-zero awk exit), so a bad golden
  # is never left looking regenerated-and-fine.
  awk -v c="$c" -v ticks="$ticks" -v over="$over" -v margin="$MARGIN" '
    NF != 12 { printf "FAIL %s: row %d has %d columns (want 12)\n", c, NR, NF; bad = 1; exit 1 }
    $1 != NR - 1 { printf "FAIL %s: row %d carries tick %s\n", c, NR, $1; bad = 1; exit 1 }
    NR == 1 { first = $12; prev = $12 }
    $12 != prev { flips++; flip_tick = $1; flip_row = NR; prev = $12 }
    END {
      if (bad) { exit 1 }
      if (NR != ticks + 1) { printf "FAIL %s: %d rows (want ticks + 1 = %d)\n", c, NR, ticks + 1; exit 1 }
      if (first != "0") { printf "FAIL %s: first row IsGameOver=%s (want 0)\n", c, first; exit 1 }
      if (over == "") {
        if (flips != 0) { printf "FAIL %s: IsGameOver changed %d times (ledger: never over)\n", c, flips; exit 1 }
        printf "  gate %s: %d rows, IsGameOver constant 0 (never over)\n", c, NR
      } else {
        if (flips != 1) { printf "FAIL %s: IsGameOver changed %d times (want exactly 1)\n", c, flips + 0; exit 1 }
        if (flip_tick != over) { printf "FAIL %s: IsGameOver flips at %s (ledger: %s)\n", c, flip_tick, over; exit 1 }
        if (prev != "1") { printf "FAIL %s: last row IsGameOver=%s (want 1)\n", c, prev; exit 1 }
        trail = NR - flip_row + 1
        if (trail < margin) { printf "FAIL %s: %d trailing 1 rows (want >= %d)\n", c, trail, margin; exit 1 }
        printf "  gate %s: %d rows, IsGameOver 0->1 at tick %s, %d trailing 1s\n", c, NR, flip_tick, trail
      }
    }
  ' "$out"
done
