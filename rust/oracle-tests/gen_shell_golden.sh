#!/usr/bin/env bash
# Regenerates golden/shell_<case>.txt — the Step-4½d G2 shell gate and the MILESTONE (design
# §6.2-§6.6): oracle_dump_shell runs the REAL C++ Gfx::RunOneFrame headlessly over every committed
# golden/shell_<case>_script.txt (+ its _setup.cfg, read by the REAL Settings::FromToml). The
# scripts are written by `cargo run -p oracle-tests --example gen_slice4_5d -- write <golden dir>`
# and never touched here. Needs the full C++ build (links the `game` target), so this is a
# LOCAL/MANUAL step — NOT run in the lightweight rust.yml CI. Override PRESET for other platforms
# (e.g. linux-x64). cd's to ROOT, so any cwd works. SHELL_PPM_DIR=<dir> also writes every
# presented frame as <dir>/<case>/f_NNNN.ppm (eyeballing only; never committed).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cd "$ROOT"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON -DCMAKE_EXPORT_COMPILE_COMMANDS=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_shell
n=0
for scn in rust/oracle-tests/golden/shell_*_script.txt; do
  c="$(basename "$scn" _script.txt)"
  out="rust/oracle-tests/golden/${c}.txt"
  ppm=()
  if [ -n "${SHELL_PPM_DIR:-}" ]; then
    mkdir -p "$SHELL_PPM_DIR/$c"
    ppm=(--ppm-dir "$SHELL_PPM_DIR/$c")
  fi
  "build/$PRESET/Release/oracle_dump_shell" "$scn" "$out" "${ppm[@]}"
  expect=$(awk '$1 == "expect" { print $2 }' "$scn")
  # C++-SIDE GATE (design §6.4): one 8-field boot line presenting once; 11-field f lines in frame
  # order with presents in {0,1}, '-' exactly when nothing was presented, upd in {M,W,G}, top in
  # {M,G,-}; one end line agreeing with `expect`.
  awk -v c="$c" -v expect="$expect" '
    BEGIN { k = 0; boots = 0; ends = 0 }
    /^#/ { next }
    $1 == "boot" {
      if (NF != 8 || $2 != "1" || boots++ || k) { printf "FAIL %s: bad boot line\n", c; exit 1 }
      next
    }
    $1 == "f" {
      if (NF != 11) { printf "FAIL %s: f line with %d fields\n", c, NF; exit 1 }
      if ($2 != k) { printf "FAIL %s: frame %s out of order (want %d)\n", c, $2, k; exit 1 }
      if ($3 !~ /^[MWG]$/ || $9 !~ /^[MG-]$/) { printf "FAIL %s: bad upd/top on frame %s\n", c, $2; exit 1 }
      if ($4 != "0" && $4 != "1") { printf "FAIL %s: presents %s\n", c, $4; exit 1 }
      if (($4 == "0") != ($5 == "-")) { printf "FAIL %s: presented vs presents on frame %s\n", c, $2; exit 1 }
      k++; next
    }
    $1 == "end" {
      if (ends++ || $3 != expect) { printf "FAIL %s: end %s, expected %s\n", c, $3, expect; exit 1 }
      next
    }
    { printf "FAIL %s: unexpected line: %s\n", c, $0; exit 1 }
    END {
      if (!boots || !ends) { printf "FAIL %s: missing boot or end\n", c; exit 1 }
      printf "  gate %s: boot + %d frames, end %s\n", c, k, expect
    }
  ' "$out"
  echo "wrote $out"
  n=$((n + 1))
done
test "$n" -eq 11 || { echo "FAIL: $n shell scripts (want 11)"; exit 1; }
