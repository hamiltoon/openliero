# Step 3, Slice 3f — wasm bring-up (WebGL2 in the browser): Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax. **Test-first where there is logic to pin**: write the failing test
> (or its assertion) before the implementation, run it and SEE it fail, then make it pass. 3f is
> an **integration slice** — several tasks are build/wiring/verify rather than new gated math, so
> "test-first" often means "make the build fail the right way, then fix it" plus the standing
> native-golden re-run as the regression gate.

**Goal:** Compile the existing `game` binary to `wasm32-unknown-unknown` on the **WebGL2**
backend and render the *same fixed-seed scripted demo* 3c runs natively — now in a browser
`<canvas>`, from **embedded** assets (no filesystem, no async fetch). Prove the wasm sim (and,
cheaply, the wasm CPU frame) is **bit-identical** to native via 3c's determinism guard in a debug
wasm build. Ship a serve/deploy recipe (dev loop + static bundle, the C++ emscripten/gh-pages
analog). **No input (Step 4), no audio (Step 4), no netplay (Step 5).** This is the **LAST Step-3
slice** — completing it declares Step 3 (rendering) done. Companion spec:
`specs/2026-07-12-liero-rs-step3-slice3f-wasm-design.md` (cited **spec §N**).

**Architecture:** Additive + `cfg`-gated. The bit-exact pixel core (`sim`, `render`, `game`'s
ARGB→RGBA blit + Bevy wiring) recompiles to wasm **unchanged**. 3f touches exactly three seams:
(1) an **asset-source shim** in the `scenario` crate (`std::fs` on native, embedded bytes on
wasm) — `assets`/`sim`/`render` are NOT touched; (2) **Bevy features** split across cargo target
tables (`+webgl2` on wasm, `x11`/`wayland` non-wasm only); (3) a small `cfg(target_arch =
"wasm32")` fork in `game`'s entry (compile-time scenario, embedded text/golden, canvas). Plus a
small render-track tidy: the 3e `shot`-CLI HUD/scenario follow-up (T0).

**Tech stack:** Rust (`scenario` new asset module, `game` Cargo/entry, `shot` tidy), Bevy 0.19
wasm (WebGL2), `include_dir`/`include_bytes!` for the embed, `wasm-server-runner` + `wasm-bindgen`
for serve/deploy, a new **build-only** CI job. `data/TC/openliero` real TC. Native goldens
(`oracle-tests/golden/`) are the standing regression gate, run via `cargo test --workspace
--exclude game`.

## Global constraints

*(inherit every 3a–3e constraint; the 3f-specific ones follow)*

- **`render` crate stays edition 2021 and Bevy-free.** 3f does **not** touch `render`, `sim`,
  `sim-core`, or `assets`. `cargo tree -p render` shows no `bevy*`; `grep -rn "bevy" rust/render/`
  empty. The asset shim lives in `scenario`; the wasm entry fork in `game`.
- **Native must be provably unperturbed.** The asset-source shim's **native** branch is
  byte-for-byte the current reads (`std::fs::read(root.join(rel))`, same order). The hard gate
  after T1: `cargo test --workspace --exclude game` — every `render_slice3a/3b/3e_*` frame golden,
  every `sim_slice*.txt`, and `test_determinism` stay **byte-identical / green, unchanged**. A
  moved golden is a real bug — STOP.
- **WebGL2, not WebGPU.** Add the `webgl2` cargo feature on the wasm target only; never `webgpu`
  (it overrides `webgl2` and drops non-WebGPU browsers). Never `x11`/`wayland` on wasm
  (`wayland-sys` needs build-time pkg-config). Never `--features dynamic` (dynamic_linking) with
  the wasm target.
- **No new determinism-relevant crate; no sim/render behaviour change.** The wasm bytes are
  identical to the fs bytes ⇒ identical `SimState` ⇒ identical frames. 3f adds no gated math.
- **Presentation is NOT bit-gated in the browser** (overview): the CPU frame is authoritative and
  already gated natively. The wasm determinism guard is a **witness** (debug-only), not a CI gate.
  No browser pixel-golden, no Playwright.
