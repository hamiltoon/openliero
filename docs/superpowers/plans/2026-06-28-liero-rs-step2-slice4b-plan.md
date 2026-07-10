# Step 2, Slice 4b — Terrain destruction (`DrawDirtEffect`): Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking. **Test-first**: write the failing
> test (or its assertion) before the implementation it pins.

**Goal:** Port `BlowUpObject`'s `dirt_effect` branch → **`DrawDirtEffect`** for
**`greenball`** (the *modify-terrain-with-zero-secondary-objects* weapon) into the
`sim` crate, so the Rust sim reproduces the C++ master `HashGameState` **and** all
component hashes **tick-for-tick** — including the **`level` component hash now
moving** at the explode tick. This is the slice where the **level-hash goes live**
(first mid-run terrain change since Slice 1). Everything else (Fire, flight,
collision, explode, pool free, the `process_frame` driver, the `weapon` directive)
is **unchanged from 4a** — 4b only adds the dirt-effect tail of `BlowUpObject` plus
the assets it reads.

**Architecture:** Extend `rust/sim/` (no new crate; deps unchanged: `sim-core`,
`assets`; Bevy-free, float-free). `SimState` carries the `large_sprites` bank +
`textures` table; `LevelSim` gains `background`/`any_dirt`/`dirt`/`dirt2` reads +
`set_material` (the first `material_id` writer). New `draw_dirt_effect` (in
`weapon.rs` or a new `blit.rs`) ports `gfx/blit.cpp:534-622`. `blow_up` (4a) gains
the `dirt_effect>=0 ⇒ draw_dirt_effect` branch. The C++ dumper gains **one line**
(`settings->shadow=false`) to omit `CorrectShadow` (O4); the
slice-1/2/3/4a goldens must stay byte-identical.

**Tech stack:** Rust (`sim` extend, `oracle-tests`) + one oracle-gated C++ dumper
line (non-sim). Golden regenerated locally via the (already-extended) dumper
(`OPENLIERO_BUILD_ORACLE_DUMP`); CI (`cargo test --workspace`) runs the committed
golden. `data/TC/openliero` real TC; weapon **greenball**.

## Global constraints

- **Bit-exact vs C++.** Sources of truth: `weapon.cpp:78-125` (`BlowUpObject`, the
  `dirt_effect` branch `:117-124`), `gfx/blit.cpp:534-622` (`DrawDirtEffect`),
  `gfx/blit.hpp:39-47` (`Texture` fields + their semantics), `material.hpp:5-27`
  (flag bits + `Background`/`AnyDirt`/`Dirt`/`Dirt2`/`DirtRock`), `level.hpp:36-90`
  (`material_id`, `SetPixel`), `tc.cfg:134-195` (the 9 textures), `hash.rs:36`
  (`fold_level` = the level hash over `material_id`).
- **RNG order is the contract.** Greenball draws: Fire = spread `vel.x`
  `rand(16000)`, spread `vel.y` `rand(16000)`, colour `rand(2)` (`start_frame<0`);
  **no** time-var (`time_to_explo_v=0`). On explode, **`DrawDirtEffect` draws exactly
  one `rand(tex.r_frame)=rand(2)` at the top, before any pixel write** (`blit.cpp:537`).
  Thread the one `sim-core::Rand` through; never pull ad hoc. (Design, *RNG audit*.)
- **Level goes live — and only here.** greenball `create_on_exp=-1`/`splinter_amount=0`,
  worms out of `detect_distance=3` (non-firing worm invisible) ⇒ no sobject/nobject/
  blood/splinter, no worm-hit RNG. The **only** new `material_id` writer is
  `draw_dirt_effect`; the **only** new RNG is its `rand(2)`.
- **`n_draw_back=false` writes Background cells only.** Greenball texture 6 deposits
  dirt into Background (`blit.cpp:584-621`); the scenario must straddle the surface or
  `level` never moves. Port the `n_draw_back=true` carving half too (complete fn),
  exercised only in unit tests this slice.
