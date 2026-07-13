# Step 4, Slice 4a — Live input core: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax. **Test-first**: write the failing test (or its assertion) before the
> implementation it pins, run it and SEE it fail, then make it pass.

**Goal:** Make a native `cargo run -p game -- --live` match **playable from the keyboard** (1-player +
2-player hotseat), the sim ticking at `1000/14 Hz` with **exactly one input snapshot per tick**, Dig =
Left+Right chord, Esc quits — while the scripted path stays unchanged and its **pass-through
determinism gate** proves the new sampler reproduces every scenario golden bit-exact. Companion spec:
`specs/2026-07-12-liero-rs-step4-slice4a-live-input-design.md` (cited as **spec §N**).

**Architecture:** Additive inside the `game` binary (the only Bevy crate — 3c). New module
`game/src/input.rs`: a **generic, Bevy-free-testable** per-worm sampler (`PlayerBindings<K>` +
`control_state`, Dig OR'd into Left/Right — spec §3/§4.1), an `InputSource` enum (`Scripted` = the
existing recorded path behind the source; `Live` = `ButtonInput<KeyCode>` poll — spec §4.2), and the
default `InputMap` mirroring the C++ defaults (spec §2). The single `FixedUpdate` tick system samples
the source **once, before `process_frame`** (spec §4.3). `sim`/`render`/`scenario` are **unchanged and
Bevy-free**; no new sim field; the isolation firewall holds (3c). The 4b recorder seam is marked, not
built (spec §6).

**Tech stack:** Rust (`game` only). Bindings map DOS scancodes (`settings.cpp:36-37`) → SDL
(`keys.cpp:9-58`) → Bevy `KeyCode` (spec §2 table). CI: the fast determinism job stays
`cargo test --workspace --exclude game`; 4a upgrades 3c's `cargo build -p game` step to
`cargo test -p game` so the sampler unit tests + the headless pass-through replay run (spec §7).

## Global constraints

*(inherit every Step 2/3 constraint; the 4a-specific ones follow)*

- **Bevy stays confined to `game`.** `sim`, `render`, `scenario`, `assets`, `sim-core` remain
  Bevy-free and **unchanged** in behavior. `cargo tree -p sim`/`-p render`/`-p scenario` show no
  `bevy*`. 4a adds no sim field and no `f32`/`Vec2`/`Transform` into the sim (the 3c isolation
  firewall).
- **Exactly one input snapshot per tick, in the FixedUpdate tick system, from level state** (spec
  §4.3/§4.4). Never an `Update`→resource accumulator; never `just_pressed`/keyboard events for the
  snapshot (only `keys.pressed`). This is the central determinism invariant.
- **Dig is a pure Left+Right chord, never a stored bit** (spec §3); default-unbound (spec §2).
- **Determinism self-check retained for scripted, retired for live** (spec §4.3 step 6 / §9).
- **Non-live paths byte-identical.** Every committed `sim_slice*` / `render_slice3b_*` golden stays
  byte-identical — 4a is a source indirection, not a sim change. `git diff --stat` on the goldens
  MUST be empty (a standing re-diff assertion; no regen needed).
- **No AI / "Generated with" taglines; no commit trailers** (no `Co-Authored-By`, no
  `Claude-Session`). **Commit on branch `liero-rs-step-4`** (the accumulating Step-4 PR). **Do NOT
  push and do NOT open a PR** — the controller owns push + PR. **fmt only new/edited files.** **No
  sub-subagents.** **Bash discipline:** one command per call; no `>>`/heredoc/`&&`/`;`/`$VAR`; no
  `cd`+`git`; create files with the editor.

## File structure

- `rust/game/src/input.rs` — **NEW.** `PlayerBindings<K>` + `control_state` (generic, Bevy-free
  core); `InputMap { players: Vec<PlayerBindings<KeyCode>> }` + `default_bindings()` (spec §2);
  `InputSource` enum (`Scripted(Scenario)` / `Live(InputMap)`) + `sample(tick, &keys) -> [ControlState; N]`;
  `Mode`/CLI helpers; unit tests.
- `rust/game/src/main.rs` — replace the inline `scenario.input` feed (`:257-261`) with
  `source.sample(...)`; `--live` CLI branch; gate loop/reload + self-check on scripted mode; add the
  `mod input;`. `close_on_esc` unchanged.
