# Step 3 · Slice 3a — render-crate foundation: detailed design

Status: **draft for review** · 2026-07-10
Part of: `2026-07-10-liero-rs-step3-rendering-overview.md` (cited as **overview**)
Sources: `2026-07-10-liero-rs-step3-cpp-render-pipeline-map.md` (**render-map §N**),
`2026-07-10-liero-rs-step3-bevy-api-research.md` (**bevy-research §N**)
Companion output feeds: `superpowers:writing-plans` (this is the spec, not the plan).

This is the executable design for the first Step 3 slice. It fixes exact scope
(which C++ functions port, with `file:line`), the Rust module layout of the new
`render` crate, the C++ dumper change (opt-in so existing goldens stay byte-identical),
the golden format + first scenario, the test list, edge cases, and done-when. No code
beyond short illustrative signatures/format examples.

---

## Goal of 3a

Produce the **first pixel-exact terrain frame** and stand up the machinery every later
render slice reuses: the `Bitmap`, the per-frame palette build, `DrawLevel`, the
two-viewport 320×200 player layout with centering, the FNV-1a frame hash, and the C++
frame-dumper + golden. Shadows, sprites, HUD, and minimap are **out of 3a** (3b/3e).

3a proves: (1) the ARGB `Bitmap` + `pal32` model, (2) the per-frame palette build
including `RotateFrom` color-cycling driven by `cycles>>3` (this is why palette cycling
is *in* 3a — it is the per-frame palette build, not a later add-on), (3) `DrawLevel`
Classic path, (4) the frame-hash oracle mechanics and the opt-in dumper, (5) isolation
(sim `HashGameState` unchanged).

---

## Exact scope — C++ functions ported in 3a

Ported **verbatim in math**, idiomatic in API (overview locked-decision 3). All
`file:line` per render-map.

| C++ source | What | render-map |
|---|---|---|
| `bitmap.hpp:12-24` | `Bitmap` fields: `w,h,pitch` (**pitch in pixels**), `u32* pixels`, borrowed `pal32`, `clip_rect`, `mode`, `cycles` | §2 |
| `bitmap.hpp:50-54` | `SetPixel` — `clip_rect.Inside` gate, write `pal32[index]` | §2, §4 |
| `blit.cpp:39` | `Fill(bmp, index)` — repaint whole clip through the LUT | §1 step 5, §5 |
| `blit.cpp:20-37` | `FillRect` — clip-clamped (needed for the `Fill`/clip clamp math) | §2, §5 |
| `renderer.cpp:23-30` | `UpdatePal32` — pack `pal32[i] = 0xFF000000 \| r<<16 \| g<<8 \| b` | §2, §4 |
| `game.cpp:171-183` | Per-frame palette build order: reset `Origpal` → `RotateFrom` cycle → `LightUp` flash → `UpdatePal32`, **before any blit** | §1 steps 1–4, §4 |
| `palette.cpp:50-57` | `RotateFrom(source, from, to, cycles>>3)` — rotate a sub-range | §4 |
| `palette.cpp:42-48` | `LightUp(amount)` `(v*(32-a)+a*255)>>5` — **ported but inert in 3a** (screen_flash=0) | §4 |
| `blit.cpp:194` | `DrawLevel(bmp, level, x, y)` — per pixel `AppearanceAt` | §5 |
| `level.hpp:59-64` | `AppearanceAt` **Classic arm** = `pal32[material_id[idx]]` | §4 |
| `viewport.cpp:22-57` | `Viewport::Process` — center/scroll/clamp; shake-`rand()` branch **gated on `shake>0`** (inert in 3a) | §2, §6 |
| `viewport.cpp:196-210` | Viewport world-block **terrain portion only**: set `clip_rect = rect`, `bmp.cycles = game.cycles`, `DrawLevel` at `kOffs = rect.Ul() - (x,y)` | §2, §3 step 7–8 |
| `framehash_main.cpp:26-50` | `HashFrame` — FNV-1a, R,G,B order, `FadeChannel` | §7 |

