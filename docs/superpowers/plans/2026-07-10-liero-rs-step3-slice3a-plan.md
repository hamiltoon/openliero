# Step 3, Slice 3a — render-crate foundation (Bitmap + palette build + DrawLevel + two-viewport frame hash): Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax. **Test-first**: write the failing test (or its assertion) before the
> implementation it pins, run it and SEE it fail, then make it pass.

**Goal:** Produce the first **pixel-exact terrain frame** and stand up the machinery every later
Step-3 slice reuses: a new Bevy-free `render` crate with an ARGB `Bitmap`, the per-frame palette
build (reset → `RotateFrom` color-cycle → `LightUp` flash → `pal32` pack), `DrawLevel` (Classic
arm), the two-viewport 320×200 player layout with `Viewport::process` centering, and the FNV-1a
frame hash — plus the C++ dumper's **opt-in** `render player` directive emitting a terrain-only
frame golden. Proven by a Rust golden test matching the C++ frame hash tick-for-tick over a
palette-cycling scenario, with the frame hash **visibly changing on a `cycles>>3` boundary**
(`RotateFrom` proven) and the joint `state_hash` column proving rendering did not perturb the sim
(isolation). Shadows, sprites, HUD, minimap are OUT of 3a (3b/3e).

**Architecture:** Add a fourth determinism-relevant crate `rust/render/` (deps `sim-core`, `assets`,
`sim`; **no Bevy**) that ports the C++ CPU renderer verbatim in *math*, idiomatic in *API*: indices
resolve to ARGB through a `pal32` LUT at each write, the LUT is finalized before any blit, and the
frame hash is the regression primitive. The C++ Step-2 dumper `sim_physics_dump.cpp` gains one
gated scenario directive so untouched scenarios regenerate byte-identically while render scenarios
emit a sidecar frame golden; `oracle-tests` gains `render` as a dev-dependency and hosts the
frame-hash golden test beside the sim goldens.

**Tech stack:** Rust (`render` new crate, `oracle-tests` test + fixtures). Frame/sim goldens
generated LOCALLY/MANUALLY via the rebuilt dumper (`OPENLIERO_BUILD_ORACLE_DUMP`, `PRESET` default
`macos-arm64`); CI (`cargo test --workspace`) runs the committed goldens. `data/TC/openliero` real
TC. The C++ dumper task (T4) may be built in a **separate worktree** because CMake is slow (the
Step-2 parallel-build lesson) — but it is planned as an ordinary task here.

## Global constraints

- **`render` crate is edition 2021, NO Bevy dependency.** Deps are exactly `sim-core`, `assets`,
  `sim` (all path deps). `render` is the new determinism-relevant crate; it must stay Bevy-free so
  the headless/hash path runs with no GPU/window (overview *Layer 1*, locked decision 1).
- **Verbatim math, idiomatic API.** Every ported primitive reproduces the C++ arithmetic
  bit-for-bit; the structure is idiomatic Rust (methods on `Bitmap`, slices, plain functions). Cite
  the C++ `file:line` in a comment at each ported formula (render-map §N). Sources of truth:
  `bitmap.hpp:12-54` (`Bitmap`/`SetPixel`), `blit.cpp:20-41` (`FillRect`/`Fill`),
  `blit.cpp:194-213` + `macros.hpp:3-22` (`DrawLevel` + `CLIP_IMAGE`), `level.hpp:59-64`
  (`AppearanceAt` Classic arm), `renderer.cpp:23-30` (`UpdatePal32` pack), `palette.cpp:42-57`
  (`LightUp`/`RotateFrom`), `game.cpp:171-183` (palette build order), `viewport.cpp:22-57` +
  `viewport.hpp:35-54` (`Process`/`SetCenter`/`ScrollTo`) + `viewport.cpp:196-210` (world block),
  `framehash_main.cpp:26-50` (`HashFrame`/`FadeChannel`).
- **`pitch` in PIXELS, not bytes.** `Bitmap` addressing is `idx = y*pitch + x`; the port uses
  `pitch` (not `w`) everywhere so 3b/3e sub-bitmaps stay correct. For 3a `pitch == w == 320`
  (`bitmap.hpp:15`, render-map §2).
- **Palette build order, before any blit.** `reset to Origpal → RotateFrom (each `color_anim`) →
  LightUp (if `screen_flash>0`, INERT in 3a) → pack pal32`, finalized before `fill`/`draw_level`
  (`game.cpp:171-183`, render-map §1). `RotateFrom` source is always the ORIGINAL `origpal`, never
  the partially-rotated working palette. `cycles>>3` is a **signed** shift then unsigned distance:
  `(cycles >> 3) as u32` (cycles is a non-negative frame counter).
- **FNV-1a frame hash constants (exact):** offset `1469598103934665603`, prime `1099511628211`
  (`framehash_main.cpp:26-27`). Iterate the ARGB buffer row-major (`y`,`x`); per pixel extract
  **R, G, B** in that order (`(c>>16)&0xff`, `(c>>8)&0xff`, `c&0xff`), drop alpha, apply
  `FadeChannel(v, fade) = if fade >= 32 { v } else { ((v as i32 * fade) >> 5) as u8 }`, hash each of
  the 3 bytes with `h = (h ^ b) * prime` (u64 wrapping). **Frame 0 is hashed with `fade = 0`**
  (whole surface reads black — a fixed constant for a given size), all later frames `fade = 33`
  (identity). The running total accumulator is `all = (all ^ frame_hash) * prime`, seeded with the
  FNV offset (`framehash_main.cpp:110`).
- **`pal32` pack is straight, no re-quantization.** Classic `origpal` entries are already 6-bit-VGA
  derived (`assets::Palette` via the sprite TGA loader, `(v&63)<<2`); `pack_pal32` does
  `0xFF000000 | (r<<16) | (g<<8) | b` with the entries AS-IS (`renderer.cpp:23-30`, overview risk
  *Classic 6-bit VGA*). No second `<<2` / `&63`.
- **Isolation is a hard invariant.** The `render` crate never mutates `SimState`; it reads
  `state.level`, `state.worms`, `state.cycles` only. The viewport-local `Rand` is a *separate*
  default-seeded `sim_core::Rand` stored on `Viewport`; in 3a it is constructed but **never
  advanced** (shake gated on `shake>0`, inert). The dumper's render path never touches
  `game.rand`/sim state, so the sim golden for a render scenario is provably identical to a
  non-render run (render-map §6).
- **Re-diff gate (hard).** The C++ `render` directive is **opt-in**: absent (every existing
  scenario) → the dumper constructs no `Renderer`, draws nothing, and its 11-column sim line is
  unchanged. T4 regenerates EVERY committed `sim_slice*.txt` via its gen script and asserts
  `git diff` is empty. This is the same discipline as the Step-2 opt-in tokens.
- **`render` scenarios are Classic-only.** Only the Classic `AppearanceAt` arm (`pal32[material_id]`)
  is implemented; `ColorMode::Modern` stays an unimplemented `match` arm so Modern is a later
  addition, not a refactor (overview *Classic vs Modern*, locked decision 5).
- **No AI / "Generated with" taglines; no commit trailers** (no `Co-Authored-By`, no
  `Claude-Session`). **Commit on branch `liero-rs-step-3`** (the accumulating Step-3 PR). **Do NOT
  push and do NOT open a PR** — the controller owns push + PR. **Bash discipline:** one command per
  call; no `>>`/heredoc/`&&`/`;`/`$VAR`; no `cd`+`git`; create files with the editor.

## File structure

- `rust/Cargo.toml` — add `"render"` to workspace `members`.
- `rust/render/Cargo.toml` — new crate manifest (edition 2021; deps `sim-core`, `assets`, `sim`).
- `rust/render/src/lib.rs` — crate docs + `pub mod` re-exports.
- `rust/render/src/bitmap.rs` — `Rect`, `Pal32`, `ColorMode`, `Bitmap` (`set_pixel`/`fill`/
  `fill_rect`).
- `rust/render/src/palette.rs` — `rotate_from`, `light_up`, `pack_pal32`, `build_palette`.
- `rust/render/src/hash.rs` — `FNV_OFFSET`/`FNV_PRIME`, `fade_channel`, `hash_frame`.
- `rust/render/src/level_draw.rs` — `draw_level` (Classic arm + `CLIP_IMAGE` clamp).
- `rust/render/src/viewport.rs` — `Viewport` (`new`/`player_layout`/`set_center`/`scroll_to`/
  `process`).
- `rust/render/src/frame.rs` — `draw` (reduced `Game::Draw`: build palette → fill(0) → per-viewport
  process/clip/draw_level → restore clip).
- `src/tools/oracle_dump/sim_physics_dump.cpp` — add the gated `render <layout>` directive + a
  sidecar frame golden (Renderer + two viewports + terrain-only draw + HashFrame).
- `rust/oracle-tests/Cargo.toml` — add `render` dev-dependency.
- `rust/oracle-tests/golden/render_slice3a_scenario.txt` — new scenario (≥24 ticks, static worms).
- `rust/oracle-tests/golden/render_slice3a_sim.txt` — the 11-column sim golden (isolation source).
- `rust/oracle-tests/golden/render_slice3a.txt` — the sidecar frame golden.
- `rust/oracle-tests/gen_render_slice3a_golden.sh` — gen script (rebuilt dumper).
- `rust/oracle-tests/tests/render_slice3a_golden.rs` — the Rust frame-hash golden test.
- `docs/superpowers/liero-rs-PROGRESS.md` + the overview's slice-3a line — updated in T7.

## Tasks

### T0 — `render` crate skeleton + `Bitmap`/`Rect`/`ColorMode` + primitives  [Opus]

**Files**
- Create: `rust/render/Cargo.toml`, `rust/render/src/lib.rs`, `rust/render/src/bitmap.rs`
- Modify: `rust/Cargo.toml` (workspace `members`)

