# Step 3 — Rendering: overview / altitude decisions

Status: **active** · 2026-07-12 · slices **3a + 3b + 3c + 3d SHIPPED** (see *Slice ordering* below); 3e–3f pending
Part of: `2026-06-26-liero-rs-roadmap.md`
Detailing: the "Step 3 — Rendering" section of `2026-06-26-liero-rs-steps2-5-preliminary-breakdown.md`
Built on: `2026-07-10-liero-rs-step3-cpp-render-pipeline-map.md` (C++ map, cited as **render-map §N**)
and `2026-07-10-liero-rs-step3-bevy-api-research.md` (Bevy 0.19 research, cited as **bevy-research §N**)

This is the Step 3 architecture/strategy decision document — one level more concrete
than the preliminary breakdown, but **not** a per-slice spec and **not** a TDD task
list. It locks the cross-cutting decisions every slice of Step 3 inherits (the
two-layer architecture, the crate graph, the frame-hash oracle mechanics, the
Classic-only starting posture, the slice ordering) so each slice spec can be written
against a stable foundation. The first slice's real spec (`slice3a`) is the companion
document (see *Next artifact*).

Steps 0–2 are complete and merged: `sim-core`, `assets`, and a deterministic,
Bevy-free `sim` crate whose per-tick `HashGameState` matches C++ bit-for-bit over
long fuzzed runs (`2026-06-28-liero-rs-step2-overview.md`). Step 3 gives that
already-correct simulation a picture — natively, headless, and in the browser.

---

## Goal / done-when

Port the C++ **CPU-bitmap renderer** to Rust and drive it from Bevy so the simulated
world is drawn:

1. **Pixel-exact vs the C++ CPU renderer** for the world view (terrain, shadows,
   sprites, worms, ninjarope, laser sight), proven by an FNV-1a frame-hash golden
   differential-tested against C++ (the hard gate — see *Oracle strategy*).
2. **On screen natively** at real time (`cargo run`, fixed-seed scripted demo).
3. **Headless to PNG** with no GPU/window (the CPU buffer *is* the frame).
4. **In the browser** (WebGL2 wasm) at C++ emscripten parity.

**Done when:** the world-view frame hash matches C++ over the reused sim scenarios
(Classic color mode), the full player-view (HUD/minimap) is pixel-gated, a native
fixed-seed demo runs, a headless screenshot CLI + in-repo run-skill exist, and the
wasm build renders the same frame. Presentation (the Bevy window/wasm) is **not**
bit-gated; the CPU renderer that fills the buffer **is**.

Rendering reads sim state and never writes it: the Step 2 `HashGameState` must stay
identical after rendering is wired in (isolation is a hard invariant — see *Risks*).

---

## Architecture: two layers

John's decision (brainstorming complete, not re-litigated here): a **hybrid CPU-blit
architecture** in two layers, mirroring the C++ separation of *simulation → CPU
rasterized frame → presentation* (render-map §1, iteration-exploration §1a). This is
bevy-research recommendation 2 (CPU blit `2a`, not the GPU palette shader `2b`),
chosen for maximum C++ parity and to sidestep every GPU-indexed-texture portability
risk on WebGL2.

### Layer 1 — `render` crate (pure Rust, no Bevy)

A new crate `render` porting the C++ CPU renderer verbatim in *math*, idiomatic in
*API* (modernisation charter, §"Locked decisions"). Deps: `sim`, `assets`,
`sim-core`. **No Bevy dependency** — this is what makes the headless/hash path run
with no GPU, no window, no llvmpipe (bevy-research §3b).

It owns:
- **`Bitmap`** — ARGB8888 destination (`u32` pixels, **pitch in pixels**, borrowed
  `pal32` LUT, `clip_rect`), the port of `bitmap.hpp:12-54`. Indices resolve to ARGB
  *at each write* via `pal32`; there is no post-hoc palette apply (render-map §2, §4).
- **Per-frame palette build** — `reset to Origpal → RotateFrom color-cycle → LightUp
  screen-flash → pack pal32 LUT`, in that exact order and *before any blit*
  (render-map §1 steps 1–4, `game.cpp:171-183`). Classic 6-bit VGA quantization
  (`palette.cpp`); `RotateFrom(source, from, to, cycles>>3)` (`palette.cpp:50-57`);
  `LightUp` `(v*(32-a)+a*255)>>5` (`palette.cpp:42-48`).
