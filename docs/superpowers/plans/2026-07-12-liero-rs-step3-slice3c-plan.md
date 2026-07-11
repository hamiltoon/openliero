# Step 3, Slice 3c — Bevy window (native): Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax. **Test-first**: write the failing test (or its assertion) before the
> implementation it pins, run it and SEE it fail, then make it pass. Where a Bevy 0.19 API cannot be
> verified without a build, the step is marked **[verify at implementation]** with a fallback: adjust
> to the real 0.19 signature and record the delta in the task's done-report — never paste a guessed
> signature as if it were certain.

**Goal:** Stand up the project's **first Bevy code** — a new `game` binary crate such that
`cargo run -p game` opens a window that **plays a deterministic, fixed-seed scenario in real time**
(no input — input is Step 4), nearest-neighbor integer-scaled ×3 in a fixed 960×600 window, closable
via Esc / the close button, exiting cleanly. It proves the Bevy-free `render` crate (shipped pixel-
exact in 3a+3b) runs **live natively** and that the Bevy tick→render→present loop is correct and
determinism-safe. 3c is **NOT bit-gated** — the hard gate is the CPU frame hash, already green in 3b;
3c is *presentation*. The one load-bearing new unit is the ARGB→RGBA blit (byte-order-sensitive), plus
the deterministic loop-by-reload. A **debug-only** per-tick `hash_game_state` self-check against the
committed blood golden guards the determinism firewall at zero release cost.

**Architecture:** Two new crates.
1. A new **Bevy-free library crate `rust/scenario`** that owns (a) the scenario **parser** (moved
   verbatim from `oracle-tests/src/scenario.rs`, all 40+ parser tests move with it) and (b) a
   **loader** `load(tc_root, &Scenario) -> Loaded` factored verbatim out of the T8 harness
   `render_slice3b_common::build()`. Deps: `sim`, `assets`, `render`, `sim-core` — all Bevy-free, so
   `game` stays one dependency-hop from Bevy. `oracle-tests` re-exports it so the ~30 existing
   `oracle_tests::scenario::Scenario` references compile **unchanged**; every committed golden test
   stays green (the proof the factor-out did not drift).
2. A new **binary crate `rust/game`** — the **only** Bevy crate (Bevy 0.19, `default-features=false` +
   an explicit minimal feature list, edition 2024). It holds the sim in a `Resource`, ticks it in one
   `FixedUpdate` system at `1000/14 Hz`, renders each tick with `render::frame::draw` into an owned
   `Bitmap`, blits that into a Bevy `Image`, and shows it as one ×3-scaled `Sprite` under a `Camera2d`.
   `render`/`sim`/`assets`/`sim-core`/`scenario` stay Bevy-free and unchanged.

Crate graph after 3c (no cycle; all arrows except `game` are Bevy-free):

```
sim-core ◄ assets ◄ sim ◄ render
                      ▲       ▲
                      └── scenario ──┘        (NEW lib: deps sim, assets, render, sim-core)
                            ▲     ▲
                          game    oracle-tests(dev-dep, re-exports scenario)
                        (NEW bin: + bevy 0.19)
```

**Tech stack:** Rust. `scenario` + `game` new crates. Bevy 0.19 (`bevy_sprite`/`bevy_winit`/
`bevy_window` + Linux `x11`/`wayland`), pinned by `Cargo.lock` (large, expected lock diff — hundreds of
transitive crates). First clean build is **minutes** (whole Bevy tree) — every `cargo build/run -p game`
in this plan needs `timeout: 600000`. Real acceptance is the **local milestone run** on John's Mac
(eyeball the live window); CI proves only `cargo build -p game` compiles.

## Global constraints

*(inherit every 3a/3b constraint that still applies; the 3c-specific ones follow)*

- **Refactor must not change behavior (hard gate).** Moving the parser + factoring the loader is a
  **pure re-home**. Every existing `oracle-tests` golden/fuzz test (`sim_slice*`, `render_slice3a`,
  `render_slice3b_*`) must be GREEN **unchanged**, and **no golden file is touched**. The proof is
  `cargo test --manifest-path rust/Cargo.toml --workspace` staying green after T0 with an **empty**
  `git diff -- rust/oracle-tests/golden`. If any golden test needs its assertions edited, the loader
  drifted — STOP and reconcile against `build()` verbatim.
- **Minimal-diff compatibility for the parser move.** Prefer a **crate re-export** in
  `oracle-tests/src/lib.rs` (`pub use scenario;`) so all ~30 `oracle_tests::scenario::Scenario`
  references (see T0 list) compile with ZERO edits. Fallback if the re-export path does not resolve:
  bulk-replace `oracle_tests::scenario` → `scenario` across the test files and add `scenario` as a
  dev-dep import — also valid, larger diff. Pick the re-export first.
- **`scenario` crate is edition 2021, NO Bevy.** Deps exactly `sim`, `assets`, `render`, `sim-core`
  (all path). `cargo tree -p scenario` must show no `bevy*`. The loader is a verbatim move of `build()`
  — same reads, same `resolve_weapons`, same `weapon 0` override applied to BOTH worms, same post-`new`
  TC-scalar assignments, same `killed_timer`-left-at-150 fixed-camera invariant.
- **`game` is the ONLY Bevy crate; edition 2024 / Rust ≥ 1.95.** `bevy = "0.19"` (caret; patch pinned by
  `Cargo.lock` — do NOT hard-pin `=0.19.x`). `default-features = false` + the exact feature floor
  below. Build **only** against the stable public `Sprite`/`Image`/`Camera2d`/`ImageSampler`/`Time<Fixed>`
  surface — never `bevy_render`-internal types (bevy-research §1: 2D internals may be reworked; confine
  all that risk to `game`). The other crates stay edition 2021 and unchanged (mixed editions per member
  are fine).
- **Determinism firewall (hard rule).** The Bevy layer mutates `SimState` **only** inside the single
  `FixedUpdate` `tick_and_render` system, and **only** via `process_frame`. No other system touches the
  `Sim` resource. No `f32`/`Vec2`/`Transform`/Bevy value ever flows *into* the sim — Bevy types are
  one-directional consumers of the fixed-point sim (overview locked-decision 4). `render::frame::draw`
  already takes `&SimState` (immutable), so isolation is enforced by the type.
- **One draw per tick — tick + render live in ONE `FixedUpdate` system.** Interpolation is OFF (draw
  the latest tick). Rendering exactly once per tick keeps the viewport-local laser-spark RNG advancing
  once per draw, identical to the golden's one-draw-per-tick cadence, so the self-check stays
  meaningful and a slow render frame (two `FixedUpdate` runs) cannot double-advance the viewport RNG.
  **Do NOT also render in `Update`.**
