#!/usr/bin/env bash
# Regenerates golden/sim_slice4_5f_<case>.txt for the Step-4½f-1 G-AI and G-HP scenarios (plan T6).
# Every case is a `settings` scenario on a `generate <level_seed>` level (the REAL
# Level::GenerateFromSettings over its own Rand; the sidecar read by the REAL Settings::FromToml;
# the worms in the C++ LocalController start state, sim_physics_dump.cpp).
#   G-AI (ai_*): the `ai` directive — a REAL DumbLieroAI per controller-1 player, run in
#     LocalController's order before each tick — adds columns 13-16: per worm the CPU's word
#     right after the AI step (%02x) and its AI's rand.last (%08x), `- -` for a human, and
#     `- - - -` on the tick-0 row. 16 columns.
#   G-HP (hp_*): unequal per-player healths, the 12 settings-path columns.
# Needs the full C++ build (links the `game` target), so this is a LOCAL/MANUAL step — NOT run
# in the lightweight rust.yml CI. Override PRESET for other platforms (e.g. linux-x64).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cd "$ROOT"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_sim_physics
MARGIN=200  # rows recorded after a game-over tick (>= the 180-frame C++ post-mortem)
for c in ai_idle ai_vs_human ai_vs_ai ai_weapons ai_close hp_killemall hp_scales hp_tag; do
  scn="rust/oracle-tests/golden/sim_slice4_5f_${c}_scenario.txt"
  out="rust/oracle-tests/golden/sim_slice4_5f_${c}.txt"
  "build/$PRESET/Release/oracle_dump_sim_physics" "$scn" "$out"
  echo "wrote $out"
  # Optional checked run: CHECKED_DUMPER names an oracle_dump_sim_physics built with
  # -D_GLIBCXX_ASSERTIONS and -fsanitize=address (a scratch build dir); it must run clean and
  # write the same bytes (the `ai` directive's reacts fill makes a CPU case reproducible there).
  if [ -n "${CHECKED_DUMPER:-}" ]; then
    chk="$(mktemp)"
    ASAN_OPTIONS="${ASAN_OPTIONS:-detect_leaks=0}" "$CHECKED_DUMPER" "$scn" "$chk"
    cmp "$chk" "$out"
    rm -f "$chk"
    echo "  checked $c: clean under the checked dumper, byte-identical"
  fi
  ticks="$(awk '$1 == "ticks" { print $2 }' "$scn")"
  case "$c" in
    ai_*)
      cols=16
      grep -qx 'ai' "$scn" || { echo "FAIL $c: an ai_ case needs the ai directive"; exit 1; }
      ;;
    *)
      cols=12
      if grep -qx 'ai' "$scn"; then echo "FAIL $c: an hp_ case has no ai directive"; exit 1; fi
      ;;
  esac
  # The CPU players (controller = 1) of the sidecar, players 0 and 1: their columns are hex.
  setup="rust/oracle-tests/golden/$(awk '$1 == "settings" { print $2 }' "$scn")"
  cpus="$(awk '/^\[player[12]\]/ { p = substr($1, 8, 1) - 1 } $1 == "controller" && p != "" { if ($3 == 1) printf "%d", p; p = "" }' "$setup")"
  # The ledger's expectation for column 12: "never over", or "game over at tick N" (then it
  # must flip 0 -> 1 exactly once, at N, and stay 1 for at least MARGIN rows).
  over="$(sed -n 's/^# LEDGER (Rust, driven state): game over at tick \([0-9]*\),.*/\1/p' "$scn")"
  if [ -z "$over" ] && ! grep -q '^# LEDGER (Rust, driven state): never over$' "$scn"; then
    echo "FAIL $c: the scenario header states no game-over expectation"
    exit 1
  fi
  if [ "$cols" = 16 ] && [ -z "$cpus" ]; then
    echo "FAIL $c: no CPU player in $setup"
    exit 1
  fi
  # C++-SIDE GATE. A failure aborts the script (set -e + a non-zero awk exit), so a bad golden
  # is never left looking regenerated-and-fine.
  awk -v c="$c" -v ticks="$ticks" -v over="$over" -v margin="$MARGIN" -v cols="$cols" -v cpus="$cpus" '
    function fail(msg) { printf "FAIL %s: row %d: %s\n", c, NR, msg; bad = 1; exit 1 }
    NF != cols { fail(sprintf("%d columns (want %d)", NF, cols)) }
    $1 != NR - 1 { fail(sprintf("carries tick %s", $1)) }
    cols == 16 {
      for (w = 0; w < 2; w++) {
        p = $(13 + 2 * w); l = $(14 + 2 * w)
        if (NR == 1 || index(cpus, w) == 0) {
          if (p != "-" || l != "-") fail(sprintf("worm %d AI columns `%s %s` (want `- -`)", w, p, l))
        } else {
          if (p !~ /^[0-9a-f][0-9a-f]$/ || l !~ /^[0-9a-f]{8}$/) fail(sprintf("worm %d AI columns `%s %s` (want hex)", w, p, l))
          if (NR > 2 && l != lastl[w]) moved[w]++
          lastl[w] = l
        }
      }
    }
    NR == 1 { first = $12; prev = $12 }
    $12 != prev { flips++; flip_tick = $1; flip_row = NR; prev = $12 }
    END {
      if (bad) { exit 1 }
      if (NR != ticks + 1) { printf "FAIL %s: %d rows (want ticks + 1 = %d)\n", c, NR, ticks + 1; exit 1 }
      if (first != "0") { printf "FAIL %s: first row IsGameOver=%s (want 0)\n", c, first; exit 1 }
      ai = ""
      if (cols == 16) {
        for (w = 0; w < 2; w++) {
          if (index(cpus, w) == 0) continue
          if (moved[w] * 2 < NR - 2) { printf "FAIL %s: worm %d rand.last changed on only %d rows\n", c, w, moved[w]; exit 1 }
          ai = ai sprintf(", CPU w%d rand.last changed on %d rows", w, moved[w])
        }
      }
      if (over == "") {
        if (flips != 0) { printf "FAIL %s: IsGameOver changed %d times (ledger: never over)\n", c, flips; exit 1 }
        printf "  gate %s: %d rows x %d columns, IsGameOver constant 0 (never over)%s\n", c, NR, cols, ai
      } else {
        if (flips != 1) { printf "FAIL %s: IsGameOver changed %d times (want exactly 1)\n", c, flips + 0; exit 1 }
        if (flip_tick != over) { printf "FAIL %s: IsGameOver flips at %s (ledger: %s)\n", c, flip_tick, over; exit 1 }
        if (prev != "1") { printf "FAIL %s: last row IsGameOver=%s (want 1)\n", c, prev; exit 1 }
        trail = NR - flip_row + 1
        if (trail < margin) { printf "FAIL %s: %d trailing 1 rows (want >= %d)\n", c, trail, margin; exit 1 }
        printf "  gate %s: %d rows x %d columns, IsGameOver 0->1 at tick %s, %d trailing 1s%s\n", c, NR, cols, flip_tick, trail, ai
      }
    }
  ' "$out"
done