**Interfaces**
- Produces:
  - `render::bitmap::Rect { pub x1: i32, pub y1: i32, pub x2: i32, pub y2: i32 }` — `Copy`; methods
    `fn new(x1: i32, y1: i32, x2: i32, y2: i32) -> Rect`, `fn width(&self) -> i32`,
    `fn height(&self) -> i32`, `fn ul(&self) -> (i32, i32)`, `fn inside(&self, x: i32, y: i32) -> bool`.
  - `render::bitmap::Pal32 = [u32; 256]` (type alias).
  - `render::bitmap::ColorMode { Classic, Modern }` — `Copy`, `Default` = `Classic`.
  - `render::bitmap::Bitmap { pub w: i32, pub h: i32, pub pitch: i32, pub pixels: Vec<u32>, pub clip: Rect, pub cycles: i32 }`
    with `fn new(w: i32, h: i32) -> Bitmap` (pitch = w, `pixels` = `w*h` zeros, `clip` = full,
    `cycles` = 0), `fn set_pixel(&mut self, x: i32, y: i32, idx: u8, pal: &Pal32)`,
    `fn fill(&mut self, idx: u8, pal: &Pal32)`,
    `fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, idx: u8, pal: &Pal32)`.

**Steps**

- [ ] Add the crate to the workspace. In `rust/Cargo.toml`, change
      `members = ["sim-core", "assets", "sim", "oracle-tests"]` to
      `members = ["sim-core", "assets", "sim", "render", "oracle-tests"]`.
- [ ] Create `rust/render/Cargo.toml`:
      ```toml
      [package]
      name = "render"
      version = "0.1.0"
      edition = "2021"

      [dependencies]
      sim-core = { path = "../sim-core" }
      assets = { path = "../assets" }
      sim = { path = "../sim" }
      ```
- [ ] Create `rust/render/src/lib.rs`:
      ```rust
      //! CPU bitmap renderer for Liero-rs (no Bevy, no floating point in the hashed
      //! path). Ports the C++ `renderer`/`blit`/`viewport`/`palette` math verbatim,
      //! idiomatic in API: drawing takes 8-bit palette indices and resolves them to
      //! ARGB through a `pal32` LUT at each write (`bitmap.hpp:50-54`); the LUT is
      //! finalized once per frame before any blit (`game.cpp:171-183`). The FNV-1a
      //! frame hash (`hash`) is the regression primitive differential-tested against
      //! C++. Slice 3a scope: `Bitmap`, per-frame palette build, `DrawLevel` Classic
      //! arm, the two-viewport 320x200 player layout, and the frame hash. Shadows,
      //! sprites, HUD, minimap arrive in 3b/3e.
      pub mod bitmap;
      ```
      (Later tasks append `pub mod palette; pub mod hash; pub mod level_draw; pub mod viewport;
      pub mod frame;` as they land.)
- [ ] **RED:** write `rust/render/src/bitmap.rs` with the type declarations and a `#[cfg(test)]`
      module whose tests reference the not-yet-written method bodies. Write the tests FIRST (bodies
      `unimplemented!()`), then run and see them fail:
      ```rust
      //! ARGB8888 destination surface + clip rect. Port of `bitmap.hpp:12-54`,
      //! `blit.cpp:20-41`. `pitch` is in PIXELS (`bitmap.hpp:15`); addressing is
      //! `y*pitch + x`. Drawing resolves an 8-bit index through `pal32` at write time.

      /// Colour mode of the owning renderer. Classic resolves terrain via
      /// `pal32[material_id]`; Modern (deferred, `level.hpp:220`) returns authored
      /// ARGB. Kept in the API so Modern is a later `match` arm, not a refactor.
      #[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
      pub enum ColorMode {
          #[default]
          Classic,
          Modern,
      }

      /// The 256-entry ARGB lookup the renderer packs each frame (`renderer.cpp:23`).
      pub type Pal32 = [u32; 256];

      /// Integer rectangle. Port of `BasicRect<int>` (`math/rect.hpp:60-113`); only
      /// the members 3a needs. Half-open: `inside` is `[x1,x2) x [y1,y2)`.
      #[derive(Clone, Copy, PartialEq, Eq, Debug)]
      pub struct Rect {
          pub x1: i32,
          pub y1: i32,
          pub x2: i32,
          pub y2: i32,
      }

      impl Rect {
          pub fn new(x1: i32, y1: i32, x2: i32, y2: i32) -> Rect {
              Rect { x1, y1, x2, y2 }
          }
          /// `rect.hpp:70`.
          pub fn width(&self) -> i32 {
              self.x2 - self.x1
          }
          /// `rect.hpp:71`.
          pub fn height(&self) -> i32 {
              self.y2 - self.y1
          }
          /// `rect.hpp:75` `Ul()`.
          pub fn ul(&self) -> (i32, i32) {
              (self.x1, self.y1)
          }
          /// `rect.hpp:77-81`: `dx>=0 && dx<width && dy>=0 && dy<height`.
          pub fn inside(&self, x: i32, y: i32) -> bool {
              let dx = x - self.x1;
              let dy = y - self.y1;
              dx >= 0 && dx < self.width() && dy >= 0 && dy < self.height()
          }
      }

      /// ARGB8888 surface. `bitmap.hpp:12-24`.
      #[derive(Clone, PartialEq, Eq, Debug)]
      pub struct Bitmap {
          pub w: i32,
          pub h: i32,
          /// In PIXELS, not bytes (`bitmap.hpp:15`).
          pub pitch: i32,
          pub pixels: Vec<u32>,
          pub clip: Rect,
          /// Frame counter for animated terrain (`bitmap.hpp:23`); unused by the
          /// Classic `draw_level` arm but kept for 3b/Modern parity.
          pub cycles: i32,
      }

      impl Bitmap {
          /// Allocate a `w x h` surface (pitch = w), zero-filled, clip = full.
          /// Mirrors `Bitmap::Alloc` (`bitmap.hpp:31-46`).
          pub fn new(w: i32, h: i32) -> Bitmap {
              Bitmap {
                  w,
                  h,
                  pitch: w,
                  pixels: vec![0u32; (w * h) as usize],
                  clip: Rect::new(0, 0, w, h),
                  cycles: 0,
              }
          }

          /// `bitmap.hpp:50-54`: clip-gated write of `pal32[idx]` at `y*pitch + x`.
          pub fn set_pixel(&mut self, x: i32, y: i32, idx: u8, pal: &Pal32) {
              if self.clip.inside(x, y) {
                  self.pixels[(y * self.pitch + x) as usize] = pal[idx as usize];
              }
          }

          /// `blit.cpp:39-41`: repaint the WHOLE buffer through the LUT. C++ `Fill`
          /// ignores `clip_rect` (it fills `pixels .. pixels + pitch*h`); 3a relies
          /// on this to paint the HUD gap as `pal32[0]`.
          pub fn fill(&mut self, idx: u8, pal: &Pal32) {
              let argb = pal[idx as usize];
              for p in self.pixels.iter_mut() {
                  *p = argb;
              }
          }

          /// `blit.cpp:20-37`: clip-CLAMPED rectangle fill (used by later slices; kept
          /// here so the clamp math is unit-tested now).
          pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, idx: u8, pal: &Pal32) {
              let mut x0 = x.max(self.clip.x1);
              let mut y0 = y.max(self.clip.y1);
              let x1 = (x + w).min(self.clip.x2);
              let y1 = (y + h).min(self.clip.y2);
              if x1 > x0 {
                  let argb = pal[idx as usize];
                  while y0 < y1 {
                      let mut cx = x0;
                      while cx < x1 {
                          self.pixels[(y0 * self.pitch + cx) as usize] = argb;
                          cx += 1;
                      }
                      y0 += 1;
                  }
                  let _ = &mut x0; // silence unused-mut if refactored
              }
          }
      }

      #[cfg(test)]
      mod tests {
          use super::*;

          // pal[i] = 0xFF000000 | i (distinct per index so a pixel reveals its index).
          fn ramp_pal() -> Pal32 {
              let mut p = [0u32; 256];
              for (i, e) in p.iter_mut().enumerate() {
                  *e = 0xFF00_0000 | i as u32;
              }
              p
          }

          #[test]
          fn set_pixel_writes_inside_and_uses_pitch() {
              let pal = ramp_pal();
              // pitch != w so a wrong `w`-vs-`pitch` index is caught.
              let mut b = Bitmap {
                  w: 4,
                  h: 3,
                  pitch: 6,
                  pixels: vec![0u32; 6 * 3],
                  clip: Rect::new(0, 0, 4, 3),
                  cycles: 0,
              };
              b.set_pixel(2, 1, 7, &pal);
              // index = y*pitch + x = 1*6 + 2 = 8.
              assert_eq!(b.pixels[8], 0xFF00_0007);
              assert_eq!(b.pixels[2 + 1 * 4], 0, "must NOT index by w");
          }

          #[test]
          fn set_pixel_drops_outside_clip() {
              let pal = ramp_pal();
              let mut b = Bitmap::new(4, 4);
              b.clip = Rect::new(1, 1, 3, 3);
              b.set_pixel(0, 0, 5, &pal); // outside clip
              b.set_pixel(2, 2, 5, &pal); // inside clip
              assert_eq!(b.pixels[0], 0, "outside-clip write dropped");
              assert_eq!(b.pixels[2 * 4 + 2], 0xFF00_0005, "inside-clip write kept");
          }

          #[test]
          fn fill_ignores_clip_and_paints_whole_buffer() {
              let pal = ramp_pal();
              let mut b = Bitmap::new(3, 2);
              b.clip = Rect::new(1, 1, 2, 2); // narrow clip must NOT limit fill
              b.fill(9, &pal);
              assert!(b.pixels.iter().all(|&p| p == 0xFF00_0009), "Fill is whole-buffer");
          }

          #[test]
          fn fill_rect_clamps_to_clip() {
              let pal = ramp_pal();
              let mut b = Bitmap::new(5, 5);
              b.clip = Rect::new(1, 1, 4, 4);
              b.fill_rect(0, 0, 10, 10, 3, &pal); // overspills on every side
              // Corners outside clip stay 0; a pixel inside clip is painted.
              assert_eq!(b.pixels[0], 0, "(0,0) outside clip");
              assert_eq!(b.pixels[4 * 5 + 4], 0, "(4,4) outside clip");
              assert_eq!(b.pixels[1 * 5 + 1], 0xFF00_0003, "(1,1) inside clip");
              assert_eq!(b.pixels[3 * 5 + 3], 0xFF00_0003, "(3,3) inside clip");
          }
      }
      ```
- [ ] Run `cargo test -p render` — expect the compile+RED to turn GREEN once the bodies above are
      in place (they are written inline; this task ships the bodies with the tests). Then run
      `cargo build -p render` and confirm the crate compiles with **no** Bevy in the tree:
      `cargo tree -p render` must show only `sim-core`, `assets`, `sim` (no `bevy*`).
