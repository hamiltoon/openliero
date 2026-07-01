# Step 2, Slice 2 — One worm, physics only (`Worm::ProcessPhysics`)

Status: **draft for review** · 2026-06-28
Part of: `2026-06-28-liero-rs-step2-overview.md`
Follows: `2026-06-28-liero-rs-step2-slice1-level-and-statehash-design.md`
(the proven `sim` crate, the `oracle_dump_*` dumper lineage, the golden
differential-test pipeline).

## Purpose

The **first dynamics slice** of Step 2. Slice 1 proved the harness at tick 0 (a
static `SimState` hashes bit-for-bit against C++). Slice 2 makes the worm *move*:
it ports the terrain-collision + gravity physics — `Worm::CalculateReactionForce`,
the reaction-force orchestration inside `Worm::Process`, and `Worm::ProcessPhysics`
— for a single visible worm under scripted (empty) input, and proves the Rust sim
reproduces the C++ **worm component hash tick-for-tick over N≈100 ticks**.

This slice introduces the **per-tick checksum time series** oracle (not just tick
0): the C++ dumper drives the worm N ticks and emits one hash record per tick;
the Rust sim runs the same scenario and matches line-for-line. Per-component
hashes localise any divergence to the exact tick and subsystem.

No control, no aiming, no firing, no death, no `ProcessFrame` ordering yet — those
are Slices 3–6. The worm starts in mid-air and falls under gravity until it
collides with terrain and bounces/settles; that single behaviour exercises
fixed-point integration, the material-map pixel reads, the bounce arithmetic, and
the `CheckedMatWrap` index quirk — the determinism-critical core of the physics.

## Scope

**Included** (ported to Rust, matched against C++):

1. **`Worm::CalculateReactionForce`** (`src/game/worm.cpp:97-147`): the four
   directional 7-point collision probes that read `level.CheckedMatWrap(x,y)
   .Background()` and count non-background hits into `reacts[dir]`.
2. **The reaction-force orchestration block in `Worm::Process`**
   (`src/game/worm.cpp:221-283`): `next = pos + vel`, `i_next = Ftoi(next)`, the
   4-iteration `CalculateReactionForce` loop with the per-iteration level-edge
   additions (`i_next.x < 4` → `reacts[kRfRight]+=5`, etc.) and the `WormFloat`
   branch, then the two `pos.y ± Itof(1)` nudge corrections that re-probe left/right.
3. **`Worm::ProcessPhysics`** (`src/game/worm.cpp:149-208`): horizontal friction
   when grounded, the horizontal/vertical bounce-or-stop with `MinBounce*` /
   `FallDamage*`, gravity (`reacts[kRfUp]==0` → `vel.y += WormGravity`), and the
   guarded position integration (`reacts[...] < 2` → `pos += vel`).
4. **The per-tick oracle**: a new C++ dumper that drives one (or two) worm(s) N
   ticks via `worm->Process(game)` and emits a per-tick hash record; a scenario
   file; an N-line golden; a Rust N-tick differential test.

**Explicitly NOT included** (later slices; see *Boundary* for the exact C++ lines):

- Control/movement/aiming/jump/dig/weapon-change/ninjarope-throw — **Slice 3**.
- Firing / recoil / `Worm::Fire` — **Slice 4**.
- Bonus pickup — **Slice 5**.
- Death, blood/splinter spawn, respawn, `Game::ProcessFrame` ordering
  (`cycles++`, the bonus-drop RNG roll, the object `Process` loops, ninjarope
  `Process`, game-mode logic) — **Slice 6**.

## Boundary: what is "just physics" (precise C++ line references)

`Worm::Process` (`worm.cpp:210-452`) is the per-worm tick. With `visible == true`
it runs, in order:

| `worm.cpp` lines | What | Slice 2? |
|---|---|---|
| 221-283 | reaction-force orchestration (`next`, 4× `CalculateReactionForce`, edge adds, `WormFloat` branch, the two nudge corrections) | **IN** |
| 285-322 | bonus pickup loop (reads `game.bonuses`, may `rand()`) | OUT → Slice 5 (pool empty in S2 ⇒ skipped) |
| 324 | `ProcessSteerables` (reads `wobjects`) | OUT → S3/S4 (pool empty ⇒ skipped) |
| 326-330 | `movable` reset | OUT → Slice 3 |
| 332 | `ProcessAiming` | OUT → Slice 3 |
| 333 | `ProcessTasks` (jump / ninjarope) | OUT → Slice 3 |
| 334 | `ProcessWeapons` (weapon timer countdown) | OUT → Slice 3 |
| 336-343 | `Fire` gate | OUT → Slice 4 |
| **345** | **`ProcessPhysics`** | **IN** |
| 346 | `ProcessSight` (laser raycast) | OUT → S3 (no hashed effect) |
| 348-353 | `ProcessWeaponChange` / `ProcessMovement` | OUT → Slice 3 |
| 355-367 | low-health smoke nobject (`rand()`) | OUT → Slice 6 (health=100 ⇒ skipped) |
| 369-426 | death: blood/splinters, `--lives`, respawn flag (`rand()`) | OUT → Slice 6 (health>0 ⇒ skipped) |
| 428-430 | animation `current_frame` update | OUT (not hashed) |

`CalculateReactionForce` (97-147) and `ProcessPhysics` (149-208) are standalone
functions; the orchestration (221-283) is inline in `Process`. Slice 2 ports all
three as one Rust unit (the worm-physics pass).

**Why driving full `worm->Process` is still an honest physics oracle.** The C++
dumper calls the *unmodified* `worm->Process(game)` (no refactor, no extraction —
the oracle's C++ path is exactly the shipping game's worm logic). Under
**empty input and `health == settings->health`**, every OUT row above either is
skipped (empty pools, `health` not `< health/4`, `health > 0`) or touches only
fields the **worm component hash does not read** (`ProcessWeapons` mutates
`delay_left`; `ProcessAiming`/`ProcessSight` are inert with `aiming_speed == 0`
and a non-laser weapon; the frame update writes `current_frame`). The worm
component hash reads exactly `{pos.x, pos.y, vel.x, vel.y, health, lives, visible,
timer}` (`stateHash.hpp:145-153`); under no-input non-death physics only
`pos`/`vel` change. So the Rust physics subset reproduces the worm component hash
**exactly**, even though Rust does not yet run the OUT rows. (Verified: the only
`rand()` calls reachable from `Process` are in the OUT rows that are all skipped,
so **RNG is never consumed** — see *RNG decision*.)

> **Considered and rejected: extract a `Worm::ProcessReactions` C++ method** so the
> dumper could run reactions+physics in isolation and Slice 2 could match the
> *master* hash too. Rejected for this slice: it edits determinism-critical sim
> code whose only guard is the Slice-1 golden + `test_determinism`, for no oracle
> benefit (the component hash is already the right granularity, per the overview's
> Slice-2 definition). The master hash turns on in Slice 3 when `ProcessAiming`/
> `ProcessWeapons` are ported and the un-ported gap closes.

## RNG decision: isolated worm physics, RNG pristine

**Decision: drive the worm with `worm->Process(game)` directly in an N-tick loop —
NOT `Game::ProcessFrame` — and consume no RNG.**

`Game::ProcessFrame` (`game.cpp:267`) draws RNG **every tick**: after `++cycles`
(line 357) it rolls the bonus-drop chance `rand(c[CBonusDropChance]) == 0`
(359-361) *before* the worm loop (364-366). That single draw moves `rand.last`
every tick and is load-bearing for the master hash and the `rng` component — but
it is **`ProcessFrame` integration, which is Slice 6**, not physics. Pulling it in
now would couple the physics oracle to the bonus subsystem and the full
frame-ordering decision.