- **CorrectShadow OMITTED** (O4): `settings->shadow=false` in the dumper. `CorrectShadow`
  (gated on `settings->shadow`) writes `material_id` and IS reachable from the dumper's
  Process loop (worm dig, dirt-effect/`expl_ground` explosions); it is inert to 1–4a
  ONLY because those scenarios trigger no such event in the dumped ticks — **re-diff to
  prove it**. (`MakeShadow`, the other shadow `material_id` writer, runs only via
  `GenerateFromSettings`, which the dumper never calls — a secondary point, not the reason.)
- **Only `material_id` is hashed** — no `materials` cache, no `display_valid`, no
  dirty list in the Rust port. Branch reads `material_flags[material_id[idx]]`; writes
  `material_id[idx]`.
- **`cycles` stays 0.** (greenball `num_frames=0` ⇒ no `cur_frame` anim even though
  `(cycles&7)==0` is always true; `weapon.cpp:260`.)
- **Truncating division / shifts.** `Ftoi`=`>>16` (arithmetic), `Itof`=`<<16`; `/100`
  etc. are Rust `/`, never `>>`. Same discipline as 4a.
- **Scenario is the single source of truth**, read by both the dumper and the Rust
  test. Golden regen LOCAL/MANUAL; `PRESET` defaults `macos-arm64`.
- **No AI / "Generated with" taglines.** **Bash discipline:** one command per call;
  no `>>`/heredoc/`&&`/`;`/`$VAR` chaining; create files with the editor.

## File structure

- `rust/sim/src/state.rs` — MODIFY: `SimState` carries `large_sprites:
  assets::sprite::SpriteSet` + `textures: Vec<assets::tc::Texture>`; `SimState::new`
  takes them; `LevelSim` gains `background`/`any_dirt`/`dirt`/`dirt2` + `set_material`;
  thread `large_sprites`/`textures` into the `wobjects` loop → `blow_up`.
- `rust/sim/src/weapon.rs` — MODIFY: `blow_up` gains the `dirt_effect>=0` branch →
  `draw_dirt_effect`. (New `draw_dirt_effect` here, or a new `rust/sim/src/blit.rs`
  with `pub mod blit;` in `lib.rs` — prefer a dedicated `blit.rs` so the port mirrors
  the C++ file boundary.)
- `rust/sim/src/blit.rs` — NEW (recommended): `draw_dirt_effect` + the `CLIP_IMAGE`
  helper. `pub mod blit;` in `lib.rs`.
- `rust/sim/src/lib.rs` — MODIFY: `pub mod blit;` (if used).
- `rust/oracle-tests/golden/sim_slice4b_scenario.txt` — NEW.
- `rust/oracle-tests/gen_sim_slice4b_golden.sh` — NEW (LOCAL/MANUAL).
- `rust/oracle-tests/golden/sim_slice4b.txt` — NEW (committed).
- `rust/oracle-tests/tests/sim_slice4b_golden.rs` — NEW.
- `src/tools/oracle_dump/sim_physics_dump.cpp` — MODIFY: **one line**,
  `settings->shadow = false;` after `Settings` construction. (Oracle-gated, non-sim.)

---

### Task 0: datamodel — `SimState` assets + `LevelSim` flag reads + `set_material`

De-risk the shapes before behaviour.

**Files:** `rust/sim/src/state.rs`.

- [ ] **Step 1 (test) — flag reads:** `LevelSim::background(x,y)` tests
      `material_flags[material_id[idx]] & MAT_BACKGROUND (1<<3)`; `any_dirt` tests
      bits `(1<<0)|(1<<1)`; `dirt` bit `1<<0`; `dirt2` bit `1<<1` (`material.hpp:15-23`).
      Pin with a synthetic level: a background cell, a dirt cell, a dirt2 cell, a rock
      cell — assert each predicate. (These are in-bounds reads; OOB is not exercised by
      `DrawDirtEffect` because it clips first — but mirror 4a's `dirt_rock` OOB posture
      if the helper is shared.)
- [ ] **Step 2 (impl):** add the four 1-liner predicates (reuse the 4a `MAT_*` consts).
- [ ] **Step 3 (test) — `set_material`:** `LevelSim::set_material(idx, v)` sets
      `material_id[idx]=v` and nothing else (no extra fields); a subsequent
      `background(...)`/`dirt(...)` reflects the new material via `material_flags`.
