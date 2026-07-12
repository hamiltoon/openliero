# Step 3, Slice 3d — headless screenshot CLI + in-repo run-skill: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended)
> or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`)
> syntax. **Test-first**: write the failing test (or its assertion) before the implementation it pins,
> run it and SEE it fail, then make it pass. Each task is executed by a **separate** implementer subagent
> that does **not** see the other tasks' context — every task brief stands on its own.

**Goal:** Ship a new **Bevy-free, GPU-free** binary `shot` (a workspace member `rust/shot`, lib + bin)
that drives a fixed-seed 3b scenario to a chosen tick, PNG-encodes that tick's world-view frame to a
known path, and — with `--hashes` — prints the machine-readable `<tick> <frame_hash> <state_hash>`
sidecar lines for regression diffing. Plus an **in-repo** `.claude/skills/` run-skill that documents the
build → screenshot → judge → compare loop so a future agent (and the `run`/`verify` harness) can drive
it. This is the automated/agent observation surface of iteration §1a — no window, no wgpu, no display.

**3d is NOT bit-gated in a new way.** The hard render gate is the FNV-1a frame hash, already green in 3b.
3d *packages* the already-proven render path behind a stable CLI and **proves the CLI reproduces a
committed 3b golden sidecar exactly** (so the PNG it emits is provably the pixel-exact frame). The PNG is
advisory; the hash dump is the gate (iteration §3).

**Architecture:** One new crate, **no** new render architecture. `rust/shot` is a **lib + bin**:
`src/lib.rs` holds the pure, testable drive/render/hash/encode path (reusing `scenario::load` from 3c +
the T8 `render_tick` per-tick semantics, copied CLI-local); `src/main.rs` is a thin
`std::env::args` → `Config` → call-into-lib shell. The `image` crate (`default-features=false`, `png`
only) is the sole new dependency and is pulled **only** by `shot`. `render`/`sim`/`assets`/`sim-core`/
`scenario` are unchanged and stay Bevy-free.

Crate graph after 3d (no cycle; all Bevy-free except `game`):

```
sim-core ◄ assets ◄ sim ◄ render
                      ▲       ▲
                      └── scenario ──┘        (from 3c: load() + SceneData + parser)
                          ▲   ▲   ▲
                       game  oracle  shot     (NEW bin+lib: + image[png]; Bevy-free)
                    (bevy)   -tests