- **Embed is curated:** sprites + weapons/nobjects/sobjects configs + tc.cfg + the demo level +
  the demo scenario text + (debug) its golden sidecar. **Do NOT embed `sounds/`** (audio = Step 4)
  or the unused big levels (`modern_test.lev`, non-demo levels).
- **Verify-at-implementation, don't invent APIs.** For any Bevy-0.19-wasm / `include_dir` /
  `wasm-server-runner` / winit-canvas detail flagged "verify at implementation" in the spec,
  confirm against current docs (context7 / docs.rs) and use the documented fallback rather than a
  guessed API.
- **No AI / "Generated with" taglines; no commit trailers** (no `Co-Authored-By`, no
  `Claude-Session`). **Commit on branch `liero-rs-step-3`** (the accumulating Step-3 PR). **Do NOT
  push and do NOT open a PR** — the controller owns push + PR. **fmt only new/edited files**
  (`cargo fmt` drift on untouched files is out of scope). **No sub-subagents.** **Bash discipline:**
  one command per call; no `>>`/heredoc/`&&`/`;`/`$VAR`; no `cd`+`git`; create files with the
  editor.

## File structure

- `rust/scenario/src/assets.rs` — **new.** `read_asset(tc_root: &Path, rel: &str) -> Vec<u8>` with
  two `cfg` impls (native `std::fs`; wasm embedded manifest via `include_dir`/`include_bytes!`).
  The wasm manifest lookup lives here.
- `rust/scenario/src/loader.rs` — route the ~5 direct reads + the `Objects::load` closure through
  `read_asset` (native bytes/order unchanged).
- `rust/scenario/src/lib.rs` — `pub mod assets;`.
- `rust/scenario/Cargo.toml` — `[target.'cfg(target_arch = "wasm32")'.dependencies] include_dir`.
- `rust/game/Cargo.toml` — split `bevy` features across base / non-wasm / wasm target tables
  (`+webgl2` wasm; `x11`/`wayland` non-wasm).
- `rust/game/src/main.rs` — `cfg(target_arch = "wasm32")` fork: compile-time scenario, embedded
  scenario text + (debug) sidecar, no `env::args`/`read_dir`, canvas window; keep the determinism
  guard (debug) and add the frame-hash witness.
- `rust/.cargo/config.toml` — **new (or extended):** `[target.wasm32-unknown-unknown] runner =
  "wasm-server-runner"` for the dev loop.
- `web/index.html` — **new (minimal):** the static-deploy `<canvas>` + wasm-bindgen glue harness.
- `rust/shot/src/lib.rs` — T0: generalize scenario-path resolution (find `render_slice3e_*`) +
  `--hud` opt-in flag (flip `draw_hud`/`map`).
- `.github/workflows/rust.yml` — a **new build-only job** `game-wasm` (`rustup target add
  wasm32-unknown-unknown` + `cargo build -p game --target wasm32-unknown-unknown`).
- `.claude/skills/liero-shot/` — extend the run-skill note with the wasm dev-loop command (T5).
- `docs/superpowers/liero-rs-PROGRESS.md` + the overview's 3f line + Q6 — updated in T6.

## Tasks

### T0 — `shot`-CLI 3e follow-up: HUD-aware + 3e-scenario-aware (render-track tidy)  [Opus]

**Files**
- Modify: `rust/shot/src/lib.rs` (scenario-path resolution + a `--hud` flag), `rust/shot/src/main.rs`
  (USAGE string).

**Interfaces**
- Consumes: `scenario::SceneData::as_scene` (which returns a `Scene` with `draw_hud`/`map` fields,
  3e T5). Produces: a `shot` that can (a) load `render_slice3e_*` scenarios and (b) draw the full
  player view when `--hud` is passed.