- [ ] **Step 4 (impl):** add `set_material`.
- [ ] **Step 5 (test+impl) — assets carried:** `SimState` carries `large_sprites:
      SpriteSet` + `textures: Vec<Texture>`; `SimState::new` takes them; unit-test that
      a constructed state exposes `textures[6] == {m_frame:38, r_frame:2, s_frame:82,
      n_draw_back:false}` (greenball) and `large_sprites.sprite(38)` is a 256-byte
      (16×16) slice. Update all `SimState::new` call sites (slice-2/3/4a tests) to pass
      the new args (or default-empty where objects never explode).
- [ ] **Verify:** `cargo test -p sim` green; `cargo test -p oracle-tests sim_slice1
      sim_slice2 sim_slice3 sim_slice4a` still green.

---

### Task 1: `draw_dirt_effect` (test-first) — the core port

**Files:** `rust/sim/src/blit.rs` (new) [or `weapon.rs`].

Source: `gfx/blit.cpp:534-622`. Signature `draw_dirt_effect(level: &mut LevelSim,
large_sprites: &SpriteSet, textures: &[Texture], dirt_effect: i32, x: i32, y: i32,
rand: &mut Rand)`. Reads `textures[dirt_effect]`, the large-sprite fill + mask;
writes `material_id` via `set_material`.

- [ ] **Step 1 (test) — RNG first + fill/mask selection:** with a seeded `Rand` and
      the greenball texture (6: `s_frame=82`, `r_frame=2`, `m_frame=38`,
      `n_draw_back=false`), assert `rand(2)` is consumed **before** any pixel changes
      (seed a known stream; assert `rand.last` after = exactly one `rand(2)` draw), and
      the fill sprite is `82 + that_draw` ∈ {82,83}, mask = sprite 38 (`blit.cpp:537-538`).
- [ ] **Step 2 (test) — `n_draw_back=false` (greenball) cases over Background:** on a
      synthetic level where the window is **all Background**, with a hand-built mask
      (sprite 38 stand-in) containing cells valued 6/10/2/1/other: assert case 6/10 ⇒
      `material_id = fill[((my&15)<<4)+(mx&15)]`; case 2 ⇒ `2`; case 1 ⇒ `1`; other ⇒
      unchanged (`blit.cpp:584-621`). Assert **non-Background** cells are left untouched
      (the `Background()` guard) — this is the "writes only background" property.
- [ ] **Step 3 (test) — `n_draw_back=true` (carving half):** with a `n_draw_back=true`
      texture and a level of Dirt/Dirt2 cells: case 6 ⇒ AnyDirt cell becomes fill texel;
      case 1 ⇒ Dirt2→`2`, Dirt→`1`, neither→unchanged (`blit.cpp:551-583`). (Greenball
      never hits this; it is the dig/sobject path — port + pin it now so the fn is
      complete.)
- [ ] **Step 4 (test) — texture wrap:** verify `fill[((my&15)<<4)+(mx&15)]` uses
      **level** coords `(mx,my)=(x+x_, y+y_)`, not window offsets — place the window at
      a non-(0,0) `(x,y)` and assert the sampled texel index matches the C++ wrap
      (`blit.cpp:559/593`).
- [ ] **Step 5 (test) — clip at edges:** window top-left near the level edge and past
      it; assert `CLIP_IMAGE` clamps to `Rect(0,0,width,height-1)` (note **`height-1`**,
      `blit.cpp:545`) and never writes OOB — pin the exact start/extent for a window
      straddling the right and bottom edges.
- [ ] **Step 6 (impl):** Port `draw_dirt_effect` verbatim: `rand(r_frame)` first;
      `CLIP_IMAGE` to `Rect(0,0,w,h-1)`; the `BLITL`-equivalent double loop walking the
      mask in row-major order; both `n_draw_back` branches with the cases above; the
      `((my&15)<<4)+(mx&15)` wrap; `set_material` for writes only. No `materials`/
      `display_valid`/`MarkDirty`.
- [ ] **Verify:** `cargo test -p sim` green.

---

