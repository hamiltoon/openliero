# Step 4½, Slice 4½c-0 — the unported Step-2 weapon branches: design

Status: **DESIGN** · 2026-09-10 · branch `liero-rs-step-4-5` (base `03de202`: 4½a-1 + 4½b landed)
Part of: `2026-09-10-liero-rs-step4.5-game-shell-overview.md` (the 4½c-0 bullet; cited **overview**)
Origin: `2026-09-10-liero-rs-step4.5-slice4.5a-match-config-and-settings-design.md` §1.3 finding 8 and
§11 Q1 (cited **4½a design**); controller ruling 2026-09-10 (a dedicated slice before 4½c)
Precedents: `2026-06-28-liero-rs-step2-slice4c-explosion-objects-design.md` (port shape),
`plans/2026-09-10-liero-rs-step4.5-slice4.5a1-plan.md` (the settings-driven golden machinery reused here)
Plan: `plans/2026-09-10-liero-rs-step4.5-slice4.5c0-plan.md`

Weapon selection (4½c) makes every TC weapon one menu press away. Step 2 proved the sim bit-exact on
the weapons its fixtures fired and left the rest of `WObject::Process` and its neighbours behind
`debug_assert!` tripwires (or, in one case, a silent no-op). 4½c-0 ports every such branch bit-exact,
with C++ goldens, so 4½c can offer all forty weapons.

---

## 0. Summary of findings (read this first)

1. **It is thirteen weapons, not five.** The overview and the 4½a design name RIFLE, WINCHESTER,
   LASER, GAUSS GUN and MISSILE. Deriving the reached branches from the shipped configs (§2) adds eight
   more: **LARPA, BOUNCY LARPA, CRACKLER** (the particle trail, `weapon.cpp:201-210`, tripwire
   `weapon.rs:355-358`), **MINI NUKE** (splinters via `Create1`, `weapon.cpp:107-114`, tripwire
   `weapon.rs:853-863` — and its `small_nukes` leave a trail), **BIG NUKE, NAPALM, HELLRAIDER** (their
   splinter nobjects leave an sobject trail, `nobject.cpp:133-138`, tripwire `nobject.rs:501-509`) and
   **BOOBY TRAP** (chain explosions, `sobject.cpp:148-150`, tripwire `sobject.rs:314-317`). 4½a-1 made
   HELLRAIDER "bonus-only" (`weap_table = 1`), i.e. *droppable*: its scan simply skipped seeds that
   tripped. The same argument that created 4½c-0 applies to all thirteen, so **4½c-0 covers all
   thirteen** (§9 Q1).
2. **MISSILE is not a panic, it is a silent divergence.** `ProcessSteerables` has no tripwire: the Rust
   worm loop's step 3 is the comment "no-op this slice" (`state.rs:2007`). A fired MISSILE flies
   (its `ST_STEERABLE` flight, `weapon.rs:378-385`, *is* ported — but has never been C++-gated), yet
   Left/Right never turn it and never freeze the worm, in debug and release alike.
3. **Porting `ProcessSteerables` forces the camera deferral closed.** The moment `steerable_count > 0`
   is reachable, the Step-3 guard `debug_assert_eq!(worm.steerable_count, 0)` (`viewport.rs:86`) trips
   in every debug build of `game` and `shot`. 4½c-0 therefore ports the steerable centroid
   (`viewport.cpp:30-32`) and the two `steerable_sum` accumulators — the "real DEFER" of Step 4d
   (PROGRESS §4d) ends here, with a C++ `render_live` golden.
4. **An input-application-point mismatch surfaces with MISSILE.** C++ sets every worm's
   `control_states` *before* `Game::ProcessFrame` (`localController.cpp:58-80`, then `:175`; the replay
   and rollback controllers likewise). Rust applies each worm's input *inside* the worm loop
   (`state.rs:1933-1937`), and so does the dumper's reduced tail (`sim_physics_dump.cpp:1226-1235`) —
   so the object loops read the *previous* tick's input. This was unobservable because no golden ever
   reached an object-loop control read; MISSILE's Up boost (`weapon.cpp:152`) is the first (RemExp,
   `weapon.cpp:139`, the other). Without a fix the live Rust game and C++ `.lrp` replays would lag the
   missile boost by a tick, and the dumper's two paths (reduced tail vs `render_live`, which already
   calls the real `ProcessFrame`) would disagree on a MISSILE scenario. 4½c-0 moves the application
   point to the top of the tick in both Rust and the reduced tail (§4.3, §9 Q2).
