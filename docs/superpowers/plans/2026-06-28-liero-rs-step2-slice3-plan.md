# Step 2, Slice 3 — Worm control + aiming: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking. **Test-first**: write the failing
> test (or its assertion) before the implementation it pins.

**Goal:** Port the rest of `Worm::Process` minus combat — `ProcessAiming`,
`ProcessTasks` (jump / ninjarope), `ProcessWeapons` (weapon-timer countdown),
`ProcessWeaponChange`, `ProcessMovement` (dig body deferred), plus the
control-state bit mutations — into the `sim` crate, and prove the Rust sim
reproduces the C++ **master `HashGameState`** *and* all component hashes
**tick-for-tick** under **scripted input**, for two visible worms over ≈150 ticks.
This is the slice where the master assertion turns on.

**Architecture:** Extend `rust/sim/` (no new crate; deps unchanged: `sim-core`,
`assets`; Bevy-free, float-free). `WormState` gains the non-hashed control state
the ported methods read/write (`aiming_speed`, `direction`, `movable`,
`able_to_jump`, `able_to_dig`, `key_change_pressed`, `current_weapon`,
`fire_cone`, `leave_shell_timer`). A new `ControlConsts` (from `TcConfig`) carries
the aim/move/jump/ninjarope constants. `ControlState` gains mutating
`press`/`release`/`pressed_once`. The Slice-2 driver `process_worm_physics` becomes
`process_worms`, running the full per-worm `Process` in C++ order with `reacts`
shared by tasks + physics. **No C++ changes** — the existing
`oracle_dump_sim_physics` already drives the full `Process`, parses `input` lines,
and dumps the master hash; Slice 3 adds a new scripted scenario + golden + test
and flips Slice 2's master assertion on.

**Tech stack:** Rust only (`sim` extend, `oracle-tests`). The committed C++
golden is regenerated locally via the *existing* `oracle_dump_sim_physics`
(`OPENLIERO_BUILD_ORACLE_DUMP`); CI (`rust.yml`, `cargo test --workspace`) runs
against the committed golden. `data/TC/openliero` real TC.

## Global constraints

- **Bit-exact vs C++.** Sources of truth: `worm.cpp:1003-1062` (`ProcessAiming`),
  `959-1001` (`ProcessTasks`), `811-848` (`ProcessWeapons`), `1064-1098`
  (`ProcessWeaponChange`), `850-957` (`ProcessMovement`), `210-353` (the `Process`
  ordering), `worm.hpp:185-201` (`Pressed`/`Press`/`Release`/`PressedOnce`),
  `stateHash.hpp:25-50` (master worm fields). All integer arithmetic uses
  `wrapping_*`; division truncates toward zero (Rust `/` / `wrapping_div`, **not**
  `>>`); `Ftoi` is arithmetic `>>16`, `Itof` is `<<16` (already in
  `sim-core::fixed`).
- **RNG + level pristine.** No `rand()` is consumed and no pixel is written under
  the scenario (no Left+Right ⇒ no dig; no Fire; `health == 100`). `rand.last`
  stays `0`, the `level` component stays constant. (Design doc, *RNG decision*.)
- **Master hash ON.** The new test asserts `state_hash` **and** all 9 component
  columns per tick. The Slice-2 test also flips its master assertion on (now
  matches). (Design doc, *Master-hash flip-on*.)
- **No new *hashed* field.** `WormState` already carries every master-hashed field
  (the hash is built; `hash.rs` already folds them). The added fields are
  non-hashed control state driving the hashed fields' evolution. Defaults must be
  the verified post-`ResetWorms`/ctor constants. (Design doc, *Datamodel*.)
- **`ProcessSight` omitted; dig body deferred to Slice 4.** Both have no hashed
  effect under the scenario; the dig body draws RNG + writes the level and is
  guarded by a `debug_assert!`.
- **Scenario is the single source of truth**, read by both the (unchanged) C++
  dumper and the Rust test via `oracle_tests::scenario`.
- **Golden regen is LOCAL/MANUAL** (full C++ build links `game`); CI runs against
  the committed golden. `PRESET` defaults to `macos-arm64`.
