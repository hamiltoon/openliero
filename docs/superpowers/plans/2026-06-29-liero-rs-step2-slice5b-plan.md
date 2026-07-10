# Step 2, Slice 5b — Worm damage + blood (O10): Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking. **Test-first**: write the failing
> test (or its assertion) before the implementation it pins.

**Goal:** Make a worm standing inside the **`medium_explosion`** (cannon) blast **take
damage** and **bleed** — `DoDamage` (RNG-free) + a `rand(128)` blood-nobject fan +
`rand(3)` hit-sound, and the blood nobjects' **blood-trail** dripping **bobjects**
(`CreateBObject` + `BObject::Process` + the `bobjects` driver loop) — so the Rust sim
reproduces the C++ master `HashGameState` **and** all 9 component hashes
**tick-for-tick**, with the **worm wounded, not killed** (O20). Death/respawn is 5d;
bonuses 5c; the wobject/nobject worm-hit bodies stay deferred.

**Architecture:** Extend `rust/sim/` (deps unchanged: `sim-core`, `assets`; Bevy-free,
float-free). Sim changes: replace the `sobject.rs:182` worm-damage tripwire and the
`nobject.rs:373` blood-trail tripwire with their live bodies; port `Game::DoDamage*`;
make `BObject::Process` + `CreateBObject` + the `process_frame` `bobjects` loop live;
add `++cycles` to `process_frame` (`game.cpp:357` point); add `SimState.blood: i32`.
The **C++ dumper** changes twice (O15 base `StatsRecorder`, O17 `++cycles`) — the only
C++ edit. Then a new scenario + golden + `sim_slice5b_golden` difftest, mirroring 5a.

**Tech stack:** Rust (`sim` extend, `oracle-tests`) + a one-file C++ dumper edit.
Goldens regenerated **LOCALLY/MANUALLY** via the rebuilt dumper
(`OPENLIERO_BUILD_ORACLE_DUMP`); CI (`cargo test --workspace`) runs the committed
goldens. `data/TC/openliero` real TC; weapon **cannon**, sobject **medium_explosion**
(`damage=10, detectRange=14`), blood **nobject type 6** (`bloodTrailDelay=10`).

## ⚠️ Controller gate before T0 — the O17 `cycles`-fold ripple

`stateHash.hpp:19` folds `cycles` into the **master** hash. The 8 prior goldens
(`sim_slice1..5a`) were generated with `cycles=0`; `++cycles` changes their **master
column (col 2)** for every tick ≥ 1 (the 9 component columns stay byte-identical). The
prior-slice DoD "git diff empty" therefore **cannot hold**; it is **redefined** to
"component columns identical, master column regenerated." **T0 regenerates the 8 prior
goldens and REQUIRES controller sign-off** (it mutates committed goldens and redefines
the regression gate). Do not dispatch T0 without that sign-off. See the design's ripple
section for the full evidence.

## Global constraints

- **Bit-exact vs C++.** Sources of truth: `game.cpp:546-589` (`DoDamage*`),
  `sobject.cpp:47-114` (worm-damage arm), `nobject.cpp:95-97` (blood-trail arm),
  `bobject.cpp:7-49` (`CreateBObject` + `BObject::Process`), `game.cpp:334-357` (object
  loops + `++cycles`), `stateHash.hpp:19/31/55-56` (master folds `cycles`/`health`/
  `bobject pos`) + `:149/160-161` (components). RNG order is the contract.
- **RNG order at the hit tick** (`sobject.cpp:92-111`, verified): after the proven
  `medium_explosion` stream (sound `rand(4)` → dirt-throw → crater `rand(2)`), for the
  in-box worm with `health>0`: `DoDamage` (no rand) → per blood (`kBloodAmount =
  blood*power_sum/100`, `blood=100`): `rand(128)` (angle) → `Create2`[`rand(speedV=40)`
  **first**, then `rand(distribution*2=40000)` ×2] → then `rand(3)` (hit-sound gate,
  **always drawn**) → on 0, `rand(3)` (which sound). Thread the one `sim-core::Rand`.
- **`DoDamage` RNG-free.** `DoDamageDirect` (`game.cpp:546-553`); `DoDamage` ==
  `DoDamageDirect` in `kGmKillEmAll` (`:567-589`; ScalesOfJustice branch mode-gated,
  the dumper is `kGmKillEmAll`). No rand, no healing path entered.
