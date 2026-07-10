# Step 2, Slice 4a — Projectile lifecycle: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking. **Test-first**: write the failing
> test (or its assertion) before the implementation it pins.

**Goal:** Port the projectile birth-to-death path for one weapon — `Worm::Fire` →
`WObject::Process`/`BlowUpObject` for **`fan`** (the *explodes-into-nothing*
weapon) — into the `sim` crate, extend the driver into a **ProcessFrame subset**
(object loops then worms), and prove the Rust sim reproduces the C++ master
`HashGameState` **and** all component hashes **tick-for-tick** under scripted input
that **fires**. This is the slice where **RNG goes live** (the `rng` column moves);
the **level stays pristine** (no `DrawDirtEffect` — that is 4b).

**Architecture:** Extend `rust/sim/` (no new crate; deps unchanged: `sim-core`,
`assets`; Bevy-free, float-free). `WObject` gains `owner_idx`; `WormWeapon` gains
`available()`; `SimState` carries the resolved `weapons` table + the `cossin`
table; `LevelSim` gains `inside()` + `dirt_rock()`. New `weapon.rs` (or extend
`physics.rs`) holds `worm_fire`/`weapon_fire`/`wobject_process`/`blow_up`. The
Slice-3 driver `process_worms` becomes `process_frame`: the object loops
(sobjects→wobjects→nobjects→bobjects, empty except wobjects) run **before** the
worm loop, and the Fire gate is inserted into the per-worm pass between
`process_weapons` and `process_physics`. The C++ `oracle_dump_sim_physics` is
**extended** (object loops + a `weapon <slot> <name>` scenario directive); the
slice-2/3 goldens must stay byte-identical.

**Tech stack:** Rust (`sim` extend, `oracle-tests`) + one oracle-gated C++ dumper
edit (non-sim). Golden regenerated locally via the extended dumper
(`OPENLIERO_BUILD_ORACLE_DUMP`); CI (`cargo test --workspace`) runs the committed
golden. `data/TC/openliero` real TC; weapon **fan**.

## Global constraints

- **Bit-exact vs C++.** Sources of truth: `worm.cpp:1099-1148` (`Worm::Fire`) +
  `336-340` (Fire gate), `weapon.cpp:16-76` (`Weapon::Fire`), `127-338`
  (`WObject::Process`), `78-125` (`BlowUpObject`), `game.cpp:333-355` (ProcessFrame
  order), `exactObjectList.hpp:36-94` (pool spawn/free/iterate), `stateHash.hpp:96-110`
  (wobject master fields), `material.hpp` (`DirtRock`), `level.hpp` (`Inside`).
- **RNG order is the contract.** Fire draws, in order and only when guarded: spread
  `vel.x` `rand(distribution*2)`, spread `vel.y` `rand(distribution*2)`, colour
  `rand(2)` (start_frame<0), time-var `rand(time_to_explo_v)`. Thread one
  `sim-core::Rand` through; never pull ad hoc. (Overview, *RNG audit*.)
- **Level pristine.** fan `dirt_effect=-1`/`create_on_exp=-1`/`splinter_amount=0`,
  worms out of `detect_distance` (non-firing worm invisible) ⇒ no `DrawDirtEffect`,
  no sobject/nobject/blood ⇒ `level` component constant, `level`-writing code never
  runs this slice.
- **`cycles` stays 0.** Driver runs no `++cycles`, no bonus-drop roll, no ninjarope.
- **Truncating division** everywhere (`/100`, `*100/speed`, `*recoil/100`): Rust
  `/` / `wrapping_*`, never `>>`. `Ftoi` = `>>16` arithmetic, `Itof` = `<<16`
  (sim-core `fixed`).
- **No new *hashed* wobject field** — `WObject` already carries all of them.
- **Scenario is the single source of truth**, read by both the (extended) dumper and
  the Rust test. Golden regen LOCAL/MANUAL; `PRESET` defaults `macos-arm64`.
- **No AI / "Generated with" taglines.** **Bash discipline:** one command per call;
  no `>>`/heredoc/`&&`/`;`/`$VAR` chaining at the permission prompt; create files
  with the editor.

## File structure

- `rust/sim/src/state.rs` — MODIFY: `WObject` gains `owner_idx`; `WormWeapon` gains
  `available()`; `SimState` carries `weapons: Vec<assets::object::Weapon>` + `cossin:
  [Vec2;128]`; `LevelSim` gains `inside()` + `dirt_rock()`; rename
  `process_worms`→`process_frame` running the object loops then worms with the Fire
  gate.
