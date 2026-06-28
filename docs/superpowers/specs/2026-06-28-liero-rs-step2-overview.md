# Step 2 — Sim core: overview / altitude decisions

Status: **draft for review** · 2026-06-28
Part of: `2026-06-26-liero-rs-roadmap.md`
Detailing: the Step 2 section of `2026-06-26-liero-rs-steps2-5-preliminary-breakdown.md`

This is the Step 2 architecture/strategy decision document — one level more concrete
than the preliminary breakdown, but **not** a per-slice spec and **not** a TDD task
list. It locks the cross-cutting decisions every slice of Step 2 inherits (crate
graph, where the sim lives, how the time-series oracle works, the pool model, the
ordering discipline) so each slice spec can be written against a stable foundation.
The first slice's real spec and plan are companion documents (see *Next artifact*).

Step 2 is the crown-jewel step: it ports `Game::ProcessFrame` and the per-entity
`Process()` logic into Rust so that, given identical seed + level + per-frame input,
the Rust sim reproduces C++'s `HashGameState` **tick by tick** over a long run. This
is where determinism is won or lost.

## What exists to build on

- **`sim-core`** (dependency-free, no float): `fixed` (16.16), `vec` (`Vec2` = IVec2),
  `math` (integer sqrt / vector length), `tables` (precomputed sin/cos), `rng`
  (ported MT19937, restorable, `last` tracked). The locked, bit-exact base.
- **`assets`** (serde/toml, no Bevy): `level::LevelData` (+ material map, 1a–1c),
  `palette`, `sprite`, `tc::TcConfig` (constants `c[]`, material flags, hacks `h[]`,
  textures, bonuses, AI params — 1e-1), `object::{Weapon, NObjectType, SObjectType,
  Objects}` (1e-2), `wav` (1e-3). Everything `processFrame` reads is parsed.
- **`oracle-tests`**: the golden differential-test pattern — a CMake-gated C++ dumper
  (`OPENLIERO_BUILD_ORACLE_DUMP`, links the `game` lib) emits FNV-1a digests; a Rust
  test reproduces them. Seven dumpers exist (`oracle_dump_level/palette/sprite/tc/
  object/wav`) wired in `CMakeLists.txt:372–385`; per-slice `gen_*_golden.sh` +
  `golden/*.txt` + `tests/*_golden.rs`.

## The C++ oracle, precisely (read from source, to be mirrored)

### `Game::ProcessFrame` order (`src/game/game.cpp:267`)

Verified against the source. One tick is, in this exact order:

1. `stats_recorder->PreTick` (stats; **not** sim state — skip in the port).
2. `if (screen_flash > 0) --screen_flash;` (in `GameSnapshot`, **not** in the hash).
3. viewport shake decay for `viewports` and `spectator_viewports` (render-only;
   `shake -= 4000`).
4. **bonuses**: `for each bonus: bonus->Process(*this)`.
5. banner gate `if ((cycles & 1) == 0)`: adjust `viewport.banner_y` /
   `spectator_viewport.banner_y` from `killed_timer` (render-only).
6. **sobjects**: `for each: Process`.
7. **wobjects**: `for each: Process`.
8. **nobjects**: `for each: Process`.
9. **bobjects**: `for each: if (!Process) Free(it)` (blood; pool free on death).
10. `++cycles;`
11. bonus-drop RNG roll: `if (!h[HBonusDisable] && max_bonuses>0 &&
    rand(c[CBonusDropChance]) == 0) CreateBonus();` — **consumes RNG every tick**.
12. **worms**: `for each: worm->Process(*this)`.
13. **ninjaropes**: `for each: worm->ninjarope.Process(*worm, *this)`.
14. game-mode logic (`switch settings->game_mode`): tag/holdazone timers etc.
15. (viewport follow + `store prev controls` happen in the controller around the
    tick, not inside `ProcessFrame`; the prev-control store is sim-relevant and is
    captured by `prev_control_states` in `WormSimState`).

The read-after-write chain s→w→n→b before worms, then the RNG roll *between*
`++cycles` and the worm loop, is load-bearing: a single reordered or extra `rand()`
desyncs everything downstream.

### The two oracle hashes (`src/game/stateHash.hpp`)

`HashGameState(Game&)` — one `uint32_t`, the full per-tick checksum. Accumulates,
in order, with `h = h*31 + field` (and `h = h*33 ^ byte` for the level):

- `rand.last`, then `cycles`;
- level: every `material_id[i]` over `width*height` (`h*33 ^ byte`);
- per worm (in `game.worms` order): `pos.x, pos.y, vel.x, vel.y, aiming_angle,
  health, lives, kills, timer, visible, control_states.Pack()`; then per weapon
  (`NUM_WEAPONS`): `ammo, delay_left, loading_left, type->id` (id only if type set);
  then `ninjarope.out, ninjarope.pos.x, ninjarope.pos.y`;