5. **Smaller facts.** C++ `WObject::Process` hard-codes `w.id == 28` (`weapon.cpp:337`) — the original
   Liero LASER slot, which *is* LASER in this TC (`tc.cfg:4`, `common.cpp:495` `id = index`).
   `LC(RemExpObject) = 35` (`tc.cfg:83`) makes the RemExp object the **BOOBY TRAP** (index 34); the hack
   is off (`tc.cfg:269`). `scenario::load` never assigns `state.laser_weapon` (it stays 0), so on the
   game/`shot` path the LASER's sight walk (`worm.cpp:1196`) never arms — render-only, fixed here in
   passing (§4.9). Doc nits, not code: the 4½a-1 test comment "indices 2/7/28/32/39" lists GAUSS GUN and
   MISSILE swapped (MISSILE = 32, GAUSS GUN = 39; the *set* is right); the `render_slice3b_laser`
   scenario comment calls RIFLE "shot_type 0" (it is 4 — harmless, that scenario never fires).

---

## 1. Goal / done-when

**Goal.** Every C++ branch a TC weapon can reach is ported bit-exact, so 4½c's weapon menu can offer
all forty weapons and a bonus can hand out any of them.

**Done when (6):**

1. **No weapon-branch tripwire remains.** `debug_assert!(` occurs zero times in
   `rust/sim/src/{weapon,nobject,sobject}.rs` and `rust/render/src/viewport.rs`; the inventory test
   (§2.3) pins which weapon reaches which mechanism.
2. **Four settings-driven sim goldens bit-exact vs C++** — `laser`, `missile`, `trails`, `booby`
   (§5.2), 12 columns incl. `IsGameOver`, each with per-branch *reach witnesses* re-asserted from the
   genuinely driven Rust state (§5.3), `weap_table` all zero and bonuses on (the ban lift, §6).
3. **One `render_live` golden bit-exact vs C++** — the steerable camera over the real
   `Game::ProcessFrame` (frame hash + state hash + the isolation triple), with the camera proven to sit
   on the missile centroid (§5.4).
4. **Every prior golden byte-identical** (the Rust re-diff), and the dumper's input-point edit proven
   hash-neutral by regenerating 21 goldens across every dumper path with a clean `git status` (§5.5).
5. `cargo test --workspace --exclude game` (DEBUG — some tests are `#[should_panic]` on
   `debug_assert!`s), `cargo test -p game`, and the wasm build are green.
6. The overview's 4½c-0 bullet and PROGRESS say "all forty weapons are safe"; 4½c needs no hidden
   weapons.

---

## 2. Inventory

### 2.1 How it was derived

From the C++ source: every branch of `WObject::Process` (`weapon.cpp:127-338`), `Weapon::Fire`
(`:16-76`), `BlowUpObject` (`:78-125`), `NObject::Process` (`nobject.cpp:68-234`), the wobject loop of
`SObjectType::Create` (`sobject.cpp:118-153`), `Worm::ProcessSteerables` (`worm.cpp:1214-1241`) and
`Viewport::Process` (`viewport.cpp:22-57`). From the Rust source: every `debug_assert!`, `unimplemented!`
and "deferred"/"no-op" comment in `rust/sim/src` and `rust/render/src`. From the data: each of the forty
`data/TC/openliero/weapons/*.cfg`, followed transitively through `splinterType`/`partTrailObj` into the
nobject configs (`leaveObj`, `leaveObjDelay`) and their sobjects.

### 2.2 The table

Weapon index = `tc.cfg [types] weapons` order (`tc.cfg:4`) = `weap_table` index = C++ `Weapon::id`.

| Weapon (idx) | C++ branch reached | Rust today | C++ | Rust |
|---|---|---|---|---|
| RIFLE (2) | `ST_LASER` do-loop, ≤ 8 steps/tick | debug panic; release: 1 step | `weapon.cpp:144`, `:336-337`; `rifle.cfg:39` | `weapon.rs:351-354`, `:362-363` |
| WINCHESTER (7) | `ST_LASER` do-loop | same | same; `winchester.cfg:39` | same |
| GAUSS GUN (39) | `ST_LASER` do-loop (5 parts, `startFrame 86` → `cur_frame 0`) | same | same; `gauss_gun.cfg:22,37,39` | same (Fire side already ported, `weapon.rs:121-122`) |
| LASER (28) | `ST_LASER` do-loop **unbounded** (`w.id == 28`) | same | `weapon.cpp:337`; `laser.cfg:39` | same |
| MISSILE (32) | `ProcessSteerables` (turn `cur_frame`, `movable = false`, centroid sums); steerable camera; object-loop Up read | silent no-op; camera `debug_assert` (trips once ported); input lag | `worm.cpp:324`, `:1214-1241`; `viewport.cpp:30-32`; `weapon.cpp:152`; `missile.cfg:39` | `state.rs:2007`, `:1933-1937`; `viewport.rs:86` |
| LARPA (11) | particle trail, type 1: `Create1(vel / SplinterLarpaVelDiv)` | debug panic; release: skipped | `weapon.cpp:201-204`; `larpa.cfg:46-48` | `weapon.rs:355-358`, `:496-498` |
| BOUNCY LARPA (27) | same | same | same; `bouncy_larpa.cfg:46-48` | same |
| CRACKLER (20) | particle trail, else: `rand(128)`, `Create2(vel / SplinterCracklerVelDiv)` | same | `weapon.cpp:205-209`; `crackler.cfg:46-48` | same |
| MINI NUKE (22) | splinters via `Create1(obj vel)` (`splinterScatter 1`); `small_nukes` leave `nuke_smoke` | debug panic (O18); release: no splinters | `weapon.cpp:107-114`; `nobject.cpp:133-138`; `mini_nuke.cfg:43-44`; `small_nukes.cfg:26-27` | `weapon.rs:853-863`; `nobject.rs:501-509` |
| BIG NUKE (19) | `large_nukes` leave `nuke_smoke` | debug panic; release: no trail | `nobject.cpp:133-138`; `large_nukes.cfg:26-27` | `nobject.rs:501-509` |
| NAPALM (30) | `napalm_fireballs` leave `small_explosion__silent` (damage 7) | same | same; `napalm_fireballs.cfg:25-26` | same |
| HELLRAIDER (25) | `hellraider_bullets` leave `hellraider_smoke` | same | same; `hellraider_bullets.cfg:26-27` | same |
| BOOBY TRAP (34) | chain explosion: `BlowUpObject` recursion from inside the blast; the RemExp object (hack off) | debug panic (O9); release: no chain; RemExp omitted | `sobject.cpp:148-150`, `weapon.cpp:87`; `weapon.cpp:137-142`; `booby_trap.cfg:11,48` | `sobject.rs:314-317`; `weapon.rs:316-320` |

