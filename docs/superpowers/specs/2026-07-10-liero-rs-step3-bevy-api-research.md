# Liero-rs — Step 3 (Bevy rendering): fresh Bevy API research

Status: **RESEARCH — inputs for the step-3 just-in-time spec** · 2026-07-10
Part of: `2026-06-26-liero-rs-roadmap.md` · answers the "do fresh API research
right before this step" flag in `2026-06-26-liero-rs-steps2-5-preliminary-breakdown.md` §"Step 3 — Rendering"

> **Scope.** This is API-reconnaissance, not a spec and not code. It fixes the
> current Bevy version, verifies the exact API surface step 3 will touch, and
> ends with concrete recommendations for the step-3 design. Every claim is tagged
> **[verified]** (seen in current docs.rs / bevy.org docs or a current example)
> or **[likely]** (durable Bevy knowledge, not re-confirmed against 0.19 source —
> re-check when writing code). Sources are linked inline.

Convention note on the render/sim boundary (unchanged from the breakdown): every
API below lives on the **render side**. Bevy's `Vec2`/`Transform`/`f32` are
one-directional consumers of the fixed-point sim in the `sim` crate; nothing here
feeds back into `SimState`.

---

## 1. Current Bevy version & stability

**Bevy 0.19** is the current stable release. `v0.19.0` tagged **2026-06-18**,
announced **2026-06-19** (261 contributors, 1,185 PRs). docs.rs "latest" resolves
to 0.19 — all context7 lookups below are against 0.19. **[verified]**

- **MSRV: Rust 1.95.0.** **[verified]** (docs.rs crate page). Bevy's MSRV policy
  tracks close to latest stable Rust, so expect it to keep moving. **[likely]**
- **Edition: 2024.** The 0.19 getting-started Cargo.toml uses `edition = "2024"`.
  **[verified]** Our workspace should move to edition 2024 for the render crate
  (the `sim`/`sim-core` crates can stay on whatever they use — they have no Bevy
  dep). Check current `rust-toolchain`; 1.95 is required.
- **Cadence: ~quarterly.** History is roughly one minor every ~3 months, so **0.20
  is likely ~Sept–Oct 2026**. No committed date published; the team mentioned an
  updated Bevy book targeted at the 0.20 cycle. **[likely]**

**Is the API we need stable or mid-rework?** The *user-facing 2D API* we need —
`Sprite` (component), `Camera2d`, `Image` asset, `Material2d`, `Time<Fixed>`,
`Screenshot` — is **mature and stable**. The big ergonomic churn (Required
Components; `Sprite`/`Camera2d` becoming plain components; `Handle`-as-component →
`Sprite{image}` / `Mesh2d`/`MeshMaterial2d`) landed in the 0.15–0.16 era and has
settled. **[verified via current examples]**

One caveat worth flagging: Bevy 0.19's own notes list **"unify 2D and 3D
rendering internals"** as a *future* priority — meaning the 2D render *internals*
(not the `Sprite`/`Material2d` surface) may be reworked in a coming release. This
doesn't threaten step 3's public API but is a reason to (a) keep the renderer
thin and swappable, and (b) not build against private/`bevy_render`-internal
types. **[verified the priority exists; [likely] on impact]**

