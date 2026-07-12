# Step 3, Slice 3e — HUD / font / bars / minimap (the full player view): Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax. **Test-first**: write the failing test (or its assertion) before the
> implementation it pins, run it and SEE it fail, then make it pass.

**Goal:** Complete the **full player view** so the whole `Viewport::Draw` surface — the world block
(3a/3b, already pixel-exact) **plus** the HUD bars, the kills/lives/reloading **font** text, and
the 52×36 **minimap** — is **pixel-exact vs C++**, pixel-gated by per-scenario FNV-1a frame-hash
goldens. 3e ports the HUD half of `Viewport::Draw` (`viewport.cpp:78-189` pre-block + `:593-635`
minimap), the **font pipeline** (`font.tga` load + `Font::DrawChar/DrawString`, `common.cpp:405-434`,
`font.cpp:8-80`), and `DrawBar` (`blit.cpp:105-113`); extends the C++ dumper to draw the HUD behind
a new opt-in `render_hud` directive; and adds the `render_slice3e_*` HUD-including goldens. **This
is the last pixel-gated render slice before wasm (3f).** Companion spec:
`specs/2026-07-12-liero-rs-step3-slice3e-hud-design.md` (cited as **spec §N**).

**Architecture:** Additive within the existing `render` crate (deps `sim-core`, `assets`, `sim`;
**no Bevy**). New modules — `font.rs` (glyph bank + `DrawChar`/`DrawString`), `hud.rs` (bar/text
block + minimap) — plus `DrawBar` in `blit.rs`, a widened `frame::Scene`/`draw`, and a `Font` +
label strings threaded through `scenario::load`. Draw math ported **verbatim** from C++ (each
formula carries a `file:line` comment); structure idiomatic Rust. Rendering stays a **pure consumer**
of `SimState` (the HUD reads `worm.health/kills/lives/weapons/killed_timer/visible`, `state.level`,
`worm.stats_x` — all existing, all hash-silent for HUD purposes). The C++ dumper's world-only
`render_and_hash` is extended to *also* draw the HUD pre-block + minimap behind `render_hud`;
`oracle-tests` gains `render_slice3e_*` frame-hash goldens beside the sim goldens.

**Tech stack:** Rust (`render` new modules, `scenario` loader, `oracle-tests` tests + fixtures).
Frame/sim goldens generated LOCALLY/MANUALLY via the rebuilt C++ dumper
(`OPENLIERO_BUILD_ORACLE_DUMP`, `PRESET` default `macos-arm64`); CI (`cargo test --workspace
--exclude game`) runs the committed goldens. `data/TC/openliero` real TC. The C++ dumper task (T6)
may be built in a **separate worktree** because CMake is slow — but it is planned as an ordinary
task here.

## Global constraints

*(inherit every 3a–3d constraint; the 3e-specific ones follow)*

- **`render` crate is edition 2021, NO Bevy dependency.** Deps stay exactly `sim-core`, `assets`,
  `sim` (all path). `cargo tree -p render` must show no `bevy*`; `grep -rn "bevy" rust/render/`
  empty. The new modules add no dependency.
- **Verbatim math, idiomatic API.** Every ported primitive reproduces the C++ arithmetic
  bit-for-bit; cite the C++ `file:line` at each ported formula. Primary sources of truth for 3e:
  `src/game/gfx/font.cpp:8-80` (`DrawChar`/`DrawString`), `src/game/common.cpp:405-434` (font.tga
  post-process), `src/game/gfx/font.hpp:11-14` (`Font::Char`), `src/game/gfx/blit.cpp:20-37,105-113`
  (`FillRect`/`DrawBar`), `src/game/viewport.cpp:84-189` (HUD pre-block) + `:593-635` (minimap),
  `src/game/level.cpp:489-507` (`DrawMiniature`), `src/game/worm.hpp:203` (`MinimapColor`),
  `src/game/level.hpp:30-31` (`kHudMinimapW/H = 52/36`), `src/game/macros.hpp:3-22` (`CLIP_IMAGE`).
- **HUD is DECOUPLED from the world block via a new opt-in directive `render_hud`** (spec §3).
  The world-only draw path is untouched when `render_hud` is absent — that is the re-diff proof.
  The Rust `frame::draw` takes a matching `draw_hud: bool` (in `Scene`). Same opt-in discipline as
  3a's `render` and 3b's `render_shadow`.
- **`frame::draw` ↔ dumper `render_and_hash` symmetry.** The Rust HUD/minimap block and the C++
  dumper's inlined HUD/minimap block must be **1:1** — same draw order (HUD pre-block into the
  full-surface clip **before** the per-viewport world clip; minimap into the full-surface clip
  **after** the world block), same per-viewport worm, same `stats_x`/`kMapX`/`kMapY`/step math,
  same `is_replay=false` / `game_mode==KillEmAll` arms. Whenever an element is added on one side,
  add it on the other in the same commit (the T6 lesson: both sides move together).
- **New scenario token `render_hud` needs BOTH parser arms** (Rust `scenario`/`oracle-tests`
  parser **and** C++ `ParseScenario` in `sim_physics_dump.cpp`), added in the same commit (T6).
- **HUD reads are hash-silent / sim-neutral.** The HUD reads only existing `WormState`/`LevelSim`
  fields; no new sim field is added. The dumper's `render_hud` flips `settings->map=true` /
  `is_replay=false` for the **draw window only** (after the tick's sim `Process`, restored before
  the next) — neither reaches a sim mutation, so every `sim_slice*.txt` regenerates byte-identical.