**Explicitly NOT in 3a** (drawn by C++ `Viewport::Draw` but deferred; these are *not*
hash-inert — the 3a golden is a deliberate terrain-only render, and both the Rust
renderer and the dumper omit them identically):
- HUD bars/text/banners (`viewport.cpp:78-189`) → 3e
- shadow pass (`viewport.cpp:274-398`) → 3b
- sprite pass (`viewport.cpp:400-591`) → 3b
- minimap (`viewport.cpp:593-635`) → 3e

`ShadowQuery`, `BlitImage*`, fonts, bars are **not** ported in 3a.

---

## Rust module structure — new `render` crate

`rust/render/` (edition 2021, deps `sim-core`, `assets`, `sim`; **no Bevy**). Add to
the workspace `members` (`rust/Cargo.toml`). Proposed modules:

```
render/
  Cargo.toml
  src/
    lib.rs          // pub mod re-exports; crate docs (CPU renderer, ARGB, charter)
    bitmap.rs       // Bitmap { w, h, pitch, pixels: Vec<u32>, clip: Rect, cycles }
                    //   + Rect { inside(), ul() }, set_pixel(), fill(), fill_rect()
    palette.rs      // Pal32 LUT + build_palette(): reset→rotate_from→light_up→pack
                    //   ports palette.cpp RotateFrom/LightUp; UpdatePal32 pack
    level_draw.rs   // draw_level(&mut Bitmap, &LevelSim, &Pal32, x, y)  (Classic arm)
    viewport.rs     // Viewport { rect, x, y, shake, rand: Rand, ... }
                    //   process() centering/clamp (shake branch present, inert);
                    //   draw_world_terrain() = clip + draw_level at kOffs (3a subset)
    frame.rs        // Frame::draw(&SimState, &Common-ish, layout) -> &Bitmap
                    //   the Game::Draw analog: build palette, Fill(0), per-viewport
    hash.rs         // hash_frame(&Bitmap, fade) -> u64  (FNV-1a, R,G,B, FadeChannel)
    rng.rs (maybe)  // OR reuse sim_core::Rand for the viewport-local RNG
```

Notes:
- The **viewport-local `Rand`** is the default-seeded `sim_core::Rand` (`rand.hpp`
  semantics, `operator()(max) = (rand64*max)>>32`). In 3a it is constructed and stored
  on `Viewport` but never advanced (no shake, no laser). Reuse `sim_core::Rand`
  directly — no new RNG type.
- `Pal32` is a `[u32; 256]` LUT; `Origpal` is the loaded TC palette
  (`assets::Palette`, entries already 6-bit-VGA-derived via `load_vga`, overview
  *Classic vs Modern*). `build_palette` must **not** re-quantize; it packs the
  RotateFrom/LightUp-adjusted entries straight into `pal32` (`renderer.cpp:23-30`).
- `level_draw::draw_level` reads `sim::LevelSim` (`state.rs:614`: `width, height,
  material_id`). The Classic arm needs only `material_id` + the LUT; `mode`/`cycles`
  params are kept in the signature for the future Modern arm but unused in 3a.
- Keep `mode: ColorMode` on `Bitmap`/`draw_level`/`ShadowQuery`-to-come as an enum
  arg defaulting to `Classic`, so Modern is a later `match` arm, not a refactor
  (overview *Classic vs Modern*).

### Illustrative signatures (not final)

```rust
pub struct Bitmap { pub w: i32, pub h: i32, pub pitch: i32,
                    pub pixels: Vec<u32>, pub clip: Rect, pub cycles: i32 }
impl Bitmap {
    fn set_pixel(&mut self, x: i32, y: i32, idx: u8, pal: &Pal32);  // clip-gated
    fn fill(&mut self, idx: u8, pal: &Pal32);
}
pub type Pal32 = [u32; 256];
pub fn build_palette(origpal: &Palette, color_anim: &[ColorAnim],
                     cycles: i32, screen_flash: i32) -> Pal32;
pub fn draw_level(dst: &mut Bitmap, lvl: &LevelSim, pal: &Pal32, x: i32, y: i32);
pub fn hash_frame(bmp: &Bitmap, fade: i32) -> u64;   // FNV-1a, R,G,B, FadeChannel
```

---

## The palette build (the heart of 3a)

Reproduce `game.cpp:171-183` exactly (render-map §1, §4):