- [ ] Reviewer (Opus): `pitch`-based indexing everywhere (not `w`); `fill` is whole-buffer (matches
      `blit.cpp:39`, NOT clip-clamped); `fill_rect` clamp matches `blit.cpp:20-37`; `set_pixel` clip
      gate matches `bitmap.hpp:50`; `inside` is half-open. No Bevy dep.
- [ ] **Commit** (branch `liero-rs-step-3`):
      - `git add rust/Cargo.toml rust/render/Cargo.toml rust/render/src/lib.rs rust/render/src/bitmap.rs`
      - `git commit -m "render(3a): crate skeleton + Bitmap/Rect/ColorMode + clip-gated primitives"`

---

### T1 — palette module: `RotateFrom` / `LightUp` / `pack_pal32` / `build_palette`  [Opus]

**Files**
- Create: `rust/render/src/palette.rs`
- Modify: `rust/render/src/lib.rs` (add `pub mod palette;`)

**Interfaces**
- Consumes: `render::bitmap::Pal32`; `assets::palette::{Palette, Color}`
  (`Palette { entries: [Color; 256] }`, `Color { r: u8, g: u8, b: u8 }`); `assets::tc::ColorAnim`
  (`ColorAnim { from: i32, to: i32 }`).
- Produces:
  - `render::palette::rotate_from(dst: &mut Palette, source: &Palette, from: i32, to: i32, dist: u32)`
  - `render::palette::light_up(pal: &mut Palette, amount: i32)`
  - `render::palette::pack_pal32(pal: &Palette) -> Pal32`
  - `render::palette::build_palette(origpal: &Palette, color_anim: &[ColorAnim], cycles: i32, screen_flash: i32) -> Pal32`

**Steps**

- [ ] Add `pub mod palette;` to `rust/render/src/lib.rs`.
- [ ] **RED:** create `rust/render/src/palette.rs` with the signatures and a test module that pins
      the C++ formulas on hand tables. Write tests first (bodies `unimplemented!()`), run, see FAIL:
      ```rust
      #[test]
      fn rotate_from_matches_cpp_formula() {
          use assets::palette::{Color, Palette};
          // entries[i] = (i, 0, 0) so a rotation is visible in `r`.
          let mut src = Palette { entries: [Color::default(); 256] };
          for (i, e) in src.entries.iter_mut().enumerate() {
              e.r = i as u8;
          }
          // Rotate sub-range [2, 5] (count 4) by dist 1.
          let mut dst = src.clone();
          rotate_from(&mut dst, &src, 2, 5, 1);
          // entries[from+i] = source[from + ((i + count - dist) % count)], count=4, dist=1.
          // i=0 -> src[2 + (0+4-1)%4]=src[2+3]=src[5]; i=1 -> src[2]; i=2 -> src[3]; i=3 -> src[4].
          assert_eq!(dst.entries[2].r, 5);
          assert_eq!(dst.entries[3].r, 2);
          assert_eq!(dst.entries[4].r, 3);
          assert_eq!(dst.entries[5].r, 4);
          // Outside the range is untouched.
          assert_eq!(dst.entries[1].r, 1);
          assert_eq!(dst.entries[6].r, 6);
      }

      #[test]
      fn rotate_from_dist_zero_is_identity_and_wraps_modulo_count() {
          use assets::palette::{Color, Palette};
          let mut src = Palette { entries: [Color::default(); 256] };
          for (i, e) in src.entries.iter_mut().enumerate() {
              e.r = i as u8;
          }
          let mut id = src.clone();
          rotate_from(&mut id, &src, 10, 13, 0);
          for i in 10..=13 {
              assert_eq!(id.entries[i].r, i as u8, "dist 0 is identity");
          }
          // dist %= count: count=4, dist 5 behaves like dist 1.
          let mut d5 = src.clone();
          let mut d1 = src.clone();
          rotate_from(&mut d5, &src, 10, 13, 5);
          rotate_from(&mut d1, &src, 10, 13, 1);
          assert_eq!(d5.entries[10].r, d1.entries[10].r);
      }

      #[test]
      fn light_up_matches_cpp_and_clamps() {
          use assets::palette::{Color, Palette};
          let mut pal = Palette { entries: [Color::default(); 256] };
          pal.entries[0] = Color { r: 100, g: 0, b: 200 };
          light_up(&mut pal, 8);
          // (v*(32-a)+a*255)>>5, a=8: r=(100*24+8*255)>>5=(2400+2040)>>5=4440>>5=138;
          // g=(0*24+2040)>>5=63; b=(200*24+2040)>>5=(4800+2040)>>5=6840>>5=213.
          assert_eq!(pal.entries[0], Color { r: 138, g: 63, b: 213 });
          // Clamp to 255: a=31, v=255 -> (255*1 + 31*255)>>5 = (255+7905)>>5 = 255.
          let mut hi = Palette { entries: [Color { r: 255, g: 255, b: 255 }; 256] };
          light_up(&mut hi, 31);
          assert_eq!(hi.entries[0], Color { r: 255, g: 255, b: 255 });
      }

      #[test]
      fn pack_pal32_packs_argb_without_requantizing() {
          use assets::palette::{Color, Palette};
          let mut pal = Palette { entries: [Color::default(); 256] };
          pal.entries[1] = Color { r: 0x12, g: 0x34, b: 0x56 };
          let p = pack_pal32(&pal);
          assert_eq!(p[1], 0xFF12_3456, "0xFF000000|r<<16|g<<8|b, entries verbatim");
          assert_eq!(p[0], 0xFF00_0000);
      }

      #[test]
      fn build_palette_order_reset_rotate_pack_and_screen_flash_inert() {
          use assets::palette::{Color, Palette};
          use assets::tc::ColorAnim;
          let mut origpal = Palette { entries: [Color::default(); 256] };
          for (i, e) in origpal.entries.iter_mut().enumerate() {
              e.r = i as u8;
          }
          let anim = [ColorAnim { from: 2, to: 5 }];
          // cycles>>3 = 8>>3 = 1 -> the [2,5] range rotates by 1 (as rotate_from test).
          let p8 = build_palette(&origpal, &anim, 8, 0);
          assert_eq!(p8[2] & 0xFF, 5, "cycles 8: entry 2 rotated to src[5].r");
          // cycles 0..7 -> dist 0 -> identity in the animated range.
          let p0 = build_palette(&origpal, &anim, 0, 0);
          assert_eq!(p0[2] & 0xFF, 2, "cycles 0: identity");
          // screen_flash 0 is inert: the LUT equals the rotate-only build.
          let p8b = build_palette(&origpal, &anim, 8, 0);
          assert_eq!(p8, p8b);
      }
      ```
- [ ] **GREEN:** implement the module. `rotate_from` mirrors `palette.cpp:50-57`; `light_up`
      mirrors `palette.cpp:24-28,42-48`; `pack_pal32` mirrors `renderer.cpp:23-30`; `build_palette`
      mirrors `game.cpp:171-183` (reset → rotate each anim FROM origpal → optional light_up → pack):
      ```rust
      //! Per-frame palette build. Ports `game.cpp:171-183` (order),
      //! `palette.cpp:42-57` (LightUp/RotateFrom), `renderer.cpp:23-30` (pal32 pack).
      //! Classic entries are already 6-bit-VGA (`(v&63)<<2`); the pack is straight,
      //! with NO second quantization.

      use crate::bitmap::Pal32;
      use assets::palette::Palette;
      use assets::tc::ColorAnim;

      /// `palette.cpp:50-57`: rotate the sub-range `[from, to]` of `dst` from the
      /// UNROTATED `source`. `count = to-from+1`, `dist %= count`,
      /// `dst[from+i] = source[from + ((i + count - dist) % count)]`.
      pub fn rotate_from(dst: &mut Palette, source: &Palette, from: i32, to: i32, dist: u32) {
          let count = to - from + 1;
          let d = (dist % count as u32) as i32;
          for i in 0..count {
              let s = from + ((i + count - d) % count);
              dst.entries[(from + i) as usize] = source.entries[s as usize];
          }
      }

      /// `palette.cpp:24-28,42-48`: `(v*(32-a)+a*255)>>5`, clamped to 255, per channel.
      pub fn light_up(pal: &mut Palette, amount: i32) {
          let f = |v: u8| -> u8 {
              let x = (v as i32 * (32 - amount) + amount * 255) >> 5;
              x.min(255) as u8
          };
          for e in pal.entries.iter_mut() {
              e.r = f(e.r);
              e.g = f(e.g);
              e.b = f(e.b);
          }
      }

      /// `renderer.cpp:23-30`: `0xFF000000 | r<<16 | g<<8 | b`, entries verbatim.
      pub fn pack_pal32(pal: &Palette) -> Pal32 {
          let mut out = [0u32; 256];
          for (i, e) in pal.entries.iter().enumerate() {
              out[i] = 0xFF00_0000 | ((e.r as u32) << 16) | ((e.g as u32) << 8) | e.b as u32;
          }
          out
      }

      /// `game.cpp:171-183`: reset to origpal -> RotateFrom each color_anim FROM
      /// origpal (source is always the ORIGINAL) -> LightUp if screen_flash>0 (inert
      /// in 3a) -> pack. `cycles>>3` is a signed shift then an unsigned distance.
      pub fn build_palette(
          origpal: &Palette,
          color_anim: &[ColorAnim],
          cycles: i32,
          screen_flash: i32,
      ) -> Pal32 {
          let mut pal = origpal.clone();
          let dist = (cycles >> 3) as u32;
          for a in color_anim {
              rotate_from(&mut pal, origpal, a.from, a.to, dist);
          }
          if screen_flash > 0 {
              light_up(&mut pal, screen_flash);
          }
          pack_pal32(&pal)
      }
      ```
- [ ] Run `cargo test -p render palette` — expect PASS.
- [ ] Reviewer (Opus): `rotate_from` source is `origpal` per anim (not the running pal); `dist %=
      count`; `light_up` clamps to 255 only (no lower clamp needed — non-negative inputs); pack does
      NOT re-quantize; `build_palette` order is reset→rotate→light_up→pack; `screen_flash` branch
      present but gated `>0`.