- **Re-diff gates (hard, TWO of them).**
  1. **Sim re-diff:** every committed `sim_slice*.txt` regenerates byte-identical (`render_hud`
     absent in sim scenarios; the draw-time `map`/`is_replay` flips never reach a sim mutation).
     `git diff --stat` MUST be empty.
  2. **Render re-diff (`render_slice3a.txt` + `render_slice3b_*.txt`):** the HUD-extended dumper
     regenerates every prior render golden **byte-identical** (those scenarios lack `render_hud` ⇒
     the world-only draw is unchanged). This is a **done-when assertion, NOT a regen** — a stray
     pixel is a real pass-gating bug, caught immediately.
- **Isolation as a standing gate.** Each `render_slice3e_*` golden carries the Step-2 `state_hash`
  column; the Rust test asserts it equals both `hash_game_state` AND the pre-existing sim golden's
  master column. Rendering never perturbs the sim.
- **Non-vacuity is mandatory (spec §4).** Every reachable HUD element ships a control render where
  it is suppressed and the frame hash provably MOVES (à la 3b shadow/laser). A HUD golden that
  passes while the element paints nothing is a failed proof.
- **Classic-only.** Font/bars/minimap are Classic (palette-indexed); no Modern arm.
- **ASCII-only font decode (spec §7 Q2).** Port an ASCII decode path + a proof-test that the
  reachable labels (`Kills`/`Lives`/`Reloading`) + digits are `< 0x80`. Full CP437 table deferred.
- **Deferrals keep tripwires** (spec §6): `DrawTextSmall`/`text_sprites`, Holdazone HUD,
  GameOfTag/replay HUD, spawn-preview, name-labels, AI-debug — each guarded by
  `debug_assert!`/`unreachable!` at its would-be call site, never silently skipped.
- **No AI / "Generated with" taglines; no commit trailers** (no `Co-Authored-By`, no
  `Claude-Session`). **Commit on branch `liero-rs-step-3`** (the accumulating Step-3 PR). **Do NOT
  push and do NOT open a PR** — the controller owns push + PR. **fmt only new files** (`cargo fmt`
  drift on untouched files is out of scope). **No sub-subagents.** **Bash discipline:** one command
  per call; no `>>`/heredoc/`&&`/`;`/`$VAR`; no `cd`+`git`; create files with the editor.

## File structure

- `rust/render/src/font.rs` — `Font { chars: Vec<Char{ data:[u8;56], width:i32 }> }` +
  `Font::load(font_tga_bytes) -> Font` (the `common.cpp:405-434` post-process) + `Font::draw_char`
  + `Font::draw_string` + the ASCII decode helper.
- `rust/render/src/hud.rs` — `draw_hud(bmp, pal, state, vp, font, labels, render_res_y, multiplier)`
  (the `viewport.cpp:84-189` pre-block, KillEmAll/Scales arms) + `draw_minimap(bmp, pal, state,
  center_x, render_res_y)` (the `viewport.cpp:593-613` minimap + worm dots, `level.cpp:489`).
- `rust/render/src/blit.rs` — add `draw_bar` (`blit.cpp:105-113`); `fill_rect` only if reached.
- `rust/render/src/frame.rs` — `Scene` gains `font: &Font`, `labels: HudLabels`, `draw_hud: bool`,
  `map: bool`; `draw` calls `draw_hud` per viewport (before world clip) + `draw_minimap` (after).
- `rust/render/src/lib.rs` — add `pub mod font; pub mod hud;`.
- `rust/scenario/src/loader.rs` — read `font.tga` bytes → `Font`; carry the `Texts` labels
  (`Kills`/`Lives`/`Reloading`) into `Loaded.scene`.
- `src/tools/oracle_dump/sim_physics_dump.cpp` — add the `render_hud` directive + the HUD pre-block
  + minimap draw (behind it); set `settings->map`/`is_replay` for the draw window.
- `rust/scenario/src/parser.rs` — parse `render_hud` (the Rust scenario parser lives HERE since 3c;
  `rust/oracle-tests/src/` only has a stub lib.rs. BOTH parser arms — Rust + C++ `ParseScenario` —
  move together).
- `rust/oracle-tests/golden/render_slice3e_*_scenario.txt` — new scenarios (T7).
- `rust/oracle-tests/golden/render_slice3e_*.txt` + `_sim.txt` — generated goldens (T7).
- `rust/oracle-tests/gen_render_slice3e_*.sh` — gen scripts (T7).
- `rust/oracle-tests/tests/render_slice3e_common/mod.rs` + `render_slice3e_*_golden.rs` (T8).
- `docs/superpowers/liero-rs-PROGRESS.md` + the overview's slice-3e line — updated in T9.

## Tasks

### T0 — font pipeline: `render::font::Font` + `font.tga` loader + labels threaded  [Opus]

**Files**
- Create: `rust/render/src/font.rs`
- Modify: `rust/render/src/lib.rs` (`pub mod font;`), `rust/scenario/src/loader.rs` (read
  `font.tga` → `Font`; carry `Kills`/`Lives`/`Reloading` labels into `Loaded.scene`)

**Interfaces**
- Consumes: `assets::sprite::Tga::load` (to read the raw 7×(250*8) buffer — the same header
  loader; NB `font.tga` is a plain uncompressed indexed TGA, so `Tga::load` parses it, then 3e
  post-processes the pixels), `assets::tc::Texts { Reloading, Kills, Lives }` (`tc.rs:239-242`).
- Produces:
  - `render::font::Font { chars: Vec<Char> }`, `Char { data: [u8; 7*8], width: i32 }`.
  - `render::font::Font::load(tga: &assets::sprite::Tga) -> Font` (250 chars; the
    `common.cpp:414-433` per-char scan: `p==0→0`, `p==50→8`, else `width=x; break`).
  - `scenario::Loaded.scene` gains `font: Font` + `labels: { kills: String, lives: String,
    reloading: String }` (or a small `HudLabels` struct).