- `rust/sim/src/weapon.rs` — NEW: `worm_fire`, `weapon_fire`, `wobject_process`
  (returns a `WObjectOutcome`), `blow_up`. `pub mod weapon;` in `lib.rs`.
- `rust/sim/src/lib.rs` — MODIFY: `pub mod weapon;`.
- `rust/oracle-tests/src/scenario.rs` — MODIFY: parse `weapon <slot> <name>`.
- `rust/oracle-tests/golden/sim_slice4a_scenario.txt` — NEW.
- `rust/oracle-tests/gen_sim_slice4a_golden.sh` — NEW (LOCAL/MANUAL).
- `rust/oracle-tests/golden/sim_slice4a.txt` — NEW (committed).
- `rust/oracle-tests/tests/sim_slice4a_golden.rs` — NEW.
- `src/tools/oracle_dump/sim_physics_dump.cpp` — MODIFY: object loops before worms;
  `weapon <slot> <name>` directive. (Oracle-gated, non-sim C++.)

---

### Task 0: datamodel — `WObject.owner_idx`, `WormWeapon::available`, `SimState.weapons`/`cossin`, `LevelSim` probes

De-risk the shapes before behaviour.

**Files:** `rust/sim/src/state.rs`.

- [ ] **Step 1 (test):** `WObject` has `owner_idx: i32` (default 0); existing
      wobject hash tests (`hash.rs`) unaffected (owner_idx not hashed). `WormWeapon::
      available()` returns **`loading_left == 0`** (the exact `WormWeapon::Available()`,
      `worm.hpp:35` — **not** `ammo>0`; the gate also tests `delay_left<=0` separately).
- [ ] **Step 2 (impl):** add the field + method.
- [ ] **Step 3 (test):** `LevelSim::inside(x,y)` is a true range check
      (`0<=x<width && 0<=y<height`), distinct from the wrapping `checked_mat_
      background`; `dirt_rock(x,y)` reads `material_flags[material_id[idx]]` and tests
      the `DirtRock` bit set, `false` when `!inside`. Pin with a synthetic level
      (a dirt cell, a rock cell, a background cell, an OOB probe).
- [ ] **Step 4 (impl):** implement `inside` + `dirt_rock`; add the `material.hpp`
      bit consts `kDirt=1<<0`, `kDirt2=1<<1`, `kRock=1<<2` (`kBackground=1<<3`);
      `DirtRock = flags & (kDirt|kDirt2|kRock)` (`material.hpp:22`).
- [ ] **Step 5 (test+impl):** `SimState` carries `weapons: Vec<Weapon>` and `cossin:
      [Vec2;128]`; `SimState::new` takes them (or builds `cossin` via
      `sim_core::tables::precompute_cossin()`). Keep the driver signature `(state,
      inputs)`. Unit-test that `cossin` matches the sim-core table and `weapons` is
      carried.
- [ ] **Verify:** `cargo test -p sim` green; `cargo test -p oracle-tests sim_slice1
      sim_slice2 sim_slice3` still green.

---

### Task 1: `worm_fire` + `weapon_fire` (test-first)

**Files:** `rust/sim/src/weapon.rs`.

Sources: `worm.cpp:1099-1148`, `weapon.cpp:16-76`. Reads `aiming_angle`, `vel`,
`current_weapon`, the weapon def, `cossin`; writes the worm (`ammo`, `delay_left`,
`fire_cone`, `vel` recoil) and **spawns a `WObject`** into the pool; draws RNG.

- [ ] **Step 1 (test) — Fire RNG order + spawn (fan constants):** With a seeded
      `Rand` and the fan `Weapon`, `worm_fire` on a worm at known `pos`/`aiming_angle`/
      `vel`:
      - spawns exactly `parts` (=1) wobjects; the spawned `vel = cossin[angle]*speed/
        100 + firing_vel` **then** `+= (rand(24000)-12000, rand(24000)-12000)` — assert
        the RNG is consumed **x then y** (seed a known stream, hand-compute);
      - `cur_frame = color_bullets - rand(2)` (start_frame<0 path);
      - `time_left = time_to_explo - rand(time_to_explo_v)` (fan: `45 - rand(10)`);
      - worm: `ammo` decremented, `delay_left = w.delay`, `fire_cone = w.fire_cone`,
        `vel -= cossin[Ftoi(aiming_angle)]*recoil/100` (recoil applied **after** the
        parts loop);
      - **leave-shell guard**: `leave_shells=0` ⇒ no `rand` drawn (assert `rand.last`
        reflects exactly 4 draws for fan: spread x, spread y, colour, time-var).
