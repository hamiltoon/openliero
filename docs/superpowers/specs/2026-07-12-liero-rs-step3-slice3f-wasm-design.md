# Step 3, Slice 3f — wasm bring-up (WebGL2 in the browser): Design / SPEC

Status: **active** · 2026-07-12 · the **LAST** Step-3 slice (3a–3e SHIPPED)
Part of: `2026-07-10-liero-rs-step3-rendering-overview.md` (the §3f slice line + open question **Q6**)
Built on: `2026-07-10-liero-rs-step3-bevy-api-research.md` §5 (wasm) — cited **bevy-research §N**
Context: `2026-06-27-liero-rs-web-lightweight-netplay-exploration.md` (WebRTC/netplay is **Step 5**, NOT here)
Companion plan: `plans/2026-07-12-liero-rs-step3-slice3f-plan.md`

> **Scope in one sentence.** Compile the existing `game` binary to `wasm32-unknown-unknown`
> with the WebGL2 backend, feed it **embedded** assets (there is no filesystem in the
> browser), and render the *same fixed-seed scripted demo* 3c already runs natively —
> now in a browser `<canvas>`. **No input** (that is Step 4), **no audio** (Step 4), **no
> netplay** (Step 5). This is a build-and-present slice, not a port slice: the `render`/`sim`
> pixels are already bit-exact and gated natively; 3f just gives them a browser.

---

## Goal / done-when

The overview's 3f line: *"WebGL2 build, `include_bytes!`/embedded assets (dodge async fetch),
serve/deploy recipe to C++ emscripten parity. **Proves:** the same frame renders in the
browser. **Done-when:** the wasm build renders the world view and is servable."*

**Done when — the honest checklist:**

1. **`cargo build --target wasm32-unknown-unknown -p game` compiles** (the minimal CI gate,
   Q6 adjudicated below).
2. **The demo renders in a browser canvas** via WebGL2 — the *same* fixed-seed scenario the
   native 3c demo shows, ticking at the C++ cadence (`1000/14 Hz`), from **embedded** assets
   (no fetch, no fs). This is a **manual milestone eyeball** (the Srgb-check / windowed-review
   precedent — GPU presentation is not bit-gated).
3. **The wasm sim is provably bit-exact** in a debug wasm build: 3c's per-tick determinism
   guard (`state_hash` vs the committed golden) stays green in the browser — and, cheaply,
   the CPU **frame** hash vs the embedded golden column too (the sidecar already carries both).
   *This runtime proof IS the wasm parity evidence — the crown-jewel-survives-wasm gate the
   netplay-exploration doc (§6, M0) asks for, delivered inside 3f.*
4. **A serve/deploy recipe exists and is verified** — John can run one `cargo` command and open
   a browser (dev loop), and there is a static-bundle recipe (the C++ emscripten/gh-pages
   analog).
5. **Native is provably unperturbed** — every `render_slice3a/3b/3e_*` frame golden, every
   `sim_slice*.txt`, and `test_determinism` regenerate/stay byte-identical (the asset-source
   refactor changes native reads to a shim that is *literally* `std::fs::read` — same bytes).