- bobjects (pool order): `pos.x, pos.y`;
- bonuses: `x, y, timer, weapon, frame`;
- sobjects: `id, cur_frame`;
- nobjects: `pos.x, pos.y, vel.x, vel.y, cur_frame, type->id`;
- wobjects: `pos.x, pos.y, vel.x, vel.y, cur_frame, time_left, type->id`.

`HashGameComponents(Game&)` — a `ComponentHashes` struct of per-subsystem hashes
(`rng, level, worms[2], bobjects, bonuses, sobjects, nobjects, wobjects`) using a
*subset* of the fields above. This is the **divergence-localising superpower**: when
a tick mismatches, the component hash names the subsystem and the existing C++
deep-compare diagnostics (`test_determinism.cpp:341–565`) name the field.

> Note both hashes read `rand.last`. In Rust `Rand::last` is currently private
> (`sim-core/src/rng.rs:34`); Step 2 must expose a read accessor. (Risk R1.)

### The state inventory (`src/game/serialization/fast_snapshot.hpp`)

`WormSimState` is the authoritative list of per-worm sim fields (pos, vel,
logic_respawn, hotspots, aiming_angle/speed, the able/movable/animate/visible/ready/
flag bools, health/lives/kills, timer/killed_timer, current_frame, flags, ninjarope,
current_weapon, last_killed_by_idx, fire_cone, leave_shell_timer, reacts[4],
weapons[NUM_WEAPONS], direction, control_states, prev_control_states, steerable_*,
index). `GameSnapshot` adds the game-level set: `rand, cycles, screen_flash,
last_killed_idx, got_changed, holdazone, worms[2]`, the four object pools (Bonus /
WObject / SObject / NObject lists), the BObject array + count, level `material_id`
(`level_data`) and `display_valid`. This is the checklist of "what counts as sim
state" — the Rust hash and snapshot must cover it (display_data/materials are derived
and intentionally omitted; we follow the same omissions).

## Locked decisions (inherited by every slice)

These are fixed for Step 2; slices build on them rather than re-litigate them.

1. **The sim is a pure Rust module, Bevy-free.** It is driven by Bevy only in
   Step 3+. `ProcessFrame` order, RNG sequence, and fixed-point math are entirely
   under our control. The sim is **not** ECS-native: Bevy `Vec2`/`Transform`
   (float) may never appear in it. (Roadmap charter + breakdown "Bevy trap".)
2. **Tick architecture: a sim-core-driver tick, not ECS-native systems.** The tick
   is one ordered Rust function (`SimState::process_frame`) that calls per-entity
   `process` helpers in the exact C++ order. We deliberately reject ECS-native
   systems-with-ordering for the determinism-critical core — the breakdown's open
   question is resolved toward the de-risking option. Step 5's `GgrsSchedule` will
   call this one function; the sim stays a plain `(state, input) -> state`.
