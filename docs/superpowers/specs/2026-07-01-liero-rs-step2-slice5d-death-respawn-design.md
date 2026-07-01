# Step 2, Slice 5d — Death + respawn (the worm-loop death path goes live)

Status: **draft for review** · 2026-07-01
Part of: `2026-06-29-liero-rs-step2-slice5-object-families-overview.md`
Follows: `2026-06-29-liero-rs-step2-slice5c-bonuses-design.md`
(the proven `sim` crate; `oracle_dump_sim_physics` with the `weapon <slot> <name> [ammo]`
+ `max_bonuses <n>` directives; the `process_frame` driver with the object loops +
`++cycles` + the gated bonus-drop roll all live since 5b/5c; `sobject_create` +
`DoDamage` worm-damage arm + blood `nobject_types[6]` + the `bobjects` pool live since
5b; `NObject::Create1`/`Create2` live since 5a.)

## Purpose

5d turns on the **worm-loop death path** — the deepest and last rung of Slice 5. It ports
the four unported branches of `Worm::Process` (`worm.cpp:210-451`) that Slice 4a
deliberately left inert:

1. the **health clamp** (`:213`) and the **game-mode / lives gate** (`:215`);
2. the **pre-death blood drip** (`:355-367`) that fires while a worm is alive but under
   `settings->health/4`;
3. the **death block** (`:369-426`) — death sound, `--lives`, kill bookkeeping,
   `visible=false`, `killed_timer=150`, the `kMax`-particle blood spray and the 8-part
   worm-gib spray;
4. the **dead-worm `else` arm** (`:431-450`) — `killed_timer` countdown, `ready`-on-fire,
   **`BeginRespawn`** (`:711-742`, the level-reading RNG position search — the single most
   desync-sensitive path in Step 2) and **`DoRespawning`** (`:755-809`, the drop-in
   convergence + the lone raw `rand()&1` aiming draw).

It is the first slice where a worm's `visible` flips to `false` then back to `true`,
where `health` crosses `<=0` and is restored, and where `lives`/`kills` mutate. It earns
a **fuzz** pass because `BeginRespawn`'s trial count varies with the live level pixels and
the enemy position.

### What changes vs Slice 5c

| Invariant | 5c | **5d** |
|---|---|---|
| worm `if (visible)` arm | active-sim only (4a) | active-sim **+ pre-death drip + death block** |
| worm dead `else` arm (`:431-450`) | UNPORTED (inert) | **PORTED** — `killed_timer` countdown + `BeginRespawn` + `DoRespawning` |
| health clamp (`:213`) + lives gate (`:215`) | UNPORTED (inert) | **PORTED** (hash-neutral for priors) |
| `visible` / `health<=0` / `lives` / `kills` | constant | **mutate** (death → respawn cycle) |
| `nobjects` storm | ≤ splinter/blood counts | **death spray** `kMax=120` blood + 8 worm-gibs (nears the pool cap → O3) |
| C++ dumper | bonus loop + roll + `max_bonuses` | **NO C++ CHANGE** — the dumper already drives the unmodified `Worm::Process` |
| Rust `Pool::spawn` full-pool | `expect(...)` panic (`nobject.rs:118`) | **overwrite last slot** (C++ `NewObjectReuse`, O3) |
| prior goldens (1–5c) | byte-identical (the `max_bonuses==0` short-circuit) | **byte-identical (git diff empty)** — no dumper change at all |

## ✅ The dumper needs NO change — 5d is a pure-Rust slice (like 5a)

**Finding: unlike 5a's splinter arm (which was a Rust-only change but re-diffed the C++
goldens), 5b's stats+cycles, and 5c's bonus loop+roll+directive, Slice 5d requires ZERO
C++ dumper edits.** The death/respawn logic lives entirely inside `Worm::Process`, and the
dumper already drives the **unmodified** `w->Process(game)` for each worm every tick
(`sim_physics_dump.cpp:394-398`). The two preconditions the overview flagged are already
satisfied by defaults:

- **`quick_sim == false`** (`game.hpp:153` `bool quick_sim{false}`). `BeginRespawn` is
  gated on `killed_timer == 0 && !game.quick_sim` (`worm.cpp:443`); the dumper's
  hand-rolled `Game` never sets `quick_sim`, so it keeps the `false` default and
  `BeginRespawn`/`DoRespawning` are already reached.