- **Blit primitives** — `Fill`/`FillRect`/`SetPixel`, `DrawLevel`, `BlitImage`,
  `BlitImageTrans`, `BlitImageR`, `BlitShadowImage`, `BlitFireCone`, `DrawLine`,
  `DrawNinjarope`, `DrawLaserSight`, `DrawShadowLine` (render-map §5), plus later the
  HUD/font/bar/minimap primitives (§5, deferred to 3e).
- **`DrawLevel`** — terrain via `Level::AppearanceAt` Classic path `pal32[material_id[idx]]`
  (`blit.cpp:194`, `level.hpp:59-64`).
- **Shadow pass** — the `+4` palette-index shift for `SeeShadow` materials, queried
  against `level.material_id` via a `ShadowQuery` port (`shadow_query.hpp:38-46`).
- **Sprite pass** — bonuses/objects/worms/ninjarope/fire-cone/laser-sight/crosshair,
  two-pass ordering (all shadows, then all sprites) which is load-bearing for
  pixel-identity (render-map §3, §8).
- **`Viewport`** — the 320×200 split-screen player layout (two `Rect(0,0,158,158)` /
  `Rect(160,0,318,158)` viewports), `Process` centering/scroll/clamp
  (`viewport.cpp:22-57`), and `Draw` world-block at `kOffs = rect.Ul() - (x,y)`
  (`viewport.cpp:196-210`).
- **Viewport-local `Rand`** — a *separate*, default-seeded RNG (`rand.hpp`) advanced
  only by laser-sight sparks and (gated on `shake>0`) screen-shake, **never**
  `game.rand` (render-map §6). Reproduce its seed + call order exactly or laser
  frames diverge.
- **Frame hash** — FNV-1a over the ARGB buffer, R,G,B order dropping alpha, with
  composition-time `FadeChannel` (render-map §7). The regression primitive.

### Layer 2 — `game` binary crate (Bevy 0.19)

A new binary crate `game`: `default-features = false` + a trimmed Bevy feature list
(`bevy_sprite`, `bevy_winit`, `bevy_window`, `x11`/`wayland`, `webgl2`; drop
`bevy_pbr`/`bevy_ui`/`bevy_gltf`/3D/audio — bevy-research §6, rec 7). It:
- Drives the sim tick from `FixedUpdate` at a configurable hz (`Time::<Fixed>`,
  `Sim(SimState)` resource, one input snapshot per tick, interpolation OFF —
  bevy-research §4).
- Calls the `render` crate to fill the CPU buffer, copies it into one `Image` asset
  (`RenderAssetUsages::all()`, `ImageSampler::nearest()`), draws one `Sprite` under
  a `Camera2d`, integer-scaled by an outer camera (`pixel_grid_snap` pattern —
  bevy-research §2a/§2c).

Presentation is **not** bit-gated. Bevy `Vec2`/`Transform`/`f32` are one-directional
consumers of the fixed-point sim; nothing feeds back (breakdown float-boundary rule).

### Crate graph

```
sim-core   (no deps: fixed, vec, math, rng, tables)
   ▲   ▲
   │   └──────────────┐
assets                sim
(serde, toml)     (sim-core, assets)
   ▲   ▲                ▲
   │   └──────┬─────────┤
   │          │         │
   └────────render ─────┘        ← NEW crate (deps: sim, assets, sim-core; NO bevy)
             ▲   ▲
             │   └──────────────┐
           game                 oracle-tests
     (bevy 0.19, render,    (dev-deps: sim, assets, render;
      sim, assets) NEW bin   frame-hash goldens live here)
```

`sim`/`sim-core`/`assets` stay **unchanged and Bevy-free**. `render` is the new
determinism-relevant crate; `game` is the only Bevy crate. `oracle-tests` gains
`render` as a dev-dependency and hosts the frame-hash differential tests beside the
existing sim goldens. The `render` crate stays on edition 2021 like `sim`; only
`game` needs edition 2024 / Rust ≥1.95 (bevy-research §1).

---

## Locked decisions (inherited by every slice)

1. **Two layers, CPU blit.** `render` (no Bevy) owns the pixels; `game` (Bevy) only
   presents them. The authoritative frame is the CPU `Vec<u8>`, exactly as C++
   `framehash`/`videotool` dump `renderer.bmp.pixels` (render-map §7,
   iteration-exploration §1a).
