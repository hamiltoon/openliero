#!/usr/bin/env bash
# Regenerates golden/shell_<case>.txt — the Step-4½d G2 shell gate and the MILESTONE (design
# §6.2-§6.6): oracle_dump_shell runs the REAL C++ Gfx::RunOneFrame headlessly over every committed
# golden/shell_<case>_script.txt (+ its _setup.cfg, read by the REAL Settings::FromToml). The
# scripts are written by `cargo run -p oracle-tests --example gen_slice4_5d -- write <golden dir>`
# and never touched here. Needs the full C++ build (links the `game` target), so this is a
# LOCAL/MANUAL step — NOT run in the lightweight rust.yml CI. Override PRESET for other platforms
# (e.g. linux-x64). cd's to ROOT, so any cwd works. SHELL_PPM_DIR=<dir> also writes every
# presented frame as <dir>/<case>/f_NNNN.ppm (eyeballing only; never committed).
# Slice 4½e-1 adds the tops O/I/B and two opt-in line kinds: a script with `detail` gets a `d` line
# after every f line, a script with `fs` ends with `file` lines after the end line (formats:
# docs/superpowers/plans/2026-09-26-liero-rs-step4.5-slice4.5e1-plan.md, §Formats pinned). The
# e-1 scripts come from `--example gen_slice4_5e1_shell -- write <golden dir>`.
# EXPECTED_SHELL_CASES (default 22: the 4½d 11 + the e-1 11, i.e. the plan's 10 and Batch 5's
# key_edges) overrides the expected script count.
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
  detail=$(awk '$1 == "detail" { d = 1 } END { print d + 0 }' "$scn")
  fs=$(awk '$1 == "fs" { f = 1 } END { print f + 0 }' "$scn")
  # C++-SIDE GATE (design §6.4): one 8-field boot line presenting once; 11-field f lines in frame
  # order with presents in {0,1}, '-' exactly when nothing was presented, upd in {M,W,G,O,I,B},
  # top in {M,G,O,I,B,-}; one end line agreeing with `expect`. With `detail`: exactly one 6-field
  # d line right after each f line, same frame, cur in {M,S}, cfg16 16 hex, state8 8 hex or '-'.
  # With `fs`: at least one 3-field file line after the end line, rel strictly sorted bytewise,
  # fnv16 16 hex. With neither, the 4½d rules unchanged (no d or file line at all).
  LC_ALL=C awk -v c="$c" -v expect="$expect" -v detail="$detail" -v fs="$fs" '
    function hex(s, n) { return length(s) == n && s ~ /^[0-9a-f]+$/ }
    BEGIN { k = 0; boots = 0; ends = 0; need_d = 0; files = 0; prev = "" }
    /^#/ { next }
    need_d && $1 != "d" { printf "FAIL %s: no d line after f %d\n", c, k - 1; exit 1 }
    $1 == "boot" {
      if (NF != 8 || $2 != "1" || boots++ || k) { printf "FAIL %s: bad boot line\n", c; exit 1 }
      next
    }
    $1 == "f" {
      if (ends) { printf "FAIL %s: f line after end\n", c; exit 1 }
      if (NF != 11) { printf "FAIL %s: f line with %d fields\n", c, NF; exit 1 }
      if ($2 != k) { printf "FAIL %s: frame %s out of order (want %d)\n", c, $2, k; exit 1 }
      if ($3 !~ /^[MWGOIB]$/ || $9 !~ /^[MGOIB-]$/) { printf "FAIL %s: bad upd/top on frame %s\n", c, $2; exit 1 }
      if ($4 != "0" && $4 != "1") { printf "FAIL %s: presents %s\n", c, $4; exit 1 }
      if (($4 == "0") != ($5 == "-")) { printf "FAIL %s: presented vs presents on frame %s\n", c, $2; exit 1 }
      need_d = detail; k++; next
    }
    $1 == "d" {
      if (!need_d) { printf "FAIL %s: a d line not right after an f line\n", c; exit 1 }
      if (NF != 6 || $2 != k - 1) { printf "FAIL %s: bad d line for frame %d\n", c, k - 1; exit 1 }
      if ($3 !~ /^[MS]$/ || $4 !~ /^[0-9]+$/ || !hex($5, 16) || ($6 != "-" && !hex($6, 8))) {
        printf "FAIL %s: bad d fields on frame %s\n", c, $2; exit 1
      }
      need_d = 0; next
    }
    $1 == "end" {
      if (ends++ || $3 != expect) { printf "FAIL %s: end %s, expected %s\n", c, $3, expect; exit 1 }
      next
    }
    $1 == "file" {
      if (!fs || !ends) { printf "FAIL %s: unexpected file line: %s\n", c, $0; exit 1 }
      if (NF != 3 || !hex($3, 16)) { printf "FAIL %s: bad file line: %s\n", c, $0; exit 1 }
      if (files && !((prev "") < ($2 ""))) { printf "FAIL %s: file lines not sorted at %s\n", c, $2; exit 1 }
      prev = $2; files++; next
    }
    { printf "FAIL %s: unexpected line: %s\n", c, $0; exit 1 }
    END {
      if (!boots || !ends) { printf "FAIL %s: missing boot or end\n", c; exit 1 }
      if (need_d) { printf "FAIL %s: no d line after the last f line\n", c; exit 1 }
      if (fs && !files) { printf "FAIL %s: an fs case with no file line\n", c; exit 1 }
      printf "  gate %s: boot + %d frames%s, end %s%s\n", c, k, detail ? " (+ d lines)" : "", expect,
        fs ? sprintf(", %d file lines", files) : ""
    }
  ' "$out"
  echo "wrote $out"
  n=$((n + 1))
done
want="${EXPECTED_SHELL_CASES:-22}"
test "$n" -eq "$want" || { echo "FAIL: $n shell scripts (want $want)"; exit 1; }
