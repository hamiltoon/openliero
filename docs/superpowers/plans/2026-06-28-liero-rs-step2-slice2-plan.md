# Step 2, Slice 2 — One worm, physics only: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking. **Test-first**: write the failing
> test (or its assertion) before the implementation it pins.

**Goal:** Port the worm terrain-collision + gravity physics
(`Worm::CalculateReactionForce`, the reaction-force orchestration in
`Worm::Process`, and `Worm::ProcessPhysics`) into the `sim` crate, and prove the
Rust sim reproduces the C++ **worm component hash tick-for-tick over N≈100 ticks**
for a single visible worm falling under gravity onto terrain, under scripted
(empty) input. This is the first *dynamics* slice and introduces the **per-tick
checksum time-series** oracle.

**Architecture:** Extend `rust/sim/` (no new crate; deps unchanged: `sim-core`,
`assets`; Bevy-free, float-free). `LevelSim` gains a 256-entry material-flag table
+ a `checked_mat_background` helper reproducing `Level::CheckedMatWrap(...)
.Background()`. A new `sim::physics` module (or `SimState`/`WormState` methods)
runs the reaction probes + `ProcessPhysics`, driven by
`SimState::process_worm_physics(&mut self, inputs)`. `WormInit`/`WormState` gain
`start_pos` + `visible`. A `PhysicsConsts` (from `TcConfig`) carries the physics
constants. Correctness is proven by a NEW C++ dumper
(`oracle_dump_sim_physics`, links `game`) that drives `worm->Process` N ticks and
emits a per-tick hash record, a committed scenario file, an N-line golden, and an
N-tick differential test.

**Tech stack:** Rust (`sim` extend, `oracle-tests`), C++ oracle dumper
(`sim_physics_dump.cpp`, links `game`), CMake option `OPENLIERO_BUILD_ORACLE_DUMP`,
the engine's `uint32_t` hashes (emitted as hex), `data/TC/openliero` real TC.

## Global constraints

- **Bit-exact vs C++.** Sources of truth: `worm.cpp:97-147` (`CalculateReactionForce`),
  `worm.cpp:221-283` (reaction orchestration in `Process`), `worm.cpp:149-208`
  (`ProcessPhysics`), `level.hpp:124-130` (`CheckedMatWrap`), `material.hpp:11`
  (`kBackground = 1<<3`). All integer arithmetic uses `wrapping_*`; bounce/friction
  division **truncates toward zero** (Rust `/` / `wrapping_div`, **not** `>>`);
  `Ftoi` is arithmetic `>>16` (toward −∞). Reproduce the `CheckedMatWrap` unsigned
  flatten + OOB→flag-table-`[0]` quirk exactly.
- **RNG pristine.** Drive `worm->Process(game)` directly (NOT `ProcessFrame`); no
  `rand()` is consumed under the Slice-2 scenario, so `rand.last == 0` and the
  `rng` component is `0` every tick. Do **not** `++cycles`, do **not**
  `GenerateFromSettings`. (Design doc, *RNG decision*.)
- **Match target = component hashes, per tick.** Assert `worms[0]`, `worms[1]`
  (the physics) plus the invariants `rng==0`, constant `level`, all five pools `==1`,
  for all `N+1` ticks. The **master hash is dumped but NOT asserted** (it diverges
  via the un-ported `ProcessWeapons` `delay_left` countdown; flipped on in Slice 3).
- **Scenario file is the single source of truth**, read by both the C++ dumper and
  the Rust test (no duplicated fixture constants). Empty input = absence of
  override lines.
- **No new persisted worm state beyond `start_pos`/`visible`.** `reacts` is a
  per-tick local. Do not add `movable`/`able_to_jump`/`aiming_speed`/`direction`
  (Slice 3). Resist widening (design doc, *Datamodel*).
- **No Bevy, no float in `sim`.** Worm pos/vel/consts use `sim-core` `Vec2`/`Fixed`.
- **Modernise, don't transliterate.** Idiomatic Rust; the oracle proves behaviour.
  C++ dumper matches `sim_dump.cpp`'s style.