The other 27 weapons reach only ported code (their configs take no deferred branch; the explosion,
bonus, worm-hit, blood-trail and shell paths they use were closed in Steps 2–4½a).

### 2.3 Grouped by C++ mechanism

| # | Mechanism | Weapons | Section |
|---|---|---|---|
| M1 | `ST_LASER` do-loop incl. the `id == 28` arm | RIFLE, WINCHESTER, GAUSS GUN, LASER | §4.1 |
| M2 | `ProcessSteerables` + `steerable_sum` + the steerable camera | MISSILE | §4.2 |
| M3 | Input application point (object-loop control reads) | MISSILE (Up boost), BOOBY TRAP (RemExp) | §4.3 |
| M4 | RemExp early-explode | BOOBY TRAP (hack off in this TC) | §4.4 |
| M5 | Particle trail | LARPA, BOUNCY LARPA, CRACKLER | §4.5 |
| M6 | Splinter scatter via `Create1` | MINI NUKE | §4.6 |
| M7 | NObject `leave_obj` trail | MINI NUKE, BIG NUKE, NAPALM, HELLRAIDER | §4.7 |
| M8 | Chain explosion + free-before-explode | BOOBY TRAP | §4.8 |

A new `oracle-tests/tests/weapon_branch_inventory.rs` derives this table from the configs (transitively
through splinter chains, depth-capped) and pins it, plus the facts the ports rely on (LASER is index 28;
`LaserWeapon − 1 == 28`; `RemExpObject − 1` is BOOBY TRAP; RemExp off; both trail divisors 3; no
`ST_LASER` weapon spawns a trail; no chain-exploding weapon carries a damaging obj trail; every trail
delay > 0).

---

## 3. Inherited locked decisions

- **Overview LD 3** — `tick_and_render` stays the only `Sim` mutator; 4½c-0 is pure sim/render code.
- **Overview LD 4** — the scenario grammar stays frozen: no new directive. The goldens reuse 4½a-1's
  `settings <file>` path and 4d's `render player` + `render_live` + `weapon` directives.
- **Overview LD 5** — `SimState::new`'s signature does not change: the new TC constants
  (`wobject_consts`) are a post-`new` field with an inert `Default`, assigned by `build_match` and
  `scenario::load`; the new worm fields (`steerable_sum_x/y`) default to 0 in `WormState::from_init`.
- **Step-2 discipline** — bit-exact against the C++ dumper, `rand()` call order is the contract,
  integer-only, `wrapping_*` where C++ wraps, the pre-/post-`++cycles` split (object loops see
  `cycles = k-1`, the worm loop `k`).
- **Standing isolation gate** — every task re-diffs every golden; no golden file may change.
- **One accumulating PR** on `liero-rs-step-4-5`; no push/PR from executors.

---

## 4. Design per mechanism

### 4.1 M1 — the `ST_LASER` do-loop

C++ runs the *whole* body of `WObject::Process` in `do { ++iter; … } while (w.shot_type == kStLaser &&
used && (iter < 8 || w.id == 28));` (`weapon.cpp:144`, `:336-337`), breaking out on `BlowUpObject`
(`:328-331`) or `Free` (`:332-335`). Every step runs the full body: move, steering, bounce,
`mult_speed`, both trails, the collide loop, the clamp, the ground test, gravity, animation, the timer
and **the worm-hit arm** — so a beam can hit a worm on step 4 of 8, draw its blood fan and sound gate
there, and stop.

**Rust.** The existing single-pass body becomes a private `wobject_pass` (renamed, not re-indented;
the two tripwires removed); `wobject_process` becomes the loop:

```
iter = 0
loop { iter += 1; out = wobject_pass(..); if out != Keep { return out }
       if !(shot_type == ST_LASER && (iter < 8 || id == 28)) { return Keep } }
```

The signature does not change (so the driver does not either). Constants `LASER_MAX_ITERATIONS = 8`
and `LASER_UNBOUNDED_WEAPON_ID = 28` carry the C++ citations.

**`used`.** The Rust driver cannot observe C++'s `used`; it is always true at the loop check *for every
TC weapon*: the only frees in the body are the two `break` arms, and nothing else in an `ST_LASER` body
can free `this` — no `ST_LASER` weapon has an obj or particle trail (pinned in §2.3's test). The loop is
not capped: C++ has no cap, and LASER's `speed 100` (`laser.cfg:19`) guarantees ≥ ~1 px of progress per
step toward an edge, where `!Inside` explodes it (`explGround = true`).

**RNG.** Unchanged per step (the body is unchanged); only the step count changes. For LASER the steps
per tick are data-dependent (hundreds on an open level).

### 4.2 M2 — `ProcessSteerables`, the centroid, the camera

`Worm::ProcessSteerables` (`worm.cpp:1214-1241`), called in the visible arm after the bonus pickup
(`:324`): zero `steerable_count/sum_x/sum_y`; if the **current** weapon's `shot_type == kStSteerable`,
for every wobject with `type == ww.type` (same weapon index) **and** `owner_idx == index`: Left →
`cur_frame -= (cycles & 1) + 1`, Right → `+=`, then `cur_frame &= 127`, `movable = false`, sums
`+= Ftoi(pos)`, `++count`. `cycles` is the worm-loop value (post-`++cycles`). The existing movable
reset (`state.rs:2009-2015`) then keeps `movable` false while Left/Right are held, freezing aim and walk.

**Rust.** `pub fn process_steerables(worm: &mut WormState, weapons: &[Weapon], wobjects: &mut
Pool<WObject>, cycles: i32)` in `weapon.rs` (next to `worm_fire`, the other `worm.cpp` port living
there), called at step 3 of the worm loop with `*cycles`. It tolerates an unresolved slot or an empty
weapon table the way `current_weapon_loops` does (`state.rs:2873-2878`). `WormState` gains
`steerable_sum_x: i32` and `steerable_sum_y: i32` (unhashed — absent from `stateHash.hpp` and from
`WideRollbackChecksum`, `replay.cpp:177-212`); the dead arm keeps zeroing only `steerable_count`
(`worm.cpp:433`, `state.rs:2167`), as C++.

**Camera.** `Viewport::process` (`viewport.rs:80-96`) gains the C++ arm verbatim: alive+visible and
`steerable_count > 0` → `set_center(sum_x / count, sum_y / count)` (C++ `int` division truncates toward
zero; Rust `/` on `i32` does the same), else `set_center(Ftoi(pos))`. The `debug_assert` goes. Nothing
else changes in `render` or `game` (`frame::draw` and the game's `viewport_step` both call
`Viewport::process`). Rationale for closing it here rather than later: leaving it would turn a MISSILE
into a debug-build crash of `game`/`shot` the day `ProcessSteerables` lands.

### 4.3 M3 — the input application point

**Now.** Rust `process_frame` sets `worms[i].control_states` at the top of worm *i*'s pass
(`state.rs:1933-1937`), mirroring the dumper's reduced tail (`sim_physics_dump.cpp:1231-1235`). The
object loops (`game.cpp:334-355`) therefore read each worm's previous-tick input (as mutated by its
previous pass: `PressedOnce`, the death `Release(kFire)`).

**C++.** `Game::ProcessFrame` never applies input: the controller has set all control states before
calling it — `LocalController` on key events (`localController.cpp:58-80`) and AI (`:157-165`) before
`game.ProcessFrame()` (`:175`); the dumper's own `render_live` branch does exactly that
(`sim_physics_dump.cpp:1151-1160`). The object loops see *this* tick's input.

**Why it never mattered.** Only two object-loop reads exist — the steerable Up boost
(`weapon.cpp:152`) and RemExp (`:139`) — and no golden reached either. No worm-loop code reads another
worm's control state, and each worm's own pass sees its fresh input either way.

**Decision.** Apply every worm's input at the **top** of `process_frame` (before the screen-flash
decrement), and make the dumper's reduced tail `Unpack` every worm at the top of the tick (before the
bonus loop), leaving `w->Process(game)` alone in its worm loop. Hash-neutral for the whole corpus by the
argument above; proven twice (the Rust re-diff, then 21 regenerated goldens, §5.5). Rejected: keeping
the interleave and avoiding Up in the MISSILE goldens — the live game and C++ `.lrp` replays would still
lag the boost by a tick, and the two dumper paths would contradict each other on any MISSILE scenario.
This is the one C++ change 4½c-0 makes, confined to `src/tools/oracle_dump/sim_physics_dump.cpp`.

### 4.4 M4 — RemExp (`weapon.cpp:137-142`)

