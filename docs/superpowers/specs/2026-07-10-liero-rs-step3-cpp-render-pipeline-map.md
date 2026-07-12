# OpenLiero C++ CPU render pipeline — map for the Rust port (step 3 research)

Status: **RESEARCH** · 2026-07-10 · input to the step-3 design
Produced by a read-only exploration of the C++ engine at master (post PR #3 merge).
All line references are `file:line` at that revision. Focus is the **headless CPU
path** (videotool + framehash), not the SDL window path.

---

## 1. Frame flow (headless)

The headless "draw one frame" entrypoint is
**`Game::Draw(Renderer&, GameState, bool use_spectator_viewports, bool is_replay)`**
at `src/game/game.cpp:170`. It is a pure consumer of already-simulated state
(worms, objects, level, palette, `cycles`).

**Exact per-frame sequence** (identical in framehash and videotool):

framehash loop — `src/tests/framehash_main.cpp:102-126`:

```
while (replay_reader.PlaybackFrame(renderer))      // feeds recorded inputs into game
  game->ProcessFrame();                            // advance 1 tick of sim (game.cpp:267)
  renderer.Clear();                                // Fill(bmp, 0) — renderer.cpp:21
  game->Draw(renderer, kStateGame, spectator, /*is_replay=*/true);
  HashFrame(renderer);                             // hash bmp (see §7)
  renderer.fade_value = 33;                        // after frame 0, disable fade
```

videotool is the same shape — `src/video_tool/replay_to_video.cpp:94-138`, then
`ScaleDraw(...)` into the ffmpeg picture (`:130`).

`Game::Draw` internals (`game.cpp:170-198`):

1. `renderer.pal = renderer.Origpal()` — reset palette to origin (`:173`).
2. Palette cycling: loop `common->color_anim`,
   `pal.RotateFrom(Origpal(), w.from, w.to, cycles >> 3)` (`:175-177`).
3. `if (screen_flash > 0) pal.LightUp(screen_flash)` (`:179-181`).
4. `renderer.UpdatePal32()` — rebuild the 256-entry ARGB LUT (`:183`, renderer.cpp:23).
5. `Fill(renderer.bmp, 0)` — repaint background through the fresh LUT (`:189`).
6. Dispatch: `DrawSpectatorViewports(...)` or `DrawViewports(...)` (`:191-195`).

`DrawViewports` iterates `viewports` calling `Viewport::Draw` (`game.cpp:141-145`);
`DrawSpectatorViewports` iterates `spectator_viewports` calling
`SpectatorViewport::Draw` (`game.cpp:147-151`).

**State read**: `game.cycles`, `game.level`, `game.worms`,
`game.bonuses/sobjects/wobjects/nobjects/bobjects` pools, `game.holdazone`,
`game.settings`, `game.common`, plus `renderer.fade_value`, `renderer.mode`,
`screen_flash`. Viewport scroll state (`x,y,shake,banner_y`) is updated in
`ProcessViewports()` (`game.cpp:132-139`, `viewport.cpp:22`), which runs inside
`ProcessFrame` (`game.cpp:463`) — i.e. viewport centering is a **sim-phase step**,
not part of draw.

---

## 2. Buffers, resolutions, clipping, scaling

### `Renderer::bmp` format

**`Bitmap` is ARGB8888 (`uint32_t* pixels`)** — NOT 8-bit indexed. See
`src/game/gfx/bitmap.hpp:12-24`. Fields: `w, h, pitch` (pitch in **pixels**, not
bytes), `uint32_t* pixels`, borrowed `uint32_t const* pal32` LUT, `ColorMode mode`,
`int cycles`, `Rect clip_rect`.

Crucially: **all drawing primitives take 8-bit palette indices
(`PalIdx = unsigned char`, `color.hpp:5`) and resolve them to ARGB through `pal32`
at draw time** (`bitmap.hpp:50-54`, `SetPixel`). So the pipeline is "indexed source
art / indexed color args → immediate ARGB write." It is *not* a fully indexed 8-bit
framebuffer; the destination is already ARGB. For the Rust port: there is no
post-hoc palette apply on the framebuffer — the LUT must be finalized *before* any
blit (`game.cpp:171-172` comment).

`pal32[i] = 0xFF000000 | r<<16 | g<<8 | b` (renderer.cpp:23-30). `Color` is
`{r,g,b,unused}` 8-bit each (`color.hpp:7-12`).

### Resolutions

- **Player split-screen view**: renderer inits `320×200` (`replay_to_video.cpp:33`,
  `framehash_main.cpp:71` non-spectator). Individual `Viewport` rects are e.g.
  `Rect(0,0,158,158)` and `Rect(160,0,318,158)` (`replay_to_video.cpp:60-61`).
- **Spectator view (videotool)**: renderer inits to the requested `width×height`
  (e.g. `504+68 × 350` in framehash `:90`, or user-supplied in videotool `:59`).
  One `SpectatorViewport(Rect(0,0,width,height))`.
- **framehash spectator**: `640×400` (`framehash_main.cpp:71`).
- SDL window path (out of headless scope): `play_renderer` 320×200,
  `single_screen_renderer` 640×400 (`gfx.cpp:281-282`); large spectator windows are
  capped via `ComputeCappedRenderResolution` (`spectatorviewport.cpp:51`).

### Clipping / scroll

- `Bitmap::clip_rect` gates every write; `SetPixel` checks `clip_rect.Inside`
  (`bitmap.hpp:50`), fills clamp to it (`blit.cpp:20-37`), the `CLIP_IMAGE` macro
  clips image blits (`macros.hpp`).
- Scroll: `Viewport::x,y` is the world top-left; the draw offset is
  `kOffs = rect.Ul() - IVec2(x,y)` (`viewport.cpp:198`), so `screen = world + kOffs`.
  Clip set to the viewport `rect` inside a `PreserveClipRect` guard
  (`viewport.cpp:193-196`, `:13-20`). Scroll/center math in
  `Viewport::SetCenter/ScrollTo` (`viewport.hpp:35-54`) and clamping in
  `Viewport::Process` (`viewport.cpp:54-57`).

### ScaleDraw path

- **`ScaleDraw`** (`blit.cpp:798`): nearest-neighbor integer magnify ARGB→ARGB with
  fade applied at composition (`mag==1 && !faded` is a fast memcpy). Used by
  videotool to blit `bmp` into the ffmpeg frame (`replay_to_video.cpp:130`). `mag`
  from `FitScreen` (`blit.cpp:860`).
- **`ScaleDrawArea`** (`blit.cpp:828`): box-filter area-average downscale ARGB→ARGB.
  Used by the spectator CPU composite (`spectatorviewport.cpp:766`).

### Player-view vs spectator-view rendering (key difference)

- **Player `Viewport::Draw`** (`viewport.cpp:78`): draws HUD bars/text directly into
  `renderer.bmp`, then sets `clip_rect = rect` and draws the world **directly into
  `renderer.bmp`** at `kOffs` (1:1, no scaling). Minimap drawn last.
- **`SpectatorViewport::Draw`** (`spectatorviewport.cpp:152`): renders the world
  into a **separate `scratch_bmp`** (`:172`), then composites into `renderer.bmp`.
  Two sub-paths:
  - **zoom ≥ 1** (`:307-697`): world drawn 1:1 into scratch (full shadow+sprite
    passes, mirrors player view).
  - **zoom < 1** (`:178-306`): downscaled overview via
    `DrawLevelScaled`+`BlitImageScaled` — **omits shadows, text labels, fire cones,
    laser sights, crosshair, AI debug** (`:180-183`).
  - Composite: GPU handoff (SDL only, `:706`) or CPU `ScaleDrawArea`/memcpy into
    `bmp` (`:743-768`). HUD overlay drawn at native res on top (`:771-913`).
    Headless always uses the CPU composite.

---

## 3. Draw order in the player `Viewport::Draw` (`viewport.cpp:78-636`)

In strict order (HUD first into unclipped bmp, then clipped world, then minimap):

1. Life bar — `DrawBar` `viewport.cpp:86` / `:92`.
2. Ammo/loading bar — `DrawBar` `:106` / `:121`.
3. "Reloading" text — `font.DrawString` `:126`.
4. Kills text — `font.DrawString` `:131`.
5. Replay-only: worm name, colour box (`FillRect`), match time — `:135-143`.
6. Lives / Holdazone / Game-of-Tag timer text — `font.DrawString` `:151` / `:166`
   / `:182`.
7. **[clip set to viewport rect, `:196`]** `renderer.bmp.cycles = game.cycles`
   (`:209`).
8. **Terrain**: `DrawLevel(renderer.bmp, game.level, kOffs.x, kOffs.y)` — `:210`
   (impl `blit.cpp:194`).
9. Holdazone dashed box — `DrawDashedLineBox` `:229`.
10. "Press fire" / spawn-preview worm sprite — `font.DrawString` `:236-237`,
    `BlitImageTrans` `:243`.
11. Death banners (self + other worms) — `font.DrawString` `:251-267`.
12. **Shadow pass 1** (all shadows before any sprite, `:274-398`): bonuses
    `BlitShadowImage :280`; sobjects `:292`; wobjects `BlitShadowImage :330` or
    `ShadowedArgb` pixel `:339`; nobjects `:351` / `:361`; worms + ninjarope
    `DrawShadowLine :376`, `BlitShadowImage :378/:381`; bobjects
    `ShadowedArgb :392`.
13. **Sprite pass 2**:
    - Bonuses — `BlitImage :406`, names `DrawTextSmall :411`.
    - SObjects — `BlitImageR :423`.
    - WObjects — `BlitImage :457` or `SetPixel :462`, names `DrawTextSmall :476`.
    - NObjects — `BlitImage :489` or `SetPixel :494`.
    - Worms (`:500-552`): laser sight `DrawLaserSight :516`; laser weapon line
      `DrawLine :520`; ninjarope `DrawNinjarope :529` + `BlitImage :531`; fire cone
      `BlitFireCone :539`; **worm sprite `BlitImage :545`**; AI debug
      `DrawDebug :550`.
    - Aim crosshair — `BlitImage :572`; weapon-change name `DrawTextSmall :580`.
    - BObjects (blood) — `SetPixel :588`.
14. **[clip restored]** Minimap (`:593-635`): `DrawMiniature :602`, worm dots
    `SetPixel :611`, holdazone marker `:626-633`.

(Spectator equivalent order: world pass `spectatorviewport.cpp:152-697`, then HUD
block `:771-913` — bars, weapon list `:834-852`, kills/name/time, banners.)

---

## 4. Palette handling

- **8-bit→RGB lookup** happens per drawn pixel via `bmp.pal32[index]`, resolved at
  draw time inside every blit (e.g. `blit.cpp:253`, `bitmap.hpp:52`). `pal32` is
  rebuilt each frame by `UpdatePal32` (`renderer.cpp:23`).
- **Palette build order per frame** (`game.cpp:171-183`): reset to `Origpal()` →
  color-cycle → screen-flash lighten → `UpdatePal32`. **Must precede all blits**
  (comment `game.cpp:171`, renderer.hpp:14-17).
- **Cycling/animation**: `Palette::RotateFrom(source, from, to, cycles>>3)` rotates
  a palette sub-range by a `cycles`-derived distance (`palette.cpp:50-57`); driven
  by `common->color_anim` list.
- **Screen flash**: `Palette::LightUp(amount)` — `(v*(32-a)+a*255)>>5` per channel
  (`palette.cpp:24-28,42-48`).
- **Fade**: two forms. (a) `Palette::Fade` on palette entries `(v*amount)>>5`
  (`palette.cpp:18-40`) — used elsewhere. (b) In the headless path, fade is applied
  **at composition time**, not on the palette: `FadeArgb` in `ScaleDraw`
  (`blit.cpp:791-796,815`) and `FadeChannel` in framehash
  (`framehash_main.cpp:32-34`). `fade>=32` is identity. Frame 0 uses `fade_value=0`
  (black), then set to 33 (`framehash_main.cpp:125`, `replay_to_video.cpp:138`).
- **Shadows / lightTable**: no separate light table — shadow = palette-index shift
  `+4` for `SeeShadow` materials (`shadow_query.hpp:38-46`), or in Modern mode a
  display-pixel channel-halve (`:62-66`). Queried against `level.material_id`,
  never against the screen (`shadow_query.hpp:8-15`).
- **Is draw in indexed 8-bit space until the last blit?** No — **the destination
  `bmp` is already ARGB**; indices are resolved to ARGB *at each write*. The
  "indexed" world lives only in the *source art* (sprites, `level.material_id`) and
  color arguments. In **Modern** `ColorMode`, terrain can bypass the palette
  entirely via `Level::AppearanceAt`→`ResolveDisplayAt` returning authored/animated
  ARGB directly (`level.hpp:59-64,220-235`). Classic mode is byte-for-byte VGA
  (6-bit expanded, `palette.cpp:61-72`).

---

## 5. Sprite / text / primitive draw functions (signatures + behavior)

Declared in `src/game/gfx/blit.hpp`; impl in `blit.cpp`:

- `BlitImage(Bitmap& scr, Sprite spr, int x, int y)` — `blit.cpp:239`.
  Index-0-transparent sprite blit; writes `pal32[c]` for `c!=0`.
- `BlitImageTrans(..., int phase)` — `blit.cpp:264`. Same but checkerboard
  `(x^y^phase)&1` (spawn preview).
- `BlitImageR(ShadowQuery const&, Bitmap&, const PalIdx* mem, x, y, w, h)` —
  `blit.cpp:344`. Draws only where level material is water range `[160,168)`
  (sobjects/water).
- `BlitShadowImage(ShadowQuery const&, Bitmap&, const PalIdx* mem, x, y, w, h)` —
  `blit.cpp:433`. Where sprite pixel≠0, writes `shadow.ShadowedArgb(...)`.
- `BlitFireCone(Bitmap&, int fc, PalIdx* mem, x, y)` — `blit.cpp:375`. 16×16,
  threshold+index-offset per cone stage.
- `BlitImageScaled(Bitmap&, Sprite, x, y, float scale)` — `blit.cpp:74`. NN-scaled
  transparent blit (spectator overview).
- `BlitBitmap(Bitmap& scr, Bitmap const& src, x, y, w, h)` — `blit.cpp:217`. Raw
  ARGB rect memcpy (frozen-screen restore).
- `DrawLevel(Bitmap&, Level const&, x, y)` — `blit.cpp:194`. Terrain: per pixel
  `level.AppearanceAt(idx, mode, pal32, cycles)`.
- `DrawLevelScaled(Bitmap&, Level const&, view_x, view_y, float scale)` —
  `blit.cpp:55`. NN-sampled downscaled terrain.
- `Level::DrawMiniature(Bitmap&, map_x, map_y, step_x, step_y)` — `level.cpp:489`.
  Steps through material grid to a `52×36` HUD minimap (`level.hpp:30-31`).
- **Font**: `Font::DrawString(Bitmap&, char const* str, [len,] x, y, color,
  size=1)` — `font.cpp:58`; UTF-8→CP437 decode, per-glyph `DrawChar` (`font.cpp:8`)
  7×8 cells, index-0 transparent, single color. `Font::DrawFramedText` `:82`,
  `GetDims` `:87`. `Common::DrawTextSmall(Bitmap&, str, x, y)` — `common.cpp:227`,
  blits 4px-wide `text_sprites['A'..]`.
- **Primitives**: `DrawBar` (`blit.cpp:101/105`), `FillRect` (`:20`), `Fill`
  (`:39`), `Vline` (`:115`), `DrawRoundedBox`/`DrawRoundedLineBox` (`:128/:142`),
  `DrawDashedLineBox` (`:156`), `DrawLine` (`:719`), `DrawNinjarope` (`:679`,
  color-cycling Bresenham), `DrawLaserSight` (`:693`, **RNG** — see §6),
  `DrawShadowLine` (`:705`), `DrawGraph`/`DrawHeatmap` (stats, `:731/:752`).
- **Health/ammo bars**: plain `DrawBar` calls in `viewport.cpp:86-122` and
  `spectatorviewport.cpp:785-842`.
- `Sprite` = `{PalIdx* mem; int width, height, pitch}` (`sprite.hpp:8-11`);
  `SpriteSet::operator[]`/`SpritePtr` (`sprite.hpp:22-30`).

---

## 6. What draw reads from Game-state, and RNG/mutation audit

**Reads (pure inputs)**: `game.cycles`; `game.level` (`material_id`, `materials`,
`display_data/display_valid/display_anim/argb_ramps`, dims); worms (`pos`,
`current_frame`, `direction`, `aiming_angle`, `hotspot_x/y`, `health`,
`killed_timer`, `visible`, `ready`, `fire_cone`, `ninjarope`, `weapons`,
`current_weapon`, `kills/lives/timer`, `settings`, `make_sight_green`, input
`Pressed()`); object pools `bonuses/sobjects/wobjects/nobjects/bobjects` (positions
via `Ftoi`, `cur_frame`, `type`); `game.holdazone`; `game.settings`; `game.common`
(sprites, weapons, materials, fonts, `color_anim`); `screen_flash`;
`renderer.{mode,fade_value,pal32}`.

**RNG in the draw path — YES, but isolated:**

- `DrawLaserSight(scr, Rand& rand, ...)` draws random sparks and **mutates a
  `Rand`** — `blit.cpp:693-703`. The `rand` passed is the **`Viewport::rand` /
  `SpectatorViewport::rand` member** (`viewport.hpp:33`, used at `viewport.cpp:516`,
  `spectatorviewport.cpp:640`), a *separate* deterministic RNG (`rand.hpp:13`,
  default-seeded), **NOT `game.rand`**. So it does not affect simulation
  determinism, but the draw path **is not strictly side-effect-free** — it advances
  the viewport's own RNG. framehash reproducibility relies on that RNG being
  default-seeded and advanced identically.
- `Viewport::Process` / `SpectatorViewport::Process` also call `rand()` for screen
  shake (`viewport.cpp:50-51`, `spectatorviewport.cpp:135-136`) — but Process runs
  in the **sim phase** (`ProcessViewports`, `game.cpp:463`), not in `Draw`.

**Does draw mutate sim-state or draw `game.rand`?** No. The world-draw code never
calls `game.rand`, never writes worms/objects/`level.material_id`. The
level-*mutating* blitters (`BlitStone` `:462`, `BlitImageOnMap` `:409`,
`DrawDirtEffect` `:534` (takes `Rand&`), `CorrectShadow` `:624`) live in blit.cpp
but are called **only from simulation** (e.g. `worm.cpp:783,931,941`, using
`game.rand`), never from `Viewport::Draw`. `worm.cpp` contains **no draw code** —
all its `rand`/`game.rand` uses are simulation. **Conclusion for the port:
rendering is a pure consumer of sim state; the only draw-time mutation is the
display-only viewport RNG used for laser-sight sparks — reproduce it exactly
(default-seeded `Rand`, same call order) to match C++ frames.**

`Rand`: `engine` (see `rand.hpp:13-36`), `operator()(max)` = `(rand64 * max) >> 32`
(`rand.hpp:30-32`), default seed deterministic (`rand.hpp:14-18`).

---

## 7. framehash — what to reproduce

Source: `src/tests/framehash_main.cpp`.

- **CLI**: `framehash <tc-dir> <replay.lrp> [s|n] [rgb-dump-file]` (`:55`). `s` =
  spectator (640×400), else player (320×200) (`:61,71`). Optional 4th arg dumps raw
  RGB.
- **What is hashed**: `HashFrame` (`:38-50`) iterates `renderer.bmp` row-major
  `y`,`x`; for each ARGB pixel extracts **R,G,B** (drops alpha), applies
  `FadeChannel(v, fade_value)` (`:32-34`, `= v*amount>>5` when `amount<32`), and
  FNV-1a-hashes the **3 bytes in R,G,B order** (`:44-46`). Constants: offset
  `1469598103934665603`, prime `1099511628211` (`:26-27`).
- **Per-frame output**: `"<frame> <hex64>\n"`; running `all` accumulator; final
  `"total <n> <hex64>\n"` (`:111,131`).
- **Fade semantics**: frame 0 hashed with `fade_value=0` (fully black), then
  `fade_value=33` (identity) for all later frames (`:107` default 0, `:125`).
- **Viewport layout (must match)**: `worms[0].stats_x=0`, `worms[1].stats_x=218`
  (`:87-88`); one `SpectatorViewport(Rect(0,0,504+68,350))` (`:90`) + two
  `Viewport`s `Rect(0,0,158,158)` / `Rect(160,0,318,158)` (`:92-95`). But since it
  hashes `renderer.bmp` and the renderer is 320×200 (player) or 640×400
  (spectator), only the viewports fitting that surface contribute.

**Rust must reproduce, byte-exact**: (1) the ARGB `bmp` contents after
`Game::Draw` — i.e. the full palette build (RotateFrom/LightUp/UpdatePal32, incl.
6-bit VGA quantization in Classic, `palette.cpp:56-66`), all draw steps in order
(§3), shadow index `+4` logic, index-0 transparency, viewport RNG for laser sights;
(2) the composition-time `FadeChannel` `(v*amount)>>5`; (3) row-major R,G,B byte
order and the FNV-1a constants. Alpha is ignored, so only RGB must match.

**Oracle note for step 3:** framehash is driven by `.lrp` replays, which the Rust
side does not have until step 4. The step-3 oracle should instead extend the
existing scripted-scenario C++ dumper (oracle_dump family) to call `Game::Draw` +
the FNV frame hash per tick, producing frame-hash goldens for the same scenarios
the step-2 sim goldens use.

---

## 8. LoC estimates to port

Existing draw-relevant source (reference totals): `blit.cpp` 876, `viewport.cpp`
636, `spectatorviewport.cpp` 920, `font.cpp` 114, `palette.cpp` 118, `renderer.cpp`
30, `bitmap.hpp` 68, `shadow_query.hpp` 69, `game.cpp:Draw` ~30, `level`
appearance/minimap ~60.

**(a) Minimal world-view** (terrain + sprites + worms + shadows, single 1:1
viewport, no HUD/minimap/menus):

- Palette build + pal32 + Rotate/Fade/LightUp: ~120
- `Bitmap`/`Sprite`/`Rect` + clip: ~120
- Draw primitives needed: `DrawLevel`, `BlitImage`, `BlitImageTrans`, `BlitImageR`,
  `BlitShadowImage`, `BlitFireCone`, `DrawLine`, `DrawNinjarope`, `DrawLaserSight`,
  `DrawShadowLine`, `SetPixel`: ~350
- `ShadowQuery` + `Level::AppearanceAt`/`ResolveDisplayAt`: ~110
- Viewport world block (`viewport.cpp:191-591`, shadow + sprite passes): ~400
- `ScaleDraw` present + `FitScreen`: ~90
- **≈ 1,100–1,300 LoC**

**(b) Full in-game view** (adds HUD bars, ammo/kills/lives text, minimap, font,
banners) — additive over (a):

- `Font::DrawChar/DrawString/GetDims` + CP437 decode + `DrawTextSmall`: ~200 (plus
  a CP437 table)
- `DrawBar`/`FillRect`/`DrawRoundedBox`/`DrawDashedLineBox`/`Vline`: ~120
- Viewport HUD + minimap blocks (`viewport.cpp:78-189,593-635`): ~250
- **≈ +570 → ~1,700–1,900 LoC** (single/split-screen). Add the full **spectator**
  path (`spectatorviewport.cpp` incl. downscaled overview,
  `DrawLevelScaled`/`BlitImageScaled`/`ScaleDrawArea`, zoom math, HUD dirty-band
  logic) for another **~900 LoC** → **~2,600–2,800** if spectator rendering is in
  scope.

**(c) Menus** (out of step-3 scope — size note only): `menu/` ~600 + `*State.cpp`
draw ~3,000 + menu chrome in `gfx.cpp` (1,800 total, partly SDL/input). Rough
menu-render surface **~2,000–3,000 LoC**; not needed for the CPU game renderer.

### Notes / gotchas for the Rust port

- The destination is ARGB from the start; there is no indexed framebuffer to blit
  at the end — port the "index resolved via `pal32` per write" model, and finalize
  the palette *before* any draw.
- Classic vs Modern color modes diverge: Classic quantizes to 6-bit VGA
  (`palette.cpp:56-66`, `ScaleAdd`), Modern uses full 8-bit and an
  authored/animated terrain display layer (`level.hpp:220`, `shadow_query.hpp:62`).
  framehash/videotool default to Classic via the shipped palette.
- Reproduce the viewport-local `Rand` (default seed, `rand.hpp`) and the exact
  laser-sight call order, or laser frames will diverge.
- Shadow = material-index `+4` (Classic) keyed off `level.material_id`, clamped to
  256 (`shadow_query.hpp:38-46`); two-pass ordering (all shadows, then all sprites)
  is load-bearing for pixel-identity.
- Fade is applied at composition (`ScaleDraw`/`FadeChannel`), not on the palette,
  in the headless path — match `(v*amount)>>5`, identity at `amount>=32`,
  `amount=0` on frame 0.
