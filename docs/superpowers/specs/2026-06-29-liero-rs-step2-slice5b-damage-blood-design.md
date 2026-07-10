# Step 2, Slice 5b — Worm damage + blood (the O10 headline)

Status: **draft for review** · 2026-06-29
Part of: `2026-06-29-liero-rs-step2-slice5-object-families-overview.md`
Follows: `2026-06-29-liero-rs-step2-slice5a-splinters-design.md`
(the proven `sim` crate, `oracle_dump_sim_physics` with its `weapon <slot> <name>
[ammo]` directive, the `process_frame` driver, `sobject_create`/`NObject::Create2`,
`draw_dirt_effect`, the splinter arm — all live since 4a–5a.)

## Purpose

5b is the **headline sub-slice of Slice 5** (O10): it makes a worm that stands inside
an explosion **take damage** (a hashed `health` move) and **bleed** — a fan of blood
**nobjects** (type 6) whose **blood-trail** drips **bobjects** (the blood pool goes
live for the first time). It is the first slice where a **worm component MOVES** at a
sim tick and the first where the **`bobjects`** master/component column is non-empty.

5b turns **three** previously-dormant things live, all reached when a worm is hit by
the **`medium_explosion` sobject** (the cannon explosion from 5a, now with a worm in
range):

1. `Game::DoDamage` / `DoDamageDirect` / `DoHealingDirect` (RNG-free) — `game.cpp:546-589`.
2. The **sobject worm-damage arm** (`sobject.cpp:47-114`) — currently the
   `debug_assert!` tripwire at `sobject.rs:182`.
3. The **blood-trail → bobjects** path: `NObject`'s blood-trail arm
   (`nobject.cpp:95-97`, tripwire at `nobject.rs:373`) → `Game::CreateBObject` +
   `BObject::Process` (`bobject.cpp`) + the `bobjects` driver loop.

It is **not** the thinnest slice — it crosses the three Slice-5 traps at once (stats,
`cycles`, the new pool) — but they are coupled: you cannot hit a worm headless without
the stats fix, and blood-trail cadence is meaningless without `cycles`. The worm is
**wounded, not killed** (O20); death/respawn is 5d.

### What changes vs Slice 5a

| Invariant | 5a | **5b** |
|---|---|---|
| sobject worm-damage arm (`w.health>0`) | `debug_assert!` tripwire (O10) | **live** — `DoDamage` + blood fan + `rand(3)` sound |
| `DoDamage`/`DoDamageDirect`/`DoHealingDirect` | not ported | **ported** (RNG-free) |
| blood-trail arm (`nobject.cpp:95-97`) | `debug_assert!(!blood_trail)` | **live** — `CreateBObject` on `cycles % 10 == 0` |
| `BObject::Process` + `bobjects` driver loop | dormant (empty pool) | **live** — pool goes non-empty (first time) |
| worm `health`/`vel` at a tick | never moves | **moves** at the hit tick (hashed) |
| `cycles` | frozen `0` (dumper + Rust) | **advances** (`++cycles`, O17) — see the ripple below |
| C++ dumper | unchanged | **changed** (O15 stats-fix + O17 `++cycles`) |
| `SimState` | unchanged signature | gains a **`blood: i32`** field (read only in the damage path) |

## ⚠️ The O17 `cycles`-fold ripple — RIPPLE, not transparent (controller gate)

**Finding: advancing `cycles` DOES change the *master* hash of every prior scenario
(slices 1–5a). This is a real ripple. It must clear the controller before T0.**

Evidence, cited:

- `stateHash.hpp:19` folds `cycles` **directly** into the master `HashGameState`:
  `h = h * 31 + static_cast<uint32_t>(game.cycles);` — and it is folded **second**
  (right after `rand.last`, before all level/worm/object state), so any change to
  `cycles` scrambles the entire master accumulator multiplicatively. You **cannot**
  hand-patch the new master from the old; it must be regenerated.
- The **component** hashes do **not** fold `cycles` (`HashGameComponents`,
  `stateHash.hpp:129-213`; the `rng` component is just `rand.last`, `:132`).
- The prior goldens were generated with `cycles` frozen at `0` (dumper comment
  `sim_physics_dump.cpp:17-23`; the loop at `:290-328` deliberately omits `++cycles`).
  The golden column layout (`:273`) is `<tick> <master> <rng> <level> <worm0> <worm1>
  <bobjects> <bonuses> <sobjects> <nobjects> <wobjects>` — **column 2 is the master**;
  columns 3–11 are the components.