2. **Pixel-golden is a HARD gate for `render`.** Draw math is ported verbatim (like
   the sim port); the FNV-1a frame hash is differential-tested against C++
   tick-by-tick (see *Oracle strategy*). This is a real gate, not advisory — the
   iteration-exploration doc's "screenshots are advisory" caveat applies to *GPU*
   rasterization, which the CPU renderer bypasses (bevy-research §3c).
3. **Modernise, don't transliterate.** The draw-primitive *arithmetic* is bit-exact
   (palette build, `+4` shadow, index-0 transparency, FadeChannel, viewport RNG);
   the *structure* is idiomatic Rust (methods on a `Bitmap`, slices, `Result`), not
   a mirror of the C++ class layout. The frame hash proves behaviour is preserved.
4. **Rendering is a pure consumer of sim state.** The only draw-time mutation is the
   display-only viewport `Rand` (laser sparks; shake when `shake>0`). The Step 2
   `HashGameState` must be unchanged after rendering is wired — run it *jointly* with
   the frame hash as an isolation proof (render-map §6).
5. **Classic color mode first; Modern deferred.** See *Classic vs Modern* below.
6. **One accumulating PR.** All slices 3a–3f land on branch `liero-rs-step-3`
   (already cut from fresh master), merged when the whole step is done — same pattern
   as Step 2.

---

## Oracle / verification strategy

The Step 2 oracle is a per-tick **state**-checksum time series (a C++ dumper links
the `game` lib, runs a scripted scenario, emits `HashGameState` + `HashGameComponents`
per tick; Rust matches line-for-line). Step 3 adds a per-tick **frame**-hash time
series with the same shape, over the *same scenarios*.

### The frame hash, precisely (render-map §7, `framehash_main.cpp:26-50`)

`hash_frame(&Bitmap) -> u64`, FNV-1a: offset `1469598103934665603`, prime
`1099511628211`. Iterate the ARGB buffer row-major (`y`,`x`); per pixel extract
**R, G, B** (drop alpha), apply `FadeChannel(v, fade)` (`= (v*fade)>>5` when `fade<32`,
identity otherwise), and hash the **3 bytes in R,G,B order**. Frame 0 is hashed with
`fade=0` (fully black), then `fade=33` (identity) for all later frames. Alpha is
ignored — only RGB must match.

### Why extend the scripted dumper, not drive `framehash`

C++ `framehash` is driven by `.lrp` replays, which the Rust side does not have until
Step 4 (breakdown). So Step 3 **extends the existing scripted-scenario dumper**
(`sim_physics_dump.cpp`, the Step 2 time-series driver) to also build a `Renderer`,
call the draw path, and `HashFrame` per tick — producing frame-hash goldens for the
same scenarios the Step 2 sim goldens already use (fuzz/slice scenarios). Replay
(`.lrp` / `framehash`) arrives in Step 4.

### Opt-in extension → existing sim goldens stay byte-identical

Following the Step 2 opt-in-token discipline (every new dumper feature is gated so
untouched goldens regenerate byte-for-byte): the render behaviour is behind a new
scenario directive (proposed `render <layout>`; see slice3a for the exact grammar).
When the directive is **absent**, the dumper's code path and its 11-column sim line
are unchanged — the existing `sim_slice*.txt` goldens regenerate identically
(**re-diff gate**, exactly as Step 2 required). When **present**, the dumper *also*
writes a sidecar frame-hash golden (a second output file), leaving the sim golden's
format untouched. Because the draw path never touches `game.rand` or sim state, the
sim column for a render scenario is provably identical to a non-render run — that is
the **isolation proof** run jointly.

### Golden format (proposed, mirrors the Step 2 column style)

Sidecar file `render_slice3a.txt`, one line per tick plus a running total (the
`framehash_main.cpp:111,131` shape):

```
# <tick> <frame_hash_hex16> <state_hash_hex8>
0 5f2e...a1 078972e2
1 91c4...0d 1c37efc0
...
total 24 <accumulator_hex16>
```

- **`frame_hash_hex16`** — the 64-bit FNV-1a frame hash (the hard render gate).
- **`state_hash_hex8`** — the Step 2 `HashGameState` for the same tick, carried as
  the isolation column; the Rust test asserts it equals the value in the pre-existing
  sim golden for that scenario (rendering did not perturb the sim).
- The final `total` line accumulates all frame hashes (catches a divergence even if a
  per-line diff is skimmed), matching C++ `framehash`'s `total` line.