Sources: [Bevy 0.19 release notes](https://bevy.org/news/bevy-0-19/) ·
[docs.rs bevy 0.19](https://docs.rs/crate/bevy/latest) ·
[bevy releases](https://github.com/bevyengine/bevy/releases) ·
[0.19 milestone review](https://gist.github.com/alice-i-cecile/846d97e1ce5bad1ce9e81a582b54dc07)

---

## 2. 2D rendering of low-res indexed graphics

Two real options; a third (hybrid) falls out. For Liero's 504×350 world buffer /
320×200 player viewport, both are viable — the choice is driven mostly by **wasm**
(§5) and by how faithfully we want to mirror the C++ CPU-bitmap architecture.

### 2a. CPU blit → mutate an `Image` each frame → fullscreen sprite  **[verified]**

This is the **direct analog of the C++ `Renderer`** (which owns a `uint32_t*
pixels` ARGB buffer resolved through `pal32[256]` and blits it to the display).
Port that model: keep an owned RGBA `Vec<u8>` produced from the indexed sim
buffer + palette on the CPU, upload it into a Bevy `Image` asset each frame, draw
that `Image` as one sprite.

Verified API (from the `alter_sprite` and `asset_saving` examples, 0.19):

```rust
// create once
let mut image = Image::new_fill(
    Extent3d { width: 504, height: 350, depth_or_array_layers: 1 },
    TextureDimension::D2,
    &[0, 0, 0, 255],
    TextureFormat::Rgba8UnormSrgb,   // or Rgba8Unorm for linear/no-gamma
    RenderAssetUsages::all(),        // MAIN_WORLD (mutate) + RENDER_WORLD (draw)
);
image.sampler = ImageSampler::nearest();   // crisp upscaling
let handle = images.add(image);
commands.spawn(Sprite { image: handle.clone(), ..default() });

// every frame
fn blit(mut images: ResMut<Assets<Image>>, /* + sim resource */) {
    let image = images.get_mut(&handle).unwrap();
    let data = image.data.as_mut().unwrap();   // Image::data is Option<Vec<u8>> in 0.19
    // write RGBA from indexed sim buffer + palette LUT ...
}
```

- **API notes [verified]:** `Image::data` is `Option<Vec<u8>>` now (hence
  `.as_mut().unwrap()`). `get_mut` on the asset triggers re-upload to the GPU, so
  no manual dirty-flag needed. `ImagePlugin::default_nearest()` sets nearest
  sampling globally; per-image `image.sampler = ImageSampler::nearest()` overrides.
- **Performance [likely]:** 504×350×4 ≈ **706 KB/frame** rewrite + one texture
  upload at 60 Hz ≈ 42 MB/s. Negligible on desktop; fine on wasm. The palette
  lookup is a 256-entry table indexed per pixel — trivially cheap, same work the
  C++ engine already does per frame.
- **Why this is attractive here:** it reproduces the C++ separation
  (sim → CPU rasterized frame → present) almost exactly, sidesteps every
  GPU-indexed-texture portability question (esp. WebGL2, §5), and makes the
  headless/screenshot path conceptually trivial (the buffer *is* the frame). The
  cost is you don't get GPU sprite batching "for free" — but Liero's whole frame
  is one CPU-composited image anyway, so that's not a loss.

Sources: [`alter_sprite` example](https://docs.rs/bevy/latest/src/alter_sprite/alter_sprite.rs.html) ·
[`Image` docs](https://docs.rs/bevy/latest/bevy/image/prelude/struct.Image.html)

### 2b. GPU palette shader: indexed texture + palette LUT in a fragment shader

Upload the indexed buffer as an **R8Uint** texture, upload the 256-color palette
as a small texture/uniform, and resolve the index → color in a `Material2d`
fragment shader. Boilerplate in 0.19 is **moderate** (verified shape below):

```rust
#[derive(AsBindGroup, Asset, TypePath, Clone)]
struct PaletteMaterial {
    #[texture(0, sample_type = "u_int")] index_tex: Handle<Image>,  // R8Uint
    #[texture(1)] #[sampler(2)]         palette_tex: Handle<Image>, // 256x1 RGBA
}
impl Material2d for PaletteMaterial {
    fn fragment_shader() -> ShaderRef { "shaders/palette.wgsl".into() }
}
// app.add_plugins(Material2dPlugin::<PaletteMaterial>::default());
// spawn: Mesh2d(quad) + MeshMaterial2d(materials.add(PaletteMaterial{..}))
```

- **[verified]** `Material2d` trait shape, `AsBindGroup` derive with
  `#[texture]`/`#[sampler]`/`#[uniform]`, `Mesh2d` + `MeshMaterial2d`, material
  bind group at `@group(2)`, `Material2dPlugin`. (From the `Material2d` and
  `shader_material_2d` docs.)
- **[likely]** the `sample_type="u_int"` attribute / `texture_2d<u32>` +
  `textureLoad` (no filtering on integer textures) details — confirm exact
  `AsBindGroup` attribute spelling against 0.19 when writing the shader.
- **Cost/benefit:** ~a WGSL file + ~40–80 lines Rust. Only wins if we upload the
  raw *index* buffer and want the GPU to expand it (saves the CPU palette pass and
  halves/quarters upload bandwidth: R8 = 176 KB vs RGBA 706 KB). For Liero's data
  rates that saving is immaterial, and integer textures are exactly the thing that
  gets awkward on **WebGL2** (§5). So this is the *less* portable option for the
  same visual result.

Sources: [`Material2d` docs](https://docs.rs/bevy/latest/bevy/sprite_render/trait.Material2d.html) ·
[`AsBindGroup`](https://docs.rs/bevy/latest/bevy/render/render_resource/trait.AsBindGroup.html)

### 2c. Pixel-perfect retro (integer scaling, nearest)  **[verified]**

Bevy ships a current example, **`pixel_grid_snap`** (`examples/2d/pixel_grid_snap.rs`),
that is exactly our pattern:

- Render the game to a **low-res canvas `Image` used as a render target** (the
  example uses 160×90; we'd use 504×350 or the 320×200 viewport).
- An **`InGameCamera`** renders the world into that canvas; an **`OuterCamera`**
  renders the canvas to the window, upscaled.
- `ImagePlugin::default_nearest()` for nearest-neighbor upscaling; integer scaling
  keeps pixels square.

This same "render-target canvas + upscale camera" is *also* the headless path
(§3) and the natural place to enforce integer scale factors. The community crate
**`bevy_pixel_camera`** wraps the integer-scaling camera if we want it off-the-shelf.
**[verified example exists; [likely] on bevy_pixel_camera's 0.19 support]**

Note: with the **CPU-blit (2a)** approach we already own a fixed-size RGBA buffer,
so "render to low-res canvas" is automatic — we just draw our one sprite/quad at
native buffer resolution and let the `OuterCamera`/window scale it nearest. The
`pixel_grid_snap` multi-camera dance is more relevant if we render *sprites* (2b/
sprite-per-entity) into the canvas.

Source: [pixel_grid_snap example](https://bevy.org/examples/2d-rendering/pixel-grid-snap/) ·
[source](https://github.com/bevyengine/bevy/blob/latest/examples/2d/pixel_grid_snap.rs)

---

## 3. Screenshot API + headless rendering

### 3a. Screenshot API (windowed)  **[verified]**

0.19 uses the **`Screenshot` component + observer** pattern:

```rust
commands
    .spawn(Screenshot::primary_window())          // or Screenshot::image(render_target)
    .observe(save_to_disk("frame.png"));          // save_to_disk observer helper
```

- `Screenshot::image(target)` targets an off-screen render-target image;
  `Screenshot::primary_window()` grabs the window.
- Capture is **async** — the image may not be ready the same frame; a
  `ScreenshotCaptured` event fires when done, and the entity **auto-despawns**
  after save. So for `run`/`verify` we spawn the request, then let it settle a
  frame (or drive N ticks) before asserting the PNG exists.

Source: [`Screenshot` docs](https://docs.rs/bevy/latest/bevy/render/view/window/screenshot/struct.Screenshot.html)

### 3b. Headless rendering (no window)  **[verified]**

The official **`headless_renderer.rs`** example is the template. Verified 0.19
shape:

```rust
App::new()
    .add_plugins(DefaultPlugins
        .set(ImagePlugin::default_nearest())
        .set(WindowPlugin {
            primary_window: None,
            exit_condition: ExitCondition::DontExit,
            ..default()
        })
        .disable::<WinitPlugin>())        // WinitPlugin panics with no display server
    .add_plugins(ImageCopyPlugin)         // example-local: GPU->CPU copy
    .add_plugins(CaptureFramePlugin)      // example-local: save frames
    .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(1.0/60.0)))
    .run();
```

- **Render-to-texture + GPU→CPU readback** is explicit: render a camera to an
  `Image` target, copy the GPU texture into a mappable buffer, `map_async`, then
  on native call `render_device.poll(PollType::wait_indefinitely())` to complete
  the map (not needed on web — auto-polled). Watch **row-alignment**:
  `RenderDevice::align_copy_bytes_per_row` — padded rows must be trimmed back to
  the real width when copying buffer→image. **[verified]**
- `ImageCopyPlugin` / `CaptureFramePlugin` / `SceneController` are **example
  code, not engine API** — we port them (or use the community `bevy_capture`
  crate for the every-frame video variant, the Bevy analog of C++ `videotool`).
  **[verified they're example-local]**
- **`ScheduleRunnerPlugin::run_loop(Duration)`** replaces the winit runner and
  drives the loop headlessly. `run_loop(0)` / `once()` variants exist. **[verified]**

**Important architectural note for us:** with the **CPU-blit (2a)** renderer, a
"headless frame" barely needs the GPU at all — the authoritative frame is the RGBA
`Vec<u8>` we composited on the CPU. We can PNG-encode that buffer directly (via the
`image` crate) *without* any GPU readback, exactly as C++ `framehash`/`videotool`
dump `renderer.bmp.pixels`. That makes the hard-gate regression path (state
checksum + optional CPU-frame hash) run with **no GPU, no window, no llvmpipe** at
all — the fastest and most portable CI story. Reserve the GPU headless-readback
path for the *sprite/shader* renderer (2b) if we go that way. **[likely — this is a
design inference, but it follows directly from 2a owning the pixels]**

### 3c. CI requirements  **[verified/likely mix]**

- **Linux CI, GPU path:** wgpu needs an adapter. Use a **software adapter** —
  Mesa **llvmpipe** (OpenGL/WebGL2) or **lavapipe** (Vulkan) — selected via
  `WGPU_BACKEND` / `WGPU_ADAPTER_NAME` / `LIBGL_ALWAYS_SOFTWARE=1`, or run under
  **`xvfb`** for a virtual X display. This is the standard "Bevy screenshots on a
  headless runner" recipe. **[likely — durable wgpu/Bevy CI knowledge; exact env
  var set to re-confirm]**
- **macOS locally:** Metal supports offscreen rendering without a connected
  display, so the **headless render-to-texture path works locally on macOS with no
  display and no extra setup**. (A *windowed* run still wants a session, but
  headless offscreen does not.) **[likely]**
- **The pixel-diff caveat still stands** (from the iteration-exploration doc): GPU
  rasterization is not guaranteed bit-identical across adapters, so screenshot
  goldens are **advisory / tolerance / pinned-renderer** — the hard gate is the
  `sim` state checksum. With the CPU-blit renderer, the CPU frame *is*
  deterministic across machines, so a **CPU-frame hash can be a hard gate** too
  (this is precisely what C++ `framehash` does). **[verified reasoning, matches
  the exploration doc]**

Sources: [`headless_renderer` example](https://docs.rs/bevy/latest/src/headless_renderer/headless_renderer.rs.html) ·
[`render_system` / GPU readback](https://docs.rs/bevy/latest/bevy/render/renderer/fn.render_system.html)

---

## 4. Fixed timestep + driving the external `sim` crate

The `sim` crate is Bevy-free and exposes one ordered `SimState::process_frame`.
Bevy's job in step 3–4 is to hold it in a resource and tick it.

### Pattern  **[verified API, [likely] on exact glue]**

```rust
#[derive(Resource)]
struct Sim(sim::SimState);         // wrap the pure-Rust state; no Bevy types inside

app
    .insert_resource(Time::<Fixed>::from_hz(60.0))    // configurable tickrate
    .insert_resource(Sim(SimState::new(/* level, seed */)))
    .add_systems(FixedUpdate, tick_sim);

fn tick_sim(mut sim: ResMut<Sim>, input: Res<CurrentControlState>) {
    sim.0.process_frame(input.pack());   // exactly one tick per fixed step
}
```

Verified pieces (context7, 0.19):

- **`FixedUpdate` schedule** runs zero-or-more times per render frame to consume
  accumulated time. **[verified]**
- **`Time<Fixed>`** with **`set_timestep_hz(60.0)`** / `from_hz` / `timestep()`.
  Tickrate is fully configurable — assume Liero's rate is a constant we set here
  (the breakdown says "assume configurable"; don't hardcode a guessed value in the
  spec — read it from the sim/TC config). **[verified]**
- **Render interpolation** is available via **`Time<Fixed>::overstep_fraction()`**
  (`f32` fraction of the current step, `overstep()` for the `Duration`) — the
  standard "store prev+curr sim pose, lerp in `Update` by the fraction" trick.
  **[verified the API; the lerp is our code]**

### Recommendations / cautions

- **Interpolation OFF for step 3, probably.** Interpolating positions means
  turning fixed-point sim coords into `f32` render coords and blending — pure
  render-side, safe re: determinism, but it adds a moving part while we're still
  establishing visual parity against C++ (which itself renders the raw tick with
  no interpolation). Start with **no interpolation** (draw the latest tick), add
  `overstep_fraction` lerp later only if 60 Hz sim under a >60 Hz monitor looks
  juddery. **[design recommendation]**
- **One input snapshot per tick** (this is a step-4 concern, but design for it
  now): sample `ControlState` once per `FixedUpdate`, never per render frame — the
  exploration + breakdown both make this a determinism precondition for replay and
  step-5 rollback. **[verified from the specs]**
- **Step-5 forward-compat:** `bevy_ggrs` brings its **own `GgrsSchedule`** and
  will *replace* `FixedUpdate` as the tick driver. So treat the `FixedUpdate`
  wiring as step-3/4-local scaffolding that step 5 re-homes — keep `tick_sim` a
  thin, schedule-agnostic call into `sim` so moving it into `GgrsSchedule` later is
  a one-liner. The `Sim` resource wrapping is already the right shape for ggrs
  rollback registration. **[likely — consistent with the breakdown's step-5 notes]**

Source: [`Time<Fixed>` / `set_timestep_hz` / `overstep_fraction`](https://docs.rs/bevy/latest/bevy/prelude/struct.Time.html)

---

## 5. Wasm

### Renderer: WebGL2 vs WebGPU  **[verified]**

- **WebGL2 is still the default** web backend in Bevy 0.19. **[verified]**
- **WebGPU is opt-in** via the `webgpu` feature, which **overrides** `webgl2`; a
  `webgpu` build **won't run on browsers lacking WebGPU**. You currently must
  **choose one per wasm binary** — a single .wasm can't runtime-pick both yet
  (tracked, [issue #13168](https://github.com/bevyengine/bevy/issues/13168)).
  **[verified]**
- **For Liero: target WebGL2.** Broadest reach, and our render needs (one
  fullscreen textured quad, nearest sampling) are trivially within WebGL2. This
  choice also **reinforces the CPU-blit (2a) renderer**: an **R8Uint indexed
  texture + integer sampling in a shader (2b) is exactly the class of thing that
  is awkward/limited on WebGL2**, whereas an RGBA8 `Image` blitted from the CPU is
  the most boringly-portable thing there is. **[verified WebGL2 default; [likely]
  on integer-texture friction — confirm if 2b is pursued]**

### Build flow  **[likely — durable, re-confirm versions]**

Standard `wasm-bindgen` path (mirrors the C++ emscripten preset conceptually):

```
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --out-dir web --target web target/wasm32-unknown-unknown/release/liero.wasm
# serve web/ (index.html + JS glue + .wasm), open in browser
```

`wasm-opt` (binaryen) for size; `trunk` is a common one-command alternative.
No threads by default on wasm (don't rely on Bevy's multithreaded task pools).

### Asset loading in wasm  **[verified options]**

Three approaches, in order of "least wasm friction" for our small fixed asset set:

1. **Embed in the binary** — Bevy's built-in **`embedded_asset!` macro** +
   `embedded://` source, or **`bevy_embedded_assets`** (supports `^0.19`,
   modes `AutoLoad`/`ReplaceDefault`/`ReplaceAndFallback`). This is the Bevy
   analog of the C++ emscripten `--preload-file` trick and **dodges async fetch
   entirely** — best fit for Liero's small, fixed TC/palette/sprite set.
   **[verified crate supports 0.19]**
2. **`include_bytes!`** the raw TC/palette/level bytes directly into the `assets`
   crate and hand them to the sim/renderer — since our asset parsing already lives
   in the Bevy-free `assets` crate (not Bevy's `AssetServer`), this is arguably the
   *simplest* path: no Bevy asset source, no async, identical on native and wasm.
   **[likely — follows from the existing `assets` crate design]**
3. **Default HTTP fetch** — `AssetServer::load("path")` fetches async over HTTP on
   wasm. Works, but introduces async load states (the C++ build explicitly avoided
   this by preloading). Only worth it if assets get large/dynamic. **[verified
   default behavior]**

**Recommendation:** go with (1)/(2) — embed. Our data is small and fixed, and the
`assets` crate already owns parsing outside Bevy, so `include_bytes!` into that
crate is the lowest-friction, most native/wasm-symmetric route.

### How close to "works out of the box" for 2D wasm?  **[likely]**

**Close.** 2D Bevy on WebGL2 is a well-trodden path (the Bevy examples site ships
wasm builds). The real friction is not rendering but the classics the exploration
doc already named: **no threads, async asset loading (dodged by embedding), and
indexed-palette-on-GPU (dodged by CPU blit).** Choosing **CPU-blit RGBA + embedded
assets + WebGL2** removes all three wasm-specific hazards, so 2D wasm bring-up
should be near-turnkey after the native renderer works.

Sources: [Bevy + WebGPU](https://bevy.org/news/bevy-webgpu/) ·
[WebGL2/WebGPU single-wasm issue #13168](https://github.com/bevyengine/bevy/issues/13168) ·
[bevy_embedded_assets](https://github.com/vleue/bevy_embedded_assets) ·
[embedded_asset macro](https://docs.rs/bevy/latest/bevy/asset/macro.embedded_asset.html)

---

## 6. Minimal dependency surface & whether Bevy is the right tool

### Feature trimming  **[verified/likely mix]**

Bevy is modular via cargo features; `default-features = false` + an explicit list
compiles only what you use, cutting build time and binary size. 0.19 also exposes
a **`2d` meta-feature** grouping the 2D stack. **[verified the mechanism +
`cargo_features.md`; exact 0.19 feature names to pin from that file]**

For **2D sprite + window + screenshot**, the plausible trimmed set is:

- `bevy_sprite` (pulls `bevy_render`, `bevy_core_pipeline`) — sprites/`Image`/2D.
- `bevy_winit` + `bevy_window` — the window + event loop (native).
- `png` — only if we decode PNG via `AssetServer`; **not needed** if we
  `include_bytes!` + parse in the `assets` crate and blit RGBA ourselves.
- `x11` and/or `wayland` — Linux windowing.
- `webgl2` — wasm target.
- **Screenshot needs nothing extra** — it lives in `bevy_render`, already pulled
  by `bevy_sprite`. **[likely]**
- If we add the **palette Material2d (2b)** we also need `bevy_sprite`'s material
  path (already included) + shader assets. CPU-blit (2a) needs no shader features.

Deliberately **excluded**: `bevy_pbr`/3D, `bevy_ui` (unless we build HUD in Bevy
UI vs blitting the 4×4 font ourselves — the C++ HUD is CPU-drawn, so blitting is
the parity choice and drops `bevy_ui`), `bevy_gltf`, `bevy_audio` (audio is step 4;
may use a lighter crate), `bevy_gizmos`, animation, etc.

**Compile times [likely]:** first clean build of even trimmed Bevy is heavy
(hundreds of crates, minutes); **incremental** builds are in the "fine" range,
which is Bevy's stated target. Trimming features and using the `dynamic_linking`
feature *for dev only* (fast iterative linking; never ship it) are the standard
mitigations. Expect the `sim`/`assets` crates to stay fast; the Bevy render crate
is where build cost concentrates.

### Is Bevy right for *this* use case?  **[analysis]**

- **Nothing is fundamentally unsuitable.** A CPU-composited fullscreen RGBA blit +
  one window + a fixed-timestep tick is squarely inside Bevy's 2D wheelhouse; every
  API we need is present and stable (§§2–4).
- **Honest counterpoint:** for a game whose entire renderer is "own a CPU pixel
  buffer and blit it," a `pixels` + `winit` stack (or `macroquad`) maps *even more*
  directly to the C++ `Renderer` model and compiles far faster — `pixels` is
  literally "a pixel buffer to the screen." If step 3 were the whole project, that
  would be a real temptation.
- **But the project deliberately targets Bevy, and step 5 (`bevy_ggrs` rollback)
  makes Bevy effectively load-bearing** — the rollback/ECS ecosystem we need later
  is Bevy-centric, and re-homing off `pixels` at step 5 would be a rewrite. So
  **Bevy is the correct default**, and the CPU-blit approach lets us keep Bevy's
  benefits (window, input, wasm, ggrs, schedules) while borrowing the C++ engine's
  clean sim/rasterize/present separation. The learning-Bevy goal is also a stated
  project value.

**Verdict: stay on Bevy. No blocker.** Note the `pixels`/`macroquad` comparison in
the spec only as the "why not" we consciously rejected.

Sources: [cargo_features.md](https://github.com/bevyengine/bevy/blob/main/docs/cargo_features.md) ·
[Bevy config / feature trimming (cheatbook)](https://bevy-cheatbook.github.io/setup/bevy-config.html) ·
[Rust game engines 2026 comparison](https://aarambhdevhub.medium.com/rust-game-engines-in-2026-bevy-vs-macroquad-vs-ggez-vs-fyrox-which-one-should-you-actually-use-9bf93669e83f)

---

## Recommendations for the step-3 design

1. **Pin Bevy 0.19, edition 2024, Rust ≥ 1.95** for a new `render`/`game` crate in
   the workspace. Keep `sim`/`sim-core`/`assets` Bevy-free and unchanged. Expect
   0.20 ~Q3 2026; the public 2D API is stable, so a later bump should be low-cost.
2. **Renderer = CPU blit (2a), not the GPU palette shader (2b).** Port the C++
   `Renderer` model: composite indexed sim buffer + palette → owned RGBA `Vec<u8>`
   on the CPU → mutate one `Image` asset per frame → draw one `Sprite`, nearest
   sampling, integer-scaled by an outer camera. This maximizes C++ parity, removes
   every GPU-indexed-texture portability risk, and makes wasm + headless trivial.
   Revisit 2b only if profiling ever shows the CPU blit is a bottleneck (it won't
   at 706 KB/frame).
3. **Pixel-perfect** via `ImagePlugin::default_nearest()` + the `pixel_grid_snap`
   low-res-canvas / integer-upscale camera pattern. Support both the 504×350 world
   buffer and the 320×200 player viewport as canvas sizes.
4. **Headless + screenshots:** because 2a owns the CPU frame, **PNG-encode the RGBA
   buffer directly (via the `image` crate) for the hard regression path — no GPU,
   no window, no llvmpipe** — and this doubles as a deterministic **CPU-frame-hash
   gate** (the `framehash` analog). Keep the Bevy `Screenshot` API + the
   `headless_renderer` GPU-readback pattern available for windowed/visual review,
   but don't make CI depend on GPU rasterization. On macOS, headless offscreen
   works locally with no display; on Linux CI use llvmpipe/lavapipe + `WGPU_BACKEND`
   or `xvfb` *only if* a GPU-path screenshot is actually needed.
5. **Fixed timestep:** drive `sim::process_frame` from `FixedUpdate` with
   `Time::<Fixed>::from_hz(<configurable, read from TC/config>)`, `Sim(SimState)`
   as a `Resource`, exactly one input snapshot per tick. **Interpolation OFF**
   initially (match C++'s per-tick render; `overstep_fraction` lerp is a later
   polish). Keep `tick_sim` schedule-agnostic so step 5 can move it to
   `GgrsSchedule` trivially.
6. **Wasm:** target **WebGL2**; **embed assets** (`include_bytes!` into the
   `assets` crate, or `bevy_embedded_assets`) to dodge async fetch; no threads.
   With CPU-blit + WebGL2 + embedded assets, all three classic wasm hazards are
   pre-empted, so 2D wasm bring-up should be near-turnkey after native.
7. **Features:** `default-features = false` + `bevy_sprite`, `bevy_winit`,
   `bevy_window`, `x11`/`wayland`, `webgl2`; drop `bevy_pbr`, `bevy_ui`
   (blit the 4×4 font like C++), `bevy_gltf`, 3D. Use dev-only `dynamic_linking`
   for iteration speed. Pin exact feature names from `docs/cargo_features.md` at 0.19.
8. **Stay on Bevy** — no blocker for this use case; `pixels`/`macroquad` were
   considered and rejected because step-5 `bevy_ggrs` makes Bevy load-bearing and
   the project explicitly wants to learn Bevy.

### Confidence summary

- **High / [verified] against 0.19 docs+examples:** version/MSRV/edition; `Image`
  mutation API; `Sprite`/`Camera2d`/`ImageSampler`/`ImagePlugin::default_nearest`;
  `Material2d`/`AsBindGroup`/`Mesh2d`/`MeshMaterial2d`; `Screenshot` +
  `save_to_disk` observer; `headless_renderer` plugin shape + GPU readback;
  `Time<Fixed>`/`set_timestep_hz`/`overstep_fraction`/`FixedUpdate`;
  `pixel_grid_snap` example; WebGL2-default / WebGPU-opt-in; `bevy_embedded_assets`
  supports 0.19.
- **Medium / [likely] — durable knowledge, re-confirm when coding:** exact
  `AsBindGroup` integer-texture attribute spelling; precise CI env-var set for
  llvmpipe/lavapipe; exact trimmed feature names at 0.19; macOS-headless-offscreen
  specifics; the "CPU frame → direct PNG, no GPU" path (design inference, sound but
  unbuilt); 0.20 timing.
