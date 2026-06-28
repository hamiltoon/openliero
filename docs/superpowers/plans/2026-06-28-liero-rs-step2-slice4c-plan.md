# Step 2, Slice 4c — Explosion objects (`SObject`/`NObject`): Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking. **Test-first**: write the failing
> test (or its assertion) before the implementation it pins.

**Goal:** Port `BlowUpObject`'s `create_on_exp` branch → **`SObjectType::Create`** +
**`SObject::Process`** + the **`NObject`** family (`Create`/`Create1`/`Create2`/
`Process`) for **`dart` → `small_explosion`** into the `sim` crate, so the Rust sim
reproduces the C++ master `HashGameState` **and** all component hashes
**tick-for-tick** — including the **`sobjects` and `nobjects` pools going non-empty +
hashed for the first time** and the **carving `DrawDirtEffect`** (texture 2) moving
the `level` hash. This is the slice where the explosion-object RNG cluster (sound /
dirt-throw / dirt-debris `Create2`) goes live. Worm-damage/blood is **deferred**
(worms kept out of range — O10); the splinter path is **deferred** (dart spawns none
— O9).

**Architecture:** Extend `rust/sim/` (no new crate; deps unchanged: `sim-core`,
`assets`; Bevy-free, float-free). New `SObject` struct + `Pool<SObject>`; `NObject`
gains `owner_idx`/`time_left`; `SimState` carries `sobject_types`/`nobject_types`
(`cossin`/`large_sprites`/`textures` already present from 4a/4b). New `sobject.rs`
(`sobject_create`, `sobject_process`) and `nobject.rs` (`nobject_create*`,
`nobject_process`); `blow_up` (4a) gains the `create_on_exp>=0 ⇒ sobject_create`
branch; `draw_dirt_effect` (4b) is **reused verbatim**. The `process_frame` driver's
`sobjects`/`nobjects` loops (no-ops since 4a) go live. **No C++ dumper change** — the
object loops + `shadow=false` are already in place; `SObject::Create` is real game
code reached when a dart explodes.

**Tech stack:** Rust only (`sim` extend, `oracle-tests`). Golden regenerated locally
via the **unchanged** dumper (`OPENLIERO_BUILD_ORACLE_DUMP`); CI (`cargo test
--workspace`) runs the committed golden. `data/TC/openliero` real TC; weapon **dart**,
sobject **small_explosion**, dirt-debris nobject **particle__disappearing** (idx 2).

## Global constraints

- **Bit-exact vs C++.** Sources of truth: `weapon.cpp:78-125` (`BlowUpObject`, the
  `create_on_exp` branch `:89-92`), `sobject.cpp:16-228` (`SObjectType::Create`) +
  `:230-241` (`SObject::Process`), `nobject.cpp:7-66` (`Create`/`Create1`/`Create2`) +
  `:68-234` (`NObject::Process`), `game.cpp:333-355` (object-loop order) + `:567-589`
  (`DoDamage` — no rand in normal mode), `gfx/blit.cpp:534-622` (`draw_dirt_effect`,
  reused from 4b), `stateHash.hpp:72-110` (master folds) + `:179-210` (component
  folds), `exactObjectList.hpp:36-94` (pool spawn/free/iterate), `tc.cfg:5/6/148-153`
  (object lists + texture 2).
- **RNG order is the contract** (design, *RNG audit*). At the explode tick, in order:
  (1) sound `rand(num_sounds)=rand(2)`; (2) worm loop — **no draw** (worms out of
  range, O10); (3) dirt-throw row-major over `Rect(x-4,y-4,x+5,y+5)∩Bounds()`: per
  `AnyDirt` cell `rand(8)` (short-circuit `AnyDirt && rand(8)`), on `0` → `rand(128)`
  + `Create2`[`rand(speed_v)` **first**, then `rand(distribution*2)`×2]; (4)
  `draw_dirt_effect` `rand(tex.r_frame)=rand(2)` **first, before any pixel**. Thread
  the one `sim-core::Rand`; never pull ad hoc. **The dirt-throw reads pre-crater
  terrain — scan before carving.**
- **dart Fire = 0 rand.** `distribution=0`, `shotType=1` (deterministic `cur_frame`),
  `timeToExploV=0`, `leaveShells=0`, `wormCollide=false` ⇒ no Fire/Process RNG; the
  `rng` column must **not** move at the dart fire tick (a sharp assertion).