### Which scenarios

Reuse the Step 2 scenario corpus (the `sim_slice*_scenario.txt` family, all on the
Classic `Levels/physics_fall_test.lev`): slice 3a proves terrain on a scenario with
palette cycling; 3b widens to the shadow+sprite scenarios (the slice-4/5 fire/object
scenarios exercise worms, ninjarope, laser sight, fire cones, blood). No new sim
scenarios are needed — the frame hash rides the same input vectors.

### Isolation as a standing gate

Every render slice re-runs the Step 2 sim goldens (re-diff, byte-identical) **and**
asserts the joint `state_hash` column — two independent proofs that rendering never
touched determinism. This is the render analog of the Step 2 "level hash is a live
time series" discipline.

---

## Classic vs Modern color mode — take a position: **Classic-only to start**

**Decision: build Classic-only through 3a–3e; treat Modern as a deferral, revisited
only if a target scenario needs it.**

Rationale, checked against the shipped assets:
- The Step 2 sim scenarios all load `Levels/physics_fall_test.lev`, which carries
  **no POWERLEVEL/MODERNLV block** — it is a Classic level rendered through the TC's
  shipped VGA palette. So every scenario the frame-hash oracle reuses is Classic; a
  Modern renderer would be *unverified by the existing corpus*.
- C++ `framehash`/`videotool` themselves default to Classic via the shipped palette
  (render-map §4 note, §7).
- The only Modern asset in the tree is `Levels/modern_test.lev` (has MODERNLV) — a
  single fixture with no sim golden behind it.
- Modern mode diverges in two places (render-map §4, `level.hpp:220`,
  `shadow_query.hpp:62`): terrain via `ResolveDisplayAt` returning authored/animated
  ARGB instead of `pal32[idx]`, and shadows as a channel-halve instead of a `+4`
  index shift. Both are additive branches, not rewrites — deferring them costs
  nothing structurally.

The `render` crate's `AppearanceAt`/`ShadowQuery` ports keep the `mode` parameter in
their signatures (so Modern is a later `match` arm, not a refactor), but only the
Classic arms are implemented and gated in 3a–3e. Modern bring-up (+ a `modern_test`
frame golden) is an explicit deferral; adjudicate its priority at step end.

---

## Slice ordering (3a–3f)

Each slice accumulates on branch `liero-rs-step-3`. Each states its done-when and
what it *proves*.

- **3a — render-crate foundation. ✅ SHIPPED (2026-07-10).** `Bitmap` (ARGB8888/pitch/clip),
  per-frame palette build (reset→RotateFrom→pack; LightUp ported but inert until a flash
  occurs), `DrawLevel` Classic path, the two-viewport 320×200 player layout with
  `Viewport::Process` centering (shake-RNG branch present but inert), the FNV-1a
  frame hash + harness, and the C++ dumper's opt-in `render` directive emitting the
  terrain-only draw. **Proves:** the first pixel-exact terrain frame, the palette
  build (RotateFrom driven by `cycles>>3`), the frame-hash mechanism, and isolation.
  **Done-when:** `render_slice3a.txt` matches C++ tick-for-tick over a
  palette-cycling scenario; sim re-diff + joint `state_hash` unchanged. (Companion
  spec: `slice3a-render-foundation-design.md`.)
  **RESULT:** done-when met — `render_slice3a_golden` matches C++ **tick-for-tick** (all
  27 frame hashes + the total accumulator bit-exact, first run); **RotateFrom proven
  observable** (frame hash constant within each 8-tick window, flipping exactly on the
  `cycles>>3` boundaries @8/16/24); **isolation triple-proven** (`state_hash` Rust == the
  sidecar column == the pre-existing sim golden); **re-diff gate GREEN** (all 29 prior sim
  goldens byte-identical). Deferrals carried: Modern `ColorMode` arms, steerable centering
  (→3b), Rust-parser layout-token value validation, a `fade=31` boundary test; LightUp/
  shake/laser-sight RNG present-but-inert until 3b.