The brief's O17 audit ("`++cycles` does **not** change their goldens") is correct for
the **RNG stream and all 9 component columns** — no slice-1–5a nobject uses
`blood_trail`/`leave_obj` with `delay>0`, so the gated emissions never fire and the
draw order is identical — **but it does not cover the master column**, which folds
`cycles` unconditionally. So:

- **Components (cols 3–11): byte-identical** old vs new — the real physics invariant.
- **Master (col 2): changes** for every tick ≥ 1 of every prior scenario.

The literal 5a Definition-of-Done gate "slices 1–N goldens **byte-identical** (git
diff empty)" therefore **cannot hold** once `++cycles` lands. It must be **replaced**
by: *"the 9 component columns of slices 1–5a are byte-identical old↔new; only the
master column (col 2) changes, and it changes consistently with the `cycles` fold
(re-derived only by re-running the dumper)."*

**Resolution (recommended): regenerate the 8 prior goldens** (`sim_slice1`,
`sim_slice2`, `sim_slice3`, `sim_slice4a`, `sim_slice4b`, `sim_slice4c`, `sim_slice4d`,
`sim_slice5a`) with the new `++cycles` dumper, asserting the component-columns-identical
gate above. Both sides (dumper + Rust `process_frame`) then advance `cycles` and agree.

**Rejected alternative:** keep `cycles=0` for the prior scenarios by making `++cycles`
a per-run choice. It contradicts adjudicated O17 (which wants correct blood-trail
cadence), and it forks `process_frame` so prior slices run a *different* driver than
5b — losing the single-shared-driver property. Do not take this path without the
controller overriding O17.

**This requires the controller before T0 because it (a) mutates 8 committed golden
files and (b) redefines the prior-slice regression gate that 1–5a DoDs stated as "git
diff empty".** T0 below is written assuming the controller approves regeneration.

## Scope

### IN — ported this slice

- **`Game::DoDamageDirect` / `DoHealingDirect` / `DoDamage`** (`game.cpp:546-589`),
  all RNG-free. `DoDamageDirect` (`:546-553`): `if(amount>0){ w.health-=amount;
  if(w.health<=0) w.last_killed_by_idx=by_idx; }`. `DoDamage` (`:567-589`) ==
  `DoDamageDirect` in `kGmKillEmAll` (the ScalesOfJustice redistribution at `:571-587`
  is mode-gated; the dumper sets `kGmKillEmAll` at `sim_physics_dump.cpp:190`, so no
  rand, no healing). `health` is hashed (master `:31`, component `:149`).
- **The sobject worm-damage arm** (`sobject.cpp:47-114`) — replace the `sobject.rs:182`
  tripwire with the live body. RNG order (verified, `sobject.cpp:92-111`):
  ```
  if (w.health > 0) {                                    // :92
      DoDamage(w, z, owner_idx);                          // :93  (no rand)
      kBloodAmount = settings.blood * power_sum / 100;    // :96  (blood=100 default)
      for i in 0..kBloodAmount {                          // :99
          angle = rand(128);                              // :100
          nobject_types[6].Create2(angle, w.vel/3, w.pos, // :101  colour 0, w.index
                                   0, w.index, fired_by);  //       (Create2 draws
      }                                                   //       rand(speedV=40) +
      if (rand(3) == 0) {                                 // :105  rand(dist*2=40000)x2)
          snd = 18 + rand(3);                             // :106-107 ALWAYS drawn on 0
          if (!IsPlaying(&w)) Play(snd, &w);              // :108-109 (sound, not hashed)
      }
  }
  ```
  The vel-kick (`:60-80`) and `z = damage*power_sum/detect_range` (`:82-85`) are
  already ported (5a left them live; only the `health>0` body was deferred). `rand(3)`
  at `:105` is **always** drawn whenever `health>0`; the inner `rand(3)` (`:106`) is
  drawn only when the gate is 0. Both feed sound (not hashed) but **consume RNG** — the
  order is the contract.
- **Blood-trail → bobjects.** `NObject::Process` blood-trail arm (`nobject.cpp:95-97`):
  `if (blood_trail && blood_trail_delay>0 && (cycles % blood_trail_delay)==0)
  CreateBObject(pos, vel/4);` — replace the `nobject.rs:373` tripwire. blood.cfg:
  `bloodTrail=true, bloodTrailDelay=10, explGround=true, speed=75, speedV=40,
  distribution=20000, gravity=700, hitDamage=0, detectDistance=0`.
  - **`Game::CreateBObject`** (`bobject.cpp:7-15`): `obj.color =
    rand(NumBloodColours)+FirstBloodColour` (**1 rand/bobject**), `pos`/`vel` set.
    Only caller = the blood-trail arm. `color`/`vel` are **not** hashed.
  - **`BObject::Process`** (`bobject.cpp:17-49`): `pos += vel`; off-map → free (no
    rand); `vel.y += BObjGravity` if `Background`; on landing (`c in 1..2 / 77..79` →
    `SetPixel(77+rand(3))`; `AnyDirt` → `82+rand(3)`; `Rock` → `85+rand(3)`) ≤ 1
    `rand(3)` then free. Writes a level pixel (hashed via `level`).
  - **`bobjects` driver loop** (`game.cpp:349-355`): `if (i->Process(*this)) ++i; else
    bobjects.Free(i)` — swap-remove via `BloodPool::Free`. The dumper already mirrors
    this (`sim_physics_dump.cpp:308-314`); the Rust `process_frame` must drive it live.
