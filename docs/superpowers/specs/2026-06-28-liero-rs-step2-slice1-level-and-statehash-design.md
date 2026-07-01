# Step 2, Slice 1 — Level → sim-state + state-hash harness (frame 0)

Status: **draft for review** · 2026-06-28
Part of: `2026-06-28-liero-rs-step2-overview.md`
Follows: the overview's locked decisions (new `sim` crate, sim-core-driver tick,
pool model, per-tick checksum oracle).

## Purpose

The first, minimal slice of Step 2. Stand up the `sim` crate: build a `SimState`
from `assets::LevelData` + a small worm-init description, and a Rust state hash
(`hash_game_state` / `hash_components`) that mirrors C++ `HashGameState` /
`HashGameComponents`. Prove that at **tick 0 — the initial state, before any
`ProcessFrame` runs** — the Rust hashes match the C++ oracle bit-for-bit.

No dynamics: no `ProcessFrame`, no physics, no objects spawned, no RNG consumption.
This proves the *harness* and the *level + initial-worm + empty-pool* halves of the
checksum before any motion exists. Every later slice adds dynamics onto this proven
foundation.

Delivers: the `sim` crate (datamodel + empty pools + state hash), a new C++ oracle
dumper (`oracle_dump_sim`) that builds a `Game` to the same tick-0 state and dumps
both hashes, and a golden differential test reproducing them.

## Scope

**Included:**

1. **`sim` crate** (`rust/sim/`), depending on `sim-core` + `assets`, Bevy-free.
   - `SimState`: the frame-0 subset of the `fast_snapshot.hpp` inventory — level
     material buffer, `rand` (seeded, `last`), `cycles`, empty object pools, and a
     `worms` vector of initial `WormState`.
   - `WormState`: the per-worm fields the **hash** reads at tick 0, plus the few
     init fields the scenario sets. (Full `WormSimState` parity is not required this
     slice — only what tick 0 needs; later slices widen it. See *Datamodel*.)
   - Empty `Pool<T>` instances for bonuses / wobjects / sobjects / nobjects /
     bobjects, with the iteration contract chosen now (see *Pool model*).
   - `hash::{hash_game_state, hash_components, ComponentHashes}`.
2. **C++ oracle dumper** `src/tools/oracle_dump/sim_dump.cpp` (CMake-gated, links
   `game`): construct a `Game`, seed RNG, **load a fixed `.lev`** (not random-gen),
   add 2 worms + `InitWeapons` + `ResetWorms`, then dump tick-0 `HashGameState` and
   each `HashGameComponents` field — **before** calling `ProcessFrame`.
3. **Golden** `rust/oracle-tests/golden/sim_slice1.txt` + `gen_sim_golden.sh`
   + test `rust/oracle-tests/tests/sim_slice1_golden.rs`.

**NOT included** (later slices): `ProcessFrame`, `ProcessPhysics`, any per-entity
`Process`, weapon firing, terrain destruction, bonus drop, ninjarope, death/respawn,
game-mode logic, random level *generation*, the full multi-tick time series. Those
are Slices 2–6.

## The tick-0 state (what the oracle and Rust must agree on)

At tick 0, after the C++ fixture setup (`test_determinism.cpp:22–92`) but **before**
the first `ProcessFrame`:

- `rand`: seeded with a fixed seed; `rand.last == 0` (Seed resets `last`; no RNG
  consumed yet — the level is *loaded*, not generated, so generation's RNG draws
  never happen).
- `cycles == 0`.
- level `material_id`: the bytes of the loaded `.lev` (already proven by Step 1b).
- 2 worms, each: `pos == (0,0)`, `vel == (0,0)`, `aiming_angle == 0`,
  `health == settings.health`, `lives == settings.lives` (set by `ResetWorms`),
  `kills == 0`, `timer == 0`, `visible == false`, `control_states.Pack() == 0`
  (no input applied yet), `killed_timer == 150` (`kKilledTimerInitial`),
  weapons initialised by `InitWeapons` (each `ww.type` set, `ammo == type->ammo`,
  `delay_left == 0`, `loading_left == 0`), `ninjarope.out == false`,
  `ninjarope.pos == (0,0)`.
- all object pools empty (no bobjects, bonuses, sobjects, nobjects, wobjects).

This is a clean, fully-reproducible state with **no RNG draws and no motion** — ideal
for proving the harness.

> **Why a loaded level, not `GenerateFromSettings`.** Random generation consumes RNG
> heavily and would (a) move `rand.last` off 0 and (b) require porting the generator
> — out of Step 2 scope (overview, *What we do NOT decide here*). Loading a fixed
> `.lev` keeps `rand.last == 0` and reuses the level bytes Step 1b already proved, so
> Slice 1 isolates exactly the new harness logic. The dumper loads via `Level::load`,
> as `level_dump.cpp` already does.