### Task 2: `blow_up` — add the `dirt_effect` branch (test-first)

**Files:** `rust/sim/src/weapon.rs`.

Source: `weapon.cpp:117-124`. Extend the 4a `blow_up` (which freed the wobject and
skipped everything): after the free + the (still-skipped) `create_on_exp`/splinter/
sound, add `if w.dirt_effect >= 0 { draw_dirt_effect(level, large_sprites, textures,
w.dirt_effect, Ftoi(kX)-7, Ftoi(kY)-7, rand) }`. **CorrectShadow omitted** (O4).

- [ ] **Step 1 (test) — greenball explode writes terrain:** a wobject at a known
      `pos` over a Background-above-Dirt boundary, `blow_up` with the greenball weapon
      ⇒ (a) the pool slot is freed (4a behaviour preserved); (b) `material_id` in the
      `(Ftoi(x)-7, Ftoi(y)-7)` 16×16 window changes over the Background cells per the
      texture; (c) `rand` advanced by exactly the `DrawDirtEffect` `rand(2)` (no other
      draw — `create_on_exp=-1`, `splinter_amount=0`). Assert the `-7,-7` offset and
      `Ftoi` truncation.
- [ ] **Step 2 (test) — fan still inert:** `blow_up` with the fan weapon
      (`dirt_effect=-1`) writes **no** `material_id` and draws **no** RNG (the 4a path
      is unchanged) — a regression guard.
- [ ] **Step 3 (impl):** add the branch; pass `large_sprites`/`textures` through the
      `blow_up` signature (and from the driver). Keep the `create_on_exp`/splinter/
      sound branches guarded with `debug_assert!`/TODO referencing 4c so a greenball-
      unlike config fails loudly.
- [ ] **Verify:** `cargo test -p sim` green; `cargo test -p oracle-tests sim_slice4a`
      still green (fan path unaffected).

---

### Task 3: driver wiring — thread assets into the wobjects loop (test-first)

**Files:** `rust/sim/src/state.rs`.

`process_frame` already runs the wobjects loop and calls `blow_up` on Explode (4a).
4b only threads `large_sprites`/`textures` from `SimState` into that call.

- [ ] **Step 1 (test) — fire→fly→explode→terrain integration:** a grounded/visible
      worm with greenball in slot 0, real consts, aimed into the floor. A `Fire` tick:
      `ammo`↓, `delay_left=4`, `rng` moved (3 draws), one wobject, wobject `pos`==spawn
      (did not move its birth tick). Flight ticks: wobject `pos` arcs under
      `gravity=700`. The **explode tick**: wobject gone, **`level` material_id changed**
      over the impact window, `rng` advanced by the `rand(2)`. Assert `cycles==0`
      throughout and the `level` hash differs before vs after the explode tick.
