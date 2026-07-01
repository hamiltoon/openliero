# Step 2, Slice 5c — Bonuses: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking. **Test-first**: write the failing
> test (or its assertion) before the implementation it pins.

**Goal:** Make the **`bonuses` pool go live** — wire the per-tick **bonus-drop roll**
(`rand(c[CBonusDropChance])`, gated on `max_bonuses>0`), port **`CreateBonus`** (RNG
position search + frame/timer/weapon draws), **`Bonus::Process`** (fall/gravity/bounce/
expire), and the **bonuses driver loop** — so the Rust sim reproduces the C++ master
`HashGameState` **and** all 9 component hashes **tick-for-tick** when a bonus drops, falls,
and (optionally) expires. Pickup and the recursive chain-loop are **deferred with
tripwires** (thin path); the controller may pull pickup IN.

**Architecture:** Extend `rust/sim/` (deps unchanged: `sim-core`, `assets`; Bevy-free,
float-free). Sim changes: add `SimState.settings_max_bonuses: i32` (= 0) + the gated
bonus-drop roll in `process_frame` (after `*cycles += 1` at `state.rs:1051`); add the
**bonuses driver loop** at the **top** of `process_frame`'s object section (before the
sobjects loop, mirroring `game.cpp:287-290`); port `CreateBonus` + `CheckBonusSpawnPosition`
+ `Bonus::Process`; add the runtime `vel_y`/`used` fields to the existing `Bonus`. The
**C++ dumper** changes (bonus Process loop + bonus-drop roll + a `max_bonuses` directive
default 0) — the only C++ edit. Then a new scenario + golden + `sim_slice5c_golden`
difftest, mirroring 5b.

**Tech stack:** Rust (`sim` extend, `oracle-tests`) + a one-file C++ dumper edit. Goldens
generated **LOCALLY/MANUALLY** via the rebuilt dumper (`OPENLIERO_BUILD_ORACLE_DUMP`); CI
(`cargo test --workspace`) runs the committed goldens. `data/TC/openliero` real TC.

## ✅ No controller golden-gate — the bonus-drop roll is TRANSPARENT

Unlike 5b's `cycles` fold, adding the bonus-drop roll **does not change any prior golden**.
The C++ gate `!h[HBonusDisable] && settings->max_bonuses > 0 && rand(c[CBonusDropChance])`
**short-circuits**: with `max_bonuses == 0` the `rand` is never drawn. The dumper's
`max_bonuses` is the ctor default `4` (`settings.hpp:69`) and is **not** overridden today,
so a naive port WOULD ripple — therefore the dumper gains a **`max_bonuses <n>` directive
defaulting to 0** and sets `settings->max_bonuses` from it. Prior scenarios omit the
directive ⇒ `max_bonuses==0` ⇒ no draw ⇒ **goldens byte-identical (git diff empty)**. The
5a/5b literal *"git diff empty"* prior-slice gate **holds again** — no regen, no redefined
gate. See the design's transparency section.

**The one pre-T0 controller item is SCOPE, not goldens:** does 5c include **pickup**
(worm walks onto a bonus → health/weapon/booby RNG in the worm loop, a second scenario) or
**defer it with a tripwire** (thin path)? The plan below assumes the **thin path**
(pickup deferred, T5 optional). Confirm before T4/T5.

## Global constraints

- **Bit-exact vs C++.** Sources of truth: `game.cpp:216-265` (`CreateBonus`),
  `game.cpp:200-214` (`CheckBonusSpawnPosition`), `game.cpp:287-290` (bonuses driver loop),
  `game.cpp:357-362` (`++cycles` + bonus-drop roll), `bonus.cpp:6-35` (`Bonus::Process`),
  `worm.cpp:287-322` (pickup — deferred), `sobject.cpp:217-227` (chain-loop — deferred),
  `stateHash.hpp` (master folds bonus `x,y,timer,weapon,frame`; component drops `frame`).
  RNG order is the contract.