- **No AI / "Generated with" taglines** in commits or files.
- **Bash discipline:** no `>>`, heredoc, `&&`, `;`, `$VAR` chaining in commands
  that hit the permission prompt — one command per call; create files with the editor.

## File structure

- `rust/sim/src/state.rs` — MODIFY: `WormState` + `from_init` gain the 9 control
  fields; `ControlState` gains `press`/`release`/`pressed_once`; `SimState` carries
  `ControlConsts`; rename `process_worm_physics` → `process_worms` and run the full
  per-worm pass.
- `rust/sim/src/physics.rs` (or a new `rust/sim/src/control.rs`) — NEW: `ControlConsts`
  + `from_tc`, `process_aiming`, `process_tasks`, `process_weapons`,
  `process_weapon_change`, `process_movement`, `process_steerables` (no-op).
  Recommend a new `control.rs` to keep `physics.rs` focused; `pub mod control;` in
  `lib.rs`.
- `rust/sim/src/lib.rs` — MODIFY: `pub mod control;` (if used).
- `rust/oracle-tests/golden/sim_slice3_scenario.txt` — NEW: scripted scenario.
- `rust/oracle-tests/gen_sim_slice3_golden.sh` — NEW: regenerate the golden
  (uses the existing `oracle_dump_sim_physics`).
- `rust/oracle-tests/golden/sim_slice3.txt` — NEW: committed golden.
- `rust/oracle-tests/tests/sim_slice3_golden.rs` — NEW: master+component test.
- `rust/oracle-tests/tests/sim_slice2_golden.rs` — MODIFY: flip master assertion on.
- **No C++ files change.** (`sim_physics_dump.cpp`, `CMakeLists.txt` untouched.)

---

### Task 0: `WormState` control fields + `ControlConsts` + `ControlState` mutators

De-risk the data shapes before any behaviour: the new worm fields, the constants
struct, and the control-bit mutators every ported method needs.

**Files:** `rust/sim/src/state.rs`, `rust/sim/src/control.rs` (new).

- [ ] **Step 1 (test):** In `state.rs` tests: `WormState::from_init` sets the new
      fields to their post-`ResetWorms`/ctor defaults — `movable = true`,
      `current_weapon = 0`, `direction = 0`, `aiming_speed = 0`,
      `able_to_jump = false`, `able_to_dig = false`, `key_change_pressed = false`,
      `fire_cone = 0`, `leave_shell_timer = 0`. Existing Slice-1/2 tick-0 tests
      still pass (new fields are not hashed).
- [ ] **Step 2 (impl):** Add the 9 fields to `WormState`; `from_init` initialises
      them as above. (No change to `hash.rs` — none of these are hashed.)
- [ ] **Step 3 (test):** `ControlState` mutators — `press(n)`/`release(n)` set/clear
      a bit; `pressed_once(n) -> bool` returns the bit then clears it (mirroring C++
      `Press`/`Release`/`PressedOnce`, `worm.hpp:187-195`). Pin: `pressed_once`
      returns the prior bit and leaves it cleared; `pack()` reflects the clear.
- [ ] **Step 4 (impl):** Implement the mutators (`pressed_once` takes `&mut self`).
- [ ] **Step 5 (test+impl):** `ControlConsts` with the aim/move/jump/ninjarope
      fields (design doc table) + `from_tc(&TcConfig)`; unit-test `from_tc` pulls
      the documented `constants`/`hacks` (spot-check `JumpForce`, `AimFricMult`,
      `MultiJump`). Carry `ControlConsts` on `SimState` (new `new` param or build
      from the same `&tc`); keep the driver signature `(state, inputs)`.
- [ ] **Verify:** `cargo test -p sim` green; `cargo test -p oracle-tests
      sim_slice1` and `sim_slice2` still green.

---

### Task 1: `ProcessAiming` (test-first)

**Files:** `rust/sim/src/control.rs`.

Source: `worm.cpp:1003-1062`. Reads `aiming_speed`, `aiming_angle`, `direction`,
`movable`, `ninjarope.out`, control Up/Down/Change; writes `aiming_speed`,
`aiming_angle`. No RNG, no tables.