- [ ] **Step 2 (test) — `affect_by_worm` + `HSignedRecoil`:** `affect_by_worm` ⇒
      `speed=max(speed,100)`, `firing_vel = vel*100/speed`; the `HSignedRecoil` hack
      (`recoil>=128 ⇒ recoil-=256`) — fan `recoil=2`, no-op, but pin the branch with a
      synthetic recoil≥128.
- [ ] **Step 3 (impl):** Port `worm_fire`(→`weapon_fire` per part) verbatim, in C++
      statement order (leave-shell, sound skip, speed/firing_vel, parts loop, recoil).
      `weapon_fire` returns/inserts the `WObject` via `Pool::spawn` (assert `Some`).
- [ ] **Verify:** `cargo test -p sim` green.

---

### Task 2: `wobject_process` + `blow_up` (test-first)

**Files:** `rust/sim/src/weapon.rs`.

Sources: `weapon.cpp:127-338` (single non-laser pass), `78-125` (`BlowUpObject`,
fan path). Returns `WObjectOutcome { Keep | Explode | Remove }`; on Explode the
driver calls `blow_up` then frees; `blow_up` for fan only frees (no
sobject/splinter/dirt). No RNG for fan in Process under the scenario.

- [ ] **Step 1 (test) — movement + gravity:** `pos += vel`; with fan `gravity=0`,
      `vel` unchanged on a free-flight tick; assert a multi-tick straight line.
- [ ] **Step 2 (test) — boundary clamp:** `inew_pos = Ftoi(pos+vel)` past each edge
      clamps `pos.{x,y}` to `0` / `Itof(width-1)` / `Itof(height-1)` (`weapon.cpp:234-247`).
- [ ] **Step 3 (test) — ground collision explode:** when `!inside(inew) ||
      dirt_rock(inew)` and `bounce==0` and `expl_ground` ⇒ `Explode`; else (air)
      `vel.y += gravity`. Pin with a synthetic floor.
- [ ] **Step 4 (test) — timeout explode:** `time_to_explo>0` and `--time_left<0` ⇒
      `Explode`. Pin fan's `time_left` countdown to the explode tick.
- [ ] **Step 5 (test) — inert guarded branches:** bounce (`bounce=0`) and the worm-hit
      loop (worms invisible / out of range) draw no RNG and change nothing; assert
      `rand.last` unchanged across a no-fire Process. (`collide_with_objects` loop
      skips same type+owner ⇒ inert with one shot.)
- [ ] **Step 6 (impl):** Port `wobject_process` (the `do{…}while` runs once for fan)
      and `blow_up` (fan: `Pool::free`, nothing else). Guard the deferred branches
      (DrawDirtEffect/splinters/create_on_exp) with `debug_assert!`/TODO referencing
      4b/4c so an unexpected fan-unlike config fails loudly.
- [ ] **Verify:** `cargo test -p sim` green.

---

### Task 3: driver `process_frame` (object loops + Fire gate) (test-first)

**Files:** `rust/sim/src/state.rs`.

- [ ] **Step 1 (test) — fire→fly→explode integration:** hand-built grounded/visible
      worm with fan in slot 0, real consts. A `Fire` tick: `ammo`↓, `delay_left`=delay,
      `rng` moved, **one** wobject present, wobject `pos` == spawn pos (did **not**
      move its birth tick — proves object-loop-before-worms). Next ticks: wobject `pos`
      advances by `vel`, `time_left`↓. The explode tick: wobject gone (pool empty),
      worm hash stable. Assert `cycles==0` throughout.
- [ ] **Step 2 (test) — empty-input equivalence:** under empty input (no Fire), the
      component hashes equal Slice 3's (no wobjects spawn; object loops are no-ops) —
      a guard the driver rename didn't perturb worms-only behaviour.
- [ ] **Step 3 (impl):** Rename `process_worms`→`process_frame`. Per tick:
      `sobjects` loop (no-op), **`wobjects` loop** (slot-order walk: copy out, run
      `wobject_process`, write back on `Keep` / `blow_up`+`free` on `Explode`/`Remove`),
      `nobjects` loop (no-op), `bobjects` loop (no-op), then the **worms loop**
      (Slice-3 pass + Fire gate between `process_weapons` and `process_physics`).
      Destructure `SimState` to borrow `wobjects`/`rand`/`weapons`/`cossin` alongside
      `level`/`physics`/`control`/`worms` (extend the Slice-3 pattern).
