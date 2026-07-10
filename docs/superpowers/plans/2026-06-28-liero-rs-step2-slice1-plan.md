# Step 2, Slice 1 — Level → sim-state + state-hash harness: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking. **Test-first**: write the failing
> test (or its assertion) before the implementation it pins.

**Goal:** Stand up the `sim` crate and prove that the Rust state hash matches the C++
`HashGameState`/`HashGameComponents` oracle at **tick 0** (initial state, before any
`ProcessFrame`), with the sim-world built from a loaded `assets::LevelData` and the
2-worm init fixture. No dynamics.

**Architecture:** New `rust/sim/` crate (deps: `sim-core`, `assets`; Bevy-free,
float-free). `SimState` holds the frame-0 subset of the `fast_snapshot.hpp` inventory
(level material buffer, seeded `Rand`, `cycles`, empty pools, 2 initial worms).
`sim::hash` reproduces `stateHash.hpp` with `wrapping_*` + `as u32`. Correctness is
proven by a new C++ dumper (`oracle_dump_sim`, links `game`) that reaches the same
tick-0 state and emits both hashes, and a golden differential test.

**Tech stack:** Rust (`sim` new, `oracle-tests`, `sim-core` accessor), C++ oracle
dumper (`sim_dump.cpp`, links `game`), CMake option `OPENLIERO_BUILD_ORACLE_DUMP`,
the engine's `uint32_t` hashes (emitted as hex).

## Global constraints

- **Bit-exact hash vs C++.** Source of truth: `src/game/stateHash.hpp`
  (`HashGameState` lines 15–113, `HashGameComponents` 129–213). Accumulation:
  `h = h*31 + field` (master + most components) and `h = h*33 ^ byte` (level). Rust
  uses `wrapping_mul`/`wrapping_add` and `(field as i32) as u32` reinterpret casts.
  Empty component hashes seed at `1`.
- **Tick-0 fixture = `test_determinism.cpp:22–92` minus generation.** Seed; **load a
  fixed `.lev`** (NOT `GenerateFromSettings`); 2 worms (`health=settings.health`,
  `index`, `stats_x∈{0,218}`); `InitWeapons`; `ResetWorms`
  (`killed_timer=150`, `visible=false`, `lives=settings.lives`, `kills=0`,
  `current_weapon=0`). Dump **before** any `ProcessFrame`. No viewports needed.
- **`rand.last == 0` and `cycles == 0` at tick 0** (no RNG consumed — level loaded,
  not generated).
- **No Bevy, no float in `sim`.** Worm pos/vel use `sim-core` `Vec2`/`Fixed`.
- **Modernise, don't transliterate.** Idiomatic Rust structs/methods; the oracle
  proves behaviour. C++ dumper matches `level_dump.cpp`'s Google/100-col style.
- **Golden regen is LOCAL/MANUAL** (full C++ build links `game`); CI (`rust.yml`)
  runs `cargo test --workspace` against the committed golden. `PRESET` defaults to
  `macos-arm64`.
- **No AI/"Generated with" taglines** in commits or files.
- **Bash discipline:** no `>>`, heredoc, `&&`, `;`, `$VAR` chaining in commands that
  hit the permission prompt — one command per call; create files with the editor.

## File structure

- `rust/Cargo.toml` — MODIFY: add `"sim"` to workspace `members`.
- `rust/sim/Cargo.toml` — NEW: deps `sim-core` (path), `assets` (path).
- `rust/sim/src/lib.rs` — NEW: `pub mod state; pub mod pool; pub mod hash;` + re-exports.
- `rust/sim/src/state.rs` — NEW: `SimState`, `LevelSim`, `WormState`, `WormWeapon`,
  `ControlState`, `Ninjarope`, builder from `assets::LevelData` + worm-init.
- `rust/sim/src/pool.rs` — NEW: `Pool<T>` (+ blood flavour) with C++-`All()` order.
- `rust/sim/src/hash.rs` — NEW: `hash_game_state`, `hash_components`, `ComponentHashes`.
- `rust/sim-core/src/rng.rs` — MODIFY: add `pub fn last(&self) -> u32`.
- `src/tools/oracle_dump/sim_dump.cpp` — NEW: tick-0 dumper, links `game`.
- `CMakeLists.txt` — MODIFY: add `oracle_dump_sim` inside the
  `OPENLIERO_BUILD_ORACLE_DUMP` block (after `oracle_dump_wav`, lines 384–385).