- **Level goes live via carving** (texture 2, `n_draw_back=true`) — **reuse 4b's
  `draw_dirt_effect`**; 4c is its first live exercise.
- **Worms inert (O10).** All worms outside every explosion's `±detect_range=8`px box ⇒
  no `DoDamage`, no blow-away, no blood, **no `bobjects`** (avoids the `cycles=0`
  blood-trail storm). Assert worm fields follow the no-explosion trajectory.
- **Splinters deferred (O9).** Port `Create1`/the nobject explode-splinter code,
  guard the un-exercised draws (`debug_assert!`/TODO → bazooka follow-up).
- **`cycles` stays 0.** No `++cycles`, no bonus roll, no ninjarope.
- **Pools.** `Pool<SObject>` cap 700, `Pool<NObject>` cap 600; `spawn` asserts `Some`
  (O3 deferred); keep the shot count low so `nobjects < 600`.
- **Only `material_id` + the pool folds are hashed.** No `sound`, `screen_flash`,
  `shake`, stats, `has_hit`/`fired_by`, `materials` cache, `display_valid`, dirty list.
- **Truncating division / shifts.** `Ftoi`=`>>16` (arithmetic), `Itof`=`<<16`; `/100`,
  `/3` are Rust `/`, never `>>`. Same discipline as 4a/4b.
- **Scenario is the single source of truth**, read by both the (unchanged) dumper and
  the Rust test. Golden regen LOCAL/MANUAL; `PRESET` defaults `macos-arm64`.
- **No AI / "Generated with" taglines.** **Bash discipline:** one command per call; no
  `>>`/heredoc/`&&`/`;`/`$VAR` chaining; create files with the editor.

## File structure

- `rust/sim/src/state.rs` — MODIFY: add `SObject` + `Pool<SObject>` to `SimState`;
  `NObject` gains `owner_idx`/`time_left`; `SimState` carries
  `sobject_types`/`nobject_types`; `SimState::new` updated; the `process_frame`
  `sobjects`/`nobjects` loops go live (thread the new args).
- `rust/sim/src/sobject.rs` — NEW: `sobject_create`, `sobject_process`.
  `pub mod sobject;` in `lib.rs`.
- `rust/sim/src/nobject.rs` — NEW: `nobject_create`/`create1`/`create2`,
  `nobject_process`. `pub mod nobject;` in `lib.rs`.
- `rust/sim/src/weapon.rs` — MODIFY: `blow_up` gains `create_on_exp>=0 ⇒
  sobject_create`.
- `rust/sim/src/lib.rs` — MODIFY: `pub mod sobject; pub mod nobject;`.
- `rust/oracle-tests/golden/sim_slice4c_scenario.txt` — NEW.
- `rust/oracle-tests/gen_sim_slice4c_golden.sh` — NEW (LOCAL/MANUAL).
- `rust/oracle-tests/golden/sim_slice4c.txt` — NEW (committed).
- `rust/oracle-tests/tests/sim_slice4c_golden.rs` — NEW.
- **No C++ change.** (`sim_physics_dump.cpp` already has the object loops +
  `shadow=false`.)

---

### Task 0: datamodel — `SObject` + `Pool<SObject>`, `NObject` fields, `SimState` tables

De-risk the shapes before behaviour.

**Files:** `rust/sim/src/state.rs`.

- [ ] **Step 1 (test):** `SObject { id: i32, x: i32, y: i32, cur_frame: i32,
      anim_delay: i32 }` exists (Copy); a `Pool<SObject>` (cap 700) spawns lowest-free
      / iterates slot-order (reuse the `wobjects` pool tests). `NObject` has
      `owner_idx: i32` + `time_left: i32` (defaults 0); existing nobject hash tests
      unaffected (neither hashed).
- [ ] **Step 2 (impl):** add the struct + pool + fields.
- [ ] **Step 3 (test) — hash folds (first non-empty):** with a hand-built `sobjects`
      pool, the master fold = `…*31 + id; …*31 + cur_frame` (`stateHash.hpp:76-77`) and
      the component fold matches (`:184-185`). With a hand-built `nobjects` pool, the
      **master** fold = `pos.x,pos.y,vel.x,vel.y,cur_frame,type_id` (`:85-92`) and the
      **component** fold = `pos.x,pos.y` only (`:195-196`). Pin both — these were never
      exercised before (validates the Slice-1 `hash.rs` folds).
