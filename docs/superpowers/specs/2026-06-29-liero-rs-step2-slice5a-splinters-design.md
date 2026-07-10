# Step 2, Slice 5a — Splinters: `WObject::BlowUpObject`'s splinter arm

Status: **draft for review** · 2026-06-29
Part of: `2026-06-29-liero-rs-step2-slice5-object-families-overview.md`
Follows: `2026-06-28-liero-rs-step2-slice4d-deferrals-design.md`
(the proven `sim` crate, `oracle_dump_sim_physics` with its `weapon <slot> <name>
[ammo]` directive, the `process_frame` driver, `sobject_create`/`NObject::Create2`,
`draw_dirt_effect` — all live since Slice 4).

## Purpose

5a is the **first sub-slice of Slice 5** and the thinnest possible widening: it turns
on the **one** projectile path Slice 4 left guarded — the **`WObject::BlowUpObject`
splinter arm** (`weapon.cpp:96-114`, the O9 carry-over). Everything else it touches
(`SObject::Create`, the dirt-throw, `NObject::Create2`, `draw_dirt_effect`, the
pools, `WObject::Process`) is already bit-exact from 4a–4d. So 5a adds exactly **one
new RNG cluster** — `splinter_amount` × `[rand(128) + rand(2) + Create2]` — against
the same harness, with **no C++ dumper change** and **none of the three Slice-5 traps**
(stats / `cycles` / viewport).

Weapon: **`cannon`** (overview *Chosen weapons*, O18) — the only `splinterAmount>0`
weapon with `shotType=0`, no `objTrail`/`partTrail`, and `bounce=0`, so the only new
draws on top of a proven explosion are its 5 splinters.

### What changes vs Slice 4d

| Invariant | Slices 4a–4d | **5a** |
|---|---|---|
| `BlowUpObject` splinter arm | `debug_assert!(splinter_amount <= 0)` tripwire | **live** — `scatter==0` → 5× `[rand(128)+rand(2)+Create2]` |
| `nobjects` pool contents | dirt-debris / shell only | + splinter nobjects (`particle__small_damage`) |
| wobject blow-away kick (`affect_by_explosions`) | only ever run with `affect=false` (skipped) | first **live** run via cannon `affect=true` (rand-free, **hash-neutral**) |
| C++ dumper | — | **unchanged** (cannon + `[ammo]` already supported) |
| `cycles` / stats / viewport | n/a | **untouched** (worms out of range; no `++cycles`) |

## Scope

### IN — ported this slice

- **`BlowUpObject` splinter arm** (`weapon.cpp:96-114`) — replace the
  `weapon.rs` `blow_up` tripwire (`weapon.rs:414-417`) with the real loop:
  ```
  if (kSplinters = w.splinter_amount) > 0 {                 // weapon.cpp:96
      kColour = w.splinter_colour                           // :97
      if w.splinter_scatter == 0 {                          // :99  ← cannon path
          for i in 0..kSplinters {                          // :100
              angle = rand(128)                             // :101
              sub   = rand(2)                               // :102
              nobject_types[w.splinter_type].Create2(       // :103-105
                  game, angle, vel=(0,0), pos=(kX,kY),
                  kColour - sub, owner_idx, fired_by)
          }
      } else { /* scatter!=0 → Create1, GUARDED (O18) */ }  // :107-114
  }
  ```
  Placement is **between** `create_on_exp`'s `sobject_create` and the
  `dirt_effect` `draw_dirt_effect` branch (the existing `blow_up` order is already
  correct — the loop slots in where the tripwire is). `kX/kY` = the cached pre-free
  wobject centre (`weapon.cpp:82-83`); in `blow_up` that is the `pos` parameter (an
  `Ftoi` is **not** applied — `Create2` takes fixed `pos`; verify against
  `sobject_create`'s `Ftoi(pos)` contract — splinters use the **fixed** `pos`, the
  sobject uses `Ftoi`). `vel = ()` (zero) is passed to `Create2`, which then computes
  the splinter velocity from `rand(speed_v)` + `cossin[angle]` (`nobject.cpp:51-66`).
- **`Create2` is already ported** (`nobject.rs`, 4c) — the splinter arm only *calls*
  it. Per splinter `Create2` draws `rand(speed_v)=rand(140)` then (distribution=2000)
  `rand(4000)` ×2. `start_frame=0`/`time_to_explo_v=0` ⇒ no further draws.
- **Optional fold (O19): the non-default `loading_time>0` golden.** 4d shipped with
  the dumper's `loading_time=0` (instant reload). Add a small scenario/golden (or a
  focused unit assertion) that exercises a multi-tick reload countdown
  (`loading_left` decrements over N ticks) so the `ComputedLoadingTime` path
  (`weapon.cpp:8-14`, `max(s*lt/100,1)`) is golden-covered, not unit-only. Independent
  of splinters — sequence it as a separate task, droppable if it bloats 5a.

### OUT — explicitly deferred

