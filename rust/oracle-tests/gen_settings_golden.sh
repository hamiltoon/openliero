#!/usr/bin/env bash
# Regenerates rust/oracle-tests/golden/settings/{*.cfg,*.toml,hashes.txt} - the C++ side of
# the Step-4½ slice-4½a-2 settings byte gate (design §3.4, §9.3.1). oracle_dump_settings runs
# the REAL C++ Settings::load/save, WormSettings::LoadProfile/SaveProfile and UpdateHash over
# every entry of golden/settings/corpus.txt (src/tools/oracle_dump/settings_dump.cpp). The
# hand-written inputs (corpus.txt, in/) are never touched. Needs the full C++ build (links the
# `game` target), so this is a LOCAL/MANUAL step - NOT run in the lightweight rust.yml CI.
# Override PRESET for other platforms (e.g. linux-x64). cd's to ROOT, so any cwd works.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
OUT="rust/oracle-tests/golden/settings"
cd "$ROOT"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON -DCMAKE_EXPORT_COMPILE_COMMANDS=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_settings
# Outputs only (in/ is a subdirectory and is not matched): every run rewrites the full set,
# so an entry dropped from corpus.txt leaves no stale golden behind.
rm -f "$OUT"/*.cfg "$OUT"/*.toml "$OUT/hashes.txt"
# Run from ROOT: corpus paths are repo-root-relative.
"build/$PRESET/Release/oracle_dump_settings" "$OUT/corpus.txt" "$OUT"
echo "wrote $OUT"