**Why (teaching note):** 3e shipped the full player view (HUD/bars/minimap) but the screenshot CLI
still hard-codes the `render_slice3b_` prefix (`lib.rs:283-287`) and renders through
`as_scene(..)` with `draw_hud=false`/`map=false` (`loader.rs:62-78`), so the HUD is invisible
through the observe-loop. This is render-track cleanup (spec §8) — it makes 3e's deliverable
observable, which is exactly the picture we will eyeball in the browser in T5. Keep it an **opt-in**
so existing `shot` invocations and the `shot` golden test are unchanged.

**Steps**

- [ ] **RED:** add a `shot` test that resolving scenario `"hud"` (a `render_slice3e_hud_scenario.txt`
      that 3e committed) succeeds — currently it looks only for `render_slice3b_hud_scenario.txt`
      and errors. And a parse test that `--hud` sets a `Config.hud = true`. Run `cargo test -p shot`
      → FAIL.
- [ ] **GREEN (resolution):** generalize the scenario-text lookup in `run` — try
      `render_slice3e_{name}_scenario.txt` then fall back to `render_slice3b_{name}_scenario.txt`
      (or resolve by scanning both prefixes), so both corpora load. Keep the error message listing
      what was tried. Do not change the default/name semantics for existing 3b names.
- [ ] **GREEN (`--hud`):** add `--hud` to `parse_args` (`Config.hud: bool`, default false) + the
      USAGE string. When set, `render_tick`/`render_scenario` build the scene with `draw_hud=true`
      and `map=true` (mutate the returned `Scene`, or add a `hud: bool` param threaded from
      `Config`). When unset, behaviour is **byte-identical** to today (the `shot` golden test must
      stay green — verify).
- [ ] Run `cargo test -p shot` (unit + the `golden.rs` faithfulness test) — all green;
      the golden test proves the no-`--hud` path is unchanged.
- [ ] Reviewer (Opus): `--hud` is opt-in (default path byte-identical, golden green); 3e scenarios
      resolve; the HUD draw uses the existing `Scene.draw_hud`/`map` (no new render code); USAGE
      updated. This task is independent of the wasm work.
- [ ] **Commit:**
      - `git add rust/shot/src/lib.rs rust/shot/src/main.rs`
      - `git commit -m "shot(3f): HUD-aware --hud flag + resolve render_slice3e_* scenarios"`

---

### T1 — `scenario::assets::read_asset` shim + loader refactor (native byte-identical)  [Opus]

**Files**
- Create: `rust/scenario/src/assets.rs`
- Modify: `rust/scenario/src/lib.rs` (`pub mod assets;`), `rust/scenario/src/loader.rs` (route reads)

**Interfaces**
- Produces: `scenario::assets::read_asset(tc_root: &Path, rel: &str) -> Vec<u8>` — native impl
  `std::fs::read(tc_root.join(rel))` (this task ships the **native** impl only; the wasm impl is
  T2, left `unimplemented!()`/`cfg`-absent here). Consumed by `loader::load` for every TC read.

**Why (teaching note):** the browser has no filesystem, but `scenario::load` reads the TC via
`std::fs` (spec §Asset strategy). We funnel every read through one named seam so the fs/embed
divergence lives in exactly one place — and so `assets`/`sim`/`render` are never touched
(bit-exactness invariant). The whole point: the native branch is *literally the current read*, so
every golden stays byte-identical. This is the least-invasive shape (a shim, not scattered
`#[cfg]` in the load body).

**Steps**

- [ ] Add `pub mod assets;` to `scenario/lib.rs`. Create `assets.rs` with `read_asset` — native
      `cfg(not(target_arch = "wasm32"))` body `std::fs::read(tc_root.join(rel))` (map the error to
      the same `panic!`/message shape the call sites use). Leave the wasm `cfg(target_arch =
      "wasm32")` body as a `unimplemented!("wasm embed: T2")` stub so the crate still builds for
      native (the wasm target is not built until T3/CI).