- **Worm damage / blood / bobjects** → 5b (O10; needs the stats fix + `cycles`).
- **Bonuses** → 5c. **Death / respawn** → 5d.
- **`scatter!=0` → `Create1` splinter branch** → guarded + unit-tested only; no TC
  weapon hits it (`mini_nuke` uses `scatter=1` with the special `small_nukes` type,
  out of scope). Land live only when a slice/TC needs it (O18, the ProcessSight
  omission pattern).
- **The free-before-`blow_up` reorder** → kept as **free-after** here (O22). cannon's
  `affect_by_explosions=true` nudges the still-pooled cannon wobject in
  `medium_explosion`'s blow-away loop, but the nudge draws **no rand** and the wobject
  is freed the same tick **before** the hash ⇒ **provably hash-neutral** (the
  `material_id`/pool state at hash time is identical whether the kick happened or
  not). Document the neutrality in the difftest; land the free-before reorder in
  5b/5d, where a *surviving* `affect_by_explosions` object makes the ordering
  hash-relevant.

## Datamodel

**None.** `Weapon` already carries `splinter_amount`, `splinter_colour`,
`splinter_scatter`, `splinter_type` (4c). `NObject::Create2` exists. `SObjectType`
`medium_explosion` loads from the shipped TC. No `SimState::new` signature change ⇒
slices 1–4d stay **byte-identical** (a required re-diff gate).

## Scenario + golden

- **`sim_slice5a_scenario.txt`** — seed 42, `physics_fall_test.lev` (the 4b/4c
  sky-over-floor fixture), `weapon 0 cannon [ammo]`, worm1 invisible + far (out of
  `medium_explosion` `detectRange=14`), worm0 fires cannon down into the floor, **no
  L+R** (no dig). Aim/fire timing tuned so the cannon arcs, hits the dirt, and
  explodes with worm0 also outside `detectRange` (no `DoDamage`). A faithful copy of
  the 4c gen script (`gen_sim_slice5a_golden.sh`, LOCAL/MANUAL).
- **`golden/sim_slice5a.txt`** — N rows × 11 columns. Expected signature:
  `rng` flat `00000000` until fire, then moves at fire (cannon Fire = 2 draws) and
  again at explode (sound `rand(4)` → dirt-throw → crater `rand(2)` → **5 splinters
  ×5 draws**); `level` carves once at explode (`medium_explosion` `dirtEffect=1`);
  `nobjects` non-empty at/after explode (dirt-debris **+ 5 splinters** in flight);
  `sobjects` the `medium_explosion` frame cluster; `wobjects` the cannon in flight,
  freed at explode; `bobjects`/`bonuses` **empty** all rows; worm0/worm1 unchanged
  across explode (no damage). Inspect the golden numbers directly (4b/4c discipline).

## Difftest (the 5a milestone)

`sim_slice5a_golden.rs` — mirror `sim_slice4c_golden.rs`: expected parsed from the
golden (all 11 columns), actual computed from a genuinely driven `SimState` (real
`.lev`/`tc.cfg`/`Objects::load`, **cannon by name**, `id==index` for all three tables,
`SimState::new` full args), components asserted **before** master, input keyed `k-1`,
all ticks incl. tick 0. Coverage guards from driven state (non-vacuous): `nobjects`
count jumps by **≥5** at the explode tick (the splinters), `nobjects` < 600 (O3),
`level` changes once, `bobjects`/`bonuses` empty, worm0 `health==100` unchanged across
explode (no damage path entered). **Milestone:** master + all 9 component hashes
bit-exact for every tick vs the C++ golden.

## Definition of Done

1. `cargo test --workspace` green (incl. `sim_slice5a_golden` + slices 1–4d).
2. `blow_up` splinter tripwire replaced by the `scatter==0` Create2 loop; the
   `scatter!=0` `Create1` branch guarded + unit-tested (discriminating, hand-stepped
   against a separately seeded `Rand`).
3. Slices 1–4d goldens **byte-identical** (git diff empty over the 5a commits) — 5a is
   a pure-Rust slice, no C++/dumper change.
4. `sim` stays float-free + deps = `sim-core` + `assets` only.
5. Overview O18 (cannon) + O22 (free-after) + O19 (loading_time fold) resolved in this
   design; recorded in the SDD ledger.

## Open questions — resolved here

- **O18 → cannon.** The only no-trail, `shotType=0`, generic-splinter weapon. bazooka/
  blaster/missile carry cycle-gated `objTrail`s; `Create1` (scatter≠0) branch guarded.
- **O22 → keep free-after + document neutrality.** Land free-before in 5b/5d.
- **O19 → fold the `loading_time>0` golden into 5a** as a separable, droppable task.

## Tasks

See the companion plan (`plans/2026-06-29-liero-rs-step2-slice5a-plan.md`).
Sketch: **T0** splinter arm port (replace tripwire; `scatter==0` Create2 loop +
guarded `scatter!=0`; unit tests) → **T1** scenario + `gen` script + committed golden
→ **T2** `sim_slice5a_golden` difftest (milestone) → **T3** (optional) `loading_time>0`
golden → **T4** controller done-check + re-diff + docs/ledger.
