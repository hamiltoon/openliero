# Step 2, Slice 6 — Full `ProcessFrame` + game modes + chain-loop + >1000-tick fuzz: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax. **Test-first**: write the failing test (or its assertion) before
> the implementation it pins.

**Goal:** Close the `ProcessFrame` gap — port the **ninjarope `Process` loop**
(`game.cpp:368-370`) and the **game-mode switch** (`game.cpp:372-461`), the only two remaining
hashed/RNG-bearing pieces — plus the **chain-loop** (`sobject.cpp:217-227`) and
`new_object_reuse` for sobjects/wobjects, so a **multi-seed >1000-tick two-worm fuzz** (mirror of
`test_determinism.cpp`) reproduces the C++ oracle's master `HashGameState` + all 9
`HashGameComponents` tick-for-tick. LAST slice of Step 2 → PR #3 merges after it.

**Architecture:** Extend `rust/sim/` (deps unchanged: `sim-core`, `assets`; Bevy-free,
float-free) and the ONE dumper `src/tools/oracle_dump/sim_physics_dump.cpp` — promoted from a
ProcessFrame-**subset** to the full-tail (add the two hashed pieces after the worm loop; render
pieces stay omitted as hash-inert). Then chain-loop, pool-reuse, game-mode hooks, the fuzz, and
the step-2 broad review. See the design doc
(`specs/2026-07-09-liero-rs-step2-slice6-full-processframe-design.md`) for the gap analysis, the
ninjarope/chain-loop/fuzz designs, and the JOHN-BESLUT items.

**Tech stack:** Rust (`sim` extend, `oracle-tests`). Goldens generated LOCALLY/MANUALLY via the
rebuilt dumper (`OPENLIERO_BUILD_ORACLE_DUMP`, `CMakeLists.txt:388-389`, `PRESET` default
`macos-arm64`); CI (`cargo test --workspace`) runs the committed goldens. `data/TC/openliero`
real TC.

**BASE commit for T0: `ed72de0`** (branch `liero-rs-step-2`; slices 1–5′ + T10 shipped bit-exact).

## Global constraints

- **Bit-exact vs C++.** Sources of truth: `game.cpp:267-471` (`ProcessFrame` order),
  `ninjarope.cpp:7-82` (`Ninjarope::Process`), `worm.cpp:962-986` (rope throw/length),
  `sobject.cpp:217-227` (chain-loop), `game.cpp:372-461` (game-mode switch) + `:567-589` (Scales
  `DoDamage`), `stateHash.hpp` (folds), `test_determinism.cpp:14-123` (the fuzz fixture + input
  model). **RNG order is the contract.**
- **Ninjarope RNG order (load-bearing):** the terrain-attach spray is **11 × [`rand(128)` →
  `nobject_types[2].Create2`]** inside the `!attached && Inside && AnyDirt` guard
  (`ninjarope.cpp:41-45`), AFTER `pos += vel` and the anchor scan; the loop runs AFTER the worm
  loop and BEFORE the game-mode switch. No other draw in `Process`.
- **Hash contract unchanged.** Ninjarope: only `out` + `pos.{x,y}` are hashed (`stateHash.hpp:47-49`);
  new fields (`vel`/`attached`/`length`/`cur_len`/`anchor`) are NOT hashed. Game modes: only
  `worm.timer` / `worm.health` are hashed (already folded). Add nothing to any hash.
- **Float-free.** No `f32`/`f64` in `sim`. Fixed-point: `Ftoi`=`>>16`; `<< NRThrowVelX` etc. are
  fixed shifts; `kForce / cur_len` and `length` divides are truncating integer per-component;
  `wrapping_*` for all `int`-semantics arithmetic.
- **cossin[128].** The fuzz seeds worms DEAD → they respawn via `DoRespawning`, which sets
  `aiming_angle ∈ {32,96}` (`worm.cpp:799-805`), so `Ftoi(aiming_angle)` is never 0 and
  `cossin[128]` is unreachable. Do NOT hand-set `aiming_angle = 0` in any fuzz scenario.