- **`++cycles` placement.** AFTER the four object loops (`game.cpp:334-355`), BEFORE the
  bonus roll (`:359`) and worm loop (`:364`). Same point in the dumper and Rust
  `process_frame`. The blood-trail gate `cycles % 10 == 0` then fires every 10th tick.
- **`bobjects` hash = pos-only** (`stateHash.hpp:55-56`, `:160-161`); `color`/`vel` not
  hashed. The swap-remove free order (`game.cpp:349-355`, `BloodPool::Free`) is the
  whole `bobjects` contract.
- **O20 wound not kill.** `medium_explosion damage=10`, `z = 10*power_sum/14 ≤ 10`,
  `health=100` ⇒ `health` stays `> 0` every tick. Difftest asserts it (and `< 100`
  after the hit — non-vacuous).
- **Deferred (justify-in-place):** wobject worm-hit body (`weapon.cpp:287-326`);
  nobject worm-hit body (`nobject.cpp:166-203`, `nobject.rs:478` — blood has
  `hitDamage=0`/`detectDistance=0` so never reached); free-before reorder (O22); exact
  per-pixel `Worm()` test. Keep their `debug_assert!` guards intact.
- **`SimState.blood`** (= 100, `settings.hpp:70`, dumper never overrides). New
  `SimState::new` arg ⇒ update all callers; slices 1–5a stay green (read only in the
  damage arm). Independent of the `cycles` ripple.
- **Pools.** `Pool<NObject>` cap 600; blood fan ≤ ~14 + dirt-debris ⇒ well under.
  `BloodPool` swap-remove (Slice-1). `spawn` asserts `Some` (O3 deferred).
- **Truncating division / shifts.** `Ftoi`=`>>16`; `/100`, `/3`, `/4` are Rust `/`,
  never `>>`. `vel/3`, `vel/4` per the C++.
- **Scenario is the single source of truth**, read by both the dumper and the Rust
  test. Golden regen LOCAL/MANUAL; `PRESET` defaults `macos-arm64`.
- **No AI / "Generated with" taglines.** **Bash discipline:** one command per call; no
  `>>`/heredoc/`&&`/`;`/`$VAR`; no `cd`+`git`; create files with the editor.

## File structure

- `src/tools/oracle_dump/sim_physics_dump.cpp` — **C++**: base `StatsRecorder` install
  (after `Game game(...)`, ~`:202`) + `++cycles` in the driver loop (after the bobjects
  loop, ~`:315`, before the worm loop). Update the header comment.
- `rust/sim/src/game.rs` (or wherever `DoDamage*` belongs) — `do_damage` /
  `do_damage_direct` / `do_healing_direct`; unit tests.
- `rust/sim/src/sobject.rs` — replace the `:182` worm-damage tripwire with the live arm.
- `rust/sim/src/nobject.rs` — replace the `:373` blood-trail tripwire with `CreateBObject`.
- `rust/sim/src/bobject.rs` (new, or extend existing) — `BObject::Process` + `CreateBObject`.
- `rust/sim/src/state.rs` — `++cycles` + the `bobjects` driver loop in `process_frame`;
  add `blood: i32` to `SimState` + `SimState::new`.
- `rust/oracle-tests/scenarios/sim_slice5b_scenario.txt` — new scenario.
- `rust/oracle-tests/gen_sim_slice5b_golden.sh` — faithful 5a-copy (LOCAL/MANUAL).
- `rust/oracle-tests/golden/sim_slice5b.txt` — committed golden.
- `rust/oracle-tests/golden/sim_slice{1,2,3,4a,4b,4c,4d,5a}.txt` — **regenerated** (T0).
- `rust/oracle-tests/tests/sim_slice5b_golden.rs` — the difftest (milestone).

## Tasks

### T0 — Dumper (O15 + O17) + regenerate prior goldens — CONTROLLER-GATED

- [ ] **Confirm controller sign-off** on the `cycles`-fold ripple resolution
      (regenerate prior goldens; redefine the prior-slice gate). Do not proceed without it.
- [ ] **GREEN (C++):** in `sim_physics_dump.cpp`, after `Game game(...)` (~`:202`) add
      `game.stats_recorder = std::make_shared<StatsRecorder>();` (O15 — base no-op,
      `stats_recorder.cpp:8-29`; avoids the `NormalStatsRecorder::DamageDealt:53` headless
      crash). Add `++game.cycles;` in the driver loop **after** the bobjects loop
      (`:315`) and **before** the worm loop (`:317`), matching `game.cpp:357`. Update the
      file's header comment (it currently states `cycles` stays 0).