- **Golden regen is LOCAL/MANUAL** (full C++ build links `game`); CI (`rust.yml`,
  `cargo test --workspace`) runs against the committed golden. `PRESET` defaults to
  `macos-arm64`.
- **No AI/"Generated with" taglines** in commits or files.
- **Bash discipline:** no `>>`, heredoc, `&&`, `;`, `$VAR` chaining in commands
  that hit the permission prompt — one command per call; create files with the editor.

## File structure

- `rust/sim/src/state.rs` — MODIFY: `WormInit`/`WormState` gain `start_pos` +
  `visible`; `LevelSim` gains `material_flags: [u8; 256]` + `checked_mat_background`;
  `SimState::new` takes/derives the flag table + `PhysicsConsts`.
- `rust/sim/src/physics.rs` — NEW: `PhysicsConsts`, `calculate_reaction_force`, the
  reaction orchestration, `process_physics`, and `SimState::process_worm_physics`.
- `rust/sim/src/lib.rs` — MODIFY: `pub mod physics;`.
- `src/tools/oracle_dump/sim_physics_dump.cpp` — NEW: N-tick physics dumper,
  links `game`.
- `CMakeLists.txt` — MODIFY: add `oracle_dump_sim_physics` inside the
  `OPENLIERO_BUILD_ORACLE_DUMP` block (after `oracle_dump_sim`, lines 386-387).
- `rust/oracle-tests/golden/sim_slice2_scenario.txt` — NEW: committed scenario.
- `rust/oracle-tests/gen_sim_physics_golden.sh` — NEW: regenerate the golden.
- `rust/oracle-tests/golden/sim_slice2.txt` — NEW: committed N-line golden.
- `rust/oracle-tests/tests/sim_slice2_golden.rs` — NEW: N-tick differential test.
- (`rust/oracle-tests/Cargo.toml` already has `sim` as a dev-dep from Slice 1.)

---

### Task 0: Scenario format + datamodel scaffolding (`start_pos`, `visible`, flags)

De-risk the data shapes before any physics: the new worm-init fields, the level
flag table, and a parser for the scenario file the dumper and test will share.

**Files:** `rust/sim/src/state.rs`, a small scenario parser (in the test crate or
a `sim` helper — recommend the test crate, since only oracle-tests reads it).

- [ ] **Step 1 (test):** Extend `state.rs` tests: `WormState::from_init` honours a
      non-zero `start_pos` and `visible = true`; existing Slice-1 tests still pass
      with the new fields defaulted (`start_pos = (0,0)`, `visible = false`).
- [ ] **Step 2 (impl):** Add `start_pos: Vec2` and `visible: bool` to `WormInit`;
      `from_init` sets `pos = init.start_pos`, `visible = init.visible`. Update the
      Slice-1 `sim_slice1_golden.rs` `WormInit` literals to set `start_pos:
      Vec2::zero(), visible: false` (tick-0 fixture unchanged).
- [ ] **Step 3 (test+impl):** Write a tiny scenario parser (`seed`, `level`,
      `ticks`, `worm <idx> <x> <y> <health> <lives> <stats_x> <visible>`, sparse
      `input <tick> <w0> <w1>`). Unit-test it on the committed
      `sim_slice2_scenario.txt` fixture (Task 6 creates the file; for now test a
      synthetic string): yields the seed, level path, tick count, two worm inits,
      and an input lookup that returns `0` for un-overridden ticks.
- [ ] **Verify:** `cargo test -p sim` + the parser test green; `cargo test -p
      oracle-tests sim_slice1` still green.

---

### Task 1: `LevelSim` material flags + `checked_mat_background` (test-first)

The collision probes need `CheckedMatWrap(...).Background()`.

**Files:** `rust/sim/src/state.rs`.

- [ ] **Step 1 (test):** Tests for `checked_mat_background` on a synthetic 4×4
      `LevelSim` with a chosen `material_flags` table:
      - an in-bounds background pixel returns `true`; a rock pixel `false`.
      - the OOB index (`x` or `y` out of range so `x + y*width >= w*h`) returns the
        flag of **table index 0** (`common.materials[0]`), NOT `material_id[0]`'s.
      - the **wrap quirk**: a negative `x` with a `y` such that `x + y*width` lands
        back inside `[0, w*h)` reads the *wrapped* pixel (assert it reads that
        wrong-row cell, matching the unsigned flatten).