- `rust/game/tests/passthrough.rs` — **NEW.** Headless pass-through determinism gate (spec §5).
- `.github/workflows/rust.yml` — upgrade the `cargo build -p game` step to `cargo test -p game`
  (spec §7).
- `docs/superpowers/liero-rs-PROGRESS.md` + the overview's 4a line — updated in T5.

## Tasks

### T0 — pure sampler + default bindings + Dig chord (Bevy-free core)  [Opus]

**Files**
- Create: `rust/game/src/input.rs`
- Modify: `rust/game/src/main.rs` (`mod input;`)

**Interfaces**
- Produces: `PlayerBindings<K> { up,down,left,right,fire,change,jump: K, dig: Option<K> }` +
  `PlayerBindings::<K>::control_state(&self, pressed: impl Fn(K) -> bool) -> ControlState`
  (spec §4.1); `input::default_bindings() -> InputMap` (KeyCode, spec §2 table);
  `InputMap { players: Vec<PlayerBindings<KeyCode>> }`.
- Consumes: `sim::state::ControlState` (`state.rs:60-129`).

**Why (teaching note):** this is the load-bearing correctness core, made **generic over the key type**
so the bit-mapping + Dig chord are unit-testable with a mock key + a `HashSet` "pressed" closure — no
window, no `ButtonInput` — and so run in CI (spec §4.1). Dig is a *pure function*
`Left = pressed(left) || pressed(dig)`, `Right = pressed(right) || pressed(dig)` (spec §3, the steady
state of `localController.cpp:69-79`); it is never a stored bit. The default table mirrors
`settings.cpp:36-37` decoded through `keys.cpp:9-58` (spec §2) with **Dig unbound**.

**Steps**

- [ ] Add `mod input;` to `main.rs`.
- [ ] **RED (sampler):** create `input.rs` with `PlayerBindings<K>` + `control_state`
      (`unimplemented!()`) and unit tests over a mock key type (e.g. `&'static str` or a small enum)
      driven by a `|k| set.contains(k)` closure: (a) each of the 7 bits sets iff its key is pressed;
      (b) **Dig held** ⇒ Left AND Right set even with neither movement key; (c) Dig released ⇒ Left/Right
      follow their own keys again; (d) `dig: None` ⇒ chord never fires; (e) no 8th bit is ever set
      (`pack() < 0x80`). Run `cargo test -p game input` → FAIL.
- [ ] **GREEN (sampler):** implement `control_state` per spec §4.1; cite `localController.cpp:69-79`
      and `worm.hpp:45-55`.
- [ ] **RED (defaults):** add a test asserting `default_bindings()` matches the spec §2 table — P0 =
      `{KeyR,KeyF,KeyD,KeyG,ControlLeft,ShiftLeft,AltLeft, dig:None}`, P1 =
      `{ArrowUp,ArrowDown,ArrowLeft,ArrowRight,ControlRight,AltRight,ShiftRight, dig:None}`; both
      `dig` are `None`. FAIL.
- [ ] **GREEN (defaults):** implement `default_bindings()`; comment each key with its DOS scancode →
      SDL → KeyCode derivation (spec §2). Confirm the Bevy 0.19 `KeyCode` variant names compile
      (`cargo build -p game`); the DOS→SDL column is the authority if a name differs.
- [ ] Run `cargo test -p game input` — PASS. `cargo build -p game`.
- [ ] Reviewer (Opus): Dig is a pure L/R OR (never stored); default table matches the decoded C++
      defaults incl. Dig unbound; the generic core needs no Bevy in its tests; `pack() < 0x80` holds.
- [ ] **Commit:**
      - `git add rust/game/src/input.rs rust/game/src/main.rs`
      - `git commit -m "game(4a): pure per-worm sampler + default bindings + Dig chord"`

---

### T1 — `InputSource` abstraction + wire the FixedUpdate feed  [Opus]

**Files**
- Modify: `rust/game/src/input.rs` (add `InputSource`), `rust/game/src/main.rs`

**Interfaces**
- Produces: `InputSource { Scripted(Scenario), Live(InputMap) }` +
  `sample(&self, tick: u32, keys: &ButtonInput<KeyCode>) -> [ControlState; N]` (spec §4.2). Inserted
  as a `Resource`.
