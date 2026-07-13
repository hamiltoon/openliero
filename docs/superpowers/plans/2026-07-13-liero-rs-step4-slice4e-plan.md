# Step 4, Slice 4e — `.lrp` byte-faithful reader (Phase 1): Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax. **Test-first**: write the failing test (or its assertion) before the
> implementation it pins, run it and SEE it fail (RED), then make it pass (GREEN).

**Goal (Phase 1 only):** read a real `.lrp` *byte-faithfully* through the container (`LRPF` magic +
version byte + whole-stream deflate) and the per-worm **XOR-delta input stream**, verifying the
embedded **`WideRollbackChecksum`** every 1050 frames against a Rust sim driven from a
**scenario-reconstructed** initial state, over a **C++-generated corpus**. The cereal `Game` graph is
**skipped whole** via its length prefix (Phase 2 — cereal parse + `framehash` render-diff — is a
deferred follow-on). Companion spec:
`specs/2026-07-13-liero-rs-step4-slice4e-lrp-reader-design.md` (cited **spec §N**).

**Milestone (T3):** the **Phase-1 gate is green** — the C++-generated corpus plays back **bit-exact**:
Rust `WideRollbackChecksum` == every embedded word, and the Rust `HashGameState` series == the
scenario's committed `_sim.txt` golden (cross-isolation), non-vacuously (≥1 checksum consumed; a
corrupted word fails the gate).

**Architecture (spec §0, §5):** a new Bevy-free **`rust/replay`** crate owns the container + delta
stream reader (foreign format, read-only — overview §Open Q6). The **`WideRollbackChecksum` port lives
in `sim`** (`sim/src/wide_checksum.rs`, parallel to `hash.rs`). The corpus comes from a new **headless
C++ tool** (`lrp_gen`) reusing the dumper's exact scenario→`Game` setup + `ReplayWriter`. Deflate dep:
**`flate2`** (already in `Cargo.lock` via `png`; pure-Rust `miniz_oxide` backend, wasm-safe).

## Global constraints

*(inherit every Step 2/3/4a/4b/4c/4d constraint; the 4e-specific ones follow)*

- **Phase 1 does NOT parse cereal.** The initial `Game` blob and every mid-stream
  `Settings`/`WormSettings` tag (0x81/0x82) are **skipped** via their `[uint32 len]` prefix
  (`replay.cpp:60-71,271-289`). No `cereal` / `serde` graph work in this slice. Parsing them is
  Phase 2 (deferred).
- **`WideRollbackChecksum` is a SECOND, distinct hash — never conflate with `hash_game_state`.** It
  folds a larger field set in a different order via `Mix32` (`replay.cpp:168-258`). Port it line-for-
  line (spec §2). `worm.flags` folds a constant `0` (GameOfTag flag-count; always 0 in the KillEmAll
  corpus); `prev_control_states` is **reader-tracked** (Rust `WormState` has no such field).
- **Bevy stays confined to `game`.** `replay`, `sim`, `scenario`, `assets`, `sim-core` remain
  Bevy-free (`cargo tree -p replay` shows no `bevy*`). `flate2` is added only to `replay`.
- **The corpus generator reuses the dumper's EXACT tick-0 setup** (`sim_physics_dump.cpp:396-460`) so
  the `.lrp`'s initial `Game` is bit-identical to Rust's `SimState::new`+`loader::load` — the checksum
  desyncs at frame 1050 otherwise (spec §6 risk 2). Headless traps: **`NullSoundPlayer`** +
  **base `StatsRecorder`** (`make_shared<StatsRecorder>()`, NOT `NormalStatsRecorder` — the 5b crash
  trap); no viewports/renderer/SDL needed (the checksum folds no viewport state).
- **No golden regressions.** New fixtures live under `golden/` (the `.lrp` files + their `_sim.txt`
  cross-check). `git diff --stat golden/` shows only **new** 4e files; no existing
  `sim_slice*`/`render_slice*` golden moves. The `lrp_gen` tool is a **new** `add_executable`; the
  existing dumper is **untouched**.
- **C++ side mirrors the repo style** (clang-format-22 + clang-tidy clean; run
  `scripts/clang-format-diff.sh` + full-file `--dry-run -Werror` after deletes; one `add_executable`
  block per tool — CLAUDE.md). The shared scenario grammar must parse on both sides.
- **No AI / "Generated with" taglines; no commit trailers** (no `Co-Authored-By`, no
  `Claude-Session`). **Commit on branch `liero-rs-step-4`** (the accumulating Step-4 PR). **Do NOT
  push and do NOT open a PR** — the controller owns push + PR. **fmt only new/edited files** (no
  blanket-fmt). **No sub-subagents.** **Bash discipline:** one command per call; no
  `>>`/heredoc/`&&`/`;`/`$VAR`; no `cd`+`git`; create files with the editor.

## Model tiers

