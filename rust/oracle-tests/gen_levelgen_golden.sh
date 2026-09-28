#!/usr/bin/env bash
# Regenerates golden/levelgen.txt by running the REAL C++ level generator
# (Level::GenerateRandom / MakeShadow / GenerateFromSettings, plus a self-checked stage
# replica and a CorrectShadow dig stage — see src/tools/oracle_dump/levelgen_dump.cpp).
# Needs the full C++ build (links the `game` target), so this is a LOCAL/MANUAL step — it
# is NOT run in the lightweight rust.yml CI. Override PRESET for other platforms (e.g.
# linux-x64). Unlike the older gen_*.sh it cd's to ROOT first, so it works from any cwd.
# CMAKE_EXPORT_COMPILE_COMMANDS gives clang-tidy a compile database for the dumper.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cd "$ROOT"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON -DCMAKE_EXPORT_COMPILE_COMMANDS=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_levelgen
# Run from ROOT so Common::load("data/TC/openliero") and the file cases resolve.
"build/$PRESET/Release/oracle_dump_levelgen" "rust/oracle-tests/golden/levelgen.txt"
echo "wrote rust/oracle-tests/golden/levelgen.txt"