- Consumes: `scenario::Scenario::input` (`parser.rs:261-268`); `bevy::input::ButtonInput<KeyCode>`.

**Why (teaching note):** `InputSource` is the extension seam (spec §4.2/§6): `Scripted` makes the
existing recorded path a literal pass-through source; `Live` polls `ButtonInput`. 4b later adds a
`Replay` arm here. The FixedUpdate system samples the source **once, before `process_frame`** (spec
§4.3) — the same single system, only the input line changes — preserving one-snapshot-per-tick with no
render-rate coupling. `ButtonInput::pressed` is Bevy's level-triggered held set, so polling it at tick
time *is* the C++ "consume whatever bits are set at `ProcessFrame`" (spec §4.4).

**Steps**

- [ ] **RED (pass-through):** unit test that `InputSource::Scripted(scn).sample(t, &empty_keys)` equals
      `[ControlState::unpack(scn.input(t,0)), ControlState::unpack(scn.input(t,1))]` for several ticks
      of a parsed scenario (an empty `ButtonInput` is fine — Scripted ignores keys). FAIL first.
- [ ] **GREEN:** implement `InputSource::sample` (Scripted = pass-through; Live = per-worm
      `control_state(|k| keys.pressed(k))`). Cite `parser.rs:261-268`.
- [ ] **Wire main.rs:** replace the inline `[unpack(scenario.input(t,0)), ...]` (`main.rs:257-261`)
      with `source.sample(demo.tick, &keys)`; insert `InputSource::Scripted(scenario.clone())` as a
      resource in `setup` (the scripted default path is unchanged). Add `keys: Res<ButtonInput<KeyCode>>`
      to the tick system. Leave a `// 4b recorder seam` comment at the sampled `inputs` (spec §6).
- [ ] Run the existing in-binary debug self-check path mentally/structurally intact: the scripted
      default still feeds recorded inputs (no behavior change). `cargo build -p game`;
      `cargo test -p game` — green.
- [ ] Reviewer (Opus): sampling is one call at the top of the single FixedUpdate system (not Update,
      not edges); Scripted is a faithful pass-through; the 4b seam is marked; `game` compiles with the
      new `ButtonInput` param; no sim/render change.
- [ ] **Commit:**
      - `git add rust/game/src/input.rs rust/game/src/main.rs`
      - `git commit -m "game(4a): InputSource (Scripted/Live) fed once per FixedUpdate tick"`

---

### T2 — CLI `--live` vs scenario mode; retire the guard for live  [Sonnet]

**Files**
- Modify: `rust/game/src/main.rs` (+ small helpers in `input.rs` if cleaner)

**Interfaces**
- Produces: `--live [name]` → `Mode::Live` (source `Live(default_bindings())`, loop/reload +
  self-check **off**, run indefinitely); `<name>` → `Mode::Scripted` (unchanged, guard + loop **on**);
  no args → scripted `blood` (spec §7). Native-only (`cfg(not(wasm))`); wasm keeps the scripted witness
  (spec §8).

**Why (teaching note):** live play has no golden, so the debug self-check (`main.rs:281-287`) and the
loop-at-`ticks` reload (`main.rs:263-272`) must be **scripted-only** — but they must stay for scripted
so the regression path is not silently disabled (spec §9). Live reuses `scenario::load` for the initial
state (level + worms) and swaps only the input source.

**Steps**

- [ ] **RED:** a unit test for the arg parser: `["--live"]` → `(Mode::Live, "blood")`;
      `["--live","dart"]` → `(Mode::Live,"dart")`; `["dart"]` → `(Mode::Scripted,"dart")`; `[]` →
      `(Mode::Scripted,"blood")`; unknown name still errors (reuse `available_scenarios`). FAIL first.
- [ ] **GREEN:** extend `resolve_scenario` (`main.rs:120-134`) to return `(Mode, String)`; consume a
      leading `--live` flag. Thread `Mode` into `setup`: pick `InputSource::Live(default_bindings())`
      vs `Scripted`. Gate the loop/reload and the debug self-check on `Mode::Scripted` (spec §4.3
      step 4/6). Live loads the same `scenario::load` state; do not load the golden column in live
      mode.