1. **Reset**: working palette = `Origpal` (the TC-loaded `assets::Palette`).
2. **Color-cycle**: for each `ColorAnim { from, to }` in `common.color_anim`
   (`assets::tc::ColorAnim`, up to `NUM_COLOR_ANIM = 4`), apply
   `RotateFrom(origpal, from, to, (cycles as u32) >> 3)` — `palette.cpp:50-57`:
   `count = to-from+1; dist %= count; entries[from+i] = source[from + (i+count-dist)%count]`.
3. **Screen flash**: `if screen_flash > 0 { LightUp(screen_flash) }` — inert in 3a
   (`screen_flash == 0` for terrain-only scenarios). Ported now so 3b only wires the
   input, not the math.
4. **Pack**: `pal32[i] = 0xFF00_0000 | (r<<16) | (g<<8) | b` (`renderer.cpp:23-30`).

`cycles>>3` means the rotation advances one step every 8 ticks — so the frame hash
changes at ticks 8, 16, 24… **only if** the terrain window contains a cycled index
(see *First scenario* and overview open question 1). This is the concrete proof that
the palette is rebuilt *per frame*.

---

## The 3a draw path (`frame::draw`, the reduced `Game::Draw`)

Reproduce the terrain-only subset of `game.cpp:170-198` + `viewport.cpp:196-210`:

1. `pal32 = build_palette(...)` (above) — **before any blit**.
2. `bmp.fill(0, &pal32)` — repaint the whole 320×200 surface with index 0 through the
   fresh LUT (`game.cpp:189`).
3. For each of the two viewports (player layout, below):
   a. `viewport.process(sim)` — center on its worm, clamp (`viewport.cpp:22-57`).
   b. `bmp.clip = viewport.rect`; `bmp.cycles = sim.cycles`.
   c. `draw_level(&mut bmp, &sim.level, &pal32, kOffs.x, kOffs.y)` where
      `kOffs = rect.ul() - (viewport.x, viewport.y)` (`viewport.cpp:198`, §2).
   d. restore `bmp.clip` to full surface.

Result: whole frame is `pal32[0]`, except each 158×158 viewport rect shows its worm's
terrain window. Fully deterministic and directly comparable to the C++ dumper running
the same reduced draw.

### Player layout (must match the dumper, render-map §7)

- Surface 320×200.
- Two `Viewport`s: `Rect(0,0,158,158)` (worm 0) and `Rect(160,0,318,158)` (worm 1)
  (`framehash_main.cpp:92-95`). Worm `stats_x`: 0 / 218 (`:87-88`) — irrelevant to 3a
  terrain but set for parity.
- The gap column 158–159 and rows 158–199 stay `pal32[0]` (HUD area, unfilled in 3a).

---

## C++ dumper change — opt-in, existing goldens byte-identical

**Extend `src/tools/oracle_dump/sim_physics_dump.cpp`** (the Step 2 time-series driver;
its scenario grammar and 2-worm setup are the single source of truth). Follow the
Step 2 opt-in-token discipline exactly (every dumper feature that could move an
existing golden is gated so untouched scenarios regenerate byte-for-byte — the same
way `settings->shadow=false`, `max_bonuses`, `game_mode`, and the `weapon` directive
were added without perturbing earlier goldens).

### The opt-in directive

Add one scenario directive, parsed in `ParseScenario` (`sim_physics_dump.cpp:128-188`):

```
render <layout>        # layout = "player" (320x200 two-viewport) for 3a
```

- **Absent** (every existing scenario): the dumper never constructs a `Renderer`,
  never draws, and its primary output line is the **unchanged** 11-column sim record
  (`sim_physics_dump.cpp:326-328`). All `sim_slice*.txt` regenerate **byte-identical**
  → the **re-diff gate** passes untouched. This is the hard requirement.
- **Present**: after each tick's existing ProcessFrame-subset (and after the tick-0
  dump), the dumper additionally:
  1. builds a `Renderer` at 320×200 with the shipped TC palette as `Origpal`, and two
     `Viewport`s at the layout rects (once, at setup);
  2. runs the palette build + `Fill(0)` + per-viewport `Process` + `clip` + `DrawLevel`
     (the **terrain-only** subset — NOT full `Game::Draw`, so it matches the reduced
     Rust renderer; the HUD/shadow/sprite/minimap draws are omitted here and added in
     the 3b/3e dumper extensions);
  3. `HashFrame` (FNV-1a per §7) with `fade=0` on tick 0, `fade=33` after;
  4. writes `<tick> <frame_hash_hex16> <state_hash_hex8>` to a **sidecar** file
     (`out.frames.txt`, or a 4th argv), plus a final `total <n> <acc_hex16>` line.