- [ ] **Step 2 (impl):** Add `material_flags: [u8; 256]` to `LevelSim`; implement
      `checked_mat_background(&self, x, y) -> bool` per the design doc (unsigned
      flatten via `(x.wrapping_add(y.wrapping_mul(width))) as u32`, compare against
      `material_id.len()`, OOB → `material_flags[0]`, test bit `0x08`). `SimState::new`
      fills `material_flags` from the caller-supplied `TcConfig.materials` (add a
      `&[u8; 256]` or `&TcConfig` param, or a dedicated `material_flags` arg —
      keep it explicit; the test/dumper pass the real table).
- [ ] **Verify:** `cargo test -p sim` green.

---

### Task 2: `calculate_reaction_force` (test-first)

**Files:** `rust/sim/src/physics.rs`, `rust/sim/src/lib.rs`.

- [ ] **Step 1 (test):** Port the `kColPoints[4][7]` table + `kColPointCount =
      {3,7,3,7}` and test `calculate_reaction_force(&level, x, y, dir, &mut reacts)`
      on a synthetic level:
      - all-background neighbourhood → `reacts[dir] == 0`.
      - a fully solid (non-background) neighbourhood → `reacts[dir] ==
        kColPointCount[dir]` (3 for DOWN/UP, 7 for LEFT/RIGHT).
      - a partial pattern → the exact count of non-background probe points,
        confirming the probe offsets and `dir` indexing match `worm.cpp:98-146`.
- [ ] **Step 2 (impl):** Implement `calculate_reaction_force` reproducing the
      C++ table and loop exactly (`reacts[dir] = 0`; for each of `kColPointCount[dir]`
      points, `if !checked_mat_background(x+dx, y+dy) { reacts[dir] += 1 }`).
      Mirror the `enum { kRfDown=0, kRfLeft=1, kRfUp=2, kRfRight=3 }` indices.
- [ ] **Verify:** `cargo test -p sim` green.

---

### Task 3: reaction orchestration + `process_physics` + driver (test-first)

The heart of the slice: free-fall, collision, bounce, integration.

**Files:** `rust/sim/src/physics.rs`, `rust/sim/src/state.rs` (driver),
`rust/sim/src/lib.rs`.

- [ ] **Step 1 (test):** Unit tests on synthetic levels + a `PhysicsConsts` built
      from the real openliero values (gravity 1500, fric 89/100, MinBounce ±53248,
      FallDamage 0, hacks false):
      - **Free-fall:** a worm in all-background space, `vel = 0`, over K ticks:
        `vel.y` increases by `WormGravity` each tick (no `reacts[kRfUp]`), `pos.y`
        advances by `vel.y` (guarded integration); assert exact fixed-point values
        for the first few ticks (hand-folded).
      - **Bounce:** a worm moving down into a solid floor (`reacts` for the
        downward read ≥ some value, `kAbsvel.y > mbv`): `vel.y` becomes
        `(-vel.y)/3` (truncating), and the integration is suppressed when the
        relevant `reacts < 2` fails. Assert the sign flip + magnitude.
      - **Stop:** slow downward velocity (`kAbsvel.y <= mbv`) → `vel.y = 0`.
      - **Friction:** `reacts[kRfUp] > 0` grounds the worm → `vel.x = (vel.x*89)/100`
        (truncating); assert on a negative `vel.x` (truncation toward zero).
      - **Edge additions:** a worm near `x < 4` / `y < 5` / `x > width-5` /
        `y > height-6` accumulates the `+5` reaction additions (every iteration of
        the 4-loop), per `worm.cpp:231-247`.
- [ ] **Step 2 (impl):** Implement the reaction orchestration (`worm.cpp:221-283`:
      `next = pos+vel`, `i_next = Ftoi(next)`, 4× `calculate_reaction_force` with the
      in-loop edge additions and the `WormFloat` branch, then the two `pos.y ±
      Itof(1)` nudge corrections) and `process_physics` (`worm.cpp:149-208`:
      friction, the two bounce/stop blocks with `mbh`/`mbv` and `FallDamage`,
      gravity, guarded integration). All `wrapping_*` + truncating `/`. Then the
      driver `SimState::process_worm_physics(&mut self, inputs: &[ControlState])`:
      apply each worm's input to `control_states`, then for each worm in `worms`
      order run reaction-orchestration + `process_physics`. **No** `cycles++`, RNG,
      or object loops.