- `rust/oracle-tests/gen_sim_golden.sh` — NEW: regenerate the sim golden.
- `rust/oracle-tests/golden/sim_slice1.txt` — NEW: committed golden.
- `rust/oracle-tests/tests/sim_slice1_golden.rs` — NEW: tick-0 differential test.
- `rust/oracle-tests/Cargo.toml` — MODIFY: add `sim = { path = "../sim" }` to
  `[dev-dependencies]`.

---

### Task 0: Create the `sim` crate skeleton and wire the workspace

De-risk the crate graph before any logic: the new crate must compile and be visible
to the workspace and to `oracle-tests`.

**Files:** `rust/Cargo.toml`, `rust/sim/Cargo.toml`, `rust/sim/src/lib.rs`,
`rust/oracle-tests/Cargo.toml`.

- [ ] **Step 1:** Add `"sim"` to the `members` array in `rust/Cargo.toml`.
- [ ] **Step 2:** Create `rust/sim/Cargo.toml`: package `sim`, edition 2021, deps
      `sim-core = { path = "../sim-core" }` and `assets = { path = "../assets" }`.
- [ ] **Step 3:** Create `rust/sim/src/lib.rs` with a crate doc comment (no Bevy, no
      float; the deterministic sim, hash lives here) and `pub mod state; pub mod
      pool; pub mod hash;` — with empty stub modules so it builds.
- [ ] **Step 4:** Add `sim = { path = "../sim" }` to `[dev-dependencies]` in
      `rust/oracle-tests/Cargo.toml`.
- [ ] **Verify:** `cargo build -p sim` and `cargo test --workspace --no-run` succeed.

---

### Task 1: `sim-core::Rand::last()` accessor (test-first)

The hash needs `rand.last`, currently a private field (`rng.rs:34`).

**Files:** `rust/sim-core/src/rng.rs`.

- [ ] **Step 1 (test):** In `rng.rs` tests, add a test: a freshly `seed`ed `Rand`
      reports `last() == 0`; after one `next_u32()`, `last()` equals that returned
      value (mirrors C++ `Rand::last`).
- [ ] **Step 2 (impl):** Add `pub fn last(&self) -> u32 { self.last }`.
- [ ] **Verify:** `cargo test -p sim-core` green.

---

### Task 2: `Pool<T>` with C++-`All()` iteration order (test-first)

The pools are empty this slice, but their contract is fixed now.

**Files:** `rust/sim/src/pool.rs`, `rust/sim/src/lib.rs`.

- [ ] **Step 1 (test):** Write unit tests for `Pool<T>`:
      - new pool with capacity N is empty; `iter()` yields nothing.
      - after `spawn(x)` ×k, `iter()` yields the k items in **slot (insertion/slot)
        order**; `len()` == k.
      - `free` of a middle slot removes it; subsequent `iter()` skips it and order of
        the survivors is preserved (free-list reuse on next `spawn`, matching C++
        `FixedObjectList`).
      - a blood-flavour pool supports free-during-iteration semantics (model the
        `BObjectList` Begin/End/Free contract — a `retain`-style pass is acceptable
        as long as order is slot-order).
- [ ] **Step 2 (impl):** Implement `Pool<T>` (fixed `Vec<Option<T>>` or value+live
      flag + free-list) and the blood flavour. `iter()` visits live slots in slot
      order. Keep it minimal — only what the tests pin.
- [ ] **Verify:** `cargo test -p sim` green.

---

### Task 3: `SimState` datamodel + builder from `LevelData` (test-first)

**Files:** `rust/sim/src/state.rs`, `rust/sim/src/lib.rs`.

- [ ] **Step 1 (test):** Write tests that build a `SimState` from a tiny synthetic
      `assets::level::LevelData` (e.g. 4×4, known `material_id`) plus a worm-init
      list of 2 worms (health, lives, index, stats_x). Assert: `cycles == 0`,
      `rand.last() == 0` after seeding, `level.material_id` equals the input,
      `worms.len() == 2`, each worm `pos==(0,0)`, `vel==(0,0)`, `aiming_angle==0`,
      `visible==false`, `killed_timer==150`, `control_states.pack()==0`, weapons
      initialised (type set, `delay_left==0`, `loading_left==0`), `ninjarope.out==
      false`. All pools empty.