- [ ] **Step 4 (impl):** Update the Slice-2/3 test call sites `process_worms`→
      `process_frame` (mechanical).
- [ ] **Verify:** `cargo test -p sim` green; `cargo test -p oracle-tests sim_slice2
      sim_slice3` still green.

---

### Task 4: C++ dumper extension (object loops + `weapon` directive)

**Files:** `src/tools/oracle_dump/sim_physics_dump.cpp`.

- [ ] **Step 1 (impl) — object loops:** In the per-tick loop, **before** the worm
      loop, add the `sobjects`/`wobjects`/`nobjects` `All()`-`Process` loops and the
      `bobjects` `Begin/End` process/free loop, in `game.cpp:333-355` order. **No**
      `++cycles`, **no** bonus-drop roll, **no** ninjarope. Update the file's header
      comment (it now drives a ProcessFrame *subset*, not worms-only).
- [ ] **Step 2 (impl) — `weapon` directive:** Parse `weapon <slot> <name>` into the
      `Scenario` (a per-slot weapon name). After `InitWeapons`/`ResetWorms`, for each
      worm set `w->weapons[slot].type = &common->weapons[resolve(name)]` and `.ammo =
      type->ammo` (resolve name→index via `common`). Keep `current_weapon=0`.
- [ ] **Step 3 (verify no regression):** Re-run `gen_sim_slice2_golden.sh` and
      `gen_sim_slice3_golden.sh`; `git diff` on `sim_slice2.txt`/`sim_slice3.txt` must
      be **empty** (object loops no-op on empty pools, no cycles/bonus change). If not,
      the extension perturbed the prior proof — stop and fix.
- [ ] **Verify:** dumper builds under `-DOPENLIERO_BUILD_ORACLE_DUMP=ON`; slice-2/3
      goldens unchanged.

---

### Task 5: scenario parser `weapon` directive (Rust) (test-first)

**Files:** `rust/oracle-tests/src/scenario.rs`.

- [ ] **Step 1 (test):** `Scenario::parse` accepts `weapon <slot> <name>` and exposes
      the per-slot loadout (e.g. `Scenario::weapon(slot) -> Option<&str>`); a malformed
      slot or duplicate errors with the 1-based line number (mirror the existing
      directive error style).
- [ ] **Step 2 (impl):** Add the directive + accessor; existing slice-2/3 scenarios
      (no `weapon` line) parse unchanged.
- [ ] **Verify:** `cargo test -p oracle-tests scenario` green.

---

### Task 6: scenario file + `gen_sim_slice4a_golden.sh` + committed golden

**Files:** `rust/oracle-tests/golden/sim_slice4a_scenario.txt`,
`rust/oracle-tests/gen_sim_slice4a_golden.sh`,
`rust/oracle-tests/golden/sim_slice4a.txt`.

- [ ] **Step 1:** Create `sim_slice4a_scenario.txt`: `seed 42`, `level
      Levels/physics_fall_test.lev`, `ticks ≈ 80`, `weapon 0 fan`, two visible worms
      (worm 1 invisible or far so it is never hit). `input` lines: worm 0 aims then
      sets `Fire` for a few ticks (one shot times out in open sky; one aimed into the
      floor to hit `expl_ground`); worm 1 a Fire-free / divergent pattern.
      **Constraints (comment them):** health 100; never Left(4)+Right(8) together;
      no shot within `detect_distance` of a *visible* worm; non-firing worm invisible.
- [ ] **Step 2:** Create `gen_sim_slice4a_golden.sh` (copy of
      `gen_sim_slice3_golden.sh`): `set -euo pipefail`, `PRESET=${PRESET:-macos-arm64}`,
      configure `-DOPENLIERO_BUILD_ORACLE_DUMP=ON`, build `oracle_dump_sim_physics`,
      run from ROOT with the slice-4a scenario + output. Mark LOCAL/MANUAL. `chmod +x`.
- [ ] **Step 3:** Run it; commit `sim_slice4a.txt`. Inspect: the `level` column is
      **constant** on every line (no terrain touched — proves no accidental
      DrawDirtEffect/dig); `rng` is `00000000` until the first fire tick, then moves;
      the `wobjects` column is non-`00000001` (non-empty) during flight; `worm0`/
      `worm1`/master change across phases. If `level` ever moves, a shot hit dirt with
      a dirt-effect weapon or a dig fired — fix the scenario/weapon.