By driving `worm->Process` directly:

- **No `rand()` is consumed** (all RNG-drawing rows of `Process` are skipped under
  the Slice-2 scenario — see *Boundary*). `rand.last` stays `0` every tick, so the
  `rng` component hash is a stable `0` and the master hash's `rand.last` term is
  fixed. Honest *and* trivial.
- **`cycles` is not incremented** (the dumper does not call `++cycles`), so
  `cycles == 0` for the whole run. The worm-physics pass reads `cycles` only in the
  un-ported animation frame update (428-430), so this is invisible to the hash.
- The level is **unchanged** (no digging without `kLeft && kRight`), so the `level`
  component hash is constant and equals the Slice-1 value — re-asserted each tick.

This is honest to the charter: Slice 2 is explicitly *pre-`ProcessFrame`*. The
full `ProcessFrame` order (cycles, bonus-roll RNG, object loops, ninjarope) grows
in at Slice 6, at which point the dumper switches from `worm->Process` to
`ProcessFrame` and the master-hash time series is matched.

## Match target

**Per tick, for N≈100 ticks, assert the component hashes** (`HashGameComponents`):

- `worms[0]`, `worms[1]` — **the physics result** (driven purely by `pos`/`vel`
  under this scenario). This is the slice's core proof.
- `rng` (constant `0`), `level` (constant, == Slice-1 value), and the five pool
  hashes (`bobjects`/`bonuses`/`sobjects`/`nobjects`/`wobjects`, all constant `1`
  — pools never spawn under no-input non-death physics). Asserted as invariants;
  any drift flags an accidental RNG draw, terrain write, or spurious spawn.

**The master hash (`HashGameState`) is dumped per tick for the record but NOT
asserted in Slice 2** — it includes `delay_left` (decremented by the un-ported
`ProcessWeapons`) so it will not match until Slice 3. The golden carries the
master column so Slice 3 can flip it on without regenerating.

> Two worms (the C++ fixture default) are used so both `worms[0]` and `worms[1]`
> are exercised. They are given *different* start positions so the two component
> hashes diverge (catching a worm-index mix-up). A worm with no other worm present
> still runs identical physics; the 2-worm setup is the overview's default.

## Datamodel additions (`sim` crate)

Slice 1's `WormState` already has `pos`, `vel`, `health`, `lives`, `visible`,
`timer`. Physics needs almost no *new persisted* worm state — `reacts[]` is fully
recomputed every tick, so it is **tick-local**, not stored. The additions are
mostly *constants* and *scenario start conditions*.

### `WormState` — no new hashed fields

`reacts: [i32; 4]` is **not** added to `WormState`: `CalculateReactionForce` sets
`reacts[dir] = 0` then accumulates, and the orchestration recomputes all four
every tick before `ProcessPhysics` reads them, so they are a per-tick local in the
physics function. (`reacts` is not in any hash.) `movable`, `able_to_jump`,
`able_to_dig`, `aiming_speed`, `direction` are **not** added — `ProcessPhysics`
and the reaction block do not read them (they belong to Slice 3). Resist widening.

Reaction-direction indices mirror C++ `enum { kRfDown, kRfLeft, kRfUp, kRfRight }`
(`worm.hpp:137`) → `0,1,2,3`.

### `WormInit` / scenario — start conditions

The Slice-2 scenario must place a **visible** worm in mid-air. Extend the scenario
worm-init with:

```text
WormInit {
    // ... existing: index, health, lives, stats_x, weapons ...
    start_pos: Vec2,   // fixed-point (16.16) start position; Slice 1 was (0,0)
    visible:   bool,   // Slice 2 sets true so Worm::Process runs the physics path
}
```

`WormState::from_init` sets `pos = init.start_pos`, `visible = init.visible`
(Slice 1 hard-coded `pos = (0,0)`, `visible = false`; those become the defaults
for a tick-0-only scenario). `vel` starts `(0,0)`.