- [ ] **Commit:**
      - `git add rust/render/src/palette.rs rust/render/src/lib.rs`
      - `git commit -m "render(3a): per-frame palette build (RotateFrom/LightUp/pal32 pack)"`

---

### T2 — hash module: FNV-1a `hash_frame` + `FadeChannel`  [Opus]

**Files**
- Create: `rust/render/src/hash.rs`
- Modify: `rust/render/src/lib.rs` (add `pub mod hash;`)

**Interfaces**
- Consumes: `render::bitmap::{Bitmap, Rect}`.
- Produces:
  - `render::hash::FNV_OFFSET: u64` (`1469598103934665603`), `render::hash::FNV_PRIME: u64`
    (`1099511628211`).
  - `render::hash::fade_channel(v: u8, amount: i32) -> u8`.
  - `render::hash::hash_frame(bmp: &Bitmap, fade: i32) -> u64`.

**Steps**

- [ ] Add `pub mod hash;` to `rust/render/src/lib.rs`.
- [ ] **RED:** create `rust/render/src/hash.rs` with the test module first (bodies
      `unimplemented!()`), run, see FAIL:
      ```rust
      #[test]
      fn fade_channel_matches_framehash() {
          // fade>=32 is identity.
          assert_eq!(fade_channel(200, 32), 200);
          assert_eq!(fade_channel(200, 33), 200);
          // fade<32: (v*amount)>>5. v=200, a=16 -> (3200)>>5 = 100.
          assert_eq!(fade_channel(200, 16), 100);
          // fade 0 -> black.
          assert_eq!(fade_channel(255, 0), 0);
      }

      #[test]
      fn hash_frame_fnv1a_rgb_order_one_pixel() {
          use crate::bitmap::{Bitmap, Rect};
          // 1x1 surface, pixel = 0xFF_11_22_33 (A=FF,R=11,G=22,B=33).
          let b = Bitmap {
              w: 1,
              h: 1,
              pitch: 1,
              pixels: vec![0xFF11_2233],
              clip: Rect::new(0, 0, 1, 1),
              cycles: 0,
          };
          // Hand FNV-1a over bytes 0x11,0x22,0x33 (R,G,B), fade=33 (identity).
          let mut h = FNV_OFFSET;
          for byte in [0x11u8, 0x22, 0x33] {
              h = (h ^ byte as u64).wrapping_mul(FNV_PRIME);
          }
          assert_eq!(hash_frame(&b, 33), h);
      }

      #[test]
      fn hash_frame_fade_zero_is_all_black_constant() {
          use crate::bitmap::{Bitmap, Rect};
          // Two DIFFERENT 2x1 frames hash to the SAME value at fade=0 (all channels ->0).
          let a = Bitmap { w: 2, h: 1, pitch: 2, pixels: vec![0xFFAA_BBCC, 0xFF01_0203],
                           clip: Rect::new(0, 0, 2, 1), cycles: 0 };
          let b = Bitmap { w: 2, h: 1, pitch: 2, pixels: vec![0xFF99_8877, 0xFF44_5566],
                           clip: Rect::new(0, 0, 2, 1), cycles: 0 };
          assert_eq!(hash_frame(&a, 0), hash_frame(&b, 0));
          // And it equals FNV over 6 zero bytes.
          let mut h = FNV_OFFSET;
          for _ in 0..6 {
              h = (h ^ 0u64).wrapping_mul(FNV_PRIME);
          }
          assert_eq!(hash_frame(&a, 0), h);
      }

      #[test]
      fn hash_frame_ignores_alpha() {
          use crate::bitmap::{Bitmap, Rect};
          let a = Bitmap { w: 1, h: 1, pitch: 1, pixels: vec![0x0011_2233],
                           clip: Rect::new(0, 0, 1, 1), cycles: 0 };
          let b = Bitmap { w: 1, h: 1, pitch: 1, pixels: vec![0xFF11_2233],
                           clip: Rect::new(0, 0, 1, 1), cycles: 0 };
          assert_eq!(hash_frame(&a, 33), hash_frame(&b, 33), "alpha dropped");
      }
      ```
- [ ] **GREEN:** implement the module (`framehash_main.cpp:26-50`). Iterate row-major over
      `w x h` using `pitch` for addressing (so a `pitch != w` sub-bitmap is hashed over its used
      columns only, exactly like `HashFrame`):
      ```rust
      //! FNV-1a frame hash over the ARGB buffer, R,G,B order, alpha dropped, with a
      //! composition-time fade. Port of `framehash_main.cpp:26-50`. The regression
      //! primitive: two renderers agree iff their frame hashes agree.

      use crate::bitmap::Bitmap;

      /// `framehash_main.cpp:26`.
      pub const FNV_OFFSET: u64 = 1469598103934665603;
      /// `framehash_main.cpp:27`.
      pub const FNV_PRIME: u64 = 1099511628211;

      /// `framehash_main.cpp:32-34`: identity at `amount>=32`, else `(v*amount)>>5`.
      pub fn fade_channel(v: u8, amount: i32) -> u8 {
          if amount >= 32 {
              v
          } else {
              ((v as i32 * amount) >> 5) as u8
          }
      }

      /// `framehash_main.cpp:38-50`: FNV-1a over each pixel's R,G,B (alpha dropped),
      /// faded, row-major. `fade` is 0 on frame 0 (black), 33 afterwards (identity).
      pub fn hash_frame(bmp: &Bitmap, fade: i32) -> u64 {
          let mut h = FNV_OFFSET;
          for y in 0..bmp.h {
              for x in 0..bmp.w {
                  let c = bmp.pixels[(y * bmp.pitch + x) as usize];
                  let r = fade_channel(((c >> 16) & 0xFF) as u8, fade);
                  let g = fade_channel(((c >> 8) & 0xFF) as u8, fade);
                  let b = fade_channel((c & 0xFF) as u8, fade);
                  h = (h ^ r as u64).wrapping_mul(FNV_PRIME);
                  h = (h ^ g as u64).wrapping_mul(FNV_PRIME);
                  h = (h ^ b as u64).wrapping_mul(FNV_PRIME);
              }
          }
          h
      }
      ```
- [ ] Run `cargo test -p render hash` — expect PASS.
- [ ] Reviewer (Opus): constants exact; R,G,B order (not B,G,R); alpha dropped; `pitch` addressing
      (iterating `0..w`, not `0..pitch`); `fade=0` yields the all-black constant; wrapping u64 mul.
- [ ] **Commit:**
      - `git add rust/render/src/hash.rs rust/render/src/lib.rs`
      - `git commit -m "render(3a): FNV-1a frame hash + FadeChannel"`

---

### T3 — `draw_level` (Classic + `CLIP_IMAGE`) + `Viewport` + `frame::draw`  [Opus]

**Files**
- Create: `rust/render/src/level_draw.rs`, `rust/render/src/viewport.rs`, `rust/render/src/frame.rs`
- Modify: `rust/render/src/lib.rs` (add `pub mod level_draw; pub mod viewport; pub mod frame;`)

**Interfaces**
- Consumes:
  - `render::bitmap::{Bitmap, Rect, Pal32, ColorMode}`; `render::palette::build_palette`.
  - `sim::state::{SimState, LevelSim, WormState, KILLED_TIMER_INITIAL}` where
    `LevelSim { width: i32, height: i32, material_id: Vec<u8>, .. }`,
    `WormState { pos: sim_core::vec::Vec2, visible: bool, health: i32, killed_timer: i32, steerable_count: i32, .. }`,
    `SimState { cycles: i32, level: LevelSim, worms: Vec<WormState>, .. }`.
  - `sim_core::vec::Vec2 { x: i32, y: i32 }`; `sim_core::fixed::ftoi(v: i32) -> i32` (`v >> 16`).
  - `sim_core::rng::Rand` (`Rand::new()`, `bound(max: u32) -> u32`, `draws() -> u64`).
  - `assets::palette::Palette`; `assets::tc::ColorAnim`.
- Produces:
  - `render::level_draw::draw_level(dst: &mut Bitmap, lvl: &LevelSim, pal: &Pal32, x: i32, y: i32, mode: ColorMode)`
  - `render::viewport::Viewport { pub rect: Rect, pub worm_idx: usize, pub x: i32, pub y: i32, pub shake: i32, pub center_x: i32, pub center_y: i32, pub banner_y: i32, pub max_x: i32, pub max_y: i32, pub rand: Rand }`
    with `fn new(rect: Rect, worm_idx: usize) -> Viewport`,
    `fn player_layout() -> [Viewport; 2]`,
    `fn set_center(&mut self, x: i32, y: i32)`,
    `fn scroll_to(&mut self, dest_x: i32, dest_y: i32, iter: i32)`,
    `fn process(&mut self, worm: &WormState, level_w: i32, level_h: i32)`.
  - `render::frame::draw(bmp: &mut Bitmap, state: &SimState, origpal: &Palette, color_anim: &[ColorAnim], viewports: &mut [Viewport], screen_flash: i32)`.

**Steps**

