# Liero-rs — interactive iteration: start, observe, iterate (EXPLORATION)

Status: **EXPLORATION — not a committed plan, not a spec** · 2026-06-26
Part of: `2026-06-26-liero-rs-roadmap.md` · informs: `2026-06-26-liero-rs-steps2-5-preliminary-breakdown.md` (steps 3–4)

> **Read this first.** This document explores *how Claude Code and the user will
> start the Bevy app, observe what it's doing on screen / in behaviour, and
> iterate on that behaviour in a tight loop* — for development iteration and for
> regression testing. It is an **exploration**: it maps options and trade-offs
> and ends with a recommendation, but it commits nothing. The concrete decisions
> (exact Bevy APIs, screenshot mechanism, launch flags) belong in the
> just-in-time specs for steps 3 and 4 and **must be re-validated with fresh
> research then** — Bevy's rendering/window APIs move fast. Everything here is at
> the strategy level.

---

## TL;DR (the recommendation)

- **The loop we want:** `change behaviour → launch app → see a frame → judge it →
  adjust`. Make that loop work for *both* a human and an agent.
- **Build three observation surfaces, not one** — they serve different jobs and
  the project already proves this pattern in C++:
  1. **Native window** (winit) — primary *dev* iteration surface for the user.
     Lowest friction, full perf, instant feedback.
  2. **Headless screenshot + replay regression** — the *automated/agent* surface.
     A fixed-seed scenario or recorded input timeline runs N ticks and dumps a
     PNG (and/or a frame checksum) at fixed frames; diff vs golden. This is the
     robust regression story and it **reuses the step-2 determinism oracle**.
  3. **Web (wasm) build** — the *sharing + zero-setup observation* surface. The
     emscripten path already exists for C++ and ships to gh-pages; mirror it for
     Bevy. Easy for the user to open on any device and easy for an agent to drive
     in a headless browser and screenshot the canvas.
- **Native vs web is a false binary.** Use **native for dev iteration, headless
  screenshots/replay for automated regression, and a web build for sharing and
  for agent-driven browser observation.** Do *not* make wasm the *only* interface
  — that adds friction to the inner dev loop and to determinism debugging.
- **Determinism is the regression superpower.** Because the sim is bit-exact,
  *replay-driven regression* (run a recorded input timeline + seed, checksum or
  screenshot at fixed ticks, diff vs golden) is far more reliable than
  pixel-diffing a live, timing-dependent UI. The C++ engine already does exactly
  this with `framehash` and `videotool` — copy the idea into Rust.
- **For Claude Code's `run`/`verify` skills to work well**, the project must ship
  (a) a **project-specific launch skill** the `run` skill can discover, and
  (b) a **headless screenshot mode** + a **fixed-seed "demo" launch** + a
  **replay player** behind a stable CLI. Those are step-3/4 deliverables; this
  doc says what they need to be, not how to build them.

---

## 1. Current state of "run & observe"

### 1a. The C++ engine today (the precedent we should copy)

The C++ engine is already unusually well set up for headless observation, and the
design choices it made are directly portable in spirit to Bevy. From `CLAUDE.md`
and the code:

- **Headless smoke test.** `SDL_VIDEODRIVER=dummy SDL_AUDIODRIVER=dummy timeout 8
  ./openliero` launches the full game with no display/audio; exit code 124
  (timeout) means it ran cleanly. This is the cheapest "does it even start"
  signal and `CLAUDE.md` insists on it before declaring UI/startup changes done
  (ctest alone won't catch SDL/menu init breakage).
- **Software framebuffer, display-agnostic.** `Renderer` (`src/game/gfx/renderer.hpp`)
  owns a `Bitmap` of `uint32_t* pixels` (ARGB8888). *All* rendering goes into this
  CPU buffer; an 8-bit palette index is resolved through `pal32[256]` at draw
  time. The SDL window is just a final blit of that buffer to a texture
  (`ScaleDraw` → SDL texture → present). The renderer comment explicitly calls
  out "CPU paths (videotool, single-screen replay, dummy driver)". **Frame
  capture needs no window, no GPU.**
- **`videotool`** (`src/video_tool/replay_to_video.cpp`, `tools_main.cpp`,
  `video_recorder.c`): headless replay → MP4. Initializes a `Renderer` with an
  in-memory bitmap (320×200 player view, or 1280×720 spectator), loads palette
  from TC, reconstructs state from the `.lrp`, then loops
  `ReplayReader::PlaybackFrame()` → `Game::ProcessFrame()` → `Game::Draw()`,
  captures `renderer.bmp.pixels`, scales, encodes H.264 via ffmpeg. No display.
- **`framehash`** (`src/tests/framehash_main.cpp`, built per `CMakeLists.txt`):
  the visual-regression primitive. Headless replay playback that prints an FNV-1a
  hash of each frame's RGB pixels — and optionally dumps raw 24-bit RGB per frame
  to a file (`framehash <tc-dir> <replay.lrp> [s|n] [rgb-dump-file]`). This is
  *exactly* the "run N ticks, checksum at fixed frames, diff vs golden" pattern,
  already operational. It is **built but not yet wired into CI** — the
  infrastructure exists; the baseline comparison job doesn't.
- **`.lrp` replays + `ReplayController`** (`src/game/replay.{hpp,cpp}`,
  `src/game/controller/replayController.cpp`): deflate-compressed binary, header
  magic + version, serialized initial state (cereal), then per-frame
  delta-encoded `ControlState`, with checksums written every 1050 frames for
  desync detection. `ReplayController::PlaybackFrame()` drives the sim from
  recorded input; supports rewind to start. **A recorded input timeline + seed
  reproduces a match exactly** — this is the determinism contract made concrete.
- **Web build already exists.** The `emscripten` preset
  (`CMakePresets.json`, `tools/cmake/ConfigurePresetTemplates.json`,
  `FindEmscriptenToolchain.cmake`) builds `openliero.html/.js/.wasm/.data`. The
  main loop swaps `while` for `emscripten_set_main_loop_arg`
  (`src/game/gfx.cpp`), assets are preloaded into the wasm FS
  (`--preload-file data/...`), and SDL3 maps canvas→WebGL2, keyboard/gamepad→web
  events, audio→Web Audio with essentially no `#ifdef` in game code. CI builds it
  (`.github/workflows/build.yml`) and **auto-deploys to gh-pages**
  (`.github/workflows/pages.yml`, releases + last-10 dev builds, generated
  index). You serve it locally with `python3 -m http.server -d install/emscripten`
  then open `openliero.html`. So "playable in a browser" is a *solved* path on the
  C++ side — the Rust side just needs to reach parity.

**Takeaway:** the C++ engine separates *simulation*, *CPU-rasterized frame*, and
*presentation (window vs wasm vs video vs hash)*. That separation is exactly what
makes it observable headlessly. The Bevy rewrite should preserve the same
separation in spirit (sim state is authoritative and renderer-agnostic; the
"surface" is swappable), even though Bevy renders on the GPU rather than to a CPU
bitmap.

### 1b. What the future Bevy app will offer

Per the roadmap, step 3 is "Rendering — playable image in a window **and** in the
browser (Wasm)" and step 4 is "Loop + input — playable single-player". So the
Bevy app will, by construction, have:

- a **native window** (Bevy default: winit + wgpu),
- a **wasm/web** target (the roadmap names web as an explicit step-3 goal),
- and — if we ask for it — a **headless render** mode and a **replay player**,
  which are *not* free in Bevy the way they were in the C++ CPU-bitmap design and
  must be deliberately built (see §2).

The crucial difference from C++: Bevy renders through **wgpu** on the GPU, so
"headless" doesn't mean "skip the GPU" — it means "render to an off-screen GPU
texture (or a software adapter) and read it back," which is a real, supported
thing but a deliberate setup (§2a).

---

## 2. How a Bevy app is driven & observed programmatically

> Versions and exact APIs intentionally omitted — **research these fresh at step
> 3** (the breakdown already flags "API churn (research-before-build)" for
> rendering). What follows is the durable strategy.

### 2a. Native headless screenshots

Two established approaches (confirmed current as of mid-2026; re-verify at step 3):

- **Screenshot API** — Bevy exposes a screenshot capability: spawn a `Screenshot`
  request targeting a window/render target, with an observer that fires when the
  capture is ready and saves/encodes it. Simplest path when you want *one* (or a
  few, at chosen ticks) images rather than every frame. Best fit for `run`/`verify`.
- **Headless renderer** — render a camera to a GPU image render target, copy
  GPU→buffer→CPU, save to PNG. Bevy ships an official `headless_renderer.rs`
  example doing exactly this: no primary window, `WinitPlugin` disabled (it panics
  without a display server), `ScheduleRunnerPlugin` to drive the loop. The
  community `bevy_capture` crate wraps the every-frame variant with GIF/MP4
  encoders (this is the Bevy analogue of `videotool`).
- **Running without a real display in CI:** drive wgpu over a software/CPU
  adapter — `WGPU_BACKEND` + Mesa **llvmpipe** (or lavapipe) — or `xvfb` for a
  virtual X display. This makes native headless screenshots work on a stock CI
  runner. (Determinism note: GPU rasterization is *not* guaranteed bit-identical
  across adapters, so pixel-exact golden screenshots should be pinned to one
  agreed renderer — see §3 on why we lean on *state* checksums, not pixels, for
  the hard regression gate.)

### 2b. Scripted / synthetic input injection

For replay and for agent-driven "press these keys" tests, the loop must accept
input from a source other than the keyboard. Two levels:

- **Sim-level (preferred, deterministic):** feed the per-tick `ControlState`
  stream directly into the fixed-timestep driver — i.e. a `ReplayController`
  equivalent. This bypasses winit entirely and is the bit-exact path (mirrors the
  C++ `.lrp` design). This is what regression should use.
- **Window-level (for true end-to-end UI tests):** inject `KeyboardInput` /
  `ButtonInput` events into Bevy's input resources, or (for wasm) synthesize DOM
  keyboard events in the browser. Useful for menu/flow tests; not needed for sim
  regression.

### 2c. The wasm/web option

Build with `wasm-bindgen` (Bevy renders to a `<canvas>` via WebGL2/WebGPU). To
*observe* it programmatically: serve the build, open it in a **headless browser**
(Playwright/Puppeteer), and `screenshot` the canvas; Playwright has first-class
visual-comparison/screenshot support. To *drive* it: dispatch synthetic keyboard
events to the canvas, or expose a tiny JS hook from Rust (a `#[wasm_bindgen]`
function like `load_replay(bytes)` / `tick_to(frame)`) so the harness can put the
app into a known state before screenshotting. This is the agent-friendly,
no-native-display path and it doubles as the user's share link.

**Caveat:** wasm constraints (no threads by default, async asset loading, indexed
palette → GPU) are real (the C++ emscripten build preloads all assets into the FS
to dodge async loading; Bevy will face the same choice). These are step-3 details.

---

## 3. Determinism as the regression superpower

This is the most important section, because it changes *what kind of testing we
even do*.

The whole project premise is a **bit-exact deterministic sim**: `processFrame` is
a pure function of (state, input), RNG is part of the state, no floats, fixed
iteration order. The step-2 oracle already produces a **per-tick `HashGameState`
checksum** (+ per-component hashes via `HashGameComponents`, `src/game/stateHash.hpp`).
That determinism is the lever for a regression approach that's *far* more robust
than UI pixel-diffing:

### Replay-driven regression

```
fixed seed  +  recorded input timeline  ─►  run N ticks  ─►  {state checksum, screenshot} at fixed frames  ─►  diff vs golden
```

- **State-checksum gate (authoritative, the real regression test).** Reuse the
  step-2 `HashGameState` time-series oracle. A behaviour change that *shouldn't*
  alter the sim leaves every checksum identical; a change that *does* alter it
  shows the exact tick (and, via the component hash, the exact subsystem) that
  diverged. This is deterministic, platform-stable, and tiny to store (hashes,
  not images). It is the same machinery the roadmap already plans for step-2
  differential testing and step-5 rollback desync detection — **we get the
  regression harness almost for free.**
- **Screenshot gate (human-facing, advisory).** At chosen ticks, also dump a PNG.
  Use it for *visual* review ("does the explosion look right?") and for catching
  *rendering* regressions that don't touch sim state (palette, sprite offset,
  viewport framing). Because GPU rasterization isn't guaranteed bit-identical
  across adapters/drivers, treat pixel-diffs as **advisory / tolerance-based**,
  pinned to one agreed renderer — *not* as the hard pass/fail gate. The hard gate
  is the state checksum.
