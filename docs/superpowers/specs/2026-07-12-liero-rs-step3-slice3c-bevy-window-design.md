# Step 3 · Slice 3c — Bevy window (native): detailed design

Status: **draft for review** · 2026-07-12
Part of: `2026-07-10-liero-rs-step3-rendering-overview.md` (cited as **overview**)
Sources: `2026-07-10-liero-rs-step3-bevy-api-research.md` (**bevy-research §N**),
`2026-06-26-liero-rs-interactive-iteration-exploration.md` (**iteration §N**),
`2026-07-10-liero-rs-step3-slice3a-render-foundation-design.md` / `…slice3b-shadow-sprite-design.md`
(the sibling slice designs whose shape this mirrors).
Companion output feeds: `superpowers:writing-plans` (this is the spec, not the plan).

This is the executable design for **the project's first Bevy code**. Slices 3a+3b
shipped: the Bevy-free `render` crate produces a pixel-exact, differential-tested CPU
frame (a 320×200 ARGB8888 `Bitmap`) of the whole world view. 3c stands up the `game`
binary that puts that frame **on screen, live, at real time** — `cargo run -p game`
shows Liero.

**3c is NOT bit-gated.** The hard gate is the CPU frame hash, already green in 3b.
3c is *presentation*: a thin Bevy layer that ticks the sim, calls `render`, and blits
the resulting buffer into a window. So this spec is light on oracle machinery and heavy
on product architecture and the determinism firewall between Bevy and the sim.

---

## Goal of 3c

`cargo run -p game` opens a window that **plays a deterministic, fixed-seed scenario in
real time** (no input — input is Step 4), nearest-neighbor integer-scaled, closable.
It proves the `render` crate runs live natively and that the Bevy tick→render→present
loop is correct and determinism-safe.

**Done when:**
1. `cargo run -p game` opens a window showing the world view of a scripted scenario,
   advancing at ~71.4 sim-ticks/s (the C++ cadence, below), looping deterministically.
2. Scaling is nearest-neighbor and integer (crisp pixels, no blur).
3. The window is closable (Esc / close button) and the process exits cleanly.
4. `cargo build -p game` is green in CI (the window is not launched in CI).
5. The Bevy layer never mutates `SimState` outside the `FixedUpdate` tick, and no
   `f32`/`Vec2`/`Transform` ever flows *into* the sim (isolation invariant, overview
   locked-decision 4). A debug-only self-check asserts the demo's per-tick
   `hash_game_state` matches the committed golden for the demo scenario.

Not in 3c: input (Step 4), audio (Step 4), headless screenshot CLI + run-skill (3d),
HUD/font/bars/minimap (3e), wasm (3f), render interpolation (overview deferral).

---

## Tickrate — what C++ runs at

The C++ main loop paces itself with a fixed inter-frame delay:

```
src/game/gfx.cpp:1176   static unsigned int const kDelay = 14U;
src/game/gfx.cpp:1178   auto wanted_time = last_frame + kDelay;   // busy/SDL_Delay until reached
```

`kDelay = 14 ms` ⇒ **1000/14 ≈ 71.43 Hz**, one `processFrame` per loop iteration
(classic Liero cadence). So 3c drives the sim at `Time::<Fixed>::from_hz(1000.0/14.0)`.

Determinism does **not** depend on this number — the sim advances by *tick count*, not
wall-clock — so the exact hz only affects perceived playback speed. Matching 14 ms makes
the demo feel identical to C++. (Verify at implementation that no frame-skip multiplier
sits between the delay and `processFrame`; the loop reads as 1 tick per 14 ms.)

---

## Crate setup — `rust/game`

A new **binary** crate `game`, added to `rust/Cargo.toml` `members`. It is the **only**
Bevy crate (overview crate graph); `render`/`sim`/`assets`/`sim-core` stay Bevy-free and
unchanged. It is also the first **edition-2024 / Rust ≥ 1.95** crate (bevy-research §1);
the others stay on edition 2021. The workspace supports mixed editions per member.

