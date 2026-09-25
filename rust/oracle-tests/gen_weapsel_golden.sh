#!/usr/bin/env bash
# Regenerates golden/weapsel_<case>.txt — the C++ weapon-selection phase, line for line (Step 4½
# slice 4½c, design §6.2-§6.4). oracle_dump_weapsel runs the REAL WeaponSelection over every
# committed weapsel_<case>_scenario.txt (+ its _setup.cfg, read by the REAL Settings::FromToml)
# and self-checks its input replica against a real LocalController. The scenario/setup inputs
# are written by `cargo run -p oracle-tests --example gen_slice4_5c -- write <golden dir>` and
# never touched here. Needs the full C++ build (links the `game` target), so this is a
# LOCAL/MANUAL step — NOT run in the lightweight rust.yml CI. Override PRESET for other
# platforms (e.g. linux-x64). cd's to ROOT, so any cwd works.
#
# The same run also writes the RENDER sidecar golden/weapsel_<case>_frames.txt (plan Addendum
# A1: the Rust weapon-selection screen is gated bit-exact against these C++ frames in T8): one
# line per frame the REAL WeaponSelection::Draw drew (every frame but the one the phase ends on),
#   <frame> <frame_hash16 at fade 33> <menu_cycles the draw read> <real fade = min(frame+1, 33)>
# under a '#' header naming the --menu-cycles start. The golden text is byte-identical with or
# without render mode. Every case is a render case except bots_only_7 (done on frame 0: nothing
# is drawn). MENU_CYCLES below is the per-case start of Gfx::menu_cycles (the real game inherits
# the main menu's); the non-zero ones vary the frozen frame's palette phase, and repeat_edge's
# wraps the unsigned counter (2^32 % 7 != 0) on its frame 16. Set WEAPSEL_PPM_DIR to also write
# every drawn frame as <dir>/<case>/weapsel_NNNN.ppm (eyeballing only; never committed).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cd "$ROOT"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON -DCMAKE_EXPORT_COMPILE_COMMANDS=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_weapsel
declare -A MENU_CYCLES=(
  [disabled_saved_s1]=3
  [disabled_saved_s2]=1000
  [few_enabled]=1
  [repeat_edge]=4294967280
  [match_bot]=6
)
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT
n=0
for scn in rust/oracle-tests/golden/weapsel_*_scenario.txt; do
  c="$(basename "$scn" _scenario.txt)"
  case="${c#weapsel_}"
  out="rust/oracle-tests/golden/${c}.txt"
  end=$(awk '$1 == "weapsel" { f = $2 } END { print f }' "$scn")
  render=()
  cycles="${MENU_CYCLES[$case]:-0}"
  if [ "$end" -gt 0 ]; then
    render=(--frames "$tmp" --menu-cycles "$cycles")
    if [ -n "${WEAPSEL_PPM_DIR:-}" ]; then
      mkdir -p "$WEAPSEL_PPM_DIR/$case"
      render+=(--ppm-dir "$WEAPSEL_PPM_DIR/$case")
    fi
  fi
  "build/$PRESET/Release/oracle_dump_weapsel" "$scn" "$out" "${render[@]}"
  # C++-SIDE GATE (design §6.3): exactly one 7-field `init` first; 15-field `f` lines for
  # frames 0..end in order, done=1 only on the last `weapsel` frame (§6.1); one 7-field `final`.
  awk -v c="$c" -v end="$end" '
    BEGIN { frames = 0; finals = 0; lines = 0 }
    /^#/ { next }
    { lines++ }
    lines == 1 { if ($1 != "init" || NF != 7) { printf "FAIL %s: line 1 is not a 7-field init\n", c; exit 1 } next }
    $1 == "f" {
      if (NF != 15) { printf "FAIL %s: f line with %d fields\n", c, NF; exit 1 }
      if ($2 != frames) { printf "FAIL %s: frame %s out of order (want %d)\n", c, $2, frames; exit 1 }
      if ($15 != ($2 == end ? "1" : "0")) { printf "FAIL %s: done=%s on frame %s\n", c, $15, $2; exit 1 }
      frames++; next
    }
    $1 == "final" { if (NF != 7 || finals++) { printf "FAIL %s: bad final line\n", c; exit 1 } next }
    { printf "FAIL %s: unexpected line: %s\n", c, $0; exit 1 }
    END {
      if (!finals || frames != end + 1) { printf "FAIL %s: %d frames (want %d), final=%d\n", c, frames, end + 1, finals; exit 1 }
      printf "  gate %s: init + %d frames (done on %d) + final\n", c, frames, end
    }
  ' "$out"
  echo "wrote $out"
  if [ "$end" -gt 0 ]; then
    fout="rust/oracle-tests/golden/${c}_frames.txt"
    {
      echo "# oracle_dump_weapsel $scn <out> --frames <this> --menu-cycles $cycles — the REAL"
      echo "# C++ WeaponSelection::Draw, headless (Step 4½c plan Addendum A1). One line per drawn frame"
      echo "# (the frame the phase ends on is not drawn):"
      echo "# <frame> <frame_hash16 at fade 33> <menu_cycles the draw read> <real fade min(frame+1,33)>"
      cat "$tmp"
    } >"$fout"
    # RENDER GATE: frames 0..end-1 in order, 4 fields, a 16-hex hash, menu_cycles = start + k
    # (unsigned 32-bit wrap), fade = min(k+1, 33).
    awk -v c="$c" -v end="$end" -v start="$cycles" '
      BEGIN { k = 0 }
      /^#/ { next }
      {
        if (NF != 4) { printf "FAIL %s frames: %d fields\n", c, NF; exit 1 }
        if ($1 != k) { printf "FAIL %s frames: frame %s out of order (want %d)\n", c, $1, k; exit 1 }
        if ($2 !~ /^[0-9a-f]{16}$/) { printf "FAIL %s frames: bad hash %s\n", c, $2; exit 1 }
        if ($3 != (start + k) % 4294967296) { printf "FAIL %s frames: menu_cycles %s on frame %d\n", c, $3, k; exit 1 }
        if ($4 != (k + 1 < 33 ? k + 1 : 33)) { printf "FAIL %s frames: fade %s on frame %d\n", c, $4, k; exit 1 }
        k++
      }
      END {
        if (k != end) { printf "FAIL %s frames: %d drawn (want %d)\n", c, k, end; exit 1 }
        printf "  gate %s frames: %d drawn, menu_cycles from %s\n", c, k, start
      }
    ' "$fout"
    echo "wrote $fout"
  fi
  n=$((n + 1))
done
test "$n" -eq 16 || { echo "FAIL: $n weapsel scenarios (want 16)"; exit 1; }