- [ ] **Step 4 (test+impl) — tables carried:** `SimState` carries
      `sobject_types: Vec<SObjectType>` + `nobject_types: Vec<NObjectType>`;
      `SimState::new` takes them (`cossin`/`large_sprites`/`textures` already present).
      Unit-test `sobject_types[2]` (small_explosion: `start_sound>=0`, `num_sounds=2`,
      `damage=5`, `detect_range=8`, `blow_away=3000`, `dirt_effect=2`, `anim_delay=2`,
      `num_frames=5`) and `nobject_types[2]` (particle__disappearing: `speed=80`,
      `speed_v=40`, `distribution=10000`, `gravity=700`, `expl_ground=true`,
      `start_frame=0`, `num_frames=0`, `bounce=0`). Update all `SimState::new` call
      sites (slice-2/3/4a/4b tests).
- [ ] **Verify:** `cargo test -p sim` green; `cargo test -p oracle-tests sim_slice1
      sim_slice2 sim_slice3 sim_slice4a sim_slice4b` still green.

---

### Task 1: `nobject_create` / `create1` / `create2` (test-first)

**Files:** `rust/sim/src/nobject.rs`.

Source: `nobject.cpp:7-66`. The dirt-throw uses **`create2`**; port the whole family
(`create1` + the `start_frame>0` `create` rand are needed for the splinter path — O9 —
so port now, guard the un-exercised draws with coverage notes).

- [ ] **Step 1 (test) — `create2` RNG order (dirt-debris constants):** with a seeded
      `Rand`, `cossin`, and `nobject_types[2]`, `create2(angle, vel=0, pos=Itof(cell),
      color=kPix, …)` draws **in this order**: `rand(speed_v)=rand(40)` (kRealSpeed,
      **first**, `:53`), then `distribution=10000>0` ⇒ `rand(20000)` (x), `rand(20000)`
      (y) (`:59-60`); then `create`: `start_frame=0` & `color=kPix≠0` ⇒ `cur_frame=kPix`
      (**no rand**, `:26-27`), `time_to_explo_v=0` ⇒ no rand; finally `obj.pos +=
      obj.vel` (`:65`). Hand-compute `vel = cossin[angle]*kRealSpeed/100 + (rand- dist,
      rand- dist)` and `pos`. Assert exactly 3 draws.
- [ ] **Step 2 (test) — `create` cur_frame branch:** `start_frame>0 ⇒ rand(num_frames
      +1)`; `start_frame<=0 & color≠0 ⇒ color`; `& color==0 ⇒ color_bullets`
      (`:24-30`); `time_to_explo_v>0 ⇒ rand` (`:34-36`). Pin each with a synthetic type.
- [ ] **Step 3 (test) — `create1` RNG order:** `distribution>0 ⇒ rand(distribution*2)`
      ×2 (`:44-45`, **no speed draw** — distinct from `create2`), then `create`. Pin the
      contrast with `create2` (which draws `speed_v` first).
- [ ] **Step 4 (impl):** port the three functions verbatim, C++ statement order; each
      spawns via `Pool::spawn` (assert `Some`).
- [ ] **Verify:** `cargo test -p sim` green.

---

### Task 2: `nobject_process` (test-first)

**Files:** `rust/sim/src/nobject.rs`.

Source: `nobject.cpp:68-234`. Returns an outcome `{ Keep | Explode | Remove }` (the
driver frees on Explode/Remove). The dirt-debris path draws **no rand**.

- [ ] **Step 1 (test) — move + gravity + boundary:** `pos += vel` (`:74`); air ⇒
      `vel.y += gravity=700` (`:140`); boundary clamp (`:100-113`) past each edge.
- [ ] **Step 2 (test) — ground explode:** `!inside(inew) || dirt_rock(inew)` ⇒
      `vel.Zero()`; `expl_ground ⇒ Explode` (`:115-131`). Pin with a synthetic floor;
      `particle__disappearing` `expl_ground=true`, `draw_on_map=false`,
      `start_frame=0` ⇒ no `BlitImageOnMap` (`:119` guard false). On Explode:
      `create_on_exp=-1`/`dirt_effect=-1`/`splinter_amount=0` ⇒ just free (no rand).
- [ ] **Step 3 (test) — bounce (guarded):** `bounce=0` ⇒ skipped, no change; pin a
      synthetic `bounce>0` type for the `x`/`y` reflect (`:81-93`) so the branch is
      covered.