3. **Pools stay pool-shaped, in the sim module.** Objects (b/n/s/wobjects, bonuses)
   are modelled as fixed-capacity pools with stable, free-list iteration order
   mirroring C++ `FixedObjectList::All()` / `BObjectList` — **not** Bevy entities
   with an ordering component. Deterministic spawn/free/iteration order is a
   correctness requirement, and the pool model gives it to us for free, exactly as
   C++ has it. (Resolves the breakdown's pool open question.) See *Pool model*.
4. **Oracle = per-tick state checksum time series, not static golden.** The C++
   dumper (a new `oracle_dump_sim`, same `OPENLIERO_BUILD_ORACLE_DUMP` lineage,
   links `game`) runs a seeded + scripted/fuzzed scenario for N ticks and dumps
   `HashGameState` **and** every `HashGameComponents` field **per tick**. Rust runs
   the same scenario and matches line-for-line. Per-component hashes are the
   debugging tool at divergence.
5. **Charter: modernise, don't transliterate.** Locked bit-exact: the math /
   fixed-point / RNG / tables / ordering and all sim-affecting data. Free:
   idiomatic Rust (`Result`, slices, `from_le_bytes`, methods on a `SimState`) — not
   a mirror of the C++ class layout. The oracle proves behaviour is preserved.

## Crate graph decision: a new `sim` crate

**Recommendation: add a new `sim` crate; do not extend `sim-core`.**

`sim-core` is, by roadmap mandate, **dependency-free** — it isolates the locked
determinism primitives (fixed/vec/math/rng/tables) from churn, and currently has an
empty `[dependencies]`. The sim module must read `assets` types (`LevelData`,
`Objects`, `TcConfig`, `Weapon`/`NObjectType`/`SObjectType`), and `assets` pulls in
`serde` + `toml`. Folding the sim into `sim-core` would drag `assets` (and serde/toml)
into the dependency-free crate, violating its charter and coupling the primitives to
the data-format layer. So:

```
sim-core   (no deps: fixed, vec, math, rng, tables)
   ▲   ▲
   │   └────────────┐
assets              sim            ← NEW crate
(serde, toml)   deps: sim-core, assets
   ▲                ▲
   └──────┬─────────┘
      oracle-tests   (dev-deps: sim-core, assets, sim)
```

- `sim` depends on `sim-core` (math/rng/pools-primitives/Vec2) and `assets`
  (the parsed data the tick reads). It owns the sim datamodel, the pools, the
  `ProcessFrame` port, and the state hash.
- `assets` is unchanged (it does not depend on `sim-core` today and need not).
- `oracle-tests` gains `sim` as a dev-dependency; the time-series differential
  tests live there beside the existing golden tests.
- The future Bevy `game` crate (Step 3) will depend on `sim` and drive it; because
  the hash lives in `sim`, the headless oracle and the game binary share one
  implementation. (Resolves "where does the Rust state-hash live".)

## Where the state hash lives

In the `sim` crate: `sim::hash::{hash_game_state, hash_components}` operating on
`sim::SimState` (and a `ComponentHashes` mirror of the C++ struct). This is the one
shared implementation used by (a) the headless oracle tests now and (b) the Step 3+
game binary and Step 5 ggrs checksum later. It must reproduce the C++ accumulation
(`wrapping_mul`, `i32 as u32` casts, `h*31` / `h*33 ^`) bit-for-bit.

## Pool model

C++ object pools (`Game::BonusList`/`WObjectList`/`SObjectList`/`NObjectList` and the
`BObjectList`) are fixed-capacity with `NewObject()`/`Free()` and an `All()` range
that yields live objects in stable slot order with free-list reuse. The hashes and
the `test_determinism` deep-compares iterate in exactly this order, so iteration
order is part of the contract.

**Recommendation:** implement a small generic `Pool<T>` in the `sim` crate (or
`sim-core` if it proves dependency-free and reusable) reproducing this contract:
contiguous backing storage, an explicit live/free marker or free-list, `spawn`/`free`,
and an `iter()` that visits live slots in slot order matching C++ `All()`. Keep the
two pool flavours C++ has (the `All()`-range pools for b/n/s/wobjects+bonuses; the
`Begin/End/Free`-during-iteration `BObjectList` for blood). At Slice 1 every pool is
empty, so only the *representation and iteration contract* are exercised — but they
are chosen now so later slices add objects into a settled structure. Spawn/free order
must be driven from the ported `Process` logic in C++ order, never from Bevy.

## Slice ordering (thin-vertical, then widen)

Each slice is independently differential-testable against a C++ component hash.
Refines the breakdown's six-step ordering:

1. **Level → sim-state + state-hash harness (frame 0, no dynamics).** Build
   `SimState` from `assets::LevelData`; stand up `hash_game_state`/`hash_components`;
   match C++ at tick 0 before any motion. Proves the harness + the level + initial
   worm + empty-pool halves of the checksum. **← DONE & bit-exact** (new `sim` crate,
   `oracle_dump_sim` dumper, golden `sim_slice1.txt`; the Rust tick-0 `HashGameState`
   reproduces C++ `ae317bb5` and every component hash bit-for-bit on real TC data).
2. **One worm, physics only.** Port `Worm::ProcessPhysics` (gravity, terrain
   collision via material flags, fixed-point pos/vel); match the worm component hash
   under scripted input. **← DONE & bit-exact** (`sim::physics`: `CheckedMatWrap`
   port, `calculate_reaction_force`, reaction orchestration + `process_physics` +
   `process_worm_physics` driver; new per-tick `oracle_dump_sim_physics` time-series
   oracle, golden `sim_slice2.txt`; Rust matches C++ worm component hash tick-for-tick
   over 101 ticks incl. a floor bounce. Master hash carried un-asserted until slice 3.)
3. **Worm control + aiming.** The rest of `Worm::Process` minus combat (movement,
   aim, jump/dig, direction, weapon-change); match the full worm hash. **← DONE &
   bit-exact** (`sim::control`: `process_aiming`/`process_tasks`/`process_weapons`/
   `process_weapon_change`/`process_movement`; `process_worms` per-worm pass; the
   **master** `state_hash` turned ON here, golden `sim_slice3.txt` matches C++
   tick-for-tick over 146 ticks incl. aim/jump/weapon-change/ninjarope phases.)
4. **One weapon, full lifecycle.** `Worm::Fire` → `WObject::Process` (move, collide,
   explode) → terrain destruction → resulting `SObject`/`NObject`. Simplest
   projectile first; the roadmap's headline milestone. **← 4a + 4b DONE & bit-exact**
   (**4a = fan**, the explodes-into-nothing projectile: `worm_fire`/`weapon_fire`/
   `wobject_process`/`blow_up`, the driver promoted to a ProcessFrame subset
   `process_frame` (object loops before worms + Fire gate), C++ dumper extended with
   object loops + a `weapon` directive, golden `sim_slice4a.txt` matches C++ over 93
   ticks — **RNG now live**, level still pristine. **4b = greenball** terrain
   destruction: `draw_dirt_effect` (the `DrawDirtEffect` blit port, both `n_draw_back`
   branches), `blow_up`'s `dirt_effect` branch, `settings->shadow=false` in the dumper
   (O4), golden `sim_slice4b.txt` matches C++ master+components over 91 ticks — **the
   `level` hash is now a live time series** (`95f63601→ddd76202→63307ba3` across two
   shots). 4c sobjects+nobjects / 4d Slice-3 deferrals planned next. 4a's milestone
   also surfaced + fixed a latent `if(visible)` worm-gate bug.)
5. **Remaining object families.** nobjects (incl. splinters), sobjects (blast →
   terrain + worm damage), bobjects (blood), bonuses (spawn/pickup). Each vs its
   component hash.
6. **Full `ProcessFrame` integration + game mode.** Wire entities in exact C++
   order, add `cycles`, the bonus-drop RNG roll, ninjarope, death/respawn, and
   Kill-em-all. **Milestone:** full `HashGameState` matches C++ for N>1000 ticks
   under fuzzed input (mirror of `test_determinism.cpp`'s 2-worm 1000-frame loop).

## The hard 10% (risks carried across the step)

- **RNG as ordered, shared state.** `rand` is consumed mid-tick (bonus-drop roll,
  fire spread, splinters, respawn search). Call order must match C++ exactly; thread
  `sim-core::Rand` through the tick in C++ order, never pull ad hoc.
- **No float, ever.** Any `f32` in the sim is a latent cross-machine desync. The sim
  stays in `sim-core` integer/fixed types; Bevy float types are Step 3 render-only.
- **Iteration / spawn / free order.** Made deterministic by keeping the pool model
  (decision 3) rather than ECS query order.
- **Death/respawn + level-dependent RNG search** (`Worm::BeginRespawn`,
  `worm.cpp:711`) is the known desync-sensitive path — its RNG search reads live
  level pixels; port carefully and fuzz it (Slice 6; the C++ death-fuzz test is the
  template).
- **Hash arithmetic fidelity.** The C++ hash relies on `uint32_t` wraparound and
  `int → uint32_t` casts; Rust must use `wrapping_*` and `as u32` with identical
  semantics or the checksum diverges even when the state is correct.

## Scenario / input-vector format (established here, used by every slice)

- **Input vector:** one snapshot per tick per worm, the 7-bit `ControlState`
  (`worm.hpp` `Pack`/`Unpack`, bits: Up=0 Down=1 Left=2 Right=3 Fire=4 Change=5
  Jump=6), exactly as `test_determinism.cpp` feeds `input_rng() & 0x7f` then
  `Unpack`. The scenario file is `seed`, the level source, and a per-tick list of
  per-worm 7-bit inputs (scripted for early slices, fuzzed for Slice 6). Frame 0
  consumes no input, but the format is fixed now so later slices only add ticks.
- **Worm setup** mirrors the C++ fixtures: 2 worms, `health = settings.health`,
  `index`, `stats_x ∈ {0,218}`, `InitWeapons`, `ResetWorms`
  (`killed_timer = 150`, `visible = false`, `lives = settings.lives`). Whether a
  1-worm scenario is meaningful or the 2-worm fixture is mandatory is an open
  question (the component hash has `worms[2]`); default to the 2-worm setup the C++
  fixtures assume.
- **Coverage** (final matrix per-slice): a few seeds × a fixed loaded level ×
  scripted inputs early; multiple seeds × fuzzed inputs × Kill-em-all at Slice 6.

## What we do NOT decide here

- The full coverage matrix (seed/level/mode count) for Slice 6 — sketched per slice,
  finalised at that slice's spec.
- Exact `Pool<T>` API surface — settled in the Slice 1 spec (where it is first
  instantiated, empty).
- Random level *generation* (`Level::GenerateFromSettings`) — **out of Step 2's
  critical path**: the oracle loads a fixed `.lev` so the level material map is the
  one Step 1b already proved, keeping RNG pristine at tick 0. Porting the random
  generator is a separate concern (it consumes RNG heavily); Step 2 does not require
  it.

## Next artifact

The first slice's detailed spec and plan (companion documents):
- `specs/2026-06-28-liero-rs-step2-slice1-level-and-statehash-design.md`
- `plans/2026-06-28-liero-rs-step2-slice1-plan.md`