- **Fixed camera (Option A, controller-ratified default).** Run the demo scenario exactly as the
  golden: every worm keeps `killed_timer == 150` (`SimState::new` default), so `viewport::process`
  takes neither centering arm and the camera is pinned at (0,0), window `[0,158)²`. This is faithful to
  the corpus AND enables the free per-tick sim-hash self-check. Follow-cam (Option B) is a deliberate
  later `--follow` toggle — **not** built here.
- **Default scenario `blood`, loop by re-seed.** Default demo is `blood` (a committed 3b golden: DART
  into own feet, blood pool + carving, worm0 survives ~95 ticks — the liveliest corpus entry, and a
  golden so the self-check is free). When `tick` passes `scenario.ticks`, **rebuild from the loader at
  tick 0** (re-seed the `Rand`, reset worms/pools/viewports). Fixed-seed + input-free ⇒ every loop is
  bit-identical. A `cargo run -p game -- <name>` positional arg to pick another committed scenario is
  built in T4 (optional per done-when, but cheap).
- **Debug-only golden self-check (nearly free).** Because the default scenario is a golden, load its
  per-tick `state_hash` column at startup and
  `#[cfg(debug_assertions)] debug_assert_eq!(hash_game_state(&sim.0), golden[tick])` each tick. Zero
  release/wasm cost. Keep it in both camera modes (the sim hash is comparable even under follow-cam;
  only the *frame* would differ).
- **CARGO_MANIFEST_DIR paths, not CWD.** The loader resolves the TC root as a compile-time constant
  relative to the crate: `concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero")` from
  `rust/game` (and from `rust/oracle-tests`, unchanged). Committed scenario text + the self-check
  golden are read from `concat!(env!("CARGO_MANIFEST_DIR"), "/../oracle-tests/golden/…")` — a dev
  convenience; shipped/wasm embedding is 3f. `cargo run -p game` works from any CWD.
- **No wasm / no dev-linking leak.** `game`'s default features must NOT enable `webgl2`/`webgpu`
  (wasm is 3f) or `dynamic_linking` (dev-only). `dynamic_linking` is exposed only as an opt-in
  passthrough `dynamic` feature for John's inner loop (`cargo run -p game --features dynamic`); CI and
  3f must never set it.
- **CI keeps the determinism gate fast + Bevy-independent.** The hard gate runs
  `cargo test --manifest-path rust/Cargo.toml --workspace --exclude game` (so `cargo test --workspace`
  does NOT compile the Bevy tree into the fast sim/render gate), plus a **separate**
  `cargo build --manifest-path rust/Cargo.toml -p game` step (own cache) so `game` compilation is still
  gated. The window is never launched in CI.
- **`cargo fmt` footgun (T8 lesson).** Format **only new files** — `cargo fmt -p scenario` /
  `cargo fmt -p game` (or `rustfmt` on the specific new files). Do NOT run a workspace-wide `cargo fmt`
  (it would reformat unrelated crates and pollute the diff).
- **Bash discipline.** One command per call; no `>>`/heredoc/`&&`/`;`/`$VAR`; no `cd`+`git`; create
  files with the editor. First `cargo build -p game` needs `timeout: 600000`.
- **No AI / "Generated with" taglines; no commit trailers** (no `Co-Authored-By`, no
  `Claude-Session`). **Commit on branch `liero-rs-step-3`** (the accumulating Step-3 PR). **Do NOT push
  and do NOT open a PR** — the controller owns push + PR.

## Preflight (before T1)