- **`settings->blood == 100`** (`settings.hpp:70` `int32_t blood{100}`). The dumper never
  overrides `blood`, so the death spray is `kMax = 120*100/100 = 120` particles — the same
  default `blood` value 5b's damage-blood already used and matched.
- **The `worm <idx> <x> <y> <health> <lives> <stats_x> <visible>` directive already sets
  per-worm health + lives** (`sim_physics_dump.cpp:144`), so a low-health victim needs no
  new directive.

Consequently the **re-diff of slices 1–5c is byte-identical trivially** (there is no
dumper change to ripple). The 5a/5b/5c literal *"git diff empty"* prior-slice gate holds
again with the weakest possible justification: nothing in the C++ path moved.

**Optional (recommended only if the spray must be bounded):** add a `blood <n>` scenario
directive defaulting to `100` (= the current implicit value) that sets `settings->blood`.
Because the default equals today's implicit value, prior goldens stay byte-identical, and
5d gains a knob to tune `kMax` down (e.g. to keep `nobjects` comfortably under the 600 cap
in the milestone golden while the fuzz stresses the cap). See O3 and Scenario below. If the
default-100 spray already stays under cap in the milestone (it does — one death is
~120+8+damage-blood ≈ 150 < 600), the directive is unnecessary for the milestone and can be
skipped.

## Scope

### IN — ported this slice

- **Health clamp + game-mode/lives gate** (`worm.cpp:213,215`). `health = min(health,
  settings_health)` (always); the body runs iff `(mode != KillEmAll && mode != Scales) ||
  lives > 0`. Port the `KillEmAll` path (the openliero TC mode); keep `Scales` /
  `GameOfTag` game-mode branches present-but-guarded (the TC leaves the mode `KillEmAll`).
  Both are **hash-neutral for priors** (health already `== settings_health`; `lives > 0`
  always in 1–5c).
- **Pre-death drip** (`worm.cpp:355-367`), verified RNG order — fires at the END of the
  visible arm (after the movement/change gate, `:348-353`), gated `health <
  settings_health / 4`:
  - `rand(health + 6)` (`:356`); on `== 0`:
    - `rand(3)` (`:357`); on `== 0`: `rand(3)` sound index `18 + …` (`:358-359`, the inner
      draw is ALWAYS taken on the gate — the `IsPlaying` check that follows draws no rand);
    - **then, unconditionally within the outer gate**, `nobject_types[6].Create1(vel, pos,
      0, index)` (`:365`): `Create1` draws `rand(distribution*2)` twice (x, y) when the
      blood type's `distribution != 0` (`nobject.cpp:44-45`), then `Create` (blood
      `start_frame <= 0`, `time_to_explo_v == 0` ⇒ no further draw). **2 draws.**
  - So a drip-fire costs `rand(health+6)[=0]` + `rand(3)` + `[rand(3) sound]` +
    `rand(dist*2)×2` (4 draws if the sound gate misses, 5 if it hits). **Note:** with a
    **low-health victim** the drip fires on every pre-explosion tick — this is a *feature*,
    it exercises the drip path as a real witness (see Scenario).