- [ ] `cargo build -p game`; `cargo test -p game` — green.
- [ ] Reviewer (Opus): guard + loop kept for scripted, off for live; live still loads a real level via
      `scenario::load`; wasm path untouched (still scripted witness); Esc/quit unchanged.
- [ ] **Commit:**
      - `git add rust/game/src/main.rs rust/game/src/input.rs`
      - `git commit -m "game(4a): --live vs scenario mode; guard/loop scripted-only"`

---

### T3 — pass-through determinism gate (headless) + CI wiring  [Opus]

**Files**
- Create: `rust/game/tests/passthrough.rs`
- Modify: `.github/workflows/rust.yml`

**Interfaces**
- Consumes: `scenario::{Scenario, load}`, `sim::{state::SimState, hash::hash_game_state}`,
  `game`'s `InputSource` (expose the needed items `pub(crate)`/`pub` as required).
- Produces: for each committed scenario, a headless per-tick assertion that
  `hash_game_state` driven through `InputSource::Scripted` equals the golden `state_hash` column
  (spec §5).

**Why (teaching note):** this is the objective 4a gate — determinism survives the new sampler (spec
§5, done-when 3). It runs **headless** because `Scripted` touches no Bevy/window; it is the direct
ancestor of 4b's record→replay round-trip. It must run in CI, so 3c's `cargo build -p game` step is
upgraded to `cargo test -p game` (keeping the fast `--workspace --exclude game` job Bevy-independent —
spec §7).

**Steps**

- [ ] **RED:** create `tests/passthrough.rs` — for each committed scenario name (enumerate the
      `render_slice3b_*_scenario.txt` corpus, or a representative subset incl. `blood`+`dart`), read +
      parse the scenario text and its golden sidecar `state_hash` column (mirror
      `load_golden_hashes`, `main.rs:356-375`), `scenario::load` the state, then for each tick:
      `sim.process_frame(&InputSource::Scripted(scn).sample(t, &empty))` and
      `assert_eq!(hash_game_state(&sim), golden_state[t])`. Also assert `total`/final tick. Run
      `cargo test -p game --test passthrough` → FAIL if any wiring is off, then PASS.
- [ ] **GREEN:** make `InputSource` + `sample` reachable from the integration test (`pub`); ensure
      `scenario::load` + hashes resolve the `GOLDEN_DIR` path the way `main.rs` does
      (`CARGO_MANIFEST_DIR`-relative — `main.rs:31-36`). Constructing an empty `ButtonInput<KeyCode>`
      for Scripted is fine (Scripted ignores it).