- **`++cycles`** in the Rust `process_frame` at the `game.cpp:357` point (AFTER the
  four object loops `:334-355`, BEFORE the bonus roll `:359` and worm loop `:364`).
  Mirror the dumper change. (`SimState.cycles` already exists, `state.rs:570`; today
  `process_frame` reads it as a value and never mutates — `state.rs:805-808`.)
- **`SimState` gains `blood: i32`** (= 100, the `Settings::blood` default,
  `settings.hpp:70`; the dumper never overrides it, `sim_physics_dump.cpp:189-199`).
  Read **only** in the sobject damage arm. Thread it through every `SimState::new`
  caller; slices 1–5a stay green because their scenarios never enter the damage path.

### OUT — explicitly deferred

- **wobject worm-hit body** (`weapon.cpp:287-326`, inside `WObject::Process`) → stays
  deferred. The 5b scenario triggers damage via the **sobject explosion** path, not a
  direct projectile hit; the cannon wobject is freed at explode. (One new live damage
  path per the thin-vertical discipline.)
- **nobject worm-hit body** (`nobject.cpp:166-203`, tripwire `nobject.rs:478`) → stays
  deferred. It is gated on `hit_damage>0` (`:167`); blood (type 6) has `hitDamage=0`
  and `detectDistance=0` (blood.cfg), so blood nobjects **never** reach it. No splinter
  is aimed at a worm in 5b. The over-approximate box `CheckForSpecWormHit` (5a) is
  untouched — the exact per-pixel `Worm()` test lands when a slice needs an nobject to
  damage a worm (5d/later).
- **Death / respawn** → 5d. **Bonuses** → 5c (the bonus-drop roll and bonus loop stay
  out of the dumper subset).
- **The free-before-`blow_up` reorder** (O22) → still free-after; no surviving
  `affect_by_explosions` object makes ordering hash-relevant in 5b.

## Datamodel

- **`BObject`** already exists (`state.rs:405`, "blood particle, hash reads `pos`") and
  the `BloodPool` (Slice-1 swap-remove `FastObjectList`) is in place; 5b makes its
  `Process` + the driver loop live.
- **`SimState.blood: i32`** — the one new field (default 100). A `SimState::new`
  signature change ⇒ all callers updated; slices 1–5a goldens unaffected by *this*
  field (it is read only in the damage arm, which their scenarios never enter). This is
  **independent** of the `cycles` ripple above.
- Blood **nobject type 6** loads from the shipped TC (`nobjects/blood.cfg`); `Create2`
  is already ported.
- Hash contract (cited): `bobjects` fold **pos.x, pos.y only** (master
  `stateHash.hpp:55-56`, component `:160-161`); `color`/`vel` **not** hashed. The
  swap-remove slot order is the entire `bobjects` contract.

## Scenario + golden

- **`sim_slice5b_scenario.txt`** — seed 42, `physics_fall_test.lev` (the 5a fixture),
  `weapon 0 cannon [ammo]`. Place a worm **inside** `medium_explosion`'s
  `detectRange=14` box at the explode tick so the worm-damage arm fires. `damage=10`
  (medium_explosion.cfg) ⇒ `z = 10 * power_sum / 14 ≤ 10` against `health=100` ⇒ the
  worm is **wounded, not killed** every tick (O20) with default health. `blood=100` ⇒
  `kBloodAmount = power_sum` (several blood nobjects). Run **enough ticks past the hit
  (> 10)** that the blood nobjects survive to a `cycles % 10 == 0` tick and drip
  **bobjects** (else the `bobjects` column never goes non-empty and the slice is
  vacuous). Tune aim/fire timing so the shell explodes adjacent to the placed worm.