- **3b — shadow + sprite pass. ✅ SHIPPED (2026-07-11).** Two-pass world block: shadow pass
  (bonuses/objects/worms/ninjarope/blood) then sprite pass (all 6 object families in C++
  order, `viewport.cpp:274-590`; worm sprites, ninjarope, fire cone, laser sight +
  **viewport-local RNG**, aim crosshair, blood), plus `LightUp` screen-flash and shake now
  going live. Ported: the blit primitives (`BlitImage`/`Trans`/`R`, `BlitShadowImage`,
  `FireCone`, `DO_LINE` Bresenham, ninjarope/laser-sight/shadow-line/line), `ShadowQuery`
  (+4/clamp/`SeeShadow`), the fire-cone table/bank, sprite selectors, `wobj_remap`, a widened
  `frame::draw` (Scene + LightUp/shake live); sim-side the render-only non-hashed `hotspot_x/y`
  + the `ProcessSight` port (closed the laser-origin gap). The dumper renders the full-world
  block behind `render_shadow`/`render_shake`/`render_flash` directives; 3 new fixture levels
  (`render_stage`/`see_shadow_test`/`water_stage`) + Rust parser arms. Reused the slice-4/5
  fire/object scenarios. **Proves:** the world view is pixel-exact over motion, explosions,
  and the laser-sight RNG trap; the two-pass order is load-bearing.
  **Done-when:** met — world-view frame hash matches C++ over the object scenarios; viewport
  RNG frames match.
  **RESULT:** 🎯 **MILESTONE — the world view is PIXEL-EXACT vs the C++ oracle.** 7 golden
  scenarios (laser/shadow/shake/fan/dart/blood/dart_water), **225 frame rows, ALL matched on
  the first run**; triple-isolation proof per tick (`state_hash` Rust == sidecar == the
  pre-existing sim golden). **Non-vacuity proven** — laser: 6 distinct per-tick hashes with
  **both** viewport RNGs live; shadow ON≠OFF; shake steps + a flash blip; pool-drain changes
  the frame (incl. the positive `BlitImageR`-over-water witness). Two real findings en route:
  (1) the T3 review caught an inverted laser `rand(2)` order (the plan had misread C++ —
  `rand(2)` is drawn **only inside** the clip; fixed + plan corrected); (2) the T0b insert
  caught a C++ UB `cossin[128]` OOB on facing-flip (Rust masks `&0x7f`; scenario-constrained
  and documented). Deferrals carried to later slices: Modern `ColorMode` display-halve arm;
  steerable centering (`WormState` has no `steerable_sum`) — assert kept; `bonus_frames` empty
  + `BONUS_FLICKER_TIME` hardcoded (no bonus scenario); spawn-preview `BlitImageTrans` unreached
  (`names_on_bonuses=false`, no `kChange`); Rust-side layout-token value validation; C++
  render-path edits are not caught by CI's re-diff (gen-scripts are local); the `cossin`-UB
  sites (ninjarope/`worm_fire`) left unmasked; `MAT_SEE_SHADOW`-constant placement in `render`.
  Name labels / font (`DrawTextSmall`), HUD/bars/banners/holdazone/minimap → **3e**.