- **RNG order, per tick when enabled** (verified): after `++cycles`, **one**
  `rand(CBonusDropChance)`; on `== 0`, `CreateBonus` → (no rand before the `Size>=max`
  early-out) → per search trial `rand(BonusSpawnRectW)` **then** `rand(BonusSpawnRectH)`
  (2 draws) until `CheckBonusSpawnPosition` (no rand) passes → `frame = rand(2)` (unless
  `HBonusOnlyHealth/Weapon`) → `timer = rand(bonus_rand_timer[frame][1]) +
  bonus_rand_timer[frame][0]` → **for `frame==0`** `do { rand(weapons.size()) } while
  weap_table==2` → spawn-flash `sobject_types[7].Create`. Thread the one `sim-core::Rand`.
- **`Bonus::Process` is RNG-free** (`bonus.cpp:6-35`); only the **expiry sobject**
  (`sobject_types[bonus_s_objects[frame]].Create`) may draw — keep the scenario over clean
  ground.
- **Bonus pool ordering.** `bonuses` is `ExactObjectList<Bonus,99>` — **lowest-free-index
  allocate, free-by-slot** (NOT swap-remove). The Rust `Pool<Bonus>` already matches
  (`pool.rs:24-27`). Allocate via `bonuses.spawn(..)` (returns the lowest free slot);
  expiry frees by slot.
- **Loop placement.** The bonuses **Process loop** is at the TOP of `ProcessFrame`
  (`game.cpp:287-290`, BEFORE sobjects) — insert it before the sobjects loop at
  `state.rs:924`. The bonus-drop **roll** is in the tail (`game.cpp:359-362`, AFTER
  `++cycles` at `state.rs:1051`, BEFORE the worm loop at `:1053`). These two are at
  DIFFERENT points — do not conflate them.
- **`max_bonuses` gate.** New `SimState.settings_max_bonuses: i32` (= 0; the real
  `Settings` default is 4 but the dumper overrides to the scenario value). New
  `SimState::new` arg ⇒ update all callers; slices 1–5b stay byte-identical (the gate
  short-circuits at 0). Mirror the dumper's `max_bonuses` directive (default 0).
- **`HBonusDisable`** (TC hidden flag, false for openliero) folds into the gate as
  `!h_bonus_disable`; model it as a loaded const (already false) — it does not change the
  thin path but keep it for fidelity.
- **Deferred (justify-in-place):** bonus pickup (`worm.cpp:287-322`) — tripwire in the
  worm-loop bonus branch; the chain-loop (`sobject.cpp:217-227`) — a real tripwire on a
  damaging sobject reaching a non-empty bonus pool. Keep their guards intact.
- **Truncating division / shifts.** `Ftoi`=`>>16`; `Itof`=`<<16`; bounce `/BonusBounceDiv`
  and the pickup `*health/100`, `/3` are Rust `/`, never `>>`. `vel_y` is Q16.16 like worm
  `vel`.
- **Pools.** `bonuses` cap 99 (`BONUS_CAPACITY`); `CreateBonus` early-outs at
  `Size >= max_bonuses` and `NewObject` may return `None` (handle it, `game.cpp:244-246`).
- **Scenario is the single source of truth**, read by both the dumper and the Rust test.
  Golden regen LOCAL/MANUAL; `PRESET` defaults `macos-arm64`.
- **No AI / "Generated with" taglines.** **Bash discipline:** one command per call; no
  `>>`/heredoc/`&&`/`;`/`$VAR`; no `cd`+`git`; create files with the editor.

## File structure

- `src/tools/oracle_dump/sim_physics_dump.cpp` — **C++**: add a `max_bonuses` scenario
  field (default 0) + parse a `max_bonuses <n>` directive + set
  `settings->max_bonuses = scn.max_bonuses` (after the other settings overrides, ~`:204`);
  add the **bonuses Process loop** at the top of the per-tick driver (before the sobjects
  loop, ~`:317`, mirroring `game.cpp:287-290`); add the **bonus-drop roll** after
  `++game.cycles` (~`:346`, before the worm loop) mirroring `game.cpp:359-362`. Update the
  header comment (it currently states the roll is excluded).
- `rust/sim/src/state.rs` — `settings_max_bonuses: i32` on `SimState` + `SimState::new`;
  the bonuses Process loop at the top of `process_frame`'s object section; the gated
  bonus-drop roll after `*cycles += 1`; thread the bonus constants/flags.