- [ ] **Step 2 (test) — pristine-when-no-explode:** under a no-Fire input the `level`
      stays constant (regression: the wiring didn't introduce a spurious write).
- [ ] **Step 3 (impl):** destructure `large_sprites`/`textures` from `SimState`
      alongside `wobjects`/`rand`/`weapons`/`cossin`; pass them into `blow_up` in the
      Explode arm of the wobjects loop.
- [ ] **Verify:** `cargo test -p sim` green; `cargo test -p oracle-tests sim_slice2
      sim_slice3 sim_slice4a` still green.

---

### Task 4: C++ dumper — `settings->shadow=false` (the O4 omission)

**Files:** `src/tools/oracle_dump/sim_physics_dump.cpp`.

- [ ] **Step 1 (impl):** after the `Settings` is constructed (the
      `make_shared<Settings>()` line), add `settings->shadow = false;`. Add a one-line
      comment: omits `CorrectShadow` for the dirt-effect slices; inert to 1–4a because
      `MakeShadow` is only reached via `GenerateFromSettings`, which the dumper never
      calls. **No other change** — the object loops, `weapon` directive, and
      ProcessFrame subset are already present from 4a; `BlowUpObject`'s `dirt_effect`
      branch is real game code reached automatically when a greenball explodes.
- [ ] **Step 2 (verify no regression):** re-run `gen_sim_slice2_golden.sh`,
      `gen_sim_slice3_golden.sh`, `gen_sim_slice4a_golden.sh`; `git diff` on
      `sim_slice2.txt`/`sim_slice3.txt`/`sim_slice4a.txt` must be **empty** (the
      `shadow=false` flip is inert for them). If not, the flip perturbed a prior proof —
      stop and investigate (it should be impossible given the `MakeShadow`/`load`
      analysis; a diff means an unaudited `settings->shadow` reader).
- [ ] **Verify:** dumper builds under `-DOPENLIERO_BUILD_ORACLE_DUMP=ON`; slice-2/3/4a
      goldens unchanged.

---

### Task 5: scenario file + `gen_sim_slice4b_golden.sh` + committed golden

**Files:** `rust/oracle-tests/golden/sim_slice4b_scenario.txt`,
`rust/oracle-tests/gen_sim_slice4b_golden.sh`,
`rust/oracle-tests/golden/sim_slice4b.txt`.

- [ ] **Step 1:** Create `sim_slice4b_scenario.txt`: `seed 42`, `level
      Levels/physics_fall_test.lev`, `ticks ≈ 90`, `weapon 0 greenball`, two worms
      (worm 1 invisible/far so it is never hit). `input` lines: worm 0 aims toward the
      floor and sets `Fire` so the ball arcs into the dirt **surface** and explodes
      (sky above ⇒ Background cells in the impact window); optionally a **second** shot
      so `level` changes twice. Worm 1 a Fire-free/divergent pattern.
      **Constraints (comment them):** health 100; never Left(4)+Right(8) together; no
      shot within `detect_distance=3` of a *visible* worm (greenball `worm_collide=true`);
      non-firing worm invisible; **impact must straddle the surface** so Background cells
      exist in the 16×16 window (or `level` never moves).
- [ ] **Step 2:** Create `gen_sim_slice4b_golden.sh` (copy of
      `gen_sim_slice4a_golden.sh`): `set -euo pipefail`, `PRESET=${PRESET:-macos-arm64}`,
      configure `-DOPENLIERO_BUILD_ORACLE_DUMP=ON`, build `oracle_dump_sim_physics`, run
      from ROOT with the slice-4b scenario + output. Mark LOCAL/MANUAL. `chmod +x`.
- [ ] **Step 3:** Run it; commit `sim_slice4b.txt`. Inspect: the `level` column is
      **constant until the explode tick, then changes** (the headline — terrain went
      live); `rng` is `00000000` until the first fire tick, then moves (3 draws at fire,
      +1 at explode); the `wobjects` column is non-empty during flight; `worm0`/`worm1`/
      master change across phases. If `level` **never** moves, the impact landed in
      solid dirt (no Background) or in open sky (never explodes) — fix the aim.
- [ ] **Verify:** `sim_slice4b.txt` has `ticks+1` lines, 11 columns; `level` takes ≥2
      distinct values; `rng` moves; `wobjects` non-empty for ≥1 tick.

---

### Task 6: Rust differential test `sim_slice4b_golden` (test-first against golden)

**Files:** `rust/oracle-tests/tests/sim_slice4b_golden.rs`.

- [ ] **Step 1:** Mirror `sim_slice4a_golden.rs` setup: parse the scenario; load the
      `.lev`, `TcConfig` (materials + `PhysicsConsts` + `ControlConsts`), the `Objects`
      weapon table, **the large-sprite bank, and the textures table**; resolve
      `weap_order`; build worm inits with the `weapon 0 greenball` override
      (`WeaponInit { ty: Some(greenball_id), ammo }`, `current_weapon=0`); build
      `SimState::new(... weapons, cossin, large_sprites, textures ...)`. `parse_golden`
      keeps all columns incl. `state_hash`.
- [ ] **Step 2:** Assert tick-0 (master + 9 components) against the fresh state. For
      `k` in `1..=ticks`: `process_frame([unpack(scn.input(k-1,0)), unpack(scn.input(
      k-1,1))])` (**input keyed `k-1`**) and assert master + all 9 components against
      golden line `k`. Assert **components first** (rng → level → worm0 → worm1 →
      pools → wobjects) then master, so a divergence localises before the master fires.
- [ ] **Step 3 (coverage guard):** across the run assert the **`level` component column
      changes ≥1 time** (ideally ≥2 distinct post-explode values for two shots),
      `wobjects` is non-empty for ≥1 tick, and `rng` + some worm's weapon `ammo` each
      take ≥2 distinct values — so the golden actually exercises Fire **and** the
      terrain write, not just flight.
- [ ] **Verify:** `cargo test -p oracle-tests sim_slice4b` green.

---

### Task 7: wire-up review + done-check

- [ ] **Step 1:** `cargo test --workspace` green; `sim` has no Bevy / no float / no
      deps beyond `sim-core` + `assets`.
- [ ] **Step 2:** Re-read `weapon.cpp:117-124`, `gfx/blit.cpp:534-622`,
      `gfx/blit.hpp:39-47`, `material.hpp:5-27` against `blit.rs` + `blow_up`: the top
      `rand(r_frame)`, the `CLIP_IMAGE` to `height-1`, both `n_draw_back` branches, the
      `((my&15)<<4)+(mx&15)` wrap, the `Ftoi(x)-7,Ftoi(y)-7` offset, and **writes
      `material_id` only** must match exactly (note in the PR).
- [ ] **Step 3:** Confirm in `sim_slice4b.txt`: `level` constant then moves at the
      explode tick(s); `rng` moves at fire (3) and explode (+1); scenario never sets L+R
      or hits a visible worm; impact straddles the surface (Background cells present).
- [ ] **Step 4:** Confirm the **only** C++ change is the oracle-gated dumper line
      (`settings->shadow=false`) and that slice-2/3/4a goldens are byte-identical ⇒
      `test_determinism`/`test_rollback_*` unaffected (note in PR; no need to run).
- [ ] **Step 5:** Update the Step-2 overview *Slice ordering* + the Slice-4 overview
      (mark 4b done, level-hash live; the `CorrectShadow`/O4 decision recorded as
      omit-via-shadow-false) (docs only). Don't commit unrelated changes.
- [ ] **Definition of done:** every checkbox in the 4b design's *Definition of done*
      is satisfied.

## Notes for the implementer

- **The level-hash is the whole new thing.** Build the master golden test early and
  lean on the `level` + `rng` component columns: `level` must be flat until the
  explode tick, then jump; `rng` must move by 3 at fire and +1 at explode. If `level`
  never moves, the shot missed the surface (open sky never explodes) or hit solid dirt
  (no Background cells) — fix the aim, not the port.
- **`n_draw_back=false` = additive.** Greenball *creates* dirt in Background cells —
  per the `Texture` struct comment (`blit.hpp:40-42`), `false` is "creating dirt", not
  carving. The milestone (level-hash live) holds regardless; just make sure the impact
  window has Background cells. Port the `n_draw_back=true` carving half too (unit-
  tested) so the one function is complete for 4c/4d.
- **`rand(r_frame)` is drawn first, before the blit.** `r_frame=2` for greenball ⇒ a
  real `rand(2)`; place it at the very top of `draw_dirt_effect`. Misplacing it
  desyncs every later `rand.last`. (No openliero texture has `r_frame==0`, so the
  `rand(0)` edge case is a forward-looking note, not a 4b concern.)
- **Write `material_id` only.** No `materials` cache, no `display_valid`, no dirty
  list — those are non-hashed. The branch reads `material_flags[material_id[idx]]`; the
  write sets `material_id[idx]`; the next read re-derives the `Material`.
- **CLIP_IMAGE clips to `height-1`, not `height`** (`blit.cpp:545`). Port the
  start/extent math exactly; an off-by-one on the bottom row diverges the fold.
- **CorrectShadow stays out** (O4). The dumper's `settings->shadow=false` is one line;
  re-diff the 1–4a goldens to prove it is inert (the gate). If they diff, an unaudited
  `settings->shadow` reader exists — stop and find it.
- **Truncating shifts.** `Ftoi(x)-7` = `(x>>16)-7` (arithmetic shift). Same discipline
  as 4a.
- **Don't touch sim-critical C++.** The only C++ edit is the oracle-gated dumper line.