- **3c — Bevy window (native). ✅ SHIPPED (2026-07-12).** The `game` crate: `FixedUpdate` sim tick,
  CPU buffer → `Image` → `Sprite` + `Camera2d`, nearest, integer-scaled; a fixed-seed scripted
  demo playing a deterministic scenario in real time (no input). **Proves:** the
  renderer runs live natively; `cargo run` shows Liero. **Done-when:** met — the demo
  window renders the world view natively at real time.
  **RESULT:** 🎯 **MILESTONE — `cargo run -p game` shows Liero LIVE, the project's first Bevy code.**
  A native 960×600 window presents the `blood` scenario in real time: the sim ticks on `FixedUpdate`
  at the **exact C++ cadence** `1000/14 ≈ 71.43 Hz` (`kDelay=14ms`, `gfx.cpp:1176` — **not** the 60 Hz
  the plan had assumed), the Bevy-free `render` crate's CPU frame is copied into one `Image`
  (`RenderAssetUsages::all()`, nearest sampler) and drawn as a `Sprite` at **×3**, and the scenario
  **loops bit-identically** off its own recorded inputs while a debug determinism guard (per-tick
  `state_hash` vs the sim golden) stays GREEN over ~26 loops / 15 s. **Three controller open questions
  ratified & resolved:** (1) a **shared `scenario` crate** owns the parser (lifted verbatim out of
  `oracle-tests`) + the loader (factored out of the T8 harness) — **3d reuses `scenario::load`**;
  (2) CI keeps the **determinism gate Bevy-free** via `--exclude game` (proven with `cargo tree`) plus
  a separate `cargo build -p game` step; (3) **Option A — fixed ×3 camera** is the default (follow-cam
  is a later `--follow` toggle, not the default). Bevy 0.19 feature set used: `default-features=false`
  + `bevy_sprite`/`bevy_winit`/`bevy_window`/`x11`/`wayland` + `bevy_render`/`core_pipeline`/
  `sprite_render`. Dev tool `render_snapshot.rs` (headless BMP dumper) shipped the first images of
  Rust-Liero. Two real findings: (1) Bevy's `bevy_sprite` feature alone ships **no GPU backend** —
  `sprite_render`→`core_pipeline`→`render`→`wgpu`/`naga` are required (the window had opened
  renderer-less and the draft report carried false lock-claims); (2) a brief bug — **empty** per-tick
  inputs diverged from the golden, so the scenario's **recorded** inputs are fed (the debug guard
  caught it live). **Deferrals carried to the slice that needs each:** keyboard **input** /
  game-loop-with-input and **audio** → Step 4; **render interpolation** (`overstep_fraction` lerp —
  draw the latest tick) → overview deferral; **resize-aware integer-fit camera** (fixed ×3 window
  shipped) and **follow-cam** (Option B `--follow` toggle, `killed_timer` zeroing — diverges from the
  goldens) → later; **live shake/flash wiring** (the `ProcessViewports` equivalent) → Step 4;
  **texture-format gamma** (`Rgba8` Srgb vs linear — advisory, resolved by eyeball) → note the choice;
  **HUD/font/bars/minimap** → 3e; **wasm** (WebGL2, embedded assets; no wasm feature pulled here) → 3f.
  Minor tidy carried: `blit.rs` fmt-drift fixup, scenario-without-sidecar debug-panic comment,
  setup-tick-0 assert gap.

- **3d — headless screenshot CLI + run-skill. ✅ SHIPPED (2026-07-12).** A CLI that
  PNG-encodes the CPU buffer directly (via the `image` crate — no GPU readback,
  bevy-research §3b/§4) at a fixed tick, plus an in-repo `.claude/skills/` run/observe
  skill (iteration-exploration §5/§6). **Proves:** the agent/`verify` loop — change →
  screenshot → judge — with no GPU. **Done-when:** met — the CLI writes a deterministic
  PNG and the skill drives it.
  **RESULT:** 🎯 **MILESTONE — the agent screenshot/compare loop lands.** A new **Bevy-free
  `shot` crate** (lib+bin; deps `scenario`/`render`/`sim`/`assets`/`sim-core` + `image` with
  `default-features=false` png-only — `cargo tree` proves **no bevy/jpeg/gif/rayon**)
  `render_scenario`-drives a 3b scenario `0..=up_to` rendering **every** tick (viewport-RNG
  correctness; recorded inputs `input(k-1)`; per-tick flash/shake injection + `draw_shadow`),
  then `encode_png` writes the CPU buffer as **raw RGB, nearest ×scale, pitch-correct, with NO
  fade** (so tick 0 is not black — distinct from the frame-hash path). `run` resolves paths via
  `CARGO_MANIFEST_DIR`, writes one file (single tick) or a dir (many), and emits **machine-readable**
  per-tick `frame_hash`+`state_hash` sidecar grammar to **stdout** behind `--hashes` (info to
  stderr); `--scale 0` is rejected. A **golden-faithfulness test** (`rust/shot/tests/golden.rs`)
  locks the CLI render bit-for-bit against the committed C++ sidecars for **blood** (base path)
  and **shake** (the CLI copy's ONE new path — the per-tick `render_flash`/`render_shake` injection
  in `render_tick`): every per-tick frame+state hash, the folded FNV `total`, and the row count,
  **GREEN on the first run**. The project's **first in-repo run-skill** (`.claude/skills/liero-shot`,
  resolving **Q5 in-repo** below) drives the build/screenshot/compare loop (all commands verified
  run; 960×600 PNG; hashes match golden exactly). **CI confirmed unchanged** — the existing
  `cargo test --workspace --exclude game` already sweeps `shot` (T0 added it to the workspace
  members), png-encode is pure Rust (miniz_oxide/flate2 — no apt package), and the golden data
  (TC + `oracle-tests/golden/render_slice3b_*`) is committed and checked out by CI exactly as the
  3b oracle-tests already rely on; **11 shot tests (9 unit + 2 golden) run green in the exact CI
  command**. **Per-tick-driver decision — option B:** `render_scenario`/`render_tick` is a
  deliberate **CLI-local copy** of the T8 frame-hash harness (`render_slice3b_common::run`), NOT a
  shared factorisation — T8 is left untouched and the golden-test guards the copy against drift;
  extracting the driver into `scenario` is **deferred until a third consumer** exists. Deferrals
  carried: HUD/font/bars/minimap → 3e; wasm/headless-browser-canvas → 3f; `.lrp`-replay +
  input-timelines → Step 4 (`shot` becomes the replay-regression driver then); driver factorisation
  (third consumer); video/GIF output (out of scope — `image` is png-only by design); a standing CI
  `--hashes` diff-job (redundant — the golden **test** already gates the render); plus review minors
  (parse quirks: flag-as-value consumed silently, last-wins duplicates; the pitch≠w test's `h=1`
  non-vacuity). Reviews: T0/T1/T2 two-stage-reviewed, **0 Critical / 0 Important** blocking (T2's one
  Important fixed in `5dfa9c0`).