- [ ] **CI:** in `.github/workflows/rust.yml`, change the 3c `cargo build -p game` step to
      `cargo test -p game` (runs the T0/T1/T2 unit tests + this gate). Leave the fast determinism job
      (`cargo test --workspace --exclude game`) untouched. Confirm the trimmed Bevy feature set still
      builds test targets on the CI runner (as 3c's build step already does).
- [ ] Run `cargo test -p game` (all game tests) + `cargo test --workspace --exclude game` (priors
      untouched). Confirm `git diff --stat` on `golden/` is empty (no golden regen).
- [ ] Reviewer (Opus): the gate drives the REAL `InputSource::Scripted` (not a bypass), asserts the
      full per-tick column + `total`, runs headless, and is wired into CI via `cargo test -p game`;
      the fast job stays Bevy-free.
- [ ] **Commit:**
      - `git add rust/game/tests/passthrough.rs .github/workflows/rust.yml`
      - `git commit -m "game(4a): headless pass-through determinism gate + CI cargo test -p game"`

---

### T4 — MILESTONE: manual live playability + focus-loss  [Opus]

**Files**
- Modify: `rust/game/src/main.rs` (focus-loss handling only if Bevy doesn't auto-release)

**Interfaces**
- Consumes: `bevy::window::WindowFocused` (only if a manual release system is needed, spec §4.5).

**Why (teaching note):** live keyboard cannot be bit-gated (spec Oracle strategy) — the acceptance is
a human playing. Focus loss can leave a key "stuck" if winit drops the key-up; Bevy releases pressed
keys on `WindowFocused(false)` by default, but this must be **verified** on `bevy` 0.19 and backstopped
if absent (spec §4.5). This is the 4a playability proof (iter §1b/§4).

**Steps**

- [ ] Run `cargo run -p game -- --live` (native). Verify: P0 keys `R/F/D/G` + `LCtrl/LShift/LAlt` drive
      worm 0 (move/aim, fire, change, jump); P1 arrows + `RCtrl/RAlt/RShift` drive worm 1; **holding
      Left+Right digs** (both worms); Esc quits cleanly. Record the observation in the done-report.
- [ ] Verify one snapshot per tick behaves sanely (no double-speed under a slow frame; held keys keep
      moving). If `--features dynamic` speeds iteration, use it (dev-only, spec/3c).
- [ ] **Focus-loss check:** alt-tab away while holding a movement key; on return the worm must not be
      stuck. If Bevy auto-releases (expected), note it. If NOT, add a minimal `Update` system reacting
      to `WindowFocused(false)` that clears held state (spec §4.5) — RED via the manual repro, GREEN by
      the fix. Keep it live-only / not gated.
- [ ] `cargo test -p game`; `cargo test --workspace --exclude game` — still green.
- [ ] Reviewer (Opus): both keysets control the right worm; Dig chord works with the default
      (dig-unbound) keys via L+R; Esc quits; focus loss does not stick keys; the scripted self-check
      still runs for scripted mode.
- [ ] **MILESTONE.** A native `--live` match is playable, one snapshot/tick, hotseat 2P, Dig chord,
      Esc quit; the pass-through gate (T3) proves determinism survives the sampler.
- [ ] **Commit (only if focus-loss code was added):**
      - `git add rust/game/src/main.rs`
      - `git commit -m "game(4a): release held keys on focus loss (live)"`

---

### T5 — wasm note + PROGRESS + overview 4a line + broad slice review  [Sonnet]

**Files**
- Modify: `docs/superpowers/liero-rs-PROGRESS.md`; the overview's 4a bullet + open-questions
  (`docs/superpowers/specs/2026-07-12-liero-rs-step4-input-replay-overview.md`)

**Steps**

- [ ] Confirm the full green board: `cargo test -p game` (sampler units + pass-through gate),
      `cargo test --workspace --exclude game` (all priors), `cargo build -p game`. Confirm `sim`/
      `render`/`scenario` Bevy-free and unchanged (`git diff --stat` shows only `game` + docs + CI);
      the `golden/` re-diff is empty.
- [ ] Update `docs/superpowers/liero-rs-PROGRESS.md` (use the real `currentDate`): Step 4 slice 4a
      DONE — the game is playable from the keyboard (1P + 2P hotseat), one input snapshot per
      `FixedUpdate` tick, Dig = Left+Right chord, default bindings mirror C++; the scripted path and
      its determinism self-check are retained; the headless pass-through gate proves the new sampler
      reproduces every scenario golden bit-exact; the 4b recorder seam is marked.
- [ ] Update the overview's 4a bullet to landed (companion spec implemented) and record how the spec
      open questions resolved (sampler home + CI step; default run mode; live-mode camera). Note wasm
      stays on the scripted witness (live-wasm deferred, spec §8) and gamepad/menu/record remain later
      slices.
- [ ] **Broad slice review (Opus):** re-read the whole 4a diff against the spec — one-snapshot-per-tick
      in the FixedUpdate system (not Update/edges), Dig chord pure, default table exact, guard
      scripted-only, pass-through gate real + CI-wired, no sim/render change, 4b seam marked, wasm
      untouched.
- [ ] **Commit:**
      - `git add docs/superpowers/liero-rs-PROGRESS.md docs/superpowers/specs/2026-07-12-liero-rs-step4-input-replay-overview.md`
      - `git commit -m "docs(4a): PROGRESS + overview slice-4a landed; wasm/live deferrals"`

## Done-report (each task)

(a) what changed + why, (b) files touched, (c) tests/risks. Per-task commit, local, on branch
`liero-rs-step-4`. **Do not push, do not open a PR** — the controller owns push + PR. Surface in the
final report: the **pass-through gate evidence** (per-tick hashes == goldens over the corpus, headless,
in CI via `cargo test -p game` — T3), the **manual playability result** (both keysets, Dig chord, Esc,
focus-loss behavior — T4), the **default-binding table** as implemented (spec §2), and confirmation the
**sim/render/scenario crates are byte-unchanged** (goldens re-diff empty).