- [ ] Refactor `loader::load` (`loader.rs`) to call `read_asset(tc_root, rel)` for: `sprites/small.tga`,
      `sprites/large.tga` (both `load_sprites` sites), `sprites/font.tga`, the level (`&scenario.level`),
      `tc.cfg`, and the `Objects::load` closure (`|sub, id| read_asset(tc_root, &format!("{sub}/{id}.cfg"))`).
      Preserve the **exact read order** and the exact path strings (relative to `tc_root`).
- [ ] **Native re-diff gate (hard):** run `cargo test --workspace --exclude game`. Every
      `render_slice3a`/`render_slice3b_*`/`render_slice3e_*` frame golden, every `sim_slice*.txt`,
      the `shot` golden, and `test_determinism` must be **green, unchanged** — the shim is a
      no-op on native. Also `cargo test -p scenario` (the `load_yields_font_and_labels` loader
      test still passes). Paste the green summary in the done-report.
- [ ] `cargo build -p game` (native) — still compiles (it calls `scenario::load`).
- [ ] Reviewer (Opus): native `read_asset` == the prior `std::fs::read(format!(...))` byte-for-byte
      (same join, same order); all reads routed; `assets`/`sim`/`render` untouched; goldens green;
      the wasm stub does not affect the native build.
- [ ] **Commit:**
      - `git add rust/scenario/src/assets.rs rust/scenario/src/lib.rs rust/scenario/src/loader.rs`
      - `git commit -m "scenario(3f): read_asset shim (native std::fs); loader routes all TC reads"`

---

### T2 — wasm embedded-asset manifest (curated `include_dir`/`include_bytes!`)  [Opus]

**Files**
- Modify: `rust/scenario/src/assets.rs` (wasm `read_asset` body + the embedded manifest),
  `rust/scenario/Cargo.toml` (wasm-target `include_dir` dep)

**Interfaces**
- Produces: the `cfg(target_arch = "wasm32")` `read_asset` — keys `rel` into the embedded manifest,
  returns the bytes (`.to_vec()`); a missing key `panic!`s (build-time-known set).

**Why (teaching note):** the asset set is small and fixed, so we embed it in the binary and dodge
async fetch entirely (spec §Asset strategy; bevy-research §5). `include_dir` embeds a subtree and
looks up by the **same relative path** the fs closure builds (one source of truth), which matters
because the object configs are *dynamic* (ids from `tc.types`). We curate — sprites + the three
object dirs + `tc.cfg` + the one demo level — and deliberately exclude `sounds/` (audio = Step 4)
and the big unused levels, keeping the embed ≈250–300 KB.

**Steps**

- [ ] Add `include_dir = "0.7"` (verify current version) under
      `[target.'cfg(target_arch = "wasm32")'.dependencies]` in `scenario/Cargo.toml` — wasm-only,
      so native never pulls it.