- [ ] **Step 1 (test):** Hand-folded unit tests with real openliero aim constants:
      - **Integrate:** `aiming_speed != 0` ⇒ `aiming_angle += aiming_speed`.
      - **Friction:** no Up/Down ⇒ `aiming_speed = aiming_speed * AimFricMult /
        AimFricDiv` (truncating; test a negative speed).
      - **Clamp (direction 1):** `Ftoi(aiming_angle) > AimMaxRight` ⇒ `aiming_speed
        = 0`, `aiming_angle = Itof(AimMaxRight)`; same for `< AimMinRight`.
      - **Clamp (direction 0):** the `AimMaxLeft`/`AimMinLeft` mirror.
      - **Accel:** `movable && (!ninjarope.out || !Change)` with Up (dir 0) ⇒
        `aiming_speed += AimAccLeft` capped at `MaxAimVelLeft`; the dir-1 / Down
        branches per `worm.cpp:1037-1061`.
      - **Gated off:** `!movable` or (`ninjarope.out && Change`) ⇒ no accel.
- [ ] **Step 2 (impl):** Port `process_aiming(worm, c: &ControlConsts)` verbatim,
      including the order (integrate+clamp first, then accel) and the
      direction-dependent branches.
- [ ] **Verify:** `cargo test -p sim` green.

---

### Task 2: `ProcessTasks` — jump + ninjarope (test-first)

**Files:** `rust/sim/src/control.rs`.

Source: `worm.cpp:959-1001`. Takes `reacts` (from the orchestration). Reads
control Change/Jump/Up/Down, `reacts[kRfUp]`, `able_to_jump`, `ninjarope.out`,
hacks `AirJump`/`MultiJump`; writes `vel.y`, `able_to_jump`, `ninjarope.out/pos`,
and (via `PressedOnce(kJump)`) the Jump control bit. No RNG.

- [ ] **Step 1 (test):**
      - **Jump (grounded):** `!Change && Jump`, `reacts[kRfUp] > 0`, `able_to_jump`
        ⇒ `vel.y -= JumpForce`, `able_to_jump = false`, `ninjarope.out = false`.
      - **Jump gated:** airborne (`reacts[kRfUp] == 0`, `AirJump` off) ⇒ no impulse.
      - **`able_to_jump` reset:** `!Change && !Jump` ⇒ `able_to_jump = true`.
      - **Throw:** `Change && PressedOnce(Jump)` ⇒ `ninjarope.out = true`,
        `ninjarope.pos = worm.pos`, **Jump bit cleared** in `control_states`.
        (`ninjarope.vel`/`length` skipped — not hashed; design doc OQ5.)
      - **Rope pull/release** (`Change`, `ninjarope.out`, Up/Down): clamps
        `length` to `[NRMinLength, NRMaxLength]` — *only if* the `length` field is
        added; if not, document that pull/release is a no-op on hashed state (rope
        `length` is not hashed and `pos`/`out` are unchanged by pull) and skip.
- [ ] **Step 2 (impl):** Port `process_tasks(worm, reacts, c)`; the `MultiJump`/
      `AirJump` hacks come from `ControlConsts`. Use `pressed_once` for the Jump
      clear. Set only `ninjarope.out`/`pos` on throw (rope frozen, design doc).
- [ ] **Verify:** `cargo test -p sim` green.

---

### Task 3: `ProcessWeapons` — weapon-timer countdown (test-first)

**Files:** `rust/sim/src/control.rs`.

Source: `worm.cpp:811-848`. The only **hashed** effect under the scenario is the
`delay_left` countdown on every slot. Reads `current_weapon`; the `ammo <= 0`
reload branch (sets `loading_left`/`ammo`) is **never entered** (`ammo > 0` for
all slots, no Fire), `fire_cone`/`leave_shell_timer` start `0`.

- [ ] **Step 1 (test):**
      - **Countdown:** each weapon with `delay_left >= 0` decrements by 1;
        `delay_left == 0 → -1`; `-1` stays `-1` (the `>= 0` guard). Assert across
        all 5 slots over 2 ticks.
      - **Inert branches:** `fire_cone == 0` stays `0`; `leave_shell_timer == 0`
        stays `0` (the `> 0` branch — which draws RNG in C++ — is never entered);
        `loading_left == 0` stays `0` (the `ammo <= 0` branch not entered).