- [ ] **Step 2 (impl):** Define `SimState`, `LevelSim`, `WormState`, `WormWeapon`,
      `ControlState` (u32, `pack()`/`unpack()` masking to 7 bits, per `worm.hpp`),
      `Ninjarope`, and a builder
      `SimState::new(level: &LevelData, worms_init: &[WormInit], seed: u32, objects: &Objects?) -> SimState`.
      The builder sets `cycles=0`, seeds `rand`, copies `material_id`+dims, and
      constructs each `WormState` to the tick-0 values. Weapon init reproduces
      `InitWeapons`: resolve `ww.ty`/`ammo` for the selectable weapons (see open
      question 2 in the spec — use `Objects`/order if needed, else identity if the
      fixture makes it so). Empty pools via Task 2.
- [ ] **Verify:** `cargo test -p sim` green.

---

### Task 4: `sim::hash` mirroring `stateHash.hpp` (test-first)

**Files:** `rust/sim/src/hash.rs`, `rust/sim/src/lib.rs`.

- [ ] **Step 1 (test):** Write unit tests that pin the arithmetic against
      hand-computed reference values for a **trivial** state (so the test is
      independent of the C++ build):
      - a `SimState` with a 1-byte level, 0 worms, empty pools → compute the expected
        `h` by hand from the documented accumulation (`h=1`, `+rand.last`, `+cycles`,
        `h*33 ^ byte`) and assert `hash_game_state` equals it.
      - `ComponentHashes` for empty pools all equal the empty seed (`1`), `rng ==
        rand.last`, `level` equals the by-hand level hash.
      - Add a worm with chosen field values and assert the worm contribution matches
        a by-hand fold (covers field order + `as u32` casts + the per-weapon
        `if type` push and `ninjarope` tail).
- [ ] **Step 2 (impl):** Implement `hash_game_state(&SimState) -> u32` and
      `hash_components(&SimState) -> ComponentHashes` in the exact order from the
      spec, using `wrapping_mul`/`wrapping_add` and `(x as i32) as u32`. Push the
      weapon `ty.id` only when `ty.is_some()`. Bool fields as `0/1`.
- [ ] **Verify:** `cargo test -p sim` green. Cross-check the field order against
      `stateHash.hpp` line by line in the PR description.

---

### Task 5: C++ oracle dumper `oracle_dump_sim` + CMake target

**Files:** `src/tools/oracle_dump/sim_dump.cpp`, `CMakeLists.txt`.

- [ ] **Step 1:** Create `src/tools/oracle_dump/sim_dump.cpp` (style per
      `level_dump.cpp`). In `main`: `PrecomputeTables()`; load `Common` from
      `data/TC/openliero`; build `Settings` (`game_mode=kGmKillEmAll`, `lives`,
      `loading_time=0`); `Game game(common, settings, NullSoundPlayer)`;
      `game.rand.Seed(seed)`. **Load a fixed `.lev`** into `game.level` via
      `Level::load` (choose the file per spec open-question 1; reuse what
      `level_dump.cpp` opens if convenient) — do **not** call `GenerateFromSettings`.
      Add 2 worms exactly as the fixture (`settings`, `health`, `index`, `stats_x`);
      `InitWeapons` each; `ResetWorms`. Do **not** add viewports and do **not** call
      `ProcessFrame`.
- [ ] **Step 2:** Emit one line to the output file:
      `<seed> <width> <height> <HashGameState> <c.rng> <c.level> <c.worms[0]>
      <c.worms[1]> <c.bobjects> <c.bonuses> <c.sobjects> <c.nobjects> <c.wobjects>`,
      each hash as `%08x` (from `HashGameComponents`). Accept `<level_path>
      <out_path>` (and optionally `<seed>`) as argv, like the other dumpers.
- [ ] **Step 3:** In `CMakeLists.txt`, inside the `OPENLIERO_BUILD_ORACLE_DUMP`
      block (after the `oracle_dump_wav` block at 384–385), add:
      `add_executable(oracle_dump_sim src/tools/oracle_dump/sim_dump.cpp)` and
      `target_link_libraries(oracle_dump_sim PRIVATE game)`.