## Datamodel (the frame-0 sim state, idiomatic Rust)

Mirrors the `fast_snapshot.hpp` inventory but **only the subset tick 0 needs**, in
idiomatic Rust. Lives in the `sim` crate.

```text
SimState {
    rand:    sim_core::rng::Rand,     // seeded; last accessible for the hash
    cycles:  i32,
    level:   LevelSim { width: i32, height: i32, material_id: Vec<u8> },
    worms:   Vec<WormState>,          // 2 in the fixture
    bonuses:  Pool<Bonus>,            // empty this slice
    wobjects: Pool<WObject>,          // empty
    sobjects: Pool<SObject>,          // empty
    nobjects: Pool<NObject>,          // empty
    bobjects: BoboolPool<BObject>,    // empty (free-during-iter flavour)
}

WormState {
    pos: Vec2, vel: Vec2,             // sim-core Vec2 (IVec2), fixed-point
    aiming_angle: Fixed,
    health: i32, lives: i32, kills: i32, timer: i32,
    visible: bool,
    killed_timer: i32,
    control_states: ControlState,     // 7-bit, Pack()/Unpack()
    weapons: [WormWeapon; NUM_WEAPONS],
    ninjarope: Ninjarope { out: bool, pos: Vec2 },
    // (later slices add the rest of WormSimState)
}
WormWeapon { ty: Option<WeaponId>, ammo: i32, delay_left: i32, loading_left: i32 }
ControlState(u32)                     // istate & 0x7f; pack() -> u32
```