- **[Opus]:** every task touching the checksum port, the byte-stream framing/XOR decode, the
  tick-0-alignment C++ generator, the Phase-1 gate/milestone, and the final review (the hard ports +
  all gates).
- **[Sonnet]:** mechanical sub-steps only — the `Cargo.toml`/`CMakeLists.txt` wiring, the `gen_*.sh`
  script, the crate skeleton. None of the ordering- or checksum-sensitive tasks. When in doubt, Opus.

## Tasks

### T0 [Opus] — C++ corpus generator `lrp_gen` + committed corpus (the oracle)

- [ ] Add `src/tools/oracle_dump/lrp_gen.cpp` + its `add_executable`/`target_link_libraries` block
      (`CMakeLists.txt:374-389` pattern, `PRIVATE game`). Reuse `sim_physics_dump.cpp`'s `ParseScenario`
      + tick-0 `Game` setup **verbatim** (worm construction, `resolve_weapons`, blood-pool sizing that
      mirrors `StartGame`, `game_mode`/`max_bonuses`) — the initial `Game` must match Rust's
      `loader::load` bit-for-bit (spec §6 risk 2). Install **`NullSoundPlayer`** +
      **base `StatsRecorder`** (`sim_physics_dump.cpp:389-396` — NOT `NormalStatsRecorder`). No
      viewports/renderer.
- [ ] Drive: `ReplayWriter` over `DeflateWriter`→`FileWriter`; `BeginRecord(game)`; per tick set each
      worm's `control_states` from the scenario input, then `RecordFrame()` **then** `game.ProcessFrame()`
      (the `LocalController` order — input-map §1b/§4). Destructor writes `0x83`.
- [ ] Author a `gen_lrp_*.sh` (Sonnet-ok) that emits a **> 1050-tick** `.lrp` over an existing scenario
      (so ≥1 `WideRollbackChecksum` word is embedded) + a couple of short fixtures (empty-frame / end /
      2-worm order). Commit the `.lrp` under `golden/`. **Sanity:** play each back through the C++
      `framehash`/`ReplayReader` (`CMakeLists.txt:552`) and confirm it is a valid, non-desyncing replay
      before committing.
- [ ] **Re-diff gate:** the new tool must not perturb any existing golden — `git diff --stat golden/`
      shows only **new** `.lrp` (+ script) files.

### T1 [Opus] — `WideRollbackChecksum` port in `sim` (hand-folded)

- [ ] **RED:** `sim/src/wide_checksum.rs` unit test — hand-fold `Mix32`/`MixBytes` + the full field
      order (spec §2, `replay.cpp:181-257`) over a fixture `SimState` (mirror the `hash.rs` by-hand
      tests): seed=`rand.last`, `cycles`, per-worm 21 fields + per-weapon triples, the four pools
      (count + per-object fields), then the material buffer. Pin `worm.flags`→constant `0` and prove
      `prev_control_states` is a **caller-supplied** baseline (the fn takes per-worm prev istate, or a
      `&[u32]`, since `WormState` lacks the field). SEE it fail (no impl yet).
- [ ] **GREEN:** implement `wide_rollback_checksum(state, prev_istates: &[u32]) -> u32` with
      `wrapping_*` throughout and `as u32` reinterpret casts. Export from `sim`.
- [ ] **Non-vacuity:** a decoy test — mutating a folded field (e.g. `wobject.owner_idx`, which
      `hash_game_state` ignores) **changes** `wide_rollback_checksum` (proving it folds the wider set),
      while an unfolded field leaves it unchanged.

### T2 [Opus] — `rust/replay` crate: container + delta-stream reader

- [ ] Scaffold `rust/replay` (Bevy-free; deps `sim`, `flate2`; add to workspace — Sonnet-ok).
- [ ] **RED (deflate framing — the empirical pin, spec §4):** a test inflates a **committed T0 `.lrp`**
      and asserts a non-empty round-trip (magic `LRPF` visible at offset 0 of the inflated bytes). Try
      `flate2::read::ZlibDecoder`; if it fails, fall back to `DeflateDecoder`. SEE the reader absent
      first.
- [ ] **RED (framing + XOR decode):** a test decodes a short committed fixture into
      `Vec<[ControlState; N]>` + a `cycle -> u32` checksum map, asserting: magic/version validated
      (reject `> kMyReplayVersion`, `replay.cpp:121`); the cereal `Game` blob **skipped** via its
      `uint32` length; `0x80` empty-frame repeats the last input; `< 0x80` frames decode per worm as
      `Unpack(byte ^ prev.pack())` in `game.worms` order with the reader maintaining
      `prev = control_states` after each frame (`replay.cpp:293-307`); `0x81`/`0x82` settings tags
      **skipped** via `uint32` length (+ the `worm_idx` for 0x82); `0x83` ends; the `WideRollbackChecksum`
      word is captured at the `cycles % 1050 == 0` tail. Values checked against the known scenario
      inputs (hand-derived).
