# Step 2, Slice 5a — Splinters (`BlowUpObject` splinter arm): Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking. **Test-first**: write the failing
> test (or its assertion) before the implementation it pins.

**Goal:** Turn on the **`WObject::BlowUpObject` splinter arm** (`weapon.cpp:96-114`,
the O9 carry-over) for **`cannon` → `medium_explosion` + 5 splinters**, so the Rust
sim reproduces the C++ master `HashGameState` **and** all 9 component hashes
**tick-for-tick** — adding exactly **one** new RNG cluster (`splinter_amount` ×
`[rand(128) + rand(2) + Create2]`) on top of the already-proven explosion path. **No
C++ dumper change.** Worm-damage/blood (O10) is 5b; bonuses 5c; death/respawn 5d.

**Architecture:** Extend `rust/sim/` only (deps unchanged: `sim-core`, `assets`;
Bevy-free, float-free). The single sim change is in `weapon.rs::blow_up`: replace the
`debug_assert!(splinter_amount <= 0)` tripwire (`weapon.rs:414-417`) with the
`scatter==0` Create2 splinter loop (calling the already-ported `nobject_create2`),
keeping the `scatter!=0` `Create1` branch guarded. **No `SimState::new` signature
change** ⇒ slices 1–4d goldens stay byte-identical. Then a new scenario + golden +
`sim_slice5a_golden` difftest, mirroring 4c.

**Tech stack:** Rust only (`sim` extend, `oracle-tests`). Golden regenerated locally
via the **unchanged** dumper (`OPENLIERO_BUILD_ORACLE_DUMP`); CI (`cargo test
--workspace`) runs the committed golden. `data/TC/openliero` real TC; weapon **cannon**,
sobject **medium_explosion**, splinter nobject **particle__small_damage**.

## Global constraints

- **Bit-exact vs C++.** Sources of truth: `weapon.cpp:78-125` (`BlowUpObject`,
  splinter arm `:96-114`), `nobject.cpp:51-66` (`Create2`, already ported),
  `sobject.cpp:16-228` (`SObjectType::Create`, proven 4c), `stateHash.hpp:72-110`
  (master folds) + `:179-210` (component folds), `exactObjectList.hpp:36-94` (pool).
- **RNG order is the contract.** At the cannon explode tick, in order: `medium_explosion`
  `SObject::Create` runs its **proven** stream (sound `rand(4)` → dirt-throw row-major
  per-`AnyDirt` `rand(8)`/on-0 `rand(128)`+`Create2` → crater `draw_dirt_effect`
  `rand(2)`), **then** the splinter loop: per splinter (×5) `rand(128)` (angle) →
  `rand(2)` (colour-sub) → `Create2`[`rand(speed_v)=rand(140)` **first**, then
  `rand(distribution*2)=rand(4000)` ×2]. Splinters come **after** the sobject, **before**
  cannon's own `dirt_effect` (`=-1` ⇒ none). Thread the one `sim-core::Rand`; never
  pull ad hoc.
- **cannon Fire = 2 rand** (`distribution=300` ⇒ spread x,y). `startFrame=83>=0` with
  `loopAnim=false` ⇒ **no** frame draw; `timeToExploV=0`, `leaveShells=0` ⇒ none.
  Verify against `weapon.cpp:16-76` (the `rng` column moves by exactly 2 draws at fire).
- **`Create2` reused verbatim** (`nobject.rs`, 4c) — the splinter arm only *calls* it,
  passing `vel=(0,0)`, fixed `pos=(kX,kY)`, `colour = splinter_colour - sub`. Confirm
  the splinter uses the **fixed** `pos` (no `Ftoi`), unlike `sobject_create`'s
  `Ftoi(pos)`.
- **Worms inert (O10 posture).** All worms outside `medium_explosion`'s
  `detectRange=14`px box ⇒ no `DoDamage`/blow-away/blood, **no `bobjects`**. The
  `cycles=0` posture is unchanged (no `++cycles`).
