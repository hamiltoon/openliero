# Step 2, Slice 5′ — Per-pixel worm-hit + in-flight arms + pickup: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking. **Test-first**: write the failing
> test (or its assertion) before the implementation it pins.

**Goal:** Land the **per-pixel `CheckForSpecWormHit`** and the two **in-flight worm-hit
arms** (wobject `weapon.cpp:287-326`, nobject `nobject.cpp:166-203`) that hang off it, plus
the 5c-deferred **bonus pickup** (`worm.cpp:287-322`), so the Rust sim reproduces the C++
master `HashGameState` **and** all 9 component hashes **tick-for-tick** when a **moving
projectile wounds a worm on sprite contact** and when a **worm walks onto a bonus**. Per-pixel
worm-hit replaces 5a's 16×16 box over-approximation (`nobject.rs:250-265`), which over-fired
(the `fd33bbc`/5b-T5b blocker: transparent-corner + muzzle false positives). Needed for
open-gate weapons (dart/cannon) and the slice-6 fuzz with moving worms.

**Architecture:** Extend `rust/sim/` (deps unchanged: `sim-core`, `assets`; Bevy-free,
float-free). Sim changes only — **no C++ dumper edit** (the arms + pickup live inside the
unmodified `WObject::Process`/`NObject::Process`/`Worm::Process`, which the dumper already
drives every tick; see the design's "no dumper change" finding). Build the worm-sprite bank
in `SimState::new` from `large_sprites` (`common.cpp:509-537`); add `MAT_WORM`; promote
`animate` + compute `current_frame`; replace the box `check_for_spec_worm_hit` with the
per-pixel loop; re-apply the `fd33bbc` wobject arm and replace the nobject `debug_assert!`
body, both gated by the per-pixel test; port pickup into the worm loop. Then scenarios +
goldens + difftests (milestones) and a moving-worms fuzz.

**Tech stack:** Rust (`sim` extend, `oracle-tests`). Goldens generated **LOCALLY/MANUALLY**
via the already-built dumper (`OPENLIERO_BUILD_ORACLE_DUMP`, **unchanged binary**); CI
(`cargo test --workspace`) runs the committed goldens. `data/TC/openliero` real TC.

**BASE commit for T0: `256cd19`** (branch `liero-rs-step-2`; slices 1–4 + 5a–5d shipped
bit-exact).

## ✅ No dumper change, no controller golden-gate

Like 5a and 5d (and unlike 5b/5c), 5′ requires **zero C++ dumper edits**: the per-pixel
predicate reads `common.WormSprite(...)` + `common.materials[...]` (the dumper's `Common`
loads the whole TC); the in-flight arms and pickup live inside the already-compiled `Process`
functions the dumper drives unmodified (`sim_physics_dump.cpp:394-398`); `settings->blood ==
100` (default) already matched in 5b. Therefore **slices 1–5d re-diff byte-identical
trivially** — the literal *"git diff empty"* prior-slice gate holds. All work is Rust-side.

## Global constraints

- **Bit-exact vs C++.** Sources of truth: `worm.cpp:1162-1188` (`CheckForSpecWormHit`),
  `:454-473` (`AngleFrame`), `:429-430` (`current_frame`), `:287-322` (pickup),
  `weapon.cpp:287-326` (wobject arm), `nobject.cpp:166-203` (nobject arm),
  `common.cpp:509-537` (worm-sprite precompute), `material.hpp:12,25` (`kWormM`/`Worm()`),
  `settings.cpp:15` (`kWormAnimTab = {0,7,0,14}`), `nobject.cpp:7-66` (`Create2` RNG),
  `stateHash.hpp` (folds). **RNG order is the contract.**
- **The two arms' RNG order DIFFERS — do not unify:**
  - **wobject** (`weapon.cpp:287-326`): gate → vel-kick (no rand) → `DoDamage` (no rand) →
    **blood fan** (`blood_on_hit*blood/100` × [`rand(128)` + `Create2`]) → **hit-sound gate**
    (`rand(3)==0` → `rand(3)` sound, inner always taken) → `worm_collide` (`rand(worm_collide)`).
  - **nobject** (`nobject.cpp:166-203`): gate → vel-kick → `DoDamage` → **hit-sound gate**
    (`rand(3)` then `rand(3)`) → **blood fan** → `worm_explode`/`worm_destroy`.
  - i.e. wobject = **blood then sound**; nobject = **sound then blood**. Load-bearing.
- **Per-pixel predicate is NOT the box.** `check_for_spec_worm_hit` must intersect the
  `±dist` rect with `Rect(0,0,16,16)` **then** scan pixels, returning `true` only on a
  `material_flags[worm_sprite[cy*16+cx]] & MAT_WORM` pixel. Index `material_flags`
  **directly** by the sprite pixel (NOT via `material_id[]` — that is level-only). The worm
  is `worm_sprite = WormSprite(current_frame, direction, 0)` (**colour 0 always**).
- **`current_frame` is unhashed but load-bearing.** `current_frame = AngleFrame() +
  kWormAnimTab[animate ? ((cycles & 31) >> 3) : 0]`. Do NOT add it (or `animate`) to any
  hash. It is pinned only transitively (a wrong frame → wrong hit → wrong `rng`/`health`).
- **`animate` promotion.** Currently render-only-skipped (`control.rs:469`). Add the field;
  wire `true` at `worm.cpp:870,886` (walk/move), `false` at `:954` (idle) and `:1073`
  (fire). Default `false`.
- **Truncating division / shifts.** `Ftoi`=`>>16`; `x >> 3` in `AngleFrame` is arithmetic
  shift on a signed `int`; `blood_on_hit*blood/100`, `vel*blow_away/100`, `vel/3` are integer
  `/`. `(cycles & 31) >> 3` is mask-then-shift. Match exactly.
- **Pools / O3.** `new_object_reuse` overwrite is **live since 5d** — the fuzz's blood storms
  reuse it; no new pool work.
- **Game mode.** `KillEmAll` only exercised; `Scales`/`GameOfTag` pickup/damage branches
  present-but-guarded.
- **Scenario is the single source of truth**, read by both the (unchanged) dumper and the
  Rust test. Golden regen LOCAL/MANUAL; `PRESET` defaults `macos-arm64`.
- **No AI / "Generated with" taglines.** **Bash discipline:** one command per call; no
  `>>`/heredoc/`&&`/`;`/`$VAR`; no `cd`+`git`; create files with the editor.

## File structure

- `rust/assets/src/sprite.rs` (or a `SimState::new` build step) — construct the worm-sprite
  bank (16×16 ×42 colour-0, or ×84) from `large_sprites[16+i]` per `common.cpp:509-537`.
- `rust/sim/src/state.rs` — `SimState.worm_sprites`; `MAT_WORM` const + per-pixel accessor;
  `WormState.current_frame` + `animate`; `angle_frame()` + `WORM_ANIM_TAB`; compute
  `current_frame` at the end of the visible arm; thread pickup consts; pickup in the worm
  loop.
- `rust/sim/src/control.rs` / `physics.rs` — set `animate` at the four C++ sites (promote
  from skipped).
- `rust/sim/src/nobject.rs` — replace the box `check_for_spec_worm_hit` (`:250-265`) with
  the per-pixel loop; replace the deferred worm-hit `debug_assert!` body (`:496-509`) with
  the nobject arm.
- `rust/sim/src/weapon.rs` — re-apply the `fd33bbc` wobject in-flight arm in
  `wobject_process`, per-pixel-gated; extend its signature to thread
  `worms`/`nobjects`/`nobject_types`/`blood`/`cossin`/`worm_sprites`/`material_flags`.
- `rust/oracle-tests/golden/sim_slice5prime*_scenario.txt` + `.txt` goldens +
  `gen_sim_slice5prime*_golden.sh` + `tests/sim_slice5prime*_golden.rs` /
  `sim_slice5prime_fuzz.rs`.
- Slices 1–5d goldens — **unchanged** (re-diff to prove it; do NOT regenerate).

## Tasks

### T0 — Dumper no-change verification + re-diff 1–5d byte-identical  [Opus]

- [ ] Confirm from `sim_physics_dump.cpp` that the per-tick driver calls the **unmodified**
      `w->Process(game)` (`:394-398`) and the object `Process` loops, and that
      `CheckForSpecWormHit` + the arms + pickup are all inside those functions (nothing
      gated behind a dumper-omitted flag; `common.materials`/`WormSprite` fully loaded).
      Conclude: **no C++ edit needed**.
- [ ] Re-diff: regenerate slices 1–5d goldens via their gen scripts against the (unchanged)
      dumper binary; assert **git diff empty**. Trivial (no dumper change) but the
      transparency proof. Do NOT commit any change to those goldens.
- [ ] Reviewer (Opus): the "no dumper change" claim is real (the worm-hit + pickup code is
      entirely inside the `Process` functions, not behind anything the dumper omits).

### T1 — Data foundation: worm-sprite bank + `MAT_WORM` + `current_frame`/`animate`  [Opus]

- [ ] **RED:** unit tests: (a) the built worm bank matches C++ `WormSprite(f,0,0)` /
      `WormSprite(f,1,0)` pixel-for-pixel for a few frames (the mirror for `dir=0`, the
      straight `dir=1`) against `large_sprites[16+f]`; (b) `angle_frame()` matches
      `worm.cpp:454-473` across `aiming_angle`×`direction` (incl. the `<0`/`>6` clamps and
      the `dir!=0` `6-x` flip); (c) `current_frame` for an **idle** worm (`animate=false`) =
      `angle_frame()` (offset 0) and for a **moving** worm (`animate=true`) =
      `angle_frame() + WORM_ANIM_TAB[(cycles&31)>>3]`; (d) `MAT_WORM = 0x20` and the accessor
      returns true only on a `Worm`-flagged palette index; (e) slices 1–5d difftests stay
      **byte-identical** (all new fields unhashed).
- [ ] **GREEN:** build `SimState.worm_sprites` in `new` from `large_sprites` per
      `common.cpp:509-537` (colour-0 sub-bank required; port full ×84 for fidelity or
      tripwire the `w=1` path). Add `MAT_WORM = 1 << 5` + `fn worm_pixel(pal: u8) -> bool`.
      Add `WormState.current_frame: i32` + `animate: bool` (defaults: fresh worm → frame 0,
      `animate=false`). Port `angle_frame()` + `WORM_ANIM_TAB = [0,7,0,14]`. Wire `animate`
      at `worm.cpp:870,886` (true), `:954,1073` (false) — the promotion from render-skipped.
      Compute `current_frame` at the END of the visible arm (`worm.cpp:428-430`), reading the
      pre-`++cycles`/`cycles` value the animation gate uses (verify which `cycles` snapshot).
- [ ] Slices 1–5d difftests **green and byte-identical**.
- [ ] Reviewer (Opus): the bank build (mirror + colour-swap offsets), `angle_frame` clamps +
      flip, the `animate` sites all wired, `current_frame` placement + the `cycles` snapshot,
      none of the new fields hashed, priors unchanged. **Also decide/verify the tick order:
      do the object loops (which read `current_frame`) run before or after the worm loop
      (which writes it)? Match the C++ `processFrame` order exactly** — load-bearing for the
      contact tick.

### T2 — Per-pixel `CheckForSpecWormHit` (replace the box over-approx)  [Opus]

- [ ] **RED:** unit tests (non-tautological): invisible worm → false; a solid silhouette
      pixel inside the `±dist` rect → true; **the `fd33bbc` transparent-corner case** — a
      point whose `±dist` rect overlaps the 16×16 box ONLY at a background/transparent pixel
      (e.g. col 15 of a frame) → **false** (the box returned true; per-pixel must return
      false); a worm far out of range → false. Drive with real `worm_sprites` +
      `material_flags` for a known frame.
- [ ] **GREEN:** replace the box `check_for_spec_worm_hit` body (`nobject.rs:250-265`) with
      the per-pixel loop (`worm.cpp:1162-1188`): `worm_sprite = WormSprite(current_frame,
      direction, 0)`; deltas `+7`/`+5`; `Rect(delta±dist, +1)` intersected with `(0,0,16,16)`;
      scan `cy,cx`, return true on the first `worm_pixel(worm_sprite[cy*16+cx])`. Signature
      gains `&worm_sprites` + `&material_flags` (thread from `SimState`).
- [ ] Slices 1–5d difftests still **byte-identical** (5a/5b/5d never had a worm in range, so
      box and per-pixel both returned false there — verify the re-diff is empty).
- [ ] Reviewer (Opus): the intersect-then-scan order, `material_flags` indexed by the sprite
      pixel **directly**, colour `w=0`, the transparent-corner test genuinely distinguishes
      per-pixel from box.

### T3 — wobject in-flight worm-hit arm (`weapon.cpp:287-326`, re-apply `fd33bbc`)  [Opus]

- [ ] **RED:** unit tests against a seeded `Rand`: on a per-pixel hit with the dart params
      (`hit_damage=5`, `blood_on_hit=10`, `blow_away=28`), the arm draws **blood fan first**
      (`10*blood/100` × [`rand(128)` + `Create2` blood sub-draws]) **then** the sound gate
      (`rand(3)`; on 0 also `rand(3)`); asserts `worm.vel` kicked, `DoDamage` applied
      (`health -= 5`), the exact draw count; a **no-hit** (per-pixel false) draws **nothing**;
      the `worm_collide` branch (for a fan) draws `rand(worm_collide)`.
- [ ] **GREEN:** re-apply the `fd33bbc` arm in `wobject_process` after the timeout, gated by
      the new per-pixel `check_for_spec_worm_hit`. Extend the signature to thread
      `&worms`/`&mut nobjects`/`nobject_types`/`settings_blood`/`cossin`/`worm_sprites`/
      `material_flags`. Route blood through `nobject_create2` (5b). Return
      `Explode`/`Remove` on the `worm_collide` outcome (the `WObjectOutcome` contract).
- [ ] Reviewer (Opus): **blood-before-sound** order, the inner `rand(3)` always taken on the
      gate, `DoDamage` sets `last_killed_by_idx` only on a kill, the vel-kick uses
      `vel*blow_away/100` integer, no self-hit regression on the fire tick (per-pixel keeps
      the muzzle clear unless C++ hits too).

### T4 — nobject in-flight worm-hit arm (`nobject.cpp:166-203`, replace the panic)  [Opus]

- [ ] **RED:** unit tests against a seeded `Rand`: on a per-pixel hit with a splinter
      (`hit_damage=2`), the arm draws the **sound gate first** (`rand(3)`; on 0 also
      `rand(3)`) **then** the blood fan (`blood_on_hit*blood/100` × [`rand(128)` + `Create2`])
      — the **opposite order to T3** (assert the transposition explicitly); `worm.vel`
      kicked, `DoDamage` applied; `worm_explode`/`worm_destroy` outcomes; a no-hit draws
      nothing (the OLD `debug_assert!(false)` is gone).
- [ ] **GREEN:** replace the deferred body (`nobject.rs:496-509`) with the arm
      (`nobject.cpp:172-199`), per-pixel-gated. Reuse `nobject_create2` + `do_damage`.
- [ ] Reviewer (Opus): **sound-before-blood** (vs T3's blood-before-sound), the
      `!do_explode && hit_damage > 0` outer guard, `worm_destroy && used` free, the panic
      fully removed.

### T5 — wobject scenario + gen script + committed golden  [Opus]

- [ ] Author `sim_slice5prime_scenario.txt`: **`dart`** (open-gate) in worm0 slot 0; worm0
      fires so the descending dart crosses worm1's **solid silhouette** (resurrect the 5b
      dart geometry, ledger `:477`); worm1 **idle** (`animate=false`) at reduced health so the
      hit **wounds without killing** (health e.g. 50 → 45; keep the 5d death path out).
      Window ~60–120 ticks. Tune via `OL_PHYS_TRACE`; assert the contact tick fires the fan.
      **Add a near-miss counterpart** (dart grazing the transparent corner — the tick-49
      case) that fires **nothing** (the anti-false-positive witness).
- [ ] `gen_sim_slice5prime_golden.sh` = faithful copy of `gen_sim_slice5d_golden.sh` (exec,
      LOCAL/MANUAL, `PRESET` default `macos-arm64`), pointing at the (unchanged) dumper.
- [ ] Generate + commit `golden/sim_slice5prime.txt`. Inspect directly: `rng` flat until the
      contact tick, then a burst (10 blood ×4 + `rand(3)` gate); worm1 `health` −5, `vel`
      kicked; `nobjects` gains 10 (type 6) then drains; the near-miss ticks flat.
- [ ] Reviewer (Opus): the golden is self-consistent; the contact is a genuine per-pixel hit
      (not the box); the near-miss really fires nothing; `nobjects < 600`.

### T6 — `sim_slice5prime_golden` difftest (MILESTONE 5′a)  [Opus]

- [ ] Mirror `sim_slice5d_golden.rs`: expected from the golden (all 11 columns); actual from
      a genuinely driven `SimState` (real `.lev`/`tc.cfg`/`Objects::load`, **`dart` by
      name**, `id==index`, full `SimState::new`); components before master; input keyed
      `k-1`; all ticks.
- [ ] Coverage guards (non-vacuous, from driven state): worm1 `health` **drops by exactly
      `hit_damage`** on the contact tick; `rng` **bursts on the contact tick** and is flat on
      the near-miss ticks (**the per-pixel discrimination witness — this directly pins the
      `fd33bbc` blocker as FIXED**); worm1 `vel` **changes** on the contact tick;
      `nobjects > 0` on the contact tick (blood type 6) and `< 600`.
- [ ] **Milestone:** master + all 9 component hashes bit-exact every tick vs the C++ golden
      **and** slices 1–5d re-run byte-identical (git diff empty). `cargo test --workspace`
      green.
- [ ] Reviewer (Opus): honesty (expected from golden, actual from driver), non-vacuous
      guards, the wobject RNG order, "could it pass while the sim is wrong?" — especially the
      near-miss (a box over-approx would FAIL the near-miss ticks; per-pixel passes them).

### T7 — nobject-arm scenario + golden (cannon splinters)  [Opus]

- [ ] Author `sim_slice5prime_nobj_scenario.txt`: **`cannon`** fired so a splinter
      (`hit_damage=2` nobject) crosses worm1's silhouette. Generate + commit its golden.
      Reuse the milestone harness in a second difftest (`sim_slice5prime_nobj_golden.rs`)
      asserting master + 9 components bit-exact — the guard for the **nobject sound-before-
      blood order** (the mirror of T6's wobject order). (Controller may fold this into the
      T10 fuzz instead — see design JOHN-BESLUT #2 recommendation; a dedicated golden is
      recommended so the order asymmetry has a named milestone.)
- [ ] Reviewer (Opus): the splinter genuinely hits per-pixel; the nobject RNG order matches;
      goldens honest.

### — 5′a | 5′b cut — (design JOHN-BESLUT #1: ship 5′a here if decomposed) —

### T8 — Bonus pickup (`worm.cpp:287-322`)  [Opus]

- [ ] **RED:** unit tests against a seeded `Rand`: the **11×11 AABB** gate (`ipos±5` vs
      `Ftoi(bonus)`); **health bonus** (frame 1) with `health < settings_health` →
      `rand(BonusHealthVar)` + `DoHealing((rand+BonusMinHealth)*settings_health/100)` (worm
      heals, bonus freed); with `health >= settings_health` → **no free, no rand**; **weapon
      bonus** (frame 0) → **always** `rand(BonusExplodeRisk)`; `> 1` → reload (`ww.type`/
      `ammo` set, `fire_cone=0` unless `HBonusReloadOnly`, bonus freed, no further rand);
      `<= 1` → booby `sobject_types[0].Create` (its own draws) + free. Assert each branch's
      exact draw count; iterate multiple bonuses in pool order.
- [ ] **GREEN:** port `worm.cpp:287-322` inside the `if visible` worm body (after the
      reaction-force block, `:285`). Thread `BonusHealthVar`/`BonusMinHealth`/
      `BonusExplodeRisk` + `HBonusReloadOnly` from the TC (post-`new`, no signature churn).
      Reuse `do_healing_direct` (or port it — RNG-free clamp) + `sobject_create` (5b). Remove
      the 5c pickup tripwire. Keep the chain-loop tripwire (booby sobject may reach it).
- [ ] Reviewer (Opus): the box test, the health `< settings_health` guard (no rand when
      full), the weapon `rand(BonusExplodeRisk)` **always** drawn, the reload sets hashed
      `ww.type→id`/`ammo`, `bonuses.Free` shrinks the pool, the booby branch spawns the
      sobject, integer `*settings_health/100`.

### T9 — Pickup scenario + gen + golden + difftest (MILESTONE 5′b)  [Opus]

- [ ] Author `sim_slice5prime_pickup_scenario.txt`: reuse 5c's bonus-drop (`max_bonuses ≥ 1`,
      seed tuned to drop early), then **walk worm0 onto the bonus box**. Variant (a) health
      bonus (worm0 `health < settings_health` → heals); variant (b) weapon bonus → reload.
      Force frame via `HBonusOnlyHealth`/`HBonusOnlyWeapon` or seed. Keep the booby branch to
      unit tests (thin chain-loop path) unless the controller wants it in-golden. Generate +
      commit the golden(s).