- **This is exactly what C++ already does.** `framehash` = the screenshot/RGB
  checksum gate; the `test_determinism`/replay suites = the state-checksum gate;
  `videotool` = human-watchable artifact. The Rust equivalents are: the step-2
  oracle harness (state), a Bevy `headless_renderer`/`bevy_capture` mode
  (screenshot/video), and a `ReplayController` (drive). Note even C++ hasn't wired
  `framehash` into CI yet — an easy win that the Rust side should do from the
  start.
- **Tie to `.lrp` / `fast_snapshot`.** The recorded input timeline is the `.lrp`
  concept (initial state + per-tick `ControlState`); `fast_snapshot.hpp`'s field
  inventory defines "what counts as state" for the checksum. A nice property:
  the same recorded timeline drives (a) the state-checksum regression, (b) the
  screenshot regression, (c) a human-watchable video, and (d) later, the step-5
  rollback tests. One artifact, four uses.

**Why this matters for "iterate on behaviour":** when the user/agent changes
gameplay, the question "did I break anything I didn't mean to?" is answered
*objectively and instantly* by the checksum diff, and "does my intended change
look right?" by the screenshot/video. That's the tight loop, with a safety net
that a normal UI app can only dream of.

---

## 4. The web-based hypothesis — honest pros/cons