### `LevelSim` — material flags table

`CalculateReactionForce` reads `level.CheckedMatWrap(x,y).Background()`. In C++,
`CheckedMatWrap` (`level.hpp:124-130`) indexes the per-pixel `materials` vector
(precomputed `materials[idx] = common.materials[material_id[idx]]`), returning
`zero_material = common.materials[0]` (`level.hpp:24`) when the index is
out of range. The flag bits come from `TcConfig.materials: [u8; 256]`
(`assets/src/tc.rs:324`), with `Background = 1 << 3` (`material.hpp:11`).

Add the 256-entry flag table to `LevelSim`:

```text
LevelSim {
    width: i32, height: i32, material_id: Vec<u8>,
    material_flags: [u8; 256],   // NEW: from TcConfig.materials; Background = bit 3
}
```

A helper reproduces `CheckedMatWrap(...).Background()` **exactly**, including the
unsigned-wrap index quirk:

```text
fn checked_mat_background(&self, x: i32, y: i32) -> bool {
    // C++: idx = static_cast<unsigned int>(x + y*width); in-range -> materials[idx],
    // else zero_material == materials[ material_id[0] ]'s flags? NO:
    // zero_material = common.materials[0], i.e. flag table entry 0, NOT material_id[0].
    let idx = (x.wrapping_add(y.wrapping_mul(self.width))) as u32;   // two's-complement wrap
    let flags = if (idx as usize) < self.material_id.len() {
        self.material_flags[self.material_id[idx as usize] as usize]
    } else {
        self.material_flags[0]                                       // common.materials[0]
    };
    (flags & 0x08) != 0          // kBackground = 1 << 3
}
```

> **Trap (load-bearing):** the OOB fallback is the flag table at **index 0**
> (`common.materials[0]`), *not* `material_flags[material_id[0]]`. And the bounds
> test is on the *flattened* `x + y*width` reinterpreted as `unsigned`, with **no
> separate `x`-range check** — a negative `x` paired with a `y` that keeps
> `x + y*width` inside `[0, w*h)` reads a wrapped, wrong-row pixel. Both behaviours
> are deterministic and must be reproduced bit-for-bit (see *The hard 10%*).

### `PhysicsConsts` — TC constants the physics reads

A small struct built from `TcConfig.constants` + `TcConfig.hacks`, carried by
`SimState` (or passed to the physics fn). Fields (with `data/TC/openliero` values):

| Field | TC const | openliero value |
|---|---|---|
| `worm_gravity` | `WormGravity` | `1500` |
| `worm_fric_mult` / `worm_fric_div` | `WormFricMult` / `WormFricDiv` | `89` / `100` |
| `min_bounce_up/down/left/right` | `MinBounce*` | `-53248 / 53248 / -53248 / 53248` |
| `fall_damage_right/down` | `FallDamageRight/Down` | `0` / `0` |
| `worm_float_level` / `worm_float_power` | `WormFloatLevel` / `WormFloatPower` | `163` / `-8386178` |
| `h_fall_damage` | hack `FallDamage` | `false` |
| `h_worm_float` | hack `WormFloat` | `false` |

(`FallDamageLeft`/`Up` are unused by `ProcessPhysics`; omit. The reaction block's
`WormFloat` branch needs `worm_float_level`/`_power`; with the hack `false` it
takes the `reacts[kRfUp] += 5` path, but port both faithfully.)

## The physics port (`sim::physics`), mirroring `worm.cpp`

A new module `sim/src/physics.rs` (or methods on `SimState`/`WormState`). The
driver:

```text
SimState::process_worm_physics(&mut self, inputs: &[ControlState]) {
    // 1. apply scripted input to each worm's control_states (Slice 2: all empty)
    // 2. for each worm in worms order: run the worm-physics pass below
    //    (no cycles++, no RNG roll, no object loops — those are Slice 6)
}
```