- [ ] **Step 2 (impl):** Port `process_weapons(worm)`: decrement all `delay_left`
      (guarded `>= 0`); decrement `fire_cone` if `> 0`; the `leave_shell_timer > 0`
      branch and the `ammo <= 0` reload branch may be left as faithful-but-inert
      (guard them; the shell-drop `rand()` + nobject spawn and the
      `ComputedLoadingTime` reload land in Slice 4 when Fire depletes ammo —
      document with a `debug_assert!`/TODO that they are unreached this slice).
- [ ] **Verify:** `cargo test -p sim` green.

---

### Task 4: `ProcessWeaponChange` + `ProcessMovement` (test-first)

**Files:** `rust/sim/src/control.rs`.

Sources: `worm.cpp:1064-1098` (`ProcessWeaponChange`), `850-957`
(`ProcessMovement`). The change/movement split is the `if Pressed(kChange)` gate
at `worm.cpp:348-353`.

- [ ] **Step 1 (test) — weapon change:**
      - **First change-tick:** `!key_change_pressed` ⇒ `Release(Left)`,
        `Release(Right)`, `key_change_pressed = true` (Left/Right bits cleared in
        `control_states`).
      - **Cycle:** `PressedOnce(Left)` ⇒ `current_weapon` decrements (wrapping to
        `NUM_WEAPONS-1`); `PressedOnce(Right)` ⇒ increments (wrapping to `0`);
        each clears its bit. (`fire_cone = 0`, `animate = false` are non-hashed.)
- [ ] **Step 2 (test) — movement:**
      - **Walk right:** `movable && Right && !Left` ⇒ `vel.x += WalkVelRight`
        capped below `MaxVelRight`; `direction` 0→1 with the `aiming_angle` flip
        (`aiming_angle = Itof(128) - aiming_angle` when `<= Itof(64)`),
        `aiming_speed = 0`.
      - **Walk left:** the `WalkVelLeft`/`MaxVelLeft` + `direction` 1→0 mirror.
      - **`able_to_dig` toggle:** `!(Left && Right)` ⇒ `able_to_dig = true`.
      - **Dig deferred:** `Left && Right && able_to_dig` ⇒ `able_to_dig = false`
        then the terrain body is `debug_assert!(false, "dig DrawDirtEffect deferred
        to Slice 4; scenario must not hold Left+Right")` (unreached under the
        scenario). Pin with a `#[should_panic]` test in debug.
- [ ] **Step 3 (impl):** Port `process_weapon_change(worm)` and
      `process_movement(worm, c)`; the change/movement gate lives in the driver
      (Task 5). Use `pressed_once`/`release` for the bit clears.
- [ ] **Verify:** `cargo test -p sim` green.

---

### Task 5: per-worm pass + driver rename `process_worms` (test-first)

**Files:** `rust/sim/src/state.rs` (driver), `rust/sim/src/control.rs`
(`process_steerables` no-op).

The orchestration: `reacts` computed **once** and shared by `process_tasks` and
`process_physics`; methods run in exact C++ order (design doc, *Per-worm pass*).

- [ ] **Step 1 (test):** Integration tests on a hand-built grounded worm + real
      consts:
      - **Jump sequence:** worm grounded (`reacts[kRfUp] > 0`), one empty tick sets
        `able_to_jump`, then a `Jump` tick gives `vel.y -= JumpForce` *before*
        physics applies gravity/bounce — assert the resulting `vel.y`.
      - **Aim sequence:** several `Up` ticks change `aiming_angle` monotonically
        until the clamp.
      - **Weapon-change tick:** `Change|Right` cycles `current_weapon` and the
        Right bit is cleared in `control_states.pack()`.
      - **Empty-input equivalence:** under empty input the worm-component hashes
        equal Slice 2's (pos/vel only) — a guard that the new methods don't perturb
        physics.