- [ ] **Verify:** `cargo test -p sim` green. Cross-read `worm.cpp:149-283` against
      `physics.rs` line by line (note in the PR).

---

### Task 4: C++ dumper `oracle_dump_sim_physics` + CMake target

**Files:** `src/tools/oracle_dump/sim_physics_dump.cpp`, `CMakeLists.txt`.

- [ ] **Step 1:** Create `sim_physics_dump.cpp` (style per `sim_dump.cpp`). In
      `main`: parse the scenario file (`<scenario> <out> [seed-override]`);
      `PrecomputeTables()`; load `Common` from `data/TC/openliero`; `Settings`
      (`game_mode = kGmKillEmAll`, `lives` from scenario, `loading_time = 0`);
      `Game game(...)`; `game.rand.Seed(seed)`. **Load the fixed `.lev`** from the
      scenario via `Level::load` (as `sim_dump.cpp`). Add 2 worms (`settings`,
      `health`, `index`, `stats_x`); `InitWeapons`; `ResetWorms`; then apply the
      scenario start conditions (`w->pos = {x,y}`, `w->visible = true`). **No
      viewports.**
- [ ] **Step 2:** Emit tick 0, then loop `ticks` times: for each worm apply its
      scripted input (`control_states.Unpack(input_for(tick, idx))`), call
      `worm->Process(game)` for each worm in `game.worms` order, then emit the
      record. Each line: `<tick> <HashGameState> <c.rng> <c.level> <c.worms[0]>
      <c.worms[1]> <c.bobjects> <c.bonuses> <c.sobjects> <c.nobjects> <c.wobjects>`
      (`tick` decimal, hashes `%08x`). Do **not** call `ProcessFrame` / `++cycles` /
      `GenerateFromSettings`.
- [ ] **Step 3:** In `CMakeLists.txt`, inside `OPENLIERO_BUILD_ORACLE_DUMP` (after
      the `oracle_dump_sim` block at 386-387), add
      `add_executable(oracle_dump_sim_physics src/tools/oracle_dump/sim_physics_dump.cpp)`
      and `target_link_libraries(oracle_dump_sim_physics PRIVATE game)`.
- [ ] **Verify (local/manual):** configure with `-DOPENLIERO_BUILD_ORACLE_DUMP=ON`
      and build `oracle_dump_sim_physics` (one CMake command per Bash call). It
      links and runs, writing `N+1` well-formed lines. Confirm `test_determinism`
      still passes (the dumper does not modify sim code).

---

### Task 5: scenario file + `gen_sim_physics_golden.sh` + committed golden

**Files:** `rust/oracle-tests/golden/sim_slice2_scenario.txt`,
`rust/oracle-tests/gen_sim_physics_golden.sh`,
`rust/oracle-tests/golden/sim_slice2.txt`.

- [ ] **Step 1:** Create `sim_slice2_scenario.txt`: `seed 42`, `level
      Levels/modern_test.lev`, `ticks 100`, two `worm` lines with start positions
      chosen so the worms fall through background and **collide within 100 ticks**
      (read `modern_test.lev` to pick a column with open sky above terrain;
      different `x` per worm). No `input` lines (empty input).
- [ ] **Step 2:** Create `gen_sim_physics_golden.sh` following
      `gen_sim_golden.sh`: `set -euo pipefail`; `PRESET=${PRESET:-macos-arm64}`;
      configure with `-DOPENLIERO_BUILD_ORACLE_DUMP=ON`; build `--target
      oracle_dump_sim_physics`; run it from ROOT with the scenario + output paths.
      Mark LOCAL/MANUAL. `chmod +x`.
- [ ] **Step 3:** Run it to produce `sim_slice2.txt`; commit both files. Inspect
      the golden: tick 0 worm columns reflect the start state, later ticks change
      (falling), and at least one tick shows a `vel.y` sign flip (bounce) — if the
      worms only free-fall, adjust start positions or `ticks` and regenerate.