The sim golden (`argv[2]`) format is **never** widened — the frame data goes to the
sidecar. So a render scenario's sim golden is still a normal sim golden and doubles as
the **isolation proof**: its `state_hash` column is identical to a non-render run
because `Game::Draw` advances only the viewport-local `Rand`, never `game.rand` or sim
state (render-map §6). The Rust test asserts the sidecar's `state_hash` column equals
the sim golden's — proving rendering did not perturb determinism.

### CMake

No new `add_executable` — `sim_physics_dump.cpp` is unchanged as a target
(`CMakeLists.txt:388-389`), only extended in-source under the gated directive. Add a
gen script `gen_render_slice3a_golden.sh` mirroring `gen_sim_slice4b_golden.sh`
(builds `oracle_dump_sim_physics` under `OPENLIERO_BUILD_ORACLE_DUMP=ON`, runs the 3a
scenario, writes both the sim golden and the sidecar frame golden). Local/manual, not
in the lightweight `rust.yml` CI (same as the Step 2 gen scripts).

### Palette source in the dumper

`Common::load` already loads the TC palette (Classic). The dumper's new `Renderer`
uses that as `Origpal` and `common->color_anim` for `RotateFrom` — no new asset
plumbing.

---

## Golden format + first scenario

### Sidecar frame golden `golden/render_slice3a.txt`

```
# render slice 3a — terrain-only player-layout frame hash.
# <tick> <frame_fnv1a_hex16> <state_hash_hex8>
0 0000000000000000 <s0>      # tick 0 hashed with fade=0 => all-black => the FNV of a
                             #   zeroed RGB buffer (constant); still a real line
1 <h1> <s1>
...
24 <h24> <s24>
total 25 <acc_hex16>
```

- Column 2 is the hard render gate; column 3 is the isolation column (must equal the
  sim golden `HashGameState` for the same scenario/tick).
- Tick 0's frame hash is the fade=0 (black) hash — a known constant for a given
  surface size; keep the line (matches C++ `framehash` frame-0 semantics, §7).

### First scenario `golden/render_slice3a_scenario.txt`

Reuse the Step 2 grammar + the Classic `Levels/physics_fall_test.lev`, 2 worms, plus
the new `render player` directive. Requirements:
- **≥ 24 ticks** so `cycles>>3` advances at least twice (steps at ticks 8, 16, 24),
  exercising `RotateFrom` across multiple distinct rotation distances.
- Worms placed so each viewport window **contains at least one `color_anim`-cycled
  palette index** — otherwise `RotateFrom` runs but the hash never moves and cycling
  is unproven (overview open question 1). If `physics_fall_test.lev`'s terrain in the
  centered windows uses no cycled index, either author a small cycling test level or
  widen the animated range; **3a must show the frame hash change on a `cycles>>3`
  boundary.** This is a done-when criterion, not optional.
- Static or near-static worms (minimal/zero input) so the *terrain* frame is stable
  and the only per-frame change is the palette rotation — isolating the RotateFrom
  proof. (Worm sprites aren't drawn in 3a, so worm motion wouldn't show anyway, but
  keeping them still keeps the viewport centering constant and the hash change
  attributable to palette cycling alone.)

---

## Test list

### Unit tests (in `render`)

- `bitmap`: `set_pixel` respects `clip_rect` (writes inside, drops outside); `fill`
  clamps to clip; pitch-vs-width indexing (`idx = y*pitch + x`) is correct for
  `pitch != w`.
- `palette`: `RotateFrom` matches the `palette.cpp:50-57` formula on a hand table
  (rotate by 0 = identity; `dist %= count`; wrap-around); `LightUp` matches
  `(v*(32-a)+a*255)>>5` and clamps to 255; `build_palette` order (reset→rotate→pack)
  and the `0xFF000000|r<<16|g<<8|b` pack.