**Why (teaching note):** `font.tga` is **not** a `SpriteSet` — C++ reads a raw 7-wide ×
`chars*8`-tall index buffer and post-processes each 7×8 cell into a glyph whose "on" pixels are
palette index **8** and whose advance `width` is auto-detected by the first pixel that is neither 0
nor 50 (a sentinel column). The glyph transform is a pure asset step (no sim, no RNG), but the
pixel gate hashes the drawn glyphs, so the `0/50→0/8` remap and the width detection must be
**bit-exact**. This is why `Font` lives in `render` (render-only) while the raw TGA read reuses the
generic `assets` loader.

**Steps**

- [ ] Add `pub mod font;` to `render/lib.rs`.
- [ ] Read `rust/assets/src/sprite.rs` — confirm `Tga::load` accepts a 7×2000 indexed TGA (it is
      generic over dims; `font.tga` = 7 wide × 250*8=2000 tall). Confirm the pixel de-flip
      (bottom-to-top) matches what `common.cpp` feeds `ReadSpriteTga` (it does — same loader).
- [ ] **RED:** create `rust/render/src/font.rs` with `Font`/`Char` + `Font::load` (body
      `unimplemented!()`) and a test that loads the REAL `data/TC/openliero/sprites/font.tga` and
      asserts: (a) `chars.len() == 250`; (b) a known glyph (e.g. `'K'` at CP437 byte 75, so
      `chars[75-2]`) has `width > 0`; (c) its `data` contains only 0 and 8. Run
      `cargo test -p render font` → FAIL.
      ```rust
      // font.tga: 7 wide, 250 chars * 8 rows. Each glyph: p==0 stays 0 (hole),
      // p==50 -> 8 (the single "on" index the renderer resolves via pal32[color]),
      // any other value marks the advance width (x) and ends the row scan.
      ```
- [ ] **GREEN:** implement `Font::load` porting `common.cpp:414-433` verbatim (the per-char scan
      over the de-flipped 7×8 cell; `ch.width = 0` default; the `break` on first non-{0,50} pixel).
      Cite the C++ lines.
- [ ] **Thread into `scenario::load`:** read `sprites/font.tga` bytes → `Tga::load` → `Font::load`;
      carry it + the three `Texts` labels into `Loaded.scene`. (The `Texts` already parse in
      `assets::tc`; no new asset parsing.) Add a loader unit test that the real TC produces a
      non-empty `Font` and the expected label strings (`"Kills: "`, `"Lives: "`, `"Reloading..."`).
- [ ] Run `cargo test -p render font` + `cargo test -p scenario` — PASS. `cargo build -p render`.
- [ ] Reviewer (Opus): the `0/50→0/8` remap + width-detection match `common.cpp:414-433`
      byte-for-byte; `chars.len()==250`; the raw TGA read reuses `assets` (no duplicate TGA parser);
      labels come from `Texts` verbatim; `render` stays Bevy-free.
- [ ] **Commit:**
      - `git add rust/render/src/font.rs rust/render/src/lib.rs rust/scenario/src/loader.rs`
      - `git commit -m "render(3e): Font + font.tga loader (0/50->0/8, width-detect) + labels"`

---

### T1 — `Font::draw_char` / `draw_string` + ASCII decode + label proof-test  [Opus]

**Files**
- Modify: `rust/render/src/font.rs`

**Interfaces**
- Consumes: `render::bitmap::{Bitmap, Pal32, Rect}`; `render::font::Font`.
- Produces:
  - `Font::draw_char(&self, scr: &mut Bitmap, pal: &Pal32, c: u8, x: i32, y: i32, color: i32,
    size: i32)` (`font.cpp:8-43`).
  - `Font::draw_string(&self, scr: &mut Bitmap, pal: &Pal32, s: &str, x: i32, y: i32, color: i32,
    size: i32)` (`font.cpp:58-80`), ASCII decode path.
  - a private `ascii_to_font_byte(cp: char) -> u8` (identity for printable ASCII; `1` = skip).

**Why (teaching note):** `DrawString` decodes each codepoint → CP437 byte, skips `<2 || >=252`,
then `c -= 2` and calls `DrawChar(c)`, which re-checks `c>=2 && c<252` on the **already-decremented**
`c` (the `font.cpp:9` "TODO is this correct" quirk — port it verbatim, do not "fix" it). Advance is
`x += chars[c].width * size`; a newline (cp==0 in the C++ decode) resets x and steps y by `8*size`.
Index-0 glyph pixels are the transparent hole; "on" pixels (index 8) resolve to `pal32[color]` —
the glyph value is discarded, the caller's `color` wins. For the shipped TC the HUD strings are
pure ASCII, so `ascii_to_font_byte` is the identity on `0x20..0x7f` and the CP437 table is not
needed (spec §2/§7 Q2).

**Steps**

- [ ] **RED (draw_char):** add a test — a synthetic `Font` with one glyph (e.g. `chars[k]` a 2px-wide
      "L" of index-8 pixels over index-0 holes) drawn at `(x,y)` with `color=10`: index-8 pixels →
      `pal[10]`, holes stay the screen sentinel; `size=1`; then a clipped draw (glyph straddling the
      clip edge) draws only inside cells (`CLIP_IMAGE`). Bodies `unimplemented!()`, run
      `cargo test -p render font` → FAIL.