- [ ] **Step 4 (test) — inert guarded branches:** `blood_trail=false`,
      `num_frames=0` (no anim), `time_to_explo=0` (no timeout), `hit_damage=0` (no
      worm-hit) ⇒ no rand, no extra state; assert `rand.last` unchanged across a
      dirt-debris Process. **Note the `cycles=0` trap** in a comment: a `blood_trail`
      type would spawn a BObject every tick — guard with `debug_assert!`/TODO (O10).
- [ ] **Step 5 (test) — explode side-effects guarded (O9):** the `create_on_exp`/
      `dirt_effect`/splinter arms (`:206-228`) are ported; pin the splinter arm with a
      synthetic `splinter_amount>0` type (`rand(128)`+`rand(2)`+`create2`) so the code
      is covered, but assert dirt-debris hits none.
- [ ] **Step 6 (impl):** port `nobject_process` (move, bounce-guarded, blood-trail-
      guarded, clamp, ground/expl_ground, gravity, anim-guarded, timeout-guarded,
      worm-hit-guarded, explode-arms-guarded). Reuse 4a's `inside`/`dirt_rock`,
      4b's `draw_dirt_effect` for the `dirt_effect` arm.
- [ ] **Verify:** `cargo test -p sim` green.

---

### Task 3: `sobject_create` + `sobject_process` (test-first) — the core port

**Files:** `rust/sim/src/sobject.rs`.

Source: `sobject.cpp:16-228` (`Create`) + `:230-241` (`Process`). `sobject_create`
spawns the sobject, runs the explosion, draws the cluster, spawns dirt-debris, carves.

- [ ] **Step 1 (test) — sound + obj init:** with a seeded `Rand` and
      `sobject_types[2]`, `sobject_create(x, y, …)` spawns a sobject with `id=2`,
      `x=x-8`, `y=y-8`, `cur_frame=0`, `anim_delay=2` (`:35-39`); `start_sound>=0` ⇒
      one `rand(num_sounds)=rand(2)` drawn **first** (`:23-25`); `screen_flash`/`shake`
      not modeled. Assert the sound draw is consumed.
- [ ] **Step 2 (test) — worm loop inert (O10):** with all worms outside the
      `±detect_range=8`px box, the per-worm loop (`:48-114`) draws **nothing** and
      mutates no worm. Pin a worm just outside (±9) and just inside (±7, synthetic) to
      cover the box test, but the 4c scenario keeps all worms outside.
- [ ] **Step 3 (test) — dirt-throw RNG + spawn order:** with a synthetic level whose
      `9×9` box around the impact has a **known** `AnyDirt` pattern, assert: the loop is
      **row-major `y` then `x`** over `Rect(x-4,y-4,x+5,y+5)∩Bounds()` (`:191-196`);
      `rand(8)` drawn **only** for `AnyDirt` cells (short-circuit, `:197`); on `0`,
      `kPix=Pixel(x,y)` then `rand(128)` (`:199`) then `nobject_types[2].create2`
      (`:200`) — assert each spawned debris carries `cur_frame=kPix` and the `create2`
      draws follow (`speed_v`, dist×2). Hand-build a pattern with a fixed seed so the
      `rand(8)==0` hits are deterministic; assert the **exact** total draw count.
- [ ] **Step 4 (test) — carving DrawDirtEffect reused (live):** after the dirt-throw,
      `dirt_effect=2 ⇒ draw_dirt_effect(level, …, 2, x-7, y-7, rand)` (`:209-210`) draws
      one `rand(2)` and carves (`n_draw_back=true`, texture 2) — assert `material_id`
      changes over the `AnyDirt` cells in the window and the `rand(2)` is the **last**
      cluster draw. `CorrectShadow` skipped (shadow off). **The dirt-throw must read
      pre-carve terrain** — assert a debris was spawned from a cell that the carve then
      cleared.
- [ ] **Step 5 (test) — `sobject_process`:** `--anim_delay<=0 ⇒ anim_delay=2,
      ++cur_frame, free when cur_frame>num_frames=5` (`:234-240`); no rand. Pin the
      ~12-tick lifetime (`anim_delay=2`, frames 0..5).