- [ ] **Verify:** the golden has `101` lines, each with 11 columns; `rng` is
      `00000000` and `level`/pool columns are constant on every line.

---

### Task 6: Rust differential test `sim_slice2_golden` (test-first against golden)

**Files:** `rust/oracle-tests/tests/sim_slice2_golden.rs`.

- [ ] **Step 1:** Write the test: parse `sim_slice2_scenario.txt` (seed, level,
      ticks, worm inits, inputs) via the Task-0 parser; load the same `.lev` via
      `assets::level::load`; load `TcConfig` from `data/TC/openliero/tc.cfg` for the
      `materials` flag table + physics constants; resolve weapons as
      `sim_slice1_golden.rs` does; build `SimState::new(...)` with the scenario worm
      inits (`start_pos`, `visible = true`).
- [ ] **Step 2:** Read `sim_slice2.txt`. Assert tick-0 component columns against the
      built state, then loop: `state.process_worm_physics(&inputs_for_tick)` and
      assert each tick's component columns (`rng`, `level`, `worm0`, `worm1`, and
      the five pools) equal the golden line. Assert components FIRST (debugging
      ladder), localising any divergence to a tick + subsystem. **Do not** assert
      the master (`state_hash`) column — note in a comment that it diverges via the
      un-ported `ProcessWeapons` and is matched in Slice 3.
- [ ] **Verify:** `cargo test -p oracle-tests sim_slice2` green; then `cargo test
      --workspace` green.

---

### Task 7: Wire-up review and done-check

- [ ] **Step 1:** Confirm `cargo test --workspace` is green and `sim` has no Bevy /
      no float / no deps beyond `sim-core` + `assets`.
- [ ] **Step 2:** Re-read `worm.cpp:97-208` and `221-283` against `physics.rs`; the
      probe table, edge additions, bounce/friction arithmetic, sign-test indices,
      and integration guards must match exactly. Re-check `CheckedMatWrap` against
      `checked_mat_background` (unsigned flatten, OOB → table `[0]`).
- [ ] **Step 3:** Confirm the dumper never calls `ProcessFrame` / `++cycles` /
      `GenerateFromSettings`, and that `rand.last == 0` on every golden line
      (`rng` column all `00000000`).
- [ ] **Step 4:** Confirm `test_determinism` + `test_rollback_*` still pass on the
      C++ side (the new dumper links `game` but changes no sim code).
- [ ] **Step 5:** Update the Step 2 overview's *Slice ordering* (mark Slice 2 done
      + bit-exact) and *Next artifact* if appropriate (docs only). Do **not** commit
      unrelated changes.
- [ ] **Definition of done:** every checkbox in the slice-2 spec's *Definition of
      done* is satisfied.

## Notes for the implementer

- The component worm hash reads only `{pos, vel, health, lives, visible, timer}`;
  under this scenario only `pos`/`vel` move, so the time series IS the physics.
  Keep the test focused there; the master column is carried for Slice 3, not asserted.
- The single highest-risk detail is `CheckedMatWrap`'s unsigned-flatten + OOB→`[0]`
  semantics (design doc, *The hard 10%*). Unit-test the wrap before trusting the
  golden — a one-pixel collision error desyncs silently after the first bounce.
- Bounce `/3` and friction `/div` truncate toward zero; `Ftoi` shifts toward −∞.
  Do not unify them. Test a negative-velocity bounce (falling-left) explicitly.
- `reacts` is recomputed every tick — keep it a `[i32; 4]` local in the physics
  pass, NOT a stored `WormState` field. Do not add movement/aiming/jump state;
  that is Slice 3.
- Start positions are load-bearing for the golden but not for correctness: Rust
  matches whatever the dumper produced because both read the same scenario file.
  Pick positions that produce a bounce so the golden has collision coverage.
- Resist calling the driver `process_frame`: it is a worms-only physics pass. The
  real `process_frame` (with cycles, the bonus-drop RNG roll, and the object loops
  in C++ order) arrives in Slice 6; naming it narrowly now avoids a golden churn
  when the RNG roll lands.
