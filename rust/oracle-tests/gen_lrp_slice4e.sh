#!/usr/bin/env bash
# Regenerates the Slice-4e `.lrp` byte-faithful-reader corpus (the C++ oracle for the Rust
# `replay` container + delta-stream + WideRollbackChecksum reader). Builds the headless
# `oracle_dump_lrp_gen` tool, which reuses `sim_physics_dump`'s EXACT tick-0 `Game` setup
# (so the `.lrp`'s serialised initial `Game` is bit-identical to what Rust's
# `scenario::loader::load` reconstructs) and drives the REAL `Game::ProcessFrame` through a
# `ReplayWriter` (RecordFrame BEFORE ProcessFrame, the LocalController order) to emit a real
# `.lrp`: `LRPF` magic + version + cereal `Game`, the per-worm XOR-delta input stream, and
# the embedded WideRollbackChecksum word every 1050 frames. NullSoundPlayer + a BASE
# StatsRecorder; no viewports/renderer/SDL.
#
# Corpus (committed under golden/):
#   lrp_render_slice4d_live.lrp  — SHORT (320t): firing + blood + death + 2-worm input
#       deltas. Source scenario: render_slice4d_live_scenario.txt. Cross-checked below: its
#       per-tick HashGameState series (lrp_gen 4th-arg sidecar) == the committed
#       render_slice4d_live_sim.txt golden (produced independently by sim_physics_dump's
#       render_live path) — proving tick-0 alignment + trajectory fidelity vs the Rust
#       sim's own oracle. framehash validates the container framing + the cycle-0 checksum
#       + the 0x83 end tag.
#   lrp_slice4e_long.lrp         — LONG (1120t > 1050): worms aim down + fire EXPLOSIVES
#       into distant ground, digging the material buffer (non-trivial checksum fold at the
#       1050 boundary) with no mutual blood. Embeds a WideRollbackChecksum word at cycles 0
#       AND 1050 (non-vacuity). framehash replays it clean END-TO-END, including the
#       cycle-1050 checksum comparison.
#
# LOCAL/MANUAL — needs the full C++ build (links `game`); NOT in the lightweight rust.yml
# CI. Once committed, the Phase-1 Rust gate (oracle-tests) needs no C++ at test time.
# Override PRESET for other platforms.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cd "$ROOT"

cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON -DOPENLIERO_BUILD_TESTS=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_lrp_gen framehash

GEN="$ROOT/build/$PRESET/Release/oracle_dump_lrp_gen"
FRAMEHASH="$ROOT/build/$PRESET/Release/framehash"
GOLD="$ROOT/rust/oracle-tests/golden"
TC="$ROOT/data/TC/openliero"

# --- Short fixture (render4d source) + the HashGameState cross-check sidecar. ---
HASH_TMP="$(mktemp)"
"$GEN" "$GOLD/render_slice4d_live_scenario.txt" "$GOLD/lrp_render_slice4d_live.lrp" 42 "$HASH_TMP"
if ! diff -q "$HASH_TMP" "$GOLD/render_slice4d_live_sim.txt" >/dev/null; then
  echo "FAIL: lrp_gen HashGameState series != render_slice4d_live_sim.txt (tick-0 drift)" >&2
  diff "$HASH_TMP" "$GOLD/render_slice4d_live_sim.txt" | head >&2
  exit 1
fi
rm -f "$HASH_TMP"
echo "cross-check OK: lrp_render_slice4d_live HashGameState == render_slice4d_live_sim.txt"

# --- Long fixture (> 1050 ticks; dug material buffer at the 1050 checksum boundary). ---
"$GEN" "$GOLD/lrp_slice4e_long_scenario.txt" "$GOLD/lrp_slice4e_long.lrp" 42

# --- Sanity: each .lrp plays back through the real C++ ReplayReader/framehash without
#     desync (a valid, non-desyncing replay). The long fixture crosses the 1050 boundary,
#     so its framehash run verifies the embedded WideRollbackChecksum word too. ---
for lrp in lrp_render_slice4d_live lrp_slice4e_long; do
  if ! "$FRAMEHASH" "$TC" "$GOLD/$lrp.lrp" >/dev/null 2>"$GOLD/../.lrp_framehash.err"; then
    echo "FAIL: framehash desynced on $lrp.lrp" >&2
    cat "$GOLD/../.lrp_framehash.err" >&2
    exit 1
  fi
  echo "framehash OK (non-desyncing): $lrp.lrp"
done
rm -f "$GOLD/../.lrp_framehash.err"

echo "wrote lrp_render_slice4d_live.lrp + lrp_slice4e_long.lrp"