### `rust/game/Cargo.toml` (illustrative; pin exact feature names against 0.19 `docs/cargo_features.md`)

```toml
[package]
name = "game"
version = "0.1.0"
edition = "2024"

[dependencies]
render   = { path = "../render" }
sim      = { path = "../sim" }
assets   = { path = "../assets" }
sim-core = { path = "../sim-core" }
scenario = { path = "../scenario" }   # NEW shared crate — see "Scenario playback"

bevy = { version = "0.19", default-features = false, features = [
    "bevy_sprite",   # sprites / Image / 2D pipeline (pulls bevy_render, bevy_core_pipeline)
    "bevy_winit",    # window + event loop
    "bevy_window",
    "x11",           # Linux windowing (CI + Linux dev); no-op on macOS
    "wayland",       # Linux windowing (optional companion to x11)
] }

# Dev-only fast-iteration linking. NEVER a default; never enabled in CI or wasm.
[features]
dynamic = ["bevy/dynamic_linking"]
```

**Decisions taken:**

- **Feature list.** `bevy_sprite` + `bevy_winit` + `bevy_window` is the minimum for
  "one fullscreen sprite in a window" (bevy-research §6/§7, rec 7). Deliberately dropped:
  `bevy_pbr`/3D, `bevy_ui` (the C++ HUD is CPU-blit — we blit the font ourselves in 3e,
  not Bevy UI), `bevy_gltf`, `bevy_audio`, `bevy_gilrs`. **No `png`** — assets are parsed
  by the Bevy-free `assets` crate, not `AssetServer`. **No `webgl2`/`webgpu`** — wasm is
  3f; pulling it now would drag the wasm hazard surface into a native slice.