- **Scenario is the single source of truth**, read by BOTH the (rebuilt) dumper and the Rust
  test. Golden regen LOCAL/MANUAL; `PRESET` default `macos-arm64`.
- **Prior-golden gate is NOT "git diff empty" this slice.** The game-mode switch is inert for all
  priors (KillEmAll → `default: break`); the ninjarope loop changes only rope-throwing goldens.
  T0's re-diff classifies each golden and regenerates the changed ones with a documented
  first-divergence tick + proven-identical prefix (the 5b cycles-ripple precedent).
- **No AI / "Generated with" taglines; no commit trailers.** **Bash discipline:** one command per
  call; no `>>`/heredoc/`&&`/`;`/`$VAR`; no `cd`+`git`; create files with the editor.

## File structure

- `src/tools/oracle_dump/sim_physics_dump.cpp` — add the ninjarope loop + game-mode switch after
  the worm loop; add a `game_mode` directive; update the header comment (no longer a "subset").
- `rust/sim/src/state.rs` — `Ninjarope` fields; `ninjarope_process` loop in `process_frame`
  after the worm loop, before game-mode; the game-mode switch; thread NR/game-mode consts.
- `rust/sim/src/control.rs` — un-skip the throw's `vel`/`length`/`attached` (OQ5) in
  `process_tasks`.
- `rust/sim/src/sobject.rs` / `weapon.rs` — thread `&mut Pool<Bonus>` through `sobject_create`
  and its callers; the chain-loop scan; `new_object_reuse` overwrite for sobjects/wobjects.
- `rust/oracle-tests/golden/sim_slice6*_scenario.txt` + `.txt` goldens + `gen_sim_slice6*.sh` +
  `tests/sim_slice6*.rs`.
- Prior goldens — re-diffed at T0; rope-throwing ones regenerated (documented), the rest asserted
  byte-identical.

## Tasks

### T0 — Dumper → full `ProcessFrame` tail + prior-golden re-diff gate  [Opus]

- [ ] **Extend `sim_physics_dump.cpp`:** after the per-worm `Process` loop (`:394-398`), add
      (a) the ninjarope loop `for w in worms: w->ninjarope.Process(*w, game)` (`game.cpp:368-370`)
      and (b) the game-mode switch (`game.cpp:372-461`); add a `game_mode <n>` scenario directive
      (default `kGmKillEmAll`). Keep screen_flash / shake / banner / `ProcessViewports` omitted
      (hash-inert — cite §1 of the design). Rebuild the dumper (`OPENLIERO_BUILD_ORACLE_DUMP`).
- [ ] **Re-diff gate:** regenerate EVERY committed `sim_slice*` golden via its gen script against
      the rebuilt dumper. Classify each: **byte-identical** (assert `git diff` empty) OR
      **diverges only from tick T** (T = first rope-out tick). Record the classification. Expect
      `sim_slice3` to change (scripted rope throw, ticks 93-97); check each fuzz golden
      (`5d*`, `5prime*`) for `Change|Jump` activity. Regenerate + commit the changed goldens ONLY,
      each with its documented first-divergence tick and a proof its pre-T prefix is unchanged.
- [ ] Reviewer (Opus): the two added pieces are in the exact C++ order; the render omissions are
      genuinely hash-inert; every "byte-identical" claim is real; every regenerated golden's
      pre-onset prefix is proven identical (surgical diff, not a blind overwrite). **This is
      JOHN-BESLUT #2 — surface the regen scope in the done-report.**

### T1 — Ninjarope datamodel + throw un-skip (OQ5)  [Opus]