- [ ] **Verify:** `sim_slice4a.txt` has `ticks+1` lines, 11 columns; `level` constant;
      `rng` moves; `wobjects` non-empty for ≥1 tick.

---

### Task 7: Rust differential test `sim_slice4a_golden` (test-first against golden)

**Files:** `rust/oracle-tests/tests/sim_slice4a_golden.rs`.

- [ ] **Step 1:** Mirror `sim_slice3_golden.rs` setup: parse the scenario; load the
      `.lev`, `TcConfig` (materials + `PhysicsConsts` + `ControlConsts`) and the
      `Objects` weapon table; resolve `weap_order`; build worm inits with the `weapon
      0 fan` override (`WeaponInit { ty: Some(fan_id), ammo }` in slot 0,
      `current_weapon=0`); build `SimState::new(... weapons, cossin ...)`.
      `parse_golden` keeps all columns incl. `state_hash`.
- [ ] **Step 2:** Assert tick-0 (master + 9 components) against the fresh state. For
      `k` in `1..=ticks`: `process_frame([unpack(scn.input(k-1,0)), unpack(scn.input(
      k-1,1))])` (**input keyed `k-1`**) and assert master + all 9 components against
      golden line `k`. Assert **components first** (rng → level → worm0 → worm1 →
      pools → wobjects) then master, so a divergence localises before the master fires.
- [ ] **Step 3 (coverage guard):** across the run assert `wobjects` is non-empty for
      ≥1 tick, and `rng`, some worm's weapon `ammo`, and its `delay_left` each take
      ≥2 distinct values — so the golden actually exercises Fire, not just flight. Also
      assert the `level` component column is constant (the pristine-level guard).
- [ ] **Verify:** `cargo test -p oracle-tests sim_slice4a` green.

---

### Task 8: wire-up review + done-check

- [ ] **Step 1:** `cargo test --workspace` green; `sim` has no Bevy / no float / no
      deps beyond `sim-core` + `assets`.
- [ ] **Step 2:** Re-read `worm.cpp:1099-1148` + `336-340`, `weapon.cpp:16-76` +
      `127-338` + `78-125`, `game.cpp:333-355` against `weapon.rs` + the driver:
      Fire RNG order, recoil, the single non-laser Process pass, ground-collision
      explode, free-during-iteration, and the object-loop-before-worms order must
      match exactly (note in the PR).
- [ ] **Step 3:** Confirm `level` constant + `rng` moves only at fire ticks in
      `sim_slice4a.txt`; confirm scenario never sets L+R or hits a visible worm.
- [ ] **Step 4:** Confirm the **only** C++ change is the oracle-gated dumper (object
      loops + `weapon` directive) and that slice-2/3 goldens are byte-identical ⇒
      `test_determinism`/`test_rollback_*` unaffected (note in PR; no need to run).
- [ ] **Step 5:** Update the Step-2 overview *Slice ordering* + the Slice-4 overview
      (mark 4a done, RNG live, level still pristine) (docs only). Don't commit
      unrelated changes.
- [ ] **Definition of done:** every checkbox in the 4a design's *Definition of done*
      is satisfied.

## Notes for the implementer

- **RNG order is the whole game.** Spread-x, spread-y, colour, time-var — in that
  order, each only when its guard fires. Build the master golden test early and lean
  on the `rng` + `wobjects` component columns to localise before the master fires.
- **Object loops run before worms; Fire spawns after.** The shot must not move its
  birth tick. This is the single most likely off-by-one.
- **fan is "explode into nothing" on purpose.** Don't port DrawDirtEffect /
  create_on_exp / splinters here — guard them with `debug_assert!`/TODO pointing at
  4b/4c. Keeping the level pristine is a *feature* of 4a (one fewer new invariant).
- **`DirtRock`/`Inside` ≠ `CheckedMatWrap`.** `WObject::Process` uses a real range
  check plus `PixelMat().DirtRock()`, not the wrapping worm-physics probe. Port the
  right one; audit the `DirtRock` bits in `material.hpp`.
- **Truncating division** (`/100`, `*100/speed`, `*recoil/100`): Rust `/`, never
  `>>`. Same discipline as Slices 2–3.
- **Don't touch sim-critical C++.** The only C++ edit is the oracle-gated dumper;
  re-diff slice-2/3 goldens to prove it.
- **Pool full semantics (O3) are out of scope** — assert `spawn` returns `Some`;
  document the `NewObjectReuse` overwrite-on-full divergence on `Pool` for Slice 6.
