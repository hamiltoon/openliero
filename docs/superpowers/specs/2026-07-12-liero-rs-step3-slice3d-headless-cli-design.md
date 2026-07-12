# Step 3 · Slice 3d — headless screenshot CLI + in-repo run-skill: detailed design

Status: **draft for review** · 2026-07-12
Part of: `2026-07-10-liero-rs-step3-rendering-overview.md` (cited as **overview**; 3d scope: "headless
screenshot CLI + in-repo run-skill (change → screenshot → judge, no GPU)")
Sources: `2026-06-26-liero-rs-interactive-iteration-exploration.md` (**iteration §N** — the run-skill
requirements: fixed-seed screenshot to a known path, a stable CLI, state-checksum as the hard gate),
`2026-07-12-liero-rs-step3-slice3c-bevy-window-design.md` (**3c** — the shared `scenario` crate and its
`load`/`SceneData` this slice reuses), the shipped `render_snapshot.rs` dev BMP dumper (3d's starting
point), and the T8 harness `render_slice3b_common::render_tick` (the per-tick render semantics this
slice must reproduce).
Companion output feeds: `superpowers:writing-plans` (this is the spec; the plan is the companion).

3a+3b shipped a pixel-exact, differential-tested CPU frame; 3c put it on screen live via Bevy. **3d is
the automated / agent observation surface** (iteration §1a TL;DR surface #2): a **Bevy-free, GPU-free**
CLI that drives a fixed-seed scenario to a chosen tick, PNG-encodes the CPU frame to a known path, and
(machine-readably) dumps the frame-hash + state-hash so the change → screenshot → judge loop — and the
authoritative replay-checksum regression — work with no window, no wgpu, no display.

**3d is NOT bit-gated in a new way.** The hard render gate is still the FNV-1a frame hash, already green
in 3b. What 3d adds is a *packaging* of the already-proven render path behind a stable CLI, plus a proof
that the CLI's render path reproduces a committed 3b golden sidecar (so the screenshot it emits is
provably the pixel-exact frame). The PNG itself is *advisory* (iteration §3): the hash dump is the gate.

---

## Goal of 3d

A new **Bevy-free** binary `shot` such that:

```
cargo run -p shot -- --scenario blood --tick 35 --out /tmp/blood_t35.png
```

drives the `blood` scenario deterministically from tick 0 to tick 35 (rendering every intervening tick —
the viewport-RNG caveat below), PNG-encodes tick 35's world-view frame at nearest ×3 to
`/tmp/blood_t35.png`, and exits 0. With `--hashes` it also prints the sidecar-format
`<tick> <frame_hash_hex16> <state_hash_hex8>` lines (+ `total`) to stdout for regression diffing. An
in-repo `.claude/skills/` run-skill documents this entrypoint so a future agent can build, screenshot at
a fixed seed/scenario/tick, find the PNG on a known path, and compare against the golden/state-hash.

**Done when:**
1. `cargo run -p shot -- --scenario <name> --tick <N> --out <path>` writes a deterministic PNG of tick
   `N`'s world view (byte-identical across two runs / two machines — no GPU in the path).
2. The CLI reproduces a committed 3b frame-hash golden **exactly** for at least one scenario (a
   `shot`-crate integration test drives the library over `0..=ticks` and asserts every
   `frame_hash` + the `total` accumulator equal `golden/render_slice3b_blood.txt` — proving the emitted
   screenshot is the pixel-exact frame).
3. `--hashes` emits the machine-readable sidecar lines (the authoritative regression hook).
4. An in-repo `.claude/skills/` run-skill drives the CLI (build → screenshot → judge → compare) and is
   discoverable by the `run`/`verify` harness skills (iteration §5).
5. The determinism CI gate stays **Bevy-free and green**; the new crate compiles and its golden test
   runs inside the existing fast `--exclude game` job (the CLI is Bevy-free — it belongs in the fast
   gate, not behind the separate `game` build step).

**Not in 3d:** HUD/font/bars/minimap (3e — the CLI renders the same world view 3b/3c do, HUD-less); wasm
/ headless-browser screenshotting (3f); `.lrp` replay + input timelines (Step 4 — 3d drives the existing
fixed-seed *scenario* corpus, not recorded replays); follow-cam; render interpolation; video/GIF (the
`videotool` analog — out of scope, the CLI emits single frames).

---

## Why a CLI target, not another example

`render_snapshot.rs` (`oracle-tests/examples/`) already dumps ×3 BMP frames headlessly and is the
*shape* 3d generalizes. But it is a **dev example**, not a product surface: it is invoked as
`cargo run -p oracle-tests --example render_snapshot`, its args are positional and undocumented, it
writes BMP (not the PNG the run-skill/`verify` loop wants), it has **no golden self-proof**, and — the
load-bearing gap — it renders each requested tick with `as_scene(0, scenario.shadow())`, i.e. it hard-
codes `screen_flash = 0` and injects **no** `shake`, so it does **not** reproduce the goldens for the
`shake`/flash scenario. iteration §5/§6 asks for a *stable CLI* (item 6: "a small, stable set of flags")
and a *fixed-seed screenshot mode writing a PNG to a known path* (item 1/2). 3d is that: a real binary
with a documented flag contract, PNG output, the full per-tick render semantics, and a golden proof.

The `image` crate is a new dependency — kept minimal: `default-features = false` + only the `png`
feature (no JPEG/GIF/TIFF/WebP decoders, no `rayon`). It is pulled **only** by `shot`; the determinism-
relevant crates (`render`/`sim`/`assets`/`sim-core`/`scenario`) never gain it.

---

## Crate setup — `rust/shot` (lib + bin, Bevy-free)

A new workspace member `rust/shot`, **edition 2021** (like every crate except `game`), Bevy-free. It is a
**lib + bin**: the drive/render/hash/encode logic lives in `src/lib.rs` (pure, unit- and golden-testable
without spawning a process); `src/main.rs` is a thin arg-parse + call-into-lib shell.

### `rust/shot/Cargo.toml`

```toml
[package]
name = "shot"
version = "0.1.0"
edition = "2021"

[dependencies]
scenario = { path = "../scenario" }   # load() + SceneData + the parser (from 3c)
render   = { path = "../render" }      # Bitmap, frame::draw, hash::{hash_frame, FNV_*}
sim      = { path = "../sim" }         # SimState, ControlState, hash::hash_game_state
assets   = { path = "../assets" }
sim-core = { path = "../sim-core" }

# PNG encoder only — no other formats, no rayon. Keep the dep surface tiny.
image = { version = "0.25", default-features = false, features = ["png"] }
```

**Decisions taken:**

- **Bevy-free ⇒ stays in the fast determinism gate.** `cargo test --workspace --exclude game` already
  compiles + tests every Bevy-free member; `shot` (and its golden test) ride that job. The `game`
  exclusion is about Bevy, not about "non-sim crates" — `shot`'s golden test is a *differential proof*
  and belongs beside the sim/render goldens in the fast gate (overview *Oracle strategy*). So CI needs
  **no structural change** beyond the crate becoming a member (which `--workspace` picks up).
- **lib + bin.** The golden-verification test needs to call the render+hash path directly (driving a
  process and re-parsing stdout would be brittle). Exposing `shot::render_scenario` as a library function
  makes the golden test a plain `#[test]` in `shot/tests/`, and keeps `main.rs` to arg-parsing.
- **No `clap`.** The flag set is tiny and stable; hand-rolled `std::env::args` parsing (mirroring the
  project's zero-extra-dep posture and `render_snapshot`'s own arg handling) avoids another dependency.
  *(Open question 3 — if the controller prefers `clap` for `--help` ergonomics, it is a small add.)*

---

## The render path — reuse `scenario::load` + the full per-tick semantics

3d introduces **no new render architecture** (overview locked-decision; 3c *Deferrals*: "the shared
`scenario::load` is exactly what 3d reuses"). The CLI's core is `render_snapshot`'s loop **plus** the T8
`render_tick`'s flash/shake/shadow/fade handling, so it is golden-faithful for *every* scenario:

```rust
// shot::render_scenario — the whole headless path, Bevy-free, GPU-free.
pub struct Frame {
    pub tick: u32,
    pub frame_hash: u64,   // render::hash::hash_frame(&bmp, fade)  — the 3b gate value
    pub state_hash: u32,   // sim::hash::hash_game_state(&state)     — the isolation column
    pub png: Option<Vec<u8>>, // Some(...) only for requested ticks (see `wanted`)
}

/// Drive `scenario` from tick 0 to `up_to`, rendering EVERY tick (viewport-RNG
/// correctness), returning one `Frame` per tick 0..=up_to. PNG bytes are attached
/// only for ticks in `wanted` (encode is the expensive part; skip it otherwise).
pub fn render_scenario(
    tc_root: &Path, scenario: &Scenario, up_to: u32, wanted: &[u32], scale: u32,
) -> Vec<Frame> { … }
```

The load-bearing details it must copy from `render_slice3b_common::render_tick`
(`render_slice3b_common/mod.rs:146-176`) and `run` (`:257-296`) — get any of these wrong and the frame
hash diverges from the golden:

1. **Recorded inputs, not empty.** Advance with `ControlState::unpack(scenario.input(k-1, worm))` for
   both worms — **not** `ControlState::default()`. (This is exactly the bug 3c hit: empty inputs diverge
   from the golden. `render_snapshot.rs:82-86` already does this correctly.)
2. **Tick 0 is rendered/hashed BEFORE the first `process_frame`** (the sim-golden convention). The loop
   is: render tick 0 → for k in 1..=up_to { `process_frame(inputs[k-1])`; render tick k }.
3. **Per-tick flash + shake, inject-and-restore.** For each drawn tick: `screen_flash =
   scenario.flash_at(tick).unwrap_or(0)`; for each `(vp, amount)` in `scenario.shake_at(tick)` set
   `viewports[vp].shake = itof(amount)` **before** the draw and reset to `0` **after** (dumper
   semantics). `draw_shadow = scenario.shadow()`.
4. **Fade convention for the hash.** `fade = if tick == 0 { 0 } else { 33 }`, passed to
   `render::hash::hash_frame`. Frame 0 hashes fully black; all later frames identity.
5. **The `total` accumulator** folds every tick's frame hash: seed `render::hash::FNV_OFFSET`, then
   `acc = (acc ^ fh).wrapping_mul(render::hash::FNV_PRIME)` per tick — matching C++ `framehash`'s
   `total` line and the T8 `run` accumulator.

### Viewport-RNG caveat — render ALL ticks up to the target (do not skip)

The draw path is **not** side-effect-free: `DrawLaserSight` and shake advance the viewport-local `Rand`
once per draw (overview *Risks*, render-map §6). So the viewport RNG state at tick `N` depends on having
**drawn every tick 0..N**, not just tick `N`. A CLI that rendered *only* the requested tick would produce
the wrong RNG sequence for any scenario with laser/shake and its frame would not match the golden. **The
CLI therefore renders every tick 0..=`up_to`** (encoding a PNG only for the `wanted` subset). This is the
exact reason `render_snapshot` loops `for k in 0..=max_tick` and renders inside the loop. Document this in
the CLI help so a future caller understands why `--tick 500` on a 500-tick scenario is not instant.

### PNG encoding — raw RGB, NOT the faded hash bytes

The frame **hash** applies `FadeChannel` and blacks out frame 0 (`fade=0`) — that is a *hashing* artifact
for cross-checking C++, **not** what the screenshot should show. The **PNG** must encode the *actual*
palette-resolved colors so a human/agent can judge it. So:

- Source: the `Bitmap` `pixels: Vec<u32>` packed `0xAARRGGBB` (`bitmap.rs:70`).
- Per pixel → RGB byte order `[(px>>16)&0xff, (px>>8)&0xff, px&0xff]` (drop alpha — the CPU frame's alpha
  is always `0xFF`; PNG can be `Rgb8`). This is the same channel extraction the frame hash uses **minus
  the fade**, and it honors `pitch != w` (source stride) exactly as `render_snapshot::write_bmp` does.
- Upscale **nearest ×`scale`** (default 3, matching the 3c window and `render_snapshot`): `sx = x/scale`,
  `sy = y/scale`. Nearest keeps pixels crisp; no filtering.
- Encode via `image::RgbImage` + `image::codecs::png::PngEncoder` (or `RgbImage::save`) to the `--out`
  path. Deterministic bytes (PNG encoding of identical pixels is stable for a fixed encoder config).

**Determinism of the PNG file:** identical pixels + fixed `image` version ⇒ byte-identical PNG. The
regression *gate* is nonetheless the frame **hash** (encoder-version-independent), not a PNG byte-diff —
mirroring iteration §3 (screenshots advisory, checksums authoritative). A `shot` test asserts two runs
produce the same frame *hash* (and, as a bonus, the same PNG bytes under the pinned `image` version).

---

## CLI flag contract (stable — the run-skill depends on it)

Keep it small and stable (iteration §6 item 6). Proposed:

| Flag | Meaning | Default |
|---|---|---|
| `--scenario <name>` | committed 3b scenario name (`blood`, `dart`, `dart_water`, `laser`, `shadow`, `shake`, `fan`) | required |
| `--tick <N>` | target tick to screenshot; repeatable for multiple frames in one drive | required (≥1 occurrence) |
| `--out <path>` | output PNG path (single `--tick`) or **directory** (multiple `--tick`; files `<name>_tick<N>.png`) | required unless `--hashes`-only |
| `--scale <n>` | nearest integer upscale factor | `3` |
| `--hashes` | also print `<tick> <frame_hash_hex16> <state_hash_hex8>` for every tick `0..=max(--tick)` + a `total` line to **stdout** (machine-readable regression hook) | off |
| `--tc-root <path>` | TC assets root | `CARGO_MANIFEST_DIR/../../data/TC/openliero` |

- **Scenario name validation:** resolve `<tc-golden>/render_slice3b_<name>_scenario.txt`; if absent,
  print the available names + exit non-zero. The scenario text + `--tc-root` default use the compile-time
  `CARGO_MANIFEST_DIR` convention (3c *Asset path resolution*), so `cargo run -p shot` works from any CWD.
- **stdout discipline:** the PNG goes to a *file*; `--hashes` output goes to *stdout* in the exact
  sidecar grammar (`parse_frames`, `render_slice3b_common/mod.rs:52-78`) so an agent/test can diff it
  against a committed golden with no reformatting. Progress/info messages (if any) go to stderr.
- **Exit codes:** `0` success; non-zero on bad args / unknown scenario / IO error. (No `--verify`
  built-in — golden comparison is a test + a skill step, keeping the CLI a pure producer.)

---

## Open question adjudicated in this spec — machine-readable hash dump: **YES**

> The overview's open question for 3d (and iteration §3/§6) asks whether the CLI should also dump the
> state-hash/frame-hash to stdout for regression comparison.

**Recommendation: YES — ship `--hashes`.** iteration §3 is explicit that the **state checksum is the
authoritative regression gate and screenshots are advisory**. A screenshot alone cannot answer "did I
break the frame?" objectively across machines (and a PNG byte-diff is encoder-fragile); the FNV-1a frame
hash + the sim state hash can, and they are already the currency of the 3b goldens. Emitting them in the
committed sidecar grammar means the run-skill's "compare against golden" step is a trivial `diff`, and it
future-proofs the CLI as the Step-4 replay-regression driver (iteration §6 item 5). The cost is ~10 lines
(the accumulator + a formatted print). This makes the CLI serve **both** observation surfaces the loop
needs: the PNG (human/agent visual judgement) and the hash (objective regression).

---

## Where the per-tick render semantics live — adjudicated: **CLI-local, T8 untouched**

The flash/shake/shadow/fade per-tick logic currently exists only in the T8 test harness
`render_slice3b_common::render_tick`. 3d needs the same logic. Three options:

- **(A) Factor a driver into `scenario`** (a `Playback`/`render_tick` helper) reused by both the CLI and
  T8 — the "one source of per-tick truth" payoff, mirroring 3c's `load()` factor-out.
- **(B) CLI carries its own ~20-line `render_tick` copy in `shot::lib`; T8 untouched.** *(recommended)*
- (C) Duplicate inline in `main.rs` (rejected — not testable).

**Recommendation: (B).** The slice charter is "no new architecture, small slice"; re-touching the T8
harness (A) re-opens the "refactor must not change a golden byte" gate for **zero functional gain in
3d** (the CLI still needs its own encode/arg surface regardless), and adds risk to a slice whose value is
the CLI + skill. The ~20-line per-tick copy lives in `shot`'s **library** (not `main`), so it is directly
unit- and golden-tested — and the golden-verification test (done-when 2) *is* the proof the copy is
faithful. If a **third** consumer appears (e.g. the 3f wasm harness, or Step-4 replay regression wanting
the same drive loop), factoring the driver into `scenario` at that point is the right call — flag it as a
deferred cleanup, not a 3d task. *(Open question 1 — the controller may prefer (A) now for DRY; the
recommendation is (B) for a minimal, low-risk slice.)*

---

## The in-repo run-skill — `.claude/skills/`

iteration §5 (and the overview open question Q5) asks *where* the project launch/observe skill lives:
in-repo `.claude/skills/` (travels to worktrees, agents inherit it) vs `~/.claude`.

**Recommendation (as the overview standing rec): in-repo.** Committed under the worktree's
`.claude/skills/` so any dispatched agent working a Liero-rs worktree discovers it via the `run`/`verify`
harness (which "first looks for a project skill that already covers launching the app", iteration §5
item 1). The skill file's *placement* is outside `docs/` (it is `.claude/skills/…/SKILL.md`), so this
spec only **describes** its content; the implementer task creates it.

### Skill content (what the plan's skill task must produce)

A `SKILL.md` (name e.g. `liero-shot` / `render-observe`) whose body documents, for a future agent:

1. **Build:** `cargo build --manifest-path rust/Cargo.toml -p shot` (fast — Bevy-free; **not** the
   minutes-long `game` Bevy build). No GPU / display needed.
2. **Screenshot at a fixed seed/scenario/tick:**
   `cargo run -p shot -- --scenario blood --tick 35 --out <known-path>.png` → PNG on a known path the
   agent then *views* to judge the change (the change → screenshot → judge loop, iteration §6).
3. **Deterministic-regression compare (the hard gate):**
   `cargo run -p shot -- --scenario blood --tick 40 --hashes` → diff the emitted sidecar lines against
   `rust/oracle-tests/golden/render_slice3b_blood.txt`; any frame-hash or `total` mismatch = a render
   regression at the named tick (state-hash mismatch = a *sim* regression). iteration §3: checksum is
   authoritative, screenshot advisory.
4. **The available scenarios** (the 7 committed 3b goldens) and the flag contract, so the agent can pick
   a scenario that exercises the subsystem it changed (laser → `laser`, shadows → `shadow`, blood/carve →
   `blood`, water → `dart_water`, shake+flash → `shake`).
5. **Fixed-seed note:** every scenario is seed-42, input-scripted, and GPU-free — the same frame every
   run, on any machine (no adapter dependence, overview *GPU-independence of the gate*).

The skill is a **docs-style deliverable** (a plan task), but its file lands at `.claude/skills/…` — the
plan describes the content; the implementer writes the file.

---

## Verification

- **Golden faithfulness (the real proof, done-when 2):** a `shot/tests/` integration test drives
  `shot::render_scenario` over `0..=ticks` for `blood` (min) and asserts each `frame_hash` == the
  `golden/render_slice3b_blood.txt` line and the accumulated `total` == the golden `total`. This proves
  the CLI's render path is the pixel-exact 3b path (not a drifted copy). Extending to a second scenario
  that exercises flash+shake (`shake`) is a cheap, high-value add — it is the only scenario where the CLI
  copy differs from `render_snapshot`, so it directly guards the (B) decision.
- **Determinism of output:** a test asserting two `render_scenario` runs yield identical frame hashes
  (and identical PNG bytes under the pinned `image`).
- **Arg-parser unit tests:** pure `parse_args(&[…]) -> Config` cases (required flags, repeated `--tick`,
  default scale, unknown scenario error, `--hashes`-only with no `--out`).
- **Smoke:** `cargo run -p shot -- --scenario blood --tick 5 --out <tmp>.png` writes a non-empty PNG and
  exits 0; `--hashes` prints parseable sidecar lines.
- **CI:** the existing fast job (`cargo test --workspace --exclude game`) compiles `shot` and runs its
  golden test — **no workflow edit needed** beyond the crate being a member (confirm the `image` dep does
  not pull anything heavy; `default-features=false`+`png` keeps it to `png`/`miniz_oxide`/`flate2`).

---

## Test list

1. **Golden frame-hash reproduction** (`shot/tests/golden.rs`): `blood` (required) — every tick's
   `frame_hash` + `total` == `render_slice3b_blood.txt`; state-hash column == `hash_game_state`. Add
   `shake` (recommended) to cover flash+shake per-tick injection (the (B)-copy's only novel path).
2. **Output determinism** (`shot/tests/` or lib `#[cfg(test)]`): two `render_scenario` runs → equal frame
   hashes (and equal PNG bytes).
3. **PNG shape** (lib unit): a 1×1 / 2×1 hand-built `Bitmap` → `encode_png` yields the expected
   dimensions (`w*scale × h*scale`) and the expected top-left RGB triple; guards the ARGB→RGB channel
   order and the `pitch`-vs-`w` stride (mirrors the `bitmap.rs`/3c-blit tests).
4. **Arg parser** (lib unit): the flag cases above.
5. **Viewport-RNG-all-ticks guard** (lib unit or golden): rendering only the target tick vs rendering
   all ticks up to it produces *different* hashes for `laser`/`shake` (proves the "render all ticks" rule
   is load-bearing, not incidental) — the render analog of the 3b non-vacuity controls.

---

## Risks & the hard 10%

- **Skipping intermediate ticks corrupts the viewport RNG** (the single biggest trap). The CLI **must**
  render every tick `0..=up_to`. Test #5 guards it.
- **Empty vs recorded inputs** — the 3c-repeat trap. Use `scenario.input(k-1, worm)`; test #1 catches a
  regression (the golden would not match).
- **Fade in the PNG.** The frame hash blacks out frame 0 (`fade=0`) and fades — the **PNG must not**.
  Encode raw RGB; the hash and the PNG are computed from the same buffer but the hash applies fade and
  the PNG does not. Getting this wrong yields an all-black `--tick 0` screenshot.
- **`image` feature creep.** Keep `default-features = false` + `png` only — no rayon/jpeg/etc. Confirm
  `cargo tree -p shot` shows no `bevy*` and no unexpected heavy dep. The determinism gate must stay lean.
- **stdout contamination.** If any info line leaks to stdout, `--hashes` output stops being a clean
  diff target. Route all non-hash output to stderr; test the stdout is pure sidecar grammar.
- **Encoder-version PNG drift.** A future `image` bump could change PNG bytes (not pixels). The *gate* is
  the frame hash, not PNG bytes — never gate CI on a PNG byte-diff (iteration §3, advisory screenshots).

---

## Deferrals (explicitly out of 3d)

- HUD / font / bars / minimap — **3e** (the CLI renders the same HUD-less world view as 3b/3c).
- Wasm / headless-browser canvas screenshot (Playwright) — **3f** (overview Q6).
- `.lrp` replay + recorded input timelines — **Step 4** (3d drives the fixed-seed *scenario* corpus; the
  same CLI becomes the replay-regression driver when replays land, iteration §6 item 5).
- Factoring the per-tick render driver into `scenario` (option A) — deferred until a third consumer needs
  it; the CLI carries a tested local copy for now.
- Video / GIF output (the `videotool` analog) — out of scope; the CLI emits single frames.
- Follow-cam, render interpolation, resize-aware scaling — overview/3c deferrals, unrelated to 3d.
- Wiring `--hashes` diff into CI as a standing job — the golden *test* already gates the render path in
  CI; a CLI-driven diff job is redundant here (revisit if replay corpora grow in Step 4).

---

## Open questions for the controller (max 3)

1. **Per-tick driver location.** Accept **(B)** — the CLI carries its own tested ~20-line per-tick
   `render_tick` (flash/shake/shadow/fade) in `shot::lib`, T8 untouched — vs **(A)** factoring a
   `scenario::playback` driver reused by both now? (Recommendation: **(B)** — minimal, low-risk slice;
   factor when a third consumer appears.)
2. **Golden-test scenario coverage.** Require the CLI golden test to cover **`blood` only** (minimal), or
   **`blood` + `shake`** (adds the flash+shake per-tick path — the only path the CLI copy adds over
   `render_snapshot`)? (Recommendation: **both** — `shake` is the cheap, targeted guard for decision B.)
3. **Arg parsing dep.** Hand-rolled `std::env::args` parsing (zero new dep, matches project posture) vs
   adding `clap` for `--help`/validation ergonomics? (Recommendation: **hand-rolled** — the flag set is
   tiny and stable; revisit if the flag surface grows.)

---

## Next artifact

The companion implementation plan: `plans/2026-07-12-liero-rs-step3-slice3d-plan.md`.