- `rust/sim/src/bonus.rs` (new, or extend) — `Bonus::Process` + `CreateBonus` +
  `CheckBonusSpawnPosition`; add `vel_y`/`used` to the `Bonus` struct (`state.rs:376`).
- `rust/oracle-tests/scenarios/sim_slice5c_scenario.txt` — new scenario (`max_bonuses 4`).
- `rust/oracle-tests/gen_sim_slice5c_golden.sh` — faithful 5b-copy (LOCAL/MANUAL).
- `rust/oracle-tests/golden/sim_slice5c.txt` — committed golden.
- `rust/oracle-tests/tests/sim_slice5c_golden.rs` — the difftest (milestone).
- Slices 1–5b goldens — **unchanged** (re-diff to prove it; do NOT regenerate).

## Tasks

### T0 — Dumper (bonus loop + roll + `max_bonuses` directive) + re-diff 1–5b byte-identical

- [ ] **GREEN (C++):** in `sim_physics_dump.cpp`, add `int max_bonuses = 0;` to
      `Scenario` (default 0); parse `max_bonuses <n>` in `ParseScenario`; set
      `settings->max_bonuses = scn.max_bonuses;` after the existing overrides (~`:204`).
      Add the **bonuses Process loop** at the TOP of the per-tick driver (before the
      sobjects loop), mirroring `game.cpp:287-290`. Add the **bonus-drop roll** after
      `++game.cycles` (before the worm loop), mirroring `game.cpp:359-362`
      (`if (!common->h[HBonusDisable] && settings->max_bonuses > 0 &&
      game.rand(common->c[CBonusDropChance]) == 0) game.CreateBonus();`). Update the header
      comment (the roll is now wired but gated; default `max_bonuses=0` keeps it inert).
- [ ] Rebuild the dumper (`-DOPENLIERO_BUILD_ORACLE_DUMP=ON`); smoke-run it on the 5b
      scenario.
- [ ] **Re-diff:** regenerate slices 1–5b goldens via their gen scripts against the
      rebuilt dumper and assert **git diff empty** (the `max_bonuses==0` short-circuit ⇒
      no new draw ⇒ byte-identical). This is the transparency proof. **Do NOT commit any
      change to those goldens** — they must be unchanged.
- [ ] Reviewer (Opus): the three dumper edits (loop placement vs `game.cpp:287-290`, roll
      placement vs `:359-362`, the `max_bonuses` default-0 override), and the empty-diff
      result for 1–5b.

### T1 — Rust `settings_max_bonuses` + `Bonus.vel_y`/`used` + gated roll + bonuses loop

- [ ] **RED:** unit-assert `SimState.settings_max_bonuses == 0` after `new` for the dumper
      config; assert `process_frame` draws **no** RNG from the bonus path when
      `settings_max_bonuses == 0` (rand.last unchanged across a tick on an empty scenario);
      assert the bonuses Process loop runs (no-op on the empty pool).
- [ ] **GREEN:** add `settings_max_bonuses: i32` to `SimState` + `SimState::new` (= 0);
      thread through every caller. Add the bonuses **Process loop** at the top of
      `process_frame`'s object section (before the sobjects loop at `state.rs:924`),
      driving `bonus_process` over `bonuses` (empty ⇒ no-op this task). Add the gated
      **bonus-drop roll** after `*cycles = cycles.wrapping_add(1)` (`state.rs:1051`),
      before the worm loop: `if !h_bonus_disable && settings_max_bonuses > 0 &&
      rand.get(c_bonus_drop_chance) == 0 { create_bonus(..) }` (with `create_bonus` a stub
      that asserts unreachable until T2, since `max_bonuses==0` here). Add `vel_y: i32` and
      `used: bool` to `Bonus` (not hashed).
- [ ] Slices 1–5b difftests **green and byte-identical** (the gate is inert at
      `max_bonuses==0`). This is the Rust side of the T0 transparency proof.
- [ ] Reviewer (Opus): loop placement (top, before sobjects), roll placement (after
      `++cycles`, before worm loop), short-circuit order matches C++ `&&`, `vel_y`/`used`
      not hashed, `settings_max_bonuses` threaded to all callers, prior difftests
      unchanged.

### T2 — `CreateBonus` + `CheckBonusSpawnPosition` (search + frame/timer/weapon RNG)