- **macOS vs Linux windowing.** macOS needs *no* windowing feature — winit uses the
  built-in Cocoa/AppKit backend automatically. The `x11`/`wayland` features gate
  Linux-only deps and are inert on macOS, so enabling them unconditionally makes Linux CI
  (`cargo build -p game` on ubuntu) and John's macOS dev both build with **no extra system
  packages** (a trimmed Bevy with no `bevy_audio`/`bevy_gilrs` needs no alsa/udev headers;
  winit's x11/wayland crates dlopen at runtime, so no `-dev` packages at build time).
- **`dynamic_linking` = dev-only opt-in.** Not in default features (it breaks wasm and
  must never ship). Exposed as a passthrough `dynamic` feature so John iterates with
  `cargo run -p game --features dynamic`. *Why:* Bevy links as a single `.dylib` in that
  mode, cutting the per-edit link step from seconds to sub-second — the standard Bevy
  iteration trick. CI and 3f must not set it.
- **Version pin.** `bevy = "0.19"` (caret). The exact patch is pinned by `Cargo.lock`;
  a hard `=0.19.x` is over-tight. Keep the renderer thin and only against the stable
  public `Sprite`/`Image`/`Camera2d` surface (bevy-research §1, "2D internals may be
  reworked" — all that risk is confined to `game`).

### Cargo.lock — expect a large, correct diff

Adding Bevy pulls **hundreds** of transitive crates (wgpu, winit, naga, the `bevy_*`
family, …). The `Cargo.lock` diff will be big and is **expected**; call it out in the PR
so a reviewer doesn't mistake it for churn. First clean build is heavy (minutes);
incremental builds are fine, and `--features dynamic` keeps the inner loop fast
(bevy-research §6).

---

## Architecture — resources & systems

Layered exactly as bevy-research §4 / rec 2 recommends: the sim lives in a resource,
Bevy only ticks it and presents its CPU frame.

### Resources

```rust
#[derive(Resource)]
struct Sim(sim::state::SimState);          // the pure-Rust sim; NO Bevy types inside

#[derive(Resource)]
struct Demo {                              // everything the per-tick render needs
    scenario: scenario::Scenario,
    viewports: [render::viewport::Viewport; 2],
    surface: render::bitmap::Bitmap,       // the 320×200 ARGB CPU buffer, owned
    tick: u32,                             // 0..=scenario.ticks, wraps (loop)
    // owned Scene ingredients (origpal, color_anim, fire_cone bank, nr_begin/end, laser_weapon)
    scene: scenario::SceneData,
    #[cfg(debug_assertions)]
    golden: Vec<u32>,                      // per-tick state_hash column for the self-check
}

#[derive(Resource)]
struct FrameImage(Handle<Image>);          // the one Image the sprite samples
```

`Sim` wraps `SimState` with no Bevy types — the same shape Step 5's `bevy_ggrs` rollback
registration will want, so `tick_sim` stays schedule-agnostic (bevy-research §4).

### `setup` (Startup)

1. `scenario::load(tc_root, &scenario)` → `(SimState, [Viewport;2], SceneData)` (the
   shared loader; see next section). Insert as `Sim` + `Demo`.
2. Build the CPU surface `Bitmap::new(320, 200)`.
3. Create one `Image` sized 320×200, `RenderAssetUsages::all()` (MAIN_WORLD mutate +
   RENDER_WORLD draw), `sampler = ImageSampler::nearest()` (bevy-research §2a). Store its
   handle in `FrameImage`.
4. Spawn `Camera2d` + one `Sprite { image: handle, .. }` scaled ×3 (see *Camera & scaling*).
5. Render tick 0 into the surface and upload once so the window shows frame 0 immediately.

### `tick_and_render` (FixedUpdate, `Time::<Fixed>::from_hz(1000.0/14.0)`)

One system does **tick → CPU render → upload**, in that order, exactly once per tick:

```rust
fn tick_and_render(mut sim: ResMut<Sim>, mut demo: ResMut<Demo>,
                   mut images: ResMut<Assets<Image>>, frame: Res<FrameImage>) {
    // 1. advance the sim exactly one tick (no input in 3c → empty ControlStates)
    let inputs = [ControlState::default(), ControlState::default()];
    sim.0.process_frame(&inputs);
    demo.tick += 1;
    if demo.tick > demo.scenario.ticks { reload(&mut sim, &mut demo); }  // loop, re-seed

    // 2. render the latest tick into the owned CPU surface (render is a pure consumer)
    let scene = demo.scene.as_scene();                 // borrow ingredients into a Scene
    render::frame::draw(&mut demo.surface, &sim.0, &mut demo.viewports, &scene);

    // 3. upload: ARGB u32 buffer → the Image's Rgba8 bytes; get_mut triggers re-upload
    let image = images.get_mut(&frame.0).unwrap();
    blit_surface_into_image(&demo.surface, image);

    // 4. debug-only determinism self-check (default scenario is a golden)
    #[cfg(debug_assertions)]
    debug_assert_eq!(sim::hash::hash_game_state(&sim.0), demo.golden[demo.tick as usize]);
}
```

**Why tick + render live in one FixedUpdate system (not split into Update):** interpolation
is OFF (draw the latest tick, bevy-research §4), and the viewport-local RNG (laser sparks)
advances *once per draw*. Rendering exactly once per tick keeps the draw cadence identical
to the golden's one-draw-per-tick, so the self-check stays meaningful and the demo can't
double-advance the viewport RNG when `FixedUpdate` runs twice in a slow render frame. **Do
not** also render in `Update`.

`get_mut` on the `Image` asset marks it dirty and re-uploads to the GPU — no manual dirty
flag (bevy-research §2a). Payload is 320×200×4 = **256 KB/frame**; at 71.4 Hz ≈ 18 MB/s —
trivially cheap (even lighter than the 706 KB/frame the research sized for the 504×350
world buffer, because our surface is the 320×200 player view).

### `blit_surface_into_image` — the one genuinely unit-testable piece

The `render` `Bitmap` stores pixels as `Vec<u32>` packed `0xAARRGGBB`
(`pack_pal32`: `0xFF000000 | r<<16 | g<<8 | b`). A Bevy `Image` wants a `Vec<u8>` in
`Rgba8*` **byte** order `[R, G, B, A]`. The conversion is pure and byte-order-sensitive
(get it wrong and colors are swapped/blue-tinted):

```rust
// per pixel px: [ (px>>16)&0xff, (px>>8)&0xff, px&0xff, (px>>24)&0xff ]  == [R,G,B,A]
```

Unit-test this against a hand-built `Bitmap` (see *Test list*). This is the single
non-smoke test in 3c.

### Camera & scaling — fixed integer scale, one sprite

The CPU buffer **is** the low-res canvas (bevy-research §2c: with CPU-blit you already own
a fixed-size RGBA buffer, so the multi-camera `pixel_grid_snap` render-target dance is
unnecessary — that pattern is for GPU sprite rendering *into* a canvas). So:

- one `Camera2d` (default), one `Sprite` at native 320×200, `Transform` scale ×3.
- window fixed at **960×600** (= 320×200 × 3), title `"Liero-rs — 3c demo"`.
- nearest sampling via per-image `ImageSampler::nearest()` (or the global
  `ImagePlugin::default_nearest()`).

×3 gives a comfortable window on a laptop and keeps pixels square. A resize-aware
integer-fit (recompute the largest integer scale for the current window) is a nice-to-have
**deferred** — a fixed 3× window is enough for the 3c smoke/milestone.

**Texture format is a small open question** (see *Open questions*): `Rgba8UnormSrgb`
vs `Rgba8Unorm` decides whether the pipeline gamma-round-trips the VGA palette bytes.
The CPU frame hash is the real gate (already green), so window color is *advisory* —
pick whichever renders the palette faithfully on John's machine at implementation.

---

## Scenario playback — share the loader in a new `scenario` crate

**Problem.** The demo needs (a) the scenario **grammar/parser** and (b) the
**"scenario text + TC root → `SimState` + `[Viewport;2]` + Scene ingredients"** load path.
Today the parser lives in `oracle-tests/src/scenario.rs` (`oracle_tests::scenario`), and
the load path lives in the **test-only** T8 harness `render_slice3b_common::build()`
(`oracle-tests/tests/…`). `game` **cannot** depend on `oracle-tests`: it is a test crate
(only `[dev-dependencies]`, no library deps), and `game → oracle-tests` is the wrong
direction architecturally.

**Decision (recommended): introduce a new Bevy-free library crate `rust/scenario`.**
It owns both pieces:

- **the parser** — moved verbatim from `oracle-tests/src/scenario.rs` (all 40+ parser
  tests move with it). Deps: none beyond `std` (the parser is std-only).
- **the loader** — `load(tc_root: &Path, s: &Scenario) -> Loaded`, factored out of the
  T8 `build()`: read `small.tga` (origpal), the level, `tc.cfg`, `Objects::load`, resolve
  weapons, `SimState::new(...)` + the post-`new` TC-scalar assignments, build the two
  viewports and the fire-cone bank, and return an owned `SceneData` (origpal, color_anim,
  fire_cone, nr_begin/nr_end, laser_weapon). Loader deps: `sim`, `assets`, `render`,
  `sim-core` — **all Bevy-free**, so `game` stays one dependency-hop from Bevy.

Wiring:

```
sim-core ◄ assets ◄ sim ◄ render
                      ▲       ▲
                      └── scenario ──┘        (NEW: deps sim, assets, render, sim-core)
                            ▲     ▲
                          game    oracle-tests(dev-dep)
```

No cycle: `render` does not depend on `scenario`; `oracle-tests` dev-depends on
`scenario` + `render`; `game` depends on `scenario`. All Bevy-free.

**De-duplication bonus.** The T8 harness `build()` collapses into a thin wrapper over
`scenario::load` (it currently *is* that logic, inline). **3d** (headless PNG CLI) needs
the identical load path — so factoring it now pays off twice. To keep churn minimal, leave
a re-export in `oracle-tests` (`pub use ::scenario::Scenario;` under the old
`oracle_tests::scenario` path) so the ~25 existing test references don't have to change.

**Lighter alternative (if the controller wants minimal 3c footprint):** move *only* the
parser to a shared spot and let `game` carry its own ~80-line loader mirroring `build()`.
This duplicates the load logic (and 3d would duplicate it again), so the shared crate is
the better call — but the parser-only move is a valid smaller step. *(Open question 1.)*

### Which scenario, and how it's chosen

**Default: `blood`** — one of the 7 committed 3b goldens (`golden/render_slice3b_blood.*`):
a worm fires a DART into its own feet, spraying blood (bobjects live ~30 ticks) and carving
terrain while worm0 survives ~95 ticks. It is the liveliest of the corpus (motion +
particles + carving) and, being a golden, gives the determinism self-check for free (below).
`dart` (debris + carve) and `dart_water` (the `BlitImageR` water witness) are good
alternates.

Selection: **hard-coded to `blood` first** (simplest, deterministic). A `cargo run -p game
-- <name>` positional arg to pick another committed scenario is a trivial, welcome add but
not required for done-when; specify it as optional.

### End-of-scenario behavior: loop by re-seed

When `tick` passes `scenario.ticks`, **rebuild from the loader at tick 0** (re-seed the
`Rand`, reset worms/pools/viewports). Because the scenario is fixed-seed and input-free,
each loop is **bit-identical** to the last — a clean, deterministic, indefinitely-running
demo. (Freezing the last frame was considered and rejected as more boring and no simpler.)

### Asset path resolution — CARGO_MANIFEST_DIR, not CWD

The loader resolves the TC root as a **compile-time** constant relative to the crate, the
same convention the tests already use:

```
concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero")
```

From `rust/game` this resolves to `<repo-root>/data/TC/openliero` — identical to the
oracle-tests root. This makes `cargo run -p game` work from any working directory (no CWD
dependency), which is the right ergonomics for a dev `cargo run`. (Shipped/wasm builds
embed assets — 3f, out of scope.)

---

## Camera question — fixed camera (faithful) vs follow-cam

The golden scenarios keep every worm at `killed_timer == 150` (the `WormInit` default), so
`Viewport::Process` takes neither centering arm and the camera is **pinned at (0,0)**,
window `[0,158)²` (the T7/T8 "fixed-camera invariant", `render_slice3b_common/mod.rs:17-23`).

- **Option A — fixed camera (RECOMMENDED default).** Run the scenario exactly as the
  golden. The worms and their action are already inside the visible window (they stand on
  the `render_stage` floor at y=120 and fire), so the demo is lively enough for a smoke.
  Faithful to the corpus, and it **enables the free determinism self-check** (the demo's
  per-tick sim-hash equals the golden's `state_hash` column — see below). No divergence
  caveat.
- **Option B — follow-cam.** Zero `killed_timer` in the demo loader so
  `Viewport::Process` centers on the worm ("more Liero feel"). This is determinism-safe
  (the centering is viewport-local render state; it never touches `SimState`), **but** the
  demo render then diverges from every golden, so it is **no longer bit-comparable** and
  the self-check must be dropped.

**Recommendation:** ship **Option A** as the default (faithful + free safeguard), and note
follow-cam as a deliberate later toggle (`--follow`) that trades golden-comparability for a
tracking camera. Since 3c is not bit-gated, Option B is *allowed* — but defaulting to A
buys the determinism guard at zero cost.

---

## Determinism protection

3c is not bit-gated, but it must not perturb the sim. Two guards:

1. **Architectural firewall (hard rule).** The Bevy layer mutates `SimState` **only**
   inside the single `FixedUpdate` `tick_and_render` system, and **only** via
   `process_frame`. No other system touches `Sim`. No `f32`/`Vec2`/`Transform`/Bevy value
   ever flows into the sim — Bevy types are one-directional consumers of the fixed-point
   sim (overview locked-decision 4, breakdown float-boundary rule). `render::frame::draw`
   already takes `&SimState` (immutable) — isolation is enforced by the type.
2. **Debug-only golden self-check (nearly free).** Because the default scenario is a
   golden, load its `state_hash` column at startup and
   `debug_assert_eq!(hash_game_state(&sim), golden[tick])` each tick. This catches any
   accidental sim regression from the Bevy wiring (wrong input, double-tick, reordered
   systems) the moment it happens, at zero release/wasm cost (`#[cfg(debug_assertions)]`).
   Drop it automatically under Option B (follow-cam) since the sim path is unchanged but
   the intent is a non-golden run — actually the *sim* hash is still comparable under
   Option B (follow-cam only changes viewport centering), so the check can stay; only the
   *frame* would differ. Keep the sim self-check in both modes.

The viewport-local `Rand` (laser sparks) is display-only, re-seeded fresh by the loader and
by each loop — it never feeds the sim.

---

## Verification

3c is **not bit-gated** — no new frame golden, no dumper change.

- **Local milestone (the real acceptance):** John runs `cargo run -p game`, sees the
  world view playing live with crisp integer scaling, closes the window. This human
  eyeball is the 3c milestone (iteration §1b/§4: the native window is the primary dev
  surface).
- **Smoke:** `cargo build -p game` compiles (locally and in CI). Optionally, a local
  `timeout`-bounded `cargo run` "does it start" check — but a *windowed* Bevy run needs a
  display session, so this is **not** a reliable headless CI check; real headless is 3d.
- **CI (`.github/workflows/rust.yml`):** the existing gate runs
  `cargo test --manifest-path rust/Cargo.toml --workspace`. `cargo test --workspace`
  **compiles every binary target in every member** (binaries without tests are still
  built), so simply adding `game` to `members` makes CI compile the whole Bevy tree on
  every Rust CI run — coupling the fast determinism gate to Bevy's compile time and pulling
  wgpu/winit onto the runner.
  **Recommendation:** keep the determinism gate fast and Bevy-independent by running it as
  `cargo test --workspace --exclude game`, and add a **separate** `cargo build -p game`
  step (own cache) so `game` compilation is still gated without slowing or destabilizing
  the sim/render tests. This also isolates any future Bevy-toolchain breakage from the
  hard-gate job. *(Confirm the trimmed feature set builds on stock `ubuntu-latest` with no
  extra apt packages — expected, since audio/gilrs are dropped; if a system lib is missing,
  add it to that one build step, never to the test job.)*
- **Wasm features must not leak:** `game`'s default features must not enable `webgl2`/
  `webgpu` or `dynamic_linking` (3f / dev-only).

---

## Test list

Almost all of 3c is smoke (window/camera/upload). What is genuinely unit-testable:

1. **`blit_surface_into_image` byte order (the one load-bearing unit test).** Build a tiny
   `Bitmap` with known ARGB `u32`s (e.g. `0xFF_2A_00_00`) and assert the produced `Vec<u8>`
   is `[0x2A, 0x00, 0x00, 0xFF]` (`[R,G,B,A]`), including a pixel with a non-FF alpha and a
   `pitch != w` surface (guard against a `w`-vs-`pitch` stride bug, mirroring the
   `bitmap.rs` tests). Catches the classic swapped-channel bug.
2. **Loop wrap (`reload`) logic.** A pure `next_tick`/`reload` helper: after `scenario.ticks`
   the tick resets to 0 and the sim is rebuilt to a fresh tick-0 hash equal to the first
   run's tick-0 hash (deterministic loop). Testable without Bevy.
3. **Loader sanity** (mostly covered by moving the parser + `oracle-tests` T8 goldens over
   the shared `scenario::load`): a `game`- or `scenario`-level test that `load` returns a
   `SimState` with the expected worm count and a tick-0 `hash_game_state` matching the
   demo scenario's golden line 0. This doubles as proof the shared loader didn't drift when
   factored out of `build()`.

Everything else — window creation, `Image` upload, `Camera2d`, nearest scaling, the
FixedUpdate cadence (Bevy's `Time<Fixed>`, no custom accumulator) — is smoke, validated by
the milestone run.

---

## Risks & the hard 10%

- **CI compile-time / dependency blast radius.** Adding `game` to the workspace makes
  `cargo test --workspace` pull and build the entire Bevy tree. Mitigated by
  `--exclude game` + a dedicated `cargo build -p game` step (Verification).
- **Cargo.lock explosion.** Hundreds of new transitive crates; large lock diff is
  expected — flag it in the PR.
- **First-build compile time.** Clean Bevy build is minutes; incremental is fine;
  `--features dynamic` keeps John's inner loop sub-second (dev-only).
- **Bevy 0.19 2D-internals churn.** Build only against the stable public
  `Sprite`/`Image`/`Camera2d`/`ImageSampler` surface, never `bevy_render`-internal types
  (bevy-research §1). All Bevy risk is confined to `game`; `render` is immune.
- **macOS main-thread requirement.** winit must own the main thread; `App::run()` handles
  this — no action needed. A windowed run needs a GUI session (fine on John's Mac; not a
  headless CI check — that's 3d).
- **Texture-format gamma.** `Rgba8UnormSrgb` vs `Rgba8Unorm` changes on-screen color of the
  VGA palette. Advisory only (CPU hash is the gate); resolve by eyeball at implementation.
- **Per-frame Image mutation cost.** 256 KB/frame @ 71 Hz ≈ 18 MB/s — negligible
  (bevy-research §2a).

---

## Deferrals (explicitly out of 3c)

- Keyboard input / game-loop-with-input — **Step 4**.
- Audio — **Step 4**.
- Headless screenshot CLI + in-repo run/observe skill — **3d** (3c must not block it; the
  shared `scenario::load` is exactly what 3d reuses).
- HUD / font / bars / minimap — **3e**.
- Wasm (WebGL2, embedded assets) — **3f**; no wasm feature pulled here.
- Render interpolation (`overstep_fraction` lerp) — overview deferral; draw the latest tick.
- Resize-aware integer-fit camera — nice-to-have; fixed 3× window for 3c.
- Follow-cam as default (Option B) — allowed but not the default; ships as a later toggle.

---

## Open questions for the controller (max 3)

1. **Scenario-sharing scope.** Ratify the new Bevy-free `scenario` crate owning **both**
   the parser (moved from `oracle-tests`, re-exported for compat) **and** the
   `load(tc_root, &Scenario) -> (SimState, [Viewport;2], SceneData)` loader factored out of
   the T8 `build()` — vs the lighter "move only the parser, duplicate an ~80-line loader in
   `game`." (Recommendation: the shared crate — 3d reuses the loader and it de-dups the
   harness.)
2. **CI shape.** Accept `cargo test --workspace --exclude game` + a separate
   `cargo build -p game` step (keeps the determinism gate fast and Bevy-independent), vs
   letting the existing `--workspace` test compile `game` inline (simpler, but couples the
   hard gate to Bevy compile time)? (Recommendation: `--exclude` + separate build step.)
3. **Demo camera default.** Confirm **Option A (fixed camera, faithful to the golden +
   free sim-hash self-check)** as the 3c default, with follow-cam (Option B) a later
   `--follow` toggle — or prefer follow-cam now for "Liero feel" at the cost of
   golden-comparability? (Recommendation: Option A.)