The user suspects the *interface* may need to become web (wasm) to be
observable. Evaluating that directly:

### Pros of a web/wasm primary observation surface
- **Zero-setup for the user.** Open a URL on any machine/phone; no toolchain, no
  native build. The gh-pages auto-deploy already does this for C++.
- **Easy for Claude Code to drive.** Headless browser + canvas screenshot is a
  well-trodden, stable path (Playwright), and the `run` skill explicitly has a
  **"browser-driven" fallback pattern** — a web build slots straight into it.
- **No native display needed** on CI/agent machines (no xvfb/llvmpipe fiddling).
- **Matches the roadmap goal** (web is an explicit step-3 target anyway), so the
  build has to exist regardless — making it an observation surface is low marginal
  cost.
- **Great for sharing** progress ("here's tick 1000 of the new engine, click").

### Cons / why it should *not* be the *only* surface
- **Slower inner dev loop.** wasm build+bundle+serve+browser is heavier than
  `cargo run` on a native window. For the human's minute-to-minute iteration,
  native is lower friction and full-perf.
- **Determinism debugging is harder in the browser.** When a checksum diverges,
  you want a native debugger, `dbg!`, native test harness, and fast rebuilds —
  not a wasm console. The hard 10% (determinism) is debugged natively.
- **wasm constraints leak** (threads, async asset load, indexed-palette
  rendering). Fighting those *first* would slow steps 3–4; better to get it
  working natively, then bring up wasm.
- **Pixel screenshots are still advisory**, browser or not — the authoritative
  regression gate is the state checksum, which runs fastest as a native headless
  test with no rendering at all.

### Recommendation on web-vs-native
**Don't replace native with web — add web as one of three surfaces.**

| Surface | Primary user | Job | When |
|---|---|---|---|
| **Native window** (`cargo run`) | the human dev | fast inner-loop iteration, full perf, debugging | step 3–4, first |
| **Headless state-checksum + screenshot/replay** | CI + agents | objective regression, "did I break the sim?", auto-screenshots for `run`/`verify` | step 3 (screenshot) / step 4 (replay) |
| **Web (wasm) build** | the user (sharing) + agents (browser-driven obs) | zero-setup viewing, share links, browser screenshot path for `run` | step 3 (reach C++ parity), polish later |

The web build is **valuable and should be built** (it's a roadmap goal and a
genuinely good observation/sharing surface) — just **not at the expense of** the
native dev loop or the determinism-first regression gate.

---

## 5. How Claude Code's `run` / `verify` skills slot in

These are built-in harness skills (no readable markdown on disk; behaviour known
from their descriptions):

- **`run`** — "Launch and drive this project's app to see a change working. First
  looks for a **project skill that already covers launching the app**; otherwise
  falls back to built-in patterns per project type (CLI, server, TUI, Electron,
  **browser-driven**, library)."
- **`verify`** — "Verify a code change by running the app and observing behavior —
  run the app, observe, validate."

What this implies for us:

1. **Ship a project-specific launch skill.** Because `run` *first* looks for a
   project skill, the highest-leverage thing we can add is a small project skill
   (e.g. `.claude/skills/run/SKILL.md` in the repo) that tells the harness exactly
   how to launch and observe *this* app: the native dev command, the
   headless-screenshot command (with a fixed seed so the frame is reproducible),
   how to take and where to find the screenshot, and the web-serve command + the
   browser-driven path. Without this, `run` guesses; with it, the loop is reliable.
2. **Give it a deterministic, screenshot-emitting entry point.** A `verify`-style
   check needs *a frame it can look at and reason about*. That means a
   **fixed-seed "demo" launch** (deterministic scenario so the same frame appears
   every time) plus a **headless screenshot mode** that writes a PNG to a known
   path and exits. Then the loop is literally: `change behaviour → /run (fixed-seed
   headless screenshot) → model views PNG → judge → adjust`.