- [ ] **Step 2 (impl):** `process_steerables` = set `steerable_count = 0` (no-op;
      empty `wobjects`). Rewrite the driver as `process_worms(&mut self, inputs:
      &[ControlState])`: **for each worm in order** — `control_states = input[i]`
      (overwrite, mirroring `Unpack`), then run the pass: `worm_reactions` →
      `process_steerables` → movable-reset → `process_aiming` → `process_tasks(reacts)`
      → `process_weapons` → `process_physics(reacts)` → `if Pressed(Change)
      { process_weapon_change } else { key_change_pressed = false; process_movement }`.
      **No** `cycles++`, RNG, object/ninjarope loops, ProcessSight.
- [ ] **Step 3 (impl):** Update the Slice-2 test call `process_worm_physics` →
      `process_worms` (mechanical).
- [ ] **Verify:** `cargo test -p sim` green; `cargo test -p oracle-tests sim_slice2`
      still green (components unchanged under empty input). Cross-read
      `worm.cpp:210-353` against the driver order (note in the PR).

---

### Task 6: scenario file + `gen_sim_slice3_golden.sh` + committed golden

**Files:** `rust/oracle-tests/golden/sim_slice3_scenario.txt`,
`rust/oracle-tests/gen_sim_slice3_golden.sh`,
`rust/oracle-tests/golden/sim_slice3.txt`.

- [ ] **Step 1:** Create `sim_slice3_scenario.txt`: `seed 42`, `level
      Levels/physics_fall_test.lev`, `ticks ≈ 150`, two visible mid-air worms
      (different `x`/`y`). Add `input` lines exercising the phases (fall+land →
      walk right → aim up/down → walk left → jump → weapon-change → ninjarope
      throw/retract; design doc, *Input scenario*). **Constraints (comment them):
      never set Left(4)+Right(8) together, never set Fire(16), keep health 100.**
      Worm 1 gets a different L+R-free / Fire-free pattern so the hashes diverge.
- [ ] **Step 2:** Create `gen_sim_slice3_golden.sh` (copy of
      `gen_sim_physics_golden.sh`): `set -euo pipefail`, `PRESET=${PRESET:-macos-arm64}`,
      configure `-DOPENLIERO_BUILD_ORACLE_DUMP=ON`, build the **existing**
      `oracle_dump_sim_physics`, run it from ROOT with the slice-3 scenario + output
      paths. Mark LOCAL/MANUAL. `chmod +x`.
- [ ] **Step 3:** Run it to produce `sim_slice3.txt`; commit both. Inspect the
      golden: the `rng` column is `00000000` and the `level`/pool columns are
      constant on every line (proves RNG+level pristine — i.e. no accidental dig);
      `worm0`/`worm1` and the **master** column change across the phases. If `rng`
      or `level` ever moves, the scenario triggered dig/Fire — fix the input.
- [ ] **Verify:** `sim_slice3.txt` has `ticks + 1` lines, 11 columns each; `rng`
      all `00000000`, `level` + 5 pools constant.

---

### Task 7: Rust differential test `sim_slice3_golden` (test-first against golden)

**Files:** `rust/oracle-tests/tests/sim_slice3_golden.rs`.

- [ ] **Step 1:** Mirror `sim_slice2_golden.rs` setup: parse
      `sim_slice3_scenario.txt` via `oracle_tests::scenario`; load the same `.lev`,
      `TcConfig` (materials + `PhysicsConsts::from_tc` + `ControlConsts::from_tc`),
      resolve weapons; build `SimState::new(...)` with the scenario worm inits.
      `parse_golden` keeps **all** columns including `state_hash` (master).
- [ ] **Step 2:** Assert tick-0 (master + 9 components) against the freshly-built
      state. Then for `k` in `1..=ticks`: call `process_worms([unpack(scn.input(k-1,
      0)), unpack(scn.input(k-1, 1))])` (**input keyed `k-1`**, design doc *Input
      timing*) and assert master + all 9 components against golden line `k`. Assert
      **components first** (debugging ladder: rng → level → worm0 → worm1 → pools),
      then the master, so a divergence localises to a tick + subsystem before the
      master flags it.