- [ ] In `assets.rs`, behind `cfg(target_arch = "wasm32")`: `include_dir!` the curated subdirs
      (`sprites/`, `weapons/`, `nobjects/`, `sobjects/` under `$CARGO_MANIFEST_DIR/../../data/TC/openliero`),
      and `include_bytes!` `tc.cfg` + the **demo level** the default scenario references (read the
      `blood` scenario's `level` line to pin the filename, e.g. `Levels/render_stage.lev`). Do NOT
      embed `sounds/` or unused levels.
- [ ] Implement wasm `read_asset(_, rel)`: match the leading path segment (`sprites/…`,
      `weapons/…`, `nobjects/…`, `sobjects/…`) to its `Dir::get_file(rel).contents().to_vec()`;
      match the exact top-level names (`tc.cfg`, the level path) to their `include_bytes!` slice;
      else `panic!("wasm embed: no asset {rel}")`. **Verify at implementation:** the `include_dir`
      lookup API (`get_file`/`contents`); fallback = an explicit `include_bytes!` table if the
      crate API differs.
- [ ] Guard native unaffected: `cargo test --workspace --exclude game` still green (the wasm block
      is `cfg`-compiled out on native; `include_dir` is not in the native dep tree — confirm via
      `cargo tree -p scenario` showing no `include_dir` for the host target).
- [ ] (Cannot fully build wasm yet — `game`'s wasm entry is T3. If a quick `cargo build -p scenario
      --target wasm32-unknown-unknown` is feasible after `rustup target add`, run it to smoke the
      manifest compiles; otherwise defer the wasm compile proof to T3.)
- [ ] Reviewer (Opus): curated set matches the demo scenario's needs (level pinned from the
      scenario file); `sounds/` + big levels excluded; wasm-only dep (native tree clean); paths key
      exactly as the fs closure builds them; missing-key panics.
- [ ] **Commit:**
      - `git add rust/scenario/src/assets.rs rust/scenario/Cargo.toml`
      - `git commit -m "scenario(3f): embedded wasm asset manifest (curated include_dir/bytes)"`

---

### T3 — `game` wasm build: Cargo target-split features + entry fork (canvas, embed, guard)  [Opus]

**Files**
- Modify: `rust/game/Cargo.toml` (feature split), `rust/game/src/main.rs` (`cfg(wasm)` fork)

**Interfaces**
- Produces: a `game` crate that **compiles for `wasm32-unknown-unknown`** (`+webgl2`, no
  `x11`/`wayland`), whose wasm entry loads the compile-time-default scenario from embedded bytes,
  renders into a canvas, and (debug) runs the determinism + frame-hash guard.

**Why (teaching note):** on wasm there are no CLI args and no fs, so `resolve_scenario()`'s
`env::args` + `available_scenarios()`'s `read_dir` + the golden `read_to_string` cannot run
(spec §Entry/§Q4). The wasm entry hard-codes the default scenario, embeds its text + (debug)
sidecar via `include_bytes!`, and lets bevy_winit create the browser canvas. The `Time<Fixed>`
cadence is unchanged — determinism is by tick count, not wall-clock (spec §Q5). Keeping the guard
on wasm is the parity proof (spec §Q3): the same `render`/`sim` producing the same `state_hash`
(and, added here, the same CPU `frame_hash`) in the browser demonstrates the crown jewel survived
wasm.

**Steps**

- [ ] **Cargo feature split** (`game/Cargo.toml`): base `[dependencies] bevy` keeps the shared
      draw features (`bevy_render`, `bevy_core_pipeline`, `bevy_sprite_render`, `bevy_sprite`,
      `bevy_winit`, `bevy_window`); move `x11`/`wayland` to `[target.'cfg(not(target_arch =
      "wasm32"))'.dependencies] bevy`; add `webgl2` under `[target.'cfg(target_arch =
      "wasm32")'.dependencies] bevy`. **Verify at implementation:** that features union across the
      repeated `bevy` target deps (standard cargo); fallback = per-target `[features]` re-export.
      Keep the `dynamic` feature note ("never with wasm").
- [ ] **Entry fork** (`main.rs`): behind `cfg(target_arch = "wasm32")`, replace `resolve_scenario`
      (env::args) + `available_scenarios` (read_dir) + the scenario `read_to_string` +
      `load_golden_state_hashes` (read_to_string) with **embedded** equivalents: a
      `const DEFAULT` scenario name, `include_str!` its `render_slice3b_<default>_scenario.txt`,
      and — under `cfg(all(target_arch = "wasm32", debug_assertions))` — `include_str!` its
      golden sidecar for the guard. Native keeps the existing fs paths (unchanged). Keep it DRY:
      factor the shared post-parse logic; only the *byte source* forks.
- [ ] **Canvas window:** on wasm, configure the `Window` so bevy_winit renders to a browser canvas
      (the auto-appended canvas, or a selector matching `web/index.html`'s `<canvas>` — **verify**
      the Bevy 0.19 `Window` canvas field at implementation; fallback = default auto-canvas). Keep
      the `960×600` resolution / `ImagePlugin::default_nearest()` / ×3 sprite (identical present
      path).
- [ ] **Strengthen the guard (debug, both targets or wasm-only):** alongside the existing
      `state_hash` `debug_assert_eq!`, add a per-tick CPU `frame_hash` (`render::hash::hash_frame`
      on `demo.surface`, with the tick-0 `fade=0`/else-33 rule) checked against the embedded
      sidecar's `frame_hash` column — the render-parity witness (spec §Q3). Reuse the sidecar
      parse (extend `load_golden_state_hashes` to also return the frame-hash column). Keep it
      `#[cfg(debug_assertions)]` so release wasm has zero cost.
- [ ] **Build both targets:** `cargo build -p game` (native, unchanged) AND — after `rustup target
      add wasm32-unknown-unknown` — `cargo build -p game --target wasm32-unknown-unknown`. The wasm
      build **must compile** (the T3 done-when). Fix any `std::fs`/`x11`/`wayland` leak the compile
      surfaces.
- [ ] Reviewer (Opus): wasm build compiles; feature split correct (no `x11`/`wayland` on wasm, no
      `webgpu`); the wasm entry has no `env::args`/`read_dir`/`std::fs`; native entry unchanged;
      guard kept + frame-hash witness added, debug-only; canvas configured; `Time<Fixed>` cadence
      untouched.
- [ ] **Commit:**
      - `git add rust/game/Cargo.toml rust/game/src/main.rs`
      - `git commit -m "game(3f): wasm build — webgl2 features, embedded scenario, canvas, guard"`

---

### T4 — serve/deploy recipe: dev loop (`wasm-server-runner`) + static bundle (`wasm-bindgen`)  [Opus]

**Files**
- Create: `rust/.cargo/config.toml` (or extend), `web/index.html`

**Interfaces**
- Produces: a one-command dev loop (`cargo run -p game --target wasm32-unknown-unknown`) and a
  static-bundle recipe (the C++ emscripten/gh-pages analog).

**Why (teaching note):** minimal tooling, no bundler (spec §Q6). `wasm-server-runner` as the cargo
runner gives "cargo run → build+bindgen+serve+canvas" for the dev loop; `wasm-bindgen --target web`
+ a tiny `index.html` gives the shareable static bundle that parallels the C++ `emscripten` preset.
`trunk` is rejected (bundler config surface we do not need).

**Steps**

- [ ] Create/extend `rust/.cargo/config.toml`: `[target.wasm32-unknown-unknown] runner =
      "wasm-server-runner"`. Confirm it does not affect native cargo invocations (it is target-scoped).
- [ ] Create a minimal `web/index.html`: a `<canvas>` (id matching the T3 window selector, or a
      bare canvas Bevy appends to) + the wasm-bindgen JS glue `import`/`init` for the static path.
      Keep it self-contained and tiny.
- [ ] **Verify the dev loop MANUALLY (the milestone eyeball — see T5):** `cargo install
      wasm-server-runner` (document), `rustup target add wasm32-unknown-unknown`, then `cargo run
      -p game --target wasm32-unknown-unknown` → open the served `localhost` URL → the demo renders
      in the canvas. **Verify at implementation:** `wasm-server-runner`'s Bevy-canvas handling;
      fallback = the static recipe below.
- [ ] **Document the static-bundle recipe** (in the done-report and the T6 docs): `cargo build -p
      game --release --target wasm32-unknown-unknown`; `wasm-bindgen --out-dir web --target web
      target/wasm32-unknown-unknown/release/game.wasm`; serve `web/` (any static server). Note
      `wasm-opt` as an optional size knob (not required).
- [ ] Reviewer (Opus): dev loop is one `cargo` command; `.cargo/config.toml` target-scoped (native
      unaffected); `index.html` minimal + self-contained; both recipes documented; no bundler.
- [ ] **Commit:**
      - `git add rust/.cargo/config.toml web/index.html`
      - `git commit -m "game(3f): wasm serve/deploy recipe (wasm-server-runner + wasm-bindgen)"`

---

### T5 — MILESTONE: browser bring-up eyeball + wasm determinism witness + run-skill note  [Opus]

**Files**
- Modify: `.claude/skills/liero-shot/` (add the wasm dev-loop command to the run-skill note)

**Interfaces**
- Consumes: T3's wasm build + T4's serve recipe. Produces: the recorded milestone evidence + the
  run-skill's wasm entry.

**Why (teaching note):** GPU presentation is not bit-gated (overview), so the browser check is a
**manual milestone eyeball** — the Srgb-check / windowed-review precedent — *plus* the automatable
runtime witness: a **debug** wasm build's determinism guard (`state_hash` + the T3 frame-hash
column) proves the wasm sim AND CPU frame are bit-identical to native. That witness is the wasm
parity proof (spec §Q3; the netplay-exploration M0 gate delivered inside 3f).