6. **Docs updated** (PROGRESS + the overview's 3f line marked landed; Step 3 declared complete).

**Explicitly NOT done-when (deferred, with honest homes):** keyboard/gamepad input → Step 4;
audio → Step 4; WebRTC P2P / signaling / matchbox → Step 5; menus/`*State.cpp` → later; a
headless-browser (Playwright) pixel screenshot in CI → deferred (Q6 = minimal); wasm-threads /
COOP-COEP / Web Worker split → deferred (single-threaded main-thread canvas is enough for a
no-input demo; the exploration doc's Worker split is a Step-4/5 concern).

---

## Parity — defined honestly

The C++ repo already ships an **`emscripten` CMake preset** (`CMakePresets.json:127`,
`tools/cmake/ConfigurePresetTemplates.json:200`, `FindEmscriptenToolchain.cmake`) and a wasm
main loop (`gfx.cpp:15` `#if OPENLIERO_EMSCRIPTEN` → `emscripten_set_main_loop_arg`,
`gfx.cpp:1657`). The web is a **first-class existing platform** in the original; the C++
emscripten build is a *whole game* — SDL3 canvas, input, audio, menus, the capped-canvas world
clip (`gfx.cpp:1608,1727`).

**3f parity does NOT mean matching that feature set.** It means: *the Rust `game` binary's
fixed-seed scripted demo draws the same world view in a browser canvas that the native 3c demo
draws.* We match the **picture**, not the **product**. Input/audio/menus/netplay are C++ web
features that map to *later* Rust steps (4/5), so their absence in 3f is on-plan, not a parity
gap. The one property we DO match verbatim — and prove — is **determinism across the wasm
target** (same `state_hash`/`frame_hash` as native), because that is the crown jewel and it must
survive the browser. Everything visual is the already-gated CPU frame presented through WebGL2.

Rationale for the WebGL2 (not WebGPU) target: C++ emscripten renders through GLES/WebGL, WebGL2
is Bevy 0.19's **default** web backend with the broadest browser reach, and the CPU-blit
renderer (one nearest-sampled RGBA `Image` → one `Sprite`) is *trivially* within WebGL2 —
sidestepping every GPU-indexed-texture friction (bevy-research §2/§5). The `webgpu` cargo
feature **overrides** `webgl2` and drops non-WebGPU browsers (confirmed against Bevy 0.19
`cargo_features.md`, 2026-07-12), so we do not enable it.

---

## Architecture — what moves, what must NOT

The **entire pixel path is already done and bit-gated**: `sim` (deterministic, Bevy-free),
`render` (CPU blit → ARGB `Bitmap`, Bevy-free, frame-hash-gated), `game`'s ARGB→RGBA blit
(`game/src/blit.rs`, pure Rust) and its `Image`/`Sprite`/`Camera2d`/`FixedUpdate` wiring
(`game/src/main.rs`, 3c). **All of it recompiles to wasm unchanged.** 3f touches exactly three
seams, none of them the bit-exact core:

1. **Assets** — the browser has **no filesystem**. `scenario::load` (and `game`'s scenario-text
   / golden reads) do `std::fs::read`. These become reads against a small **asset source** that
   is `std::fs` on native and an **embedded byte map** on wasm. *`assets`, `sim`, `render` are
   NOT touched* — the shim lives in `scenario` (the loader crate) and `game` (its two extra
   reads). The wasm bytes are identical to the fs bytes ⇒ identical `SimState` ⇒ identical
   frames. (Least-invasive: the native branch is `std::fs::read(root.join(rel))`, so native is
   provably unchanged — §Asset strategy.)

2. **Bevy features** — add `webgl2` on the wasm target; keep `x11`/`wayland` **off** on wasm
   (Linux-only; `wayland-sys` needs pkg-config at build time and cannot build for wasm). Split
   the feature list across cargo target tables (§Features).

3. **`game` entry** — on wasm there are no CLI args and no fs, so the scenario is a
   **compile-time default** and its text/golden are embedded; the winit **canvas** replaces the
   native window. `main.rs` gains a small `cfg(target_arch = "wasm32")` fork (§Entry).

The crate graph is otherwise the 3c/3e graph. `render` stays edition 2021 + **Bevy-free** (the
standing invariant; `game` alone is Bevy). No new determinism-relevant crate.

---

## Sakfråga 1 — Asset strategy: **embedded, via a `scenario` asset-source shim** (recommended)

**Decision: embed (not fetch).** Roadmap default; bevy-research §5 recommendation 6; the
netplay-exploration doc's "dodge async fetch." Our asset set is **small and fixed**, and — the
load-bearing point — **asset parsing already lives in the Bevy-free `assets` crate, driven by
`scenario::load` doing plain `std::fs::read`**, *not* through Bevy's async `AssetServer`. So
embedding is a pure byte-source swap with **zero async, zero load-states, and native/wasm
symmetry**. `AssetServer`-fetch (bevy-research §5 option 3) is rejected: it re-introduces the
async fetch the C++ emscripten build itself avoided with `--preload-file`.

### How it wires through (minimally invasive)

`scenario::load(tc_root, scenario)` today reads (all `std::fs`, `loader.rs`):
`sprites/small.tga`, `sprites/large.tga`, `sprites/font.tga`, the level file (`scenario.level`,
e.g. `Levels/render_stage.lev`), `tc.cfg`, and — via the `Objects::load` closure —
`weapons/<id>.cfg`, `nobjects/<id>.cfg`, `sobjects/<id>.cfg` (the ids come from `tc.types`).
`game`/`shot` additionally read the scenario **text** and (debug) the **golden sidecar** from
`oracle-tests/golden/`.

Introduce a tiny read seam in `scenario`:

```rust
// scenario::assets (new small module) — the ONLY place fs/embed diverge.
pub fn read_asset(tc_root: &Path, rel: &str) -> Vec<u8>;   // rel e.g. "sprites/font.tga"
```

- **Native** (`cfg(not(target_arch = "wasm32"))`): `std::fs::read(tc_root.join(rel))` — byte-for-byte
  what the code does today. The `Objects::load` closure becomes
  `|sub, id| read_asset(root, &format!("{sub}/{id}.cfg"))`. **Every native golden stays green**
  because the bytes and the read order are unchanged.
- **Wasm** (`cfg(target_arch = "wasm32")`): `rel` keys into an **embedded manifest** (§below);
  `tc_root` is ignored. Missing key ⇒ `panic!` (build-time-known set, so a miss is a bug).

The loader body keeps its exact structure — same reads, same order, same `resolve_weapons`, same
`SimState::new` call. This is the *least-invasive* option that satisfies "assets/sim/render must
not be disturbed (bit-exactness)": `assets`/`sim`/`render` are untouched; only the loader's read
calls (and `game`'s two extra reads) route through `read_asset`. A `cfg`-branch **inside** the
loader body was considered and rejected in favour of the shim — the shim keeps the branch in one
named function instead of scattering `#[cfg]` across the load path.

### The embedded manifest (wasm-only, curated)

Recommended mechanism: `include_dir` (a wasm-only dep of `scenario`, added under
`[target.'cfg(target_arch = "wasm32")'.dependencies]`) over the **curated** TC subdirs, plus
`include_bytes!` for the individual top-level files:

```
sprites/   (include_dir)  — small.tga 7 KB, large.tga 28 KB, font.tga 14 KB  → ~52 KB
weapons/   (include_dir)  — ~40 tiny .cfg text files                          → tens of KB
nobjects/  (include_dir)  — ~24 tiny .cfg                                     → "
sobjects/  (include_dir)  — ~14 tiny .cfg                                     → "
tc.cfg     (include_bytes!)                                                    → 7 KB
<demo level> (include_bytes!, e.g. Levels/render_stage.lev)                    → 172 KB
```

**Deliberately EXCLUDED from the embed:** `sounds/` (audio is Step 4 — do NOT embed), the unused
big `Levels/` (`modern_test.lev` is 1.2 MB; `physics_fall_test.lev`; the non-demo test levels)
— embed only the level(s) the shipped demo scenario references. Rough total embed ≈ **250–300 KB**
(one dominant 172 KB level + ~52 KB sprites + small text) — an entirely acceptable wasm payload
(the netplay-exploration doc flags bundle size as a *later* optimization, not a 3f blocker). The
exact byte total is an implementation detail measured at build; the `du`-level accounting is the
curation rule, not a hard number.

> **Why `include_dir` over a hand-written `include_bytes!` table:** the object configs are
> *dynamic* (ids from `tc.types`), so a flat `include_bytes!` list would hard-code ~80 filenames
> and rot when the TC changes. `include_dir` embeds the subtree and looks up by the same relative
> path the fs closure builds — one source of truth. `bevy_embedded_assets`/`embedded_asset!`
> (bevy-research §5 option 1) is **not** used: those feed Bevy's `AssetServer`, and our assets go
> through the Bevy-free `assets` crate, not the asset server. **Verify at implementation:** the
> `include_dir` 0.x API (`Dir::get_file` / `contents`) — fallback is the explicit `include_bytes!`
> manifest if the crate churns; either satisfies the seam.

The scenario **text** and (debug) the **golden sidecar** live outside the TC (`oracle-tests/
golden/`) — embed those two in the **`game`** crate via `include_bytes!` on the wasm target
(they are the compile-time-default scenario's files; §Entry).

---

## Sakfråga 2 — Bevy features for wasm

Base (shared, native + wasm) — the 3c set that actually draws:
`bevy_render`, `bevy_core_pipeline`, `bevy_sprite_render`, `bevy_sprite`, `bevy_winit`,
`bevy_window` (all `default-features = false`).

- **Add on wasm:** `webgl2` (the default web backend; confirmed present in Bevy 0.19
  `cargo_features.md`, 2026-07-12). Do **NOT** add `webgpu` — it overrides `webgl2` and drops
  non-WebGPU browsers.
- **Keep OFF on wasm:** `x11`, `wayland` — Linux windowing; `wayland-sys` resolves via
  pkg-config **at build time** and will not build for `wasm32`. Move them to the non-wasm target
  table.
- **`bevy_winit` stays on for wasm** — winit supports the browser and creates/attaches the
  `<canvas>` and drives the event loop via `requestAnimationFrame` (bevy-research §5; the C++
  `emscripten_set_main_loop` analog).
- **`dynamic_linking` (the crate's `dynamic` feature)** — dev-only, **cannot** be used on wasm.
  It is already opt-in (never a default, never in CI). Document: "never pass `--features dynamic`
  with the wasm target."

Cargo shape (features union across target tables for the same dependency — standard cargo;
**verify at implementation** that repeating the `bevy` dep in a target table unions features, or
use per-target feature blocks):

```toml
# base
[dependencies]
bevy = { version = "0.19", default-features = false, features = [
  "bevy_render", "bevy_core_pipeline", "bevy_sprite_render",
  "bevy_sprite", "bevy_winit", "bevy_window",
] }

[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
bevy = { version = "0.19", default-features = false, features = ["x11", "wayland"] }

[target.'cfg(target_arch = "wasm32")'.dependencies]
bevy = { version = "0.19", default-features = false, features = ["webgl2"] }
```

---

## Sakfråga 3 — Determinism guard on wasm: **KEEP it (debug); it is the parity proof**

3c's `game` carries a `#[cfg(debug_assertions)]` per-tick guard: `hash_game_state(&sim)` vs the
committed golden's `state_hash` column (`main.rs:252-258`). **Recommendation: keep it on wasm**,
and *strengthen* it there cheaply. Reasoning:

- It is the **runtime proof that the wasm sim is bit-exact** — exactly the netplay-exploration
  doc's M0 gate ("the wasm build's per-tick `HashGameState` matches native… proves the crown
  jewel survived wasm", §5/§6). Delivering it inside 3f means wasm parity is *demonstrated*, not
  asserted.
- The golden's sidecar line already carries **both** `frame_hash` and `state_hash`
  (`render_slice3b_<name>.txt`; the overview golden format). So a debug wasm build can *also*
  compute the CPU `frame_hash` (via `render::hash::hash_frame`, already compiled in) and assert
  it against the embedded column — proving the **render** frame is bit-exact on wasm too, not
  just the sim. This is the render-parity witness the browser gives us for near-free.
- It is **debug-only**: a typical *release* wasm bundle (`--release`) has `debug_assertions`
  off, so the guard and the embedded sidecar compile out — zero release payload cost. Embed the
  sidecar text only under `cfg(all(target_arch = "wasm32", debug_assertions))`.

The golden is *not* an "asset problem": the sidecar is a few KB of text, embedded like the
scenario text, and only in debug. Do **not** stand up a browser-side pixel-golden gate (the
overview is explicit: WebGL presentation is not bit-gated; the CPU frame is authoritative and is
already gated natively). The wasm guard is a *witness*, the native oracle-tests remain *the gate*.

---

## Sakfråga 4 — Scenario selection on wasm: **compile-time default; query-param deferred**

`std::env::args()` on wasm yields only the program name — there is no positional scenario arg,
and `available_scenarios()` (a `read_dir` of `GOLDEN_DIR`) has no filesystem. **Recommendation:**
on wasm the scenario is the **compile-time default** (`blood`, matching the native default), its
scenario text + debug sidecar embedded via `include_bytes!`, its level in the embedded TC
manifest. A `?scenario=<name>` **query-param** switch is a **nice-to-have, deferred**: it needs
`web-sys` (`window().location().search()`) *and* embedding every candidate scenario's level +
text, which bloats the bundle for a demo. Minimal 3f ships one embedded demo; the query-param and
its multi-level embed are a documented follow-up (lift it if a share-demo wants scenario
switching).

---

## Sakfråga 5 — Cadence on wasm: **identical; `Time<Fixed>` handles it**

The native demo ticks `FixedUpdate` at `Time::<Fixed>::from_hz(1000.0/14.0)` (`main.rs:93`, the
C++ `kDelay=14ms` cadence). On wasm, bevy_winit drives the schedule from `requestAnimationFrame`
(~display Hz), and `Time<Fixed>` accumulates *real elapsed time* and runs `FixedUpdate` the
correct number of steps to hit `71.43 Hz` — **the same accumulator, unchanged**. Determinism is
by **tick count**, not wall-clock (the 3c invariant, `main.rs:90-92`), so a browser running the
demo faster/slower than 71 Hz still produces the identical per-tick `state_hash`. Bevy's `Time`
uses `web_time`/`performance.now()` under the hood on wasm — engine-internal, we do not touch it.
**Verify at implementation** (durable, high confidence): that `FixedUpdate` + `Time<Fixed>` tick
under the wasm winit runner exactly as native — fallback if not: drive the tick from `Update`
with a manual accumulator (same math), but this is not expected to be needed.

---

## Sakfråga 6 — Serve / dev flow: **`wasm-server-runner` for the loop; `wasm-bindgen` for deploy**

Two recipes, minimal tooling, **no bundler**:

1. **Dev loop (one command, John's `cargo … + open browser`):** `wasm-server-runner` set as the
   cargo runner for the wasm target in `rust/.cargo/config.toml`:
   ```toml
   [target.wasm32-unknown-unknown]
   runner = "wasm-server-runner"
   ```
   Then `cargo run -p game --target wasm32-unknown-unknown` builds, wasm-bindgens, serves on
   `localhost`, and generates the minimal HTML+canvas harness automatically (the Bevy-examples
   web workflow). Least tooling for "cargo run then open the browser"; **no `index.html` to
   hand-write** for the dev loop. Install: `cargo install wasm-server-runner`,
   `rustup target add wasm32-unknown-unknown`.
2. **Static deploy (the C++ emscripten/gh-pages analog — a shareable URL):**
   ```
   cargo build -p game --release --target wasm32-unknown-unknown
   wasm-bindgen --out-dir web --target web target/wasm32-unknown-unknown/release/game.wasm
   # serve web/ (a small hand-written index.html with a <canvas> + the JS glue) statically
   ```
   optionally `wasm-opt` for size. This is the servable bundle that parallels the C++
   `emscripten` preset's static artifact. Ship a minimal `web/index.html` in the repo for this
   path.

`trunk` was considered and **rejected** for 3f: it adds an `index.html`/`Trunk.toml`
bundler-config surface we do not need for a single canvas. **Verify at implementation:**
`wasm-server-runner`'s current handling of Bevy's winit canvas auto-create (it is the documented
Bevy path) — fallback is recipe 2 (manual `wasm-bindgen` + a `basic-http-server`/`python -m
http.server` on `web/`). Canvas: let bevy_winit create/append the default canvas (or target an
existing `<canvas id="...">` via `Window { canvas: Some(...) }` in the static `index.html`);
**verify** the Bevy 0.19 `Window` canvas-selector field name at implementation, fallback = the
auto-appended canvas.

---

## Sakfråga 7 — CI depth: **MINIMAL — `cargo build --target wasm32-unknown-unknown -p game`**

**This resolves overview open question Q6.** Adjudication (set at Step-3 start, reaffirmed):
**minimal** — a **build-only** gate, no browser, no Playwright canvas screenshot.

- **What CI runs:** `rustup target add wasm32-unknown-unknown` + `cargo build --manifest-path
  rust/Cargo.toml -p game --target wasm32-unknown-unknown`. This catches the real regression
  class: a change that breaks the wasm compile (a native-only API, an `x11`/`wayland` leak onto
  wasm, an fs read that escaped the shim, a non-wasm dep).
- **Separate job (recommended), not a step in the existing `game` build.** Mirror the existing
  discipline (the determinism gate is `--exclude game`; the native `game` build is its own step
  with its own `wayland`/`xkbcommon` apt install). A wasm build needs **neither** the wayland
  headers **nor** a display — but it *does* need the wasm target and a separate target dir
  (its own cache). Its own job = clean failure surface + isolated cache, exactly the rationale
  the current workflow's comments give for splitting the native `game` build out.
- **Cache cost:** a wasm Bevy build is a full separate `target/` — heavy on a cold cache, but
  it is **build-only** (no test compile of every binary), and it runs only on `rust/**` /
  `oracle_dump` / workflow changes (the existing path filter). Acceptable.
- **Explicitly deferred (Q6 = minimal):** a headless-browser (Playwright) canvas screenshot in
  CI — the GPU-presentation pixel path is *not* bit-gated (the CPU frame is, natively), so a
  browser screenshot would be advisory at best and adds a whole browser-in-CI apparatus for no
  hard gate. **Optional cheap add, documented but NOT required for 3f:** a `wasm-bindgen` smoke
  step (run `wasm-bindgen` on the built `.wasm` to catch the "compiles but bindgen chokes"
  class) — one extra tool, no browser. Left as a follow-up so the CI stays *minimal* as
  adjudicated.

---

## Sakfråga 8 — The 3e `shot`-CLI follow-up: **take it as a small T0 in 3f** (recommended)

From the 3e ledger / `shot`'s current code: `shot::run` hard-codes the scenario path prefix
`render_slice3b_{}_scenario.txt` (`shot/src/lib.rs:283-287`) and renders through
`SceneData::as_scene(screen_flash, draw_shadow)`, which sets `draw_hud = false` / `map = false`
(`loader.rs:62-78`). So the screenshot CLI **cannot** load the 3e `render_slice3e_*` scenarios
and **cannot** draw the HUD/minimap that 3e shipped — the full player view is invisible through
the CLI's observe-loop.

**Recommendation: fold it in as a small, self-contained T0 of 3f.** It is render-track tidy
(not wasm), it is low-risk, and it makes 3e's deliverable — the full player view — *observable*
through the screenshot/eyeball loop, which is directly useful for the 3f browser bring-up (the
same picture we will eyeball in the canvas). Concretely: (a) generalize `shot`'s scenario
resolution to also find `render_slice3e_*` (try the 3e prefix, or accept an explicit scenario
name/prefix), and (b) add a `--hud` opt-in flag that flips `draw_hud`/`map` on the returned
`Scene`. Keep it minimal — an opt-in, no behaviour change to existing `shot` invocations (so the
`shot` golden test stays green). The alternative — a separate micro-slice — is rejected as
overhead: there is no better home now that 3e has shipped, and 3f is the render-track closer.
It is scoped as an **independent** first task so it does not entangle the wasm work.

---

## Risks & the hard 10%

- **A stray `std::fs` on the wasm path.** Any read not routed through `read_asset` (or a
  `game`-local embed) fails to compile or panics at runtime on wasm. Mitigation: the wasm CI
  build catches compile-time escapes; grep the load path; the embedded-miss `panic!` catches
  runtime ones fast. `available_scenarios()`'s `read_dir` and the golden `read_to_string` in
  `game` are the known sites — both `cfg`-forked in T3.
- **Native regression from the asset-source refactor.** The shim's native branch MUST be
  byte-identical to today's reads (same path join, same order). Gate: re-run `cargo test
  --workspace --exclude game` — every `render_slice3a/3b/3e_*` + `sim_slice*.txt` +
  `test_determinism` green, unchanged. This is the "assets/sim/render undisturbed" proof.
- **`wayland`/`x11` leaking onto wasm.** If left in the shared feature list, `wayland-sys`'s
  build script breaks the wasm build. Mitigation: target-table split (§Features); the wasm CI
  build proves the split holds.
- **Bevy 2D-internals churn.** Same standing risk as 3c (bevy-research §1): the 3f code only
  touches the stable public `Sprite`/`Image`/`Camera2d`/`Window`/`Time<Fixed>` surface + cargo
  features — no `bevy_render`-internal types. Confined to `game`.
- **wasm determinism (crown jewel).** Highest-value, high-confidence: the sim is integer/
  fixed-point (no floats — `sim-core`), and wasm `i32`/`i64` are well-defined, so bit-exactness
  is expected. But it is *proven*, not assumed — that is exactly what the debug wasm guard
  (§Q3) demonstrates at runtime. If a float ever leaked into the sim, the browser is where it
  bites, and the guard trips.
- **Bundle size.** ~250–300 KB assets + the Bevy/wgpu wasm (multi-MB) is a non-trivial first
  load. Not a 3f blocker (a no-input demo); `wasm-opt`/brotli are documented *later* knobs
  (netplay-exploration §6). Do not add them proactively (snappy-machine principle).

---

## Deferrals (explicitly out of 3f scope)

- **Keyboard/gamepad input**, the game-loop-with-input → **Step 4**.
- **Audio** (`sounds/` embed, the whole audio pipeline) → **Step 4** (do NOT embed `sounds/`).
- **WebRTC P2P / signaling / matchbox / Web Worker / OffscreenCanvas / wasm-threads /
  COOP-COEP** → **Step 5** (the entire netplay-exploration doc is post-3f; 3f is single-threaded
  main-thread canvas).
- **`?scenario=` query-param switch** + its multi-level embed → follow-up (§Q4).
- **Headless-browser (Playwright) canvas screenshot in CI** and the **`wasm-bindgen` smoke
  step** → deferred (Q6 = minimal; §Q7).
- **Bundle-size optimization** (`wasm-opt`, brotli, code-strip) → later (§Risks).
- **A wasm build of `oracle-tests`/`shot`** (running the full frame-hash differential in the
  browser) → not needed; the debug `game` guard already witnesses wasm bit-exactness, and native
  oracle-tests are the gate.
- **Menus / `*State.cpp` / spectator / Modern color** → per the overview's standing deferrals.

---

## Resolved open questions (recorded for the overview)

- **Q6 (wasm CI depth) — RESOLVED: MINIMAL.** Build-only `cargo build --target
  wasm32-unknown-unknown -p game` in its own job; no browser/Playwright. `wasm-bindgen` smoke is
  a documented optional, not required.
- **Asset strategy — RESOLVED: embedded** via a `scenario::read_asset` shim (native `std::fs`,
  wasm curated `include_dir`/`include_bytes!`), assets/sim/render untouched.
- **Determinism guard on wasm — RESOLVED: keep (debug), strengthen with the frame-hash column**
  — the wasm parity proof, zero release cost.
- **Scenario selection on wasm — RESOLVED: compile-time default**; query-param deferred.
- **The 3e `shot` follow-up — RESOLVED: taken as 3f T0** (render-track tidy + `--hud`).
</content>
</invoke>