- [ ] Rebuild the dumper (`-DOPENLIERO_BUILD_ORACLE_DUMP=ON`); smoke-run it on the 5a
      scenario.
- [ ] **Regenerate** all 8 prior goldens (`sim_slice1`, `2`, `3`, `4a`, `4b`, `4c`,
      `4d`, `5a`) via their gen scripts against the rebuilt dumper.
- [ ] **Gate:** for each regenerated golden, assert the **9 component columns (cols
      3–11) are byte-identical** to the old file and **only the master column (col 2)
      differs**. (A per-column diff; the components-identical result is the proof that
      the RNG/physics streams are unchanged and only the `cycles` fold moved.)
- [ ] Reviewer (Opus): the two dumper edits (placement vs `game.cpp:357`, base-vs-Normal
      recorder), and the component-identical gate output for all 8 goldens.

### T1 — Rust `++cycles` + `SimState.blood` (prior tests green vs regen goldens)

- [ ] **RED:** assert (unit) that `process_frame` increments `cycles` once per tick at
      the `game.cpp:357` point; assert `SimState.blood == 100` after `new` for the
      dumper config.
- [ ] **GREEN:** add `++cycles` to `process_frame` (after the four object loops, before
      the bonus/worm logic; `state.rs:805-808` currently reads `cycles` as a value —
      now mutate it). Add `blood: i32` to `SimState` + `SimState::new` (= 100); thread
      through every caller.
- [ ] Slices 1–5a difftests **green against the regenerated goldens** (master now folds
      the advancing `cycles`; components unchanged). This is the Rust side of the T0 gate.
- [ ] Reviewer (Opus): `++cycles` placement, no double-increment, `blood` threaded to
      all callers, prior difftests green vs regen goldens.

### T2 — `DoDamage` / `DoDamageDirect` / `DoHealingDirect` (RNG-free)

- [ ] **RED:** unit tests (hand-stepped, against a separately seeded `Rand` to prove
      **no draw**): `do_damage_direct` subtracts `amount`, sets `last_killed_by_idx` only
      when `health<=0`; `do_damage` in `kGmKillEmAll` == `do_damage_direct` (no healing,
      no rand); `amount<=0` is a no-op; `do_healing_direct` clamps to `settings.health`.
- [ ] **GREEN:** port `game.cpp:546-589`. `rand.last` unchanged across all three.
- [ ] Reviewer (Opus): line-by-line vs `:546-589`; confirm the ScalesOfJustice branch is
      mode-gated and unreachable in `kGmKillEmAll`.

### T3 — sobject worm-damage arm live

- [ ] **RED:** `sobject` unit tests against a seeded `Rand` (non-tautological): an in-box
      worm with `health>0` draws exactly `kBloodAmount × [rand(128), rand(speedV=40),
      rand(40000)×2]` then `rand(3)` (+ `rand(3)` iff the gate is 0), spawns
      `kBloodAmount` type-6 nobjects (colour 0, `vel=w.vel/3`, `pos=w.pos`), and
      `health` drops by `z` but stays `> 0`; a `health<=0` worm draws nothing extra
      (existing test at `sobject.rs:561` stays green).
- [ ] **GREEN:** replace the `sobject.rs:182` tripwire with the `sobject.cpp:92-111`
      body (`DoDamage` → blood fan → `rand(3)` sound-gate). The vel-kick / `z` above it
      are already live. Read `kBloodAmount = game.blood * power_sum / 100`.
- [ ] Reviewer (Opus): RNG order (the **always-drawn** `rand(3)` at `:105`, the inner
      `rand(3)` only on 0), `vel/3`, colour 0, `Create2` call shape, `DoDamage` before blood.

### T4 — blood-trail → `CreateBObject` + `BObject::Process` + `bobjects` driver loop

- [ ] **RED:** unit tests against a seeded `Rand`: `create_bobject` draws **1**
      (`rand(NumBloodColours)+FirstBloodColour`), sets `pos`/`vel`; `bobject_process`
      off-map returns false (no rand); on `Background` adds `BObjGravity` to `vel.y`; on
      landing draws **one** `rand(3)`, writes the right pixel band (`77`/`82`/`85 +
      rand(3)`), returns false; the blood-trail arm calls `create_bobject(pos, vel/4)`
      **only** when `blood_trail && delay>0 && cycles % delay == 0` (test at `cycles=10`
      fires, `cycles=5` does not).