Named `process_worm_physics`, **not** `process_frame`: it is honestly a
worms-only physics pass. Slice 6 introduces `SimState::process_frame` that wraps
the full C++ `ProcessFrame` order (cycles, bonus-roll RNG, object loops,
ninjarope) around the per-worm processing. Naming it narrowly now avoids a golden
churn when the RNG roll lands.

The per-worm pass reproduces, in this exact order:

1. **Reaction orchestration** (`worm.cpp:221-283`): `next = pos.add(vel)`,
   `i_next = (ftoi(next.x), ftoi(next.y))`; for `i in 0..4` call
   `calculate_reaction_force(&level, i_next.x, i_next.y, i, &mut reacts)` and apply
   the edge additions inside the loop (note: applied **every iteration**, matching
   the C++ comment "Liero does this in every iteration"); the `WormFloat` branch;
   then the two `pos.y ± itof(1)` nudge corrections that re-probe `kRfLeft`/
   `kRfRight`. `reacts` is a local `[i32; 4]`.
2. **`process_physics`** (`worm.cpp:149-208`): friction, bounce/stop, gravity,
   guarded integration — see *Fixed-point traps*.

`calculate_reaction_force` (`worm.cpp:97-147`) ports the `kColPoints[4][7]` table
and `kColPointCount[4] = {3,7,3,7}` verbatim, counting non-background probes.

### Fixed-point / arithmetic traps (must reproduce exactly)

- **Friction** `vel.x = (vel.x * WormFricMult) / WormFricDiv`:
  `vel.x.wrapping_mul(89) / 100` — C++ `int` division **truncates toward zero**
  (use Rust `/` or `wrapping_div`, *not* `>>`).
- **Bounce** `vel.x = -vel.x / 3`: C++ parses `(-vel.x) / 3`, truncating toward
  zero. Port as `vel.x.wrapping_neg().wrapping_div(3)` (the `wrapping_neg` guards
  `i32::MIN`). Same for `vel.y = -vel.y / 3`.
- **`abs(vel)`**: `fixedvec(std::abs(vel.x), std::abs(vel.y))` — use
  `wrapping_abs` (guards `i32::MIN`); compared `> mbh`/`> mbv` where
  `mbh = vel.x > 0 ? MinBounceRight : -MinBounceLeft` (note `-MinBounceLeft`,
  with `MinBounceLeft` negative → positive).
- **Gravity** `vel.y += WormGravity`: `wrapping_add`.
- **Integration** `pos.x += vel.x` guarded by `reacts[vel.x >= 0 ? kRfLeft :
  kRfRight] < 2`; `pos.y += vel.y` guarded by `reacts[vel.y >= 0 ? kRfUp :
  kRfDown] < 2` — `wrapping_add`. The `>= 0` sign tests pick the reaction index;
  reproduce exactly (`vel.x >= 0` selects `kRfLeft`).
- **`Ftoi`/`Itof`**: `ftoi` = arithmetic `>> 16` (rounds toward −∞); `itof` =
  `wrapping_shl(16)` — already in `sim-core::fixed`. The reaction probes work in
  *integer* pixel space (`i_next = Ftoi(next)`); collision points add small integer
  offsets (`kColPoints`) to those.

## Input-vector / scenario file format (established here, used by Slices 3-6)

The scenario is a small committed text file read by **both** the C++ dumper and
the Rust test (single source of truth — no duplicated fixture constants, unlike
Slice 1). Proposed `rust/oracle-tests/golden/sim_slice2_scenario.txt`:

```text
# Step 2 Slice 2 scenario — one/two worms, physics only.
seed 42
level Levels/modern_test.lev
ticks 100
# worm <index> <pos_x_fixed> <pos_y_fixed> <health> <lives> <stats_x> <visible>
worm 0 6553600 3276800 100 10 0   1
worm 1 13107200 3276800 100 10 218 1
# Sparse per-tick input overrides (7-bit ControlState per worm); absent => 0.
# Slice 2 has NONE (empty input every tick); Slice 3+ adds lines:
# input <tick> <worm0_7bit> <worm1_7bit>
```