- [ ] **Step 3:** Add a coverage guard: assert that across the run `aiming_angle`,
      some weapon's `delay_left`, `control_states.pack()`, and `ninjarope.out`
      each take at least two distinct values (so the golden actually exercises the
      ported paths, not just physics).
- [ ] **Verify:** `cargo test -p oracle-tests sim_slice3` green.

---

### Task 8: flip Slice-2 master assertion on

**Files:** `rust/oracle-tests/tests/sim_slice2_golden.rs`.

- [ ] **Step 1:** Change `parse_golden` to keep the `state_hash` column; in
      `assert_components`, assert the master too (`hash_game_state(state)` ==
      golden master). Update the module doc: the master now matches because Slice 3
      ported the `ProcessWeapons` `delay_left` countdown (the gap Slice 2
      documented).
- [ ] **Verify:** `cargo test -p oracle-tests sim_slice2` green (master now
      asserted on the empty-input golden).

---

### Task 9: Wire-up review and done-check

- [ ] **Step 1:** `cargo test --workspace` green; `sim` has no Bevy / no float /
      no deps beyond `sim-core` + `assets`.
- [ ] **Step 2:** Re-read `worm.cpp:1003-1062, 959-1001, 811-848, 1064-1098,
      850-957` and `210-353` against `control.rs`/the driver: aiming branches,
      jump gate, weapon-timer countdown, weapon-cycle wrap, walk clamps + direction
      flip, the control-bit clears, and the C++ method order must match exactly.
- [ ] **Step 3:** Confirm the scenario never sets Left+Right or Fire, and that
      `sim_slice3.txt`'s `rng` column is all `00000000` and `level`/pool columns
      constant (RNG + level pristine).
- [ ] **Step 4:** Confirm **no C++ changed** — the dumper, `CMakeLists.txt`, and
      all sim code are untouched, so `test_determinism` / `test_rollback_*` are
      unaffected (no need to rebuild/run them, but note this in the PR).
- [ ] **Step 5:** Update the Step 2 overview's *Slice ordering* (mark Slice 3 done
      + master-hash matched) (docs only). Do **not** commit unrelated changes.
- [ ] **Definition of done:** every checkbox in the slice-3 design's *Definition of
      done* is satisfied.

## Notes for the implementer

- **The master hash is the whole point.** The component worm hash reads only
  `{pos, vel, health, lives, visible, timer}`, so aiming/control/weapons/ninjarope
  bugs are invisible until the master assertion. Build the master test (Task 7)
  early and lean on the component columns to localise before the master fires.
- **Control bits are hashed.** `control_states.Pack()` is in the master hash, read
  *after* `Process` clears bits (`PressedOnce`/`Release`). Overwrite from the
  tick's input, then apply the clears, exactly as the dumper does — `PressedOnce`
  degenerates to a per-tick read+clear because the input is re-`Unpack`ed each tick
  (design doc, *Control-state mutation*).
- **`reacts` is shared and computed once.** Jump (tasks) and bounce/gravity
  (physics) read the same `reacts`. Do not recompute it between, and do not let the
  nudge corrections run twice.
- **Velocity ordering is asymmetric.** Jump writes `vel.y` *before* physics; walk
  writes `vel.x` *after* physics. Copy both.
- **Truncating division.** Aim friction (`* AimFricMult / AimFricDiv`) and the walk
  velocity arithmetic truncate toward zero — Rust `/` / `wrapping_*`, never `>>`.
- **Dig is deferred, not skipped silently.** Port the `able_to_dig` control flow;
  the terrain body is a `debug_assert!(false, ...)` so an accidental Left+Right
  scenario panics loudly. `DrawDirtEffect` (RNG + level mutation) lands in Slice 4.
- **No C++ work.** The existing `oracle_dump_sim_physics` already produces the
  master hashes for scripted input — this slice is all Rust plus a new scenario,
  golden, and test, and flipping Slice 2's master on.
- **Defaults are load-bearing.** `from_init` must set `movable = true`,
  `current_weapon = 0`, `direction = 0`, etc. (post-`ResetWorms`/ctor) — these are
  not hashed at tick 0 but steer the hashed fields under input.