- [ ] **RED:** unit tests: (a) `Ninjarope` gains `vel`/`attached`/`length`/`cur_len`/`anchor`,
      defaults match the C++ ctor, none hashed (a hash test over two states differing only in the
      new fields is equal); (b) `process_tasks` throw (Change + `PressedOnce(Jump)`) now sets
      `vel = (cossin[Ftoi(aiming_angle)].x << NRThrowVelX, .y << NRThrowVelY)`,
      `length = NRInitialLength`, `attached = false` — for a known `aiming_angle`; (c) the
      Change+Up/Down `length` adjust + `[NRMinLength, NRMaxLength]` clamp
      (`worm.cpp:964-972`); (d) slices 1–5′ non-rope difftests still byte-identical (new fields
      unhashed, throw writes only non-hashed fields until T2's `Process` runs).
- [ ] **GREEN:** add the fields to `Ninjarope` (state.rs); thread the NR* constants
      (`NRThrowVelX/Y`, `NRInitialLength`, `NRPullVel`, `NRReleaseVel`, `NRMinLength`,
      `NRMaxLength`) from the TC (post-`new`, no signature churn) + `&cossin` into `process_tasks`;
      un-skip the OQ5 block (`control.rs:284-293`) per `worm.cpp:962-986`.
- [ ] Reviewer (Opus): the `cossin` index is `Ftoi(aiming_angle)` (guaranteed in-range by the
      respawn-seeded aim, §8), the shift constants match, only non-hashed fields written, the
      length clamp order correct.

### T2 — Port `Ninjarope::Process` + wire the loop (hashed + RNG-bearing)  [Opus]

- [ ] **RED:** unit tests against a seeded `Rand`: (a) `out == false` → no-op, no rand; (b) rope
      over open air (no attach, no anchor) → `pos += vel`, `vel.y += NinjaropeGravity`, owner
      `vel` unchanged unless `cur_len > length`, **no rand**; (c) rope reaching **dirt** →
      `attached` latches, `length = NRAttachLength`, and **exactly 11 × [`rand(128)` + a type-2
      `Create2`]** are drawn (assert the count), then `vel.Zero()`; (d) rope reaching a **rock /
      level edge** with no `AnyDirt` → attach, **no spray, no rand**; (e) **worm-anchor** branch →
      `anchor.vel -= kForce/cur_len` (when `cur_len > length`), `vel = anchor.vel`,
      `pos = anchor.pos`; (f) the attached tail adds `owner.vel += kForce/cur_len`.
- [ ] **GREEN:** port `ninjarope_process` (`ninjarope.cpp:7-82`) — `pos += vel`; anchor re-scan
      via `check_for_spec_worm_hit(ipos, 1)` over the OTHER worms; `kDiff`/`kForce`/`cur_len`
      (`VectorLength`); the terrain-attach / worm-anchor / detached branches; the attached-vs-
      gravity tail. Wire it as a loop **after** the worm loop and **before** the game-mode switch
      in `process_frame`, indexing `worms` (owner + anchor reachable). Reuse `nobject_create2`.
- [ ] **GREEN:** `sim_slice3` golden (and any T0-flagged rope-throwing fuzz golden) now matches
      the regenerated golden tick-for-tick.
- [ ] Reviewer (Opus): the `rand(128)` ×11 lands at the exact `:41-45` point inside the
      `!attached && Inside && AnyDirt` guard; the loop order (after worms, before modes); the
      `kForce/cur_len` truncating divides; owner AND anchor `vel` mutations correct; the
      regenerated `sim_slice3` diff onset == its first rope-out tick.

### T3 — Chain-loop port (`sobject.cpp:217-227`, deferral #1)  [Opus]

- [ ] **RED:** unit tests against a seeded `Rand`: a bonus inside an explosion's `detect_range`
      is **freed** and replaced by a `sobject_types[0].Create` at its `Ftoi(pos)`; a bonus out of
      range is untouched; two bonuses (one in, one out) → exactly one freed; the recursion
      terminates (the booby `Create` scanning a now-smaller pool does not loop forever).
- [ ] **GREEN:** thread `&mut Pool<Bonus>` into `sobject_create`; after the damage block, scan
      `bonuses` in slot order and for each in range `free(slot)` + recurse
      `sobject_create(&sobject_types[0], ix, iy, ...)`. Forward `&mut bonuses` from every caller
      (blow_up in the wobjects loop, worm death, pickup booby, nobject explode). **Remove the
      chain-loop tripwire.** Keep the O9 `chain_explosion → BlowUpObject` `debug_assert!` (a
      different chain) unless T9 reaches it.
- [ ] Reviewer (Opus): the `&mut bonuses` threads cleanly (no aliasing) through the whole
      call graph; recursion terminates; the freed-then-Create order matches `:224-225`; RNG order
      preserved (the recursive `Create`'s sound draw at the C++ point).

### T4 — `new_object_reuse` overwrite for sobjects + wobjects (deferral #2)  [Sonnet]

- [ ] **RED:** unit tests: filling a `sobjects` (700) / `wobjects` (600) pool to cap then
      spawning once more **overwrites the oldest slot** (matching `FixedObjectList::NewObjectReuse`
      / the 5d bobject/nobject behaviour) instead of failing; iteration order after overwrite
      matches C++ `All()`.
- [ ] **GREEN:** replace the `.expect("pool not full")` in `sobject_create` (sobject.rs:134) and
      `worm_fire` (weapon.rs) with the overwrite-on-full path (reuse the 5d `Pool` primitive).
- [ ] Reviewer (Sonnet→Opus spot-check): the overwrite slot selection matches C++; no hash
      contract change (the reused slot is deterministic).

### T5 — Game modes: GameOfTag + Scales hooks + goldens (JOHN-BESLUT #1)  [Opus]

- [ ] **RED:** unit tests: (a) the GameOfTag per-tick gate (`game.cpp:373-388`) bumps
      `last_killed_by->timer` only when `!some_invisible && (cycles%70)==0 && timer <
      time_to_lose` — rand-free, `timer` hashed; (b) the Scales `DoDamage` redistribution
      (`game.cpp:570-587`) heals the OTHER worms via `DoHealingDirect` — un-defer the branch in
      `state.rs:do_damage` (426-431); rand-free, `health` hashed.
- [ ] **GREEN:** port the GameOfTag gate + Scales redistribution behind the game-mode switch in
      `process_frame` (default KillEmAll = no-op); thread `game_mode` + `time_to_lose` consts.
      **Holdazone `SpawnZone`/`SelectSpawn` stays DEFERRED** (design §5 / JOHN-BESLUT #1) — leave
      a documented `unimplemented!`/guard for the Holdazone arm.
- [ ] Author `sim_slice6_gametag_scenario.txt` (`game_mode 1`) + `sim_slice6_scales_scenario.txt`
      (`game_mode 3`, a worm takes damage so redistribution fires); gen + commit their goldens;
      difftests assert master + 9 components bit-exact.
- [ ] Reviewer (Opus): the GameOfTag `cycles%70` gate + the Scales heal-others order match C++;
      Holdazone deferral documented; goldens honest. **Surface JOHN-BESLUT #1 in the done-report.**

### T6 — cossin[128] disposition + T8 truncating-division test (deferrals #3, #4)  [Sonnet]

- [ ] **cossin[128] (deferral #3):** document in-code (a comment at `worm_fire`) that the fuzz's
      respawn-seeded aim makes `cossin[128]` unreachable (§8), and record the residual hardening
      as deferred past Step 2. *Optional (if John picks belt-and-suspenders in JOHN-BESLUT #3):* a
      one-line clamp/guard on `Ftoi(aiming_angle)` + a unit test.
- [ ] **T8 gap (deferral #4):** add a unit test for the pickup heal
      `(rand(BonusHealthVar) + BonusMinHealth) * settings_health / 100` with `settings_health`
      NOT a multiple of 100 (e.g. 150) — pins the truncating divide (`worm.cpp:295`).
- [ ] Reviewer (Sonnet): the cossin argument is sound; the truncation test is non-vacuous.

### T7 — >1000-tick fuzz scenarios + gen + goldens  [Opus]

- [ ] Author **3–5** fuzz variants `sim_slice6_fuzz{1..N}_scenario.txt` mirroring
      `test_determinism.cpp`: 2 worms seeded DEAD (`visible 0`, `pos 0 0`, `killed_timer 150` via
      ResetWorms, high `lives`), `max_bonuses 4`, a **loaded** real arena `.lev` (NOT
      GenerateFromSettings), **> 1000 ticks** (e.g. 1500), per-tick per-worm input
      `input_rng() & 0x7f` (add an `input_seed`/fuzz-input directive to the dumper OR pre-expand
      the inputs into the scenario — pick the lower-divergence option and document it). Vary the
      game `seed` and `input_rng` seed across variants.
- [ ] `gen_sim_slice6_fuzz{1..N}.sh` (copy the 5′ fuzz gen pattern; rebuilt dumper; `PRESET`
      default `macos-arm64`). Generate + commit the goldens. Tune seeds with `OL_PHYS_TRACE` so
      each variant actually produces: ≥1 death, ≥1 respawn, ≥1 bonus pickup, ≥1 in-flight hit,
      ≥1 ninjarope attach, and pool pressure (a blood storm nearing cap).
- [ ] Reviewer (Opus): the worms genuinely respawn (not hand-placed); `aiming_angle` never 0
      (cossin safe); the level is loaded not generated; the input model is `& 0x7f` verbatim; the
      target events occur.

### T8 — Fuzz difftest — MILESTONE (full ProcessFrame bit-exact >1000 ticks)  [Opus]

- [ ] `tests/sim_slice6_fuzz.rs`: expected from each golden (all 11 columns); actual from a
      genuinely driven `SimState` (real `.lev`/`tc.cfg`/`Objects::load`, worms via the full
      respawn path, `SimState::new` + post-`new` consts); components before master; input keyed
      `k-1`; **all > 1000 ticks** per variant.
- [ ] Coverage guards (non-vacuous, from driven state, never re-parsed from the golden): a worm's
      `lives` **decreases** (death); the `rng` column **moves on a respawn-search tick**; the
      `bonuses` pool **grows then shrinks** (drop + pickup); a worm's `health` **drops on an
      in-flight-hit tick**; the `nobjects` column **bumps on a ninjarope-attach tick** (the
      `rand(128)` spray); `nobjects`/blood approach but never exceed cap (`new_object_reuse`
      exercised). **Optional backstop:** two `SimState` runs per seed asserted hash-identical
      every tick.
- [ ] **MILESTONE:** master + all 9 component hashes bit-exact every tick vs the C++ oracle across
      all variants; `cargo test --workspace` green. **Re-confirm deferral #7** (multi-worm
      destroy+explode ordering) — the fuzz's explosions with multiple worms empirically close it;
      if any variant diverges, investigate before claiming the milestone.
- [ ] Reviewer (Opus): honesty (expected from golden, actual from driver); non-vacuous guards;
      the ninjarope + game-mode + chain-loop paths are actually hit; "could it pass while the sim
      is wrong?"; deferral #7 re-confirmed or opened.

### T9 — Step-2-wide broad review + PR #3 readiness  [Opus broad review]

- [ ] `cargo test --workspace` green (all `sim_slice*` incl. slice 6; priors re-diffed per T0).
- [ ] `sim` float-free (`grep -rn 'f32\|f64' rust/sim/src` empty); deps = `sim-core` + `assets`
      only.
- [ ] **Prior-golden ledger (NOT "git diff empty" this slice):** record which goldens were
      byte-identical and which were regenerated (rope-throwing), each with its first-divergence
      tick — the JOHN-BESLUT #2 transparency artifact.
- [ ] Update `docs/superpowers/liero-rs-PROGRESS.md` + the overview's slice-6 line (full
      `ProcessFrame` live: ninjarope loop + game-mode switch ported; chain-loop closed;
      sobjects/wobjects reuse-overwrite; the >1000-tick fuzz bit-exact) + the SDD ledger (all 10
      deferrals resolved/deferred with rationale; the render-only omissions proven inert; the
      full-regen transparency note; Holdazone + cossin-guard + items 5/8/9/10 deferred past
      Step 2).
- [ ] Whole-step broad review (Opus): the full `ProcessFrame` matches `game.cpp:267-471` piece
      for piece; RNG order end-to-end; no hidden float; the deferral ledger is honest. Then push
      slice 6 to PR #3, update the PR body, and mark **Step 2 complete**.

## Done-report (each task)

(a) what changed + why, (b) files touched, (c) tests/risks. Per-task commit, local. Push the
whole slice to PR #3 after the broad review. Controller owns push + PR.