- `pos_*` are 16.16 fixed-point (`6553600 == Itof(100)`). The implementer picks
  start positions from `modern_test.lev` such that the worm falls through open
  background and **collides with terrain within `ticks`** (so the golden exercises
  a bounce, not only free-fall — verify a `vel.y` sign flip appears). Both worms at
  the same `y`, different `x`, so the two component hashes differ.
- **Sparse input overrides**: a worm's input at tick `t` is `0` unless an
  `input <t> <w0> <w1>` line sets it. Slice 2 has zero such lines (clean: empty
  input is the *absence* of override lines). Slice 3 adds them (or a dense stream);
  Slice 6 can point at a fuzz seed instead. The format is fixed now.

The level path is relative to the TC root (`data/TC/openliero`), as Slice 1.

## C++ dumper (`oracle_dump_sim_physics`)

**Decision: a NEW dumper `src/tools/oracle_dump/sim_physics_dump.cpp` + target
`oracle_dump_sim_physics`, not an extension of `sim_dump.cpp`.** Rationale: the
tick-0 dumper (`sim_dump.cpp`) and its golden (`sim_slice1.txt`) are committed and
CI-verified; the physics dumper has different argv (a scenario file), different
output (N lines), and a different driver (`worm->Process` loop). Keeping them
separate avoids entangling two goldens and leaves Slice 1's proof untouched.

The dumper (style per `sim_dump.cpp` / `level_dump.cpp`):

1. `PrecomputeTables()`; load `Common` from `data/TC/openliero`; `Settings`
   (`game_mode = kGmKillEmAll`, `lives` from scenario, `loading_time = 0`);
   `Game game(common, settings, NullSoundPlayer)`; `game.rand.Seed(seed)`.
2. **Load the fixed `.lev`** from the scenario (via `Level::load`, as `sim_dump`),
   not `GenerateFromSettings`.
3. Add 2 worms (`settings`, `health`, `index`, `stats_x`); `InitWeapons`;
   `ResetWorms`. Then apply the scenario start conditions: `w->pos = scenario_pos`,
   `w->visible = true`. **No viewports** (we never call `ProcessFrame`, so the
   banner/shake code that reads viewports never runs).
4. **Dump tick 0** (initial state, before any motion) and then loop `ticks`
   times: apply each worm's scripted input via `control_states.Unpack(input)`,
   call `worm->Process(game)` for each worm in `game.worms` order, then dump the
   record. (Decide whether to emit tick 0 then N more, or N records starting at
   tick 1 — recommend: emit a record for tick 0 *and* after each of the N
   `Process` passes, i.e. `N+1` lines, so the series is anchored at the proven
   tick-0 state.)
5. Each record line: `<tick> <state_hash> <rng> <level> <worm0> <worm1>
   <bobjects> <bonuses> <sobjects> <nobjects> <wobjects>` — `tick` decimal, every
   hash `%08x`.

The dumper must **not** call `Game::ProcessFrame`, `++game.cycles`, or
`GenerateFromSettings`.

## Golden format

`rust/oracle-tests/golden/sim_slice2.txt` — one line per tick (`N+1` lines):

```text
<tick> <state_hash> <rng> <level> <worm0> <worm1> <bob> <bon> <sob> <nob> <wob>
```

Tick 0's `worm0`/`worm1` reflect the scenario start (visible, mid-air, vel 0);
later ticks show the falling/bouncing physics. `rng` is `00000000` on every line;
`level` and the five pool columns are constant. The Rust test asserts the
component columns per line (master column carried for Slice 3).

## Definition of done

- [ ] `sim` gains the physics port: `LevelSim.material_flags` +
      `checked_mat_background`, `calculate_reaction_force`, the reaction
      orchestration, `process_physics`, and the `process_worm_physics` driver.