- [ ] **GREEN (draw_char):** port `font.cpp:8-43` verbatim — the `c>=2 && c<252` guard, the 7×8
      cell, `size`-nested inner loops, `CLIP_IMAGE` clip, `if (kC) *rowdest = pal[color]`. Reuse the
      existing `clip_image` helper (`blit.rs`, from 3b) for the clip if the addressing matches;
      otherwise inline the `CLIP_IMAGE` math and cite `macros.hpp:3-22`.
- [ ] **RED (draw_string + ASCII):** a test drawing `"Kills: 0"` at `(0,0)` color 10 asserts the
      total advance == `sum(chars[byte-2].width)` and that a couple of glyph origins land where the
      running `x` predicts. Add the **label proof-test**: `assert!("Kills: Lives: Reloading..."
      .bytes().all(|b| b < 0x80))` (the reachable labels + digits are ASCII, so the ASCII decode is
      bit-exact). FAIL first.
- [ ] **GREEN (draw_string):** port `font.cpp:58-80` — ASCII decode (identity for `0x20..0x7f`,
      `1`=skip for the rest), the `c>=2 && c<252` gate, `c-=2`, `draw_char`, advance, newline reset.
- [ ] Run `cargo test -p render font` — PASS. `cargo build -p render`.
- [ ] Reviewer (Opus): the double `c>=2 && c<252` guard preserved (not "fixed"); advance uses
      `chars[c].width * size`; index-8→`pal[color]` (glyph value discarded); `CLIP_IMAGE` correct;
      the ASCII proof-test is a real gate (non-ASCII labels would fail it, forcing the full table).
- [ ] **Commit:**
      - `git add rust/render/src/font.rs`
      - `git commit -m "render(3e): Font::draw_char + draw_string (ASCII decode, index-8 glyphs)"`

---

### T2 — `draw_bar` (+ `fill_rect` if reached)  [Opus]

**Files**
- Modify: `rust/render/src/blit.rs`

**Interfaces**
- Consumes: `render::bitmap::{Bitmap, Pal32}`.
- Produces: `render::blit::draw_bar(scr, pal, x, y, width, height, color)` (`blit.cpp:105-113`);
  `render::blit::fill_rect(scr, pal, x, y, w, h, color)` (`blit.cpp:20-37`) **only if** a reachable
  HUD element needs it (the reachable bars use `draw_bar`; `fill_rect`'s only reachable-adjacent use
  is the deferred replay color box → defer `fill_rect` unless T3 shows a reached use).

**Why (teaching note):** `DrawBar` (`blit.cpp:105-113`) is an **unclipped** horizontal fill:
`height` rows of `width` pixels of `pal32[color]` starting at `(x,y)`, guarded only by `width > 0`
(no `clip_rect` clamp — the HUD is drawn into the full-surface clip and the caller's coordinates are
trusted). `FillRect` (`blit.cpp:20-37`) **is** clip-clamped. Match each exactly: a HUD life bar
that clamps where C++ does not (or vice-versa) shifts the bar pixels.

**Steps**

- [ ] **RED:** a test — `draw_bar` at `(x,y)` writes `height` rows × `width` cols of `pal[color]`;
      `width<=0` writes nothing; **no** clip clamp (unlike `fill_rect`). FAIL first.
- [ ] **GREEN:** port `blit.cpp:105-113` (`draw_bar`) verbatim. Port `fill_rect` (`blit.cpp:20-37`,
      clip-clamped) **only if** T3 needs it; otherwise leave a `// deferred: FillRect (replay color
      box only)` note.
- [ ] Run `cargo test -p render blit` — PASS.
- [ ] Reviewer (Opus): `draw_bar` unclipped + `width>0` guard; `fill_rect` (if added) clip-clamped
      per `blit.cpp:25-28`; `pal` an explicit arg (no `pal32` on `Bitmap`, matching 3b convention).
- [ ] **Commit:**
      - `git add rust/render/src/blit.rs`
      - `git commit -m "render(3e): draw_bar (unclipped HUD bar fill)"`

---

### T3 — HUD bar/text block (`render::hud::draw_hud`)  [Opus]

**Files**
- Create: `rust/render/src/hud.rs`
- Modify: `rust/render/src/lib.rs` (`pub mod hud;`)

**Interfaces**
- Consumes: `render::{bitmap::*, blit::draw_bar, font::Font}`; `sim::state::{SimState, WormState,
  WormWeapon}`; `assets::object::Weapon`; the label strings.
- Produces: `render::hud::draw_hud(scr: &mut Bitmap, pal: &Pal32, state: &SimState, worm_idx: usize,
  font: &Font, labels: &HudLabels, render_res_y: i32, multiplier: i32)` — the `viewport.cpp:84-189`
  pre-block for one viewport's worm (KillEmAll/Scales arms only).

**Why (teaching note):** the HUD pre-block draws into the **full-surface clip** (before the world
clip is set), at `worm.stats_x * multiplier`. It is per-viewport-worm: viewport 0's worm at
`stats_x=0`, viewport 1's at `stats_x=218`. The life bar has **two arms** — visible
(`health*100/maxhealth`, `viewport.cpp:85-87`) and dying (`100-(killed_timer*25)/37` clamped,
`:88-95`). The ammo bar has two arms — available+ammo (`:101-109`) and reloading loading-bar +
blinking "Reloading" text (`:110-128`, gated `(cycles%20)>10 && visible`). Kills text is
**always** drawn (`:131-132`); Lives text is the KillEmAll/Scales arm (`:148-153`). Every color
constant (`w/10+234`, `w/10+245`, 50, 10, 6) is load-bearing. Holdazone/GameOfTag/replay arms are
**tripwired** (unreachable in our scenarios).