- [ ] Add `pub mod level_draw; pub mod viewport; pub mod frame;` to `rust/render/src/lib.rs`.
- [ ] **RED (level_draw):** create `rust/render/src/level_draw.rs`. Test on a tiny synthetic
      `LevelSim`. Build a `LevelSim` directly (it is a plain struct). Write the test first, see FAIL:
      ```rust
      #[cfg(test)]
      mod tests {
          use super::*;
          use crate::bitmap::{Bitmap, ColorMode, Rect};
          use sim::state::LevelSim;

          fn ramp_pal() -> [u32; 256] {
              let mut p = [0u32; 256];
              for (i, e) in p.iter_mut().enumerate() {
                  *e = 0xFF00_0000 | i as u32;
              }
              p
          }
          // 2x2 level, material_id = [10, 11, 12, 13] row-major.
          fn lvl() -> LevelSim {
              LevelSim { width: 2, height: 2, material_id: vec![10, 11, 12, 13],
                         material_flags: [0u8; 256] }
          }

          #[test]
          fn draw_level_resolves_material_through_pal32_at_offset() {
              let pal = ramp_pal();
              let mut b = Bitmap::new(4, 4); // clip = full
              draw_level(&mut b, &lvl(), &pal, 1, 1, ColorMode::Classic);
              // level (0,0)->screen (1,1); material_id[0]=10.
              assert_eq!(b.pixels[1 * 4 + 1], 0xFF00_000A);
              assert_eq!(b.pixels[1 * 4 + 2], 0xFF00_000B); // (1,0)->(2,1) mat 11
              assert_eq!(b.pixels[2 * 4 + 1], 0xFF00_000C); // (0,1)->(1,2) mat 12
              assert_eq!(b.pixels[2 * 4 + 2], 0xFF00_000D); // (1,1)->(2,2) mat 13
              // Untouched pixel stays 0.
              assert_eq!(b.pixels[0], 0);
          }

          #[test]
          fn draw_level_clips_to_clip_rect() {
              let pal = ramp_pal();
              let mut b = Bitmap::new(4, 4);
              b.clip = Rect::new(2, 2, 4, 4); // only the bottom-right quadrant
              draw_level(&mut b, &lvl(), &pal, 1, 1, ColorMode::Classic);
              // Only level (1,1)->screen (2,2) is inside the clip.
              assert_eq!(b.pixels[2 * 4 + 2], 0xFF00_000D);
              assert_eq!(b.pixels[1 * 4 + 1], 0, "clipped out");
              assert_eq!(b.pixels[1 * 4 + 2], 0, "clipped out");
          }

          #[test]
          fn draw_level_negative_offset_clamps_source() {
              let pal = ramp_pal();
              let mut b = Bitmap::new(2, 2); // clip = full 2x2
              // Offset (-1,-1): level (1,1)->screen(0,0); the rest is off-surface.
              draw_level(&mut b, &lvl(), &pal, -1, -1, ColorMode::Classic);
              assert_eq!(b.pixels[0], 0xFF00_000D, "source clamped by top/left");
              assert!(b.pixels[1..].iter().all(|&p| p == 0));
          }
      }
      ```
- [ ] **GREEN (level_draw):** port `DrawLevel` (`blit.cpp:194-213`) with the `CLIP_IMAGE` clamp
      (`macros.hpp:3-22`). Direct writes (not `set_pixel`) — clipping is done by clamping the loop
      bounds and source offset, matching C++:
      ```rust
      //! Terrain draw: paints `AppearanceAt` into the surface at `(x, y)`. Port of
      //! `DrawLevel` (`blit.cpp:194-213`) + the `CLIP_IMAGE` clamp (`macros.hpp:3-22`).
      //! Classic arm = `pal32[material_id[idx]]` (`level.hpp:59-64`); the Modern arm is
      //! deferred (overview Classic-vs-Modern).

      use crate::bitmap::{Bitmap, ColorMode, Pal32};
      use sim::state::LevelSim;

      pub fn draw_level(dst: &mut Bitmap, lvl: &LevelSim, pal: &Pal32, x: i32, y: i32, mode: ColorMode) {
          match mode {
              ColorMode::Classic => {}
              ColorMode::Modern => unimplemented!("Modern AppearanceAt deferred past 3a"),
          }
          // CLIP_IMAGE (macros.hpp): clamp width/height/x/y and slide the source index.
          let mut x = x;
          let mut y = y;
          let mut width = lvl.width;
          let mut height = lvl.height;
          let pitch = lvl.width; // source (level) pitch
          let clip = dst.clip;
          // `mem` in C++ is a pointer into material_id; we track it as a flat index.
          let mut src_idx: i32 = 0;

          let top = y - clip.y1;
          if top < 0 {
              src_idx += -top * pitch;
              height += top;
              y = clip.y1;
          }
          let bottom = y + height - clip.y2;
          if bottom > 0 {
              height -= bottom;
          }
          let left = x - clip.x1;
          if left < 0 {
              src_idx -= left;
              width += left;
              x = clip.x1;
          }
          let right = x + width - clip.x2;
          if right > 0 {
              width -= right;
          }
          if width <= 0 || height <= 0 {
              return;
          }

          let scr_pitch = dst.pitch;
          let mut scr_row = y * scr_pitch + x;
          let mut idx = src_idx;
          for _dy in 0..height {
              for dx in 0..width {
                  let mat = lvl.material_id[(idx + dx) as usize];
                  dst.pixels[(scr_row + dx) as usize] = pal[mat as usize];
              }
              scr_row += scr_pitch;
              idx += pitch;
          }
      }
      ```
- [ ] Run `cargo test -p render level_draw` — expect PASS.
- [ ] **RED (viewport):** create `rust/render/src/viewport.rs`. Build a synthetic `WormState`
      (use `WormState::default()`-equivalent via the crate's constructor if exposed; otherwise
      construct the fields the test needs — check `sim::state::WormState` field list and build one).
      Test the centering math and the inert shake RNG. Write first, see FAIL:
      ```rust
      #[cfg(test)]
      mod tests {
          use super::*;
          use crate::bitmap::Rect;
          use sim::state::WormState;
          use sim_core::vec::Vec2;

          // A worm helper: alive, visible, static, no steerables. Adjust field names to
          // WormState's actual definition (state.rs:261); set pos in 16.16 fixed.
          fn worm_at(px: i32, py: i32) -> WormState {
              let mut w = WormState::default(); // if no Default, construct explicitly
              w.pos = Vec2::new(px << 16, py << 16);
              w.visible = true;
              w.health = 100;
              w.killed_timer = 0;
              w.steerable_count = 0;
              w
          }

          #[test]
          fn process_centers_on_worm_and_clamps() {
              // rect 158x158, level 200x200 -> max_x=max_y=42, center=79.
              let mut vp = Viewport::new(Rect::new(0, 0, 158, 158), 0);
              vp.process(&worm_at(100, 100), 200, 200);
              // SetCenter: x = 100 - 79 = 21, y = 21; within [0,42] -> unclamped.
              assert_eq!((vp.x, vp.y), (21, 21));
              // A worm near the far edge clamps to max.
              vp.process(&worm_at(199, 199), 200, 200);
              assert_eq!((vp.x, vp.y), (42, 42), "clamped to max_x/max_y");
              // A worm near origin clamps to 0.
              vp.process(&worm_at(1, 1), 200, 200);
              assert_eq!((vp.x, vp.y), (0, 0), "clamped to 0");
          }

          #[test]
          fn process_shake_branch_inert_leaves_rand_untouched() {
              let mut vp = Viewport::new(Rect::new(0, 0, 158, 158), 0);
              assert_eq!(vp.shake, 0, "3a viewport has no shake");
              let before = vp.rand.draws();
              vp.process(&worm_at(100, 100), 200, 200);
              assert_eq!(vp.rand.draws(), before, "shake==0 -> no rand drawn");
          }

          #[test]
          fn player_layout_matches_framehash_rects() {
              let vps = Viewport::player_layout();
              assert_eq!((vps[0].rect.x1, vps[0].rect.y1, vps[0].rect.x2, vps[0].rect.y2),
                         (0, 0, 158, 158));
              assert_eq!(vps[0].worm_idx, 0);
              assert_eq!((vps[1].rect.x1, vps[1].rect.y1, vps[1].rect.x2, vps[1].rect.y2),
                         (160, 0, 318, 158));
              assert_eq!(vps[1].worm_idx, 1);
          }
      }
      ```
      (If `WormState` has no `Default`, construct it explicitly in `worm_at` from its actual fields
      — read `state.rs:261` for the full list; the test only depends on `pos`, `visible`, `health`,
      `killed_timer`, `steerable_count`.)
- [ ] **GREEN (viewport):** implement `Viewport` (`viewport.hpp:11-58`, `viewport.cpp:22-57`). The
      viewport-local `Rand` is a default-seeded `sim_core::Rand`, never advanced in 3a:
      ```rust
      //! Player viewport: centering/scroll/clamp. Port of `viewport.hpp:11-58` +
      //! `viewport.cpp:22-57`. The world-block draw (clip + DrawLevel at kOffs) lives
      //! in `frame::draw` (viewport.cpp:196-210). The shake RNG branch is present but
      //! gated on `shake>0` (inert in 3a); the laser-sight RNG arrives in 3b.

      use crate::bitmap::Rect;
      use sim::state::{WormState, KILLED_TIMER_INITIAL};
      use sim_core::fixed::ftoi;
      use sim_core::rng::Rand;

      pub struct Viewport {
          pub rect: Rect,
          pub worm_idx: usize,
          pub x: i32,
          pub y: i32,
          pub shake: i32,
          pub max_x: i32,
          pub max_y: i32,
          pub center_x: i32,
          pub center_y: i32,
          pub banner_y: i32,
          /// Default-seeded, display-only RNG (`viewport.hpp:33`, `rand.hpp`). Never
          /// advanced in 3a (no shake, no laser).
          pub rand: Rand,
      }

      impl Viewport {
          /// `viewport.hpp:12-22`.
          pub fn new(rect: Rect, worm_idx: usize) -> Viewport {
              Viewport {
                  center_x: rect.width() >> 1,
                  center_y: rect.height() >> 1,
                  rect,
                  worm_idx,
                  x: 0,
                  y: 0,
                  shake: 0,
                  max_x: 0,
                  max_y: 0,
                  banner_y: -8,
                  rand: Rand::new(),
              }
          }

          /// The two-viewport 320x200 player layout (framehash_main.cpp:92-95).
          pub fn player_layout() -> [Viewport; 2] {
              [
                  Viewport::new(Rect::new(0, 0, 158, 158), 0),
                  Viewport::new(Rect::new(160, 0, 318, 158), 1),
              ]
          }

          /// `viewport.hpp:35-38`.
          pub fn set_center(&mut self, x: i32, y: i32) {
              self.x = x - self.center_x;
              self.y = y - self.center_y;
          }

          /// `viewport.hpp:40-54`.
          pub fn scroll_to(&mut self, dest_x: i32, dest_y: i32, iter: i32) {
              for _ in 0..iter {
                  if self.x < dest_x - self.center_x {
                      self.x += 1;
                  } else if self.x > dest_x - self.center_x {
                      self.x -= 1;
                  }
                  if self.y < dest_y - self.center_y {
                      self.y += 1;
                  } else if self.y > dest_y - self.center_y {
                      self.y -= 1;
                  }
              }
          }

          /// `viewport.cpp:22-57`. NB: the `steerable_count > 0` centering
          /// (`viewport.cpp:31-32`) reads `steerable_sum_x/y`, which `WormState` does
          /// not yet carry; 3a scenarios keep `steerable_count == 0`, so that arm is
          /// asserted-unreachable and the pos-centering arm is used. Steerable
          /// centering lands with the sprite pass in 3b.
          pub fn process(&mut self, worm: &WormState, level_w: i32, level_h: i32) {
              self.max_x = level_w - self.rect.width();
              self.max_y = level_h - self.rect.height();

              if worm.killed_timer <= 0 {
                  if worm.visible {
                      debug_assert_eq!(
                          worm.steerable_count, 0,
                          "steerable centering deferred to 3b"
                      );
                      self.set_center(ftoi(worm.pos.x), ftoi(worm.pos.y));
                  } else {
                      self.scroll_to(ftoi(worm.pos.x), ftoi(worm.pos.y), 4);
                  }
              } else if worm.health <= 0 {
                  self.set_center(ftoi(worm.pos.x), ftoi(worm.pos.y));
                  if worm.killed_timer == KILLED_TIMER_INITIAL {
                      self.banner_y = -8;
                  }
              }

              let real_shake = ftoi(self.shake);
              if real_shake > 0 {
                  self.x += self.rand.bound((real_shake * 2) as u32) as i32 - real_shake;
                  self.y += self.rand.bound((real_shake * 2) as u32) as i32 - real_shake;
              }

              self.x = self.x.max(0);
              self.y = self.y.max(0);
              self.x = self.x.min(self.max_x);
              self.y = self.y.min(self.max_y);
          }
      }
      ```