- **Death block** (`worm.cpp:369-426`), verified RNG order (entered when `health <= 0` in
  the visible arm):
  - `leave_shell_timer = 0`, `make_sight_green = false`; `if (loop_sound) Stop` (no rand);
  - `rand(3)` death sound `15 + …` (`:378-379`);
  - `fire_cone = 0`, `ninjarope.out = false` (no rand);
  - game mode: `KillEmAll` ⇒ `--lives` (`:390`); `Scales` ⇒ `while (health<=0) { health +=
    settings_health; --lives; }` (guarded); (no rand);
  - `last_killed_idx` / `got_changed` bookkeeping, and **`++WormByIdx(last_killed_by_idx)
    ->kills`** if the killer is another worm (`:403-405`) — `kills` is hashed (master), so
    the killer's `kills` term must increment on the death tick (no rand);
  - `visible = false`; `killed_timer = kKilledTimerInitial (= 150, worm.hpp:243)`;
  - `kMax = 120 * settings_blood / 100`; **iff `kMax > 1`** (`:412`) — for
    `i = 1..=kMax`: `rand(128)` angle + `nobject_types[6].Create2(angle, vel/3, pos, 0,
    index)` (`Create2`: `rand(speed_v)` + `rand(dist*2)×2` when `distribution != 0` +
    `Create` frame/time — blood ⇒ **3 draws**). **Per blood particle = 4 draws; ×120 =
    480 draws** at default blood. (`blood == 1` ⇒ `kMax == 1`, NOT `> 1` ⇒ **no death
    spray** — a corner the guard must honour.)
  - **worm-gib spray** `for (i = 7; i <= 105; i += 14)` (`:418`): the iteration set is
    `{7,21,35,49,63,77,91,105}` = **8 iterations** (⚠ the overview's decomposition row
    says "7×" — it is **8**; `i <= 105` includes `105`. The `for` bound is the contract —
    verify against `worm.cpp:418` and pin the count in a unit test). Each:
    `nobject_types[index].Create2(i + rand(14), vel/3, pos, 0, index)` — the `rand(14)` is
    drawn in `worm.cpp` (the angle arg), then `Create2` on the **per-worm gib type**
    `nobject_types[index]` (index = worm index 0/1) draws its own `rand(speed_v)` +
    optional `rand(dist*2)×2` + `Create` frame/time. **⚠ The gib type's params
    (`speed_v`/`distribution`/`start_frame`/`time_to_explo_v`) differ from blood — load
    `nobject_types[0]`/`[1]` from the TC and mirror whatever sub-draws they trigger; do not
    assume the blood draw shape.**
  - `stats_recorder->AfterDeath` (base no-op) + `Release(kFire)` (no rand).
- **Dead-worm `else` arm** (`worm.cpp:431-450`): `steerable_count = 0`; `if
  (PressedOnce(kFire)) ready = true` (input, no rand); `if (killed_timer > 0)
  --killed_timer`; `if (killed_timer == 0 && !quick_sim) BeginRespawn`; `if (killed_timer <
  0) DoRespawning`.
- **`BeginRespawn`** (`worm.cpp:711-742`) + **`CheckRespawnPosition`** (`game.cpp:611-650`),
  verified RNG order:
  - `temp = Ftoi(pos)` (the death pos); `logic_respawn = temp - IVec2(80,80)`; `enemy =
    temp`; iff `worms.size() == 2` `enemy = Ftoi(worms[index^1].pos)` (**reads the LIVE
    enemy pos** — a desync input, no rand);
  - `do { pos.x = Itof(WormSpawnRectX + rand(WormSpawnRectW)); pos.y = Itof(WormSpawnRectY +
    rand(WormSpawnRectH)); ` — **2 rand per trial**, `rand(W)` **then** `rand(H)`; the
    drop-down `while (Ftoi(pos.y)+4 < height && Mat(x, y+4).Background()) pos.y += 1`
    (**reads the LIVE level, no rand**); `if (++trials >= 50000) break; } while
    (!CheckRespawnPosition(...))`;
  - `CheckRespawnPosition` (**no rand**): reject if within `WormMinSpawnDistLast` of the
    last death pos **or** `WormMinSpawnDistEnemy` of the enemy; else scan a `[x-3,x+3)×
    [y-4,y+4)` box, reject on any `Rock()` pixel; else accept;
  - `killed_timer = -1`.
  - **The trial count = f(level pixels, enemy pos)** — the canonical Step-2 desync trap.
- **`DoRespawning`** (`worm.cpp:755-809`), verified RNG order (runs every tick while
  `killed_timer < 0`):
  - 4× converge `logic_respawn` toward `Ftoi(pos) - 80` by ±1 each (no rand); `LimitXy`
    clamps to `[0, width-158]×[0, height-158]` (no rand);
  - `dest = Ftoi(pos) - 80`, `LimitXy(dest)`;
  - iff `logic_respawn` within ±5 of `dest` **and `ready`**: `DrawDirtEffect(rand, level,
    0, ipos.x-7, ipos.y-7)` (dirt draws — the 4c-ported `DrawDirtEffect`); `CorrectShadow`
    (gated `settings->shadow`, **false** in the dumper ⇒ skipped); `ready = false`; sound;
    `visible = true`; `fire_cone = 0`; `vel.Zero()`; `health = settings_health` (unless
    Scales); **the lone `rand() & 1`** (`:799`, the ONLY no-arg `rand()` in this family) →
    `aiming_angle = Itof(32), direction = 0` **or** `aiming_angle = Itof(96), direction =
    1`; `AfterSpawn` (no-op).

### OUT — deferred / kept-guarded