- [ ] **RED:** unit tests against a seeded `Rand` (non-tautological): on a clear-ground
      position `create_bonus` draws `rand(W)`+`rand(H)` (first trial succeeds), then
      `rand(2)` (frame), then `rand(timer_range)` (+ the weapon `do/while` reject loop iff
      `frame==0`), spawns one `Bonus` at the lowest free slot with `x=Itof(ix)`,
      `y=Itof(iy)`, `vel_y=0`, the drawn `frame`/`timer`/`weapon`; `Size >= max_bonuses`
      early-outs with **no** rand; `check_bonus_spawn_position` rejects a 5×5 box with any
      DirtRock pixel (no rand) and accepts clear ground.
- [ ] **GREEN:** port `game.cpp:216-265` + `:200-214`. Honour `HBonusSpawnRect`
      offset / `HBonusOnlyHealth/Weapon` frame overrides (openliero leaves them false).
      Spawn the flash `sobject_types[7]` (RNG folds in). Replace the T1 `create_bonus`
      stub. The weapon reject loop uses `weap_table` (load it).
- [ ] Reviewer (Opus): the 2-draws-per-trial order, frame/timer draw order, the weapon
      reject loop (`weap_table==2` rejected), the `Size>=max` early-out (no rand), lowest-
      free-index spawn, `Itof` positions.

### T3 — `Bonus::Process` (fall/gravity/bounce/expire) + expiry sobject

- [ ] **RED:** unit tests against a seeded `Rand`: `bonus_process` adds `BonusGravity` to
      `vel_y` over `Background`; bounces (`vel_y = -(vel_y*BonusBounceMul)/BonusBounceDiv`,
      zeroed if `|vel_y|<100`) on floor/`DirtRock`; `y += vel_y`; **no direct rand**; on
      `--timer <= 0` spawns `sobject_types[bonus_s_objects[frame]]` and frees the bonus by
      slot iff `used`. (Confirm the expiry sobject is RNG-clean for the scenario type.)
- [ ] **GREEN:** port `bonus.cpp:6-35`; wire it into the T1 bonuses Process loop (now
      non-trivial). Free-by-slot via `Pool::free` (lowest-free-index pool).
- [ ] Reviewer (Opus): gravity/bounce arithmetic (truncating `/`, the `<100` zero), the
      `Mat(ix,iy+1).Background()` gravity gate, the `Ftoi(y+vel_y)` look-ahead, expiry
      `Free` only when `used`, pos-/timer- hashed fields move.

### T4 — Bonus chain-loop tripwire (port iff the scenario reaches it)

- [ ] Thread the `bonuses` pool into `sobject_create` (the chain-loop reads it,
      `sobject.cpp:217-227`). Add a **real tripwire**: a `debug_assert!` that fires if a
      damaging sobject (`detect_range>0`) is created while the bonus pool is non-empty and
      a bonus falls in its blast box — the currently un-tripwired gap. If 5c's scenario
      keeps bonuses away from explosions (thin path), this never fires; **port the
      recursion only if the chosen expiry/flash sobject has `detect_range>0`** (verify the
      TC config in T3). Note the deferral for slice 6.
- [ ] Reviewer (Opus): the tripwire is reachable (not dead), the deferral is justified by
      the scenario layout.

### T5 — *(OPTIONAL, controller-gated)* Bonus pickup (health / weapon / booby)

- [ ] **Only if the controller pulls pickup IN.** Otherwise: keep the worm-loop bonus
      branch a `debug_assert!` tripwire and skip to T6.
- [ ] **RED:** unit tests against a seeded `Rand`: a worm in a bonus's 11×11 box — health
      bonus (`frame==1`, `health<settings_health`) draws `rand(BonusHealthVar)` then
      `DoHealing(... +BonusMinHealth)*health/100)` and frees the bonus; weapon bonus
      (`frame==0`) always draws `rand(BonusExplodeRisk)` — `>1` reload (set `ww.type`/
      `ammo`, free), else booby `sobject_types[0].Create` + free; a full-health worm over a
      health bonus draws nothing and leaves the bonus.