```

**Tech stack:** Rust, edition 2021 (like every crate except `game`). `shot` new lib+bin. `image` 0.25
(`default-features=false`, features `["png"]`). Bevy-free ⇒ compiles + tests in the existing fast
`--exclude game` CI job (no minutes-long Bevy build; no GPU). Real acceptance is the golden-faithfulness
test (the CLI reproduces `render_slice3b_blood.txt` bit-for-bit) + a smoke that a PNG lands on a known
path.

## Global constraints

*(inherit every 3a/3b/3c constraint that still applies; the 3d-specific ones follow)*

- **No new render architecture.** 3d reuses `scenario::load` (3c) + the shipped `render::frame::draw` /
  `render::hash::hash_frame`. The only genuinely new logic is the CLI's arg parsing, the PNG encode, and
  a CLI-local copy of the T8 `render_tick` per-tick semantics. Do **not** add render features, do **not**
  touch `render`/`sim`/`assets`/`sim-core`/`scenario` source.
- **`shot` is Bevy-free, edition 2021.** Deps exactly `scenario`/`render`/`sim`/`assets`/`sim-core` (path)
  + `image` (`default-features=false`, `features=["png"]`). `cargo tree -p shot` must show **no `bevy*`**
  and no JPEG/GIF/rayon pull. It is a workspace member, so the fast determinism gate compiles and tests
  it — keep it fast and lean.
- **Golden faithfulness is the hard proof.** The CLI's render path must reproduce the committed 3b frame
  hashes **exactly**. That means copying the T8 `render_slice3b_common::render_tick` semantics precisely
  (`render_slice3b_common/mod.rs:146-176`, `run` `:257-296`):
  1. Advance with **recorded** inputs `ControlState::unpack(scenario.input(k-1, worm))` for both worms —
     **never** `ControlState::default()`.
  2. **Tick 0 is rendered/hashed BEFORE the first `process_frame`**; then `for k in 1..=up_to {
     process_frame(inputs[k-1]); render(k) }`.
  3. Per drawn tick: `screen_flash = scenario.flash_at(tick).unwrap_or(0)`; for each
     `(vp, amount) in scenario.shake_at(tick)` set `viewports[vp].shake = itof(amount)` **before** the
     draw and reset to `0` **after**; `draw_shadow = scenario.shadow()`.
  4. Hash with `fade = if tick == 0 { 0 } else { 33 }` via `render::hash::hash_frame`.
  5. `total` accumulator: seed `render::hash::FNV_OFFSET`, `acc = (acc ^ fh).wrapping_mul(render::hash::FNV_PRIME)` per tick.
- **Render EVERY tick 0..=target — never skip.** The viewport-local `Rand` (laser sparks / shake)
  advances once per draw, so tick `N`'s frame depends on having drawn every tick `0..N`. Rendering only
  the requested tick would corrupt the RNG for laser/shake scenarios and miss the golden. Render all
  ticks `0..=up_to`; PNG-encode only the requested (`wanted`) subset (encode is the costly part).
- **PNG is raw RGB, NOT the faded hash bytes.** The frame hash blacks out frame 0 (`fade=0`) and applies
  `FadeChannel` — a hashing artifact. The PNG must encode the *actual* colors: per pixel
  `[(px>>16)&0xff, (px>>8)&0xff, px&0xff]` (drop alpha), nearest ×`scale`, honoring `pitch != w`. Do NOT
  apply fade to the PNG (else `--tick 0` is all black).
- **stdout discipline.** The PNG goes to a **file**; `--hashes` output goes to **stdout** in the exact
  sidecar grammar (`<tick> <frame_hash_hex16> <state_hash_hex8>` per line + a `total <n> <acc_hex16>`
  line — the `parse_frames` grammar, `render_slice3b_common/mod.rs:52-78`). Any info/progress message
  goes to **stderr**, so `--hashes` stdout stays a clean `diff` target.
- **CARGO_MANIFEST_DIR paths, not CWD.** Default TC root `concat!(env!("CARGO_MANIFEST_DIR"),
  "/../../data/TC/openliero")`; scenario text + goldens `concat!(env!("CARGO_MANIFEST_DIR"),
  "/../oracle-tests/golden/…")` (overridable via `--tc-root`). `cargo run -p shot` works from any CWD.
- **CI keeps the determinism gate fast + Bevy-independent.** `shot` is Bevy-free, so it rides the
  existing `cargo test --manifest-path rust/Cargo.toml --workspace --exclude game` job — **no `--exclude
  shot`, no separate build step.** The only CI action is confirming the crate + its golden test run
  there and the `image` dep does not bloat the job (verify `cargo tree`).
- **`cargo fmt` footgun.** Format **only new files** — `cargo fmt -p shot` (or `rustfmt` on the specific
  new files). Do NOT run a workspace-wide `cargo fmt`.
- **Bash / tooling discipline (implementers).** One command per call; no `>>`/heredoc/`&&`/`;`/`$VAR`.
  **Never run `cd` and never run git for navigation/state** — use `git -C <worktree>` only for the
  explicit commit steps below, and read files with the Read tool via absolute paths. Create/edit files
  with the editor tools, not shell redirection. First `cargo build -p shot` is fast (Bevy-free) — a
  default timeout is fine.
- **No sub-subagents.** An implementer task must **not** spawn its own subagents; do the work directly.
- **No AI / "Generated with" taglines; no commit trailers** (no `Co-Authored-By`, no `Claude-Session`,
  no "Generated with Claude Code"). **Commit on branch `liero-rs-step-3`** (the accumulating Step-3 PR).
  **Do NOT push and do NOT open a PR** — the controller owns push + PR.

## Preflight (before T0)

- [ ] Confirm `render_snapshot.rs` (`rust/oracle-tests/examples/render_snapshot.rs`) and
      `render_slice3b_common/mod.rs` are the two reference sources for the drive loop; confirm the 7
      committed goldens exist (`ls rust/oracle-tests/golden/render_slice3b_*_scenario.txt`:
      blood/dart/dart_water/laser/shadow/shake/fan).

## File structure

- **New crate `rust/shot/`** (T0–T2):
  - `rust/shot/Cargo.toml` — lib+bin, edition 2021, deps `scenario`/`render`/`sim`/`assets`/`sim-core`
    (path) + `image` (`default-features=false`, `features=["png"]`).
  - `rust/shot/src/lib.rs` — `Config`, `parse_args`, `render_scenario`, `encode_png`, the per-tick
    render helper, and their unit tests (Bevy-free, `cargo test -p shot`).
  - `rust/shot/src/main.rs` — thin `std::env::args` → `Config` → call `shot::run(cfg)`; stdout/stderr
    routing; exit codes.
  - `rust/shot/tests/golden.rs` — golden-faithfulness integration test (T2).
- `rust/Cargo.toml` — add `"shot"` to `members`.
- `rust/Cargo.lock` — regenerated with the `image` (png) subtree (T0; small diff).
- **Run-skill** (T3): `.claude/skills/<skill-name>/SKILL.md` (placement outside `docs/`; content
  described in T3).
- `.github/workflows/rust.yml` — confirmed unchanged / minimal (T4).
- `docs/superpowers/liero-rs-PROGRESS.md` + the rendering overview's 3d line + open-questions +
  deferral ledger (T4).

## Tasks

---

### T0 — `shot` crate skeleton: lib+bin, `image` dep, arg parser (RED-first)  [Opus]

**Files**
- Create: `rust/shot/Cargo.toml`, `rust/shot/src/lib.rs`, `rust/shot/src/main.rs`
- Modify: `rust/Cargo.toml` (add `"shot"` to `members`)
- Regenerate: `rust/Cargo.lock` (`image` png subtree — small diff)

**Interfaces**
- Produces a `shot` binary + library. The library exposes a pure arg parser:
  ```rust
  pub struct Config {
      pub scenario: String,
      pub ticks: Vec<u32>,     // one or more --tick values (target frames)
      pub out: Option<PathBuf>,// PNG path (single tick) or dir (multi tick); None if --hashes-only
      pub scale: u32,          // default 3
      pub hashes: bool,        // --hashes
      pub tc_root: Option<PathBuf>, // --tc-root override; None => compile-time default
  }
  pub fn parse_args(args: &[String]) -> Result<Config, String>;
  ```
- `main.rs` calls `parse_args(&std::env::args().skip(1).collect::<Vec<_>>())`; on `Err`, prints the
  message + a short usage to **stderr** and exits non-zero.

**`rust/shot/Cargo.toml`**
```toml
[package]
name = "shot"
version = "0.1.0"
edition = "2021"