- `LevelSim` carries only `width/height/material_id` — the hash reads only
  `material_id` and the dimensions to bound the loop. (`materials`/`display_*` are
  derived/render and omitted, per the C++ snapshot's own omissions.)
- `WeaponId` is the weapon's `id` (array index in `assets::Objects::weapons`); the
  hash uses `type->id`. `InitWeapons` sets `ww.type` to
  `weapons[weap_order[settings.weapons[j]-1]]` for the selectable weapons — Slice 1
  may model `weap_order` as identity if the chosen fixture's `weap_table`/order makes
  it so, **or** read the real `Objects` to resolve it (decide in the plan; the hash
  only needs the resulting `id` and `ammo`). The values must match what C++
  `InitWeapons` produces for the chosen settings.
- `ControlState` reproduces `worm.hpp`'s `Pack()` (`istate`, masked to 7 bits).

## State hash (`sim::hash`), mirroring `stateHash.hpp`

`hash_game_state(&SimState) -> u32` accumulates **in C++ order** with
`h = h.wrapping_mul(31).wrapping_add(field as u32)` and, for the level,
`h = h.wrapping_mul(33) ^ (byte as u32)`:

1. `h = 1`
2. `+ rand.last()`, `+ cycles as u32`
3. level: for `i in 0..width*height`: `h = h.wrapping_mul(33) ^ material_id[i] as u32`
4. per worm (in vector order): `pos.x, pos.y, vel.x, vel.y, aiming_angle, health,
   lives, kills, timer, visible (as u32 0/1), control_states.pack()`; then per
   weapon `ammo, delay_left, loading_left, ty.id` (id pushed only if `ty.is_some()`,
   matching `if (weapon.type)`); then `ninjarope.out (0/1), ninjarope.pos.x,
   ninjarope.pos.y`
5. bobjects (pool order): `pos.x, pos.y`
6. bonuses: `x, y, timer, weapon, frame`
7. sobjects: `id, cur_frame`
8. nobjects: `pos.x, pos.y, vel.x, vel.y, cur_frame, ty.id (if set)`
9. wobjects: `pos.x, pos.y, vel.x, vel.y, cur_frame, time_left, ty.id (if set)`

`hash_components(&SimState) -> ComponentHashes` reproduces the per-subsystem subset
(`rng = rand.last`; `level` as above; `worms[i]` = `pos.x, pos.y, vel.x, vel.y,
health, lives, visible, timer`; `bobjects` = pos; `bonuses` = `x,y,timer,weapon`;
`sobjects` = `id,cur_frame`; `nobjects` = pos; `wobjects` = pos). At Slice 1 the pool
hashes all reduce to their empty-seed value (`h = 1`) — that is itself part of what we
verify.

**Fidelity traps (must reproduce exactly):**
- `wrapping_mul`/`wrapping_add` everywhere — the C++ relies on `uint32_t` overflow.
- signed→unsigned: C++ `static_cast<uint32_t>(int)` == Rust `(x as i32) as u32`
  (two's-complement reinterpret); use `as u32` on the `i32`/`Fixed` field, **not**
  on a widened value.
- empty-pool hash seed is `1` (not `0`) for the component hashes; the master hash
  threads through one running `h`.
- visible/`ninjarope.out` are bools hashed as their 0/1 int value.

## Pool model (chosen now, exercised empty)

Per the overview, a `Pool<T>` in `sim` reproducing C++ `FixedObjectList::All()`:
fixed-capacity backing store, live/free tracking, `spawn`/`free`, and `iter()` that
yields live slots in **slot order** (the order C++ `All()` and the hash use). A
second flavour for `BObjectList` (free-during-iteration blood pool). Slice 1
instantiates them empty, so only construction + an empty `iter()` are tested; the API
is settled here so Slices 2–5 add objects without reshaping state.

## C++ oracle dumper (`oracle_dump_sim`)

New target under the existing `OPENLIERO_BUILD_ORACLE_DUMP` block in `CMakeLists.txt`
(after `oracle_dump_wav`), linking `game`, following `level_dump.cpp`'s style.

It must reproduce the tick-0 fixture deterministically:

1. `PrecomputeTables();`
2. `Common common; common.load("data/TC/openliero");` (real TC — gives weapons /
   constants / settings defaults). `Settings settings;` with the fixture's choices
   (`game_mode = kGmKillEmAll`, `lives`, `loading_time = 0`); seed `rand`.
3. Load a **fixed** level via `Level::load` (e.g. shipped
   `data/TC/openliero/Levels/<small fixed level>.lev`) into `game.level` — *not*
   `GenerateFromSettings`. (Decide the exact file in the plan; reuse what
   `level_dump.cpp` opens if convenient.)
4. Add 2 worms exactly as the fixture: `w->settings = settings.worm_settings[idx];
   w->health = w->settings->health; w->index = idx; w->stats_x = idx? 218 : 0;`
   `AddWorm`. `InitWeapons`. `ResetWorms`. (Viewports are **not** needed — we never
   call `ProcessFrame`, so the banner/shake code that reads viewports never runs.)
5. Dump one record: the seed, `width height`, `HashGameState(game)`, and each
   `HashGameComponents` field (`rng level worms0 worms1 bobjects bonuses sobjects
   nobjects wobjects`), as hex — **before** any `ProcessFrame`.

The Rust test builds the same `SimState` (load the same `.lev` via `assets::level`,
same worm init, same seed) and asserts `hash_game_state` + every component equal the
golden line. The level `material_id` digest is already covered by `level_golden`, so
the value the sim test adds is: the *initial worm contribution*, the *empty-pool
contribution*, `rand.last==0`, `cycles==0`, and that the Rust hash arithmetic matches.

## Golden format

One line (single tick-0 record), space-separated hex:

```
<seed> <width> <height> <state_hash> <rng> <level> <worm0> <worm1> <bob> <bon> <sob> <nob> <wob>
```

(Extendable to multiple records if the plan picks more than one seed/level. Hashes
are the engine's `uint32_t` — emit as `%08x`.)

## Definition of done

- [ ] `sim` crate compiles, depends only on `sim-core` + `assets`, no Bevy/float.
- [ ] `SimState` builds from `assets::LevelData` + worm-init; pools instantiate empty.
- [ ] `hash_game_state` / `hash_components` implemented with `wrapping_*` + `as u32`.
- [ ] `sim-core::Rand` exposes a `last()` read accessor (Risk R1).
- [ ] `oracle_dump_sim` is a committed CMake target under
      `OPENLIERO_BUILD_ORACLE_DUMP`; `gen_sim_golden.sh` regenerates the golden.
- [ ] `cargo test` green: `sim_slice1_golden` matches C++ at tick 0 for the chosen
      seed(s)/level — full state hash **and** every component hash.
- [ ] CI (`rust.yml`, `cargo test --workspace`) runs it against the committed golden.

## Open questions (decide in the plan)

1. **Fixed level choice.** Which shipped `.lev` (small, classic, no MODERNLV needed
   — only `material_id` matters). Default: the one `level_dump.cpp` already loads, or
   a small classic level for a short hash loop.
2. **`weap_order` resolution.** Does the chosen `Settings` make
   `weapons[j].type->id` trivially the selected index, or must the dumper/Rust read
   the real `weap_order` from `Objects`/`Common`? The hash only needs the resulting
   `id`/`ammo`; pick the simplest faithful option.
3. **One record vs several.** A single seed/level is enough to prove the harness;
   the plan may add a second seed/level for confidence (cheap — no dynamics).
4. **`Pool<T>` capacity source.** Use the C++ pool limits (from `Common`/`Settings`,
   e.g. `bobjects` = `blood_particle_max`) so capacities match, even while empty.

## Next artifact

The TDD plan: `plans/2026-06-28-liero-rs-step2-slice1-plan.md`.
