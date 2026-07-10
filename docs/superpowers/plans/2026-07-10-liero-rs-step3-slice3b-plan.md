# Step 3, Slice 3b — shadow + sprite pass (the pixel-exact world view): Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax. **Test-first**: write the failing test (or its assertion) before the
> implementation it pins, run it and SEE it fail, then make it pass.

**Goal:** Complete the **world view** so it is **pixel-exact vs C++** over motion, explosions,
objects, worms, ninjarope, fire cones, blood, shadows, and the laser-sight RNG trap. 3a shipped the
`render` crate foundation + the first terrain frame (PR #4, CI green). 3b ports the two-pass world
block of `Viewport::Draw` (`viewport.cpp:272-591`) — **all shadows, then all sprites** — plus the
blit primitives (`blit.cpp`), `ShadowQuery` (`shadow_query.hpp`), the worm-sprite index selection,
the fire cone, the ninjarope, the laser sight with its **viewport-local RNG**, the aim crosshair, and
blood; and makes `LightUp` (screen flash) and screen-shake go live. Proven by per-scenario FNV-1a
frame-hash goldens differential-tested tick-by-tick against the C++ dumper, over the reused Step-2
fire/object scenarios, with the joint `state_hash` column proving isolation. **HUD/banners/minimap/
font stay in 3e; spectator and Modern stay deferred.**

**Architecture:** Additive within the existing `render` crate (deps `sim-core`, `assets`, `sim`; **no
Bevy**). Four new modules — `shadow_query.rs`, `blit.rs`, `object_draw.rs`, `fire_cone.rs` — plus
extensions to `bitmap.rs`/`viewport.rs`/`frame.rs`. Draw math ported **verbatim** from C++ (each
formula carries a `file:line` comment); structure is idiomatic Rust. Rendering stays a **pure
consumer** of `SimState`: the only draw-time mutation is the *display-only* viewport `Rand` (laser
sparks; shake when `shake>0`). The C++ dumper's `render_and_hash` lambda is upgraded from
terrain-only to the full world block (minus HUD/minimap/name-labels), behind the same opt-in
discipline; `oracle-tests` gains the per-scenario `render_slice3b_*` frame-hash goldens beside the
sim goldens.

**Tech stack:** Rust (`render` new modules, `oracle-tests` tests + fixtures). Frame/sim goldens
generated LOCALLY/MANUALLY via the rebuilt C++ dumper (`OPENLIERO_BUILD_ORACLE_DUMP`, `PRESET`
default `macos-arm64`); CI (`cargo test --workspace`) runs the committed goldens. `data/TC/openliero`
real TC. The C++ dumper task (T6) may be built in a **separate worktree** because CMake is slow (the
Step-2 parallel-build lesson) — but it is planned as an ordinary task here.

## Global constraints

*(inherit every 3a constraint; the 3b-specific ones follow)*

- **`render` crate is edition 2021, NO Bevy dependency.** Deps stay exactly `sim-core`, `assets`,
  `sim` (all path). `cargo tree -p render` must show no `bevy*`. The new modules add no dependency.
- **Verbatim math, idiomatic API.** Every ported primitive reproduces the C++ arithmetic
  bit-for-bit; cite the C++ `file:line` in a comment at each ported formula. Primary sources of
  truth for 3b: `src/game/gfx/shadow_query.hpp:16-69` (`ShadowQuery`), `src/game/gfx/blit.cpp:239-729`
  (`BlitImage`/`BlitImageTrans`/`BlitImageR`/`BlitFireCone`/`BlitShadowImage`/`DO_LINE`/
  `DrawNinjarope`/`DrawLaserSight`/`DrawShadowLine`/`DrawLine`), `src/game/viewport.cpp:22-57` +
  `:190-591` (world block), `src/game/common.hpp:145-153` (`WormSprite`/`WormSpriteObj`/
  `FireConeSprite`), `src/game/common.cpp:17-21` (`fire_cone_offset`), `src/game/material.hpp:11`
  (`kSeeShadow = 1<<4`), `src/game/macros.hpp:3-22` (`CLIP_IMAGE`), `src/game/math/rect.hpp`
  (`Inside`/`Encloses`).
- **The two-pass ordering is LOAD-BEARING.** All shadows (`viewport.cpp:274-398`) composite before
  any sprite (`:400-590`); within each pass the object families run in a **fixed order** (bonuses →
  sobjects → wobjects → nobjects → worms(+ninjarope) → bobjects). This is the render analog of the
  sim object-loop order (overview *Two-pass draw order*). A single family reordered, or a shadow
  drawn after a sprite, moves the hash. Port the order exactly.
- **`frame.rs` ↔ `render_and_hash` symmetry.** The Rust `frame::draw` world block and the C++ dumper's
  inlined `render_and_hash` world block must be **1:1** — same palette build, same `Fill(0)`, same
  per-viewport `Process` → clip → **shadow pass** (if `draw_shadow`) → **sprite pass** → restore clip,
  same object-family order, same offsets. Whenever a family is added on one side, add it on the other
  in the same commit (the T6 lesson from 3a: **both parser arms / both draw sides move together**).
- **Viewport-RNG: seed, per-viewport instance, call order.** The RNG is `Viewport::rand` — a
  *separate*, **default-seeded** `sim_core::rng::Rand`, **one instance per viewport**, **never**
  `game.rand`. It lives for the whole render session (seeded once at viewport construction, state
  accumulates across ticks). `DrawLaserSight` draws `rand(5)` per stepped Bresenham pixel and, iff
  that is 0, a `rand(2)` (write `pal32[rand(2)+83]`). The shake branch (`viewport.cpp:49-52`) draws
  **two** `rand` off the same viewport RNG, gated `shake>0`. Viewport 0 is fully processed+drawn
  before viewport 1; each worm with a visible laser-sight advances **this viewport's** rand in
  `game.worms` order. Reproduce seed + instance + order or laser/shake frames diverge (overview
  *Risks*). `Rand::bound(max) = (rand64*max)>>32` already matches C++ (Step 2).
- **Draw-time shadow gate is DECOUPLED from `settings->shadow`.** The sim keeps `settings->shadow =
  false` (so `CorrectShadow` never mutates `material_id`; sim goldens stay byte-identical). The
  **draw** shadow pass is driven by a new opt-in scenario directive `render_shadow` → the C++
  `render_and_hash` temporarily flips `settings->shadow = true` for the **draw window only** (set
  before the passes, restore immediately after, *after* the tick's sim `Process` already ran, so the
  flip cannot reach `CorrectShadow`). The Rust side takes a matching `draw_shadow: bool`. This is the
  render analog of the 3a opt-in discipline (LOCKED — controller-ratified, spec O2).
- **Shake / screen-flash are INJECTED draw-only** (LOCKED — controller-ratified, spec O4 = inject).
  The reduced dumper's viewports are not wired into `ProcessViewports`, so `vp->shake`/`screen_flash`
  are never set by the sim. Two draw-only directives set them for a single draw: `render_shake <tick>
  <vp> <amount>` sets `viewports[vp].shake` for that draw; `render_flash <tick> <amount>` sets the
  composition `screen_flash` for that draw. Sim untouched (same discipline as `render_shadow`). **If
  the T5 implementation shows injection is unfaithful to C++ semantics** (e.g. shake is a `fixed`
  needing `Ftoi`, or the RNG advance order can't be reproduced from an injected value), the task
  **falls back to a documented deferral** of shake/flash to a later slice, keeping laser-sight as the
  sole viewport-RNG proof — but injection is the **main track**; deferral is the explicit fallback.
- **Steerable centering is DEFERRED.** All 3b scenarios keep `steerable_count == 0` (no steerable
  `shot_type` 2/3 weapon centered on its viewport); the `debug_assert!(steerable_count == 0)` in
  `viewport::process` stays. No `steerable_sum_x/y` fields are added (spec O5 = keep assert).
- **Sim-side additions are OHASHADE (non-hashed) render-only fields.** Prejudicated by
  `current_frame`/`animate` (Slice 5′, `state.rs:344-363`) and `bobject.color` (`state.rs:596`): read
  by draw, omitted from `hash.rs`, so **every `sim_slice*.txt` regenerates byte-identical**. 3b adds
  only what the chosen scenarios need: `hotspot_x`/`hotspot_y` on `WormState` (T0). `fire_cone_sprites`
  is built in the `render` crate from `state.large_sprites` (no sim change); `bonus_frames` is
  deferred (no chosen scenario spawns bonuses). Each addition ships with a **hash-neutrality test**.
- **Re-diff gates (hard, TWO of them).**
  1. **Sim re-diff:** every committed `sim_slice*.txt` regenerates byte-identical (the
     `render`/`render_shadow`/`render_shake`/`render_flash` directives absent in sim scenarios; the
     temporary draw-time flips never reach a sim mutation). `git diff --stat` MUST be empty.
  2. **Render re-diff (`render_slice3a.txt`):** the upgraded full-world dumper regenerates
     `render_slice3a.txt` **byte-identical** (3a's invisible worms + empty pools ⇒ the world draw
     paints exactly what the terrain-only draw did). This is a **done-when assertion, NOT a regen** —
     if a stray pixel moves it is a real pass-gating bug, caught immediately.
- **Isolation as a standing gate.** Each `render_slice3b_*` golden carries the Step-2 `state_hash`
  column; the Rust test asserts it equals both `hash_game_state` AND the pre-existing sim golden's
  master column. Rendering never perturbs the sim.
- **Classic-only.** Only the Classic `ShadowedArgb`/`AppearanceAt` arms are implemented; `ColorMode::
  Modern` stays an unimplemented `match` arm (`shadow_query.hpp:62-66` display-halve deferred).
- **No AI / "Generated with" taglines; no commit trailers** (no `Co-Authored-By`, no
  `Claude-Session`). **Commit on branch `liero-rs-step-3`** (the accumulating Step-3 PR). **Do NOT
  push and do NOT open a PR** — the controller owns push + PR. **Bash discipline:** one command per
  call; no `>>`/heredoc/`&&`/`;`/`$VAR`; no `cd`+`git`; create files with the editor.

## File structure

- `rust/sim/src/state.rs` — add `hotspot_x`/`hotspot_y` to `WormState` (render-only, non-hashed);
  compute them in `process_weapons`. **No change to `hash.rs`.**
- `rust/render/src/shadow_query.rs` — `MAT_SEE_SHADOW`, `ShadowQuery` (`pixel_at`/`shadowed_index`/
  `shadowed_argb`).
- `rust/render/src/blit.rs` — `blit_image`, `blit_image_trans`, `blit_image_r`, `blit_shadow_image`,
  `blit_fire_cone`; the `DO_LINE` iterator + `draw_line`, `draw_ninjarope`, `draw_laser_sight`,
  `draw_shadow_line`.
- `rust/render/src/fire_cone.rs` — `FIRE_CONE_OFFSET` static table (`common.cpp:17`) +
  `build_fire_cone_sprites(&SpriteSet) -> SpriteSet` (`common.cpp:539-551`).
- `rust/render/src/object_draw.rs` — `shadow_pass()` + `sprite_pass()` (the two-pass world block,
  per-family loops), the `shot_type` 2/3 `cur_frame` remap helper, `worm_sprite_index`/
  `fire_cone_sprite_index` selectors.
- `rust/render/src/bitmap.rs` — add `Bitmap::get_pixel(x,y) -> u32`, `Bitmap::put_argb(x,y,argb)`,
  `Rect::encloses(x,y) -> bool`.
- `rust/render/src/viewport.rs` — add `hotspot`/`make_sight_green` reads at the call site (no
  struct change; shake branch already coded).
- `rust/render/src/frame.rs` — `draw(...)` gains `draw_shadow: bool` + a `fire_cone_sprites` ref +
  `bonus_frames` slice (bundled in a `Scene` struct); calls `shadow_pass` (if `draw_shadow`) then
  `sprite_pass` per viewport.
- `rust/render/src/lib.rs` — add `pub mod shadow_query; pub mod blit; pub mod fire_cone; pub mod
  object_draw;`.
- `src/tools/oracle_dump/sim_physics_dump.cpp` — upgrade `render_and_hash` from terrain-only to the
  full world block; add `render_shadow`/`render_shake`/`render_flash` directives.
- `rust/oracle-tests/src/scenario.rs` — parse `render_shadow`/`render_shake`/`render_flash` (BOTH
  sides move together).
- `rust/oracle-tests/golden/render_slice3b_*_scenario.txt` — new scenarios (T7).
- `rust/oracle-tests/golden/render_slice3b_*.txt` + `_sim.txt` — generated goldens (T7).
- `rust/oracle-tests/gen_render_slice3b_*.sh` — gen scripts (T7).
- `rust/oracle-tests/tests/render_slice3b_*_golden.rs` — the Rust frame-hash golden tests (T8).
- `rust/oracle-tests/tests/render_slice3a_golden.rs` — call-site update for the widened `frame::draw`
  signature (T6; frame hashes stay byte-identical).
- `docs/superpowers/liero-rs-PROGRESS.md` + the overview's slice-3b line — updated in T9.

## Tasks

### T0 — sim-side render-only fields: `hotspot_x`/`hotspot_y` + hash-neutrality  [Opus]

**Files**
- Modify: `rust/sim/src/state.rs` (`WormState` struct + `from_init` + `process_weapons`)

**Interfaces**
- Consumes: existing `WormState`, `process_weapons` (`state.rs` / `control.rs`), `sim_core::fixed::ftoi`.
- Produces: `WormState { .., pub hotspot_x: i32, pub hotspot_y: i32 }` (render-only, **not** in
  `hash.rs`), defaulting to 0, updated in `process_weapons` mirroring `worm.cpp:1085,1094,1207-1208`.

**Why (teaching note):** C++ `Worm::hotspot_x/y` (`worm.hpp:225`) is the laser-sight/laser-beam origin
in *pixel* space. It is set two ways: on **Fire** (`worm.cpp:1085,1094`, `hotspot = Ftoi(pos)`) and by
the **laser-sight ProcessWeapons** arm (`worm.cpp:1207-1208`, `hotspot = Ftoi(temp)` where `temp` is
the sight endpoint). The renderer only ever *reads* it, so it belongs to the same "hash-silent
selector" family as `current_frame` — adding it must leave every prior golden byte-identical. This is
the whole point of the non-hashed render-field discipline: the sim carries render state without
perturbing the determinism series.

**Steps**

- [ ] **RED (hash-neutrality):** add a test near the existing `current_frame` neutrality tests
      (`state.rs` tests, ~line 4777+) asserting a driven worm's `hotspot_x/hotspot_y` are populated by
      `process_frame` **and** that `hash_game_state`/`hash_game_components` are unchanged by their
      presence. Model it on the `current_frame_formula` / `process_frame_sets_idle_current_frame`
      tests already in-file. Write it first, `cargo test -p sim hotspot` → FAIL (field absent):
      ```rust
      #[test]
      fn hotspot_is_render_only_and_hash_neutral() {
          // A worm that Fires a projectile sets hotspot = Ftoi(pos) (worm.cpp:1085).
          // The value is read only by the renderer; adding it must not move any hash.
          let mut state = /* Step-2 minimal SimState with worm 0 ready to fire */;
          let h_before = sim::hash::hash_game_state(&state);
          // Drive one Fire tick.
          let fire = ControlState::from_bits(1 << ControlState::FIRE);
          state.process_frame(&[fire, ControlState::default()]);
          let w = &state.worms[0];
          // hotspot tracks the worm's pixel position on Fire.
          assert_eq!(w.hotspot_x, sim_core::fixed::ftoi(w.pos.x));
          assert_eq!(w.hotspot_y, sim_core::fixed::ftoi(w.pos.y));
          // The hash series is unchanged by carrying the field (compare against a
          // control run WITHOUT reading hotspot — here we simply assert the field is
          // absent from the fold by checking a second identical run hashes the same).
          let h_after = sim::hash::hash_game_state(&state);
          let _ = (h_before, h_after); // series equality is proven by the T0 re-diff gate below.
      }
      ```
      (Adjust the `SimState` construction to the in-file test helper; the load-bearing assertions are
      the two `hotspot_*` equalities and that `hash.rs` is NOT edited.)
- [ ] **GREEN:** add the two fields to `WormState` (after `animate`, with the render-only doc
      comment citing `worm.hpp:225`); default 0 in `from_init`. Set them in `process_weapons` at the
      two C++ sites — the Fire path (`worm.cpp:1085`/`:1094`, `hotspot = Ftoi(pos)`) and the
      laser-sight arm (`worm.cpp:1197-1210`, `hotspot = Ftoi(temp)` where `temp` is the sight
      endpoint). Keep both writes on the **exact** C++ conditions so a non-laser, non-firing worm
      leaves them at their prior value (matching C++). Cite each line.
      ```rust
      // WormState (render-only, NOT hashed — worm.hpp:225).
      /// `Worm::hotspot_x` (`worm.hpp:225`): laser-sight / laser-beam origin in
      /// pixel space. Set on Fire (`worm.cpp:1085,1094`) and by the laser-sight
      /// arm of `ProcessWeapons` (`worm.cpp:1207`). Read only by the renderer
      /// (`viewport.cpp:509,516,520`). **Not hashed.**
      pub hotspot_x: i32,
      /// `Worm::hotspot_y` (`worm.hpp:225`), see `hotspot_x`. **Not hashed.**
      pub hotspot_y: i32,
      ```
- [ ] **Confirm `hash.rs` untouched.** `git diff --stat rust/sim/src/hash.rs` must be empty.
- [ ] **Sim re-diff gate (hard):** `cargo test --workspace` — every `sim_slice*_golden` and
      `test_determinism` GREEN (the new fields are invisible to the fold). This is the standing
      isolation proof for T0. If ANY sim golden test fails, STOP: a hashed field leaked.
- [ ] Reviewer (Opus): fields are render-only (absent from `hash.rs`); computed at the exact C++
      sites with `Ftoi`; `from_init` defaults 0; the hash-neutrality test is non-vacuous (drives a
      Fire and reads the populated value); every prior sim golden byte-identical.
- [ ] **Commit:**
      - `git add rust/sim/src/state.rs`
      - `git commit -m "sim(3b): render-only hotspot_x/hotspot_y on WormState (non-hashed)"`

---

### T1 — blit primitives group 1: `blit_image` / `blit_image_trans` / index-0 transparency  [Opus]

**Files**
- Create: `rust/render/src/blit.rs`
- Modify: `rust/render/src/lib.rs` (add `pub mod blit;`)

**Interfaces**
- Consumes: `render::bitmap::{Bitmap, Rect, Pal32}`; `assets::sprite::Sprite` (a single sprite view;
  confirm the accessor — `SpriteSet::index(i) -> Sprite` / `spr.mem`, `spr.width`, `spr.height`,
  `spr.pitch`; read `rust/assets/src/sprite.rs` at implementation for the exact field/method names).
- Produces:
  - `render::blit::blit_image(scr: &mut Bitmap, spr: &Sprite, x: i32, y: i32)`
  - `render::blit::blit_image_trans(scr: &mut Bitmap, spr: &Sprite, x: i32, y: i32, phase: i32)`
  - A private `clip_image(...)` helper reproducing `CLIP_IMAGE` (`macros.hpp:3-22`) — clamp
    `x/y/width/height` to `scr.clip` and slide the source start index. **This is the same clamp
    `draw_level` already inlines**; port it once here as the shared image-blit clip.

**Why (teaching note):** `CLIP_IMAGE` (`macros.hpp`) is the load-bearing off-screen-clip: a sprite
straddling the viewport edge draws only its inside rows/cols — start/count are *clamped*, the sprite
is never skipped wholesale. Every image blit shares it. Index 0 is the sprite's transparent hole
(`blit.cpp:252` `if (kC)`): a worm's transparent halo must not overwrite terrain. Get either wrong and
every sprite frame diverges.

**Steps**

- [ ] Add `pub mod blit;` to `lib.rs`.
- [ ] Read `rust/assets/src/sprite.rs` — note the exact `Sprite`/`SpriteSet` accessor (field names,
      whether a sprite exposes `mem: &[u8]`, `width`, `height`, `pitch`) and use them verbatim in the
      Interfaces below. (3a's `level_draw` never blit a sprite; this is the first sprite blit.)
- [ ] **RED:** create `rust/render/src/blit.rs` with the `clip_image` helper + `blit_image` /
      `blit_image_trans` and a hand-table test module. Write tests first (bodies `unimplemented!()`),
      run `cargo test -p render blit` → FAIL:
      ```rust
      #[test]
      fn blit_image_index0_is_transparent_and_uses_pal32() {
          // 2x2 sprite: [1, 0 / 0, 2]; index 0 must NOT write.
          // Screen prefilled with a sentinel so a transparent pixel is observable.
          // After blit at (0,0): (0,0)=pal[1], (1,0)=sentinel, (0,1)=sentinel, (1,1)=pal[2].
      }

      #[test]
      fn blit_image_clips_to_clip_rect() {
          // Sprite straddling the right/bottom clip edge draws only the inside cells;
          // outside cells stay the sentinel (CLIP_IMAGE clamp, not whole-sprite skip).
      }

      #[test]
      fn blit_image_trans_checkerboard_phase() {
          // (x ^ y ^ phase) & 1 gate on a full (all-index-1) 2x2 sprite:
          // phase 0 writes (1,0) and (0,1); phase 1 writes (0,0) and (1,1). And
          // index 0 is still transparent regardless of the checker bit.
      }
      ```
- [ ] **GREEN:** implement, porting `blit.cpp:239-287` verbatim. The clip helper mirrors
      `macros.hpp` (the same math `draw_level.rs:14-46` already uses):
      ```rust
      //! Sprite/line blit primitives. Ports `blit.cpp:239-729` verbatim in math,
      //! idiomatic in API. Every image blit shares `CLIP_IMAGE` (`macros.hpp:3-22`);
      //! index 0 is the transparent hole (`blit.cpp:252`).

      use crate::bitmap::{Bitmap, Pal32};
      use assets::sprite::Sprite;

      /// `macros.hpp:3-22` CLIP_IMAGE: clamp a `w x h` blit at `(x,y)` to `clip`,
      /// sliding the source start index. Returns `None` if fully clipped, else
      /// `(x, y, w, h, src_start)`.
      fn clip_image(clip: crate::bitmap::Rect, mut x: i32, mut y: i32, mut w: i32, mut h: i32,
                    pitch: i32) -> Option<(i32, i32, i32, i32, i32)> {
          let mut src = 0i32;
          let top = y - clip.y1;
          if top < 0 { src += -top * pitch; h += top; y = clip.y1; }
          let bottom = y + h - clip.y2;
          if bottom > 0 { h -= bottom; }
          let left = x - clip.x1;
          if left < 0 { src -= left; w += left; x = clip.x1; }
          let right = x + w - clip.x2;
          if right > 0 { w -= right; }
          if w <= 0 || h <= 0 { None } else { Some((x, y, w, h, src)) }
      }

      /// `blit.cpp:239-262`: index-0-transparent sprite blit, writes `pal32[c]` for c!=0.
      pub fn blit_image(scr: &mut Bitmap, spr: &Sprite, x: i32, y: i32) {
          let (x, y, w, h, src) = match clip_image(scr.clip, x, y, spr.width, spr.height, spr.pitch) {
              Some(v) => v, None => return,
          };
          for dy in 0..h {
              for dx in 0..w {
                  let c = spr.mem[(src + dy * spr.pitch + dx) as usize];
                  if c != 0 {
                      scr.pixels[((y + dy) * scr.pitch + (x + dx)) as usize] = scr.pal32[c as usize];
                  }
              }
          }
      }
      // blit_image_trans: identical loop with the extra `((dx_abs ^ dy_abs ^ phase) & 1)`
      // gate. NB the C++ checker uses the *destination* x/y in the sprite-local frame
      // (the macro's inner `x`,`y` counters run 0..width/0..height BEFORE clipping slides
      // them) — port `blit.cpp:264-287` exactly: the counter is the sprite-local index,
      // so use the CLIPPED loop's absolute (x+dx, y+dy) only if C++ does; re-read
      // blit.cpp:271-282 and match whichever coordinate the `(x^y^phase)` uses.
      ```
      > **CRITICAL for `blit_image_trans`:** in `blit.cpp:271-282` the `(x ^ y ^ phase)` uses the
      > **loop-local** `x`/`y` (the `for (int x=0; x<width; ++x)` counters that restart at 0 each
      > blit, *after* CLIP_IMAGE has already advanced `mem` and the screen pointer). So the checker
      > phase is relative to the **clipped sprite origin**, not the screen. Reproduce that: the
      > checker index is `(dx ^ dy ^ phase) & 1` where `dx/dy` are the post-clip loop counters (which
      > is exactly what the code above computes). Pin this in the test with a clipped, phased blit.
- [ ] **`pal32` on `Bitmap`.** These blits read `scr.pal32`, but 3a's `Bitmap` has **no `pal32`
      field** — 3a threaded the LUT as a `&Pal32` param. **Decision:** keep the LUT out of `Bitmap`;
      give every blit a `pal: &Pal32` parameter (matching 3a's `set_pixel(.., pal)` convention) rather
      than storing it on `Bitmap`. Update the signatures to `blit_image(scr, pal, spr, x, y)` etc.,
      and thread `pal` from `frame::draw`. (This keeps `Bitmap` a pure surface; the C++ `scr.pal32`
      member becomes an explicit arg — idiomatic-API, verbatim-math.) Reconcile all T1–T5 signatures
      to carry `pal: &Pal32`.
- [ ] Run `cargo test -p render blit` — PASS. `cargo build -p render`.
- [ ] Reviewer (Opus): `clip_image` matches `macros.hpp` (source slide on negative top/left, trim on
      bottom/right, early-out); index-0 transparency; `pitch`-addressed source; checkerboard uses the
      post-clip loop counters; `pal` is an explicit arg (no `pal32` on `Bitmap`).
- [ ] **Commit:**
      - `git add rust/render/src/blit.rs rust/render/src/lib.rs`
      - `git commit -m "render(3b): blit_image + blit_image_trans (index-0 transparency, CLIP_IMAGE)"`

---

### T2 — `ShadowQuery` (+4 / clamp / SeeShadow) + `blit_shadow_image` + `draw_shadow_line`  [Opus]

**Files**
- Create: `rust/render/src/shadow_query.rs`
- Modify: `rust/render/src/blit.rs` (add `blit_shadow_image`; `draw_shadow_line` lands in T3 with the
  other line drawers), `rust/render/src/bitmap.rs` (add `get_pixel`/`put_argb`/`Rect::encloses`),
  `rust/render/src/lib.rs` (add `pub mod shadow_query;`)

**Interfaces**
- Consumes: `sim::state::LevelSim { width, height, material_id: Vec<u8>, material_flags: [u8;256] }`;
  `render::bitmap::{Bitmap, Pal32, ColorMode, Rect}`; `assets::sprite::Sprite`.
- Produces:
  - `render::shadow_query::MAT_SEE_SHADOW: u8 = 1 << 4` (`material.hpp:11`).
  - `render::shadow_query::ShadowQuery<'a> { level: &'a LevelSim, pal32: &'a Pal32, world_offset_x: i32,
    world_offset_y: i32, mode: ColorMode, cycles: i32 }` with `pixel_at(sx,sy) -> i32`,
    `shadowed_index(sx,sy) -> i32`, `shadowed_argb(sx,sy) -> u32`.
  - `render::bitmap::Bitmap::get_pixel(x,y) -> u32`, `Bitmap::put_argb(x,y,argb)` (raw ARGB write,
    clip-gated), `render::bitmap::Rect::encloses(x,y) -> bool`.
  - `render::blit::blit_shadow_image(scr: &mut Bitmap, shadow: &ShadowQuery, spr: &Sprite, x,y,w,h)`.

**Why (teaching note):** `ShadowQuery` queries the **level**, not the screen (`shadow_query.hpp:8-15`).
`screen + world_offset = world` (note C++ builds it with `world_offset = -kOffs`). `ShadowedArgb`
returns the *darkened terrain* ARGB (`pal32[kP+4]`) for a `SeeShadow` cell, or 0. Because it reads the
level, overlapping shadows are **idempotent** (both write the same darkened value — no
double-darkening); that is a key consequence to test. The **`+4` clamp** (`shadow_query.hpp:45,67`):
materials 252-255 would push `kP+4 >= 256`, so the query returns the *unshifted* index, not a wrap.

**Steps**

- [ ] Add `pub mod shadow_query;` to `lib.rs`. Add the three `Bitmap`/`Rect` helpers first (they are
      trivial and unblock the tests):
      ```rust
      // bitmap.rs
      impl Bitmap {
          /// Raw ARGB read (no palette). Used by the shadow-pixel path. Caller
          /// guarantees (x,y) in bounds (the C++ `GetPixel` is unchecked).
          pub fn get_pixel(&self, x: i32, y: i32) -> u32 { self.pixels[(y * self.pitch + x) as usize] }
          /// Clip-gated raw ARGB write (the shadow path writes ARGB directly, not via pal).
          pub fn put_argb(&mut self, x: i32, y: i32, argb: u32) {
              if self.clip.inside(x, y) { self.pixels[(y * self.pitch + x) as usize] = argb; }
          }
      }
      impl Rect {
          /// `rect.hpp` `Encloses(IVec2)`: confirm inclusive-vs-half-open against
          /// `math/rect.hpp` at implementation and port EXACTLY. The nobject/bobject
          /// pixel gate uses this (not `Inside`).
          pub fn encloses(&self, x: i32, y: i32) -> bool { /* per rect.hpp — see note */ }
      }
      ```
      > **`Inside` vs `Encloses` (edge case, MUST verify):** the wobject shadow pixel uses
      > `clip.Inside` (`viewport.cpp:338`); nobject/bobject pixels use `clip.Encloses(IVec2)`
      > (`:358,391,493,587`). Read `src/game/math/rect.hpp` and confirm whether `Encloses` is
      > inclusive `[x1,x2]` or half-open `[x1,x2)` and whether it differs from `Inside`. Port each
      > predicate to the matching Rust method and use the C++ one at each call site verbatim. A
      > half-open/inclusive mismatch moves single-pixel object/blood frames.
- [ ] **RED (shadow_query):** create `rust/render/src/shadow_query.rs` with a hand-table test over a
      synthetic `LevelSim` + `material_flags`. Write first, run `cargo test -p render shadow_query`
      → FAIL:
      ```rust
      // Level 4x4; material_id picks a few cells. material_flags marks some indices
      // SeeShadow (1<<4). world_offset shifts screen->world.
      #[test]
      fn pixel_at_inside_and_outside() { /* returns material_id inside, -1 outside level */ }

      #[test]
      fn shadowed_index_plus4_and_clamp() {
          // SeeShadow cell with material m<252 -> m+4; not-SeeShadow -> -1;
          // material 254 (SeeShadow) -> 254 (unshifted, +4 would be 258>=256).
      }

      #[test]
      fn shadowed_argb_reads_pal_kp_plus4_or_zero() {
          // pal32 ramp so pal[m]=0xFF..m: SeeShadow cell -> pal32[m+4]; else 0.
      }
      ```
- [ ] **GREEN (shadow_query):** port `shadow_query.hpp:16-69` (Classic arm only; keep `mode`/`cycles`
      in the struct; the Modern display-halve arm `:62-66` is `unimplemented!`/dead):
      ```rust
      //! Queries shadow/material against the LEVEL (not the screen). Port of
      //! `shadow_query.hpp:16-69`. screen + world_offset = level coords. Classic arm
      //! only; the Modern display-halve arm (`:62-66`) is deferred.

      use crate::bitmap::{ColorMode, Pal32};
      use sim::state::LevelSim;

      /// `material.hpp:11`.
      pub const MAT_SEE_SHADOW: u8 = 1 << 4;

      pub struct ShadowQuery<'a> {
          pub level: &'a LevelSim,
          pub pal32: &'a Pal32,
          pub world_offset_x: i32,
          pub world_offset_y: i32,
          pub mode: ColorMode,
          pub cycles: i32,
      }

      impl<'a> ShadowQuery<'a> {
          /// `shadow_query.hpp:27-34`: material id under screen (sx,sy), or -1 outside.
          pub fn pixel_at(&self, sx: i32, sy: i32) -> i32 {
              let wx = sx + self.world_offset_x;
              let wy = sy + self.world_offset_y;
              if wx < 0 || wy < 0 || wx >= self.level.width || wy >= self.level.height { return -1; }
              self.level.material_id[(wx + wy * self.level.width) as usize] as i32
          }
          fn see_shadow(&self, m: i32) -> bool {
              m >= 0 && (self.level.material_flags[m as usize] & MAT_SEE_SHADOW) != 0
          }
          /// `shadow_query.hpp:38-46`: `+4` for SeeShadow, clamped at 256, else -1.
          pub fn shadowed_index(&self, sx: i32, sy: i32) -> i32 {
              let p = self.pixel_at(sx, sy);
              if !self.see_shadow(p) { return -1; }
              if p + 4 < 256 { p + 4 } else { p }
          }
          /// `shadow_query.hpp:51-68`: darkened-terrain ARGB (`pal32[kP+4]`) or 0.
          pub fn shadowed_argb(&self, sx: i32, sy: i32) -> u32 {
              let p = self.pixel_at(sx, sy);
              if !self.see_shadow(p) { return 0; }
              match self.mode {
                  ColorMode::Classic => self.pal32[if p + 4 < 256 { (p + 4) as usize } else { p as usize }],
                  ColorMode::Modern => unimplemented!("Modern display-halve shadow deferred"),
              }
          }
      }
      ```
      > **Verify the C++ `Inside` bound of `pixel_at`.** `shadow_query.hpp:30` calls `level.Inside(kWx,
      > kWy)` — read `level.hpp` for its exact predicate (half-open vs inclusive of `width`/`height`)
      > and match it; the code above assumes half-open `[0,width) x [0,height)`.
- [ ] **RED (blit_shadow_image):** add a test: a sprite with a **hole** (index 0) over a `SeeShadow`
      cell writes `shadowed_argb` only where sprite `c!=0` **and** the shadow is non-zero; the hole
      leaves the screen untouched; a non-SeeShadow cell under a solid sprite pixel writes nothing.
      Add the **idempotent double-shadow** test: two overlapping shadow blits write the *same*
      darkened value (reads the level, not the screen). FAIL first.
- [ ] **GREEN (blit_shadow_image):** port `blit.cpp:433-460` (shares `clip_image`):
      ```rust
      /// `blit.cpp:433-460`: where sprite c!=0, write shadow.shadowed_argb(x,y) if non-zero.
      pub fn blit_shadow_image(scr: &mut Bitmap, shadow: &ShadowQuery, spr: &Sprite,
                               x: i32, y: i32, w: i32, h: i32) {
          let pitch = w; // BlitShadowImage takes explicit w/h; source pitch = width.
          let (x, y, w, h, src) = match clip_image(scr.clip, x, y, w, h, pitch) { Some(v)=>v, None=>return };
          for dy in 0..h {
              for dx in 0..w {
                  let c = spr.mem[(src + dy * pitch + dx) as usize];
                  if c != 0 {
                      let sh = shadow.shadowed_argb(x + dx, y + dy);
                      if sh != 0 { scr.pixels[((y+dy)*scr.pitch + (x+dx)) as usize] = sh; }
                  }
              }
          }
      }
      ```
      > NB the C++ `BlitShadowImage`/`BlitImageR` pass an explicit `width,height` and use `pitch =
      > width` (the sprite is a raw `PalIdx*`, not a `Sprite` object). Confirm whether the Rust
      > `Sprite` you pass has `pitch == width` for these banks; if the `Sprite` API only exposes a
      > `mem` slice, index it as `mem[dy*width + dx]`. Match the C++ addressing (`pitch = width`).
- [ ] Run `cargo test -p render shadow_query` and `cargo test -p render blit` — PASS.
- [ ] Reviewer (Opus): `world_offset` sign (screen + offset = world); `+4` clamp returns unshifted
      (not wrap) at 252-255; `see_shadow` uses `material_flags & (1<<4)`; `pixel_at`/`level.Inside`
      bounds match `level.hpp`; `blit_shadow_image` idempotent (reads level); `Encloses`/`Inside`
      ported per `rect.hpp`; `get_pixel` unchecked, `put_argb` clip-gated.
- [ ] **Commit:**
      - `git add rust/render/src/shadow_query.rs rust/render/src/blit.rs rust/render/src/bitmap.rs rust/render/src/lib.rs`
      - `git commit -m "render(3b): ShadowQuery (+4/clamp/SeeShadow) + BlitShadowImage + bitmap helpers"`

---

### T3 — line drawers: `DO_LINE` Bresenham + ninjarope / laser-sight (RNG!) / shadow-line / line + `blit_image_r` / `blit_fire_cone`  [Opus]

**Files**
- Modify: `rust/render/src/blit.rs`

**Interfaces**
- Consumes: `render::bitmap::{Bitmap, Pal32, Rect}`; `render::shadow_query::ShadowQuery`;
  `sim_core::rng::Rand` (`bound(max: u32) -> u32`, `draws() -> u64`); `assets::sprite::Sprite`;
  `assets::tc::Constants { NRColourBegin, NRColourEnd }` (thread the two colours as `i32` params).
- Produces:
  - `render::blit::draw_line(scr, pal, from_x, from_y, to_x, to_y, color: i32)`
  - `render::blit::draw_ninjarope(scr, pal, from_x, from_y, to_x, to_y, nr_begin: i32, nr_end: i32)`
  - `render::blit::draw_laser_sight(scr, pal, rand: &mut Rand, from_x, from_y, to_x, to_y)`
  - `render::blit::draw_shadow_line(scr, shadow: &ShadowQuery, from_x, from_y, to_x, to_y)`
  - `render::blit::blit_image_r(scr, pal, shadow: &ShadowQuery, spr, x, y, w, h)` (water range `[160,168)`)
  - `render::blit::blit_fire_cone(scr, pal, fc: i32, spr, x, y)` (4 stages)

**Why (teaching note):** `DO_LINE` (`blit.cpp:644-677`) is a major-axis Bresenham with error init
`c = -(d>>1)`. **The loop steps BEFORE the body** — so the **start pixel is skipped** and the **end
pixel is the terminator** (`cx != to_x` / `cy != to_y` exits without drawing it). Off-by-one here
diverges every line primitive. `DrawLaserSight` (`blit.cpp:693-703`) is the **RNG trap**: per stepped
pixel it draws `rand(5)` (always, 1 draw); **iff that == 0** it draws a second `rand(2)` and writes
`pal32[rand(2)+83]` at `(cx,cy)` if `clip.Inside`. So the per-pixel draw count is 1 or 2,
data-dependent — the Bresenham path length and the RNG sequence *jointly* determine the frame. This is
the whole proof that the viewport RNG is live and reproduced exactly.

**Steps**

- [ ] **RED (DO_LINE octants):** add a test that walks each of the 8 octants with a
      **recording closure** and asserts the exact `(cx,cy)` pixel sequence (start skipped, end is the
      terminator, major-axis pick, `c=-(d>>1)` error). Model the expected sequence by hand for a
      couple of short lines (e.g. `(0,0)->(3,1)` dx>dy, `(0,0)->(1,3)` dy>=dx, plus a negative-slope
      case). Write first → FAIL.
- [ ] **GREEN (DO_LINE):** port the macro as a shared iterator that yields `(cx, cy)` in the exact
      C++ order. A closure-based helper keeps the 5 drawers DRY without a macro:
      ```rust
      /// `blit.cpp:641-677` DO_LINE. Major-axis Bresenham, error c=-(d>>1); the loop
      /// STEPS BEFORE the body, so the start pixel is skipped and the end pixel is the
      /// terminator (never drawn). Calls `body(cx, cy)` for each stepped-onto pixel.
      fn do_line(from_x: i32, from_y: i32, to_x: i32, to_y: i32, mut body: impl FnMut(i32, i32)) {
          let sign = |v: i32| if v < 0 { -1 } else if v > 0 { 1 } else { 0 };
          let (mut cx, mut cy) = (from_x, from_y);
          let mut dx = to_x - from_x;
          let mut dy = to_y - from_y;
          let sx = sign(dx);
          let sy = sign(dy);
          dx = dx.abs();
          dy = dy.abs();
          if dx > dy {
              let mut c = -(dx >> 1);
              while cx != to_x {
                  c += dy; cx += sx;
                  if c > 0 { cy += sy; c -= dx; }
                  body(cx, cy);
              }
          } else {
              let mut c = -(dy >> 1);
              while cy != to_y {
                  c += dx; cy += sy;
                  if c > 0 { cx += sx; c -= dy; }
                  body(cx, cy);
              }
          }
      }
      ```
- [ ] **GREEN (draw_line / draw_ninjarope / draw_shadow_line):** port `blit.cpp:679-729`. All use
      `clip.inside(cx,cy)` (NB `DrawNinjarope`/`DrawLaserSight`/`DrawShadowLine`/`DrawLine` all use
      `Inside`, `blit.cpp:689,700,712,725`):
      ```rust
      /// `blit.cpp:719-729`.
      pub fn draw_line(scr: &mut Bitmap, pal: &Pal32, fx: i32, fy: i32, tx: i32, ty: i32, color: i32) {
          let clip = scr.clip; let pitch = scr.pitch;
          do_line(fx, fy, tx, ty, |cx, cy| {
              if clip.inside(cx, cy) { scr.pixels[(cy*pitch+cx) as usize] = pal[color as usize]; }
          });
      }
      /// `blit.cpp:679-691`: color cycles [NRColourBegin, NRColourEnd). The pre-increment
      /// `if (++color == end) color = begin;` runs BEFORE the plot — port exactly.
      pub fn draw_ninjarope(scr: &mut Bitmap, pal: &Pal32, fx:i32, fy:i32, tx:i32, ty:i32,
                            nr_begin: i32, nr_end: i32) {
          let clip = scr.clip; let pitch = scr.pitch;
          let mut color = nr_begin;
          do_line(fx, fy, tx, ty, |cx, cy| {
              color += 1; if color == nr_end { color = nr_begin; }
              if clip.inside(cx, cy) { scr.pixels[(cy*pitch+cx) as usize] = pal[color as usize]; }
          });
      }
      // draw_shadow_line: blit.cpp:705-717 — plot shadow.shadowed_argb(cx,cy) if non-zero.
      ```
- [ ] **RED (draw_laser_sight — the RNG pin):** with a **seeded/default** `Rand` oracle, assert the
      **exact draw count and sequence** over a fixed span, and the written index `rand(2)+83`.
      Non-vacuous: pick a span where the Bresenham path length is known, and pre-compute the expected
      `rand(5)`/`rand(2)` draw sequence from a reference `Rand` so the test pins BOTH the per-pixel
      draw pattern (1 draw, or 2 iff the first is 0) AND the RNG order. Write first → FAIL:
      ```rust
      #[test]
      fn draw_laser_sight_rng_order_and_write() {
          // A short span of known Bresenham length N. Drive a reference Rand to predict:
          //   per stepped pixel: r = rand(5); if r==0 { idx = rand(2)+83; plot idx }
          // Assert (a) the number of rand draws == N + (#zeros), (b) the exact pixels
          // and indices written, (c) rand.draws() advanced by that count.
      }
      ```
- [ ] **GREEN (draw_laser_sight):** port `blit.cpp:693-703` — 1 `rand(5)` per pixel, and only if 0 a
      `rand(2)` write of `pal32[rand(2)+83]`, gated `clip.Inside`:
      ```rust
      /// `blit.cpp:693-703`. RNG: per stepped pixel `rand(5)`, iff 0 a `rand(2)` write
      /// of `pal32[rand(2)+83]` at (cx,cy) if inside clip. Advances `rand` (viewport-local).
      pub fn draw_laser_sight(scr: &mut Bitmap, pal: &Pal32, rand: &mut sim_core::rng::Rand,
                              fx:i32, fy:i32, tx:i32, ty:i32) {
          let clip = scr.clip; let pitch = scr.pitch;
          do_line(fx, fy, tx, ty, |cx, cy| {
              if rand.bound(5) == 0 {
                  let idx = rand.bound(2) as i32 + 83;
                  if clip.inside(cx, cy) { scr.pixels[(cy*pitch+cx) as usize] = pal[idx as usize]; }
              }
          });
      }
      ```
      > **RNG-order subtlety:** the `rand(2)` is drawn **before** the `clip.Inside` test in C++
      > (`blit.cpp:700`: `ptr[..] = scr.pal32[rand(2)+83]` — the `rand(2)` evaluates regardless of
      > whether the write lands, because the index is computed inside the assignment that is only
      > reached after `rand(5)==0`). So: when `rand(5)==0`, `rand(2)` is **always** drawn, even for an
      > off-clip pixel. Match the code above (draw `rand(2)` unconditionally once `rand(5)==0`, THEN
      > clip-gate only the write). Pin an off-clip zero-pixel in the test to lock this.
- [ ] **GREEN (blit_image_r):** port `blit.cpp:344-373` — draws only where `shadow.pixel_at(x+dx,
      y+dy)` is in the half-open water range `[160,168)`. Add a hand-level test (cells in and out of
      range). Uses `clip_image`.
- [ ] **GREEN (blit_fire_cone):** port `blit.cpp:375-407` — the 4 stages (`fc` 0/1/2/default),
      threshold `c > {116,114,112}` (strictly `>`), index offset `c-{5,3,1}` / `c`. Add a 4-stage
      test. Uses `clip_image` with `width=height=16`.
- [ ] Run `cargo test -p render blit` — all PASS. `cargo build -p render`.
- [ ] Reviewer (Opus): `do_line` start-skipped/end-terminator/major-axis/`c=-(d>>1)` exact; all four
      line drawers use `Inside`; `draw_ninjarope` pre-increments color before plot; `draw_laser_sight`
      draws `rand(5)` always and `rand(2)` unconditionally-once-zero (before the clip test); water
      range half-open `[160,168)`; fire-cone thresholds strictly `>` and offsets subtract.
- [ ] **Commit:**
      - `git add rust/render/src/blit.rs`
      - `git commit -m "render(3b): DO_LINE + ninjarope/laser-sight(RNG)/shadow-line/line + BlitImageR/BlitFireCone"`

---

### T4 — fire-cone table + sprite-index selectors + shadow pass (`shadow_pass`)  [Opus]

**Files**
- Create: `rust/render/src/fire_cone.rs`, `rust/render/src/object_draw.rs`
- Modify: `rust/render/src/lib.rs` (add `pub mod fire_cone; pub mod object_draw;`)

**Interfaces**
- Consumes: `render::{bitmap::*, blit::*, shadow_query::ShadowQuery}`; `sim::state::{SimState,
  WormState, LevelSim, Bonus, WObject, SObject, NObject, BObject}` (pools via `.iter()`;
  `bobjects.iter()`); `assets::object::{Weapon, SObjectType, NObjectType}`; `assets::sprite::SpriteSet`;
  `sim_core::fixed::ftoi`.
- Produces:
  - `render::fire_cone::FIRE_CONE_OFFSET: [[[i32;2];7];2]` (`common.cpp:17-21`), and
    `render::fire_cone::build_fire_cone_sprites(large: &SpriteSet) -> SpriteSet` (`common.cpp:539-551`).
  - `render::object_draw::worm_sprite_index(current_frame: i32, direction: i32, index: i32) -> usize`
    (`common.hpp:150`: `f + dir*7*3 + index*2*7*3`).
  - `render::object_draw::fire_cone_sprite_index(angle_frame: i32, direction: i32) -> usize`
    (`common.hpp:153`: `f + dir*7`).
  - `render::object_draw::wobj_remap(cur_frame: i32, shot_type: i32) -> i32` (`viewport.cpp:307-326`).
  - `render::object_draw::shadow_pass(scr: &mut Bitmap, state: &SimState, shadow: &ShadowQuery,
    off_x: i32, off_y: i32, bonus_frames: &[i32])` — the six shadow families in C++ order.

**Why (teaching note):** Pass 1 is *all shadows* in a fixed family order (`viewport.cpp:274-398`).
Every offset is load-bearing and often asymmetric — sobjects shadow at `(x-3, y+3)` but their sprite
is at `(x, y)` (`:293-294`); the wobject `shot_type` 2/3 `cur_frame` remap runs identically in both
passes (`:307-326` = `:435-454`). Test the pass at unit level with a **synthetic scene** where one
shadow-casting object's shadow would fall on another object's cell, and assert the ordering effect.

**Steps**

- [ ] Add the modules to `lib.rs`. Create `fire_cone.rs` with the static table (verbatim from
      `common.cpp:17-21`) and `build_fire_cone_sprites` (port `common.cpp:539-551`, which mirrors
      `large_sprites` into a `2*7` bank with the L/R flip). **RED** first (a small test: the table
      values match; the bank has `2*7` sprites; a mirrored column equals the flipped source):
      ```rust
      /// `common.cpp:17-21`. [direction][angle_frame][x|y].
      pub const FIRE_CONE_OFFSET: [[[i32; 2]; 7]; 2] = [
          [[-3, 1], [-4, 0], [-4, -2], [-4, -4], [-3, -5], [-2, -6], [0, -6]],
          [[3, 1], [4, 0], [4, -2], [4, -4], [3, -5], [2, -6], [0, -6]],
      ];
      ```
      > Read `rust/assets/src/sprite.rs` for how `SpriteSet` is built/allocated so
      > `build_fire_cone_sprites` can construct the `16x16 * (2*7)` bank the same way
      > `sim::state::build_worm_sprites` (`state.rs:834`) does — reuse that pattern.
- [ ] **RED (selectors + remap):** in `object_draw.rs`, pin `worm_sprite_index`,
      `fire_cone_sprite_index`, `wobj_remap` on hand tables (the `shot_type` 2 and 3 branches with
      their clamps — `viewport.cpp:307-326`). Write first → FAIL.
- [ ] **GREEN (selectors + remap):** implement verbatim:
      ```rust
      /// `common.hpp:150`.
      pub fn worm_sprite_index(f: i32, dir: i32, index: i32) -> usize {
          (f + dir * 7 * 3 + index * 2 * 7 * 3) as usize
      }
      /// `common.hpp:153`.
      pub fn fire_cone_sprite_index(f: i32, dir: i32) -> usize { (f + dir * 7) as usize }
      /// `viewport.cpp:307-326` / `:435-454`: the shot_type 2/3 cur_frame remap.
      pub fn wobj_remap(mut cur_frame: i32, shot_type: i32) -> i32 {
          if shot_type == 2 {
              cur_frame += 4; cur_frame >>= 3;
              if cur_frame < 0 { cur_frame = 16; } else if cur_frame > 15 { cur_frame -= 16; }
          } else if shot_type == 3 {
              if cur_frame > 64 { cur_frame -= 1; }
              cur_frame -= 12; cur_frame >>= 3;
              if cur_frame < 0 { cur_frame = 0; } else if cur_frame > 12 { cur_frame = 12; }
          }
          cur_frame
      }
      ```
- [ ] **RED (shadow_pass):** a synthetic `SimState` (2 worms, hand pools) over a small `SeeShadow`
      level; assert the six families paint shadows at the right offsets and in the right order. Include
      the **empty-pool guard** (empty pools + invisible worms → no writes — the 3a-ripple guard,
      in-crate). Write first → FAIL.
- [ ] **GREEN (shadow_pass):** port `viewport.cpp:274-398` family-by-family, in order:
      **(1) bonuses** `:276-284` (flicker gate `timer > BonusFlickerTime || (cycles&3)==0`;
      `bonus_frames[frame]`; `BlitShadowImage(small[..], Ftoi(x)-5-? , ...)` per `:280-282`);
      **(2) sobjects** `:287-298` (`BlitShadowImage(large[cur_frame+start_frame], x-3+off, y+3+off, 16,16)`);
      **(3) wobjects** `:300-343` (`wobj_remap`, then **if `weapon.shadow`** `BlitShadowImage(small[..],
      posX-3+off, posY+3+off, 7,7)`; else if `cur_frame>0` a single `shadowed_argb` pixel at
      `(posX+off-3, posY+off+3)` gated `clip.Inside`);
      **(4) nobjects** `:345-366` (`if type.start_frame>0` BlitShadowImage; else if `cur_frame>1` a
      `shadowed_argb` pixel gated `clip.Encloses`);
      **(5) worms + ninjarope** `:368-385` (per **visible** worm in `game.worms` order: if
      `ninjarope.out` `draw_shadow_line(rope-3,+3 → worm+7-3,+4+3)` then `BlitShadowImage(large[84],
      ropeX-4, ropeY+2, 16,16)`; then `BlitShadowImage(worm_sprite(current_frame,direction,index),
      tempX-3, tempY+3, 16,16)`);
      **(6) bobjects** `:387-397` (per blood particle: `shadowed_argb` pixel at `(pos.x-3, pos.y+3)+off`
      gated `clip.Encloses`).
      Gate the whole pass on `draw_shadow` at the caller (T5); `shadow_pass` itself assumes it should
      draw. Cite each `viewport.cpp` line in a comment. Use `state.weapons[ty]`, `state.sobject_types`,
      `state.nobject_types` for the type flags; `bonus_frames` is passed in (empty `&[]` for 3b
      scenarios — the bonus family never executes its body since chosen scenarios have empty
      `bonuses`).
      > **`bonus_frames` deferral:** no chosen 3b scenario spawns bonuses, so the bonus loop body
      > (`bonus_frames[i.frame]`) is never reached. Thread `bonus_frames: &[i32]` = `&[]`; if a future
      > scenario adds bonuses, populate it then (render-only). Document in the deferral ledger (T9).
- [ ] Run `cargo test -p render fire_cone object_draw` — PASS.
- [ ] Reviewer (Opus): family order exact; offsets verbatim (`-3/+3` sobject, `-3-3`/`+3+3` wobject
      shadow, worm `-3/+3`); `Inside` vs `Encloses` per call site; `wobj_remap` clamps exact; worms
      iterate `game.worms` order and are `visible`-gated; empty pools → no writes.
- [ ] **Commit:**
      - `git add rust/render/src/fire_cone.rs rust/render/src/object_draw.rs rust/render/src/lib.rs`
      - `git commit -m "render(3b): fire-cone table/bank + sprite selectors + shadow pass (viewport.cpp:274-398)"`

---

### T5 — sprite pass (`sprite_pass`) + `frame::draw` world block + LightUp/shake live  [Opus]

**Files**
- Modify: `rust/render/src/object_draw.rs` (add `sprite_pass`), `rust/render/src/frame.rs` (widen
  `draw`), `rust/render/src/viewport.rs` (hotspot/make_sight_green reads at call site — no struct change)

**Interfaces**
- Consumes: everything from T1–T4; `render::viewport::Viewport` (`.rand`, `.shake`); `sim::state::{
  ControlState}` (bits `FIRE=4`, `CHANGE=5`); `assets::tc::Constants { LaserWeapon, NRColourBegin,
  NRColourEnd }`; `assets::sprite::SpriteSet` (fire-cone bank).
- Produces:
  - `render::object_draw::sprite_pass(scr, state, pal, vp: &mut Viewport, off_x, off_y, fire_cone_sprites:
    &SpriteSet, nr_begin, nr_end, laser_weapon, bonus_frames: &[i32])` — the six sprite families + worm
    laser/beam/rope/firecone/sprite + crosshair.
  - `render::frame::Scene<'a> { origpal: &'a Palette, color_anim: &'a [ColorAnim], fire_cone_sprites:
    &'a SpriteSet, bonus_frames: &'a [i32], nr_begin: i32, nr_end: i32, laser_weapon: i32,
    screen_flash: i32, draw_shadow: bool }` (a bundle so the wide arg list stays readable).
  - `render::frame::draw(bmp: &mut Bitmap, state: &SimState, viewports: &mut [Viewport], scene: &Scene)`
    — the widened world-block draw.

**Why (teaching note):** Pass 2 is *all sprites* on top of the fully-composited shadow layer
(`viewport.cpp:400-590`). The worm sub-loop is where the viewport RNG goes live: laser sight
(`:516`) and, iff `weapon_index == LaserWeapon-1 && Pressed(kFire)`, the laser beam (`:520`); then
ninjarope (`:529` + rope-head `large[84]` at `ropeX-1, ropeY-1`), fire cone (`:539`), and finally the
**worm sprite** (`:545`). The aim crosshair (`:566-583`) is gated on the *viewport's own* worm being
visible. `LightUp` (screen flash) already threads into `build_palette`; shake is fed via
`vp.shake` (injected at the call site).

**Steps**

- [ ] **`frame::draw` signature change — coordinate with the 3a test.** Widening `draw` breaks the
      `render_slice3a_golden.rs` call site (`draw(bmp, &state, &origpal, &color_anim, vps, 0)`). That
      is EXPECTED: T6 updates the 3a call to the new `Scene` form with `draw_shadow=false`, empty
      fire-cone bank, `screen_flash=0`, and the 3a frame hashes stay **byte-identical** (the render
      re-diff gate). Do the signature change here; fix the 3a call site in this task's build so the
      crate + oracle-tests compile, and re-run `render_slice3a_golden` to confirm byte-identical.
- [ ] **RED (sprite_pass):** extend the synthetic-scene test from T4 (or add a sibling) to exercise
      the sprite families + the worm sub-loop. Key assertions: the **two-pass ordering** (a
      shadow-casting object whose shadow falls on another object's sprite cell → the sprite survives,
      shadow drawn first); a worm sprite lands at `(tempX, tempY)` with the right
      `worm_sprite_index`; the crosshair is `worm.visible`-gated; blood `SetPixel(color)` at
      `Encloses` cells. Write first → FAIL.
- [ ] **GREEN (sprite_pass):** port `viewport.cpp:400-590` family-by-family, in order, **skipping the
      name-label `DrawTextSmall` calls** (`:411`, `:476`, `:580` → 3e) and AI debug (`:550`):
      **(7) bonuses** `:402-415` (flicker gate; `blit_image(small[bonus_frames[frame]], Ftoi(x)-3+off,
      Ftoi(y)-3+off)`; **no** name label);
      **(8) sobjects** `:418-425` (`blit_image_r(large[cur_frame+start_frame], x+off, y+off, 16,16)`);
      **(9) wobjects** `:428-481` (if `start_frame>-1`: `wobj_remap`, `blit_image(small[start_frame+
      cur_frame], kPosX+off, kPosY+off)` where `kPosX=Ftoi(pos.x)-3`; else if `cur_frame>0`:
      `set_pixel((PalIdx)cur_frame)`; **no** weapon-34 name label);
      **(10) nobjects** `:483-498` (if `type.start_frame>0`: `blit_image(small[start_frame+cur_frame],
      pos.x+off, pos.y+off)` where `pos=Ftoi(pos)-(3,3)`; else if `cur_frame>1`: `set_pixel(cur_frame)`
      gated `Encloses`);
      **(11) worms** `:500-552` (per worm in `game.worms` order; `if w.visible`): compute `tempX =
      Ftoi(pos.x)-7+off.x`, `tempY = Ftoi(pos.y)-5+off.y`, `angle_frame = angle_frame(aiming_angle,
      direction)` (reuse `sim::state::angle_frame`):
        a. if `w.weapons[current_weapon].available()`: `hotspotX = w.hotspot_x + off.x`, `hotspotY =
           w.hotspot_y + off.y`; if `weapon.laser_sight` → `draw_laser_sight(scr, pal, &mut vp.rand,
           hotspotX, hotspotY, tempX+7, tempY+4)` (`:516`); if `weapon_index == laser_weapon-1 &&
           w.control_states.get(FIRE)` → `draw_line(.., tempX+7, tempY+4, weapon.color_bullets)` (`:520`);
        b. if `w.ninjarope.out`: `draw_ninjarope(ropeX, ropeY, tempX+7, tempY+4, nr_begin, nr_end)`
           then `blit_image(large[84], ropeX-1, ropeY-1)` (`:529-531`);
        c. if `weapon.fire_cone > 0 && w.fire_cone > 0`: `blit_fire_cone(scr, pal, w.fire_cone/2,
           fire_cone_sprites[fire_cone_sprite_index(angle_frame, direction)], FIRE_CONE_OFFSET[dir]
           [angle_frame][0]+tempX, FIRE_CONE_OFFSET[dir][angle_frame][1]+tempY)` (`:539`);
        d. **worm sprite**: `blit_image(worm_sprites[worm_sprite_index(current_frame, direction,
           index)], tempX, tempY)` (`:545`);
      **(12) aim crosshair** `:566-583` gated on **the viewport's own worm** `worm.visible`:
      `temp = Ftoi(pos) - (1,2) + Ftoi(cossin_table[Ftoi(aiming_angle)] * 16) + off`;
      `blit_image(small[if make_sight_green {44} else {43}], temp.x, temp.y)`; **no** change-name label.
      (Reuse the `cossin_table` the sim already exposes — read `sim_core`/`sim::state` for the table
      and the `Fixed` mul used at `viewport.cpp:568`; port that expression exactly.)
      **(13) bobjects** `:585-590`: per blood, `set_pixel((PalIdx)color)` at `Ftoi(pos)+off` gated
      `Encloses`.
- [ ] **GREEN (frame::draw world block):** widen `draw` to the `Scene` bundle and add the world block
      after `Process` + clip: build palette (with `scene.screen_flash` → `LightUp` live when `>0`),
      `Fill(0)`, then per viewport: `process` → `clip = vp.rect` → construct `ShadowQuery{ level:
      &state.level, pal32: &pal, world_offset_x: -(ulx - vp.x), world_offset_y: -(uly - vp.y), mode:
      Classic, cycles: state.cycles }` → `draw_level(..)` → **if `scene.draw_shadow`** `shadow_pass(..)`
      → `sprite_pass(..)` → restore clip. **Mirror the C++ `render_and_hash` block 1:1** (T6). Shake
      goes live automatically: `process` already reads `vp.shake` (set by injection at the call site).
      ```rust
      // world_offset = -kOffs, kOffs = rect.ul() - (vp.x, vp.y)  (viewport.cpp:198,204-205)
      let off_x = ulx - vp.x;
      let off_y = uly - vp.y;
      let shadow = ShadowQuery { level: &state.level, pal32: &pal,
          world_offset_x: -off_x, world_offset_y: -off_y, mode: ColorMode::Classic, cycles: state.cycles };
      draw_level(bmp, &state.level, &pal, off_x, off_y, ColorMode::Classic);
      if scene.draw_shadow { shadow_pass(bmp, state, &shadow, off_x, off_y, scene.bonus_frames); }
      sprite_pass(bmp, state, &pal, vp, off_x, off_y, scene.fire_cone_sprites,
                  scene.nr_begin, scene.nr_end, scene.laser_weapon, scene.bonus_frames);
      ```
      > **Borrow-checker note:** `sprite_pass` needs `&mut vp` (for `vp.rand`) while reading
      > `bmp`/`state`; `shadow` borrows `&pal` and `&state.level`. Structure the per-viewport body so
      > the `ShadowQuery` (immutable borrows) is dropped before `sprite_pass` takes `&mut vp`, or scope
      > the shadow borrow to the `if scene.draw_shadow` block. Keep `pal` a local `Pal32` value (owned)
      > so it can be borrowed by both the query and the blits.
- [ ] Run `cargo test -p render` (whole crate) — PASS. `cargo build -p render`.
- [ ] **Faithfulness checkpoint for shake/flash injection.** Confirm `vp.shake` is the field
      `process` reads (`viewport.rs:98 ftoi(self.shake)`) and that feeding an injected integer
      reproduces C++ `Ftoi(shake)` (C++ `shake` is a `fixed`; if the injected value must be a fixed,
      inject `itof(amount)` — match the C++ type). If injection cannot faithfully reproduce the C++
      shake RNG advance (e.g. the value's fixed-point scale is ambiguous), **fall back to the
      documented deferral** (Global Constraints): ship 3b with laser-sight as the sole viewport-RNG
      proof and record shake/flash as deferred in T9. Decide here, with evidence, before authoring the
      shake scenario in T7.
- [ ] Reviewer (Opus): sprite family order exact; name-labels/AI-debug omitted (with a comment
      pointing to 3e); worm sub-loop order (laser → beam → rope → firecone → sprite); crosshair gated
      on the *viewport's own* worm; `FIRE`/`CHANGE` bits correct; `world_offset = -kOffs`; two-pass
      order (shadow before sprite); `LightUp` live via `screen_flash>0`; the borrow structure drops
      the shadow query before `&mut vp`; the 3a golden re-runs byte-identical.
- [ ] **Commit:**
      - `git add rust/render/src/object_draw.rs rust/render/src/frame.rs rust/render/src/viewport.rs`
      - `git commit -m "render(3b): sprite pass (viewport.cpp:400-590) + world-block frame::draw + LightUp/shake live"`

---

### T6 — C++ dumper: full-world `render_and_hash` + `render_shadow`/`render_shake`/`render_flash` + BOTH re-diff gates + Rust parser arms  [Opus]

> **Note:** the CMake build is slow; this task may be done in a separate worktree/checkout to build
> `oracle_dump_sim_physics` (the Step-2 parallel-build lesson). Plan it as an ordinary task.

**Files**
- Modify: `src/tools/oracle_dump/sim_physics_dump.cpp`, `rust/oracle-tests/src/scenario.rs`,
  `rust/oracle-tests/tests/render_slice3a_golden.rs` (call-site update for the widened `frame::draw`)

**Interfaces**
- Produces: the dumper's `render_and_hash` renders the **full world block** (minus HUD/minimap/
  name-labels), inlined (NOT `Viewport::Draw`), mirroring Rust `frame::draw`. New opt-in directives:
  `render_shadow` (flip `settings->shadow=true` for the draw window only); `render_shake <tick> <vp>
  <amount>` and `render_flash <tick> <amount>` (draw-only injection). The Rust `scenario.rs` parses
  the three new directives (accepted-and-applied by the frame-hash tests; **both sides move together**).

**Why (teaching note — the 3a T6 lesson):** the shared scenario file is parsed by BOTH the C++ dumper
and the Rust test. A directive added on one side but not the other silently desyncs the corpus. Add
each directive to `ParseScenario` (C++) AND `Scenario::parse` (Rust) in the same commit, with a
parser test on the Rust side.

**Steps**

- [ ] **RED (re-diff baselines):** before editing, run two existing gen scripts (e.g.
      `gen_sim_slice4c_golden.sh`, `gen_sim_slice5b_golden.sh`) and the 3a gen
      (`gen_render_slice3a_golden.sh`); confirm `git diff --stat rust/oracle-tests/golden/` is empty
      (baseline: the harness reproduces every committed golden bit-exact before the change).
- [ ] **C++ dumper — upgrade `render_and_hash` to the full world block.** Replace the terrain-only
      body (`sim_physics_dump.cpp:401-409`, currently `Fill` + per-vp `Process` + `DrawLevel`) with the
      inlined `viewport.cpp:196-590` world block **minus** HUD/minimap/name-labels/holdazone/banners.
      Add the render includes (`gfx/blit.hpp`, `gfx/shadow_query.hpp` are likely already needed).
      Structure it to mirror the Rust `frame::draw`: per viewport — `Process` → `clip_rect = rect` →
      build `ShadowQuery` (`world_offset = -kOffs`) → `DrawLevel` → **if `settings->shadow`** the
      shadow pass (`viewport.cpp:274-398`) → the sprite pass (`:400-590`, dropping the `DrawTextSmall`
      and `ai->DrawDebug` lines) → restore clip. Draw the laser/beam off `vp->rand` (the viewport's own
      RNG), exactly as `viewport.cpp:516,520`. Cite the viewport lines inline.
- [ ] **C++ dumper — `render_shadow` directive.** Add `bool render_shadow = false;` to `Scenario`;
      parse `else if (key == "render_shadow") { s.render_shadow = true; }`. In `render_and_hash`, wrap
      the draw with the temporary flip:
      ```cpp
      bool const saved_shadow = game.settings->shadow;
      if (scn.render_shadow) game.settings->shadow = true;  // draw window only (spec O2)
      // ... build palette, Fill, per-viewport world block ...
      game.settings->shadow = saved_shadow;                  // restored before next Process
      ```
      Because `render_and_hash` runs AFTER the tick's sim `Process`, the flip never reaches
      `CorrectShadow`. (If `settings->shadow` is not a mutable field on the reduced `game`, use the
      smallest mutable handle that gates `viewport.cpp:274`; read the dumper's `Settings` wiring.)
- [ ] **C++ dumper — `render_shake` / `render_flash` injection.** Parse `render_shake <tick> <vp>
      <amount>` and `render_flash <tick> <amount>` into per-tick maps. In `render_and_hash(tick)`,
      before the passes: set `viewports[vp]->shake = <amount>` (as the C++ `fixed` type — use `itof`
      if `shake` is fixed) for the matching tick, and pass `<amount>` as the `LightUp` `screen_flash`
      into the palette build (`if (screen_flash > 0) renderer->pal.LightUp(screen_flash)` before
      `UpdatePal32`, mirroring `game.cpp` + the Rust `build_palette`). Reset the injected values after
      the draw so the next tick is clean. **These are the C++ analog of the Rust call-site injection —
      keep them symmetric.**
- [ ] **Rust parser arms (BOTH sides).** In `rust/oracle-tests/src/scenario.rs`, add fields
      `render_shadow: bool`, `render_shake: Vec<(u32,usize,i32)>` (tick, vp, amount),
      `render_flash: Vec<(u32,i32)>` (tick, amount), and parse the three directives (`render_shadow`
      takes 0 args; `render_shake` 3; `render_flash` 2). Add accessor(s) `shadow()`,
      `shake_at(tick)`, `flash_at(tick)`. Add parser unit tests (arity, defaults absent → off/empty,
      round-trip) mirroring the existing `render`/`game_mode` tests. `cargo test -p oracle-tests
      scenario` GREEN.
- [ ] **Update the 3a golden test call site.** In `render_slice3a_golden.rs`, change the
      `render::frame::draw(bmp, &state, &origpal, &color_anim, vps, 0)` call to the new `Scene` form:
      `draw_shadow=false`, `fire_cone_sprites = &empty_bank` (build a `2*7` bank from
      `state.large_sprites` via `render::fire_cone::build_fire_cone_sprites`, or an empty `SpriteSet`
      since 3a has no fire cone), `bonus_frames=&[]`, `screen_flash=0`, `nr_begin/nr_end/laser_weapon`
      from `tc.constants`. Re-run `cargo test -p oracle-tests --test render_slice3a_golden` — the frame
      hashes must be **byte-identical** to the committed `render_slice3a.txt` (the render re-diff gate
      at the Rust level).
- [ ] **Re-diff gate 1 (sim, hard):** rebuild the dumper (`OPENLIERO_BUILD_ORACLE_DUMP=ON`), run
      EVERY committed `gen_sim_slice*.sh` to regenerate all `sim_slice*.txt`. Assert `git diff --stat
      rust/oracle-tests/golden/` is EMPTY for the sim goldens. If any moves, STOP: the full-world
      render path or a directive is leaking into the ungated sim path.
- [ ] **Re-diff gate 2 (render_slice3a, hard):** re-run `gen_render_slice3a_golden.sh`. Assert
      `render_slice3a.txt` (and `render_slice3a_sim.txt`) regenerate **byte-identical** — the
      upgraded full-world dumper paints exactly what the terrain-only dumper did (3a's invisible worms
      + empty pools). If a byte moves, STOP: a real pass-gating bug (paste the diff, fix the gate).
- [ ] Update the dumper's file-header comment: the full world block, the three new directives, and the
      sim-neutrality argument (the shadow flip window and the shake/flash injection contain no sim
      mutation).
- [ ] Reviewer (Opus): the render block is the FULL world block minus HUD/minimap/name-labels/AI;
      inlined (not `Viewport::Draw`); `render_shadow` flips only in the draw window; injection resets
      after the draw; the sim `out` write is untouched; **both re-diff gates empty** (paste both `git
      diff --stat`); the Rust parser arms match the C++ directives; the 3a golden re-runs
      byte-identical.
- [ ] **Commit:**
      - `git add src/tools/oracle_dump/sim_physics_dump.cpp rust/oracle-tests/src/scenario.rs rust/oracle-tests/tests/render_slice3a_golden.rs`
      - `git commit -m "oracle_dump(3b): full-world render_and_hash + render_shadow/shake/flash + parser arms (re-diff green)"`

---

### T7 — scenario corpus + gen scripts + goldens (empirical: shadow pixels, laser RNG draws)  [Opus]

**Files**
- Create: `rust/oracle-tests/golden/render_slice3b_{fan,dart,blood,shadow,laser}_scenario.txt`
  (+ `_shake` if injection is feasible per T5), `rust/oracle-tests/gen_render_slice3b_*.sh`
- Generate (committed): `rust/oracle-tests/golden/render_slice3b_*.txt` + `_sim.txt`

**Interfaces**
- Consumes: the T6 dumper (full world block + directives).
- Produces: one sidecar frame golden per scenario, each concretely exercising its target family, with
  the empirical windows VERIFIED (shadow pixels actually present; laser hash actually moves per tick).

**Selection (per spec, one proof per family + the RNG trap):**

| new render scenario | reuse base | proves |
|---|---|---|
| `render_slice3b_fan` | `sim_slice4a_scenario` | wobject sprite pass (floor-shot fan) + worm sprites |
| `render_slice3b_dart` | `sim_slice4c_scenario` | sobject explosion (`BlitImageR`) + nobject debris (`BlitImage`/`SetPixel`) + worm sprites |
| `render_slice3b_blood` | `sim_slice5b_scenario` | bobject blood `SetPixel(color)` + worm-hit sprites |
| `render_slice3b_shadow` | 4a/4c base + `render_shadow` | the shadow pass + `ShadowedArgb`/`+4` over `SeeShadow` dirt |
| `render_slice3b_laser` (new) | new | `DrawLaserSight` + viewport RNG; laser beam `DrawLine` |
| `render_slice3b_shake` (new, IF T5 feasible) | new + injected `shake`/`flash` | shake RNG + `LightUp` |

**Steps**

- [ ] For each reused scenario, copy the Step-2 `_scenario.txt` inputs and add `render player`, plus
      make the worms **visible** (unlike 3a) so the worm/crosshair/laser blocks are reached (the worm
      physics goes live — that is fine; the C++ dumper and Rust renderer see the same sim, so any
      motion is reproduced identically). Create a `gen_render_slice3b_<name>.sh` per scenario mirroring
      `gen_render_slice3a_golden.sh` (4th-arg sidecar; pass the scenario seed explicitly as arg 3).
- [ ] **`render_slice3b_shadow` — empirical SeeShadow verification (done-when, spec O3):** verify
      whether `physics_fall_test.lev`'s dirt (material 12) carries `kSeeShadow` (`material.hpp:11`,
      `1<<4`) in the openliero TC. Grep the TC `tc.cfg` materials / read `data/TC/openliero/tc.cfg`.
      **If material 12 is SeeShadow**, place a visible worm/object touching dirt and confirm the
      generated golden's shadow-pass ticks actually paint shadow pixels (diff the frame hash with vs
      without `render_shadow` on the SAME scenario — they MUST differ; if identical, the shadow pass
      ran but painted nothing → unproven). **If material 12 is NOT SeeShadow**, author a tiny
      `SeeShadow` test level (analog of 3a's cycling-level fallback) whose dirt has the flag, place a
      sprite over it, and use that. Do NOT commit `render_slice3b_shadow` until the frame hash provably
      moves with `render_shadow` on.
- [ ] **`render_slice3b_laser` — empirical RNG verification:** a **visible** worm holding a
      `laser_sight` weapon (resolve by name via `weapon <slot> <name>`) produces sparks every tick. Add
      an `input` firing the laser (`Pressed(kFire)` on the `LaserWeapon` slot) for the beam. The bevis
      window: the frame hash **changes each tick even with a near-stationary worm**, because the
      viewport RNG advances — verify the generated golden's laser ticks are NOT constant (contrast 3a's
      constant static scene). If the chosen TC has no laser-sight weapon, document and pick the nearest
      RNG-advancing draw, or author a minimal weapon override.
- [ ] **`render_slice3b_shake` (conditional):** only author if T5's faithfulness checkpoint passed.
      Add `render_shake <tick> <vp> <amount>` and `render_flash <tick> <amount>` at a chosen tick;
      verify the golden's injected tick differs from its neighbours (shake moves the viewport;
      `LightUp` shifts the palette). If T5 deferred injection, SKIP this scenario and record the
      deferral (T9).
- [ ] Generate all goldens locally (`PRESET=macos-arm64 rust/oracle-tests/gen_render_slice3b_<name>.sh`).
      **Verify first WITHOUT regenerating priors** — the gen scripts only write the new
      `render_slice3b_*` files; no existing golden is touched (re-diff discipline).
- [ ] Reviewer (Opus): each scenario reaches its target family (visible worms; the right pool
      non-empty in the bevis window per the reused sim golden); the shadow scenario's hash provably
      moves with `render_shadow`; the laser scenario's hash moves per tick (RNG live); no prior golden
      regenerated (only additions); each `_sim.txt` is a normal 11-column sim golden.
- [ ] **Commit:**
      - `git add rust/oracle-tests/golden/render_slice3b_*.txt rust/oracle-tests/golden/render_slice3b_*_scenario.txt rust/oracle-tests/gen_render_slice3b_*.sh`
      - `git commit -m "render(3b): shadow/sprite scenario corpus + gen scripts + frame/sim goldens"`

---

### T8 — Rust frame-hash golden tests — MILESTONE (world view pixel-exact + isolation + RNG proof)  [Opus]

**Files**
- Create: `rust/oracle-tests/tests/render_slice3b_{fan,dart,blood,shadow,laser}_golden.rs` (+ `_shake`
  if shipped)

**Interfaces**
- Consumes: `render::{bitmap::Bitmap, viewport::Viewport, frame::{draw, Scene}, fire_cone::
  build_fire_cone_sprites, hash::{hash_frame, FNV_OFFSET, FNV_PRIME}}`; `oracle_tests::scenario::
  Scenario`; `sim::hash::hash_game_state`; the Step-2 `SimState` setup (copy from
  `render_slice3a_golden.rs`, which already carries it); `tc.constants` (nr colours, laser weapon).
- Produces: one passing per-tick differential test per scenario, matching the sidecar **line-for-line
  + `total`**, with the joint `state_hash` column == the Step-2 sim golden.

**Steps**

- [ ] Factor a shared harness. The 3a test's parser (`parse_frames`) + SimState setup is reusable;
      lift the common bits into a small helper module in `tests/` (or copy per test, matching the
      existing per-file style — the repo currently copies boilerplate per golden, so copying is
      acceptable). Each test: load scenario + `_sim.txt` + `_scenario.txt`, build `SimState` (Step-2
      setup), build `Scene` (origpal from `small.tga`; `color_anim` from tc; `fire_cone_sprites =
      build_fire_cone_sprites(&state.large_sprites)`; `nr_begin/nr_end/laser_weapon` from
      `tc.constants`; `bonus_frames=&[]`; `draw_shadow = scenario.shadow()`; per-tick `screen_flash =
      scenario.flash_at(tick)`), and drive tick-by-tick:
      ```rust
      for k in 0..=scenario.ticks {
          if k > 0 { state.process_frame(&inputs_for(k-1)); }
          // inject shake for this draw (render_shake), matching the C++ dumper.
          for (t, vp, amt) in scenario.shake_at(k) { viewports[vp].shake = /*itof?*/ amt; }
          let scene = Scene { /* .. */ screen_flash: scenario.flash_at(k), draw_shadow: scenario.shadow() };
          render::frame::draw(&mut bmp, &state, &mut viewports, &scene);
          let fh = render::hash::hash_frame(&bmp, if k == 0 { 0 } else { 33 });
          // reset injected shake so the next tick's Process is clean (mirror the dumper).
          for (t, vp, _amt) in scenario.shake_at(k) { viewports[vp].shake = 0; }
          // assert fh == golden[k].frame_hash; state_hash == golden col == sim_master[k]; accumulate total.
      }
      ```
      Assert the `total` accumulator, and the joint `state_hash` (Rust `hash_game_state` == sidecar
      column == `_sim.txt` master column).
- [ ] **Laser RNG non-vacuity assert.** In `render_slice3b_laser_golden.rs`, add an explicit
      `assert_ne!` that two laser-active ticks have **different** frame hashes (the RNG-advance proof),
      analogous to 3a's RotateFrom-boundary asserts — so a renderer that silently no-ops the laser
      can't pass.
- [ ] **Shadow non-vacuity assert.** In `render_slice3b_shadow_golden.rs`, assert the shadow-window
      tick's hash differs from the same tick rendered with `draw_shadow=false` (proves shadow pixels
      landed) — either as an in-test control render or by referencing the fan/dart base golden.
- [ ] Run each `cargo test -p oracle-tests --test render_slice3b_<name>_golden` — expect FAIL if any
      wire is wrong, then PASS. Then `cargo test --workspace` — all green (render unit + all 3a/3b
      goldens + every sim golden).
- [ ] **(Optional, cheap-only) ASCII/PNG viz.** John likes a visual. Rendering the world view as a
      PNG is 3d scope; **skip unless trivial**. If a one-liner `image`-crate dump of `bmp.pixels` is
      easy, drop a single example under `examples/` for one scenario tick; otherwise defer to 3d.
- [ ] Reviewer (Opus): expected values from the golden files, actual from the DRIVEN `SimState` +
      `render` crate; isolation column checked both ways; the laser/shadow non-vacuity asserts are
      real inequalities; the `total` matches; frame 0 uses `fade=0`. "Could this pass while the
      renderer is wrong?" — no: the frame-hash column is the hard gate and the RNG/shadow asserts
      prove the trap families are live.
- [ ] **MILESTONE.** World view pixel-exact vs C++ over objects/worms/blood/shadows; viewport RNG
      (laser) proven live; two-pass order proven load-bearing; isolation triple-proven.
- [ ] **Commit:**
      - `git add rust/oracle-tests/tests/render_slice3b_*_golden.rs`
      - `git commit -m "render(3b): Rust frame-hash goldens — world view pixel-exact + RNG + isolation"`

---

### T9 — PROGRESS + overview slice-3b line + deferral ledger + slice close  [Opus]

**Files**
- Modify: `docs/superpowers/liero-rs-PROGRESS.md`; the overview's slice-3b bullet
  (`docs/superpowers/specs/2026-07-10-liero-rs-step3-rendering-overview.md`)

**Steps**

- [ ] `cargo test --workspace` green (render unit + all `render_slice3b_*` + `render_slice3a` +
      every prior sim golden + `test_determinism`). Confirm `render` Bevy-free (`cargo tree -p render`
      shows only `sim-core`/`assets`/`sim`; `grep -rn "bevy" rust/render/` empty).
- [ ] Confirm the **re-diff ledger** (T6): both gates green — every `sim_slice*.txt` byte-identical
      AND `render_slice3a.txt` byte-identical (paste the two empty `git diff --stat`); the new
      `render_slice3b_*` files are the only additions.
- [ ] Update `docs/superpowers/liero-rs-PROGRESS.md` (use the real `currentDate` in any Status/date
      field): Step 3 slice 3b DONE — the full world view (shadow + sprite two-pass) is pixel-exact vs
      C++; blit primitives + `ShadowQuery` + line drawers + fire cone + laser-sight viewport RNG
      ported; `LightUp`/shake live (or shake deferred — record which); `hotspot_x/y` added render-only;
      the dumper renders the full world block behind the render directives with both re-diff gates
      clean; per-scenario `render_slice3b_*` goldens match C++ tick-for-tick with the laser hash moving
      per tick (RNG proof) and the joint `state_hash` proving isolation.
- [ ] Update the overview's 3b bullet to mark it landed (companion spec implemented). Update the
      overview *Deferrals* / open-questions as resolved (O2 flip ratified, O4 inject shipped-or-
      deferred, O5 steerable kept-asserted).
- [ ] **Deferral ledger (explicit).** Record carried deferrals: name labels / font (`DrawTextSmall`
      at `:411/:476/:580`) → 3e; HUD/bars/banners/holdazone/minimap → 3e; `BlitImageTrans`
      spawn-preview (unreached — `names_on_bonuses=false`, no `kChange`) → out unless forced; AI debug
      → permanently out; Modern `ShadowedArgb` display-halve arm → deferred; steerable centering
      (`steerable_sum_x/y`) → deferred (assert kept); `bonus_frames` → deferred (no bonus scenario);
      `render_slice3b_shake`/injection → shipped OR deferred (record). A future scenario that reaches
      any of these must lift the deferral with a matching golden.
- [ ] Reviewer (Opus): docs match reality; deferrals honestly listed; the re-diff ledger is real;
      open questions marked resolved.
- [ ] **Commit:**
      - `git add docs/superpowers/liero-rs-PROGRESS.md docs/superpowers/specs/2026-07-10-liero-rs-step3-rendering-overview.md`
      - `git commit -m "docs(3b): PROGRESS + overview slice-3b landed; deferral ledger"`

## Done-report (each task)

(a) what changed + why, (b) files touched, (c) tests/risks. Per-task commit, local, on branch
`liero-rs-step-3`. **Do not push, do not open a PR** — the controller owns push + PR. Surface in the
final report: **both re-diff-gate evidences** (T6: sim goldens + `render_slice3a.txt` byte-identical),
the **laser-RNG non-vacuity** result (per-tick hash movement, T8), the **shadow empirical**
verification (SeeShadow pixels provably landed, T7), and the **shake/flash injection decision**
(shipped vs deferred, T5) with its reasoning.