- [ ] `sim_slice5prime_pickup_golden.rs` — milestone-style difftest: master + 9 components
      bit-exact all ticks. Guards: worm0 `health` **jumps up** (health) or `ww` **changes**
      (weapon); `bonuses` pool **shrinks by 1** on the pickup tick; `rng` draws the pickup
      roll; the other worm flat.
- [ ] **Milestone (5′b):** bit-exact + priors byte-identical.
- [ ] Reviewer (Opus): the pickup is a genuine walk-on (not a frozen worm), the health/weapon
      branch RNG matches, honest difftest.

### T10 — Moving-worms fuzz (O21; slice-6 precondition)  [Opus]

- [ ] Author **3–4** fixed-level multi-seed fuzz variants
      (`sim_slice5prime_fuzz{1..4}_scenario.txt`) on `physics_fall_test.lev` where **both
      worms move and fire**, so in-flight hits land at **varied `current_frame`/`direction`/
      positions** — the coverage the idle-victim milestone cannot give, and the guard for the
      `animate`/`current_frame` promotion (design O-A risk). Generate + commit goldens.
- [ ] `sim_slice5prime_fuzz.rs` — reuse the milestone harness over the variants; each asserts
      master + 9 components bit-exact all ticks. Coverage: the variants collectively land
      hits at **≥ 2 distinct `current_frame`/direction combos** (the moving-worm sprite
      selection is genuinely exercised). Blood storms reuse the 5d `new_object_reuse`
      overwrite.