**Steps**

- [ ] **Milestone eyeball (manual):** run the T4 dev loop with a **debug** build (guard live),
      open the browser, confirm: (a) the demo's world view renders in the canvas (the same picture
      as the native 3c demo); (b) it ticks/animates at ~the C++ cadence; (c) the browser console
      shows **no** `debug_assert` panic over many ticks / a loop — i.e. the `state_hash` (and
      frame-hash) guard stays green in the browser. Record this in the done-report (screenshot/
      console note) as the milestone evidence.
- [ ] If the guard trips in-browser: STOP — a float or platform divergence leaked into the sim
      (the crown-jewel risk, spec §Risks). Reproduce natively first; do not paper over it.
- [ ] **Run-skill note:** add the wasm dev-loop command (`cargo run -p game --target
      wasm32-unknown-unknown`) + the manual-eyeball step to `.claude/skills/liero-shot` (or a short
      sibling note), so the browser bring-up is reproducible from the repo. Keep it in-repo (the
      3d-ratified skill location).
- [ ] Reviewer (Opus): the milestone evidence is real (canvas render + green guard, not just "it
      built"); the wasm witness is debug-only and does not become a CI gate; the run-skill command
      is verified-run.
- [ ] **MILESTONE.** Liero-rs renders in a browser via WebGL2 from embedded assets; the wasm sim +
      CPU frame are proven bit-identical to native (debug guard green in the browser). Step 3's
      last visual deliverable is met.
- [ ] **Commit:**
      - `git add .claude/skills/liero-shot`
      - `git commit -m "game(3f): wasm browser bring-up milestone + run-skill wasm command"`

---

### T6 — CI wasm build job (minimal, build-only) + docs + Step-3-complete broad review  [Opus]

**Files**
- Modify: `.github/workflows/rust.yml` (new `game-wasm` job); `docs/superpowers/liero-rs-PROGRESS.md`;
  the overview's 3f bullet + Q6 (`docs/superpowers/specs/2026-07-10-liero-rs-step3-rendering-overview.md`)

**Interfaces**
- Produces: the minimal wasm CI gate (Q6 = build-only) + Step-3-complete docs.

**Why (teaching note):** Q6 is adjudicated **minimal** (spec §Q7): a build-only job catches the
real regression class (a wasm compile break — native-only API, `x11`/`wayland` leak, an fs read
outside the shim) without a browser or Playwright. Its own job = isolated cache + clean failure
surface, mirroring the existing `--exclude game` + separate native-`game`-build discipline. A wasm
build needs **neither** wayland headers **nor** a display.

**Steps**

- [ ] Add a `game-wasm` job to `rust.yml`: checkout, `dtolnay/rust-toolchain@stable` with the wasm
      target (`targets: wasm32-unknown-unknown`, or a `rustup target add` step), then `cargo build
      --manifest-path rust/Cargo.toml -p game --target wasm32-unknown-unknown`. **No** wayland/xkb
      apt install (not needed for wasm), **no** browser. Keep it under the existing `rust/**` path
      filter.
- [ ] Confirm the workflow YAML is valid and the job is independent (does not couple the fast
      determinism gate to the wasm target dir). Note: the first cold build is heavy (a separate
      Bevy wasm target) but build-only — acceptable (spec §Q7).
- [ ] **Native re-verify (final):** `cargo test --workspace --exclude game` green (all `render_slice*`
      + `sim_slice*` + `test_determinism` + `shot` unchanged); `cargo build -p game` (native) +
      `cargo build -p game --target wasm32-unknown-unknown` (wasm) both green. Confirm `render`
      still Bevy-free (`cargo tree -p render`; `grep -rn "bevy" rust/render/` empty). Paste the
      green summaries.
- [ ] Update `docs/superpowers/liero-rs-PROGRESS.md` (use the real `currentDate`): Step 3 slice 3f
      DONE — `game` compiles to `wasm32-unknown-unknown` on WebGL2 and renders the fixed-seed demo
      in a browser canvas from embedded assets (curated `include_dir`/`include_bytes!` via the
      `scenario::read_asset` shim; `assets`/`sim`/`render` untouched, all native goldens
      byte-identical); the debug wasm determinism + frame-hash guard proves the wasm sim + CPU
      frame are bit-identical to native; serve/deploy recipe (`wasm-server-runner` dev loop +
      `wasm-bindgen` static bundle); minimal build-only wasm CI job. **Declare Step 3 (rendering)
      COMPLETE** — all of 3a–3f shipped.
- [ ] Update the overview: mark 3f landed (companion spec + plan implemented); mark **Q6 RESOLVED —
      minimal build-only wasm CI**; record the other resolved 3f questions (embedded via
      read_asset shim; determinism guard kept on wasm as the parity witness; compile-time scenario
      default; the `shot` HUD follow-up taken as 3f T0). Mark Step 3's "in the browser" done-when
      (overview Goal item 4) met.
- [ ] **Deferral ledger (explicit).** Record carried deferrals (each with its home): input → Step 4;
      audio (`sounds/` not embedded) → Step 4; WebRTC/netplay/Worker/wasm-threads/COOP-COEP →
      Step 5; `?scenario=` query-param + multi-level embed → follow-up; Playwright/headless-browser
      CI + `wasm-bindgen` smoke → deferred (Q6 minimal); bundle-size optimization → later; a wasm
      build of `oracle-tests`/`shot` → not needed (the debug `game` guard witnesses wasm
      bit-exactness). A future need lifts each with the matching work.
- [ ] **Broad slice + Step-3 review (Opus):** re-read the whole 3f diff against the spec — asset
      shim (native byte-identical), curated embed, feature split, wasm entry fork, the determinism/
      frame-hash witness, the serve recipes, the minimal CI. Confirm `render` Bevy-free, native
      goldens unmoved, and that **Step 3 as a whole** (3a–3f) meets the overview's Goal/done-when
      (pixel-exact world + full player view natively, headless PNG, and now the browser). Deferrals
      honestly tripwired/recorded.
- [ ] Reviewer (Opus): CI job minimal + independent; docs match reality; Step 3 declared complete
      truthfully; Q6 + the 3f questions marked resolved; deferral ledger real.
- [ ] **Commit:**
      - `git add .github/workflows/rust.yml docs/superpowers/liero-rs-PROGRESS.md docs/superpowers/specs/2026-07-10-liero-rs-step3-rendering-overview.md`
      - `git commit -m "ci+docs(3f): minimal wasm build job; PROGRESS + overview — Step 3 complete"`

## Done-report (each task)

(a) what changed + why, (b) files touched, (c) tests/risks. Per-task commit, local, on branch
`liero-rs-step-3`. **Do not push, do not open a PR** — the controller owns push + PR. Surface in
the final report: the **native re-diff evidence** (T1/T6: every `render_slice*` + `sim_slice*` +
`test_determinism` byte-identical/green — the "assets/sim/render undisturbed" proof), the **wasm
build-green** evidence (`cargo build -p game --target wasm32-unknown-unknown`), the **browser
milestone** (T5: canvas render + the debug determinism/frame-hash guard staying green in-browser —
the wasm parity proof), the **serve recipe** verified-run, and the **CI job** shape. Note the
parity definition honestly (same picture, not the C++ web feature set) and confirm **Step 3 is
complete**.
</content>