- [ ] **Step 6 (impl):** port `sobject_create` whole in C++ order (sound, obj init,
      damage block [worm loop guarded, wobject/nobject blow-away loops, dirt-throw],
      `dirt_effect` reuse; bonus loop skipped; `CorrectShadow` skipped) and
      `sobject_process`. Guard the worm-damage/blood arms (O10) and the bonus recursion
      with `debug_assert!`/TODO.
- [ ] **Verify:** `cargo test -p sim` green.

---

### Task 4: `blow_up` — add the `create_on_exp` branch (test-first)

**Files:** `rust/sim/src/weapon.rs`.

Source: `weapon.cpp:89-92`. Extend the 4a/4b `blow_up`: after `wobjects.Free(this)`
(4a) and **before** `explo_sound`/splinters/`dirt_effect` (the order is load-bearing),
add `if w.create_on_exp >= 0 { sobject_create(sobject_types[create_on_exp], Ftoi(kX),
Ftoi(kY), cause_idx, …) }`.

- [ ] **Step 1 (test) — dart explode spawns the explosion:** a wobject (dart) at a
      known `pos` over dirt, `blow_up` ⇒ (a) the dart slot is freed (4a behaviour); (b)
      a sobject `id=2` spawned at `(Ftoi(x)-8, Ftoi(y)-8)`; (c) dirt-debris spawned per
      the dirt-throw; (d) `material_id` carved; (e) `rand` advanced by exactly the
      cluster (sound + dirt-throw + `rand(2)`) — `dart` `splinterAmount=0`/`dirtEffect=
      -1` ⇒ no wobject splinters / no wobject dig. Assert the `Ftoi(x),Ftoi(y)` passed
      to `sobject_create` (not `-7`/`-8`; the offsets are applied inside Create /
      DrawDirtEffect).
- [ ] **Step 2 (test) — greenball/fan still inert:** `blow_up` with greenball
      (`create_on_exp=-1`) ⇒ no sobject (4b path unchanged); fan likewise (4a) — a
      regression guard.
- [ ] **Step 3 (impl):** add the branch; thread `sobject_types`/`nobject_types`/
      `cossin`/`large_sprites`/`textures` through the `blow_up` signature (and from the
      driver). Keep the wobject-splinter branch guarded (O9).
- [ ] **Verify:** `cargo test -p sim` green; `cargo test -p oracle-tests sim_slice4a
      sim_slice4b` still green.

---

### Task 5: driver — `sobjects`/`nobjects` loops go live (test-first)

**Files:** `rust/sim/src/state.rs`.

`process_frame` already runs all four object loops (4a; `sobjects`/`nobjects` were
no-ops). 4c makes them live + threads the new args, and preserves the cross-pool
spawn ordering.