- [ ] `WormInit`/`WormState` carry `start_pos` + `visible`; `from_init` honours
      them (Slice-1 tick-0 callers default to `(0,0)` / `false` — Slice 1 golden
      still passes).
- [ ] All arithmetic uses `wrapping_*` / truncating `/`; the `CheckedMatWrap`
      index quirk (unsigned flatten, OOB → flag table `[0]`) is reproduced.
- [ ] Unit tests pin: `calculate_reaction_force` probe counts on a synthetic
      level; gravity free-fall over K ticks; a bounce (`vel.y` sign flip + `/3`);
      the friction term; the edge-addition and OOB-wrap behaviour.
- [ ] `sim_physics_dump.cpp` + `oracle_dump_sim_physics` CMake target (under
      `OPENLIERO_BUILD_ORACLE_DUMP`); `gen_sim_physics_golden.sh` regenerates
      `sim_slice2.txt` from the committed scenario.
- [ ] `cargo test`: `sim_slice2_golden` matches every component column for all
      `N+1` ticks; the golden shows at least one bounce.
- [ ] `cargo test --workspace` green; `sim` still Bevy-free / float-free.
- [ ] C++ side: after adding the dumper, `test_determinism` + `test_rollback_*`
      still pass (the dumper links `game` but does not alter sim code).

## Open questions (decide in the plan)

1. **Start positions & `N`.** Exact `pos` in `modern_test.lev` that guarantees a
   fall-then-collision within `N` (so the golden covers a bounce). Default `N=100`;
   raise if 100 ticks free-fall without hitting terrain. The implementer reads the
   level to pick a column with background above rock.
2. **Tick-0 row.** Emit `N+1` lines anchored at tick 0 (recommended) vs `N` lines
   from tick 1. Anchoring at tick 0 lets the first line cross-check against a
   Slice-1-style static hash of the start state.
3. **One worm vs two.** Two (overview default) so both `worms[i]` columns are
   exercised; different start `x` so they diverge. Confirm a 1-worm variant is not
   needed.
4. **`PhysicsConsts` location.** On `SimState` (built once from `TcConfig`) vs a
   parameter to `process_worm_physics`. Recommend storing on `SimState` so the
   driver signature stays `(state, inputs)`, matching the locked
   `(state, input) -> state` tick shape.
5. **Master-hash assertion.** Confirm Slice 2 asserts components only and carries
   the master column un-asserted (flipped on in Slice 3). The plan should state the
   exact reason the master diverges (`ProcessWeapons` `delay_left` countdown).

## The hard 10% (carried into this slice)

- **`CheckedMatWrap` index semantics** — unsigned-reinterpret flatten, no separate
  `x` bound, OOB → flag table `[0]`. The single most likely source of a one-pixel
  collision desync. Unit-test the wrap explicitly.
- **Truncating vs shifting division** — bounce `/3` and friction `/div` truncate
  toward zero; `Ftoi` shifts toward −∞. Mixing them up desyncs on negative
  velocities (a falling-left bounce).
- **`i32::MIN` edge cases** — `wrapping_abs` / `wrapping_neg` on velocity; unlikely
  in-game but the port must not panic in debug.
- **Reaction-index sign tests** — `vel.x >= 0 ? kRfLeft : kRfRight` (and the `>= 0`
  vs `> 0` distinction between the bounce read at 163-166 and the integration read
  at 201/205) must be copied verbatim; an off-by-one in the index reads the wrong
  reaction count and lets the worm tunnel.
- **Edge additions every iteration** — the `i_next.x < 4` etc. additions run inside
  the 4-iteration loop (so they accumulate `+5` up to four times), per the C++
  "Liero does this in every iteration" comment. Reproduce the accumulation, not a
  single application.

## Next artifact

The TDD plan: `plans/2026-06-28-liero-rs-step2-slice2-plan.md`.
