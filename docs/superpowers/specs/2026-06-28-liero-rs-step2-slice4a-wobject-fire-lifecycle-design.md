# Step 2, Slice 4a — Projectile lifecycle: `Worm::Fire` + `WObject::Process`

Status: **draft for review** · 2026-06-28
Part of: `2026-06-28-liero-rs-step2-slice4-weapon-lifecycle-overview.md`
Follows: `2026-06-28-liero-rs-step2-slice3-control-aiming-design.md`
(the proven `sim` crate, `oracle_dump_sim_physics`, the scenario pipeline, the
`process_worms` driver this slice renames + extends).

## Purpose

4a is the **first sub-slice** of the weapon-lifecycle milestone and the slice where
**RNG goes live**. It ports the projectile *birth-to-death* path for one weapon —
`Worm::Fire` (recoil, ammo--, `delay_left`, spread/colour RNG, spawn a `WObject`)
→ `WObject::Process` (move, gravity, boundary clamp, terrain/ground collision,
timeout) → `BlowUpObject` (free the wobject) — using **`fan`**, the one weapon that
*explodes into nothing* (`create_on_exp=-1`, `dirt_effect=-1`, `splinter_amount=0`).

So 4a proves the genuinely new machinery — `Fire`, `Pool` spawn/free inside the
object loop, `WObject::Process`, the **ProcessFrame-subset driver**, and the first
**live `rand()` sequence** matched against C++ — while keeping the **level
pristine** (no `DrawDirtEffect` yet ⇒ the `level` component hash stays the Slice-1
constant) and spawning **no** sobjects/nobjects/blood. Terrain destruction is 4b;
explosion sobjects + splinters are 4c (overview, *Decomposition*).

### What changes vs Slice 3

| Invariant | Slices 1–3 | **4a** |
|---|---|---|
| `rng` component / `rand.last` | constant `0` | **live** — moves at each fire tick (4 Fire rands for fan) |
| `wobjects` pool | empty | **non-empty** — spawn on Fire, free on explode |
| driver | worms-only `process_worms` | **`process_frame`** subset: object loops **then** worms |
| `level` hash | constant | **still constant** (no `DrawDirtEffect`) |
| `cycles` | `0` | still `0` (no `++cycles`, no bonus roll) |

## Scope

### IN — ported this slice (C++ references)

- **The Fire gate** (`worm.cpp:336-340`): `if (Pressed(kFire) && !Pressed(kChange)
  && weapons[current_weapon].Available() && weapons[current_weapon].delay_left<=0)
  Fire(game);`. The `else if` branch only stops a loop sound (no sim/RNG effect) ⇒
  port the gate, skip the sound side. Runs **after** `ProcessWeapons`, **before**
  `ProcessPhysics`, in the per-worm pass.
- **`Worm::Fire`** (`worm.cpp:1099-1148`): `--ww.ammo`; `ww.delay_left = w.delay`;
  `fire_cone = w.fire_cone`; compute `kFiring`; **leave-shell `rand`** (fan
  `leave_shells=0` ⇒ skipped, but ported as a guarded branch); `affect_by_worm` ⇒
  `speed = max(speed,100)`, `firing_vel = vel*100/speed`; `parts` × `Weapon::Fire`;
  **recoil** (`HSignedRecoil` hack; `vel -= cossin[Ftoi(aiming_angle)]*recoil/100`).
- **`Weapon::Fire`** (`weapon.cpp:16-76`): spawn via `wobjects.NewObjectReuse()`;
  `obj.vel = cossin[angle]*speed/100 + vel`; **spread RNG** (`distribution`);
  `cur_frame` (the `start_frame<0` colour-`rand` path for fan); `time_left =
  time_to_explo (- rand(time_to_explo_v))`. Stats calls (`DamagePotential`,`Shot`)
  are no-ops here.
