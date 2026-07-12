# Step 4, Slice 4b — Record / replay round-trip + CI regression: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax. **Test-first**: write the failing test (or its assertion) before the
> implementation it pins, run it and SEE it fail, then make it pass.

**Goal:** Prove **determinism survives real input** — record a live/synthetic session's per-tick
`[ControlState; N]` stream to a Rust-native artifact, replay it headlessly, and assert the **identical**
`hash_game_state` time series (Step 4's hard gate; Step 5 precondition). The artifact **is a scenario
file** in the existing grammar (no new format); the only new format code is a scenario *serializer*.
Replay reuses `InputSource::Scripted` verbatim. Companion spec:
`specs/2026-07-12-liero-rs-step4-slice4b-record-replay-design.md` (cited as **spec §N**).

**Architecture:** Additive. **`scenario` crate:** add `Scenario::to_text()` + a `with_recorded_inputs`
builder (Bevy-free, round-trip-tested against the parser — spec §3). **`game` crate:** a `Recorder`
`Resource` taps the 4a seam (`main.rs:290-292`, between sample and `process_frame` — spec §4.1),
buffers per-tick snapshots, flushes a scenario file on exit; `--record`/`--replay` CLI (spec §7); a
headless `replay_state_series` library entry (spec §5). The round-trip gate
(`game/tests/round_trip.rs`) records a **synthetic Live stream** → serialize → parse → `Scripted`
replay → asserts both state series equal (non-vacuous — spec §6). `sim`/`render` are **unchanged and
Bevy-free**; no new sim field; the isolation firewall holds. `.lrp` is **not** touched (that is 4e —
spec §8).

**Tech stack:** Rust (`scenario` + `game`). Serializer emits the `parser.rs:9-26` grammar; per-tick
words are `ControlState::pack()` (`state.rs:81`), sparse (nonzero ticks only). CI: the serializer
round-trip rides the fast `cargo test --workspace --exclude game` job; the recorder/replay/round-trip
gate ride `cargo test -p game` (already in CI since 4a). No new CI job.

## Global constraints

*(inherit every Step 2/3/4a constraint; the 4b-specific ones follow)*

- **Bevy stays confined to `game`.** `sim`, `render`, `scenario`, `assets`, `sim-core` remain
  Bevy-free and **unchanged in behavior**. The new serializer lives in `scenario` and is Bevy-free
  (`cargo tree -p scenario` shows no `bevy*`). 4b adds no sim field and no `f32`/`Vec2`/`Transform`
  into the sim (the 3c isolation firewall).
- **The artifact is a scenario file — no parallel format, no binary sibling, no delta encoding**
  (spec §3). The recorder records the **sampled array** (Dig chord already resolved — spec §4.1), so
  replay needs no bindings/chord logic. Per-tick words are absolute 7-bit (`pack()`), **not** deltas:
  the Rust sim has no `prev_control_states` baseline (spec §9 finding) — do **not** build a delta
  encoder.
- **The round-trip gate must be non-vacuous** (spec §6): record through the **real** `InputSource::Live`
  sampler over a **synthetic stream authored in the test** (distinct from every committed scenario),
  route through **text** serialize→parse, replay through `InputSource::Scripted`, assert both
  `hash_game_state` series equal tick-for-tick + spot-assert decoded words. Never record through
  `Scripted` (that is the vacuous `Scripted == Scripted`).
- **Recording is Live-mode only.** No recorder in Scripted mode → the scripted path and its 4a
  pass-through gate stay byte-unchanged.
- **Non-live paths byte-identical.** Every committed `sim_slice*` / `render_slice3b_*` /
  `render_slice3e_*` golden stays byte-identical — 4b is additive. `git diff --stat` on `golden/` MUST
  show only the **new** `record_slice4b_*` files (T4), never a change to an existing golden.
- **No AI / "Generated with" taglines; no commit trailers** (no `Co-Authored-By`, no
  `Claude-Session`). **Commit on branch `liero-rs-step-4`** (the accumulating Step-4 PR). **Do NOT
  push and do NOT open a PR** — the controller owns push + PR. **fmt only new/edited files** (watch the
  `blit.rs` rustfmt footgun — do not blanket-fmt). **No sub-subagents.** **Bash discipline:** one
  command per call; no `>>`/heredoc/`&&`/`;`/`$VAR`; no `cd`+`git`; create files with the editor.

## File structure

- `rust/scenario/src/parser.rs` (or a new `rust/scenario/src/writer.rs`) — **NEW code.**
  `Scenario::to_text(&self) -> String`; `Scenario::with_recorded_inputs(&self, per_tick: &[(u32,u32)])
  -> Scenario` (clone + replace inputs + set `ticks`). Round-trip unit tests.
- `rust/game/src/input.rs` — extend `parse_args` for `--record <path>` / `--replay <path>`; add a
  `Recorder` type (buffer + `record` + `build`); a `replay_state_series` headless helper. Unit tests.
- `rust/game/src/main.rs` — insert the `Recorder` (Live mode only) at the 4a seam; flush-on-exit
  system; `--replay` windowed branch (guard/loop off).
- `rust/game/tests/round_trip.rs` — **NEW.** The non-vacuous round-trip gate (spec §6).
- `rust/game/tests/record_regression.rs` — **NEW.** The committed-corpus drift backstop (spec §6b).
- `rust/oracle-tests/golden/record_slice4b_<name>_scenario.txt` + `record_slice4b_<name>.txt` —
  **NEW** committed artifact + its sim-produced `state_hash` sidecar (T4).
- `docs/superpowers/liero-rs-PROGRESS.md` + the overview's 4b line — updated in T5.

## Tasks

### T0 — scenario serializer + round-trip property test (Bevy-free)  [Opus]

**Files**
- Modify/Create: `rust/scenario/src/parser.rs` (or `rust/scenario/src/writer.rs` + `lib.rs` re-export)

**Interfaces**
- Produces: `Scenario::to_text(&self) -> String`; `Scenario::with_recorded_inputs(&self, per_tick:
  &[(u32, u32)]) -> Scenario`.
- Consumes: the existing `Scenario` fields + `Scenario::parse` (`parser.rs:95`).

**Why (teaching note):** the crate parses but cannot write; the round-trip artifact *is* a scenario file
(spec §3), so the one new format primitive is a serializer. Homing it in `scenario` keeps it Bevy-free,
cheaply round-trip-tested against the parser, on the fast `--workspace` job, and reusable by
`game`/`shot`/Step 5. `to_text` emits the `parser.rs:9-26` grammar; `input` lines are **sparse** (only
ticks where either word is nonzero), values are `ControlState::pack()` already 7-bit — matching the
parser's absent-tick⇒0/0 convention (`parser.rs:262`).

**Steps**

- [ ] **RED (round-trip property):** add a test asserting `Scenario::parse(&s.to_text()) == s` for
      scenarios exercising **every** directive (seed/level/ticks/worm×2/weapon-with-ammo/`game_mode`/
      `max_bonuses`/sparse `input` incl. a max 7-bit word `127`). Also assert sparse emission (an
      all-zero tick emits **no** `input` line) and worm-index order (`w0` is worm 0). `to_text`
      `unimplemented!()` first → FAIL.
- [ ] **GREEN (to_text):** implement `to_text` emitting the grammar; iterate `input` ticks in ascending
      order, skipping `(0,0)`. Cite `parser.rs:9-26` (grammar) and `state.rs:81` (`pack`).
- [ ] **RED (builder):** test `base.with_recorded_inputs(&[(5,3),(0,0),(127,0)])` yields a scenario whose
      `ticks == 3`, `input(0)==(5,3)`, `input(1)==(0,0)`, `input(2)==(127,0)`, and whose seed/level/worms
      match `base`. FAIL.
- [ ] **GREEN (builder):** implement `with_recorded_inputs` (clone base, replace the inputs map, set
      `ticks = per_tick.len()`).
- [ ] Run `cargo test -p scenario` — PASS. Confirm `cargo tree -p scenario` shows no `bevy*`.
- [ ] Reviewer (Opus): `parse∘to_text` is identity over all directives; sparse encoding matches the
      parser convention; worm-index order preserved; Bevy-free; no existing golden touched.
- [ ] **Commit:**
      - `git add rust/scenario/src/parser.rs rust/scenario/src/lib.rs`
      - `git commit -m "scenario(4b): Scenario::to_text serializer + with_recorded_inputs builder"`

---

### T1 — Recorder + tap the 4a seam + flush-on-exit + `--record`  [Opus]

**Files**
- Modify: `rust/game/src/input.rs` (Recorder + `parse_args`), `rust/game/src/main.rs`

**Interfaces**
- Produces: `Recorder { base: Scenario, snapshots: Vec<[ControlState; N_WORMS]> }` with
  `record(&mut self, inputs: &[ControlState; N_WORMS])` and `build(&self) -> Scenario` (via
  `with_recorded_inputs`); `parse_args` recognizing `--record <path>`.
- Consumes: the 4a seam in `tick_and_render` (`main.rs:290-292`); `scenario::Scenario`,
  `sim::state::ControlState`.

**Why (teaching note):** the recorder taps the **sampled array** between sample and `process_frame` (4a
§4.3 step 2 / spec §4.1) — so the Dig→Left+Right chord is already resolved and replay never needs the
bindings. Buffer in memory, flush once on `AppExit` (spec §4.2): a match's stream is tiny and a single
write avoids partial files. Live-mode only, so Scripted stays byte-unchanged.

**Steps**

- [ ] **RED (recorder unit):** test that a `Recorder` over a base scenario, fed a hand-built sequence of
      `[ControlState; N]` arrays, `build()`s a scenario whose `input(t,w)` equals each fed word and whose
      `ticks` equals the sequence length. FAIL first.
- [ ] **GREEN (recorder):** implement `Recorder::record`/`build` (map each `[cs0,cs1]` to
      `(cs0.pack(), cs1.pack())`, call `with_recorded_inputs`).
- [ ] **RED (args):** extend the `parse_args` test — `["--live","--record","/tmp/r.txt"]` →
      `(Mode::Live, name="blood", record=Some("/tmp/r.txt"))`; plain `["--live"]` → `record=None`. FAIL.
- [ ] **GREEN (args):** extend `parse_args` (`input.rs:145`) to a struct/tuple carrying `record: Option<
      PathBuf>` (and `replay` — T2). Keep the leading-`--live` + positional-name behavior intact.
- [ ] **Wire main.rs:** in Live mode insert a `Recorder` resource cloned from the base scenario; in
      `tick_and_render` call `recorder.record(&inputs)` at the seam (`main.rs:291`, Live only — behind a
      `mode == Live` gate or an `Option<ResMut<Recorder>>`). Add a flush system on `AppExit` (or `Last`
      reacting to the exit) that writes `recorder.build().to_text()` to the `--record` path. Scripted
      path unchanged (no recorder inserted).
- [ ] `cargo build -p game`; `cargo test -p game` — green (4a pass-through still passes; no recorder in
      Scripted).
- [ ] Reviewer (Opus): the tap is the sampled array (chord resolved), Live-only; flush writes a valid
      scenario via `to_text`; Scripted byte-unchanged; no sim/render change.
- [ ] **Commit:**
      - `git add rust/game/src/input.rs rust/game/src/main.rs`
      - `git commit -m "game(4b): Recorder taps the sampled stream; --record flushes a scenario file"`

---

### T2 — `--replay <path>` (windowed) + headless `replay_state_series`  [Sonnet]

**Files**
- Modify: `rust/game/src/input.rs` (`replay_state_series` + `--replay` in `parse_args`),
  `rust/game/src/main.rs` (windowed replay branch)

**Interfaces**
- Produces: `replay_state_series(tc_root, &Scenario) -> Vec<u32>` (headless: `scenario::load` →
  drive `InputSource::Scripted` → collect per-tick `hash_game_state`, tick 0 hashed before the first
  `process_frame`); `--replay <path>` → windowed Scripted playback of an arbitrary file, guard/loop off.
- Consumes: `scenario::{load, Scenario}`, `sim::hash::hash_game_state`, `InputSource::Scripted`.

**Why (teaching note):** replay is `InputSource::Scripted` verbatim — the recorded file is a scenario, so
no new replay engine (spec §5). `replay_state_series` mirrors the 4a pass-through harness
(`tests/passthrough.rs`) and the `shot::render_scenario` shape (`shot/src/lib.rs:200`), so a recorded
artifact is `shot`-drivable unchanged (4g). `--replay` loads an **arbitrary path**, bypassing the
golden-dir name validation (`main.rs:128`) that the positional `<name>` keeps.

**Steps**

- [ ] **RED (args):** `["--replay","/tmp/r.txt"]` → `replay=Some("/tmp/r.txt")` (mode irrelevant/
      Scripted); `--replay` and `--live` are mutually exclusive (error or documented precedence). FAIL.
- [ ] **GREEN (args):** add `--replay` to `parse_args`.
- [ ] **RED (headless replay):** unit/integration test: `replay_state_series` over a committed 3b
      scenario reproduces its golden `state_hash` column (reuse the passthrough helper). FAIL first if the
      drive/index alignment is off.
- [ ] **GREEN (replay):** implement `replay_state_series` (tick 0 pre-hash, then
      `Scripted.sample(k-1,&empty)` → `process_frame` → hash, exactly as `passthrough.rs`).
- [ ] **Wire main.rs:** `--replay <path>` reads the file text (not the golden dir), parses, inserts
      `InputSource::Scripted`, runs windowed with the debug self-check **off** and loop/reload **off**
      (play once through `ticks`). Native-only (`cfg(not(wasm))`); wasm path untouched.
- [ ] `cargo build -p game`; `cargo test -p game` — green.
- [ ] Reviewer (Opus): replay reuses Scripted (no new engine); `replay_state_series` matches the
      passthrough drive; `--replay` loads arbitrary paths with guard/loop off; wasm untouched.
- [ ] **Commit:**
      - `git add rust/game/src/input.rs rust/game/src/main.rs`
      - `git commit -m "game(4b): --replay arbitrary scenario + headless replay_state_series"`

---

### T3 — MILESTONE: the non-vacuous round-trip gate + CI  [Opus]

**Files**
- Create: `rust/game/tests/round_trip.rs`

**Interfaces**
- Consumes: `game`'s `InputSource::Live`, `default_bindings`, `Recorder`, `replay_state_series`;
  `bevy::input::ButtonInput<KeyCode>`; `scenario::Scenario`; `sim::hash::hash_game_state`.
- Produces: the self-checking record→replay determinism gate (spec §6), headless, in CI.

**Why (teaching note):** THE Step 4 gate — determinism survives real input (overview Hard gate 1; iter
§3). Non-vacuosity is the whole point (spec §6): record through the **real `Live` sampler** over a
**synthetic stream authored here** (not any committed scenario), route through **text** serialize→parse,
replay through **Scripted**, and assert the two independently-derived `hash_game_state` series are
identical. Different code paths (Live vs Scripted), a stream no golden encodes, and a real file hop make
any recorder/serializer/sampler asymmetry turn it red. No committed golden needed — the two series must
agree.

**Steps**

- [ ] **RED:** create `tests/round_trip.rs`. Author a synthetic per-tick key sequence over `blood`
      exercising: multi-tick held movement, fire, weapon change, key **releases**, and the **Dig chord**
      (hold Left+Right together — default-unbound dig, 4a §3). For each tick: press/release the keys on a
      plain `ButtonInput<KeyCode>`, `InputSource::Live(default_bindings()).sample(t,&keys)` → (a) feed a
      live `SimState` and collect `hash_game_state` (record-series), (b) `recorder.record`. Then
      `recorder.build().to_text()` → `Scenario::parse` → `replay_state_series` (replay-series). Assert
      `record_series == replay_series` tick-for-tick, and spot-assert a few decoded `input` words against
      hand-authored expectations (incl. the chord tick = LEFT|RIGHT set). Run
      `cargo test -p game --test round_trip` → SEE IT FAIL (before T1/T2 land it will not even build; once
      they land, a deliberately broken serializer/tap turns it red — verify by temporarily perturbing).
- [ ] **GREEN:** with T0–T2 in place the gate passes. Confirm the synthetic stream differs from every
      committed scenario (it is authored inline).
- [ ] Add a second, longer synthetic stream (more ticks, both worms active) so the gate is not a
      single-vector fluke.
- [ ] **CI:** confirm the gate runs under `cargo test -p game` (already CI-wired since 4a — no workflow
      change). Run `cargo test -p game` + `cargo test --workspace --exclude game` (priors untouched).
- [ ] Reviewer (Opus): the gate records via **Live** (not Scripted — non-vacuous), routes through text,
      replays via Scripted, asserts equal series + decoded-word spot-checks; the Dig chord is exercised;
      headless; no golden dependency.
- [ ] **MILESTONE.** Record→replay is bit-exact over synthetic Live streams — Step 4's headline gate is
      green and the Step 5 input-log precondition is delivered.
- [ ] **Commit:**
      - `git add rust/game/tests/round_trip.rs`
      - `git commit -m "game(4b): non-vacuous record->replay round-trip determinism gate"`

---

### T4 — committed recorded corpus + regression golden (drift backstop)  [Sonnet]

**Files**
- Create: `rust/game/tests/record_regression.rs`,
  `rust/oracle-tests/golden/record_slice4b_<name>_scenario.txt`,
  `rust/oracle-tests/golden/record_slice4b_<name>.txt`

**Interfaces**
- Produces: one committed recorded scenario + its sim-produced `state_hash` sidecar; a pass-through-style
  test asserting each replayed tick against the committed column (spec §6b).

**Why (teaching note):** the pure round-trip (T3) has one blind spot — a bug that drifts **both** the
recorder and the parser *identically* (spec §9). A committed artifact whose `state_hash` column is
produced by the **sim** (`shot --hashes` / `replay_state_series`) pins **absolute** values, so a symmetric
drift still turns red. It also makes the artifact concrete: `shot`-drivable (drop in the golden dir) and a
Step-5 rollback fixture ("one artifact, four uses", iter §3).

**Steps**

- [ ] Generate the artifact: run the T3 synthetic stream once through record → `to_text`, write it to
      `record_slice4b_<name>_scenario.txt` (a small helper binary/test-writer, or capture the T3
      serialization). Confirm it `Scenario::parse`s.
- [ ] Produce the `state_hash` sidecar via `shot --scenario record_slice4b_<name> --tick <ticks>
      --hashes` (drop the scenario in the golden dir so `shot` resolves it) — or via `replay_state_series`
      — writing the `<tick> <frame_hash> <state_hash>` grammar the passthrough helper reads.
- [ ] **RED→GREEN:** `tests/record_regression.rs` drives `replay_state_series` over the committed artifact
      and asserts each tick equals the committed `state_hash` column (mirror `passthrough.rs`). It fails
      if the artifact and the golden disagree; passes once both are committed and consistent.
- [ ] `cargo test -p game`; `cargo test --workspace --exclude game` — green. Confirm `git diff --stat` on
      `golden/` shows **only** the two new `record_slice4b_*` files (no existing golden touched).
- [ ] Reviewer (Opus): the sidecar column is sim-produced (independent of the input pipeline); the test is
      the pass-through shape; only new goldens added; `shot` resolves the artifact.
- [ ] **Commit:**
      - `git add rust/game/tests/record_regression.rs rust/oracle-tests/golden/record_slice4b_<name>_scenario.txt rust/oracle-tests/golden/record_slice4b_<name>.txt`
      - `git commit -m "game(4b): committed recorded corpus + state-hash regression backstop"`

---

### T5 — manual record→replay + PROGRESS + overview 4b line + broad review  [Sonnet]

**Files**
- Modify: `docs/superpowers/liero-rs-PROGRESS.md`; the overview's 4b bullet
  (`docs/superpowers/specs/2026-07-12-liero-rs-step4-input-replay-overview.md`)

**Steps**

- [ ] **Manual milestone (advisory, not gated):** `cargo run -p game -- --live --record /tmp/m.txt`,
      play a short match, quit (Esc); then `cargo run -p game -- --replay /tmp/m.txt` and eyeball that the
      replay reproduces the same match. Record the observation in the done-report (same posture as 4a's
      manual playability check).
- [ ] Confirm the full green board: `cargo test -p game` (recorder/replay units + round-trip gate +
      record-regression), `cargo test --workspace --exclude game` (all priors incl. the scenario
      serializer round-trip), `cargo build -p game`. Confirm `sim`/`render` Bevy-free and unchanged; the
      `golden/` diff shows only the new `record_slice4b_*` files (`git diff --stat`).
- [ ] Update `docs/superpowers/liero-rs-PROGRESS.md` (use the real `currentDate`): Step 4 slice 4b DONE —
      record→replay round-trip is bit-exact; the artifact is a scenario file (no new format, serializer
      added to `scenario`); replay reuses `InputSource::Scripted`; the non-vacuous round-trip gate + the
      committed-corpus backstop run in CI via `cargo test -p game`; `.lrp` remains 4e; the per-tick stream
      is the Step 5 ggrs input-log.
- [ ] Update the overview's 4b bullet to landed (companion spec implemented) and record how the spec open
      questions resolved (serializer home; `--replay` surface; committed corpus). Note the
      `prev_control_states` risk resolved as a **non-issue** for the Rust absolute-per-tick format
      (spec §9), applying only to 4e's `.lrp` reader.
- [ ] **Broad slice review (Opus):** re-read the whole 4b diff against the spec — artifact is a scenario
      file (no parallel/binary/delta format), recorder taps the sampled array Live-only, replay reuses
      Scripted, the round-trip gate is non-vacuous (Live→text→Scripted), the corpus backstop is
      sim-produced, no sim/render change, no existing golden touched, wasm untouched.
- [ ] **Commit:**
      - `git add docs/superpowers/liero-rs-PROGRESS.md docs/superpowers/specs/2026-07-12-liero-rs-step4-input-replay-overview.md`
      - `git commit -m "docs(4b): PROGRESS + overview slice-4b landed; prev_control_states non-issue"`

## Done-report (each task)

(a) what changed + why, (b) files touched, (c) tests/risks. Per-task commit, local, on branch
`liero-rs-step-4`. **Do not push, do not open a PR** — the controller owns push + PR. Surface in the
final report: the **round-trip gate evidence** (record-series == replay-series over synthetic Live
streams, incl. the Dig chord, headless, in CI — T3), the **format decision** (artifact = scenario file;
serializer the only new primitive — T0), the **committed-corpus backstop** (sim-produced state-hash
golden — T4), the **manual record→replay result** (T5), and confirmation the **sim/render crates are
byte-unchanged** and **no existing golden was touched** (only new `record_slice4b_*` added).