- [ ] **Verify (local/manual):** configure with `-DOPENLIERO_BUILD_ORACLE_DUMP=ON`
      and build `oracle_dump_sim` (one CMake command per Bash call). It links and
      runs, writing a single well-formed line.

---

### Task 6: `gen_sim_golden.sh` + committed golden

**Files:** `rust/oracle-tests/gen_sim_golden.sh`, `rust/oracle-tests/golden/sim_slice1.txt`.

- [ ] **Step 1:** Create `gen_sim_golden.sh` following `gen_level_golden.sh`: `set
      -euo pipefail`; `PRESET=${PRESET:-macos-arm64}`; configure with
      `-DOPENLIERO_BUILD_ORACLE_DUMP=ON`; build `--target oracle_dump_sim`; run it
      with the chosen level path + `golden/sim_slice1.txt` (+ seed). Mark it
      LOCAL/MANUAL (not run in `rust.yml`). `chmod +x`.
- [ ] **Step 2:** Run it to produce `golden/sim_slice1.txt`; commit the file.
- [ ] **Verify:** the golden contains the expected single (or N) tick-0 record(s)
      with 13 hex/decimal columns.

---

### Task 7: Rust differential test `sim_slice1_golden` (test-first against the golden)

**Files:** `rust/oracle-tests/tests/sim_slice1_golden.rs`.

- [ ] **Step 1:** Write the test: read `golden/sim_slice1.txt`; load the **same**
      `.lev` from `data/` via `assets::level::load` (path relative to
      `CARGO_MANIFEST_DIR/../../data/...`, as `level_golden.rs` does); build the same
      `SimState` (same seed, same 2-worm init, same `Objects` if used); compute
      `hash_game_state` + `hash_components`; assert every column equals the golden.
- [ ] **Step 2:** Make worm-init + weapon resolution in the test match the C++
      fixture exactly (the spec's open questions 2 and the chosen `Settings`). If the
      Rust `SimState::new` needs `Objects`/`TcConfig` to resolve weapon ids, load
      them from `data/` in the test the same way.
- [ ] **Verify:** `cargo test -p oracle-tests sim_slice1` green; then
      `cargo test --workspace` green.

---

### Task 8: Wire-up review and done-check

- [ ] **Step 1:** Confirm `cargo test --workspace` is green and `sim` has no Bevy /
      no float / no deps beyond `sim-core` + `assets`.
- [ ] **Step 2:** Re-read `stateHash.hpp` against `hash.rs` once more; the field
      order and the `if type`/ninjarope tail must match exactly.
- [ ] **Step 3:** Confirm the dumper never calls `ProcessFrame` and never
      `GenerateFromSettings` (tick-0, `rand.last==0`, `cycles==0`).
- [ ] **Step 4:** Update the Step 2 overview's *Next artifact* / slice-1 status if
      appropriate (docs only). Do **not** commit unrelated changes.
- [ ] **Definition of done:** every checkbox in the slice-1 spec's *Definition of
      done* is satisfied.

## Notes for the implementer

- The C++ `material_id` digest is already proven by `level_golden`; the *new* value
  here is the initial-worm + empty-pool contribution, `rand.last==0`, `cycles==0`,
  and the Rust hash arithmetic. Keep the test focused on that.
- Empty-pool component hashes are `1` — assert this explicitly; it pins the seed.
- If `weap_order`/`Objects` resolution proves fiddly, the hash only needs the
  resulting `type->id` and `ammo`; pick the simplest faithful path and document it.
- Do not introduce viewports, `ProcessFrame`, or any per-entity `Process` — those are
  Slice 2+. Resist widening `WormState` beyond what the tick-0 hash reads.
- **`StartGame()` gap (verify, then decide consciously).** The C++ fixture
  (`test_determinism.cpp:84–92`) calls `StartGame()` *between* `InitWeapons` and
  `ResetWorms`; the dumper sketch in Task 5 omits it. Before generating the golden,
  check whether `Game::StartGame()` mutates any field the tick-0 hash reads (worm
  `visible`/`killed_timer`/`timer`/weapons, `cycles`, `rand`). If it does, include it
  in the dumper (Rust matches the golden either way — this is about the dumper being a
  faithful tick-0 state, not a Rust/C++ mismatch); if it only touches unhashed/render
  state, the omission is fine and should be noted in the PR. `ResetWorms` is called
  last in both, so it dominates the worm tick-0 values regardless.