- `hash`: `hash_frame` FNV-1a constants (offset `1469598103934665603`, prime
  `1099511628211`), R,G,B byte order, `FadeChannel` `(v*amount)>>5`, identity at
  `>=32`, and the all-black fade=0 case.
- `level_draw`: on a tiny synthetic `LevelSim`, terrain pixels resolve to
  `pal32[material_id[idx]]` at the right `kOffs`; clip gating; out-of-window pixels
  untouched.
- `viewport`: `process` centering math (`SetCenter`/`ScrollTo`/clamp) on a synthetic
  worm; the shake `rand()` branch is **not** taken when `shake==0` (assert the
  viewport `Rand` state is unchanged after `process`).

### Golden test (in `oracle-tests`)

- `render_slice3a_golden.rs`: load the scenario, drive `sim::SimState::process_frame`
  N ticks (reusing the Step 2 scenario runner in `oracle-tests/src/scenario.rs`),
  render each tick with the `render` crate, and assert the sidecar golden line-for-line
  **and** the `total` line. Assert column 3 (`state_hash`) equals the value the Step 2
  sim golden already records for the same scenario (isolation).
- **Re-diff gate**: a test/CI step confirming every existing `sim_slice*.txt`
  regenerates byte-identically from the (extended) dumper — i.e. the `render` directive
  being absent changes nothing (mirrors the Step 2 re-diff gate).

---

## Edge cases

- **`clip_rect`**: `set_pixel`/`fill` must gate on `clip.inside()`; the two-viewport
  draw sets clip per viewport and restores it. A pixel written with the wrong clip
  leaks terrain into the HUD gap and corrupts the hash.
- **`pitch` in pixels**: `Bitmap` addressing is `y*pitch + x`, pitch in *pixels* not
  bytes (`bitmap.hpp`, render-map §2). For 3a `pitch == w == 320`, but the port must
  use `pitch` (not `w`) so 3b/3e viewports and any sub-bitmap stay correct.
- **Fade frame 0**: tick 0 is hashed with `fade=0` → the whole surface reads as black
  regardless of pixels; later ticks `fade=33` (identity). Getting the frame-0 fade
  wrong is a silent whole-golden corruption (render-map §7).
- **6-bit VGA**: Classic `Origpal` entries are already 6-bit-derived
  (`assets::Palette::load_vga`, `(v&63)<<2`); `build_palette`/`UpdatePal32` must pack
  them straight — **no second quantization** (overview risk).
- **`RotateFrom` on `cycles>>3` boundaries**: the hash must be constant within an
  8-tick window and change on the boundary (ticks 8/16/24) — the concrete cycling
  proof; make the scenario exhibit it.
- **`kOffs` sign / clamp**: `Viewport::Process` clamps `x,y` to `[0, max]`
  (`max = level.dim - rect.dim`); a worm near a level edge yields a clamped window.
  Reproduce the clamp (`viewport.cpp:54-57`) or edge frames diverge.
- **Empty color_anim**: if the TC has fewer than the used `color_anim` slots, iterate
  only the present ones (`common.color_anim` length), never a fixed 4.

---

## Done-when

1. `render` crate compiles (edition 2021, no Bevy), added to the workspace.
2. Unit tests above pass.
3. `oracle_dump_sim_physics` extended with the opt-in `render player` directive;
   **every existing `sim_slice*.txt` regenerates byte-identically** (re-diff gate).
4. `golden/render_slice3a.txt` + `_scenario.txt` generated; the Rust
   `render_slice3a_golden.rs` test matches it **line-for-line incl. the `total`
   line**, over a scenario where the frame hash **visibly changes on a `cycles>>3`
   boundary** (RotateFrom proven).
5. The joint `state_hash` column matches the Step 2 sim golden for the same scenario
   (isolation proven — rendering did not touch `game.rand`/sim state).
6. Committed on branch `liero-rs-step-3` (accumulating PR; no push/PR from the worker
   per dispatch rules). Done-report produced.

Out of 3a (next: 3b shadow+sprite): `ShadowQuery`, all `BlitImage*`, fire cone,
`DrawNinjarope`, `DrawLine`, `DrawLaserSight` + the live viewport RNG, `LightUp` going
live via `screen_flash`, and shake going live.
