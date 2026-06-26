#!/usr/bin/env bash
# Builds the C++ dumper and generates the golden vectors. Run from anywhere.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="$ROOT/rust/oracle-tests/golden"
mkdir -p "$OUT"
BIN="$(mktemp -d)/oracle_dump"
clang++ -std=c++20 -O2 -I "$ROOT/src/game" \
  "$ROOT/src/game/math.cpp" \
  "$ROOT/src/tools/oracle_dump/main.cpp" \
  -o "$BIN"
"$BIN" "$OUT"
echo "golden written to $OUT"