Before the do-loop, once per `Process`: if `common.h[HRemExp]` and this weapon is `LC(RemExpObject) - 1`
(BOOBY TRAP) and its owner holds Change **and** Fire, `time_left = 0` (the timer then explodes it,
`:281-285`). The hack is **off** in the only TC (`tc.cfg:269`), so no C++ golden can reach it without a
TC edit.

**Decision: port it** (six lines) rather than add a `debug_assert!(!h_rem_exp)` guard. The guard needs
the same new field; the port makes `wobject_process` a complete transcription of `WObject::Process`,
makes a RemExp TC behave instead of crash, and reads only already-ported state. It is unit-tested
(owner Change+Fire → explodes this tick; any of hack off / wrong weapon / one key → plain countdown) and
not oracle-gated, which the design states plainly. The owner read follows M3 (this tick's input).

### 4.5 M5 — the particle trail (`weapon.cpp:201-210`)

After the obj trail, before the collide loop, every `part_trail_delay` cycles (pre-`++cycles`, the same
gate the obj trail uses): type 1 → `nobject_types[part_trail_obj].Create1(vel / LC(SplinterLarpaVelDiv),
pos, 0, owner)`; otherwise → `kAngle = rand(128)` then `Create2(kAngle, vel / LC(SplinterCracklerVelDiv),
pos, 0, owner)`. `vel` is the post-steering/bounce/`mult_speed` velocity, `pos` the fixed post-move
position (no `Ftoi`), the division truncating (`Vec2::div`). `nobject_create1`/`nobject_create2` exist.

**TC constants.** A new `Copy` struct `sim::weapon::WObjectConsts { h_rem_exp, rem_exp_object,
splinter_larpa_vel_div, splinter_crackler_vel_div }` with `from_tc(&TcConfig)` and an inert `Default`
(false/0) — a `SimState.wobject_consts` field, passed by value into `wobject_process` (one parameter
instead of four; the long-argument-list style is otherwise kept). Zero divisors are never read unless a
`part_trail_obj >= 0` weapon flies, which no prior golden does.

### 4.6 M6 — splinters via `Create1` (`weapon.cpp:107-114`)

`splinterScatter != 0` (MINI NUKE): per splinter `kColorSub = rand(2)` then
`nobject_types[splinter_type].Create1(fixedvec(kVelX, kVelY), fixedvec(kX, kY), splinter_colour -
kColorSub, cause_idx)` — the exploding wobject's **own** velocity, no angle draw. `blow_up` gains a
`vel: Vec2` parameter after `pos` (the driver passes `obj.vel`, the chain path §4.8 the chained
wobject's post-nudge `vel`). The O18 `#[should_panic]` test is replaced by an RNG-order test.

### 4.7 M7 — the nobject `leave_obj` trail (`nobject.cpp:133-138`)

In the free-air branch, before gravity: `if (!bounced && leave_obj_delay != 0 && leave_obj >= 0 &&
cycles % leave_obj_delay == 0) sobject_types[leave_obj].Create(Ftoi(pos.x), Ftoi(pos.y), owner)`.
`nobject_process` already carries every argument `sobject_create` needs (its explode arm calls it).

**The stale copy.** The nobject driver processes a by-value copy; the pool still holds the pre-move
copy while the trail's `sobject_create` runs its nobject blow-away loop. C++ nudges `this` at its
*post*-move position — the blast is centred there, so both deltas are 0 and the nudge is a no-op; Rust
nudges the stale copy, which the driver then overwrites on `Keep`. Equal. (NAPALM's trail blast has
`damage 7`; the others are damage-0 smoke and skip the block entirely.)

### 4.8 M8 — chain explosions and free-before-explode

C++ `SObjectType::Create`'s wobject loop (`sobject.cpp:118-153`): after nudging an
`affect_by_explosions` wobject inside the box, `if (weapon.chain_explosion) i->BlowUpObject(game,
owner_idx)` — which **frees the wobject first** (`weapon.cpp:87`), then spawns its `create_on_exp`
sobject at `Ftoi(pos)` with the *blast's* owner as cause, plus splinters and crater. A mine's blast can
chain further mines: depth-first recursion through `Create` → `BlowUpObject` → `Create`.

**Rust.** The `wobjects.iter_mut()` loop becomes an index walk (`0..capacity`), exactly the shape the
bonus chain-loop already uses (`sobject.rs:423-453`): compute the nudge inside a scoped borrow; if the
weapon chains, `wobjects.free(slot)` and call `crate::weapon::blow_up(weapon, …, pos, vel, owner_idx,
…)`. An index walk restarted at each recursion level is order-identical to C++'s `All()` `Range`
(`exactObjectList.hpp:19-28`: `Next()` skips freed slots, the outer walk only advances), and freeing
before exploding bounds the recursion by the number of live wobjects.

**The driver must free first too.** The wobjects driver today calls `blow_up` *then* frees the slot
(`state.rs:1773-1796`), so an exploding BOOBY TRAP's own blast would find its stale slot inside the box
and chain itself a second time. The Explode arm becomes `wobjects.free(slot); blow_up(…)` — C++'s order.
For every non-chain blast this is hash-neutral (the stale slot was only ever nudged, then freed; no
wobject is spawned inside `blow_up`), which the re-diff confirms.

**Where chains can start.** Any `damage > 0` sobject creation: a wobject explosion (driver), a trail
blast (HELLRAIDER's `medium_explosion`), a nobject explosion or trail, a bonus explosion. All go through
`sobject_create`, so one port covers them. A chain started while the wobject driver holds wobject *A*'s
copy never touches *A*'s slot: *A* is either already freed (it is the one exploding) or not
`chain_explosion + affect_by_explosions` (no TC weapon with a damaging trail chains — pinned in §2.3).

### 4.9 Plumbing and the `laser_weapon` fix

`build_match` assigns `state.wobject_consts = WObjectConsts::from_tc(&tc)` next to `laser_weapon`.
`scenario::load` (the game/`shot` path, which the live binary still uses until 4½d) assigns
`wobject_consts` **and** the missing `laser_weapon` — so a LARPA picked up in the live game cannot
divide by zero and the LASER's sight arms. Both are unhashed; no golden moves (no golden holds a LASER).

---

## 5. Oracle / gates

### 5.1 Reuse, and why no dumper directive is needed

4½a-1's `settings <file>` path already lets a golden choose per-worm loadouts, `weap_table`,
`loading_time`, `max_bonuses` and fuzz inputs through the real `Settings::FromToml` and the
LocalController start. 4d's `render_live` path already drives the real `ProcessFrame` + viewports with
a `weapon 0 <name>` override. Together they reach every mechanism. The single C++ edit is M3's input
point (§4.3) — a semantics fix, not a directive.

### 5.2 Four settings-driven sim variants (by mechanism, witnesses by weapon)

All on `Levels/modern_test.lev` (the 4½a-1 fuzz arena), KillEmAll, **lives 99** (column 12 must stay
0 — these goldens are about weapons, not match end), health 100, `loadingTime 20` (fast reloads:
MISSILE 96 ticks, RIFLE 55), blood 100, shadow on, `maxBonuses 4`, **`weapTable` all 0**, the 7-bit
random input stream of 4½a-1 (`Rand(input_seed).next_u32() & 0x7f` per worm per tick):

| variant | ticks | player 1 | player 2 | input seed |
|---|---|---|---|---|
| `laser` | 1500 | RIFLE, WINCHESTER, LASER, GAUSS GUN, HANDGUN | LASER, GAUSS GUN, RIFLE, WINCHESTER, DART | 1001 |
| `missile` | 1500 | MISSILE, BAZOOKA, MISSILE, GRENADE, MISSILE | MISSILE, DART, MISSILE, CANNON, MISSILE | 1002 |
| `trails` | 2000 | LARPA, CRACKLER, NAPALM, MINI NUKE, HELLRAIDER | BOUNCY LARPA, BIG NUKE, NAPALM, MINI NUKE, CRACKLER | 1003 |
| `booby` | 2000 | BOOBY TRAP, BAZOOKA, BOOBY TRAP, GRENADE, BOOBY TRAP | BOOBY TRAP, CANNON, BOOBY TRAP, DART, BOOBY TRAP | 1004 |

Rejected: one scenario per weapon (13 C++ runs and 13 scans for no extra localisation — the witnesses
already localise per weapon) and one scenario for everything (a divergence would not localise to a
mechanism). Sidecars are *lean*: `[player1]`/`[player2]` with `health` + `weapons`, `[settings]` with
the sim keys + `version = 6`; missing keys keep defaults on both sides (`toml_archive.hpp:177-186`,
`:224-229`; the Rust reader's `TomlInputArchive` semantics), so the golden cross-checks exactly what
matters. A generator (`oracle-tests/examples/gen_slice4_5c0.rs`) writes sidecars and scenarios and
scans game seeds for the witnesses, in DEBUG; after 4½c-0 no weapon branch is a tripwire, so a panic
during a scan is a bug, never a skipped seed.

### 5.3 Reach witnesses (non-vacuity), from the driven Rust state

Shared by the generator (via `#[path]`) and the milestone test in
`oracle-tests/tests/sim_slice4_5c0_common/mod.rs`, so seed choice and assertion cannot drift. Each is
computed from a pre-tick snapshot and the post-tick state, and is exact or conservative:

- **M1 multi-step** (RIFLE, WINCHESTER, GAUSS GUN): a wobject kept its slot with `Δvel.x = 0` and
  `Δvel.y = 8 · gravity` — eight air steps (a single-step port gives `1 · gravity`).
- **M1 unbounded** (LASER): a new `very_small_explosion__silent` (LASER's only user) appeared ≥ 10 px
  (Chebyshev) from every LASER that entered the tick — beyond what 8 one-pixel steps can reach.
- **M1 hit in the loop** (any of LASER/RIFLE/WINCHESTER): the wobject vanished and no sobject of its
  `create_on_exp` type appeared anywhere that tick — the worm-collide `Remove` arm.
- **M2** left, right, step 1 (even cycle) and step 2 (odd cycle): post-tick `steerable_count > 0` with
  that key held — `steerable_count` is set *only* by `ProcessSteerables`, so this is exact.
- **M3** boost: the owner was visible and held Up while one of its missiles entered the object loop.
- **M5** per trail weapon: a wobject entered a tick whose pre-`++cycles` `cycles % part_trail_delay == 0`.
- **M7** per trail nobject (napalm fireball, small nuke, large nuke, hellraider bullet): it entered a
  tick on its delay **and** a new sobject of its `leave_obj` type appeared.
- **M6**: a MINI NUKE vanished and `small_nukes` appeared.
- **M8**: a BOOBY TRAP with `time_left > 0` vanished with no visible worm within 20 px — its only
  other exits are a worm hit and its timeout.
- **Ban lift**: a bonus offering one of the thirteen weapons appeared (required in `trails`).

Every variant also requires both worms to have spawned and a bonus to have dropped. The C++ golden is
the truth; witnesses only prove the branch was reached, and the milestone test also proves a perturbed
golden row fails.

### 5.4 The steerable camera, LIVE

A generated `render_slice4_5c0_steer` scenario on `Levels/physics_fall_test.lev` (no POWERLEVEL palette
— `modern_test.lev` would expose the known `scenario::load` palette gap on the render path): both worms
start dead at `(0,0)` with 10 lives (the slice-6 fuzz start, so they respawn in-sim with
`killed_timer <= 0` — the only way the alive camera arm can run, the 4d "killed_timer trap"),
`weapon 0 MISSILE`, `max_bonuses 0`, `render player`, `render_live`, 500 ticks. Inputs never press
Change (the current weapon stays MISSILE) and press Fire one tick in eight; Up/Down/Left/Right/Jump are
random. The generator scans *game* seeds (the respawn geometry depends on them) for: a camera tick
(visible, `killed_timer <= 0`, `steerable_count > 0`), one of them with the centroid ≥ 8 px from the
worm, both steer directions and a boost. The C++ dumper runs the real `ProcessFrame` (inputs first, so
the boost reads this tick's Up — M3 is exercised through the *real* frame here) and the real
`ProcessViewports`. The Rust test reuses the 4d live harness (`render_slice4d_common`, generalised from
a name to a file stem) — frame hash, state hash, sim master, `total` — and proves, on an off-worm tick
whose centroid camera differs from the worm camera, that the centering-only re-render equals
`clamp(sum / count − centre)`.

### 5.5 The standing gate and the dumper proof

Every task: `cargo test --workspace --exclude game` (DEBUG) and `cargo test -p game`; no golden file
may change. Each port is hash-neutral for the corpus: M1/M5/M6/M7/M8 replaced tripwires that no golden
ever reached; M2 would have diverged against C++ in any golden where a MISSILE was steered, so none was;
the free-first reorder and M3 only differ where no golden looks. For M3's C++ edit: regenerate, with the
modified dumper, every fuzz golden (5× slice-6, 4× 5d, 4× 5′), `sim_slice6_scales`,
`sim_slice6_gametag`, the four `sim_slice4_5a_*`, the seven `render_slice3b_*` (one script), the three
`render_slice3e_*` and `render_slice4d_live` — 21 script runs covering the reduced tail, the settings
path, the render injection path and the real-`ProcessFrame` path — and require
`git status --porcelain -- rust/oracle-tests/golden` to be empty.

---

## 6. The ban lift

**Where the bans live.** `rust/oracle-tests/examples/gen_slice4_5a.rs` (`BANNED`, `setup_cfg`), the three
committed sidecars `golden/sim_slice4_5a_{killemall,scales,gametag}_setup.cfg` (`weapTable` rows 2/7/28/
32/39 = 2) and `tests/sim_slice4_5a_settings_golden.rs` (`want_weap_table`). They are the *provenance* of
committed goldens and stay byte-identical: a banned weapon can never be dropped in those matches, and
their C++ goldens were produced that way. 4½c-0 does not edit them.

**The lift.** Every new 4½c-0 golden runs with `weapTable` all zero and bonuses on, so any of the forty
weapons may drop; `trails` additionally witnesses a formerly-deferred weapon offered by a bonus. After
4½c-0, `build_match` refuses no weapon, no sim path panics on any weapon, and 4½c can offer all forty.
(4½a-1's global constraint "deferred-branch weapons never appear in a golden loadout" is historical from
here on.)

---

## 7. Task outline

| Task | Deliverable | Gate |
|---|---|---|
| T0 | `weapon_branch_inventory.rs` — the §2 table + port facts, from the configs | pin |
| T1 | `WObjectConsts` + `SimState.wobject_consts` + builder/loader assignment (+ `laser_weapon` on load) + RemExp (M4) | unit + re-diff |
| T2 | M5 the particle trail | unit + re-diff |
| T3 | M1 the `ST_LASER` do-loop (`wobject_pass` + loop) | unit + re-diff |
| T4 | M6 `Create1` splinters (`blow_up` gains `vel`) | unit + re-diff |
| T5 | M7 the nobject `leave_obj` trail | unit + re-diff |
| T6 | M8 chain explosions + free-before-explode in the driver | unit + re-diff |
| T7 | M2 `process_steerables` + `steerable_sum_x/y` | unit + re-diff |
| T8 | M3 input at the top of the tick (Rust + dumper reduced tail) | unit + re-diff + 21-golden regen |
| T9 | the steerable camera arm in `Viewport::process` | unit + re-diff |
| T10 | shared witness module, generator, 4 sidecars/scenarios, C++ goldens, gen script | generator ledgers + awk gate |
| T11 | the steer render scenario + C++ golden; 4d harness generalised to stems | generator ledger |
| T12 | **MILESTONE** — 4 sim goldens + 1 render golden bit-exact, witnesses, perturbation | oracle-tests |
| T13 | full re-diff (debug), `game`, wasm, tripwire grep, PROGRESS + overview | CI commands |

Dependencies: T1 → T2 → T3 (same function; the particle trail lands before the split so the new
`consts` parameter is never unused); T4, T5, T7 after T3; T6 after T4 (it needs `blow_up(vel)`);
T7 → T8 (T8's red test uses T7's steerable state) → T9; T10 needs T1–T9; T11 needs T10's generator;
T12 needs T10+T11; T13 last.

---

## 8. Deferrals (explicitly out)

- **BOOBY TRAP's "fake bonus" name label** (`viewport.cpp:465-479`: with RemExp off and
  `names_on_bonuses` set, a booby is drawn with a weapon name like a bonus). Render-only, gated by a
  setting that defaults off and has no UI yet → the slice that exposes NAMES ON BONUSES (4½e/4½g).
- **`render_slice3e_reload` back to RIFLE.** 3e re-cut it to GRENADE to dodge the laser tripwire; moving
  it back would change a committed golden. Stays GRENADE.
- **Pool-cap self-overwrite.** If a trail/explosion's `NewObjectReuse` overwrites the *last* slot while
  the driver is processing the object in that slot, C++ continues on the overwritten `this`; Rust on its
  copy. Pre-existing, only at a full 600-object pool; a golden that diverged exactly at a cap tick would
  be reported, not fixed here.
- **Holdazone** stays refused (unchanged).

---

## 9. Open questions (with recommendations)

1. **Scope: five weapons or thirteen?** (controller) The overview's five are the ones 4½a-1 happened to
   ban; eight more reach unported branches (§0.1). **Recommendation: all thirteen in 4½c-0** — the same
   rationale, the same machinery, and 4½c needs all forty. Alternative: a 4½c-0b for the eight (4½c
   waits for both either way).
2. **Moving the input application point** (controller; **John only if he wants to keep Step 2's
   interleave convention**). **Recommendation: move it (§4.3)** — it is what C++ does, it is the only way
   the MISSILE boost matches live play and `.lrp` replays, and it is hash-neutral for the corpus.
3. **RemExp: port or guard?** (controller) **Recommendation: port (§4.4)**, unit-tested, stated as not
   oracle-gated.
4. **Touch the 4½a-1 ban provenance?** (controller) **Recommendation: no** — leave generator, sidecars
   and test byte-identical; document the lift here, in PROGRESS and in the overview.

Nothing in this slice needs John's decision to proceed.

---

## 10. Risks

- **Witness rarity.** `trails` needs seven weapons to fire and three trail nobjects to fly; `booby`
  needs a chain away from worms. Mitigation: 2000-tick variants, scan up to 200 game seeds; if a variant
  finds none, lengthen it (a documented knob in the variant table), never weaken a witness.
- **M3 touches a Step-2 convention.** Mitigated by the argument (no worm-loop cross-read; object-loop
  reads unreached), the Rust re-diff, and the 21-golden regeneration across all four dumper paths.
- **The unbounded LASER loop.** Hundreds of steps per LASER per tick; LASER fires every tick while held
  (`delay 0`, `ammo 80`). Cost is linear and bounded by the level; no cap (C++ has none).
- **Recursion depth in chains.** Bounded by live wobjects (≤ 600), each freed before its blast.
- **Render-golden geometry.** The steer golden depends on where seeded worms respawn on
  `physics_fall_test.lev` (~43 % of the spawn rectangle is dirt); the scan picks a seed whose witnesses
  hold.
- **Parallel slice 4½a-2** (TOML writer/storage, `TC_ROOT` centralisation) may touch
  `scenario/src/{loader,build}.rs`; T1's edits there are three lines — rebase trivially.
- **A witness false positive** (e.g. a new wobject reusing a slot with coincidentally equal velocity)
  would make a golden look less vacuous than it is; every witness is designed conservative, and the
  C++ golden, not the witness, is the correctness gate.