- **3e — HUD / font / bars / minimap.** The full player view: `Font::DrawString`
  (CP437), `DrawTextSmall`, `DrawBar`/`FillRect`/`DrawDashedLineBox`/`Vline`, life/
  ammo bars, kills/lives/reloading text, death banners, the 52×36 minimap
  (`DrawMiniature`) (render-map §3 steps 1–6, 14; §5; §8b). Pixel-gated. **Proves:**
  the complete in-game player frame matches C++. **Done-when:** the full player-view
  frame hash matches C++.

- **3f — wasm bring-up.** WebGL2 build, `include_bytes!`/embedded assets (dodge async
  fetch), serve/deploy recipe to C++ emscripten parity (bevy-research §5). **Proves:**
  the same frame renders in the browser. **Done-when:** the wasm build renders the
  world view and is servable.

---

## Risks & the hard 10%

- **Viewport-RNG trap.** The draw path is *not* side-effect-free: `DrawLaserSight`
  mutates the viewport-local `Rand` (`blit.cpp:693`, default-seeded `rand.hpp`), and
  `Viewport::Process` draws `rand()` for shake **only when `shake>0`** (verified,
  `viewport.cpp:48-51`). Reproduce the seed and the exact call order or laser/shake
  frames diverge. It is *not* `game.rand` — sim stays untouched (render-map §6). 3a
  scenarios have zero shake/laser, so the viewport RNG is inert there; 3b makes it
  live and is where this risk bites.
- **Fade semantics.** Fade is applied at *composition* (`FadeChannel`, not on the
  palette) in the headless path: `(v*amount)>>5`, identity at `amount>=32`, and
  **frame 0 is black (`fade=0`)** (render-map §4, §7). Getting frame 0 wrong (or
  fading the palette instead of the channel) silently corrupts the whole golden.
- **Palette build order.** `reset→RotateFrom→LightUp→pack`, finalized *before any
  blit* (render-map §1). The destination is already ARGB; there is no end-of-frame
  palette apply. A blit before the LUT is finalized reads a stale palette.
- **Two-pass draw order.** All shadows before any sprite (render-map §3) — a
  load-bearing ordering for pixel-identity, the render analog of the sim's
  object-loop order.
- **Classic 6-bit VGA quantization.** Classic palette entries derive from a 6-bit VGA
  source (`(v&63)<<2`, already how `assets::Palette::load_vga` stores them); the LUT
  pack must not double-quantize. Confirm the exact `pal32` pack against
  `renderer.cpp:23-30` at implementation.
- **Bevy churn.** Bevy 0.19's *2D internals* are flagged for a future "unify 2D/3D"
  rework (bevy-research §1); keep the `game` renderer thin and swappable, build only
  against the stable public `Sprite`/`Image`/`Camera2d` surface, never
  `bevy_render`-internal types. All Bevy risk is confined to `game`; `render` is
  Bevy-free and immune.
- **GPU-independence of the gate.** The hard gate is the CPU frame hash, which is
  deterministic across machines (bevy-research §3c). GPU screenshots (windowed
  review) stay advisory. Do not let a wgpu adapter difference gate CI.
- **Classic vs Modern.** Committed Classic-only (above); the risk is a later scenario
  silently needing Modern — mitigated by keeping `mode` in the signatures.