- [ ] Confirm the local toolchain builds edition 2024 + Bevy: `rustc --version` must be **≥ 1.95.0**.
      If lower, `rustup update stable`. (CI's `dtolnay/rust-toolchain@stable` is ≥ 1.95 by 2026-07.)
      The `scenario` crate (T0) is edition 2021 and does not need this; only `game` (T1+) does.

## File structure

- **New crate `rust/scenario/`** (T0):
  - `rust/scenario/Cargo.toml` — lib, edition 2021, deps `sim`/`assets`/`render`/`sim-core` (path).
  - `rust/scenario/src/lib.rs` — `pub mod parser; pub mod loader;` + re-exports (`pub use parser::*;`).
  - `rust/scenario/src/parser.rs` — the parser moved **verbatim** from
    `rust/oracle-tests/src/scenario.rs` (incl. its `#[cfg(test)] mod tests`).
  - `rust/scenario/src/loader.rs` — `load(tc_root, &Scenario) -> Loaded`, `Loaded`, `SceneData`,
    factored verbatim out of `render_slice3b_common::build()`.
- `rust/Cargo.toml` — add `"scenario"` and `"game"` to `members`.
- `rust/oracle-tests/Cargo.toml` — add `scenario = { path = "../scenario" }` to `[dev-dependencies]`.
- `rust/oracle-tests/src/lib.rs` — replace `pub mod scenario;` with `pub use scenario;` (re-export);
  delete `rust/oracle-tests/src/scenario.rs`.
- `rust/oracle-tests/tests/render_slice3b_common/mod.rs` — `build()` collapses to a thin wrapper over
  `scenario::load` (T0).
- **New crate `rust/game/`** (T1–T4):
  - `rust/game/Cargo.toml` — bin, edition 2024, Bevy 0.19 trimmed + `render`/`sim`/`assets`/`sim-core`/
    `scenario` (path); `dynamic` passthrough feature.
  - `rust/game/src/main.rs` — the Bevy app (window, resources, `setup`, `tick_and_render`, camera/sprite).
  - `rust/game/src/blit.rs` — pure `blit_surface_into_bytes` (ARGB→RGBA) + loop `next_tick` helper +
    their unit tests (Bevy-free, `cargo test -p game`).
- `rust/Cargo.lock` — regenerated with the whole Bevy tree (T1).
- `.github/workflows/rust.yml` — `--exclude game` on the test step + a separate `cargo build -p game`
  step (T4).
- `docs/superpowers/liero-rs-PROGRESS.md` + the rendering overview's 3c line + deferral ledger (T5).

## Tasks

---

### T0 — new `scenario` crate: move parser + factor loader out of `build()`; oracle-tests green unchanged  [Opus]

**Files**
- Create: `rust/scenario/Cargo.toml`, `rust/scenario/src/lib.rs`, `rust/scenario/src/parser.rs`,
  `rust/scenario/src/loader.rs`
- Modify: `rust/Cargo.toml` (members), `rust/oracle-tests/Cargo.toml` (dev-dep),
  `rust/oracle-tests/src/lib.rs` (re-export), `rust/oracle-tests/tests/render_slice3b_common/mod.rs`
  (`build()` → wrapper)
- Delete: `rust/oracle-tests/src/scenario.rs`

**Interfaces**
- Consumes: the existing parser (`rust/oracle-tests/src/scenario.rs`) verbatim; the exact `build()`
  body (`render_slice3b_common/mod.rs:129-233`) — every read, `resolve_weapons`, the `weapon 0`
  override applied to BOTH worms, `SimState::new(...)`, the post-`new` TC-scalar assignments
  (`num_blood_colours`, `first_blood_colour`, `bobj_gravity`, `small_sprites`, the `worm_spawn_rect_*`,
  `worm_min_spawn_dist_*`, `game_mode`), `build_fire_cone_sprites`, `Viewport::player_layout()`.
- Produces (in `rust/scenario/src/loader.rs`):
  ```rust
  use std::path::Path;
  use assets::palette::Palette;
  use assets::sprite::SpriteSet;
  use assets::tc::ColorAnim;
  use render::frame::Scene;
  use render::viewport::Viewport;
  use sim::state::SimState;

  /// Owned Scene ingredients — everything the per-tick `render::frame::draw` needs
  /// besides the surface, the SimState, and the viewports. Mirrors the `Built`
  /// fields the T8 harness carried inline.
  pub struct SceneData {
      pub origpal: Palette,
      pub color_anim: Vec<ColorAnim>,
      pub fire_cone: SpriteSet,
      pub nr_begin: i32,
      pub nr_end: i32,
      pub laser_weapon: i32,
  }

  impl SceneData {
      /// Borrow the owned ingredients into a `render::frame::Scene` for one draw.
      /// `screen_flash`/`draw_shadow` are per-draw (demo passes 0 / scenario.shadow()).
      pub fn as_scene(&self, screen_flash: i32, draw_shadow: bool) -> Scene<'_> {
          Scene {
              origpal: &self.origpal,
              color_anim: &self.color_anim,
              fire_cone_sprites: &self.fire_cone,
              bonus_frames: &[],
              nr_begin: self.nr_begin,
              nr_end: self.nr_end,
              laser_weapon: self.laser_weapon,
              screen_flash,
              draw_shadow,
          }
      }
  }

  /// The full tick-0 load: driven `SimState`, the two fixed-camera viewports
  /// (fresh default-seeded RNG), and the owned Scene ingredients.
  pub struct Loaded {
      pub state: SimState,
      pub viewports: [Viewport; 2],
      pub scene: SceneData,
  }

  /// Verbatim factor-out of `render_slice3b_common::build()`. `tc_root` is the TC
  /// directory (`data/TC/openliero`); `scenario` is the already-parsed scenario.
  pub fn load(tc_root: &Path, scenario: &Scenario) -> Loaded { /* moved body */ }
  ```
- Re-export in `rust/scenario/src/lib.rs`:
  ```rust
  pub mod loader;
  pub mod parser;
  pub use loader::{load, Loaded, SceneData};
  pub use parser::{Scenario, ScenarioWorm};
  ```
  So `scenario::Scenario` and `scenario::load` both resolve.

**Why (teaching note):** This is a **strangler** move — the load path already exists inside a test
harness; we lift it into a real library so `game` (3c) and the headless PNG CLI (3d) can both consume
it without depending on `oracle-tests` (a `game → oracle-tests` dep is architecturally backwards and
impossible anyway — `oracle-tests` has only `[dev-dependencies]`). The parser is std-only, so it moves
with no dependency change; the loader pulls `sim`/`assets`/`render`/`sim-core`, all Bevy-free, keeping
`game` exactly one hop from Bevy. The **goldens are the regression proof**: because the loader is a
byte-for-byte move of `build()`, every `render_slice3b_*` and `sim_slice*` test must stay green with
zero golden edits — that green run *is* the "did the factor-out drift?" test.

**Steps**

- [ ] Create `rust/scenario/Cargo.toml` (lib, edition 2021, `[dependencies]` sim/assets/render/sim-core
      by path). Add `"scenario"` to `rust/Cargo.toml` `members` (before `"game"`; `"game"` is added in
      T1). Add `scenario = { path = "../scenario" }` to `rust/oracle-tests/Cargo.toml`
      `[dev-dependencies]`.
- [ ] **Move the parser verbatim.** Copy `rust/oracle-tests/src/scenario.rs` → `rust/scenario/src/parser.rs`
      **unchanged** (module doc, `Scenario`, `ScenarioWorm`, all methods, and the whole
      `#[cfg(test)] mod tests` with its 40+ cases). Delete `rust/oracle-tests/src/scenario.rs`.
- [ ] **Re-export for compat.** In `rust/oracle-tests/src/lib.rs` replace `pub mod scenario;` with
      `pub use scenario;` (re-exports the crate so `oracle_tests::scenario::Scenario` still resolves —
      zero edits to the ~30 referencing test files listed below).
      References that must keep compiling unchanged (`use oracle_tests::scenario::Scenario;`):
      `render_slice3a_golden.rs`, `render_slice3b_common/mod.rs`, `sim_slice2/3/4a/4b/4c/4d/5a/5b/5c/5d`
      `_golden.rs`, `sim_slice5prime*_golden.rs`, `sim_slice5prime*_fuzz.rs`, `sim_slice6*_golden.rs`,
      `sim_slice6_fuzz.rs`, and the `examples/` (`render_slice5*`, `explore_*`, `gen_slice6_fuzz`).
      **[verify at implementation]** that `pub use scenario;` resolves the `oracle_tests::scenario::…`
      path under edition 2021; **fallback:** bulk-replace `oracle_tests::scenario` → `scenario` across
      those files (add `use scenario::Scenario;`) and document the wider diff in the done-report.
- [ ] **Verify the parser move in isolation first:** `cargo test -p scenario` → the 40+ parser tests
      pass in their new home. (No behavior change; they moved verbatim.)
- [ ] **Factor the loader.** Create `rust/scenario/src/loader.rs` with `Loaded`/`SceneData`/`load` as
      above. **Move the body of `build()` verbatim** (`render_slice3b_common/mod.rs:129-233`): the
      `load_sprites` helper (make it a private fn in `loader.rs` taking `tc_root`), the `small.tga`
      origpal read, the level read, `tc.cfg` parse, `Objects::load`, `weap_order` sort +
      `resolve_weapons`, the `weapon 0 <name>` override applied to **both** worms (keep the
      `.expect("scenario has a weapon 0 directive")`), `worms_init`, `SimState::new(...)`, the post-`new`
      TC-scalar assignments, `build_fire_cone_sprites`, and `Viewport::player_layout()`. `TC_ROOT` is no
      longer a const inside the loader — it is the `tc_root: &Path` argument. Preserve the
      `killed_timer`-left-at-150 comment (the fixed-camera invariant). Do NOT change any constant, order,
      or read.
- [ ] **Collapse `build()` to a wrapper.** Rewrite `render_slice3b_common::build()` to: read the
      `_scenario.txt` golden (unchanged, still test-side), parse it, `assert_eq!(scenario.seed, 42)`,
      call `let loaded = scenario::load(Path::new(TC_ROOT), &scenario);`, and repackage into the
      existing `Built` struct (`state`, `viewports`, `bmp: Bitmap::new(320,200)`, `origpal`,
      `color_anim`, `fire_cone`, `nr_begin`, `nr_end`, `laser_weapon`, `scenario`). Pull the fields from
      `loaded.state` / `loaded.viewports` / `loaded.scene`. Keep `TC_ROOT` in the harness for the golden
      reads. `render_tick` is unchanged (it builds a `Scene` inline; optionally it can call
      `loaded.scene.as_scene(screen_flash, draw_shadow)` — but to minimize churn, leaving the inline
      `Scene` literal is fine).
- [ ] **Behavior re-diff gate (hard):**
      `cargo test --manifest-path rust/Cargo.toml --workspace` → **all** green (every `sim_slice*`,
      `render_slice3a`, `render_slice3b_blood/dart/dart_water/…`). Then
      `git status --porcelain rust/oracle-tests/golden` and `git diff --stat -- rust/oracle-tests/golden`
      → **empty**. If any golden changed or any test needed an assertion edit, the loader drifted —
      STOP, diff against `build()` line-by-line, fix.
- [ ] `cargo tree -p scenario` shows no `bevy*` (Bevy-free) and no `oracle-tests` back-edge.
- [ ] `cargo fmt -p scenario` (new files only).
- [ ] Reviewer (Opus): parser moved verbatim (diff `parser.rs` vs old `scenario.rs` — identical);
      loader is a faithful move of `build()` (same reads/order/consts/override); `oracle-tests`
      compiles via the re-export with zero test-file edits (or documented fallback); **every golden
      byte-identical**; no cycle.
- [ ] **Commit:**
      - `git add rust/scenario rust/Cargo.toml rust/oracle-tests`
      - `git commit -m "scenario(3c): new Bevy-free crate — move parser + factor load() out of the T8 build()"`

---

### T1 — `game` crate skeleton: empty Bevy window + Cargo.lock  [Opus]

**Files**
- Create: `rust/game/Cargo.toml`, `rust/game/src/main.rs`
- Modify: `rust/Cargo.toml` (add `"game"` to members)
- Regenerate: `rust/Cargo.lock` (whole Bevy tree)

**Interfaces**
- Produces: a `game` binary whose `main` builds a Bevy `App` that opens ONE fixed 960×600 window titled
  `"Liero-rs — 3c demo"` and exits cleanly on Esc / close button. No sim yet — this task de-risks the
  Bevy dependency + feature set + Cargo.lock explosion **before** any wiring.

**`rust/game/Cargo.toml`** (feature list is the **floor** — see the verify note):
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
scenario = { path = "../scenario" }

bevy = { version = "0.19", default-features = false, features = [
    "bevy_sprite",   # sprites / Image / 2D pipeline (pulls bevy_render, bevy_core_pipeline)
    "bevy_winit",    # window + event loop
    "bevy_window",
    "x11",           # Linux windowing (CI + Linux dev); inert on macOS
    "wayland",       # Linux windowing (optional companion to x11)
] }

# Dev-only fast-iteration linking. NEVER a default; never enabled in CI or wasm.
[features]
dynamic = ["bevy/dynamic_linking"]
```

**Why (teaching note):** Bevy is modular — `default-features = false` + an explicit list compiles only
what "one fullscreen sprite in a window" needs, dropping `bevy_pbr`/3D, `bevy_ui` (we blit the HUD font
ourselves in 3e, C++-parity), `bevy_gltf`, `bevy_audio`, `bevy_gilrs`, and — critically — **no `png`**
(assets are parsed by the Bevy-free `assets` crate, not `AssetServer`) and **no `webgl2`/`webgpu`**
(wasm is 3f; pulling it now drags the wasm hazard surface into a native slice). `x11`/`wayland` gate
Linux-only deps and are inert on macOS; enabling them unconditionally lets Linux CI and John's Mac both
build with no extra system packages (a trimmed Bevy with no audio/gilrs needs no alsa/udev headers).

**Steps**

- [ ] Add `"game"` to `rust/Cargo.toml` `members`.
- [ ] Create `rust/game/Cargo.toml` (above) and a minimal `rust/game/src/main.rs`:
      ```rust
      //! Liero-rs `game` binary — the project's first Bevy code (Slice 3c).
      //! T1: an empty 960×600 window that closes on Esc / the close button. The sim
      //! wiring lands in T3. Bevy 0.19; the ONLY Bevy crate in the workspace.
      use bevy::prelude::*;
      use bevy::window::WindowResolution;

      fn main() {
          App::new()
              .add_plugins(
                  DefaultPlugins
                      // Global nearest-neighbor sampling for crisp integer upscaling.
                      .set(ImagePlugin::default_nearest())
                      .set(WindowPlugin {
                          primary_window: Some(Window {
                              resolution: WindowResolution::new(960.0, 600.0),
                              title: "Liero-rs — 3c demo".into(),
                              resizable: false,
                              ..default()
                          }),
                          ..default()
                      }),
              )
              .add_systems(Update, close_on_esc)
              .run();
      }

      /// Esc quits. The window's close button already exits via winit.
      fn close_on_esc(keys: Res<ButtonInput<KeyCode>>, mut exit: EventWriter<AppExit>) {
          if keys.just_pressed(KeyCode::Escape) {
              exit.write(AppExit::Success);
          }
      }
      ```
      **[verify at implementation]** — resolve each against real 0.19 before declaring green; fallback
      is to use the actual 0.19 signature and note the delta:
      - `DefaultPlugins` may require companion features not in the floor list (e.g. `bevy_core_pipeline`,
        `bevy_asset`, `bevy_log`, `multi_threaded`, `bevy_state`). **If the build errors on a missing
        plugin/feature, ADD the minimal feature(s) to make `DefaultPlugins` compile** and record each
        addition (with why) in the done-report. The floor list is a starting point, not a proven-complete
        set.
      - `WindowResolution::new` / `Window { resolution, title, resizable, .. }` field names.
      - `ImagePlugin::default_nearest()` as a `DefaultPlugins.set(...)` argument.
      - `ButtonInput<KeyCode>` resource + `KeyCode::Escape`; `EventWriter<AppExit>::write` vs `send`;
        `AppExit::Success` variant (0.19 made `AppExit` an enum). Adjust to whatever 0.19 exposes.
- [ ] **Regenerate `Cargo.lock`.** Run `cargo build --manifest-path rust/Cargo.toml -p game`
      (`timeout: 600000` — first clean Bevy build is minutes). This pulls hundreds of transitive crates
      and rewrites `rust/Cargo.lock` with the whole Bevy tree. The large lock diff is **expected**.
- [ ] **Build-smoke (the gate):** the command above exits 0.
- [ ] **Optional local run-smoke (John's Mac, best-effort — NOT CI):** `timeout 8 cargo run -p game`.
      A window appears; **exit code 124 = ran fine until the timeout killed it** (mirrors the C++
      `SDL_VIDEODRIVER=dummy timeout 8 … exit 124 = ran fine` pattern). A **non-124 non-zero** exit that
      happens *before* the window opens is a real failure (panic on plugin/adapter init). A windowed run
      needs a GUI session, so this check is local-only; real headless is 3d. Document the observed exit
      code in the done-report.
- [ ] `cargo fmt -p game` (new files only).
- [ ] Reviewer (Opus): feature list is `default-features=false` + the floor (+ any documented additions);
      no `webgl2`/`webgpu`/`dynamic_linking` in defaults; `dynamic` is opt-in passthrough only; the lock
      diff is Bevy-tree churn (not unrelated crates); window is fixed 960×600, titled, closable.
- [ ] **Commit (crate + lock together — the lock diff belongs with the crate that caused it):**
      - `git add rust/game rust/Cargo.toml rust/Cargo.lock`
      - `git commit -m "game(3c): new Bevy 0.19 binary crate — empty 960x600 window skeleton"`

---

### T2 — pure blit + loop helpers (`blit.rs`) with unit tests  [Opus]

**Files**
- Create: `rust/game/src/blit.rs`
- Modify: `rust/game/src/main.rs` (add `mod blit;`)

**Interfaces**
- Consumes: `render::bitmap::Bitmap` (`pixels: Vec<u32>` packed `0xAARRGGBB`, `pitch` in pixels,
  `w`/`h`).
- Produces (Bevy-free, unit-tested via `cargo test -p game`):
  - `blit_surface_into_bytes(bmp: &Bitmap, out: &mut [u8])` — writes `w*h*4` RGBA8 bytes `[R,G,B,A]`,
    honoring `pitch != w`.
  - `next_tick(tick: u32, ticks: u32) -> (u32, bool)` — loop step: returns `(tick+1, false)` normally,
    `(0, true)` when the increment would pass `ticks` (signal a reload).

**Why (teaching note):** This is the single genuinely load-bearing new logic in 3c and it is
**byte-order-sensitive**: `render`'s `Bitmap` packs `pal32` as `0xFF000000 | r<<16 | g<<8 | b`
(`palette.rs:36`), but a Bevy `Image` in `Rgba8*` wants a `Vec<u8>` in **byte** order `[R, G, B, A]`.
Get the shuffle wrong and the whole window is channel-swapped / blue-tinted. Isolating it as a pure
`&Bitmap -> &mut [u8]` function (no Bevy types) makes it directly unit-testable — mirroring how
`render`'s own `bitmap.rs` tests guard the `pitch`-vs-`w` stride. Keeping `next_tick` pure lets us prove
the deterministic loop without spinning up a window.

**Steps**

- [ ] Add `mod blit;` to `main.rs`.
- [ ] **RED:** create `rust/game/src/blit.rs` with the two functions (bodies `unimplemented!()`) and a
      test module; run `cargo test -p game blit` → FAIL:
      ```rust
      #[cfg(test)]
      mod tests {
          use super::*;
          use render::bitmap::Bitmap;

          #[test]
          fn argb_u32_becomes_rgba_byte_order() {
              // One pixel 0xFF_2A_00_00 (A=FF, R=2A, G=00, B=00) -> [0x2A,0,0,0xFF].
              let mut bmp = Bitmap::new(1, 1);
              bmp.pixels[0] = 0xFF_2A_00_00;
              let mut out = vec![0u8; 4];
              blit_surface_into_bytes(&bmp, &mut out);
              assert_eq!(out, [0x2A, 0x00, 0x00, 0xFF], "[R,G,B,A]");
          }

          #[test]
          fn non_ff_alpha_and_all_channels_round_trip() {
              // 0x80_11_22_33 -> R=0x11 G=0x22 B=0x33 A=0x80.
              let mut bmp = Bitmap::new(1, 1);
              bmp.pixels[0] = 0x80_11_22_33;
              let mut out = vec![0u8; 4];
              blit_surface_into_bytes(&bmp, &mut out);
              assert_eq!(out, [0x11, 0x22, 0x33, 0x80]);
          }

          #[test]
          fn respects_pitch_greater_than_width() {
              // 2x1 image with pitch 4 (stride padding). Only the 2 real columns emit;
              // the destination is tightly packed w*h*4 = 8 bytes.
              let mut bmp = Bitmap::new(2, 1);
              bmp.pitch = 4; // stride wider than width (guard the w-vs-pitch bug)
              bmp.pixels = vec![0xFF_11_00_00, 0xFF_22_00_00, 0, 0]; // row: [px0, px1, pad, pad]
              let mut out = vec![0u8; 8];
              blit_surface_into_bytes(&bmp, &mut out);
              assert_eq!(out, [0x11, 0, 0, 0xFF, 0x22, 0, 0, 0xFF]);
          }

          #[test]
          fn next_tick_wraps_at_ticks() {
              assert_eq!(next_tick(0, 40), (1, false));
              assert_eq!(next_tick(39, 40), (40, false));
              assert_eq!(next_tick(40, 40), (0, true), "passing ticks signals reload");
          }
      }
      ```
      Note: `Bitmap::new(w,h)` sets `pitch = w`; the pitch test overrides `pitch` and re-sizes `pixels`
      to `pitch*h` — confirm `Bitmap`'s fields are `pub` (they are: `pixels`, `pitch`, `w`, `h`).
- [ ] **GREEN:** implement:
      ```rust
      //! Pure, Bevy-free helpers for the 3c demo: the ARGB→RGBA blit (the one
      //! byte-order-sensitive piece) and the deterministic loop step. Unit-tested
      //! without a window.
      use render::bitmap::Bitmap;

      /// Convert `render`'s `Bitmap` (`Vec<u32>` packed `0xAARRGGBB`, `bitmap.rs`)
      /// into tightly-packed RGBA8 bytes `[R,G,B,A]` for a Bevy `Image`. `out` must
      /// be `w*h*4` long. Honors `pitch` (source stride in pixels) != `w`.
      pub fn blit_surface_into_bytes(bmp: &Bitmap, out: &mut [u8]) {
          debug_assert_eq!(out.len(), (bmp.w * bmp.h * 4) as usize);
          for y in 0..bmp.h {
              for x in 0..bmp.w {
                  let px = bmp.pixels[(y * bmp.pitch + x) as usize];
                  let o = ((y * bmp.w + x) * 4) as usize;
                  out[o] = ((px >> 16) & 0xff) as u8; // R
                  out[o + 1] = ((px >> 8) & 0xff) as u8; // G
                  out[o + 2] = (px & 0xff) as u8; // B
                  out[o + 3] = ((px >> 24) & 0xff) as u8; // A
              }
          }
      }

      /// Loop step for the demo. Returns `(tick+1, false)` until the increment would
      /// pass `ticks`, then `(0, true)` — the caller rebuilds from the loader at 0.
      pub fn next_tick(tick: u32, ticks: u32) -> (u32, bool) {
          if tick + 1 > ticks { (0, true) } else { (tick + 1, false) }
      }
      ```
      `cargo test -p game blit` → GREEN.
- [ ] `cargo fmt -p game` (new file only).
- [ ] Reviewer (Opus): byte order is `[R,G,B,A] = [(px>>16)&0xff, (px>>8)&0xff, px&0xff, (px>>24)&0xff]`;
      source index uses `pitch`, destination uses `w` (tight); `next_tick` wraps exactly when the
      increment passes `ticks` (matching the spec's `tick += 1; if tick > ticks { reload }`).
- [ ] **Commit:**
      - `git add rust/game/src/blit.rs rust/game/src/main.rs`
      - `git commit -m "game(3c): pure ARGB->RGBA blit + loop next_tick with unit tests"`

---

### T3 — sim integration: resources, `FixedUpdate` tick→render→present, camera/sprite, debug self-check — MILESTONE  [Opus]

**Files**
- Modify: `rust/game/src/main.rs` (resources, `setup`, `tick_and_render`, self-check)

**Interfaces**
- Consumes: `scenario::{load, Loaded, SceneData}`, `scenario::Scenario`; `render::bitmap::Bitmap`,
  `render::frame::draw`; `sim::state::{SimState, ControlState}`, `sim::hash::hash_game_state`;
  `crate::blit::{blit_surface_into_bytes, next_tick}`.
- Produces: a running window that plays the `blood` scenario at `1000/14 Hz`, looping bit-identically,
  with a debug-only per-tick sim-hash assert against the committed golden.

**Resources** (no Bevy types inside `Sim` — the Step-5 `bevy_ggrs` rollback shape):
```rust
#[derive(Resource)]
struct Sim(SimState); // pure-Rust sim; NO Bevy types inside

#[derive(Resource)]
struct Demo {
    scenario: Scenario,        // for ticks + shadow()
    loaded: scenario::Loaded,  // viewports + SceneData (state lives in Sim)
    surface: Bitmap,           // owned 320x200 ARGB CPU buffer
    tick: u32,
    name: String,              // committed scenario name (for reload path); "blood" default
    #[cfg(debug_assertions)]
    golden: Vec<u32>,          // per-tick state_hash column for the self-check
}

#[derive(Resource)]
struct FrameImage(Handle<Image>); // the one Image the sprite samples
```
NB: `Loaded` owns `state: SimState`, but the ticking sim must live in `Sim` so `tick_and_render` mutates
it as a `ResMut<Sim>`. On load, **move** `loaded.state` into `Sim` and keep the rest (`viewports`,
`scene`) in `Demo.loaded`. Two clean options — pick at implementation and note it:
(a) store `viewports`/`scene` in `Demo` as separate fields and drop the wrapping `Loaded`, or
(b) keep `Loaded` but `std::mem::replace(&mut loaded.state, <cheap-placeholder>)` — **(a) is cleaner**;
prefer restructuring `Demo` to hold `viewports: [Viewport;2]` and `scene: SceneData` directly.

**Why (teaching note):** The sim is a plain resource that Bevy only *ticks and presents* — the layered
pattern bevy-research §4 recommends and the exact shape Step 5's `bevy_ggrs` rollback registration will
want (`tick_and_render` stays a thin, schedule-agnostic call into `sim`, so moving it into
`GgrsSchedule` later is a near-one-liner). `Time<Fixed>` gives us the fixed-timestep accumulator for
free — no hand-rolled timer. Mutating the `Image` via `Assets::get_mut` marks it dirty and re-uploads
to the GPU automatically (bevy-research §2a) — no manual dirty flag; 320×200×4 = 256 KB/frame @ 71.4 Hz
≈ 18 MB/s, trivially cheap.

**Steps**

- [ ] **Tickrate + startup + system wiring** in `main`:
      ```rust
      const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
      const GOLDEN_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../oracle-tests/golden");

      // in App::new():
      .insert_resource(Time::<Fixed>::from_hz(1000.0 / 14.0)) // C++ kDelay=14ms => ~71.43 Hz
      .add_systems(Startup, setup)
      .add_systems(FixedUpdate, tick_and_render)
      ```
      `1000/14 Hz` matches C++ `gfx.cpp:1176 kDelay = 14U` (one `processFrame` per loop). Determinism
      does not depend on the number (the sim advances by tick count, not wall-clock) — it only sets
      perceived speed. **[verify at implementation]** `Time::<Fixed>::from_hz` exists in 0.19 (research
      §4 confirms `from_hz`/`set_timestep_hz`); fallback `set_timestep_hz`.
- [ ] **`setup` (Startup):**
      1. Read + parse the scenario text:
         `render_slice3b_<name>_scenario.txt` from `GOLDEN_DIR` (name = "blood" for T3; the CLI arg is
         T4). Parse via `scenario::Scenario::parse`.
      2. `let loaded = scenario::load(Path::new(TC_ROOT), &scenario);`
      3. `let surface = Bitmap::new(320, 200);`
      4. Create the `Image`: **[verify at implementation]** against 0.19 (bevy-research §2a):
         ```rust
         let mut image = Image::new_fill(
             Extent3d { width: 320, height: 200, depth_or_array_layers: 1 },
             TextureDimension::D2,
             &[0, 0, 0, 255],
             TextureFormat::Rgba8UnormSrgb, // OR Rgba8Unorm — see the texture-format note
             RenderAssetUsages::all(),      // MAIN_WORLD mutate + RENDER_WORLD draw
         );
         image.sampler = ImageSampler::nearest();
         let handle = images.add(image);
         ```
         (`images: ResMut<Assets<Image>>`.) `Image::data` is `Option<Vec<u8>>` in 0.19 — the blit uses
         `.as_mut().unwrap()`. **Texture format is advisory** (the CPU hash is the gate, already green):
         if the palette looks gamma-wrong on John's Mac, switch `Rgba8UnormSrgb` ↔ `Rgba8Unorm` and note
         which rendered the VGA palette faithfully.
      5. Spawn camera + sprite ×3:
         ```rust
         commands.spawn(Camera2d);
         commands.spawn((
             Sprite::from_image(handle.clone()),
             Transform::from_scale(Vec3::splat(3.0)),
         ));
         ```
         **[verify at implementation]** `Camera2d` as a spawnable component and `Sprite::from_image` (or
         `Sprite { image: handle.clone(), ..default() }`) against 0.19; fallback to the struct-literal
         form. The CPU buffer IS the low-res canvas (bevy-research §2c), so one sprite at native 320×200
         scaled ×3 into a 960×600 window is all that's needed — no multi-camera `pixel_grid_snap` dance.
      6. Insert `Sim(loaded.state)` (moved) + `Demo { … }` + `FrameImage(handle)`.
      7. `#[cfg(debug_assertions)]` load the self-check golden: read
         `render_slice3b_<name>.txt` from `GOLDEN_DIR`, parse the **3rd whitespace column**
         (`state_hash`, hex `u32`) of each non-`#`, non-`total` line into `Vec<u32>` (index = tick). The
         sidecar format is `<tick> <frame_hash_hex16> <state_hash_hex8>` (see
         `render_slice3b_common::parse_frames`).
      8. Render tick 0 into `surface` and blit once so the window shows frame 0 immediately (call the
         same body as `tick_and_render`'s render+blit, or a shared `render_current` helper).
- [ ] **`tick_and_render` (FixedUpdate) — tick → CPU render → upload, exactly once per tick:**
      ```rust
      fn tick_and_render(
          mut sim: ResMut<Sim>,
          mut demo: ResMut<Demo>,
          mut images: ResMut<Assets<Image>>,
          frame: Res<FrameImage>,
      ) {
          // 1. advance the sim exactly one tick (no input in 3c).
          let inputs = [ControlState::new(), ControlState::new()];
          sim.0.process_frame(&inputs);

          // 2. loop step (deterministic re-seed at end of scenario).
          let (next, reload) = crate::blit::next_tick(demo.tick, demo.scenario.ticks);
          demo.tick = next;
          if reload {
              let loaded = scenario::load(Path::new(TC_ROOT), &demo.scenario);
              sim.0 = loaded.state;
              demo.loaded.viewports = loaded.viewports; // (or the restructured fields)
              demo.loaded.scene = loaded.scene;
          }

          // 3. render the current tick into the owned CPU surface (pure consumer).
          let draw_shadow = demo.scenario.shadow();
          let scene = demo.loaded.scene.as_scene(0, draw_shadow);
          render::frame::draw(&mut demo.surface, &sim.0, &mut demo.loaded.viewports, &scene);

          // 4. upload: get_mut marks the Image dirty -> GPU re-upload.
          let image = images.get_mut(&frame.0).unwrap();
          crate::blit::blit_surface_into_bytes(&demo.surface, image.data.as_mut().unwrap());

          // 5. debug-only determinism self-check (default scenario is a golden).
          #[cfg(debug_assertions)]
          debug_assert_eq!(
              sim::hash::hash_game_state(&sim.0),
              demo.golden[demo.tick as usize],
              "sim-hash diverged from the committed golden at tick {}",
              demo.tick
          );
      }
      ```
      **Borrow note:** `demo.loaded.scene.as_scene(..)` borrows `demo` immutably while
      `render::frame::draw` needs `&mut demo.loaded.viewports` — split the borrows (build the `Scene`
      into a local by cloning the small scalars, or reborrow fields separately) exactly as the T8
      `render_tick` does. **[verify at implementation]** and adjust the borrow structure so it compiles;
      the shape mirrors `render_slice3b_common::render_tick` which already threads `Scene` + `&mut
      viewports` together.
      **Tick-count subtlety:** the self-check compares AFTER the tick+wrap. On the wrap tick, `demo.tick`
      resets to 0 and `sim` is rebuilt to fresh tick-0 — so `hash_game_state(&sim.0) == golden[0]` holds.
      On a normal tick `k`, after `process_frame` the state matches golden line `k` and `demo.tick == k`
      (tick 0 was rendered pre-first-`process_frame` in `setup`, matching the golden's tick-0 == pre-
      first-ProcessFrame convention). **Confirm the index alignment at implementation** against the
      golden (setup renders tick 0; the first `FixedUpdate` produces tick 1 and asserts `golden[1]`); if
      the run panics on a self-check mismatch, the index is off-by-one — fix the alignment, do NOT weaken
      the assert.
- [ ] **Build + local milestone run** (`timeout: 600000` on the first build):
      `cargo build --manifest-path rust/Cargo.toml -p game` → green, then
      `timeout 12 cargo run -p game` on John's Mac: a window plays the blood scenario live (worm fires
      into its feet, blood sprays, terrain carves), crisp ×3 pixels, loops. **This human eyeball is the
      3c milestone.** Debug build ⇒ the self-check is live; a determinism regression panics immediately.
      Record the observed exit code (124 = ran to timeout = success) + a one-line "what I saw" in the
      done-report.
- [ ] `cargo test -p game` → the T2 pure tests still green (build with the new deps compiles).
- [ ] `cargo fmt -p game` (new code only).
- [ ] Reviewer (Opus): the firewall holds — `SimState` is mutated ONLY in `tick_and_render` via
      `process_frame`, no Bevy type flows into the sim; exactly ONE draw per tick (no `Update` render);
      loop re-seed rebuilds via the loader; the self-check is live under `debug_assertions` and its tick
      index aligns with the golden; `Time::<Fixed>::from_hz(1000.0/14.0)`.
- [ ] **Commit:**
      - `git add rust/game/src/main.rs`
      - `git commit -m "game(3c): FixedUpdate tick->render->present of the blood scenario, live window + debug golden self-check"`

---

### T4 — CI (`--exclude game` + separate build step), scenario CLI arg, window polish  [Opus]

**Files**
- Modify: `.github/workflows/rust.yml`, `rust/game/src/main.rs`

**Interfaces**
- Produces: CI that keeps the determinism gate fast + Bevy-independent while still gating `game`
  compilation; a `cargo run -p game -- <name>` positional arg to pick a committed scenario.

**Why (teaching note):** `cargo test --workspace` **compiles every binary target in every member**
(binaries without tests are still built), so merely adding `game` to `members` would make the fast
determinism gate pull and compile the entire Bevy tree (wgpu/winit/naga) on every Rust CI run — coupling
the hard gate to Bevy's compile time and toolchain stability. Excluding `game` from the test job and
gating it in its own `cargo build -p game` step (own cache) keeps the sim/render proofs fast and
isolates any future Bevy-toolchain breakage from the hard gate.

**Steps**

- [ ] **CI test step:** in `.github/workflows/rust.yml`, change the "Run differential tests" step's
      command from
      `cargo test --manifest-path rust/Cargo.toml --workspace 2>&1 | tee test-output.txt`
      to
      `cargo test --manifest-path rust/Cargo.toml --workspace --exclude game 2>&1 | tee test-output.txt`
      (keep `set -o pipefail`, the `CARGO_TERM_COLOR: never` env, and the summary parser untouched —
      `--exclude game` leaves `test-output.txt` shape identical, only omitting the Bevy compile).
- [ ] **CI game-build step:** add a NEW step AFTER "Run differential tests" (separate cache, its own
      failure surface):
      ```yaml
      - name: Build game crate (Bevy)
        run: cargo build --manifest-path rust/Cargo.toml -p game
      ```
      **[verify at implementation]** the trimmed feature set builds on stock `ubuntu-latest` with no
      extra apt packages (expected — audio/gilrs are dropped; winit's x11/wayland crates dlopen at
      runtime, so no `-dev` packages at build time). **Fallback:** if a system lib IS missing, add the
      apt install to THIS step only — never to the fast test job.
- [ ] **Scenario CLI arg** in `setup` (T3 hard-coded `"blood"`): read
      `std::env::args().nth(1).unwrap_or_else(|| "blood".to_string())`, validate it names a committed
      `render_slice3b_<name>_scenario.txt` under `GOLDEN_DIR` (else print the available names and exit
      non-zero, or fall back to `blood` with a warning — pick and document). Thread `name` into the
      scenario-text path, the loader, the reload path, and the self-check golden path. Confirm the
      self-check still holds for the chosen golden (every committed 3b scenario is a golden; good
      alternates: `dart`, `dart_water`).
- [ ] **Window polish** (mostly already in T1/T3): confirm the title is `"Liero-rs — 3c demo"`, the
      window is fixed 960×600 (`resizable: false`), and nearest sampling is active (global
      `default_nearest` and/or per-image `ImageSampler::nearest()`). Optionally append the scenario name
      to the title (`format!("Liero-rs — 3c demo ({name})")`).
- [ ] **Verify locally:** `cargo build --manifest-path rust/Cargo.toml -p game` (green) and
      `cargo run -p game -- dart` opens the dart scenario (best-effort local, exit 124 = ok). Confirm
      the CI YAML is valid (the awk summary block is unchanged).
- [ ] `cargo fmt -p game` (new code only).
- [ ] Reviewer (Opus): the test step excludes `game`; the build step gates it separately with no apt
      additions to the test job; the CLI arg validates against committed scenarios and threads through
      load + reload + self-check; no wasm/dynamic feature leaked into CI.
- [ ] **Commit:**
      - `git add .github/workflows/rust.yml rust/game/src/main.rs`
      - `git commit -m "ci+game(3c): --exclude game on the determinism gate + separate cargo build -p game; scenario CLI arg + title polish"`

---

### T5 — PROGRESS + overview 3c line + deferral ledger + slice close  [Opus]

**Files**
- Modify: `docs/superpowers/liero-rs-PROGRESS.md`; the rendering overview
  (`docs/superpowers/specs/2026-07-10-liero-rs-step3-rendering-overview.md`) 3c bullet + open-questions

**Steps**

- [ ] Update `docs/superpowers/liero-rs-PROGRESS.md` (use the real `currentDate` in any Status/date
      field — do NOT freeze a series prefix): 3c landed — the `game` binary opens a live, nearest-scaled,
      deterministic-looping window of the blood scenario; the `scenario` crate now owns the shared
      parser + loader (reused by 3d); the determinism firewall + debug golden self-check hold. Keep the
      human-facing status map current (the whole-picture view: Step 3 slices 3a/3b/3c done, 3d–3f next).
- [ ] Update the overview's 3c bullet to mark it landed (companion spec implemented), and record the 3
      open questions as **resolved** per the spec's controller-ratified decisions: (1) shared `scenario`
      crate owning parser + loader — ratified; (2) CI `--exclude game` + separate build step — ratified;
      (3) Option A fixed camera default — ratified.
- [ ] **Deferral ledger (explicit).** Record what 3c consciously carried forward, each to be lifted by
      the slice that needs it: keyboard **input** / game-loop-with-input (Step 4); **audio** (Step 4);
      **headless screenshot CLI + run/observe skill** (3d — the shared `scenario::load` is exactly what
      3d reuses); **HUD/font/bars/minimap** (3e); **wasm** (WebGL2, embedded assets — 3f; no wasm
      feature pulled here); **render interpolation** (`overstep_fraction` lerp — overview deferral; draw
      the latest tick); **resize-aware integer-fit camera** (fixed ×3 window shipped); **follow-cam**
      (Option B — allowed later `--follow` toggle, not the default); **texture-format gamma** (Rgba8
      Srgb vs linear — advisory, resolved by eyeball, note which was chosen).
- [ ] Reviewer (Opus): docs match reality; the 3 open questions are marked resolved with the ratified
      decision; deferrals honestly listed and routed to the right future slice; PROGRESS reflects the
      whole Step-3 map.
- [ ] **Commit:**
      - `git add docs/superpowers/liero-rs-PROGRESS.md docs/superpowers/specs/2026-07-10-liero-rs-step3-rendering-overview.md`
      - `git commit -m "docs(3c): PROGRESS + overview 3c landed; open-questions resolved; deferral ledger"`

## Done-report (each task)

Each task's worker returns: (a) what changed and why (1–3 sentences), (b) files touched, (c) tests/
smokes run + result (paste the key `cargo test`/`cargo build` line + exit code; for `game` runs, the
observed `timeout` exit code and a one-line "what I saw"), (d) any **[verify at implementation]** API
delta — the real 0.19 signature used and why it differed from the plan (especially: added Bevy
features, `Sprite`/`Camera2d`/`Image` construction, `AppExit`/`EventWriter` shape, `Time<Fixed>`
constructor, texture format chosen). Do NOT push; do NOT open a PR.