- **`WObject::Process`** (`weapon.cpp:127-338`), the `shot_type != kStLaser` single
  pass (the `do…while` runs once for fan): `pos += vel`; (shot_type 2/3 steering —
  N/A for fan); bounce block (`bounce=0` ⇒ skipped, ported guarded); `mult_speed`
  (=100 ⇒ skipped); trails (none for fan); `collide_with_objects` loop (inert —
  skips same `type`+`owner`); boundary clamps (`weapon.cpp:234-247`); terrain
  collision (`weapon.cpp:249-279`): `if (!Inside(inew) || PixelMat(inew).DirtRock())`
  ⇒ `bounce==0` & `expl_ground` ⇒ `do_explode`; else `vel.y += gravity` (+ anim,
  N/A); timeout `if (time_to_explo>0 && --time_left<0) do_explode`; worm-hit loop
  (excluded by geometry, ported guarded); `if (do_explode) BlowUpObject; break`.
- **`BlowUpObject`** (`weapon.cpp:78-125`) for fan: snapshot pos/vel; `wobjects.Free
  (this)`; `create_on_exp=-1` ⇒ no sobject; sound (no-op); `splinter_amount=0` ⇒ no
  splinters; `dirt_effect=-1` ⇒ **no `DrawDirtEffect`**.
- **The ProcessFrame-subset driver** (`game.cpp:333-355` order): sobjects →
  wobjects → nobjects → bobjects → worms; **no** `++cycles`, **no** bonus-drop roll,
  **no** ninjarope, **no** game-mode (overview, *Oracle / driver decision*).