- **cannon `affect_by_explosions=true` (O22).** `medium_explosion`'s blow-away loop
  nudges the still-pooled cannon wobject (rand-free, already ported `sobject.rs:189-228`);
  the wobject is freed the same tick **before** the hash ⇒ **hash-neutral**. Keep
  free-after; **do not** reorder the free in 5a (that lands in 5b/5d). Assert the
  neutrality holds (golden matches).
- **`scatter!=0` → `Create1` splinter branch guarded (O18).** No TC weapon hits it
  (`mini_nuke` uses the special `small_nukes` type, out of scope). Port it behind a
  `debug_assert!`/guard and unit-test it against a separately seeded `Rand`.
- **Pools.** `Pool<NObject>` cap 600; `spawn` asserts `Some` (O3 deferred). cannon
  throws 5 splinters + dirt-debris ⇒ keep well under 600.
- **Only `material_id` + the pool folds are hashed.** No sound/screen_flash/shake/stats.
- **Truncating division / shifts.** `Ftoi`=`>>16` (arith), `Itof`=`<<16`; `/100`, `/3`
  are Rust `/`, never `>>`. Same discipline as 4a–4d.
- **Scenario is the single source of truth**, read by both the (unchanged) dumper and
  the Rust test. Golden regen LOCAL/MANUAL; `PRESET` defaults `macos-arm64`.
- **No AI / "Generated with" taglines.** **Bash discipline:** one command per call; no
  `>>`/heredoc/`&&`/`;`/`$VAR` chaining; no `cd`+`git`; create files with the editor.

## File structure

- `rust/sim/src/weapon.rs` — `blow_up` splinter arm (replace tripwire); unit tests.
- `rust/oracle-tests/scenarios/sim_slice5a_scenario.txt` — new scenario.
- `rust/oracle-tests/gen_sim_slice5a_golden.sh` — faithful 4c-copy (LOCAL/MANUAL).
- `rust/oracle-tests/golden/sim_slice5a.txt` — committed golden.
- `rust/oracle-tests/tests/sim_slice5a_golden.rs` — the difftest (milestone).

## Tasks

### T0 — Splinter arm port (`scatter==0` Create2 loop + guarded `scatter!=0`)

- [ ] **RED:** add `blow_up` unit tests against a separately seeded `Rand` (hand-stepped,
      non-tautological): (a) `scatter==0`, `splinter_amount=N` spawns **N** nobjects of
      `splinter_type`, drawing exactly `N × [rand(128), rand(2), rand(speed_v),
      rand(dist*2)×2]` in that order (assert the post-loop `rand.last` and the spawned
      count/colours `splinter_colour - sub`); (b) `splinter_amount=0` spawns none, draws
      none; (c) a `scatter!=0` case hits the guarded `Create1` branch (separate test,
      `#[should_panic]` on the debug guard, or assert the Create1 draw shape if ported
      live behind the guard).
- [ ] **GREEN:** replace `weapon.rs:414-417` tripwire with the `weapon.cpp:96-114` arm:
      `if splinter_amount > 0 { if scatter==0 { for _ in 0..n { angle=rand(128);
      sub=rand(2); nobject_create2(&nobject_types[splinter_type], angle, ZERO_VEL, pos,
      splinter_colour - sub, owner_idx, ...) } } else { /* Create1 branch, guarded */ } }`.
      Keep it **between** the `create_on_exp` block and the `dirt_effect` block.
- [ ] Verify slices 1–4d still green (the arm is dormant for all prior weapons:
      fan/greenball/dart/handgun `splinter_amount=0` ⇒ guard not entered).
- [ ] Reviewer (Opus): line-by-line vs `weapon.cpp:96-114` — `scatter==0` vs `!=0`
      branch split, `rand(128)`/`rand(2)` order, `splinter_colour - rand(2)` (not the
      reverse), fixed `pos` (no `Ftoi`), `Create2` call shape, the guard on `scatter!=0`.
      Confirm no `SimState::new` signature change.

### T1 — Scenario + gen script + committed golden