---

## Deferrals (explicitly out of Step 3 scope)

- **Spectator renderer** (`spectatorviewport.cpp`, ~900 LoC: scratch bitmap composite,
  `zoom<1` downscaled overview, `ScaleDrawArea`, HUD dirty-band) — render-map §2, §8.
- **Menus / `*State.cpp` draw** (~2,000–3,000 LoC render surface) — render-map §8c.
- **Keyboard input / game-loop-with-input** — Step 4.
- **Audio** — Step 4.
- **Replay / `.lrp` / `framehash`-driven regression** — Step 4 (Step 3 uses the
  scripted dumper instead).
- **Modern color mode** (`ResolveDisplayAt` terrain, channel-halve shadows) — deferred
  per *Classic vs Modern*; revisit at step end.
- **`DrawGraph` / `DrawHeatmap`** (stats overlays, `blit.cpp:731/752`) and **AI debug
  draw** (`DrawDebug`, `viewport.cpp:550`) — diagnostic-only, not game visuals.
- **Render interpolation** (`overstep_fraction` lerp) — deferred; match C++'s
  per-tick render first (bevy-research §4).
- **`ScaleDrawArea` box-filter downscale** — only the spectator path needs it.

---

## Open questions for the controller to adjudicate

> **Resolution status:** Q1–Q3 (3a dumper/scenario/golden-surface mechanics) were **resolved at 3a
> ship** — the extended `sim_physics_dump.cpp` with the opt-in `render` directive and the joint
> `frame_hash`+`state_hash` sidecar line are live and byte-identical. Q4 (Modern) stays **Classic-only**
> as committed. **Q5 (run-skill location) — RESOLVED at 3d ship: in-repo ratified** (`.claude/skills/liero-shot`,
> the project's first in-repo run-skill; travels to worktrees). A related 3d question — should the CLI dump
> **machine-readable** hashes for programmatic compare? — is **resolved YES**: `shot --hashes` ships the
> per-tick `frame_hash`+`state_hash` sidecar grammar to stdout. Q6 (wasm CI depth) lands with **3f**. The
> three **3c** companion-spec questions were **ratified** and are recorded
> in the 3c **RESULT** above: (1) a shared `scenario` crate owning the parser + loader — ratified; (2)
> CI `--exclude game` + a separate `game` build step — ratified; (3) Option A fixed ×3 camera as the
> default — ratified.

1. **Frame-golden scenario for 3a.** Confirm the reused Classic scenario's terrain
   window actually contains a `color_anim`-cycled palette index so `RotateFrom` is
   *observable* in the hash (else palette cycling is ported but unproven). If
   `physics_fall_test.lev`'s visible window uses no cycled index, do we (a) author a
   tiny cycling test level, (b) widen the animated range, or (c) accept RotateFrom
   proof sliding to 3b? (slice3a leans (a)/(b) — prove it in 3a as decided.)
2. **Dumper: extend vs fork.** The decision says *extend* `sim_physics_dump.cpp` with
   an opt-in `render` directive (one source of scenario-grammar truth). Confirm we
   accept the alternative — a sibling `sim_render_dump.cpp` executable — is rejected,
   even though a fork would even more trivially guarantee byte-identical sim goldens.
   (slice3a specs the extend path.)
3. **Frame-hash surface for the joint isolation column.** OK to widen the render
   dumper's line to carry both `frame_hash` and the Step 2 `state_hash`, or keep them
   in fully separate files diffed independently?
4. **Modern deferral.** Ratify Classic-only for 3a–3f, with Modern (+ a `modern_test`
   golden) as a post-step item — or pull Modern into 3e if a demo/share level wants
   the authored-color look?
5. **run-skill location.** ✅ **RESOLVED at 3d — in-repo ratified.** The Step 3
   run/screenshot skill lives in-repo (`.claude/skills/liero-shot`, travels to
   worktrees), not `~/.claude`. (Companion: `shot --hashes` dumps machine-readable
   per-tick `frame_hash`+`state_hash` to stdout for programmatic compare — **shipped**.)
6. **wasm CI depth.** Does 3f only need a servable build + manual check, or a
   headless-browser (Playwright) canvas screenshot in CI
   (iteration-exploration open question)?

---

## Next artifact

The first slice's detailed spec (companion document):
- `specs/2026-07-10-liero-rs-step3-slice3a-render-foundation-design.md`