- [ ] **GREEN:** port `worm.cpp:287-322` into the worm loop; replace the tripwire. Add a
      second scenario (`sim_slice5c_pickup_scenario.txt`) + golden if shipped.
- [ ] Reviewer (Opus): the always-drawn `rand(BonusExplodeRisk)`, the `health<max` gate on
      healing, free-on-pickup order, `DoHealing` (RNG-free) arithmetic.

### T6 — Scenario + gen script + committed golden

- [ ] Author `sim_slice5c_scenario.txt`: a seed where the first `rand(CBonusDropChance)`
      hits 0 mid-run; `physics_fall_test.lev` (clear band); `max_bonuses 4`; both worms
      **away from the dropped bonus's 11×11 box and any explosion** (thin path). Run enough
      ticks that the bonus drops, **falls/bounces several ticks**, and (if long enough)
      **expires**. Keep the spawn rect over clear ground (single search trial). Inspect the
      `OL_PHYS_TRACE` output to tune the seed/timing so the drop lands mid-run.
- [ ] `gen_sim_slice5c_golden.sh` = faithful copy of `gen_sim_slice5b_golden.sh` (exec,
      LOCAL/MANUAL, `PRESET` default `macos-arm64`), pointing at the rebuilt dumper.
- [ ] Generate + commit `golden/sim_slice5c.txt`. Inspect directly: `rng` flat until the
      drop, then **moves every tick**; the drop-tick burst (search + frame + timer
      [+ weapon] + flash); `bonuses` non-empty and **moving** (`y`/`timer`) as it falls;
      `worm` columns flat (no pickup); `sobjects` gains the flash (+ expiry); master moves
      with `cycles` every tick.
- [ ] Reviewer (Opus): the golden is self-consistent; the bonus drop/fall signature is
      present (not vacuous), worms untouched.

### T7 — `sim_slice5c_golden` difftest (MILESTONE)

- [ ] Mirror `sim_slice5b_golden.rs`: expected parsed from the golden (all 11 columns);
      actual from a genuinely driven `SimState` (real `.lev`/`tc.cfg`/`Objects::load`,
      `id==index` for the object tables, `SimState::new` full args incl.
      `settings_max_bonuses` = the scenario's value); components asserted **before** master;
      input keyed `k-1`; all ticks incl. tick 0.
- [ ] Coverage guards (non-vacuous, from driven state): `bonuses` count `> 0` on ≥ 1 tick;
      the dropped bonus's hashed `y` **changes** across ticks (it falls); `rng` **moves on
      every tick ≥ the drop tick** (the roll is firing); both worms' `health` unchanged and
      `worm` columns flat; `bonuses < 99`.
- [ ] **Milestone:** master + all 9 component hashes bit-exact every tick **and** slices
      1–5b re-run byte-identical (git diff empty). On failure, `systematic-debugging`
      against the diverging column (drop draws localise via `rng` + `sobjects`; fall via
      `bonuses` x/y/timer; recall the master folds `cycles` — O17 — and the gate
      short-circuit — bonus-drop transparency).
- [ ] Reviewer (Opus): honesty (expected from golden, actual from driver), non-vacuous
      guards, the drop/fall RNG order, "could it pass while the sim is wrong?" check.

### T8 — Controller done-check + docs/ledger

- [ ] `cargo test --workspace` green (incl. `sim_slice5c_golden` + slices 1–5b
      **unchanged**).
- [ ] `sim` float-free (`grep f32/f64` empty), deps = `sim-core`+`assets` only.
- [ ] **Prior-slice gate (literal restored):** slices 1–5b goldens **byte-identical**
      (git diff empty) — recorded explicitly (the `max_bonuses==0` short-circuit; contrast
      5b's `cycles` regen).
- [ ] Update `docs/superpowers/liero-rs-PROGRESS.md` + the overview's slice-5
      decomposition row (5c DONE) + the SDD ledger (bonus pool live; the chain-loop
      tripwire + pickup-scope resolutions; the bonus-drop-roll TRANSPARENT finding).
- [ ] Then broad whole-slice review (Opus) → push 5c to PR #3 + update PR body.

## Done-report (each task)

(a) what changed + why, (b) files touched, (c) tests/risks. Per-task commit, local.
Push the whole sub-slice to PR #3 after the broad review.