- [ ] Author `sim_slice5a_scenario.txt`: seed 42, `physics_fall_test.lev`,
      `weapon 0 cannon <ammo>`, worm1 invisible + far, worm0 fires cannon into the
      floor, **no L+R**, all worms outside `medium_explosion` `detectRange=14` at the
      explode tick. Tune aim/fire ticks so the arc lands in dirt.
- [ ] `gen_sim_slice5a_golden.sh` = faithful copy of `gen_sim_slice4c_golden.sh`
      (exec, LOCAL/MANUAL, `PRESET` default `macos-arm64`), pointing at the **unchanged**
      dumper.
- [ ] Generate + commit `golden/sim_slice5a.txt`. Inspect the numbers directly:
      `rng` flat `00000000` until fire → +2 draws at fire → moves again at explode
      (sound→dirt-throw→crater→**5 splinters**); `level` carves once at explode
      (`dirtEffect=1`); `nobjects` non-empty at/after explode (dirt-debris **+ 5
      splinters**); `sobjects` the `medium_explosion` cluster, freed at end; `wobjects`
      cannon in flight → freed at explode; `bobjects`/`bonuses` empty all rows; worm0/
      worm1 unchanged across explode. **WEAPON NAME = `CANNON` (uppercase)** for the
      directive.
- [ ] Reviewer (Opus): verify the golden is self-consistent and the splinter
      signature (nobject count jump ≥5 at explode) is present.

### T2 — `sim_slice5a_golden` difftest (MILESTONE)

- [ ] Mirror `sim_slice4c_golden.rs`: expected parsed from the golden (all 11 columns);
      actual from a genuinely driven `SimState` (real `.lev`/`tc.cfg`/`Objects::load`,
      **cannon by name**, `id==index` for weapons/sobject_types/nobject_types,
      `SimState::new` full args); components asserted **before** master; input keyed
      `k-1`; all ticks incl. tick 0.
- [ ] Coverage guards from driven state (non-vacuous): `nobjects` count jumps by **≥5**
      at the explode tick; `nobjects < 600` (O3); `level` changes exactly once;
      `bobjects`/`bonuses` empty every tick; worm0 `health==100` unchanged across the
      explode tick (no damage path).
- [ ] **Milestone:** master + all 9 component hashes bit-exact every tick. If it fails,
      `systematic-debugging` against the component column that diverges (splinter draws
      localise via `rng` + `nobjects` master; recall `nobjects` component folds pos-only
      — O11).
- [ ] Reviewer (Opus): honesty (expected from golden, actual from driver), non-vacuous
      guards, splinter RNG order, "could it pass while the sim is wrong?" check.

### T3 — (optional, droppable) non-default `loading_time>0` golden (O19)

- [ ] Add a small scenario/golden (or a focused difftest assertion) exercising a
      multi-tick reload countdown so `ComputedLoadingTime` (`weapon.cpp:8-14`,
      `max(s*lt/100,1)`) is golden-covered, not unit-only (4d shipped with the dumper's
      `loading_time=0`). Independent of splinters; **drop if it bloats 5a**.
- [ ] Reviewer (Opus) if pursued.

### T4 — Controller done-check + docs/ledger

- [ ] `cargo test --workspace` green (incl. `sim_slice5a_golden` + slices 1–4d).
- [ ] `sim` float-free (`grep f32/f64` empty), deps = `sim-core`+`assets` only.
- [ ] Slices 1–4d goldens **byte-identical** (git diff empty over 5a commits) — pure-
      Rust slice proof.
- [ ] Update `docs/superpowers/liero-rs-PROGRESS.md` + the overview's slice-ordering /
      slice-5 decomposition row (5a DONE) + the SDD ledger.
- [ ] Then broad whole-slice review (Opus) → push 5a to PR #3 + update PR body.

## Done-report (each task)

(a) what changed + why, (b) files touched, (c) tests/risks. Per-task commit, local.
Push the whole sub-slice to PR #3 after the broad review.