**Steps**

- [ ] Add `pub mod hud;` to `lib.rs`. Define `HudLabels { kills, lives, reloading: String }` (or
      pass `&str`s).
- [ ] **RED:** a unit test with a synthetic `SimState` (2 worms, `stats_x` 0/218, KillEmAll):
      a **visible** worm at health 60/max 100 → a life bar of width 60 at `(stats_x, render_res_y-39)`
      color `60/10+234=240`; kills text "Kills: N" at `(stats_x, render_res_y-29)` color 10; lives
      text at `(stats_x, render_res_y-22)` color 6. Assert a few bar pixels (`pal[240]`) and that the
      kills-text region is non-empty. Then a **dying** worm (`visible=false`, `killed_timer=37`) →
      the countdown life-bar arm. FAIL first.
- [ ] **GREEN:** create `hud.rs`, port `viewport.cpp:84-153` verbatim:
      life bar (both arms), ammo/loading bar (both arms) + reloading text, kills text, lives text
      (`case kGmKillEmAll`/`kGmScalesOfJustice`). Cite each `file:line`. **Tripwire** the
      Holdazone (`:155-169`), GameOfTag (`:171-185`), and replay (`:134-144`) arms with
      `debug_assert!`/a comment (`unreachable!` guarded on `game_mode`/`is_replay` — our scenarios
      never hit them). Use `to_string()` for `ToString(int)`.
- [ ] Confirm the weapon-availability reads (`ww.Available()`, `ww.ammo`, `ww.type->ammo`,
      `loading_left`, `type->ComputedLoadingTime`) map to existing `WormWeapon`/`Weapon` fields;
      if a field is missing, thread it as a render-read (no hash change) — but prefer reusing what
      the sim already carries (Slice 4d added reload state). Cite the C++ formula at each.
- [ ] Run `cargo test -p render hud` — PASS. `cargo build -p render`.
- [ ] Reviewer (Opus): both life-bar arms + both ammo-bar arms; color constants exact; kills always
      drawn, lives KillEmAll-arm; reloading-text gate `(cycles%20)>10 && visible`; unreachable arms
      tripwired (not silently dropped); `stats_x * multiplier` positioning; drawn into the
      full-surface clip.
- [ ] **Commit:**
      - `git add rust/render/src/hud.rs rust/render/src/lib.rs`
      - `git commit -m "render(3e): HUD bar/text block (life/ammo/reloading/kills/lives)"`

---

### T4 — minimap: `DrawMiniature` + worm dots  [Opus]

**Files**
- Modify: `rust/render/src/hud.rs`

**Interfaces**
- Consumes: `render::{bitmap::*, level_draw}` (for `AppearanceAt`); `sim::state::{SimState,
  LevelSim, WormState}`; `assets` palette.
- Produces: `render::hud::draw_minimap(scr: &mut Bitmap, pal: &Pal32, state: &SimState,
  center_x: i32, render_res_y: i32)` — `viewport.cpp:593-613` (minimap terrain + worm dots),
  wrapping a `draw_miniature` port of `level.cpp:489-507`.