- [ ] Run `cargo test -p render viewport` — expect PASS.
- [ ] **RED (frame):** create `rust/render/src/frame.rs`. Test the reduced draw on a synthetic
      `SimState` — but a full `SimState` is heavy to construct in a unit test; instead unit-test
      `frame::draw`'s observable contract with a **small hand-built** `SimState` only if
      practical; otherwise assert the per-piece behaviour here and defer the full-frame proof to the
      golden test (T6). Minimum RED: a synthetic `SimState` with a 2x2 level and two static worms
      draws terrain into each viewport rect and leaves the HUD gap as `pal32[0]`. If `SimState` has
      no cheap constructor, gate this behind a `#[test]` that builds it via `sim::state::SimState`'s
      public fields; if that is impractical, SKIP the frame unit test and rely on T6 (document the
      choice in the done-report). When feasible:
      ```rust
      // Illustrative shape (adapt to SimState's real constructor/fields):
      #[test]
      fn draw_fills_background_then_terrain_per_viewport() {
          // Build a SimState with level 2x2 material_id [10,11,12,13], cycles 0,
          // two worms at fixed positions; origpal ramp; empty color_anim.
          // After draw: bmp is 320x200; a pixel in the HUD gap (e.g. (159,0)) is
          // pal32[0]; a pixel inside viewport 0's rect shows a terrain material.
          // ...
      }
      ```
- [ ] **GREEN (frame):** implement the reduced `Game::Draw` (`game.cpp:170-198` +
      `viewport.cpp:196-210`), terrain-only:
      ```rust
      //! The 3a draw path: the terrain-only subset of `Game::Draw`
      //! (`game.cpp:170-198`) + the viewport world block (`viewport.cpp:196-210`).
      //! Build the per-frame palette, Fill(0) the whole surface, then for each
      //! viewport center it, clip to its rect, and DrawLevel at kOffs. Shadows,
      //! sprites, HUD, minimap are 3b/3e.

      use crate::bitmap::{Bitmap, ColorMode};
      use crate::level_draw::draw_level;
      use crate::palette::build_palette;
      use crate::viewport::Viewport;
      use assets::palette::Palette;
      use assets::tc::ColorAnim;
      use sim::state::SimState;

      pub fn draw(
          bmp: &mut Bitmap,
          state: &SimState,
          origpal: &Palette,
          color_anim: &[ColorAnim],
          viewports: &mut [Viewport],
          screen_flash: i32,
      ) {
          // 1. Per-frame palette, before any blit (game.cpp:171-183).
          let pal = build_palette(origpal, color_anim, state.cycles, screen_flash);
          // 2. Repaint the whole surface through the fresh LUT (game.cpp:189).
          bmp.fill(0, &pal);
          // 3. Per viewport: center, clip, DrawLevel at kOffs (viewport.cpp:196-210).
          let full_clip = bmp.clip;
          for vp in viewports.iter_mut() {
              let worm = &state.worms[vp.worm_idx];
              vp.process(worm, state.level.width, state.level.height);
              bmp.clip = vp.rect;
              bmp.cycles = state.cycles;
              // kOffs = rect.Ul() - (vp.x, vp.y): screen = world + kOffs.
              let (ulx, uly) = vp.rect.ul();
              draw_level(bmp, &state.level, &pal, ulx - vp.x, uly - vp.y, ColorMode::Classic);
              bmp.clip = full_clip;
          }
      }
      ```
- [ ] Run `cargo test -p render` (whole crate) — expect PASS. Run `cargo build -p render`.
- [ ] Reviewer (Opus): `draw_level` `CLIP_IMAGE` clamp matches `macros.hpp` exactly (source-index
      slide on negative top/left; width/height trim on bottom/right; early return); Classic arm =
      `pal[material_id]`; `frame::draw` order is build_palette → fill(0) → per-vp
      process/clip/cycles/draw_level → restore clip; `kOffs = ul - (x,y)`; the HUD gap and rows
      158-199 remain `pal32[0]`; viewport RNG never advanced; `steerable_count>0` arm is guarded.
- [ ] **Commit:**
      - `git add rust/render/src/level_draw.rs rust/render/src/viewport.rs rust/render/src/frame.rs rust/render/src/lib.rs`
      - `git commit -m "render(3a): DrawLevel Classic + Viewport process + reduced frame draw"`

---

### T4 — C++ dumper: opt-in `render player` directive + sidecar frame golden + re-diff gate  [Opus]

> **Note:** the CMake build is slow; this task may be done in a separate worktree/checkout to build
> `oracle_dump_sim_physics` (the Step-2 parallel-build lesson). Plan it as an ordinary task.

**Files**
- Modify: `src/tools/oracle_dump/sim_physics_dump.cpp`

**Interfaces**
- Produces: `oracle_dump_sim_physics <scenario.txt> <sim_out.txt> [seed] [frames_out.txt]` — when the
  scenario has a `render <layout>` directive, additionally writes a sidecar frame golden to
  `frames_out.txt` (argv[4]) with lines `<tick> <frame_hash_hex16> <state_hash_hex8>` and a final
  `total <n> <acc_hex16>`. When the directive is ABSENT, behaviour and the argv[2] sim output are
  byte-identical to before (no `Renderer`, no draw, no sidecar).

**Steps**

- [ ] **RED (re-diff gate baseline):** before editing, run the existing gen scripts for a couple of
      goldens (e.g. `gen_sim_slice4b_golden.sh`, `gen_sim_slice5d_golden.sh`) and confirm
      `git diff --stat rust/oracle-tests/golden/` is empty on the current tree (baseline). Record
      the current dumper builds and runs. (This proves the harness works before the change.)
- [ ] Extend the scenario grammar. In the `Scenario` struct add
      `std::string render_layout;` (empty = off). In `ParseScenario`, add a branch:
      ```cpp
      } else if (key == "render") {
        ls >> s.render_layout;
        if (s.render_layout != "player") {
          std::fprintf(stderr, "unknown render layout: %s\n", s.render_layout.c_str());
          std::exit(1);
        }
      ```
      Place it alongside the other `else if` keys (before the final unknown-key `else`).
- [ ] Add render includes near the existing includes:
      ```cpp
      #include "gfx/renderer.hpp"
      #include "gfx/blit.hpp"
      #include "viewport.hpp"
      ```
- [ ] In `main`, after opening the sim `out` file and before the tick loop, set up the renderer
      ONLY when `render_layout` is non-empty. The sidecar path is argv[4]:
      ```cpp
      // ---- Opt-in render (Slice 3a). Absent render_layout => nothing below runs and
      // the sim output stays byte-identical (the re-diff gate). ----
      std::unique_ptr<Renderer> renderer;
      std::vector<std::unique_ptr<Viewport>> viewports;
      std::FILE* frames = nullptr;
      uint64_t frames_acc = kFnvOffset;   // FNV offset, see below
      int frame_count = 0;
      if (!scn.render_layout.empty()) {
        if (argc < 5) {
          std::fprintf(stderr, "render scenario needs a 4th arg (frames sidecar path)\n");
          return 1;
        }
        frames = std::fopen(argv[4], "w");
        if (!frames) {
          std::fprintf(stderr, "cannot open %s\n", argv[4]);
          return 1;
        }
        renderer = std::make_unique<Renderer>();
        renderer->Init(320, 200);
        renderer->LoadPalette(*common);      // Origpal = common.exepal (Classic)
        // Two viewports, framehash layout (framehash_main.cpp:92-95).
        viewports.push_back(std::make_unique<Viewport>(Rect(0, 0, 158, 158), game.worms[0]->index));
        viewports.push_back(std::make_unique<Viewport>(Rect(160, 0, 158 + 160, 158), game.worms[1]->index));
      }
      ```
      Add the FNV helpers at file scope (copy from `framehash_main.cpp:26-34`):
      ```cpp
      constexpr uint64_t kFnvOffset = 1469598103934665603ULL;
      constexpr uint64_t kFnvPrime = 1099511628211ULL;
      uint64_t FnvByte(uint64_t h, uint8_t b) { return (h ^ b) * kFnvPrime; }
      uint8_t FadeChannel(uint8_t v, int amount) {
        return amount >= 32 ? v : static_cast<uint8_t>((v * amount) >> 5);
      }
      ```
