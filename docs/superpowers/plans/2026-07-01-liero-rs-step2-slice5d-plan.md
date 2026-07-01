# Step 2, Slice 5d — Death + respawn: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking. **Test-first**: write the failing
> test (or its assertion) before the implementation it pins.

**Goal:** Turn on the **worm-loop death path** — port the four unported branches of
`Worm::Process` (health clamp + lives gate, pre-death drip, death block, and the dead-worm
`else` arm with `BeginRespawn` + `DoRespawning`) so the Rust sim reproduces the C++ master
`HashGameState` **and** all 9 component hashes **tick-for-tick** when a worm is killed by an
explosion, sprays blood + gibs, counts down, respawns via the level-reading position search,
and re-aims via the lone raw `rand()&1`. The `BeginRespawn` trial count (level + enemy-pos
dependent) is the canonical Step-2 desync trap — it earns a **fuzz** pass.

**Architecture:** Extend `rust/sim/` (deps unchanged: `sim-core`, `assets`; Bevy-free,
float-free). Sim changes only — **no C++ dumper edit** (the dumper already drives the
unmodified `Worm::Process` with `quick_sim == false` and `blood == 100` defaults; see the
design's "no dumper change" finding). Restructure `process_frame`'s worm loop into the
`visible` / dead arms; port the drip + death block into the visible arm; port the dead arm +
`BeginRespawn`/`CheckRespawnPosition`/`DoRespawning`; add non-hashed runtime `Worm` fields
(`direction`, `logic_respawn`, `ready`, `last_killed_by_idx`, …); replace the O3 full-pool
panic with the C++ `NewObjectReuse` overwrite. Then a new scenario + golden +
`sim_slice5d_golden` difftest (milestone), and fixed-level multi-seed fuzz difftests.

**Tech stack:** Rust (`sim` extend, `oracle-tests`). Goldens generated **LOCALLY/MANUALLY**
via the already-built dumper (`OPENLIERO_BUILD_ORACLE_DUMP`, unchanged binary); CI
(`cargo test --workspace`) runs the committed goldens. `data/TC/openliero` real TC.

**BASE commit for T0: `b3e13be`** (branch `liero-rs-step-2`; slices 1–4 + 5a+5b+5c
shipped bit-exact).

## ✅ No dumper change, no controller golden-gate

Unlike 5a (Rust-only + re-diff), 5b (`cycles` regen), and 5c (bonus loop + directive), 5d
requires **zero C++ dumper edits**: the death/respawn logic lives entirely in the
already-compiled `Worm::Process`, which the dumper drives unmodified every tick
(`sim_physics_dump.cpp:394-398`), with `quick_sim{false}` (`game.hpp:153`) and `blood{100}`
(`settings.hpp:70`) defaults already satisfying the `BeginRespawn` gate and the death
spray. Therefore **slices 1–5c re-diff byte-identical trivially** (nothing in the C++ path
moved) — the literal *"git diff empty"* prior-slice gate holds. All work is on the Rust
side. (Optional, byte-neutral: a `blood <n>` directive defaulting to 100 to bound `kMax` —
add only if the milestone golden must be provably under the O3 cap without relying on a
single death; the default equals today's implicit value so priors stay identical.)

## Global constraints

- **Bit-exact vs C++.** Sources of truth: `worm.cpp:213-215` (clamp + gate),
  `:355-367` (pre-death drip), `:369-426` (death block), `:431-450` (dead arm),
  `:711-742` (`BeginRespawn`), `:755-809` (`DoRespawning`), `game.cpp:611-650`
  (`CheckRespawnPosition`), `nobject.cpp:7-66` (`Create`/`Create1`/`Create2` RNG),
  `fastObjectList.hpp:35-44` + `exactObjectList.hpp:57-60` (`NewObjectReuse` overwrite),
  `stateHash.hpp` (master folds worm `pos,vel,aiming_angle,health,lives,kills,timer,
  visible,Pack`; component drops `aiming_angle,kills`; **`killed_timer` and `direction` are
  in NEITHER**). RNG order is the contract.
- **RNG order, per branch** (verified — see the design's Scope for full detail):
  - **drip** (`health < settings_health/4`): `rand(health+6)`; on 0 → `rand(3)`; on 0 →
    `rand(3)` sound; then `nobject_types[6].Create1` = `rand(dist*2)×2`.
  - **death** (`health<=0`): `rand(3)` death sound; `--lives`/kills bookkeeping (no rand);
    `kMax=120*blood/100`, **iff `kMax>1`** for `i in 1..=kMax`: `rand(128)` +
    `nobject_types[6].Create2` (`rand(speed_v)` + `rand(dist*2)×2`); worm-gibs `for i in
    (7..=105).step_by(14)` (**8 iterations**: `{7,21,35,49,63,77,91,105}`): `rand(14)` +
    `nobject_types[index].Create2` (gib type's own sub-draws — **load `nobject_types[0]/[1]`
    params from TC, do NOT assume blood's shape**).
  - **`BeginRespawn`**: per trial `rand(WormSpawnRectW)` **then** `rand(WormSpawnRectH)` (2
    draws); drop-down `while` + `CheckRespawnPosition` are **rand-free** (read level +
    enemy/last pos); `killed_timer = -1`.
  - **`DoRespawning`** (on convergence + `ready`): `DrawDirtEffect` (dirt draws, 4c-ported)
    then the lone **`rand() & 1`** (no-arg) for aiming.
- **`killed_timer` is unhashed.** The 150-tick dead phase is hash-silent; the countdown is
  pinned only through *when* the `BeginRespawn` `rng` burst lands. Do not add it to any
  hash.
- **`kKilledTimerInitial = 150`** (`worm.hpp:243`). Death sets it to 150; the dead arm
  decrements while `> 0`; `== 0 && !quick_sim` → `BeginRespawn` (sets `-1`); `< 0` →
  `DoRespawning` every tick until `ready` + convergence complete the respawn.
- **Truncating division / shifts.** `Ftoi`=`>>16`, `Itof`=`<<16`; `120*blood/100`,
  `vel/3`, and the `DoRespawning` `pos-80` arithmetic are integer `/`, never `>>`.
  `rand()&1` is the raw engine value masked — use the no-arg `Rand` draw, not `rand.bound`.
- **Pools / O3.** `nobjects` cap 600. Replace the `nobject_create` full-pool panic
  (`nobject.rs:118`) with a `new_object_reuse` that **overwrites the last slot** when full
  (C++ `NewObjectReuse`), distinct from `NewObject`/`spawn`→`None`. The milestone stays
  under cap; the fuzz exercises the overwrite (so T6 precedes T9).
- **Game mode.** Port the `KillEmAll` clamp/gate/death branch; keep `Scales`/`GameOfTag`
  branches present-but-guarded (TC mode is `KillEmAll`). The lives gate skips the ENTIRE
  worm body when `lives == 0` in `KillEmAll` — hash-neutral for priors (`lives > 0` always).
- **Scenario is the single source of truth**, read by both the (unchanged) dumper and the
  Rust test. Golden regen LOCAL/MANUAL; `PRESET` defaults `macos-arm64`.
- **No AI / "Generated with" taglines.** **Bash discipline:** one command per call; no
  `>>`/heredoc/`&&`/`;`/`$VAR`; no `cd`+`git`; create files with the editor.

## File structure

- `rust/sim/src/state.rs` — restructure `process_frame`'s worm loop (`:1281-1369`) into the
  `if w.visible { …active… }` arm (adding drip + death block at the end) and the `else`
  dead arm (`killed_timer` countdown + `BeginRespawn` + `DoRespawning`); add the health
  clamp + lives gate before the split; thread `WormSpawnRect*`/`WormMinSpawnDist*` consts.
- `rust/sim/src/worm.rs` (or the worm module) — add non-hashed runtime fields to `Worm`
  (`direction`, `logic_respawn: IVec2`, `ready`, `last_killed_by_idx`, `make_sight_green`,
  `leave_shell_timer`, `fire_cone`, `steerable_count`); the drip/death/`BeginRespawn`/
  `CheckRespawnPosition`/`DoRespawning` functions.
- `rust/sim/src/nobject.rs` — `new_object_reuse` (overwrite-last-slot at cap) replacing the
  T4c panic; route the death sprays through it.
- `rust/sim/src/pool.rs` — a `spawn_reuse`/`reuse_last` method mirroring `NewObjectReuse`.
- `rust/oracle-tests/golden/sim_slice5d_scenario.txt` — new scenario (kill → respawn).
- `rust/oracle-tests/gen_sim_slice5d_golden.sh` — faithful 5c-copy (LOCAL/MANUAL).
- `rust/oracle-tests/golden/sim_slice5d.txt` — committed golden.
- `rust/oracle-tests/tests/sim_slice5d_golden.rs` — the difftest (milestone).
- `rust/oracle-tests/golden/sim_slice5d_fuzz{1..N}_scenario.txt` + `.txt` goldens +
  `sim_slice5d_fuzz.rs` — the fixed-level multi-seed fuzz.
- Slices 1–5c goldens — **unchanged** (re-diff to prove it; do NOT regenerate).

## Tasks

### T0 — Dumper no-change verification + re-diff 1–5c byte-identical  [Opus]

- [ ] Confirm from `sim_physics_dump.cpp` that the per-tick driver already calls the
      **unmodified** `w->Process(game)` (`:394-398`), that the `Game` keeps `quick_sim ==
      false` (never set ⇒ `game.hpp:153` default) and `settings->blood == 100` (never set ⇒
      `settings.hpp:70` default), and that the `worm` directive already carries `health`
      + `lives` (`:144`). Conclude: **no C++ edit is needed** for 5d.
- [ ] Re-diff: regenerate slices 1–5c goldens via their gen scripts against the (unchanged)
      dumper binary and assert **git diff empty**. This is trivial (no dumper change) but is
      the transparency proof. Do NOT commit any change to those goldens.
- [ ] Reviewer (Opus): verify the "no dumper change" claim is real (the death/respawn code
      is entirely in `Worm::Process`, not gated behind anything the dumper omits) — the one
      judgment call in T0. If a `blood <n>` bounding directive is deemed worthwhile, it
      defaults to 100 (= current) and the re-diff must still be empty.

### T1 — Rust worm-loop restructure: clamp + lives gate + visible/dead arm split  [Opus]

- [ ] **RED:** unit-assert on the driven `SimState`: (a) the health clamp `health =
      min(health, settings_health)` is applied every tick (identity for a full-health worm);
      (b) with `lives == 0` in `KillEmAll` the worm body is skipped (a synthetic worm);
      (c) slices 1–5c difftests stay **byte-identical** with the restructure in place.
- [ ] **GREEN:** in `process_frame`, before the per-worm `if w.visible`, apply the clamp
      (`:213`) and wrap the body in the lives gate (`:215`, `KillEmAll` path;
      `Scales`/`GameOfTag` guarded). Add the **`else` dead arm skeleton**: `steerable_count
      = 0`; `if PressedOnce(kFire) { ready = true }` (compare current vs previous control
      state); `if killed_timer > 0 { killed_timer -= 1 }`; `if killed_timer == 0 {
      begin_respawn(..) }`; `if killed_timer < 0 { do_respawning(..) }` — with
      `begin_respawn`/`do_respawning` **stubbed `unreachable!()`** (not reached until a worm
      actually dies, which needs T2/T3). Add the non-hashed runtime `Worm` fields
      (`direction`, `logic_respawn: IVec2`, `ready`, `last_killed_by_idx`,
      `make_sight_green`, `leave_shell_timer`, `fire_cone`, `steerable_count`), defaulted to
      match a freshly-reset C++ worm; confirm none is hashed.
- [ ] Slices 1–5c difftests **green and byte-identical** (clamp = identity, gate inert at
      `lives > 0`, dead arm unreached while all worms visible).
- [ ] Reviewer (Opus): the clamp/gate placement vs `:213-215`, the `PressedOnce` edge
      semantics, the new fields all unhashed, priors unchanged.

### T2 — Pre-death drip (`worm.cpp:355-367`)  [Opus]

- [ ] **RED:** unit tests against a seeded `Rand` (non-tautological): with `health <
      settings_health/4`, `drip` draws `rand(health+6)`; on a forced-0 outer roll it then
      draws `rand(3)`, and on a forced-0 inner roll the `rand(3)` sound, then **always**
      (within the outer gate) `nobject_types[6].Create1` = `rand(dist*2)×2`; on a nonzero
      outer roll it draws exactly one value and spawns nothing; when `health >=
      settings_health/4` it draws nothing.
- [ ] **GREEN:** port `:355-367` at the END of the visible arm (after the movement/change
      gate). Reuse the live `nobject_create1` (5a) for the blood spawn. Gate exactly on
      `health < settings_health / 4` (integer `/`).
- [ ] Reviewer (Opus): draw order (`health+6` → `3` → `3` sound → `Create1`), the `Create1`
      spawn is inside the outer gate but outside the sound gate, integer `/4`.

### T3 — Death block (`worm.cpp:369-426`)  [Opus]

- [ ] **RED:** unit tests against a seeded `Rand`: on `health <= 0`, `death` draws `rand(3)`
      death sound; `--lives` (KillEmAll); sets `visible=false`, `killed_timer=150`;
      increments the **killer's `kills`** iff `last_killed_by_idx >= 0 && != index`; sprays
      `kMax = 120*blood/100` blood particles **iff `kMax > 1`** each `rand(128)` +
      `Create2` (assert the exact draw count for `blood=100` ⇒ 120 particles ⇒ 480 draws,
      and that `blood=1` ⇒ `kMax=1` ⇒ **no spray**); sprays the **8** worm-gibs
      (`{7,21,35,49,63,77,91,105}`) each `rand(14)` + `nobject_types[index].Create2` (assert
      the loop runs 8×, pinning the `i <= 105` bound — the overview's "7×" is wrong). Load
      the gib type's params from the TC and assert its actual sub-draw count.
- [ ] **GREEN:** port `:369-426` after the drip. `KillEmAll` `--lives`; the
      `last_killed_idx`/`got_changed`/`kills` bookkeeping (`:393-405`); the blood spray via
      `nobject_create2` (5b); the gib spray via `nobject_create2` on `nobject_types[index]`.
      Keep `Scales`/`GameOfTag` branches guarded.
- [ ] Reviewer (Opus): the `kMax > 1` gate (not `>= 1`), the **8-iteration** gib loop, the
      `rand(14)` drawn as the angle arg (outside `Create2`), the gib type indexing
      (`nobject_types[index]`, per-worm), `kills++` targeting the killer, `killed_timer=150`.

### T4 — `BeginRespawn` + `CheckRespawnPosition` (the desync trap)  [Opus]

- [ ] **RED:** unit tests against a seeded `Rand` on a known level: `begin_respawn` sets
      `logic_respawn = death_pos - (80,80)` and `enemy = Ftoi(worms[index^1].pos)`; the
      `do/while` draws `rand(WormSpawnRectW)` **then** `rand(WormSpawnRectH)` per trial;
      **1-trial success on open ground far from enemy/last-pos ⇒ exactly 2 draws**; a
      position rejected by `CheckRespawnPosition` (too close to enemy, or a `Rock()` in the
      `[x-3,x+3)×[y-4,y+4)` box) forces a 2nd trial ⇒ 4 draws; the drop-down `while` (moves
      `pos.y` down over `Background`, no rand); the `trials >= 50000` break; `killed_timer =
      -1` on exit. `check_respawn_position` is **rand-free** and matches `game.cpp:611-650`
      (the last-pos/enemy min-dist rejects + the Rock box; keep the "rock respawn bug" TODO
      behaviour as-is).
- [ ] **GREEN:** port `worm.cpp:711-742` + `game.cpp:611-650`. Thread `WormSpawnRectX/Y/W/H`
      + `WormMinSpawnDistLast`/`WormMinSpawnDistEnemy` from the TC consts. Read the LIVE
      level + LIVE enemy `pos` (the desync inputs). `Itof`/`Ftoi` exactly as C++.
- [ ] Reviewer (Opus): **the 2-draws-per-trial order (W then H)**, the drop-down loop is
      rand-free, `CheckRespawnPosition`'s reject order (dist checks before the Rock box),
      the `enemy = worms[index^1]` read (only when `worms.size()==2`), `killed_timer=-1`.
      This is the desync-trap task — scrutinise the trial-count determinants.

### T5 — `DoRespawning` (convergence + the lone `rand()&1`)  [Opus]

- [ ] **RED:** unit tests against a seeded `Rand`: `do_respawning` steps `logic_respawn`
      toward `Ftoi(pos)-80` by ±1 **four times/tick** (no rand); `LimitXy` clamps to
      `[0,width-158]×[0,height-158]`; while NOT (converged within ±5 AND `ready`) it draws
      **nothing** and does not respawn; on convergence + `ready` it draws `DrawDirtEffect`
      (dirt, 4c-ported) then the lone **`rand() & 1`** (no-arg), branching
      `aiming_angle=Itof(32),direction=0` vs `Itof(96),direction=1`, and sets
      `ready=false`, `visible=true`, `fire_cone=0`, `vel=0`, `health=settings_health`
      (KillEmAll). Assert both `rand()&1` branches (force each).
- [ ] **GREEN:** port `worm.cpp:755-809`. Reuse the 4c `DrawDirtEffect`. `CorrectShadow`
      is gated on `settings->shadow` (**false** in the dumper) ⇒ omit. Use the no-arg
      `Rand` draw for `rand()&1` (NOT `rand.bound(2)` — confirm they coincide, but match the
      C++ call form).
- [ ] Reviewer (Opus): the 4-step convergence, the ±5 window, the `ready` gate, the single
      no-arg `rand()` (the only one in the family), `health=settings_health` restore,
      `vel.Zero()`.

### T6 — O3: `new_object_reuse` full-pool overwrite (before the fuzz)  [Opus]

- [ ] **RED:** unit test: spawning into a **full** 600-slot `nobjects` pool **overwrites the
      last slot** (index `limit-1`) in place and returns `limit-1` (matching C++
      `FastObjectList::NewObjectReuse` `&arr[limit-1]`, `fastObjectList.hpp:35-44`) — count
      stays at `limit`, the previous last-slot value is replaced, no free/swap. Contrast a
      `NewObject`-style spawn that returns `None` at cap (`game.cpp:244-246`) — unchanged.
- [ ] **GREEN:** add `Pool::spawn_reuse` (or `reuse_last`) mirroring `NewObjectReuse`;
      replace the `nobject_create` panic (`nobject.rs:118`) with it, so the death/damage
      blood storms match C++ at cap instead of panicking. Verify `sobjects`/`wobjects`/
      `bonuses` do NOT need it in 5d (bounded spawns) and leave their `None` handling.
- [ ] Reviewer (Opus): the overwrite targets `limit-1` (not a swap/free), `count`
      unchanged, `NewObject` vs `NewObjectReuse` distinction preserved, only `nobjects`
      rerouted.

### T7 — Scenario + gen script + committed golden  [Opus]

- [ ] Author `sim_slice5d_scenario.txt`: a `seed` where the killing air-burst lands mid-run
      and the respawn completes in-window; `physics_fall_test.lev`; `max_bonuses 0`; **worm0
      = killer** (health 100, `explosives` slot 0, positioned to catch worm1), a `Fire`
      input to launch; **worm1 = victim** (health `12`, below `settings_health/4` so the
      drip fires pre-explosion; dies from the blast), a later `Fire` input to the dead worm1
      to set `ready`. Positions near opposite edges so `BeginRespawn` is a bounded few-trial
      search and one death keeps `nobjects < 600`. Window ~250–350 ticks (fire → death →
      150-tick countdown → `BeginRespawn` → `DoRespawning` respawn). Tune via
      `OL_PHYS_TRACE`; assert the respawn completes.
- [ ] `gen_sim_slice5d_golden.sh` = faithful copy of `gen_sim_slice5c_golden.sh` (exec,
      LOCAL/MANUAL, `PRESET` default `macos-arm64`), pointing at the (unchanged) dumper.
- [ ] Generate + commit `golden/sim_slice5d.txt`. Inspect directly: the drip `rng` on
      pre-death ticks; the **death-tick burst** + worm1 `health<=0`/`visible`→false/`lives`
      --/worm0 `kills`++; flat `rng` through the dead phase; the **`BeginRespawn` burst** +
      worm1 `pos` jump; the respawn `rng`+`level` move + worm1 `visible`→true/`health`
      restored; `nobjects` non-empty then draining; `bonuses` empty.
- [ ] Reviewer (Opus): the golden is self-consistent; the full death→respawn signature is
      present (not vacuous); the trial count is bounded; `nobjects < 600`.

### T8 — `sim_slice5d_golden` difftest (MILESTONE)  [Opus]

- [ ] Mirror `sim_slice5c_golden.rs`: expected parsed from the golden (all 11 columns);
      actual from a genuinely driven `SimState` (real `.lev`/`tc.cfg`/`Objects::load`,
      `explosives` by name, `id==index` for the object tables, `SimState::new` full args);
      components asserted **before** master; input keyed `k-1`; all ticks incl. tick 0.
- [ ] Coverage guards (non-vacuous, from driven state): worm1 `health` crosses `<=0` **and**
      returns to `settings_health`; worm1 `visible` true→false→true; worm1 `lives` −1;
      worm0 `kills` +1; `rng` **bursts on the death tick AND on the `BeginRespawn` tick**
      (the trial-count witness); worm1 `pos` **jumps** at `BeginRespawn`; `level` carves at
      `DoRespawning`; `nobjects > 0` on the death tick and `< 600`; `bonuses` empty.
- [ ] **Milestone:** master + all 9 component hashes bit-exact every tick **and** slices
      1–5c re-run byte-identical (git diff empty). On failure, `systematic-debugging`
      against the diverging column (spray via `rng`+`nobjects`; trial count via `rng`+worm1
      `pos`; respawn via `rng`+`level`+`aiming_angle`; recall `killed_timer` is invisible —
      a countdown desync shows only as a mis-timed `rng` burst).
- [ ] Reviewer (Opus): honesty (expected from golden, actual from driver), non-vacuous
      guards, the death/BeginRespawn/DoRespawning RNG order, "could it pass while the sim is
      wrong?" check (esp. the trial-count witness).

### T9 — Fixed-level multi-seed respawn FUZZ (O21)  [Opus]

- [ ] Author **4** fuzz scenario variants (`sim_slice5d_fuzz{1..4}_scenario.txt`) on the
      same `physics_fall_test.lev`, each with a seed/position tuned so `BeginRespawn` takes
      a **different bounded trial count** (vary killer/victim x so `CheckRespawnPosition`'s
      enemy-dist / last-pos-dist reject a different number of trials), ~300 ticks each.
      Generate + commit their goldens (LOCAL/MANUAL, faithful gen-script copies).
- [ ] `sim_slice5d_fuzz.rs` — reuse the milestone harness over the 4 variants; each asserts
      master + 9 components bit-exact all ticks. Coverage: the variants collectively exhibit
      **≥ 2 distinct trial counts** (assert the `BeginRespawn` `rng`-burst width differs
      across variants — proves the trap's variance is covered vs the C++ oracle).
- [ ] **Optional cheap backstop:** a pure-Rust determinism guard (two `SimState` runs per
      seed asserted hash-identical every tick) — proves no nondeterminism entered the port.
- [ ] Requires **T6** (blood storms in longer variants can reach the 600 cap ⇒ the overwrite
      must be live). Reviewer (Opus): the variants really vary the trial count (not four
      copies of the same search); the goldens are honest.
- [ ] **NOTE (JOHN-BESLUT #1, design):** the bit-exact replay of the *existing* random-level
      C++ death-fuzz (needs `Level::GenerateFromSettings`) is **DEFERRED** — this
      fixed-level multi-seed fuzz is the in-scope 5d coverage. Do NOT port
      `GenerateFromSettings` in 5d unless John overrides the recommendation.

### T10 — Controller done-check + docs/ledger  [Opus broad review]

- [ ] `cargo test --workspace` green (incl. `sim_slice5d_golden` + `sim_slice5d_fuzz` +
      slices 1–5c **unchanged**).
- [ ] `sim` float-free (`grep f32/f64` empty), deps = `sim-core` + `assets` only.
- [ ] **Prior-slice gate (literal):** slices 1–5c goldens **byte-identical** (git diff
      empty) — recorded explicitly (no dumper change at all in 5d; contrast 5b's `cycles`
      regen).
- [ ] Update `docs/superpowers/liero-rs-PROGRESS.md` + the overview's slice-5 decomposition
      row (5d DONE; correct the gib-loop "7×" → **8×**) + the SDD ledger (worm-loop death
      path live; O3 `NewObjectReuse` overwrite ported; O21 fixed-level multi-seed fuzz;
      the no-dumper-change finding; the `killed_timer`-unhashed note).
- [ ] Then broad whole-slice review (Opus) → push 5d to PR #3 + update PR body.

## Done-report (each task)

(a) what changed + why, (b) files touched, (c) tests/risks. Per-task commit, local.
Push the whole sub-slice to PR #3 after the broad review.
