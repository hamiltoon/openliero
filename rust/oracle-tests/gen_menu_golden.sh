#!/usr/bin/env bash
# Regenerates golden/menu_<s>.txt — the Step-4½d G1 menu-widget gate (design §6.1):
# oracle_dump_menu runs the REAL C++ Menu / behaviors / SettingsMenu over every committed
# golden/menu_<s>_script.txt (settings scripts read their _setup.cfg with the REAL
# Settings::FromToml). The scripts are hand-written and never touched here. Needs the full C++
# build (links the `game` target), so this is a LOCAL/MANUAL step — NOT run in the lightweight
# rust.yml CI. Override PRESET for other platforms (e.g. linux-x64). cd's to ROOT, so any cwd
# works. MENU_PPM_DIR=<dir> also writes every draw as <dir>/<script>/menu_NNNN.ppm (eyeballing
# only; never committed).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cd "$ROOT"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON -DCMAKE_EXPORT_COMPILE_COMMANDS=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_menu
n=0
for scn in rust/oracle-tests/golden/menu_*_script.txt; do
  c="$(basename "$scn" _script.txt)"
  out="rust/oracle-tests/golden/${c}.txt"
  ppm=()
  if [ -n "${MENU_PPM_DIR:-}" ]; then
    mkdir -p "$MENU_PPM_DIR/$c"
    ppm=(--ppm-dir "$MENU_PPM_DIR/$c")
  fi
  "build/$PRESET/Release/oracle_dump_menu" "$scn" "$out" "${ppm[@]}"
  ops=$(awk '!/^#/ && NF && $1 !~ /^(menu|settings|item|space|bind)$/' "$scn" | wc -l)
  # C++-SIDE GATE: one 14-field line per op, in order; a 16-hex hash exactly on draws; a push
  # only on enter.
  awk -v c="$c" -v ops="$ops" '
    BEGIN { k = 0 }
    /^#/ { next }
    {
      if (NF != 14) { printf "FAIL %s: %d fields: %s\n", c, NF, $0; exit 1 }
      if ($1 != k) { printf "FAIL %s: op %s out of order (want %d)\n", c, $1, k; exit 1 }
      if (($2 == "draw") != ($14 ~ /^[0-9a-f]{16}$/)) { printf "FAIL %s: hash vs op: %s\n", c, $0; exit 1 }
      if ($13 != "0" && $2 != "enter") { printf "FAIL %s: a push on %s\n", c, $2; exit 1 }
      k++
    }
    END {
      if (k != ops) { printf "FAIL %s: %d lines for %d ops\n", c, k, ops; exit 1 }
      printf "  gate %s: %d ops\n", c, k
    }
  ' "$out"
  echo "wrote $out"
  n=$((n + 1))
done
test "$n" -eq 15 || { echo "FAIL: $n menu scripts (want 15)"; exit 1; }