- **A `DirtRock()` material probe** over the existing `material_flags` table
  (sibling to Slice 2's `checked_mat_background`).

### OUT — deferred (with target sub-slice)

| C++ | What | Deferred to |
|---|---|---|
| `weapon.cpp:117-124`, `89-92`, `96-115` | `DrawDirtEffect`, `create_on_exp` SObject, splinters in `BlowUpObject` | 4b (crater) / 4c (sobject, splinters) — fan triggers none |
| `weapon.cpp:148-167` | shot_type 2/3 steering + drunk spread | only if a steerable/drunk weapon is chosen later |
| `weapon.cpp:196-210` | obj/part trails | 4c (fan sets none) |
| `weapon.cpp:287-326` | worm-hit damage/blood/`worm_collide` | 4c (excluded by geometry in 4a) |
| `worm.cpp:355-367` | low-health smoke nobject | Slice 6 (health 100) |
| `ProcessWeapons` reload (`worm.cpp:822-827`) + shell-drop (`838-846`) | `ammo<=0 → loading_left`; `leave_shell_timer` | 4d (fan ammo 150, `leave_shells=0` ⇒ unreached) |
| `worm.cpp:346` `ProcessSight` | laser sight | 4d / skip (no hashed effect) |
| `++cycles`, bonus-drop roll, ninjarope `Process` | ProcessFrame integration | Slice 6 |

> **Stats are no-ops.** `stats_recorder->*` and the `has_hit`/`fired_by` fields are
> stats-only — not hashed, no sim effect (overview, *Datamodel*). Omitted entirely.

## RNG decision: RNG goes live; level stays pristine

**Decision: 4a matches the live `rand()` sequence but keeps the level constant**,
by choosing **fan** (no `DrawDirtEffect`) and a geometry that excludes every
non-Fire `rand()`. The **only** RNG in 4a is `Worm::Fire`'s spread+colour+time-var
(audited in the overview, *RNG audit*): **4 rands per fire tick for fan** — spread
`vel.x` `rand(24000)`, spread `vel.y` `rand(24000)`, colour `rand(2)`, time-var
`rand(10)`, in that order.

Exclusions (enforced by scenario construction; assert in the golden):

| Other `rand()` site | Reached when | Excluded by |
|---|---|---|
| `WObject` worm-hit (`weapon.cpp:303-324`) | a worm within `detect_distance=1` of the shot | non-firing worm **invisible** (`CheckForSpecWormHit` → false) + firer kept off the flight path |
| `DrawDirtEffect` (`BlowUpObject`) | `dirt_effect>=0` | fan `dirt_effect=-1` |
| splinters | `splinter_amount>0` | fan `=0` |
| dirt-throw / blood (SObject) | a sobject is created | fan `create_on_exp=-1` |
| low-health / death / dig / shell | health<25 / dead / L+R / `leave_shell_timer>0` | health 100, visible, no L+R, `leave_shells=0` |

**Consequence:** the `level` component hash is the **constant** Slice-1/2/3 value
on every line (re-asserted — a regression guard that no terrain was touched). The
`rng` column is `00000000` until the first fire tick, then tracks `rand.last`.

## The ProcessFrame-subset driver (`process_frame`)

Rename `process_worms` → **`process_frame`** (it is no longer worms-only). Per
tick, in **exact `game.cpp` order**:

1. **sobjects loop** — empty in 4a (no-op).
2. **wobjects loop** — for each live wobject in **slot order**, run
   `wobject_process`. It mutates the wobject, reads `level`/`weapons`, and may
   **free the current slot** (explode/remove). Implementation walks slot indices
   `0..capacity`; for each live slot, copy the (Copy) `WObject` out, run
   `wobject_process(&mut wo, &level, weapons, …, &mut rand)` returning an outcome
   {`Keep`, `Explode`, `Remove`}, then write `wo` back (`Keep`) or `free(slot)`
   (`Explode`/`Remove`). Copying-out gives the process fn `&mut` to the rest of
   `SimState` (level, other pools, worms) without aliasing the pool. *(In 4a nothing
   spawns into `wobjects` during this loop, so re-visiting newly-spawned slots —
   the C++ `Range.end==&arr[Limit]` behaviour — does not arise; 4c handles it.)*
3. **nobjects loop** — empty in 4a (no-op).
4. **bobjects loop** — empty in 4a (no-op).
5. **worms loop** — unchanged from Slice 3 (input-apply ≈ `Unpack`, then the full
   per-worm `Worm::Process` pass) **plus the Fire gate** (step 8 of the per-worm
   pass, previously OUT). `Worm::Fire` spawns into `wobjects` here — **after** the
   wobjects loop already ran this tick, so the new shot first moves next tick.

No `++cycles`, no bonus-drop roll, no ninjarope, no game mode. `cycles` stays `0`.

### Per-worm pass: the Fire gate slots in

Slice 3's order was: reactions → steerables → movable-reset → aiming → tasks →
weapons → physics → change/movement. 4a inserts the **Fire gate between
`process_weapons` and `process_physics`** (`worm.cpp:334-345`):

```
process_weapons(w)
if w.control_states.get(FIRE) && !w.control_states.get(CHANGE)
   && w.weapons[w.current_weapon].available()    // loading_left == 0
   && w.weapons[w.current_weapon].delay_left <= 0 {
    worm_fire(w, weapons, cossin, &mut rand, &mut wobjects)   // spawns a WObject; mutates w.vel/ammo/...
}
worm_process_physics(w, &reacts, physics)
// (ProcessSight OUT)
change/movement gate (Slice 3)
```

`available()` mirrors `WormWeapon::Available()` (`worm.hpp:35`) — it returns
**`loading_left == 0`** (the gate does **not** test `ammo` directly; `ammo>0` is
enforced *indirectly* because `ProcessWeapons` sets `loading_left>0` when `ammo<=0`,
the deferred reload branch). The fire side
mutates `w` (`vel` recoil, `ammo`, `delay_left`, `fire_cone`) and pushes into the
`wobjects` pool — so the driver must hold `&mut wobjects` and `&mut rand` while
iterating worms (disjoint from the per-worm borrow; same pattern as Slice 3's
`SimState{ level, physics, control, worms, .. }` destructure, extended with
`wobjects`, `rand`, `weapons`, `cossin`).

## Datamodel additions (`sim` crate)

No **new hashed wobject field** — `WObject {pos, vel, cur_frame, time_left, ty}`
already carries every master-hashed wobject field (`hash.rs:110-116`). Added:

| New field / type | C++ | Why (non-hashed unless noted) |
|---|---|---|
| `WObject.owner_idx: i32` | `WObject::owner_idx` | self-exclusion in the collide loop; owner for the (excluded) worm-hit path |
| `WormWeapon::available()` | `WormWeapon::Available()` (`= loading_left == 0`) | the Fire gate predicate |
| `SimState.weapons: Vec<Weapon>` | `common.weapons` | `Fire`/`Process` read the firing weapon's params |
| `SimState.cossin: [Vec2;128]` | `cossin_table` | `Fire` (vel, firing pos) + recoil; from `sim_core::tables::precompute_cossin()` |
| `LevelSim::dirt_rock(x,y) -> bool` | `PixelMat(x,y).DirtRock()` + `Inside` | `WObject::Process` ground collision |

`Weapon` is `assets::object::Weapon` (already parsed, 1e-2) — `SimState` holds the
resolved table (or the subset it needs); the differential test loads it from the TC
exactly as the dumper does. **`Spawn into `Pool` returns `Option<usize>`** — assert
`Some` in 4a (pool never full); the `NewObjectReuse` full-pool overwrite (overview
**O3**) is **not** implemented this slice but is documented on `Pool` where it is
first filled.

### `DirtRock` probe

Port `Level::PixelMat(x,y).DirtRock()` over the existing 256-entry
`material_flags`: look up `material_flags[material_id[idx]]` (the same flattened,
two's-complement index as `checked_mat_background`) and test the `DirtRock` bit
combination (`material.hpp:22`: `flags & (kDirt|kDirt2|kRock)`, i.e. bits 0|1|2;
`kBackground` is bit 3). The
out-of-bounds / `Inside()` semantics: `WObject::Process` tests `!game.level.Inside
(inew_pos)` **separately** before the material test (`weapon.cpp:249`), so port
`Inside` (a real `0<=x<width && 0<=y<height` range check, **not** the wrapping
`CheckedMatWrap`) as its own predicate, then `dirt_rock` only when inside. Confirm
against `level.hpp` `Inside` vs `CheckedMatWrap`.

## Input scenario design

A **new** `golden/sim_slice4a_scenario.txt`, same grammar as Slice 3 **plus** the
new `weapon <slot> <name>` directive (overview, *Oracle / driver decision*). Reuse
`Levels/physics_fall_test.lev` (open sky over a solid floor). `seed 42`,
`ticks ≈ 80`, two visible worms.

- `weapon 0 fan` (both worms get fan in slot 0; `current_weapon=0`).
- Worm 0: aim + **Fire** (set bit `Fire=16`) for a few ticks → spawns fan shots
  that fly (gravity 0 ⇒ straight) and explode by `timeToExplo` (~35–45 ticks) **and**
  one aimed into the floor to exercise the ground-collision explode (`expl_ground`).
- Worm 1: a different, **Fire-free** pattern (or fires at a different tick) so the
  two master hashes diverge; kept **invisible** or far so it is never hit.

**Load-bearing constraints (comment them in the file):** keep both worms `health
100`; never set Left+Right together (dig stays deferred / `debug_assert!`); place
worms so **no shot passes within `detect_distance`+sprite of a *visible* worm**
(excludes the worm-hit RNG); the non-firing worm invisible. Tune ticks so the
golden actually shows: a fire tick (`rng` moves, `ammo`↓, `delay_left`=delay, a
wobject appears), flight (wobject `pos`/`vel` evolve, `time_left`↓), and an explode
tick (wobject disappears, `rng` unchanged at explode since fan draws none there).

## Oracle / golden

Per the overview decision:

- **C++ dumper: extend `sim_physics_dump.cpp`** — (a) add the four object loops
  before the worm loop; (b) parse + apply the `weapon <slot> <name>` directive
  (resolve the name to a `Weapon*` from `common`, set `worm->weapons[slot].type`
  and `.ammo` after `InitWeapons`). **No `++cycles`, no bonus roll.** Binary name
  unchanged. **Required check:** regenerate `sim_slice2.txt`/`sim_slice3.txt` and
  confirm they are **byte-identical** (object loops are no-ops on their empty pools)
  — proves the extension didn't perturb the prior proofs.
- **New** `golden/sim_slice4a_scenario.txt`, `gen_sim_slice4a_golden.sh` (copy of
  the slice-3 gen script, pointed at the slice-4a files; LOCAL/MANUAL), committed
  `golden/sim_slice4a.txt`.
- **New** `tests/sim_slice4a_golden.rs`: assert the master `state_hash` **and** all
  9 component columns per tick (input keyed `k-1`, the established off-by-one), with
  a coverage guard that `wobjects` is non-empty for some ticks and `rng`/`ammo`/
  `delay_left` each take ≥2 distinct values.
- The Rust scenario parser (`oracle_tests::scenario`) gains the `weapon` directive;
  the Rust builder maps it to `WeaponInit { ty: Some(fan_id), ammo }` for slot 0.

### Input timing (unchanged off-by-one)

Golden line `k` (`k≥1`) = state after `process_frame` with input keyed **`k-1`**;
line 0 = the pre-motion tick-0 state with no call (Slice 2/3 *Input timing*).

## Definition of done

- [ ] `WObject` gains `owner_idx`; `WormWeapon` gains `available()`; `SimState`
      carries `weapons` + `cossin`; `LevelSim` gains `inside()` + `dirt_rock()`.
- [ ] `worm_fire` (+ `weapon_fire`) ported: ammo--, delay_left, fire_cone, leave-
      shell (guarded), `affect_by_worm`, parts loop, spread/colour/time-var RNG in
      **C++ order**, recoil (+ `HSignedRecoil`). Unit-tested with fan constants.
- [ ] `wobject_process` ported (single non-laser pass): move, bounce (guarded),
      boundary clamp, `inside`/`dirt_rock` ground-collision explode, gravity,
      timeout, worm-hit (guarded), `blow_up` (free; fan path only). Unit-tested.
- [ ] Driver renamed `process_frame`: object loops (s/w/n/b) then worms; Fire gate
      between `process_weapons` and `process_physics`; **no** cycles/bonus/ninjarope.
      Slice-2/3 call sites updated.
- [ ] Dumper extended (object loops + `weapon` directive); slice-2/3 goldens
      **byte-identical** after; new `sim_slice4a_scenario.txt` + gen script +
      committed `sim_slice4a.txt`.
- [ ] `tests/sim_slice4a_golden.rs`: master + 9 components match every tick; coverage
      guard (non-empty wobjects; rng/ammo/delay_left move). `level` column constant.
- [ ] `cargo test --workspace` green; `sim` Bevy-free / float-free; deps unchanged.
- [ ] Determinism note: only the dumper (oracle-gated, non-sim) C++ changed ⇒
      `test_determinism`/`test_rollback_*` unaffected.

## The hard 10% (this slice)

- **Fire RNG order** — spread-x, spread-y, colour, time-var, **in that order**, and
  only when their guards (`distribution`, `start_frame<0`, `time_to_explo_v`) fire.
  A reordered or extra/missing draw shifts every downstream `rand.last`.
- **Object-loop-before-worms** + **Fire spawns after the wobjects loop** — the new
  shot must sit still its birth tick. Get the driver order wrong and the wobject
  moves one tick early.
- **Free-during-iteration** — the copy-out/write-back-or-free slot walk must match
  C++ `Range`+`Free(this)` exactly (slot order, current-slot free).
- **`DirtRock` vs `CheckedMatWrap` vs `Inside`** — `WObject::Process` uses a real
  `Inside` range check **plus** `PixelMat(...).DirtRock()`, *not* the wrapping
  `CheckedMatWrap` the worm physics uses. Port the right probe; audit the `DirtRock`
  bit set.
- **Truncating fixed-point arithmetic** — `cossin*speed/100`, `vel*100/speed`,
  `*recoil/100`, `*bounce/100` truncate toward zero (Rust `/` / `wrapping_*`, never
  `>>`), same discipline as Slices 2–3.
- **`NewObjectReuse` full-pool semantics (O3)** — documented but not implemented;
  assert `spawn` returns `Some` in 4a.

## Next artifact

The TDD plan: `plans/2026-06-28-liero-rs-step2-slice4a-plan.md`.