3. **A replay player makes `verify` powerful.** "Does fix X still produce the same
   match?" = run the recorded `.lrp`-equivalent headless, compare checksum (hard)
   and screenshot at tick N (visual). `verify` can assert both.
4. **A stable CLI is the contract.** `run`/`verify` are only as reliable as the
   launch surface. The app needs a small, stable set of flags (see §6) so the
   skills don't break every time internals change.
5. **Native vs web for the skills:** against a *native window*, the skill relies
   on Bevy's screenshot API / headless mode (needs the scaffolding above; a raw
   `cargo run` window is awkward for an agent to observe). Against a *web build*,
   the skill uses its browser-driven pattern (serve + headless browser +
   canvas screenshot) — arguably the *most* turnkey for an agent, which is another
   point in favour of keeping the web build healthy.

These are **step-3/4 deliverables** — this doc specifies what they must provide;
the step specs build them.

---

## 6. Concrete, minimal recommendation — what to add and when

Ordered, minimal, tied to roadmap steps 3–4. Nothing here is built now; this is
the shopping list for those steps' just-in-time specs.

### During step 3 (Rendering)
1. **Headless screenshot mode** (the single highest-value item). A launch
   flag/mode that renders the current scene to an off-screen target and writes a
   PNG to a known path, then exits — built on Bevy's Screenshot API or the
   `headless_renderer` pattern, runnable on CI via llvmpipe/`WGPU_BACKEND` or in a
   headless browser for the wasm build. *This is what makes `run`/`verify` work
   and what makes visual review possible.*
2. **Fixed-seed "demo" launch.** A deterministic scenario (seed + level + a short
   scripted input or static pose) so screenshots are reproducible frame-for-frame.
   Pairs with #1.
3. **Web build parity + serve recipe.** Bring the Bevy wasm build to where the C++
   emscripten build already is (serve locally + a gh-pages-style deploy), and add
   the minimal JS hook needed to put the app into a known state for browser
   screenshots. (The roadmap requires the wasm build anyway; this just makes it
   *observable*.)

### During step 4 (Loop + input)
4. **Replay player (the `.lrp` equivalent).** A `ReplayController`-style mode that
   loads a recorded seed + per-tick `ControlState` timeline and drives the
   fixed-timestep loop headlessly. Reuses the step-4 deterministic input model.
5. **Replay-driven regression harness in CI.** Wire the step-2 `HashGameState`
   time-series oracle as the *authoritative* gate over a small replay corpus
   (checksum at fixed ticks vs golden) **plus** advisory screenshot diffs at those
   ticks. (Bonus: do for Rust what C++ hasn't yet — actually run `framehash`-style
   visual regression in CI.)
6. **Project `run` launch skill + stable CLI.** Commit a repo `.claude/skills`
   launch skill documenting: native dev run, fixed-seed headless screenshot, web
   serve + browser-driven observation, and replay playback — backed by a small,
   stable flag set. This is the glue that makes the agent loop dependable.

### The resulting loop (what "good" looks like)
- **Human, inner loop:** `cargo run` (native window), fixed-seed demo, eyeball it.
- **Agent / `run` / `verify`:** change behaviour → fixed-seed headless screenshot
  (native or browser) → model views the PNG → judge → adjust.
- **Regression (CI + on demand):** replay corpus → state-checksum diff (hard
  gate) + screenshot diff at fixed ticks (advisory) → exact diverging tick on
  failure.
- **Sharing:** push → gh-pages wasm build → click a URL.

---

## Open questions for the controller

- **Where should the project launch skill live** so both the user and dispatched
  agents discover it — repo `.claude/skills/run/` (committed, travels with the
  repo) vs `~/.claude`? (Recommendation: committed in-repo so agents in worktrees
  inherit it.)
- **Pixel-screenshot golden policy:** pin one renderer (which? llvmpipe in CI vs a
  named GPU) and a tolerance, or keep screenshots strictly advisory and never a
  CI gate? (This doc leans advisory + state-checksum as the hard gate.)
- **wasm screenshotting cost:** is the team willing to add a headless-browser
  (Playwright) dependency to CI for canvas screenshots, or keep wasm observation
  manual and rely on native headless for automated visual checks?
- **How early to introduce the replay/input-timeline format** — step 4 needs it,
  but defining it in step 2's oracle (the input-vector format is already an open
  question there) could let one format serve oracle + replay + regression + step-5
  rollback. Worth resolving when step 2's real spec is written.

---

*Exploration only. No code written, no commits, `rust/` and `src/` untouched.*