- **`Scales of Justice` / `Game of Tag` game-mode branches** (`worm.cpp:384-401`,
  `game.cpp:372+`) — ported-but-guarded; the TC mode is `KillEmAll`, so the scenario never
  exercises them. Keep the branch shape for fidelity; unit-test the `KillEmAll` path only.
- **Bonus interaction** — the 5d scenario keeps `max_bonuses 0` (default), so no bonus
  drops and the deferred pickup / chain-loop (5c tripwires) stay untouched.
- **`ninjarope` death interaction** beyond `ninjarope.out = false` — the rope Process loop
  (`game.cpp:368-370`) is still excluded by the dumper subset; the death block's single
  `ninjarope.out = false` write is hash-neutral (rope not in the dumped path).
- **The random-level death-fuzz** (the existing C++ `test_determinism` "Death and respawn
  determinism fuzz", 5 seeds × 5000 ticks with `GenerateFromSettings` random levels) — its
  bit-exact Rust replay would require porting `Level::GenerateFromSettings` (a large new
  surface, out of 5d scope). The 5d fuzz uses **fixed-level multi-seed** goldens instead
  (see Fuzz + O21). Porting `GenerateFromSettings` is flagged as an optional John-scope
  fork (JOHN-BESLUT KRÄVS) with a DEFER recommendation.

## Datamodel

The Rust `Worm` needs new **runtime** fields to drive the dead/respawn arm. Cross-check
each against `stateHash.hpp` — **only the already-hashed fields matter to the golden**:

- **Hashed (already on `Worm`, mutated by 5d):** `pos`, `vel`, `aiming_angle`, `health`,
  `lives`, `kills`, `visible` (master; `hash.rs:86`), and `pos`/`vel`/`health`/`lives`/
  `visible` (component). These are the fields the death→respawn cycle moves.
- **NOT hashed (new runtime fields, hash-neutral to add):**
  - `killed_timer: i32` — **already present** (`state.rs:246`, folded into WormInit but
    **absent from both hashes** — see the Hash-fold section);
  - `direction: i32` (set by `DoRespawning`'s `rand()&1`; feeds `AngleFrame`/`current_frame`
    which are themselves unhashed);
  - `logic_respawn: IVec2` (the drop-in cursor; `worm.hpp` `logic_respawn`);
  - `ready: bool` (set by `PressedOnce(kFire)` in the dead arm; the `DoRespawning`
    completion gate);
  - `last_killed_by_idx: i32` (set by the 5b damage path; read by the death block's
    `kills++`); `make_sight_green: bool`, `leave_shell_timer: i32`, `fire_cone: i32`,
    `steerable_count: i32` (written by the death/dead arm; none hashed).
  - Game-level: `last_killed_idx: i32`, `got_changed: bool` (written by the death block;
    not hashed — they feed only the game-mode switch which the dumper excludes). Model them
    minimally or omit if the `KillEmAll` path never reads them back.
- **New consts (load from TC):** `WormSpawnRectX/Y/W/H`, `WormMinSpawnDistLast`,
  `WormMinSpawnDistEnemy`, `kKilledTimerInitial = 150` (a `worm.hpp` constant, not TC). The
  per-worm gib types are already in the loaded `nobject_types` array (indices 0/1); blood
  is `nobject_types[6]` (live since 5b).
- **`SimState.blood: i32`** — **already threaded** (5b). No new `SimState` field is required
  unless the optional `blood <n>` directive is added (then it is set from the scenario,
  default 100).

## Hash-fold implications (`stateHash.hpp`)

The 5d-relevant folds (verified in `stateHash.hpp`):

| Field | Master (`HashGameState`) | Component (`HashGameComponents`) |
|---|---|---|
| `visible` | **yes** (`:35`) | **yes** (`:151`) — death/respawn flips are visible |
| `health` | **yes** (`:31`) | **yes** (`:149`) — crosses `<=0`, then restored |
| `lives` | **yes** (`:32`) | **yes** (`:150`) — `--lives` on death |
| `kills` | **yes** (`:33`) | **NO** — killer's `kills++` is master-only |
| `aiming_angle` | **yes** (`:30`) | **NO** — `DoRespawning`'s `rand()&1` result is master-only |
| `pos` / `vel` | yes | yes — `BeginRespawn` jumps `pos`; `DoRespawning` zeroes `vel` |
| **`killed_timer`** | **NO** | **NO** | 
| `direction` | **NO** | **NO** |

**The load-bearing asymmetry: `killed_timer` is invisible to BOTH hashes** (like the blood
pool's color/vel). This has three consequences the design leans on:

1. **The 150-tick dead phase is hash-silent on the timer.** Between the death tick and
   `BeginRespawn`, the victim's hashed state is *constant* (`health <= 0`, `visible =
   false`, `pos` frozen at death, `vel` whatever it was). A wrong `killed_timer` countdown
   is undetectable **directly** — but its *effect* (firing `BeginRespawn` on the wrong
   tick) IS detectable, because that tick's `rng` (global `rand.last`) jumps. So the
   countdown is pinned only transitively through when the RNG burst lands.
2. **`BeginRespawn`'s trial count is witnessed by `rng`, not by the worm hash.** The search
   draws `2 × trials` values from the shared engine; the resulting `pos` also folds. So the
   desync trap surfaces as an `rng` **and** `pos` divergence on the `BeginRespawn` tick —
   the difftest's primary non-vacuous guard (below).
3. **`nobjects` fold distinguishes blood from gibs.** Master folds `pos,vel,cur_frame,
   type→id`; the 120 blood (`type 6`) and 8 gibs (`type index`) carry different `type→id`,
   so a mis-typed or mis-counted spray localises in the `nobjects` master term while the
   component (`pos` only) still moves.

Global terms: `rng` (every RNG burst), `cycles` (every tick, since 5b), and
`material_id[]` (the `DoRespawning` `DrawDirtEffect` carve — the respawn digs a small
crater, so `level` moves on the respawn tick).

## O3 — `NewObjectReuse` full-pool overwrite (resolved here)

The death spray is the **canonical pool storm** the overview earmarked O3 for. Today
`nobject_create` panics when the pool is full:

```rust
nobjects.spawn(obj).expect("nobject pool not full in 4c (NewObjectReuse overwrite deferred)")
                              // rust/sim/src/nobject.rs:118
```

C++ `FastObjectList::NewObjectReuse` (`fastObjectList.hpp:35-44`) — and
`ExactObjectList::NewObjectReuse` (`exactObjectList.hpp:57-60`) — return **`&arr[limit-1]`**
when `count == limit`: the last slot is **overwritten in place** (no free, no swap, `count`
unchanged). The Rust `Pool::spawn` returns `None` at capacity (`pool.rs:60,140`). 5d must
replace the panic with the overwrite: **when the pool is full, write into the last slot and
return `limit-1`** (a dedicated `spawn_reuse` / `new_object_reuse` that mirrors
`NewObjectReuse`, distinct from `NewObject` which legitimately returns `None`,
`game.cpp:244-246`).

**Resolution:** port `new_object_reuse` (overwrite-last-slot) for the `nobjects` path in 5d
(its own task + unit test), so a `blood=100` storm that reaches 600 matches C++ bit-exactly
instead of panicking. Keep the **milestone golden under cap** (a single death ≈ 150
`nobjects` ≪ 600 ⇒ the overwrite never fires there — a clean first proof), and let the
**fuzz** (repeated deaths, `blood 100`) exercise the overwrite. This is why the O3 task must
land **before** the fuzz task (which would otherwise panic). Audit whether `sobjects`/
`wobjects`/`bonuses` can also storm — they cannot in 5d (bounded spawns), so their
`NewObject`/`spawn` `None` handling is unchanged.

## Dumper changes

**None required** (see the "no dumper change" finding). The dumper already drives the full
`Worm::Process` death/respawn path with `quick_sim == false` and `blood == 100`. Optional:
a `blood <n>` directive (default 100, byte-neutral) to bound `kMax` if the milestone golden
is chosen to stay demonstrably under the O3 cap without relying on a single death. **O16
(viewport count):** the death sprays and `BeginRespawn`/`DoRespawning` run inside the worm
loop (`w->Process`), **not** inside any `for (viewport)` — they are viewport-independent, so
5d inherits 5b's already-pinned viewport handling for the *killing* explosion's
(viewport-nested) sobject-damage loop and adds no new viewport dependency.

## Scenario + golden

- **`sim_slice5d_scenario.txt`** — reuse the 5b machinery, tuned to KILL then RESPAWN:
  - `seed` chosen so the killing air-burst lands mid-run and the respawn completes inside
    the window; `physics_fall_test.lev` (the open-sky-over-floor fixture, reused from
    2/3/4a/4b/4c/5a/5b/5c); `max_bonuses 0` (no bonus interaction).
  - **worm0 = killer**: full health (100), **`explosives` in slot 0** (the closed-gate,
    splinter-free weapon 5b proved), positioned so its air-burst catches worm1; a `Fire`
    input tick launches it.
  - **worm1 = victim**: **low health** (e.g. `12`, below `settings_health/4 = 25`), so (a)
    the **pre-death drip** fires on the pre-explosion ticks (exercising `:355-367` as a
    live witness) and (b) one/two `large_explosion` hits (~12 dmg each, per 5b) bring it to
    `health <= 0` ⇒ the **death block**. A later `Fire` input to the (now dead) worm1 sets
    `ready` so `DoRespawning` can complete.
  - Positions chosen so `BeginRespawn`'s `WormSpawnRect` search + `CheckRespawnPosition`
    (min-dist-from-death-pos and min-dist-from-killer) yields a **bounded, deterministic
    trial count** (worms near opposite edges ⇒ most of the level is a valid spawn ⇒ a few
    trials), and so the single death keeps `nobjects` under the 600 cap (O3 does not fire
    in the milestone).
  - **Window ~250–350 ticks**: fire (~40) → death → 150-tick `killed_timer` countdown →
    `BeginRespawn` → `DoRespawning` convergence (`logic_respawn` moves ≤4px/tick toward
    `pos-80`) → respawn (`rand()&1` + dirt carve + `visible=true`). **Tune via
    `OL_PHYS_TRACE`** so the full cycle fits; assert the respawn actually completes.
- **`golden/sim_slice5d.txt`** — N+1 rows × 11 columns
  (`<tick> <master> <rng> <level> <worm0> <worm1> <bob> <bon> <sob> <nob> <wob>`). Expected
  signature: `rng` drips on the pre-death ticks (victim drip rolls) and on `cycles`-gated
  object activity; **bursts hugely at the death tick** (`rand(3)` + 480 blood draws + 8-gib
  draws); worm1 `health`→`<=0`, `visible`→false, `lives`--, worm0 `kills`++ **at the death
  tick**; `rng` flat through the ~150 dead ticks; **bursts again at `BeginRespawn`** (`2 ×
  trials` — the trial-count witness) with worm1 `pos` **jumping**; `rng` + `level` move at
  the `DoRespawning` respawn tick (dirt carve + `rand()&1`) with worm1 `visible`→true,
  `health`→`settings_health`, `aiming_angle` set; `nobjects` non-empty from the death tick
  and draining; `bobjects` moves as the death/damage blood drips (`cycles % 10`); `bonuses`
  empty; master moves with `cycles` every tick. **Inspect the numbers directly** (4b/4c/5b/
  5c discipline). **Plus** slices 1–5c re-run **byte-identical** (no dumper change).

## Difftest (the 5d milestone)

`sim_slice5d_golden.rs` — mirror `sim_slice5c_golden.rs`: expected parsed from the golden
(all 11 columns); actual from a genuinely driven `SimState` (real `.lev`/`tc.cfg`/
`Objects::load`, **`explosives` by name**, `id == index` for the object tables,
`SimState::new` full args); components asserted **before** master; input keyed `k-1`; all
ticks incl. tick 0. Non-vacuous coverage guards from driven state:

- worm1 `health` **crosses to `<= 0`** on the death tick **and back to `settings_health`**
  after respawn (a real death→respawn, not a frozen worm);
- worm1 `visible` is `true` → `false` (death) → `true` (respawn) across the window;
- worm1 `lives` **decrements** by exactly 1; worm0 `kills` **increments** by exactly 1;
- `rng` **bursts on the death tick** (the spray) **and again on the `BeginRespawn` tick**
  (the trial-count witness — proves the level-reading search ran and matched the C++ trial
  count, the desync trap);
- worm1 `pos` **jumps** on the `BeginRespawn` tick and the respawn `level` (`material_id`)
  **carves** on the `DoRespawning` tick;
- `nobjects` count **> 0** on the death tick (blood + gibs) and drains; `nobjects < 600`
  (the milestone stays under cap — O3 not exercised here); `bonuses` empty.

**Milestone:** master + all 9 component hashes bit-exact for every tick vs the C++ golden,
**and** slices 1–5c re-run **byte-identical** (git diff empty). On failure,
`systematic-debugging` against the diverging column: death-spray count/type localises via
`rng` + `nobjects`; the trial count via `rng` + worm1 `pos`; the respawn draw via `rng` +
`level` + `aiming_angle`; recall `killed_timer` is invisible (a countdown desync shows only
as a *mis-timed* `rng` burst).

## Fuzz (O21) — `BeginRespawn` trial-count coverage

The milestone proves **one** death/respawn bit-exact. The fuzz's added value is **coverage
of the variable trial count** across many death positions / enemy positions / level
neighbourhoods. Because a bit-exact Rust replay of the *existing* random-level C++
death-fuzz would need `GenerateFromSettings` (out of scope), the 5d fuzz uses
**fixed-level, multi-seed differential goldens**:

- **Recommendation (controller-adjudicable): 4 seed/position variants × ~300 ticks**,
  reusing the T7 scenario template on the same `physics_fall_test.lev`, each seed chosen so
  the spawn search takes a **different (still bounded) trial count** (vary the killer/victim
  x so `CheckRespawnPosition`'s enemy-dist / last-pos-dist reject a different number of
  trials). Each variant gets its own golden + a difftest reusing the milestone harness.
  Rationale: exercises the desync trap's trial-count variance **vs the C++ oracle** with
  zero new dumper surface, and the fixed level keeps the search a pure function of the
  scenario (reproducible, inspectable).
- **Optionally add a pure-Rust determinism guard** (same seeds, two `SimState` runs asserted
  hash-identical every tick) — cheap, proves no accidental nondeterminism entered the
  death/respawn port. Low value on its own (integer code) but a good cheap backstop.
- The `blood 100` storms in longer/aggressive variants **require the O3 overwrite** (T6) —
  order the fuzz task after it.

The genuine scope question (random-level replay) is escalated below.

## JOHN-BESLUT KRÄVS (genuine scope forks)

1. **Fuzz fidelity — fixed-level multi-seed (recommended, in-scope) vs porting
   `Level::GenerateFromSettings` to bit-exact-replay the existing random-level C++
   death-fuzz.** The recommended fixed-level multi-seed differential (above) covers the
   `BeginRespawn` trial-count variance vs the C++ oracle **without** new surface and keeps
   5d self-contained. Bit-exact replaying the *existing* `test_determinism` death-fuzz (5
   seeds × 5000 ticks, `random_level=true`) would require porting `GenerateFromSettings`
   (level-generation RNG) into the Rust sim — a **large new surface** unrelated to death/
   respawn. **Recommendation: DEFER the `GenerateFromSettings` port to a later slice; ship
   the fixed-level multi-seed fuzz in 5d.** This is the one item that is a genuine
   scope-fork rather than controller-adjudicable — flagged for John. (If John accepts the
   recommendation, the controller can proceed with no further input.)

Everything else in 5d is **controller-adjudicable**: the no-dumper-change finding, the O3
overwrite port, the milestone scenario tuning, the 8-vs-7 gib-count (the C++ `for` bound is
authoritative), and the fixed-level fuzz seed count are all decidable from the C++ oracle +
the recommendations above.

## Deferrals (tripwire / guard, not this slice)

- `Scales`/`GameOfTag` game-mode death branches — guarded, `KillEmAll`-only exercised.
- `GenerateFromSettings` random-level fuzz replay — deferred (see JOHN-BESLUT #1).
- The 5b-deferred in-flight `CheckForSpecWormHit` / wobject+nobject in-flight worm-hit arms
  — still deferred (5d kills via the sobject explosion AABB, as 5b did).
- Bonus pickup + chain-loop — untouched (5d keeps `max_bonuses 0`).

## Tasks

See the companion plan (`plans/2026-07-01-liero-rs-step2-slice5d-plan.md`).
Sketch: **T0** dumper no-change verification + re-diff 1–5c byte-identical → **T1** Rust
worm-loop restructure (health clamp + lives gate + visible/dead arm split + new runtime
fields; priors byte-identical) → **T2** pre-death drip → **T3** death block (sound + lives/
kills + `kMax` blood spray + 8 gibs) → **T4** `BeginRespawn` + `CheckRespawnPosition` (the
desync trap) → **T5** `DoRespawning` (convergence + `rand()&1`) → **T6** O3
`new_object_reuse` overwrite (before the fuzz) → **T7** scenario + gen + golden → **T8**
`sim_slice5d_golden` difftest (MILESTONE) → **T9** fixed-level multi-seed respawn FUZZ →
**T10** done-check + ledger + PROGRESS.