[dependencies]
scenario = { path = "../scenario" }
render   = { path = "../render" }
sim      = { path = "../sim" }
assets   = { path = "../assets" }
sim-core = { path = "../sim-core" }
image = { version = "0.25", default-features = false, features = ["png"] }
```

**Why (teaching note):** `shot` is the "stable CLI" iteration §6 item 6 asks for — a small, documented
flag surface the `run`/`verify` harness can depend on without breaking when internals change. It is a
**lib + bin** so the render/hash path is a library function the golden test (T2) calls directly, rather
than driving a process and re-parsing stdout (brittle). Parsing is hand-rolled (`std::env::args`) — the
flag set is tiny and the project keeps its dependency surface minimal; `image` with
`default-features=false` + `png` pulls only the PNG encoder (`miniz_oxide`/`flate2`), not the whole codec
zoo. Being Bevy-free, `shot` compiles fast and lives in the fast determinism CI gate.

**Steps**
- [ ] Add `"shot"` to `rust/Cargo.toml` `members`.
- [ ] Create `rust/shot/Cargo.toml` (above), a stub `rust/shot/src/lib.rs` (with `Config`, a
      `parse_args` returning `unimplemented!()`, and the test module below), and a minimal
      `rust/shot/src/main.rs` that parses args and prints them (real behavior lands in T1).
- [ ] **RED:** add `parse_args` unit tests to `lib.rs`; run `cargo test -p shot parse` → FAIL:
      ```rust
      #[cfg(test)]
      mod parse_tests {
          use super::*;
          fn v(a: &[&str]) -> Vec<String> { a.iter().map(|s| s.to_string()).collect() }

          #[test]
          fn minimal_single_tick() {
              let c = parse_args(&v(&["--scenario", "blood", "--tick", "35", "--out", "/tmp/x.png"]))
                  .unwrap();
              assert_eq!(c.scenario, "blood");
              assert_eq!(c.ticks, vec![35]);
              assert_eq!(c.out.as_deref(), Some(std::path::Path::new("/tmp/x.png")));
              assert_eq!(c.scale, 3); // default
              assert!(!c.hashes);
          }
          #[test]
          fn repeated_tick_and_scale_and_hashes() {
              let c = parse_args(&v(&[
                  "--scenario", "laser", "--tick", "5", "--tick", "9",
                  "--scale", "4", "--hashes",
              ])).unwrap();
              assert_eq!(c.ticks, vec![5, 9]);
              assert_eq!(c.scale, 4);
              assert!(c.hashes);
              assert!(c.out.is_none()); // --hashes-only, no --out is allowed
          }
          #[test]
          fn missing_scenario_is_error() {
              assert!(parse_args(&v(&["--tick", "1", "--out", "/tmp/x.png"])).is_err());
          }
          #[test]
          fn missing_tick_is_error() {
              assert!(parse_args(&v(&["--scenario", "blood", "--out", "/tmp/x.png"])).is_err());
          }
          #[test]
          fn no_out_and_no_hashes_is_error() {
              // must produce SOME output (a PNG or hashes)
              assert!(parse_args(&v(&["--scenario", "blood", "--tick", "1"])).is_err());
          }
      }
      ```
- [ ] **GREEN:** implement `parse_args` (hand-rolled loop over `args`, `--flag value` pairs, repeated
      `--tick` accumulates, `--scale` parses `u32` default 3, `--hashes` boolean, `--tc-root` optional;
      validate: `scenario` + at least one `tick` required; require `out` OR `hashes`). `cargo test -p
      shot parse` → GREEN.
- [ ] **Regenerate `Cargo.lock`:** `cargo build --manifest-path rust/Cargo.toml -p shot` → exits 0
      (pulls the `image` png subtree; small lock diff, expected).
- [ ] `cargo tree -p shot` shows **no `bevy*`** and no jpeg/gif/rayon crates (only png-related:
      `image`, `png`, `miniz_oxide`, `flate2`, `fdeflate`, etc.).
- [ ] `cargo fmt -p shot` (new files only).
- [ ] Reviewer (Opus): deps are exactly the 5 path crates + `image` (default-features=false, png);
      `parse_args` enforces required flags + the "out or hashes" rule; repeated `--tick` accumulates;
      no `bevy*` in `cargo tree`; lock diff is only the png subtree.
- [ ] **Commit:**
      - `git -C <worktree> add rust/shot rust/Cargo.toml rust/Cargo.lock`
      - `git -C <worktree> commit -m "shot(3d): new Bevy-free CLI crate skeleton — arg parser + image[png] dep"`

---

### T1 — headless render + PNG encode at a fixed tick (`render_scenario` + `encode_png`)  [Opus]

**Files**
- Modify: `rust/shot/src/lib.rs` (add `render_scenario`, `encode_png`, the per-tick helper, `run`),
  `rust/shot/src/main.rs` (call `run`)

**Interfaces**
- Consumes: `scenario::{Scenario, load, Loaded, SceneData}`; `render::bitmap::Bitmap`,
  `render::frame::draw`, `render::hash::{hash_frame, FNV_OFFSET, FNV_PRIME}`;
  `sim::state::{SimState, ControlState}`, `sim::hash::hash_game_state`; `sim_core::fixed::itof`;
  `image` (RgbImage + png encoder).
- Produces:
  ```rust
  pub struct Frame { pub tick: u32, pub frame_hash: u64, pub state_hash: u32, pub png: Option<Vec<u8>> }

  /// Drive `scenario` from tick 0 to `up_to`, rendering EVERY tick (viewport-RNG
  /// correctness). PNG bytes attached only for ticks in `wanted`. `scale` is the
  /// nearest integer upscale.
  pub fn render_scenario(tc_root: &Path, scenario: &Scenario, up_to: u32,
                         wanted: &[u32], scale: u32) -> Vec<Frame>;

  /// Encode a `Bitmap` as a nearest ×scale RGB PNG (raw colors, NO fade).
  pub fn encode_png(bmp: &Bitmap, scale: u32) -> Vec<u8>;

  /// Top-level: resolve paths from `Config`, load + drive + render, write PNG(s)
  /// to `out`, print `--hashes` sidecar to stdout. Returns an exit code.
  pub fn run(cfg: &Config) -> Result<(), String>;
  ```

**Why (teaching note):** This is the whole headless path and it is **golden-faithful by construction** —
it copies the T8 `render_tick` semantics (recorded inputs, tick-0-before-first-`process_frame`,
per-tick flash/shake inject-restore, `fade = tick==0 ? 0 : 33`) rather than the *lossy*
`render_snapshot.rs` shortcut (which hard-codes `screen_flash=0` and injects no shake, so it does not
reproduce the `shake` golden). Rendering **every** tick `0..=up_to` is load-bearing: `DrawLaserSight` and
shake advance the viewport-local `Rand` once per draw, so a screenshot at tick N is only correct if every
prior tick was drawn (overview *Risks*). The PNG encodes **raw RGB** (not the faded hash bytes) so
`--tick 0` shows the real frame, not black — the hash and the PNG come from the same buffer but the hash
applies fade and the PNG does not.

**Steps**
- [ ] **Per-tick render helper** (CLI-local copy of `render_slice3b_common::render_tick`,
      `mod.rs:146-176`): a private fn taking `&mut Bitmap`, `&SimState`, `&mut [Viewport;2]`,
      `&SceneData`, `&Scenario`, `tick` → returns the tick's `frame_hash`. It sets `screen_flash =
      scenario.flash_at(tick).unwrap_or(0)`, injects `scenario.shake_at(tick)` into
      `viewports[vp].shake = itof(amount)` **before** the draw and resets to `0` **after**, uses
      `draw_shadow = scenario.shadow()`, calls `render::frame::draw`, then
      `render::hash::hash_frame(&bmp, if tick==0 {0} else {33})`.
- [ ] **`render_scenario`:** `let mut loaded = scenario::load(tc_root, scenario);` then:
      - render tick 0 via the helper (BEFORE any `process_frame`); record `state_hash =
        hash_game_state(&loaded.state)`; seed `acc = FNV_OFFSET`, fold; if `0` is in `wanted`, encode PNG.
      - `for k in 1..=up_to`: `let inputs = [ControlState::unpack(scenario.input(k-1,0)),
        ControlState::unpack(scenario.input(k-1,1))]; loaded.state.process_frame(&inputs);` render via
        helper; record hashes; fold `acc`; encode PNG iff `k` in `wanted`.
      - return the `Vec<Frame>` (0..=up_to). (Callers needing the `total` recompute `acc` the same way,
        or expose it — the T2 golden test recomputes it identically.)
      Mirror `render_slice3b_common::run` `:257-296` exactly for the drive/hash order.
- [ ] **`encode_png`:** build an `image::RgbImage` of `w*scale × h*scale`; per output pixel
      `(x,y)` sample source `(x/scale, y/scale)` at `bmp.pixels[sy*bmp.pitch as usize + sx]`, write
      `[R,G,B] = [(px>>16)&0xff, (px>>8)&0xff, px&0xff]`; encode to PNG bytes via
      `image::codecs::png::PngEncoder` (or `RgbImage`→`write_to(PNG)`). No fade.
- [ ] **RED (PNG shape + channel order):** unit test in `lib.rs` — build a 2×1 `Bitmap`, set
      `pixels = [0xFF_2A_00_00, 0xFF_00_2A_00]` (red, green), `encode_png(&bmp, 2)` → decode back (or
      inspect the pre-encode `RgbImage`) and assert dimensions `4×2` and the top-left pixel is
      `[0x2A,0,0]`, the pixel at `x=2` is `[0,0x2A,0]` (nearest ×2 duplicates columns). Also a
      `pitch != w` case (set `bmp.pitch = 4`, resize `pixels`) to guard the stride. Run → FAIL, then
      implement → GREEN.
- [ ] **RED (determinism):** test that `render_scenario(root, &blood, 5, &[5], 3)` run twice yields
      equal `frame_hash` values (and equal PNG bytes for tick 5). Run → (should pass once implemented;
      write it before wiring `run`).
- [ ] **`run`:** resolve `tc_root` (cfg or the compile-time default) and the scenario text path
      (`GOLDEN_DIR/render_slice3b_<name>_scenario.txt`); if the scenario file is missing, `Err` listing
      available names. Parse via `Scenario::parse`. `up_to = *cfg.ticks.iter().max()`. Call
      `render_scenario`. Write each requested tick's PNG to `cfg.out` (single tick → the file; multiple
      → `out` is a dir, file `<name>_tick<N>.png`). If `cfg.hashes`, print the sidecar lines for every
      tick `0..=up_to` + a `total <n> <acc_hex16>` line to **stdout**; info to **stderr**.
- [ ] **Wire `main.rs`:** call `shot::run(&cfg)`; on `Err(msg)` print to stderr + exit non-zero.
- [ ] **Smoke:** `cargo run -p shot -- --scenario blood --tick 5 --out <tmp>/blood_t5.png` writes a
      non-empty PNG, exits 0. (Record the path + byte size in the done-report.)
- [ ] `cargo test -p shot` → all green (parse + png-shape + determinism).
- [ ] `cargo fmt -p shot` (new code only).
- [ ] Reviewer (Opus): recorded inputs (not default); tick-0-before-first-`process_frame`; per-tick
      flash/shake inject-restore + `draw_shadow`; `fade = tick==0?0:33`; **every** tick rendered
      (`0..=up_to`), PNG only for `wanted`; PNG is raw RGB (no fade), nearest ×scale, honors `pitch`;
      stdout/stderr split correct.
- [ ] **Commit:**
      - `git -C <worktree> add rust/shot/src/lib.rs rust/shot/src/main.rs`
      - `git -C <worktree> commit -m "shot(3d): headless render_scenario + nearest-scaled RGB PNG encode at a fixed tick"`

---

### T2 — `--hashes` stdout + golden-faithfulness integration test (blood + shake)  [Opus]

**Files**
- Create: `rust/shot/tests/golden.rs`
- Modify: `rust/shot/src/lib.rs` (only if the `total` accumulator needs exposing for the test)

**Interfaces**
- Consumes: `shot::render_scenario`; the committed goldens
  `rust/oracle-tests/golden/render_slice3b_<name>.txt` (frame sidecar) and `…_sim.txt` (state master);
  the scenario texts `…_scenario.txt`.
- Produces: proof that the CLI's render path reproduces the 3b goldens **bit-for-bit** — so the PNG it
  emits is provably the pixel-exact frame, and `--hashes` is a trustworthy regression hook.

**Why (teaching note):** The golden test is the whole point of the slice's correctness story: it proves
the CLI-local `render_tick` copy did not drift from the T8 harness (the decision to copy rather than
factor into `scenario` is only safe *because* this test guards it — spec open question 1, option B).
`blood` covers the common path; `shake` is the targeted guard for the *only* path the CLI copy adds over
`render_snapshot` (per-tick flash + shake injection) — without it, a broken shake/flash copy would ship
green. This mirrors the 3b differential discipline: the C++ sidecar is the oracle; the Rust render must
match it line-for-line including `total`.

**Steps**
- [ ] **Confirm `--hashes` grammar** in `run` (T1) emits exactly `parse_frames`-compatible lines
      (`<tick> <frame_hash_hex16> <state_hash_hex8>` + `total <n> <acc_hex16>`), nothing else on stdout.
      If T1 left `--hashes` partial, complete it here.
- [ ] **RED → GREEN golden test** (`rust/shot/tests/golden.rs`): for each name in `["blood", "shake"]`:
      - read + parse `render_slice3b_<name>_scenario.txt`; `assert_eq!(scenario.seed, 42)`.
      - read `render_slice3b_<name>.txt`; parse it into `(tick, frame_hash, state_hash)` rows + the
        `total` line (copy the tiny `parse_frames` grammar, `render_slice3b_common/mod.rs:52-78`, or a
        local equivalent — the test crate cannot see the `oracle-tests` test-module helper).
      - `let frames = shot::render_scenario(tc_root, &scenario, scenario.ticks, &[], 3);` (no PNG
        needed — pass empty `wanted`).
      - assert each `frames[k].frame_hash == golden[k].frame_hash` **and**
        `frames[k].state_hash == golden[k].state_hash`, for all k in `0..=ticks`.
      - recompute the accumulator (`FNV_OFFSET`, fold each `frame_hash`) and assert it == the golden
        `total`.
      Paths via `concat!(env!("CARGO_MANIFEST_DIR"), "/../oracle-tests/golden/…")` and
      `"/../../data/TC/openliero"`. Run → GREEN (if RED, the CLI render drifted — reconcile against
      `render_slice3b_common::run`, do NOT weaken the assert).
- [ ] **Manual `--hashes` cross-check (smoke):** `cargo run -p shot -- --scenario blood --tick 40
      --hashes` and eyeball that the printed lines match the head/tail of
      `rust/oracle-tests/golden/render_slice3b_blood.txt` (spec's run-skill step 3 in miniature).
      Record the first + `total` line in the done-report.
- [ ] `cargo test -p shot` → all green (parse + png + determinism + golden).
- [ ] `cargo fmt -p shot` (new file only).
- [ ] Reviewer (Opus): the golden test covers `blood` **and** `shake`; asserts frame_hash + state_hash
      per tick + the `total` accumulator; reads the committed goldens unmodified; `--hashes` stdout is
      pure sidecar grammar (a clean `diff` target).
- [ ] **Commit:**
      - `git -C <worktree> add rust/shot/tests/golden.rs rust/shot/src/lib.rs`
      - `git -C <worktree> commit -m "shot(3d): golden-faithfulness test (blood+shake) + --hashes sidecar stdout"`

---

### T3 — in-repo run-skill (`.claude/skills/`)  [Opus]

**Files**
- Create: `.claude/skills/<skill-name>/SKILL.md` (name e.g. `liero-shot` or `render-observe`; pick one,
  record it in the done-report). Placement is OUTSIDE `docs/` — this is the one 3d file that lives at a
  skill path.

**Interfaces**
- Produces: a project run-skill the `run`/`verify` harness discovers ("first looks for a project skill
  that covers launching the app", iteration §5 item 1) so an agent in any Liero-rs worktree can build,
  screenshot at a fixed seed/scenario/tick, find the PNG on a known path, and compare against the
  golden/state-hash.

**Why (teaching note):** iteration §5/§6 and the overview open question Q5 land here: commit the skill
**in-repo** (`.claude/skills/`, travels to worktrees) rather than `~/.claude`, so dispatched agents
inherit it automatically. The skill turns the ad-hoc "how do I see a frame?" into the reliable loop:
change → `shot` screenshot → view PNG → judge → (hard gate) `--hashes` diff vs golden. It documents the
*stable* flag contract so the loop does not break when internals change.

**Steps**
- [ ] Create `.claude/skills/<skill-name>/SKILL.md` with a frontmatter `name` + `description` (so the
      harness can match it — the description should say it launches/observes the Liero-rs renderer
      headlessly for screenshots + regression) and a body documenting exactly:
      1. **Build:** `cargo build --manifest-path rust/Cargo.toml -p shot` — fast, Bevy-free, no GPU /
         display. (Contrast: `game` is the minutes-long Bevy window build; `shot` is the headless path.)
      2. **Screenshot at a fixed seed/scenario/tick:**
         `cargo run -p shot -- --scenario blood --tick 35 --out <known-path>.png` → a deterministic PNG
         on a known path the agent then VIEWS to judge a change (the change → screenshot → judge loop).
      3. **Regression compare (the hard gate):**
         `cargo run -p shot -- --scenario blood --tick 40 --hashes` → diff the printed sidecar lines
         against `rust/oracle-tests/golden/render_slice3b_blood.txt`; a `frame_hash`/`total` mismatch is
         a render regression at the named tick, a `state_hash` mismatch is a *sim* regression.
         (iteration §3: checksum authoritative, screenshot advisory.)
      4. **Available scenarios** (the 7 committed 3b goldens) + which subsystem each exercises: `laser`
         (laser sight + viewport RNG), `shadow` (shadow pass), `blood` (blood + terrain carve),
         `dart_water` (water `BlitImageR`), `shake` (screen shake + flash), `dart`/`fan` (objects). Plus
         the full flag contract from the spec.
      5. **Fixed-seed note:** seed-42, input-scripted, GPU-free ⇒ the same frame every run on any
         machine; render is all-ticks-up-to-target (so `--tick 500` on a 500-tick scenario is not
         instant — explain the viewport-RNG reason briefly).
- [ ] **Verify the skill's commands are correct** by running each once (build; a screenshot to a tmp
      path; a `--hashes` invocation) and confirming they behave as the skill claims. Fix any command
      that drifts from the real CLI. (Do NOT paste a command the skill author did not run.)
- [ ] Reviewer (Opus): the skill has matchable frontmatter; every documented command is real and was
      run; the build path is the Bevy-free `shot` (not `game`); the compare step names the right golden
      file; scenarios list is accurate.
- [ ] **Commit:**
      - `git -C <worktree> add .claude/skills`
      - `git -C <worktree> commit -m "shot(3d): in-repo run-skill — build/screenshot/compare loop for the headless renderer"`

---

### T4 — CI confirm + docs (PROGRESS, overview 3d line, open-questions, deferral ledger) + broad slice review  [Opus]

**Files**
- Modify: `docs/superpowers/liero-rs-PROGRESS.md`; the rendering overview
  (`docs/superpowers/specs/2026-07-10-liero-rs-step3-rendering-overview.md`) 3d bullet + open-questions
- Confirm (edit only if needed): `.github/workflows/rust.yml`

**Interfaces**
- Produces: CI confirmed to compile + test `shot` in the fast gate with no structural change; docs
  updated to reflect 3d landed; a broad final review of the whole slice.

**Why (teaching note):** `cargo test --workspace` compiles + tests every member, so merely adding `shot`
to `members` (T0) already makes the existing `--exclude game` job build and run its golden test — the
determinism gate stays fast and Bevy-free because `shot` is Bevy-free (the exclusion targets Bevy, not
non-sim crates). So CI needs **no** new step; T4 only *confirms* that and checks the `image` dep did not
bloat the job. Then the docs close the slice honestly (what shipped, what deferred, which open questions
resolved).

**Steps**
- [ ] **Confirm CI covers `shot`:** verify `.github/workflows/rust.yml`'s test step
      (`cargo test --manifest-path rust/Cargo.toml --workspace --exclude game`) compiles + runs the
      `shot` golden test (it does, via `--workspace`). Confirm the `image` (png-only) dep adds no heavy
      apt requirement (png encoding is pure-Rust `miniz_oxide`/`flate2` — no system libs). **Only if** a
      real gap is found (e.g. the golden test needs a data path CI lacks) add the minimal fix here; the
      expectation is **no workflow edit**. Record the decision in the done-report.
- [ ] Update `docs/superpowers/liero-rs-PROGRESS.md` (use the real `currentDate` in Status/date fields —
      do NOT freeze a series prefix): 3d landed — a Bevy-free `shot` CLI PNG-encodes any 3b scenario's
      world view at a fixed tick to a known path and dumps machine-readable frame/state hashes; a golden
      test proves it reproduces `render_slice3b_blood`/`shake` bit-for-bit; an in-repo `.claude/skills/`
      run-skill drives the change → screenshot → judge → compare loop. Keep the whole-Step-3 map current
      (3a/3b/3c/3d done; 3e HUD/font/minimap + 3f wasm next).
- [ ] Update the overview's **3d bullet** to mark it landed (companion spec + plan implemented), and
      resolve the relevant open questions: **Q5 (run-skill location) → in-repo, ratified**; the
      **machine-readable hash-dump question → YES (`--hashes` shipped)**. Note the per-tick-driver
      decision (option B: CLI-local copy, T8 untouched, guarded by the golden test) and its deferred
      cleanup (factor into `scenario` when a third consumer appears).
- [ ] **Deferral ledger (explicit).** Record what 3d consciously carried forward, each routed to the
      slice that needs it: **HUD/font/bars/minimap** (3e); **wasm / headless-browser canvas screenshot**
      (3f, overview Q6); **`.lrp` replay + input timelines** (Step 4 — `shot` becomes the replay-
      regression driver then); **factoring the per-tick render driver into `scenario`** (deferred until
      a third consumer); **video/GIF output** (out of scope); **CLI-driven `--hashes` diff as a standing
      CI job** (redundant — the golden *test* already gates the render path).
- [ ] **Broad slice review (Opus) — the final gate.** One reviewer pass over the WHOLE slice diff:
      `shot` is Bevy-free (`cargo tree -p shot` — no `bevy*`); the golden test is GREEN and covers
      `blood`+`shake` incl. `total`; the CLI renders all ticks up to target (viewport-RNG rule) with
      recorded inputs, tick-0-before-first-`process_frame`, per-tick flash/shake, `fade` convention; the
      PNG is raw RGB (no fade), nearest ×scale, honors `pitch`; `--hashes` stdout is clean sidecar
      grammar; the run-skill's commands are real + Bevy-free; CI stays fast/Bevy-free; no
      `render`/`sim`/`assets`/`sim-core`/`scenario` source touched; no `image` feature creep; docs match
      reality; commits on `liero-rs-step-3` with no AI taglines / trailers; no push / no PR.
- [ ] **Commit:**
      - `git -C <worktree> add docs/superpowers/liero-rs-PROGRESS.md docs/superpowers/specs/2026-07-10-liero-rs-step3-rendering-overview.md .github/workflows/rust.yml`
      - `git -C <worktree> commit -m "docs+ci(3d): PROGRESS + overview 3d landed; open-questions resolved; deferral ledger; CI confirmed"`

## Done-report (each task)

Each task's worker returns: (a) what changed and why (1–3 sentences), (b) files touched, (c) tests/
smokes run + result (paste the key `cargo test`/`cargo build`/`cargo run` line + exit code; for `shot`
runs, the written PNG path + byte size, and for `--hashes`, the first + `total` line), (d) any decision
taken where the plan left a choice (skill name, `--out` file-vs-dir handling for multiple ticks, any CI
gap found). Implementers: **never** run `cd` or git-for-navigation (`git -C <worktree>` only for the
listed commits; read files with the Read tool); **no** `Co-Authored-By`/`Claude-Session`/"Generated
with" trailers; **do not** spawn sub-subagents; **do not** push; **do not** open a PR.