**Why (teaching note):** the minimap steps the level's material grid (`step_x/step_y` = `max(ceil(
dim / {52,36}), 1)`) sampling `AppearanceAt` into a 52×36 HUD block at `(kMapX,kMapY) =
(center_x-26, render_res_y-38)`, clip-gated by `dest.clip.Inside` (`level.cpp:499`). Worm dots are a
single `SetPixel` per visible worm at `(pos/step + map)` with `MinimapColor()=129+index*4`
(`worm.hpp:203`). **Both viewports draw the minimap at the same centered position** — the second
overwrites the first (spec §7 Q6); reproduce verbatim. The Holdazone minimap marker (`:615-634`) is
**tripwired**.

**Steps**

- [ ] **RED:** a unit test with a synthetic `LevelSim` (distinctive material at a known world cell)
      + 2 visible worms → assert the minimap block samples the expected `AppearanceAt` values at
      `(kMapX,kMapY)` and that each worm dot lands at `(pos/step + map)` with `pal[129+index*4]`.
      FAIL first.
- [ ] **GREEN:** port `level.cpp:489-507` (`draw_miniature`: the `step/2` start offset, the
      `kMapEndX/Y` bounds, the `kIdx < material_id.size() && clip.Inside` gate, `AppearanceAt`) and
      `viewport.cpp:593-613` (`kMapX/kMapY`/step math, the worm-dot loop). Reuse the 3a
      `level_draw::appearance_at` (Classic). **Tripwire** the Holdazone marker (`:615-634`). Cite
      each line.
- [ ] Run `cargo test -p render hud` — PASS. `cargo build -p render`.
- [ ] Reviewer (Opus): `step = max(ceil(dim/{52,36}),1)`; `my/mx` start at `step/2`; the
      `Inside`-gate + `kIdx` bound match `level.cpp:499`; worm dot `MinimapColor=129+index*4`;
      Holdazone marker tripwired; the double-draw (both viewports) reproduced.
- [ ] **Commit:**
      - `git add rust/render/src/hud.rs`
      - `git commit -m "render(3e): minimap (DrawMiniature + worm dots)"`

---

### T5 — wire HUD/minimap into `frame::draw` (`Scene` widened; world hashes unchanged)  [Opus]

**Files**
- Modify: `rust/render/src/frame.rs`; the 3b harness call sites
  (`rust/oracle-tests/tests/render_slice3b_common/mod.rs`) + any `Scene` constructors
  (`rust/scenario/src/loader.rs`, `shot`, `game`) that must add the new `Scene` fields.

**Interfaces**
- Consumes: `render::{font::Font, hud::{draw_hud, draw_minimap}}`.
- Produces: `frame::Scene` gains `font: &'a Font`, `labels: &'a HudLabels`, `draw_hud: bool`,
  `map: bool`; `frame::draw` draws, per viewport, the HUD pre-block **before** the world clip and
  the minimap **after** the world block, gated on `draw_hud` (minimap additionally on `map`).

**Why (teaching note):** C++ `Viewport::Draw` does HUD-pre-block (full clip) → world (rect clip) →
minimap (full clip) **per viewport, inside one call**. The Rust `frame::draw` loops viewports; the
HUD pre-block and minimap slot into each iteration around the existing world block. Because
`draw_hud` defaults off for the world-only path, the 3a/3b frame hashes stay **byte-identical** —
that is the render re-diff proof, mirrored on the Rust side. The minimap draws after `bmp.clip` is
restored to `full_clip` (it is unclipped/full-surface).

**Steps**

- [ ] **RED:** in `frame.rs` tests, extend the existing `draw_fills_background_then_terrain_per_viewport`
      (or add a sibling) so that with `draw_hud=false` the frame is **identical** to today's
      world-only output (guard against accidental HUD bleed), and with `draw_hud=true` + a visible
      worm + a `map` level the HUD region + minimap region are non-empty. FAIL first (fields absent).
- [ ] **GREEN:** widen `Scene` + `draw`. Per viewport: if `draw_hud` → `draw_hud(bmp, &pal, state,
      vp.worm_idx, scene.font, scene.labels, render_res_y, multiplier)` **before** `bmp.clip =
      vp.rect`; the existing world block; then after `bmp.clip = full_clip`, if `draw_hud && map` →
      `draw_minimap(bmp, &pal, state, center_x, render_res_y)`. `render_res_y = bmp.h`,
      `multiplier = bmp.w/320`, `center_x = bmp.w/2` (render-map §7). Cite `viewport.cpp:78-635`.
- [ ] **Update all `Scene` constructors** — thread `font`/`labels`/`draw_hud=false`/`map=false` in
      the 3b harness, `scenario::load`-based callers, `shot`, and `game` so the workspace compiles
      and the **world-only frame hashes stay byte-identical**.
- [ ] Run `cargo test --workspace --exclude game` — every `render_slice3a`/`render_slice3b_*` golden
      still GREEN (world hashes unchanged with `draw_hud=false`). `cargo build -p game`.
- [ ] Reviewer (Opus): draw order (HUD pre-block full-clip → world rect-clip → minimap full-clip)
      matches `Viewport::Draw`; `draw_hud=false` yields byte-identical world frames (the 3a/3b
      goldens prove it); minimap gated on `map`; `render_res_y`/`multiplier`/`center_x` derived per
      render-map §7.
- [ ] **Commit:**
      - `git add rust/render/src/frame.rs rust/oracle-tests/tests/render_slice3b_common/mod.rs rust/scenario/src/loader.rs`
      - (add `rust/shot/` / `rust/game/` paths if their `Scene` constructors changed)
      - `git commit -m "render(3e): wire HUD + minimap into frame::draw (Scene.draw_hud/map)"`

---

### T6 — C++ dumper: `render_hud` directive + HUD/minimap draw + re-diff gate  [Opus]

**Files**
- Modify: `src/tools/oracle_dump/sim_physics_dump.cpp`;
  `rust/scenario/src/parser.rs` — the `render_hud` parser arm (the Rust scenario parser lives here
  since 3c; BOTH sides move together)

**Interfaces**
- Consumes: `Common::font`, `Common::DrawBar`, `Level::DrawMiniature`, the framehash layout
  (`stats_x` 0/218 — already scenario fields), `settings->map`, `is_replay`.
- Produces: an opt-in `render_hud` directive; `render_and_hash` draws the HUD pre-block + minimap
  behind it, into the full-surface clip, per viewport-worm — **1:1 with the Rust `frame::draw`**.

**Why (teaching note):** the dumper today renders the world block ONLY (`:69-74,453-805`; HUD/minimap
omitted, `:643-644`). 3e extends it. The HUD flip is sim-neutral: it happens after the tick's sim
`Process` and is restored before the next, so `settings->map`/`is_replay` never reach a sim
mutation and every `sim_slice*.txt` stays byte-identical. When `render_hud` is absent (all existing
scenarios), the added block is skipped ⇒ `render_slice3a.txt` + all `render_slice3b_*.txt`
regenerate byte-identical. **New scenario token ⇒ both parser arms in the same commit** (T6 lesson).

**Steps**

- [ ] Add the `render_hud` parser arm on **both** sides in this commit: the C++ `ParseScenario`
      (`sim_physics_dump.cpp`, beside `render_shadow`, `:233-235`) sets `scn.render_hud = true`; the
      Rust `scenario` parser adds a matching `hud()` accessor (beside `shadow()`). A parse round-trip
      test on each side.
- [ ] In `render_and_hash`, when `scn.render_hud`: save+set `game.settings->map = true` (restore
      after the draw); draw the **HUD pre-block per viewport-worm into the full-surface clip**
      (`renderer->bmp.clip_rect = Rect(0,0,w,h)`) **before** the per-viewport world clip is set —
      port `viewport.cpp:84-153` (KillEmAll/Scales arms; `is_replay=false` so `:134-144` skipped);
      then after the world block, draw the **minimap** (`viewport.cpp:593-613`, `DrawMiniature` +
      worm dots) into the full-surface clip, gated on `settings->map`. Mirror the Rust order EXACTLY.
      Use `common.font.DrawString` for text (the same `Font` the Rust `Font::draw_string` ports).
- [ ] **Re-diff gate (hard):** rebuild the dumper; regenerate a couple of existing render goldens
      **without** `render_hud` and confirm `git diff --stat` on `render_slice3a.txt` +
      `render_slice3b_*.txt` is EMPTY (byte-identical); regenerate a sim golden and confirm
      `sim_slice*.txt` byte-identical. Paste both empty diffs in the done-report. If ANY prior golden
      moves, STOP — the HUD block leaked into the world-only path.
- [ ] `clang-format --dry-run -Werror src/tools/oracle_dump/sim_physics_dump.cpp` (touched file).
- [ ] Reviewer (Opus): `render_hud` gated (absent ⇒ byte-identical priors); HUD pre-block into the
      full-surface clip BEFORE the world clip; minimap AFTER, gated on `map`; `is_replay=false`;
      `map`/`is_replay` flips are draw-window-only and sim-neutral (restored before next Process);
      both parser arms present; draw order 1:1 with `frame::draw`.
- [ ] **Commit:**
      - `git add src/tools/oracle_dump/sim_physics_dump.cpp rust/scenario/src/parser.rs`
      - `git commit -m "oracle(3e): render_hud directive + HUD/minimap draw (re-diff clean)"`

---

### T7 — `render_slice3e_*` scenarios + goldens + gen scripts  [Opus]

**Files**
- Create: `rust/oracle-tests/golden/render_slice3e_{hud,reload}[,death]_scenario.txt`,
  `render_slice3e_*.txt`, `render_slice3e_*_sim.txt`,
  `rust/oracle-tests/gen_render_slice3e_*.sh`

**Interfaces**
- Consumes: the rebuilt dumper (T6). Produces: per-scenario frame + sim goldens for the HUD view.

**Why (teaching note):** each HUD element must be **non-vacuous** (spec §4). The base `hud`
scenario reuses the `blood` inputs (visible worms, KillEmAll, `settings.map`) + `render` +
`render_hud`, proving life bar + kills + lives + minimap + worm dots. The `reload` scenario reuses
a firing scenario so the ammo/loading bar and reloading text are exercised (the bar width steps
across ticks). The optional `death` scenario reuses the 5d death window for the countdown life-bar
arm + banners.

**Steps**

- [ ] **`render_slice3e_hud`** — copy the `blood` `_scenario.txt` inputs; add `render player`,
      `render_hud`, and ensure both worms carry `stats_x` 0/218 and are visible; KillEmAll
      (`game_mode` default). Gen script mirrors `gen_render_slice3b_*.sh` (4th-arg sidecar; seed 42
      explicit). Generate locally (`PRESET=macos-arm64`).
- [ ] **`render_slice3e_reload`** — reuse a firing scenario (e.g. `dart`/`fan`/a handgun-like slot)
      where a worm fires and enters reload; add `render_hud`. Verify the generated golden's reload
      window has **non-constant** per-tick hashes (the ammo/loading bar width steps) — else the bar
      is unproven.
- [ ] **`render_slice3e_death` (optional, spec §7 Q3)** — reuse the 5d death→respawn window; add
      `render_hud`. Verify a banner-window tick differs from the same tick with HUD off. **If the
      T-count runs hot, SKIP this scenario** and record the deferral of the dying-life-bar arm +
      banners (T9), landing 3e on `hud`+`reload`.
- [ ] Generate all goldens locally. **Verify first WITHOUT regenerating priors** — the gen scripts
      only write the new `render_slice3e_*` files; no existing golden is touched (re-diff discipline).
- [ ] Reviewer (Opus): each scenario reaches its target HUD elements (visible worms; the reload
      window genuinely reloads per the reused sim golden); the reload hashes step; no prior golden
      regenerated (only additions); each `_sim.txt` is a normal 11-column sim golden.
- [ ] **Commit:**
      - `git add rust/oracle-tests/golden/render_slice3e_* rust/oracle-tests/gen_render_slice3e_*.sh`
      - `git commit -m "oracle(3e): HUD scenario corpus + gen scripts + frame/sim goldens"`

---

### T8 — Rust frame-hash golden tests — MILESTONE (full player view pixel-exact)  [Opus]

**Files**
- Create: `rust/oracle-tests/tests/render_slice3e_common/mod.rs`,
  `rust/oracle-tests/tests/render_slice3e_{hud,reload}[,death]_golden.rs`

**Interfaces**
- Consumes: `render::{bitmap::Bitmap, viewport::Viewport, frame::{draw, Scene}, font::Font,
  hud::*, hash::{hash_frame, FNV_OFFSET, FNV_PRIME}}`; `oracle_tests`/`scenario` parser;
  `sim::hash::hash_game_state`; `scenario::load` (now yielding `font` + `labels`).
- Produces: one passing per-tick differential test per `render_slice3e_*` scenario, matching the
  sidecar **line-for-line + `total`**, with the joint `state_hash` column, plus the non-vacuity
  controls.

**Steps**

- [ ] Factor `render_slice3e_common/mod.rs` off the 3b common harness (`render_slice3b_common`),
      adding `draw_hud=true` + `map=true` + the `font`/`labels` from `scenario::load`, and a
      `modified_frame_hash`-style control that renders a target tick with the HUD suppressed
      (`draw_hud=false`) or `map` off. Each test drives the sim tick-by-tick, renders the full player
      view, and asserts frame hash + `total` + the isolation triple (`state_hash` == `hash_game_state`
      == `_sim.txt` master).
- [ ] **HUD non-vacuity asserts** (`render_slice3e_hud_golden.rs`): (a) `draw_hud=true` hash !=
      the frozen `render_slice3b_blood` hash at the same tick (HUD paints); (b) `map=false` hash !=
      `map=true` hash (minimap+dots paint); (c) a worm-dot position differs across two ticks where
      the worm moved.
- [ ] **Reload non-vacuity assert** (`render_slice3e_reload_golden.rs`): two ticks in the reload
      window have **different** frame hashes (the ammo/loading bar width steps + reloading-text blink).
- [ ] (If shipped) **Death non-vacuity assert**: a banner-window tick != the same tick HUD-off.
- [ ] Run each `cargo test -p oracle-tests --test render_slice3e_<name>_golden` — FAIL if a wire is
      wrong, then PASS. Then `cargo test --workspace --exclude game` — all green (render unit + all
      3a/3b/3e goldens + every sim golden + `test_determinism`). `cargo build -p game`.
- [ ] Reviewer (Opus): expected from the golden files, actual from the DRIVEN `SimState` + `render`;
      isolation column checked both ways; the non-vacuity asserts are real inequalities (HUD-on vs
      frozen-3b, map on/off, reload-bar movement); `total` matches; frame 0 uses `fade=0`. "Could
      this pass while the HUD is wrong?" — no: the frame-hash column is the hard gate and the
      suppression controls prove each element paints.
- [ ] **MILESTONE.** The full in-game player frame (world + HUD bars/text + minimap) is pixel-exact
      vs C++; every reachable HUD element proven non-vacuous; isolation triple-proven.
- [ ] **Commit:**
      - `git add rust/oracle-tests/tests/render_slice3e_*`
      - `git commit -m "render(3e): Rust frame-hash goldens — full player view pixel-exact"`

---

### T9 — PROGRESS + overview slice-3e line + deferral ledger + broad slice review  [Opus]

**Files**
- Modify: `docs/superpowers/liero-rs-PROGRESS.md`; the overview's slice-3e bullet + deferrals/
  open-questions (`docs/superpowers/specs/2026-07-10-liero-rs-step3-rendering-overview.md`)

**Steps**

- [ ] `cargo test --workspace --exclude game` green (render unit + all `render_slice3e_*` +
      `render_slice3a`/`render_slice3b_*` + every prior sim golden + `test_determinism`);
      `cargo build -p game`. Confirm `render` Bevy-free (`cargo tree -p render`; `grep -rn "bevy"
      rust/render/` empty).
- [ ] Confirm the **re-diff ledger** (T6): both gates green — every `sim_slice*.txt` byte-identical
      AND every `render_slice3a.txt`/`render_slice3b_*.txt` byte-identical (paste the empty
      `git diff --stat`); the new `render_slice3e_*` files are the only additions.
- [ ] Update `docs/superpowers/liero-rs-PROGRESS.md` (use the real `currentDate`): Step 3 slice 3e
      DONE — the full player view (world + HUD bars/text + 52×36 minimap) is pixel-exact vs C++;
      `Font` (`font.tga` 0/50→0/8 + width-detect) + `Font::draw_char`/`draw_string` (ASCII decode) +
      `draw_bar` + the HUD block + minimap ported; the dumper draws the HUD behind `render_hud` with
      both re-diff gates clean; per-scenario `render_slice3e_*` goldens match C++ tick-for-tick with
      each HUD element proven non-vacuous and the joint `state_hash` proving isolation. Note the
      death-scenario decision (shipped vs deferred).
- [ ] Update the overview's 3e bullet to mark it landed (companion spec implemented). Mark 3f
      (wasm) as the sole remaining Step-3 slice. Record which spec open questions resolved how
      (`is_replay=false`, ASCII decode, death-scenario decision, `Font` in `render`, `draw_bar`-only,
      minimap double-draw).
- [ ] **Deferral ledger (explicit).** Record carried deferrals (each with its tripwire):
      `DrawTextSmall`/`text_sprites` (name-labels); Holdazone HUD (dashed box, timer text, minimap
      marker); GameOfTag/replay HUD; spawn-preview `BlitImageTrans` + "Press fire" (unless reached);
      full CP437 table; AI-debug (permanently out); Modern/spectator/menus (per overview); the
      dying-life-bar arm + banners IF the death scenario was deferred. A future scenario reaching any
      of these lifts the deferral with a matching golden.
- [ ] **Broad slice review (Opus):** re-read the whole 3e diff against the spec — HUD map coverage,
      font bit-exactness, dumper symmetry, both re-diff gates, non-vacuity proofs, Bevy-free `render`,
      deferrals honestly tripwired. Confirm the "full player view" done-when (overview) is met.
- [ ] Reviewer (Opus): docs match reality; deferrals honestly listed; the re-diff ledger is real;
      open questions marked resolved; 3f is the only remaining pixel-render work.
- [ ] **Commit:**
      - `git add docs/superpowers/liero-rs-PROGRESS.md docs/superpowers/specs/2026-07-10-liero-rs-step3-rendering-overview.md`
      - `git commit -m "docs(3e): PROGRESS + overview slice-3e landed; deferral ledger"`

## Done-report (each task)

(a) what changed + why, (b) files touched, (c) tests/risks. Per-task commit, local, on branch
`liero-rs-step-3`. **Do not push, do not open a PR** — the controller owns push + PR. Surface in the
final report: **both re-diff-gate evidences** (T6: sim goldens + `render_slice3a`/`render_slice3b_*`
byte-identical), the **HUD non-vacuity** results (HUD-on ≠ frozen-3b, map on/off, reload-bar
movement — T8), the **font bit-exactness** proof (T0/T1 glyph + ASCII label proof-test), and the
**death-scenario decision** (shipped vs deferred, T7) with its reasoning.