- **`golden/sim_slice5b.txt`** — N+1 rows × 11 columns. Expected signature: `rng` flat
  until fire → moves at fire → at explode moves a **large** burst (sound `rand(4)` →
  dirt-throw → crater `rand(2)` → **per blood: `rand(128)` + `Create2`'s 3 draws** →
  `rand(3)` sound-gate [+ `rand(3)` on 0]); `worm` column **moves** at the hit tick
  (`health` down, `vel` kicked) and the master moves with `cycles` every tick;
  `bobjects` **non-empty** from the first `cycles % 10 == 0` tick after blood spawns
  (with `rand(3)` colour draws folded into `rng`, and landing pixels into `level`);
  `nobjects` gains type-6 blood; `bonuses` empty. **Plus** the 8 regenerated prior
  goldens (T0). Inspect the numbers directly (4b/4c discipline).

## Difftest (the 5b milestone)

`sim_slice5b_golden.rs` — mirror `sim_slice5a_golden.rs`: expected parsed from the
golden (all 11 columns); actual from a genuinely driven `SimState` (real
`.lev`/`tc.cfg`/`Objects::load`, **cannon by name**, `id==index` for all three tables,
`SimState::new` full args incl. the new `blood`); components asserted **before**
master; input keyed `k-1`; all ticks incl. tick 0. Non-vacuous coverage guards from
driven state:
- the hit worm's `health` is **`> 0` every tick** (wounded not killed, O20) **and
  `< 100` after the hit tick** (damage actually landed — a real witness, not vacuous);
- `bobjects` count **> 0** on at least one tick (the pool goes live — first time);
- `nobjects` gains ≥ 1 type-6 (blood) nobject at the hit tick; `nobjects < 600` (O3);
- `bonuses` empty every tick.
**Milestone:** master + all 9 component hashes bit-exact for every tick vs the C++
golden, **and** the 8 regenerated prior goldens stay green (components byte-identical
old↔new; only their master column changed).

## Definition of Done

1. `cargo test --workspace` green (incl. `sim_slice5b_golden` + slices 1–5a against the
   **regenerated** goldens).
2. Dumper: base `StatsRecorder` installed (O15) + `++cycles` at the `game.cpp:357`
   point (O17). Rust `process_frame` mirrors `++cycles` at the same point.
3. `DoDamage`/`DoDamageDirect`/`DoHealingDirect` ported (RNG-free); sobject damage arm
   tripwire (`sobject.rs:182`) replaced; blood-trail tripwire (`nobject.rs:373`)
   replaced; `BObject::Process` + `CreateBObject` + the `bobjects` driver loop live.
4. **Prior-slice gate (REDEFINED, controller-approved):** slices 1–5a goldens'
   **component columns byte-identical** old↔new; **only the master column changed**
   (the `cycles` fold). NOT "git diff empty" — see the ripple section.
5. `SimState.blood` threaded; wobject/nobject worm-hit bodies + free-before reorder +
   exact per-pixel `Worm()` test remain deferred (justified above).
6. `sim` stays float-free + deps = `sim-core` + `assets` only.
7. Overview O10 (worm damage + blood) + O15/O16/O17/O20 recorded resolved in the SDD
   ledger; the `cycles` ripple resolution noted.

## Open questions

- **O17 cycles fold → RIPPLE (escalated).** Confirmed: master changes for all prior
  scenarios; resolution = regenerate prior goldens with the components-identical gate.
  **Needs controller sign-off before T0** (mutates 8 committed goldens + redefines the
  regression gate). This is the one item the planner could not adjudicate alone.
- **O15 → base `StatsRecorder`.** Confirmed no-op (`stats_recorder.cpp:8-29`);
  `NormalStatsRecorder::DamageDealt` crashes headless (`:53`, `worm_frame_stats.back()`
  on an empty vector). Member is `game.stats_recorder` (a `shared_ptr<StatsRecorder>`);
  install `std::make_shared<StatsRecorder>()` after `Game game(...)` at
  `sim_physics_dump.cpp:202`. Resolved.
- **O16 → no viewport nesting.** Confirmed: the viewport loop (`sobject.cpp:27-33`) does
  only `v.shake`; the worm-damage loop (`:47-114`) and blow-away loops (`:118-186`) are
  not viewport-nested. Resolved.
- **O20 → wound not kill.** Resolved by `medium_explosion damage=10` vs `health=100`.

## Tasks

See the companion plan (`plans/2026-06-29-liero-rs-step2-slice5b-plan.md`).
Sketch: **T0** dumper O15+O17 + **regenerate prior goldens** (controller-gated) →
**T1** Rust `++cycles` + `SimState.blood` (prior tests green vs regen goldens) →
**T2** `DoDamage*` port → **T3** sobject damage arm live → **T4** blood-trail →
`CreateBObject` + `BObject::Process` + `bobjects` driver loop → **T5** scenario + gen +
golden → **T6** `sim_slice5b_golden` difftest (MILESTONE) → **T7** done-check + ledger.