- [ ] Add a terrain-only render + hash helper (reduced `Game::Draw`, NOT `Game::Draw` itself —
      matches the Rust `frame::draw`):
      ```cpp
      auto render_and_hash = [&](int tick) {
        // Palette build order (game.cpp:171-183): reset -> RotateFrom -> UpdatePal32.
        renderer->pal = renderer->Origpal();
        for (auto const& w : common->color_anim) {
          renderer->pal.RotateFrom(renderer->Origpal(), w.from, w.to, game.cycles >> 3);
        }
        // screen_flash == 0 in 3a -> no LightUp.
        renderer->UpdatePal32();
        Fill(renderer->bmp, 0);
        for (auto const& vp : viewports) {
          vp->Process(game);
          renderer->bmp.clip_rect = vp->rect;
          renderer->bmp.cycles = game.cycles;
          IVec2 const off = vp->rect.Ul() - IVec2(vp->x, vp->y);
          DrawLevel(renderer->bmp, game.level, off.x, off.y);
        }
        renderer->bmp.clip_rect = Rect(0, 0, renderer->bmp.w, renderer->bmp.h);
        int const fade = (tick == 0) ? 0 : 33;   // frame 0 black, then identity
        uint64_t h = kFnvOffset;
        for (int y = 0; y < renderer->bmp.h; ++y) {
          for (int x = 0; x < renderer->bmp.w; ++x) {
            uint32_t const c = renderer->bmp.GetPixel(x, y);
            h = FnvByte(h, FadeChannel((c >> 16) & 0xFF, fade));
            h = FnvByte(h, FadeChannel((c >> 8) & 0xFF, fade));
            h = FnvByte(h, FadeChannel(c & 0xFF, fade));
          }
        }
        frames_acc = (frames_acc ^ h) * kFnvPrime;
        ++frame_count;
        std::fprintf(frames, "%d %016" PRIx64 " %08x\n", tick, h, HashGameState(game));
      };
      ```
      (Add `#include <cinttypes>` for `PRIx64`. `IVec2` is available via `math/rect.hpp`; the
      dumper already includes `math.hpp`.)
- [ ] Call the helper right after each `dump(...)`: after `dump(0);` add
      `if (renderer) render_and_hash(0);`, and after `dump(t + 1);` inside the loop add
      `if (renderer) render_and_hash(t + 1);`. After the loop, before `std::fclose(out)`:
      ```cpp
      if (frames) {
        std::fprintf(frames, "total %d %016" PRIx64 "\n", frame_count, frames_acc);
        std::fclose(frames);
      }
      ```
- [ ] Update the file header comment: document the opt-in `render <layout>` directive, the sidecar
      4th-argv output, and that the render path is provably hash-inert on the SIM output (no
      `game.rand`/sim mutation) so absent-directive goldens stay byte-identical.
- [ ] **Re-diff gate (hard):** rebuild the dumper (`OPENLIERO_BUILD_ORACLE_DUMP=ON`), then run EVERY
      committed `gen_sim_slice*.sh` (and `gen_sim_physics_golden.sh` if present) to regenerate all
      `sim_slice*.txt`. Assert `git diff --stat rust/oracle-tests/golden/` is EMPTY — the `render`
      directive being absent changes nothing. If ANY sim golden moves, STOP: the render path is
      leaking into the ungated sim path — fix the gating before proceeding.
- [ ] Reviewer (Opus): the render block is fully gated on `render_layout`; the sim `out` write is
      untouched; `Fill`/`RotateFrom`/`UpdatePal32`/`DrawLevel` are the reduced draw (NOT
      `Game::Draw`, so no HUD/shadow/sprite/minimap); `Viewport::Process` draws no `game.rand`;
      frame-0 fade is 0; the sidecar format is `<tick> <hex16> <hex8>` + `total`; **the re-diff gate
      is empty** (paste the `git diff --stat` output as evidence).
- [ ] **Commit:**
      - `git add src/tools/oracle_dump/sim_physics_dump.cpp`
      - `git commit -m "oracle_dump: opt-in render player directive + sidecar frame golden (3a)"`

---

### T5 — scenario authoring + gen script + golden generation (RotateFrom observable)  [Opus]

**Files**
- Create: `rust/oracle-tests/golden/render_slice3a_scenario.txt`,
  `rust/oracle-tests/gen_render_slice3a_golden.sh`
- Generate (committed): `rust/oracle-tests/golden/render_slice3a_sim.txt`,
  `rust/oracle-tests/golden/render_slice3a.txt`

**Interfaces**
- Consumes: the T4 dumper (`render player` directive + sidecar).
- Produces: a scenario whose frame hash is CONSTANT within each 8-tick window and CHANGES on a
  `cycles>>3` boundary (ticks 8, 16, 24), plus the two goldens.

**Steps**

- [ ] Author `rust/oracle-tests/golden/render_slice3a_scenario.txt`. Reuse the Classic
      `Levels/physics_fall_test.lev`, 2 static worms (zero input), ≥24 ticks, and the new directive:
      ```
      # Step 3 Slice 3a — terrain-only player-layout frame-hash scenario.
      # Static worms (no input) so the ONLY per-frame change is palette RotateFrom.
      seed 42
      level Levels/physics_fall_test.lev
      ticks 24
      render player
      # worm <index> <pos_x> <pos_y> <health> <lives> <stats_x> <visible>
      # Positions chosen so each 158x158 viewport window contains a color_anim-cycled
      # palette index (tc.cfg colorAnim ranges: 129-131, 133-136, 152-159, 168-171).
      worm 0 <PX0> <PY0> 100 10 0   1
      worm 1 <PX1> <PY1> 100 10 218 1
      ```
- [ ] Create `rust/oracle-tests/gen_render_slice3a_golden.sh` mirroring `gen_sim_slice4b_golden.sh`,
      passing the sidecar as the 4th arg:
      ```bash
      #!/usr/bin/env bash
      # Regenerates the Slice-3a render goldens: builds the REAL C++ Game + a headless
      # Renderer, drives 2 static worms N ticks, and for each tick dumps (a) the normal
      # 11-column sim record (render_slice3a_sim.txt, the isolation source) and (b) a
      # sidecar frame golden (render_slice3a.txt: <tick> <frame_hash16> <state_hash8> +
      # total). Terrain-only draw (no HUD/shadow/sprite/minimap). LOCAL/MANUAL — needs
      # the full C++ build (links `game`); NOT in the lightweight rust.yml CI. Override
      # PRESET for other platforms.
      set -euo pipefail
      ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
      PRESET="${PRESET:-macos-arm64}"
      cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
      cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_sim_physics
      cd "$ROOT"
      "build/$PRESET/Release/oracle_dump_sim_physics" \
        "rust/oracle-tests/golden/render_slice3a_scenario.txt" \
        "rust/oracle-tests/golden/render_slice3a_sim.txt" \
        "" \
        "rust/oracle-tests/golden/render_slice3a.txt"
      echo "wrote rust/oracle-tests/golden/render_slice3a{_sim,}.txt"
      ```
      (Passing `""` as the 3rd arg keeps the seed from the scenario — the dumper's
      `argc >= 4 ? strtoul(argv[3]) : scn.seed` uses `strtoul("")==0`; if that is undesirable, pass
      the scenario seed `42` explicitly as the 3rd arg instead so the sidecar 4th-arg slot is
      unambiguous. **Prefer passing `42` explicitly.**) Make the script executable
      (`chmod +x rust/oracle-tests/gen_render_slice3a_golden.sh`).
- [ ] Generate the goldens: run `PRESET=macos-arm64 rust/oracle-tests/gen_render_slice3a_golden.sh`.
- [ ] **Prove RotateFrom is observable (done-when):** inspect `render_slice3a.txt`. The frame hash
      MUST be constant across ticks 1–7, CHANGE at tick 8, be constant 8–15, change at 16, and
      change at 24 (cycles>>3 boundaries). If the frame hash is constant across ALL ticks, the
      chosen worm windows contain NO cycled index — adjust `<PX*>`/`<PY*>` (scan the level for a
      region using an index in the colorAnim ranges; `physics_fall_test.lev` water/animated dirt is
      the likely source) and regenerate. **Fallback (spec):** if no static window of
      `physics_fall_test.lev` exhibits a cycled index, author a tiny cycling test level OR widen an
      animated range in a scenario-local level; document the choice. Do NOT proceed until the hash
      visibly moves on a boundary.
- [ ] Reviewer (Opus): the scenario is static (no `input` lines); ≥24 ticks; `render player`
      present; the generated `render_slice3a.txt` shows a hash change at tick 8 and 16 (paste the
      relevant lines); tick 0's frame hash is the all-black constant; the `total` line is present;
      `render_slice3a_sim.txt` is a normal 11-column sim golden.
- [ ] **Commit:**
      - `git add rust/oracle-tests/golden/render_slice3a_scenario.txt rust/oracle-tests/gen_render_slice3a_golden.sh rust/oracle-tests/golden/render_slice3a_sim.txt rust/oracle-tests/golden/render_slice3a.txt`
      - `git commit -m "render(3a): palette-cycling scenario + gen script + frame/sim goldens"`

---

### T6 — Rust frame-hash golden test — MILESTONE (first pixel-exact terrain frame + isolation)  [Opus]

**Files**
- Modify: `rust/oracle-tests/Cargo.toml` (add `render` dev-dependency)
- Create: `rust/oracle-tests/tests/render_slice3a_golden.rs`

**Interfaces**
- Consumes: `render::bitmap::Bitmap`, `render::viewport::Viewport`, `render::frame::draw`,
  `render::hash::{hash_frame, FNV_OFFSET, FNV_PRIME}`; `oracle_tests::scenario::Scenario`;
  `sim::hash::hash_game_state`; the Step-2 `SimState` setup pattern (see
  `tests/sim_slice6_gametag_golden.rs`); `assets::sprite::Tga` (Classic Origpal via `small.tga`'s
  embedded palette — the C++ `exepal` source, `common.cpp:376`); `assets::tc::TcConfig`
  (`color_anim`).
- Produces: a passing per-tick differential test over `render_slice3a.txt`.

**Steps**

- [ ] Add to `rust/oracle-tests/Cargo.toml` under `[dev-dependencies]`:
      `render = { path = "../render" }`.