- [ ] **Step 1 (test) — fire→fly→explode→objects integration:** a worm with dart in
      slot 0, real consts, aimed into the floor, **worms out of explosion range**. The
      **fire tick**: `rng` **unchanged** (dart Fire = 0 rand), `ammo`↓, `delay_left=30`,
      one wobject at spawn pos (didn't move its birth tick). Flight: wobject arcs under
      `gravity=200`. The **explode tick**: wobject gone; **a sobject (`id=2`) present**
      (and **not** processed its birth tick — `cur_frame=0`); **dirt-debris present**
      (and **processed** its birth tick — it moved by `vel`, proving nobjects-loop-runs-
      after-wobjects-loop); `rng` advanced by the cluster; `level` carved. Later ticks:
      sobject `cur_frame` 0→5 then frees (~12 ticks); debris falls + frees. Assert
      `cycles==0` and `bobjects` empty throughout.
- [ ] **Step 2 (test) — pristine-when-no-explode:** under no-Fire input, `sobjects`/
      `nobjects`/`level` stay empty/constant (regression: wiring added no spurious
      spawn/write).
- [ ] **Step 3 (impl):** in the `sobjects` loop call `sobject_process` (free on
      expiry); in the `nobjects` loop call `nobject_process` (free on Explode/Remove);
      thread `sobject_types`/`nobject_types`/`cossin`/`large_sprites`/`textures` from
      `SimState` into the wobjects-loop `blow_up` and the nobjects loop. Ensure the
      `nobjects` iteration is captured **after** the `wobjects` loop so birth-tick
      debris is processed; the `sobjects` loop (already passed) leaves the new sobject
      for next tick.
- [ ] **Verify:** `cargo test -p sim` green; `cargo test -p oracle-tests sim_slice2
      sim_slice3 sim_slice4a sim_slice4b` still green.

---

### Task 6: scenario file + `gen_sim_slice4c_golden.sh` + committed golden

**Files:** `rust/oracle-tests/golden/sim_slice4c_scenario.txt`,
`rust/oracle-tests/gen_sim_slice4c_golden.sh`,
`rust/oracle-tests/golden/sim_slice4c.txt`.

- [ ] **Step 1:** Create `sim_slice4c_scenario.txt`: `seed 42`, `level
      Levels/physics_fall_test.lev`, `ticks ≈ 90`, `weapon 0 dart`, two worms (worm 1
      invisible/far). `input`: worm 0 aims toward the floor and Fires so the dart arcs
      into the **dirt surface** and explodes (dirt/rock contact) with the dirt-throw
      box overlapping `AnyDirt` cells; optionally a second dart so both pools + `level`
      move twice. Worm 1 a Fire-free/divergent pattern.
      **Constraints (comment them):** all worms outside every explosion's
      `±detect_range=8`px box (O10 — no damage/blood/`bobjects`); impact enters dirt so
      `nobjects` actually spawn **and** the wobject explodes; keep `nobjects < 600`
      (O3); health 100; never Left(4)+Right(8) together; non-firing worm invisible.
- [ ] **Step 2:** Create `gen_sim_slice4c_golden.sh` (copy of `gen_sim_slice4b_
      golden.sh`): `set -euo pipefail`, `PRESET=${PRESET:-macos-arm64}`, configure
      `-DOPENLIERO_BUILD_ORACLE_DUMP=ON`, build `oracle_dump_sim_physics`, run from ROOT
      with the slice-4c scenario + output. Mark LOCAL/MANUAL. `chmod +x`. **(No dumper
      edit — the binary is unchanged from 4b.)**
- [ ] **Step 3:** Run it; commit `sim_slice4c.txt`. Inspect: `rng` **flat at the dart
      fire tick** (Fire = 0 rand), then **jumps at the explode tick** (the cluster);
      `sobjects` non-empty (`id=2`) for ~12 ticks after explode; `nobjects` non-empty
      during debris flight; `level` constant until the explode tick then **changes**
      (carving); `bobjects` empty throughout; worm columns on the no-explosion path. If
      `nobjects` never moves, the impact missed dirt (no `AnyDirt` in the box) — fix the
      aim. If `bobjects` moves or a worm column deviates, a worm was in range — move it.
- [ ] **Verify:** `sim_slice4c.txt` has `ticks+1` lines, 11 columns; `sobjects`/
      `nobjects` go non-empty; `level` changes; `rng` flat at fire, moves at explode;
      `bobjects` empty.

---

### Task 7: Rust differential test `sim_slice4c_golden` (test-first against golden)

**Files:** `rust/oracle-tests/tests/sim_slice4c_golden.rs`.

- [ ] **Step 1:** Mirror `sim_slice4b_golden.rs` setup: parse the scenario; load the
      `.lev`, `TcConfig` (materials + `PhysicsConsts` + `ControlConsts`), the weapon
      table, the large-sprite bank, the textures table, **and the `sobject_types` +
      `nobject_types` tables**; resolve `weap_order`; build worm inits with `weapon 0
      dart` (`WeaponInit { ty: Some(dart_id), ammo }`, `current_weapon=0`); build
      `SimState::new(… weapons, cossin, large_sprites, textures, sobject_types,
      nobject_types …)`. `parse_golden` keeps all columns incl. `state_hash`.
- [ ] **Step 2:** Assert tick-0 (master + 9 components) against the fresh state. For
      `k` in `1..=ticks`: `process_frame([unpack(scn.input(k-1,0)), unpack(scn.input(
      k-1,1))])` (**input keyed `k-1`**) and assert master + all 9 components against
      golden line `k`. Assert **components first** (rng → level → worm0 → worm1 →
      bobjects → bonuses → sobjects → nobjects → wobjects) then master, so a divergence
      localises before the master fires. **Note (O11):** an `nobjects`-column match does
      **not** prove nobject `vel`/`cur_frame` — those localise via the master only.
- [ ] **Step 3 (coverage guard):** across the run assert `sobjects` non-empty for ≥1
      tick (and folds `id=2`), `nobjects` non-empty for ≥1 tick, `level` changes ≥1
      time, `rng` is **unchanged at the dart fire tick** and **moves at the explode
      tick**, `bobjects` stays empty, worm `health`/`vel`/`pos` follow the no-explosion
      path, and `nobjects` max < 600 (O3 guard).
- [ ] **Verify:** `cargo test -p oracle-tests sim_slice4c` green.

---

### Task 8: wire-up review + done-check

- [ ] **Step 1:** `cargo test --workspace` green; `sim` has no Bevy / no float / no
      deps beyond `sim-core` + `assets`.
- [ ] **Step 2:** Re-read `weapon.cpp:89-92`, `sobject.cpp:16-241`, `nobject.cpp:7-234`,
      `game.cpp:333-355`, `stateHash.hpp:72-110/179-210` against `sobject.rs`/
      `nobject.rs`/`blow_up`/the driver: the cluster RNG order (sound → dirt-throw
      `rand(8)`/`rand(128)`/`Create2` → `draw_dirt_effect` `rand(2)`), the **`Create2`
      speed-first** order, the row-major short-circuited dirt-throw on **pre-carve**
      terrain, the cross-pool spawn ordering (sobject not processed birth tick, nobject
      **is**), and free-during-iteration must match exactly (note in the PR).
- [ ] **Step 3:** Confirm in `sim_slice4c.txt`: `rng` flat at fire / moves at explode;
      `sobjects`/`nobjects` non-empty; `level` carved; `bobjects` empty; worms on the
      no-explosion path; `nobjects < 600`.
- [ ] **Step 4:** Confirm **no C++ change** in 4c ⇒ `test_determinism`/`test_rollback_*`
      unaffected; slice-1/2/3/4a/4b goldens **byte-identical** (re-diff; pure-Rust slice)
      — note in PR.
- [ ] **Step 5:** Update the Step-2 overview *Slice ordering* + the Slice-4 overview
      (mark 4c done; `sobjects`/`nobjects` live; carving `DrawDirtEffect` live;
      O5 confirmed; O9/O10/O11 recorded) (docs only). Don't commit unrelated changes.
- [ ] **Definition of done:** every checkbox in the 4c design's *Definition of done*
      is satisfied.

## Notes for the implementer

- **The cluster RNG order is the whole game.** sound `rand(2)` → dirt-throw (per
  `AnyDirt` cell `rand(8)`, on `0` → `rand(128)` + `Create2`[`speed_v` first, then
  dist×2]) → `draw_dirt_effect` `rand(2)`. Build the master golden early and lean on
  the `rng` + `sobjects` + `nobjects(pos)` columns to localise; remember the
  `nobjects` column omits `vel`/`cur_frame` (O11) — master-only there.
- **`Create2` draws `speed_v` first, then distribution.** `Create1` draws distribution
  only (no speed). Don't conflate them.
- **Dirt-throw reads pre-carve terrain; carve last.** Scan `AnyDirt` and spawn debris
  **before** `draw_dirt_effect` writes `material_id`. Order matters for both the
  `rand(8)` count and the spawned `cur_frame=kPix`.
- **dart Fire draws 0 rand** — the `rng` column must be flat at the fire tick. If it
  moves, the dart picked up a spread/colour/time-var draw it shouldn't (check the
  `shotType=1` deterministic `cur_frame` path, `weapon.cpp:50-66`).
- **Cross-pool spawn ordering.** The explosion (in the wobjects loop) spawns a sobject
  (sobjects loop already ran ⇒ first anim next tick) and dirt debris (nobjects loop
  runs next ⇒ moves this tick). Take the `nobjects` iteration after the wobjects loop.
- **Worms out of range (O10).** Keep every worm outside the `±8`px box. Exercising
  damage drags in `DoDamage` (hashed worm mutation) **and** the `cycles=0` blood-trail
  BObject storm (nobject 6, `blood_trail`+`delay=10` ⇒ a BObject every frozen tick).
- **Carving `DrawDirtEffect` goes live here** — reuse 4b's `draw_dirt_effect` verbatim
  (texture 2, `n_draw_back=true`). A 4b carving bug surfaces in this golden.
- **No C++ change.** Unlike 4a/4b, 4c edits no dumper code — re-diff the prior goldens
  to prove the shared path is untouched.
- **Truncating shifts.** `Ftoi(x)-8` = `(x>>16)-8`; `vel/3`, `*kRealSpeed/100` are
  Rust `/`. Same discipline as 4a/4b.