- [ ] **Optional cheap backstop:** a pure-Rust determinism guard (two `SimState` runs per
      seed asserted hash-identical every tick).
- [ ] Reviewer (Opus): the variants really move the worms (not idle copies); hits land at
      varied frames; goldens honest. **This fuzz is the direct precondition for slice-6 fuzz
      with moving worms — note that in the done-report.**

### T11 — Controller done-check + docs/ledger  [Opus broad review]

- [ ] `cargo test --workspace` green (incl. all `sim_slice5prime*` tests + slices 1–5d
      **unchanged**).
- [ ] `sim` float-free (`grep f32/f64` empty), deps = `sim-core` + `assets` only.
- [ ] **Prior-slice gate (literal):** slices 1–5d goldens **byte-identical** (git diff
      empty) — recorded explicitly (no dumper change at all in 5′).
- [ ] Update `docs/superpowers/liero-rs-PROGRESS.md` + the overview's slice-5 decomposition
      (5′ DONE; per-pixel worm-hit + both in-flight arms + pickup live; the box over-approx
      retired) + the SDD ledger (per-pixel `CheckForSpecWormHit` live; wobject/nobject
      in-flight arms landed — `fd33bbc` re-applied; `animate`/`current_frame` promoted;
      pickup closes 5c; the no-dumper-change finding; the wobject-vs-nobject RNG-order
      asymmetry).
- [ ] Then broad whole-slice review (Opus) → push 5′ (or 5′a then 5′b, per JOHN-BESLUT #1)
      to PR #3 + update PR body.

## Done-report (each task)

(a) what changed + why, (b) files touched, (c) tests/risks. Per-task commit, local.
Push the whole sub-slice to PR #3 after the broad review.
</content>
</invoke>