- [ ] **RED:** create `rust/oracle-tests/tests/render_slice3a_golden.rs`. Reuse the Step-2 setup
      from `sim_slice6_gametag_golden.rs` (load level, tc, objects, worms, `SimState::new`, post-new
      consts) — copy that boilerplate. Origpal = the palette embedded in `sprites/small.tga`
      (`assets::sprite::Tga::load(...).palette`), which is the C++ `common.exepal`. Then:
      ```rust
      //! Per-tick FRAME-HASH differential test for Slice-3a: the first pixel-exact
      //! terrain frame. Drives `SimState` N ticks (Step-2 scenario runner), renders
      //! each tick with the `render` crate (terrain-only, two-viewport 320x200), and
      //! matches the C++ sidecar golden (`golden/render_slice3a.txt`) line-for-line
      //! INCLUDING the `total` line. Column 3 (`state_hash`) is asserted against the
      //! Rust `hash_game_state` (isolation: rendering did not perturb the sim) AND
      //! against the sim golden's master column (`render_slice3a_sim.txt`).
      //!
      //! Proves: (1) DrawLevel Classic + palette build are pixel-exact vs C++; (2) the
      //! frame hash VISIBLY CHANGES on a cycles>>3 boundary (RotateFrom observable);
      //! (3) the render path is a pure consumer (state hashes unchanged).

      // ... (Step-2 boilerplate: TC_ROOT, load level/tc/objects, build worms_init,
      //      SimState::new + post-new consts, exactly as sim_slice6_gametag_golden.rs) ...

      struct FrameLine { tick: u32, frame_hash: u64, state_hash: u32 }

      fn parse_frames(text: &str) -> (Vec<FrameLine>, u32, u64) {
          let mut lines = Vec::new();
          let mut total_n = 0u32;
          let mut total_acc = 0u64;
          for l in text.lines() {
              let t = l.trim();
              if t.is_empty() || t.starts_with('#') { continue; }
              let mut it = t.split_whitespace();
              let head = it.next().unwrap();
              if head == "total" {
                  total_n = it.next().unwrap().parse().unwrap();
                  total_acc = u64::from_str_radix(it.next().unwrap(), 16).unwrap();
              } else {
                  let tick: u32 = head.parse().unwrap();
                  let frame_hash = u64::from_str_radix(it.next().unwrap(), 16).unwrap();
                  let state_hash = u32::from_str_radix(it.next().unwrap(), 16).unwrap();
                  lines.push(FrameLine { tick, frame_hash, state_hash });
              }
          }
          (lines, total_n, total_acc)
      }

      #[test]
      fn render_slice3a_frame_hash_matches_cpp_oracle() {
          // --- load scenario + goldens ---
          let scenario_text = std::fs::read_to_string(concat!(
              env!("CARGO_MANIFEST_DIR"), "/golden/render_slice3a_scenario.txt")).unwrap();
          let scenario = oracle_tests::scenario::Scenario::parse(&scenario_text).unwrap();
          let frames_text = std::fs::read_to_string(concat!(
              env!("CARGO_MANIFEST_DIR"), "/golden/render_slice3a.txt")).unwrap();
          let (frames, total_n, total_acc) = parse_frames(&frames_text);
          assert_eq!(frames.len(), (scenario.ticks + 1) as usize, "one line per tick 0..=ticks");

          // --- isolation source: the sim golden's master column ---
          let sim_text = std::fs::read_to_string(concat!(
              env!("CARGO_MANIFEST_DIR"), "/golden/render_slice3a_sim.txt")).unwrap();
          let sim_master: Vec<u32> = sim_text.lines()
              .filter(|l| { let t = l.trim(); !t.is_empty() && !t.starts_with('#') })
              .map(|l| u32::from_str_radix(l.split_whitespace().nth(1).unwrap(), 16).unwrap())
              .collect();

          // --- Origpal = small.tga's embedded palette (C++ common.exepal) ---
          let small_bytes = std::fs::read(format!("{TC_ROOT}/sprites/small.tga")).unwrap();
          let small_tga = assets::sprite::Tga::load(&small_bytes).unwrap();
          let origpal = small_tga.palette.clone();
          let color_anim = tc.color_anim.clone(); // from the loaded TcConfig

          // --- SimState (Step-2 setup, copied from sim_slice6_gametag_golden.rs) ---
          // let mut state = SimState::new(...); post-new consts; game_mode 0 (KillEmAll).

          // --- render harness ---
          let mut bmp = render::bitmap::Bitmap::new(320, 200);
          let mut viewports = render::viewport::Viewport::player_layout();

          let render_tick = |bmp: &mut render::bitmap::Bitmap,
                             vps: &mut [render::viewport::Viewport],
                             state: &sim::state::SimState, tick: u32| -> u64 {
              render::frame::draw(bmp, state, &origpal, &color_anim, vps, 0);
              let fade = if tick == 0 { 0 } else { 33 };
              render::hash::hash_frame(bmp, fade)
          };

          let mut acc = render::hash::FNV_OFFSET;
          let mut frame_hashes: Vec<u64> = Vec::new();

          // tick 0
          assert_eq!(frames[0].tick, 0);
          let fh0 = render_tick(&mut bmp, &mut viewports, &state, 0);
          assert_eq!(fh0, frames[0].frame_hash, "tick 0 frame hash");
          assert_eq!(sim::hash::hash_game_state(&state), frames[0].state_hash, "tick 0 state hash");
          assert_eq!(frames[0].state_hash, sim_master[0], "tick 0 isolation vs sim golden");
          acc = (acc ^ fh0).wrapping_mul(render::hash::FNV_PRIME);
          frame_hashes.push(fh0);

          for k in 1..=scenario.ticks {
              let inputs = [
                  sim::state::ControlState::unpack(scenario.input(k - 1, 0)),
                  sim::state::ControlState::unpack(scenario.input(k - 1, 1)),
              ];
              state.process_frame(&inputs);
              let fh = render_tick(&mut bmp, &mut viewports, &state, k);
              let g = &frames[k as usize];
              assert_eq!(g.tick, k);
              assert_eq!(fh, g.frame_hash, "tick {k}: frame hash");
              assert_eq!(sim::hash::hash_game_state(&state), g.state_hash, "tick {k}: state hash");
              assert_eq!(g.state_hash, sim_master[k as usize], "tick {k}: isolation vs sim golden");
              acc = (acc ^ fh).wrapping_mul(render::hash::FNV_PRIME);
              frame_hashes.push(fh);
          }

          // total line
          assert_eq!(total_n, scenario.ticks + 1, "total frame count");
          assert_eq!(acc, total_acc, "total accumulator matches C++");

          // --- RotateFrom observability (non-vacuous, from the DRIVEN render) ---
          // Constant within an 8-tick window, changes on the cycles>>3 boundary.
          assert_eq!(frame_hashes[1], frame_hashes[7], "constant within [1,7] (dist 0)");
          assert_ne!(frame_hashes[7], frame_hashes[8], "hash CHANGES at cycles>>3 boundary (tick 8)");
          assert_eq!(frame_hashes[8], frame_hashes[9], "constant within [8,15]");
          assert_ne!(frame_hashes[8], frame_hashes[16], "hash changes again at tick 16");
      }
      ```
- [ ] Fill in the Step-2 `SimState` boilerplate by copying it verbatim from
      `tests/sim_slice6_gametag_golden.rs` (the `load_large_sprites`/`load_small_sprites` helpers,
      the `TC_ROOT`, the `objects`/`weapons`/`worms_init`/`SimState::new` + post-new consts block).
      Leave `state.game_mode = 0` (KillEmAll — the scenario has no `game_mode` directive).
- [ ] Run `cargo test -p oracle-tests --test render_slice3a_golden` — expect FAIL first if any wire
      is wrong, then PASS once the boilerplate + goldens line up. Then `cargo test --workspace` —
      all green.
- [ ] Reviewer (Opus): expected values come from the golden files, actual from the DRIVEN
      `SimState` + `render` crate (honesty); the isolation column is checked BOTH against Rust
      `hash_game_state` AND the sim golden's master column; the RotateFrom-observability asserts are
      non-vacuous (real hash inequality on the boundary); the `total` accumulator matches; frame 0
      uses `fade=0`. "Could this pass while the renderer is wrong?" — no: the frame-hash column is
      the hard gate and the boundary asserts prove cycling is live.
- [ ] **MILESTONE.** First pixel-exact terrain frame proven vs C++; RotateFrom observable;
      isolation proven.
- [ ] **Commit:**
      - `git add rust/oracle-tests/Cargo.toml rust/oracle-tests/tests/render_slice3a_golden.rs`
      - `git commit -m "render(3a): Rust frame-hash golden test — pixel-exact terrain + isolation"`

---

### T7 — PROGRESS + overview slice-3a line + slice close  [Opus]

**Files**
- Modify: `docs/superpowers/liero-rs-PROGRESS.md`; the overview's slice-3a line
  (`docs/superpowers/specs/2026-07-10-liero-rs-step3-rendering-overview.md`, the 3a bullet).

**Steps**

- [ ] `cargo test --workspace` green (render unit tests + `render_slice3a_golden` + all priors).
- [ ] Confirm `render` is Bevy-free: `cargo tree -p render` shows only `sim-core`/`assets`/`sim`;
      `grep -rn "bevy" rust/render/` is empty.
- [ ] Confirm the **re-diff ledger** from T4: every existing `sim_slice*.txt` regenerated
      byte-identically (paste the empty `git diff --stat`); the two new render goldens are the only
      additions.
- [ ] Update `docs/superpowers/liero-rs-PROGRESS.md` (use the real `currentDate` 2026-07-10 in any
      Status/date field): Step 3 slice 3a DONE — `render` crate stood up (Bitmap/Rect/ColorMode,
      per-frame palette build with RotateFrom/LightUp/pal32 pack, FNV-1a frame hash, DrawLevel
      Classic, two-viewport player layout with Viewport::process, reduced frame::draw); C++ dumper
      gained the opt-in `render player` directive + sidecar frame golden (re-diff gate clean); the
      Rust `render_slice3a_golden` test matches C++ tick-for-tick with RotateFrom observable on a
      cycles>>3 boundary and the joint state_hash proving isolation. Note the deferrals carried:
      shadow/sprite/HUD/minimap → 3b/3e; Modern arm unimplemented; steerable centering → 3b;
      LightUp/shake/laser RNG present-but-inert.
- [ ] Update the overview's 3a bullet to mark it landed (companion spec implemented).
- [ ] Reviewer (Opus): docs match reality; deferrals honestly listed; the re-diff ledger is real.
- [ ] **Commit:**
      - `git add docs/superpowers/liero-rs-PROGRESS.md docs/superpowers/specs/2026-07-10-liero-rs-step3-rendering-overview.md`
      - `git commit -m "docs(3a): PROGRESS + overview slice-3a landed; deferral ledger"`

## Done-report (each task)

(a) what changed + why, (b) files touched, (c) tests/risks. Per-task commit, local, on branch
`liero-rs-step-3`. **Do not push, do not open a PR** — the controller owns push + PR. Surface in the
final report: the re-diff-gate evidence (T4), the RotateFrom-boundary lines from `render_slice3a.txt`
(T5/T6), and any scenario adjustment needed to make cycling observable.