- [ ] **GREEN:** replace the `nobject.rs:373` tripwire with the blood-trail
      `CreateBObject(pos, vel/4)` (`nobject.cpp:95-97`); implement `BObject::Process` +
      `CreateBObject` (`bobject.cpp:7-49`); make the `process_frame` `bobjects` loop live
      (swap-remove on `!Process`, `game.cpp:349-355`).
- [ ] Reviewer (Opus): the `cycles % delay` gate, `vel/4`, the pixel bands + their
      `rand(3)`, pos-only hash, swap-remove free order.

### T5 — Scenario + gen script + committed golden

- [ ] Author `sim_slice5b_scenario.txt`: seed 42, `physics_fall_test.lev`,
      `weapon 0 cannon <ammo>`, a worm placed **inside** `medium_explosion`'s
      `detectRange=14` box at the explode tick (wounded not killed), the other worm far/
      invisible. Run **> 10 ticks past the hit** so blood nobjects survive to a
      `cycles % 10 == 0` tick and drip bobjects. Tune aim/fire so the shell explodes
      adjacent to the placed worm. **WEAPON NAME = `CANNON` (uppercase)** for the directive.
- [ ] `gen_sim_slice5b_golden.sh` = faithful copy of `gen_sim_slice5a_golden.sh` (exec,
      LOCAL/MANUAL, `PRESET` default `macos-arm64`), pointing at the rebuilt dumper.
- [ ] Generate + commit `golden/sim_slice5b.txt`. Inspect directly: `worm` column moves
      at the hit tick (`health` down, `vel` kicked); `rng` bursts at explode (blood fan +
      `rand(3)` gates + bobject colour `rand(3)`s); `bobjects` non-empty from the first
      `cycles % 10 == 0` tick after blood spawns; `nobjects` gains type-6; `level` carves
      (crater + bobject landings); `bonuses` empty; master moves with `cycles` every tick.
- [ ] Reviewer (Opus): the golden is self-consistent; the worm-damage + bobjects
      signatures are present (not a vacuous scenario).

### T6 — `sim_slice5b_golden` difftest (MILESTONE)

- [ ] Mirror `sim_slice5a_golden.rs`: expected parsed from the golden (all 11 columns);
      actual from a genuinely driven `SimState` (real `.lev`/`tc.cfg`/`Objects::load`,
      **cannon by name**, `id==index` for weapons/sobject_types/nobject_types,
      `SimState::new` full args incl. `blood`); components asserted **before** master;
      input keyed `k-1`; all ticks incl. tick 0.
- [ ] Coverage guards (non-vacuous, from driven state): hit worm `health > 0` **every**
      tick **and** `< 100` after the hit; `bobjects` count `> 0` on ≥ 1 tick;
      `nobjects` gains ≥ 1 type-6 at the hit tick; `nobjects < 600`; `bonuses` empty.
- [ ] **Milestone:** master + all 9 component hashes bit-exact every tick. On failure,
      `systematic-debugging` against the diverging column (blood draws localise via `rng`
      + `nobjects`; bobjects via `bobjects` pos-only + `level`; recall the master folds
      `cycles` — O17).
- [ ] Reviewer (Opus): honesty (expected from golden, actual from driver), non-vacuous
      guards, blood/bobject RNG order, "could it pass while the sim is wrong?" check.

### T7 — Controller done-check + docs/ledger

- [ ] `cargo test --workspace` green (incl. `sim_slice5b_golden` + slices 1–5a vs the
      regenerated goldens).
- [ ] `sim` float-free (`grep f32/f64` empty), deps = `sim-core`+`assets` only.
- [ ] **Prior-slice gate (redefined):** slices 1–5a component columns byte-identical
      old↔new; only master column changed — recorded explicitly (NOT "git diff empty").
- [ ] Update `docs/superpowers/liero-rs-PROGRESS.md` + the overview's slice-5
      decomposition row (5b DONE) + the SDD ledger (O10/O15/O16/O17/O20 resolved; the
      `cycles` ripple resolution noted).
- [ ] Then broad whole-slice review (Opus) → push 5b to PR #3 + update PR body.

## Done-report (each task)

(a) what changed + why, (b) files touched, (c) tests/risks. Per-task commit, local.
Push the whole sub-slice to PR #3 after the broad review.