- [ ] **GREEN:** implement the byte-cursor reader (inflate-to-memory mirroring `replay.cpp:99-110`;
      big-endian `LRPF`; `uint32` little-vs-big per `io::ReadUint32` — **match C++'s `io::coding`
      endianness**, verify against the fixture). Return the decoded input stream + checksum map + tick
      count.
- [ ] **Edge unit tests (hand-crafted bytes):** version-too-recent rejected; a mid-stream `0x81`/`0x82`
      tag skipped without disturbing the following input frame; an unexpected header byte errors
      (`replay.cpp:309`).

### T3 [Opus] — Phase-1 gate over the corpus (THE MILESTONE)

- [ ] **RED→GREEN:** `rust/oracle-tests/tests/replay_lrp_phase1.rs` — reconstruct tick-0 `SimState` via
      `scenario::loader::load` from the source scenario; decode the committed `.lrp` (T2); replay each
      tick (apply decoded `[ControlState; N]`, `process_frame`), and at every `cycles % 1050 == 0`
      boundary assert `wide_rollback_checksum(&state, &prev_istates)` **==** the embedded word (folded
      at the same loop point C++ uses — spec §1). SEE at least one boundary assertion drive the test.
- [ ] **Cross-isolation:** assert the Rust `hash_game_state` series **==** the scenario's committed
      `_sim.txt` golden (any input-decode drift surfaces at the **first** differing tick, not at 1050);
      assert the decoded tick count matches; stream ends on `0x83`.
- [ ] **Non-vacuity + negative:** assert ≥1 checksum word was consumed (corpus > 1050); a negative test
      that corrupts one embedded word (or perturbs one folded field) makes the gate **fail**.
- [ ] **This is the milestone:** a real `.lrp` container + delta stream + `WideRollbackChecksum` reads
      bit-exact. Note in the done-report that this needs **no C++ at test time** (self-checking, like 4b).

### T4 [Opus] — legacy/version hardening + Phase-2 boundary bookkeeping

- [ ] Hand-crafted byte-fixture unit tests for the paths the generated corpus can't reach: a
      `version < 7` header is **accepted** by the container reader (Phase 1 skips the legacy palette /
      `ExpandLegacyWormRgb` expansions — they are palette-only, not folded by the checksum, spec §0);
      the settings-tag skip is version-agnostic. Confirm `version > kMyReplayVersion` still rejects.
- [ ] Confirm `replay` stays wasm-safe (`flate2`/`miniz_oxide` pure-Rust; no `bevy*` in `cargo tree
      -p replay`).
- [ ] Document the **Phase-2** follow-on concretely in the spec/PROGRESS: cereal `Game` graph
      deserialization (`serialization/cereal_types.hpp`, gated by `g_cereal_replay_version`) →
      full-state initial reconstruction → per-frame render `framehash` diff vs C++ (`CMakeLists.txt:552`)
      over arbitrary real `.lrp`; the version-legacy palette/worm-rgb expansions ride with it.

### T5 [Opus] — Broad final review + PROGRESS + deferral bookkeeping

- [ ] Full-suite green: `cargo test --workspace --exclude game` + `cargo test -p game` (the CI
      commands) + the new `replay` + `oracle-tests` cases; `git diff --stat golden/` shows only **new**
      4e `.lrp`/`_sim.txt` files; no `sim_slice*`/`render_slice*` golden moved; the C++ dumper untouched.
- [ ] Broad review pass (0 Critical / 0 Important bar): the `WideRollbackChecksum` field order +
      `flags=0`/`prev` handling (spec §2), the cereal-skip correctness (length-prefix, never parsed),
      the tick-0 alignment (generator reuses dumper setup), the deflate-framing pin, the gate
      non-vacuity, `replay` Bevy-free/wasm-safe.
- [ ] Update `docs/superpowers/liero-rs-PROGRESS.md`: 4e Phase-1 MILESTONE GREEN + the **Phase-2
      deferral** (cereal `Game` parse + `framehash` render-diff; possibly its own slice / post-Step-4
      item) and the legacy-version note. Use the real currentDate.
- [ ] Commit on `liero-rs-step-4`. No push, no PR (controller owns that).

## Task dependency / milestone map

```
T0 (C++ lrp_gen + corpus) ──┐
                            ├─> T2 (replay reader) ─┐
T1 (WideRollbackChecksum) ──┴───────────────────────┼─> T3 (PHASE-1 GATE = MILESTONE) ─> T4 (legacy) ─> T5 (review)
                                                    │
                            (T1 also feeds T3 directly)
```

T0 and T1 are independent and parallelizable; **T2 needs T0's corpus** (for the deflate/framing pins);
**T3 (milestone) needs T1 + T2**; T4/T5 build on T3. Phase 2 (cereal `Game` + `framehash`) is **out of
scope** — booked as a deferred follow-on in T4/T5.
