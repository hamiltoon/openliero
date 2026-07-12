# Step 4 · Slice 4b — Record / replay round-trip + CI regression: detailed design

Status: **draft for review** · 2026-07-12
Part of: `2026-07-12-liero-rs-step4-input-replay-overview.md` (cited as **overview**)
Sources: `2026-07-12-liero-rs-step4-slice4a-live-input-design.md` (the live-input core + the recorder
seam, **4a §N**), `2026-07-12-liero-rs-step4-cpp-input-replay-map.md` (**input-map §N**),
`2026-06-26-liero-rs-interactive-iteration-exploration.md` (**iter §N**).
Companion output feeds: `superpowers:writing-plans` (this is the spec; the plan is the sibling file).

This is the executable design for **Step 4's hard gate**: prove determinism survives *real* input by
recording the per-tick input stream from a live session and replaying it headlessly to the **identical**
`HashGameState` time series (overview Hard gate 1; iter §3). 4a delivered the live sampler and the
`InputSource` seam, and left an explicit *recorder seam* in the single `FixedUpdate` tick system
(`rust/game/src/main.rs:291`, 4a §6). 4b fills that seam, adds the replay path, and stands up the
round-trip regression. Nothing in `sim` changes; the snapshot boundary
`SimState::process_frame(&[ControlState; N])` (`rust/sim/src/state.rs:1431`) is untouched — 4b only
*taps* the array on the way in and *re-feeds* it on the way out.

---

## Goal / done-when

A live (or synthetically-driven) session's per-tick `[ControlState; N]` stream is **recorded** to a
Rust-native artifact and **replayed** headlessly to a bit-identical sim-state series — self-checking, no
C++, wired into CI via `cargo test -p game`.

**Done when:**
1. **Recorder.** In live mode the per-tick input snapshot is captured at the 4a seam and flushed to a
   Rust-native artifact that carries everything `scenario::load` needs for an exact tick-0
   reconstruction (seed / level / worms / loadout / settings) **plus** the per-tick input stream.
   `cargo run -p game -- --live --record <path>` writes it on exit.
2. **Replay.** A headless replay driver reconstructs the sim from the artifact and drives it through the
   existing `InputSource::Scripted` path. `cargo run -p game -- --replay <path>` plays it back in a
   window; the library entry drives it with no window.
3. **Round-trip gate (the Step 4 headline, objective, headless, CI):** a synthetic key stream — authored
   in the test, distinct from every committed scenario — is sampled through the **real**
   `InputSource::Live` sampler, recorded, serialized to text, parsed back, and replayed through
   `InputSource::Scripted`; the record-time and replay-time `hash_game_state` series are asserted
   **equal tick-for-tick**. Self-checking (no golden required); runs under `cargo test -p game`.
4. **Regression corpus.** One recorded artifact is committed together with its independently-produced
   `state_hash` sidecar; a pass-through-style test drives it against that golden, catching whole-pipeline
   drift over time and giving `shot` / Step 5 a concrete artifact ("one artifact, four uses", iter §3).
5. The committed `sim_slice*` / `render_slice3b_*` / `render_slice3e_*` goldens stay **byte-identical**
   (4b is additive; no sim/render change). The 4a pass-through gate stays green.

**Not bit-gated:** the live keyboard capture itself (proved by the round-trip, exactly as 4a's live path
is proved by pass-through). **Not in 4b:** audio (4c), live shake/flash (4d), `.lrp` reading (4e — a
*separate foreign* format, see §8), menu/start-flow (4f), the full run/verify-skill + CI-diff wiring
(4g — 4b wires only its own gate step).

---

## 1. What C++ does — the record/replay pipeline (context, not a port)

The overview's input-map §1b/§3 is the ground truth; the load-bearing facts for 4b:

- **C++ `.lrp` is a *foreign, delta-encoded* format.** `ReplayWriter`/`ReplayController`
  (input-map §3) write a container (`LRPF` magic + version), a cereal `Game` initial state, then a
  per-worm **XOR-delta** `ControlState` stream, with a `WideRollbackChecksum` every 1050 frames
  (input-map §5). The delta is against the *previous* tick's `control_states`
  (`prev_control_states`, input-map §1b/§3c).
- **4b does not touch `.lrp`.** Per overview locked-decision 4 and Open Q6, the *self-authored*
  round-trip artifact is a clean Rust-native format; `.lrp` is read-only foreign interop deferred to
  **4e**. Conflating them would force the round-trip format to carry cereal-`Game` baggage it does not
  need. The C++ record path is cited here only to explain *why* the Rust format can be simpler (§3, §7
  finding).

## 2. What the Rust sim needs for an exact reconstruction

`scenario::load` (`rust/scenario/src/loader.rs:99`) is a *total* function of a parsed `Scenario`: it
reads `seed`, `level`, the `worm` lines, `weapon 0 <name>`, `game_mode`, and derives everything else
deterministically (fixed `settings_weapons`, `killed_timer` default 150, TC scalars). So **a scenario
file already fully determines the tick-0 `SimState`.** The per-tick drive is then a stream of absolute
7-bit `ControlState` words — one per worm, positional by worm index — fed to `process_frame`
(`state.rs:1431`, interleaved in `worms` order, `state.rs:1405-1409`).

The scenario grammar (`rust/scenario/src/parser.rs:9-26`) already encodes **all** of this:

```text
seed <u32>            level <path>          ticks <u32>
worm <i> <x> <y> <health> <lives> <stats_x> <visible>
weapon <slot> <name> [ammo]     max_bonuses <i32>     game_mode <i32>
input <tick> <worm0_7bit> <worm1_7bit>          # sparse; absent tick => 0/0
```

The `input <tick> <w0> <w1>` line **is** the per-tick snapshot stream (`Scenario::input`,
`parser.rs:261`), decoded through `ControlState::unpack` (`state.rs:87`, masks `& 0x7f`).

## 3. The format decision — recorder writes a scenario file (no new format)

**Recommendation: the recorded-input artifact IS a scenario file in the existing grammar.** The recorder
emits a valid `<name>.txt` that the *unchanged* parser reads; replay is the *unchanged*
`InputSource::Scripted` path. This is the overview's "extend the scenario `input` grammar rather than
invent a parallel format" (overview format note) taken to its minimal conclusion — **no grammar
extension is even required**: the grammar already carries initial state + per-tick inputs. "One artifact,
four uses" becomes literal — the same file drives (a) the round-trip gate, (b) `shot` screenshots,
(c) the CI regression, and (d) later Step 5 rollback fixtures.

**The only genuinely new code is a scenario *serializer***: the crate has a parser but no writer. 4b adds
`Scenario::to_text(&self) -> String` (and a small builder that clones a base scenario and replaces its
inputs/`ticks`) to the **`scenario`** crate — Bevy-free, cheaply round-trip-tested against the parser,
and on the fast `--workspace` CI job. Everything downstream (recorder flush, replay, the gate) composes
`parse` and `to_text` with the already-shipped `load` / `InputSource::Scripted`.

**Serialization shape:**
- Emit the state metadata verbatim from the base scenario (`seed`, `level`, `worm` lines, `weapon`,
  `game_mode`, `max_bonuses`). Render-only directives (`render*`) do not affect `hash_game_state` and may
  be carried through or dropped; carry-through is simplest.
- Set `ticks` = the number of recorded ticks.
- Emit **sparse** `input` lines: one line per tick where either worm's word is nonzero, `input <t> <w0>
  <w1>` with `w = ControlState::pack()` (`state.rs:81`, already 7-bit). Absent ticks decode to `0/0`,
  matching the parser's convention (`parser.rs:262`) and keeping recorded files small and diff-friendly.

**Rejected alternatives:** a parallel Rust-native binary format, or a delta/XOR encoding mirroring C++.
Both are premature — the scenario text is human-readable, already parsed, already the `shot`/Step-5
currency, and a match's input stream is tiny. A compact binary sibling is an *explicit non-goal* for 4b
(revisit only if Step 5 volume demands it; overview Open Q6 permits it "or a compact binary sibling", but
YAGNI here). The delta encoding is a C++-`.lrp` concern that does not apply (see §6 finding).

## 4. Recorder architecture

### 4.1 Where it taps
The recorder tap is the seam 4a already marked — inside the single `FixedUpdate` tick system, **between
the sample and `process_frame`** (`main.rs:290-292`, 4a §4.3 step 2):

```
sample_tick_and_render (FixedUpdate, unchanged cadence):
  1. let inputs = source.sample(demo.tick, &keys);   // ONE snapshot / tick (4a)
  2. recorder.record(&inputs);                        // <-- 4b: tap the array (Live mode only)
  3. sim.0.process_frame(&inputs);
  4. ... (scripted-only loop/reload, render, debug self-check — unchanged)
```

Recording the *sampled array* (not the raw keys) means the recorder captures exactly what the sim saw —
including the Dig→Left+Right chord expansion (4a §3), which is resolved *before* the tap. So a replay
never needs the bindings or the chord logic; it just re-feeds absolute words. This is the whole reason
the format is simple.

### 4.2 What it holds and how it flushes
A `Recorder` `Resource` holds (a) a clone of the **base** `Scenario` (its metadata — the live match was
still launched from a scenario for its initial state, 4a §7) and (b) a growing
`Vec<[ControlState; N_WORMS]>` of per-tick snapshots. `record` pushes one array per tick.

**Flush strategy: buffer in memory, write once on exit.** A match's stream is small (tens of bytes/tick),
so an in-memory `Vec` + a single serialize-and-write on `AppExit` (Esc / window close) is simplest and
avoids partial files. The flush system reads the recorder, builds the recording scenario
(`base.with_recorded_inputs(&snapshots)`), and writes `to_text()` to the `--record` path. Trade-off: a
hard crash loses the recording — acceptable for a dev/test artifact; a periodic flush is a later
optimization if needed (noted risk §7).

Recording is **Live-mode only**: Scripted already *is* the recorded stream, so re-recording it is the
vacuous case (§6). No recorder is inserted in Scripted mode, so the scripted path — and its 4a
pass-through gate — stays byte-unchanged.

## 5. Replay path

Replay is **`InputSource::Scripted` verbatim** — the recorded file is a scenario, so playback needs *no
new replay engine*. Two entries:

- **Windowed:** `cargo run -p game -- --replay <path>` reads the scenario text from an arbitrary path
  (bypassing the golden-dir name validation `resolve_scenario` uses, `main.rs:128`), constructs
  `InputSource::Scripted`, and runs with the debug self-check **off** (no committed golden for a recorded
  file) and the loop/reload **off** (play once through `ticks`, then hold). Native-only, like `--live`.
- **Headless (the gate + library):** a small `replay_state_series(scenario) -> Vec<u32>` library
  function that mirrors the 4a pass-through harness (`rust/game/tests/passthrough.rs`): `scenario::load`
  the state, then for each tick feed `InputSource::Scripted(scn).sample(t, &empty)` into `process_frame`
  and collect `hash_game_state`. This is the object the round-trip gate and the regression corpus both
  drive — and it is deliberately the *same* shape as `shot::render_scenario`
  (`rust/shot/src/lib.rs:200`), so a recorded artifact is `shot`-drivable unchanged (§7, 4g).

## 6. The round-trip gate — non-vacuous construction

The trap the overview names: recording *through the Scripted source* and replaying it is
**vacuous** — it asserts `Scripted == Scripted`, proving nothing about the live path or the recorder.
The gate must record a stream that (a) comes from the **live** sampler and (b) is **not** any committed
scenario. Construction (`rust/game/tests/round_trip.rs`, headless):

1. **Author a synthetic key stream in the test** — a `Vec` of "keys held this tick" over a chosen base
   scenario (`blood`), exercising held movement across multiple ticks, fire, weapon change, key
   *releases*, and the **Dig chord** (holding Left+Right together, the default-unbound dig path, 4a §3).
   This stream exists only in the test; no committed golden encodes it.
2. **Record through the REAL `Live` sampler.** For each tick, press/release the synthetic keys on a plain
   `ButtonInput<KeyCode>` resource (no Bevy app, no window — `ButtonInput` is a mutable resource) and call
   the real `InputSource::Live(default_bindings()).sample(t, &keys)`. Tap each array into a `Recorder`.
   Collect the **record-time** `hash_game_state` series by also feeding each array into a live `SimState`.
3. **Serialize → parse (exercise the file format).** `recorder.build().to_text()` → `Scenario::parse`.
   Routing through *text* (not an in-memory `Scenario`) makes a serializer bug — wrong sparse encoding,
   dropped metadata, wrong `ticks` — turn the gate red.
4. **Replay through `InputSource::Scripted`.** Drive the parsed scenario through
   `replay_state_series` (§5) to get the **replay-time** series.
5. **Assert the two series are identical, tick-for-tick**, and spot-assert a few decoded `input` words
   against hand-authored expectations (so a *symmetric* bit swap is caught, not just self-consistency).

**Why this is non-vacuous — three independent asymmetries any bug breaks:**
- **Different code paths on the two sides.** Record goes through `Live` (bindings + Dig chord + `pack`);
  replay goes through `Scripted` (`Scenario::input` + `unpack`). A disagreement between `pack`/`unpack`,
  or a binding/chord error, diverges the series.
- **A stream no committed scenario contains**, so the recorded `input` values are specific to this test
  and hand-verifiable — not silently equal to a pre-existing golden.
- **A real serialize→parse hop**, so the on-disk format is exercised end-to-end, not bypassed.

The gate needs **no committed golden** — the two independently-derived series must agree — which is
exactly Hard gate 1 ("self-checking, no C++", overview). The committed corpus (§ next) adds the
absolute-value backstop.

### 6b. Committed regression corpus (the drift backstop + the shared artifact)
Separately, commit **one** recorded scenario (`record_slice4b_<name>_scenario.txt`) produced from a
synthetic stream, plus a `state_hash` sidecar whose column is produced by the **sim** (via
`shot --hashes` / `replay_state_series`), and drive it pass-through-style asserting each tick against the
committed column. This catches the one hole a pure round-trip cannot: a bug that drifts **both** the
recorder and the parser *identically* (a symmetric change) — the sim-produced absolute hashes still turn
red. It also makes the artifact concrete for `shot` (drop it in the golden dir) and for Step 5 (§8).

## 7. CLI, UX, verification, CI

- **Record:** `cargo run -p game -- --live --record <path>` — live match; on exit the recorder flushes
  the scenario file to `<path>`. `--record` is a value flag consumed alongside `--live` (native-only,
  `cfg(not(wasm))`).
- **Replay:** `cargo run -p game -- --replay <path>` — windowed playback of an arbitrary recorded file
  (guard off, loop off; §5). Extends `parse_args` (`rust/game/src/input.rs:145`) with the two flags; the
  positional-scenario / no-arg-`blood` scripted default and the wasm hard-coded Scripted path
  (`main.rs:149`) are unchanged.
- **Manual milestone (advisory, not gated):** record a short live match, replay it, eyeball that the
  replay reproduces the same match (the human-facing half of the round-trip, complementing the objective
  headless gate). Same posture as 4a's manual playability check.
- **CI:** the round-trip gate (§6) and the corpus regression (§6b) run under `cargo test -p game`
  (already in CI since 4a upgraded the `game` build step to `cargo test -p game`). The scenario serializer
  round-trip unit test rides the fast `cargo test --workspace --exclude game` job (it is in the Bevy-free
  `scenario` crate). No new CI job — 4b slots into the two existing ones. Full run/verify-skill + the
  `.lrp` framehash diff are **4g**.
- **`liero-shot` skill:** no change required for 4b — a recorded artifact is a scenario file, so
  `shot --scenario <recorded> --hashes` drives it by construction once it is in the golden dir. Extending
  the skill text with the record→replay drive is **4g**.

## 8. Seams to 4e and Step 5

- **4e (`.lrp`)** is a *separate, foreign, read-only* format (delta-encoded, cereal `Game`,
  `WideRollbackChecksum`) — overview locked-decision 4 / Open Q6 keep it distinct. 4b's artifact is never
  `.lrp`. If a `.lrp` test corpus is later wanted (overview Open Q2), a recorded scenario could be
  *transcoded* to `.lrp` by a C++-side writer — a 4e concern, not 4b.
- **Step 5 (ggrs).** The recorded per-tick `input` stream *is* the ggrs input-snapshot log: one absolute
  7-bit word per worm per tick, positional by worm index — the precondition for `bevy_ggrs` re-homing
  (overview locked-decision 1). Because the format also carries seed + worms, a recorded file is a
  complete rollback-test fixture: Step 5 replays it under `GgrsSchedule` and diffs the resulting
  `hash_game_state` series against 4b's non-rollback replay for desync detection. 4b's
  `replay_state_series` is the non-rollback reference that Step 5 diffs against.

## 9. Risks & the hard 10%

- **`prev_control_states` baseline — a non-issue for the Rust format (finding).** The overview flagged
  `prev_control_states` baseline reproduction (input-map §1b/§3c) as a 4b risk — inherited from the C++
  `.lrp` *delta* model. **The Rust sim stores no delta baseline:** each tick re-`Unpack`s an *absolute*
  7-bit word into `control_states`, and `ControlState::pressed_once` (`state.rs:117-128`) degenerates the
  C++ edge detection to a per-tick read-and-clear. So the round-trip artifact is absolute-per-tick with
  **no baseline to reconstruct** — the delta concern applies **only** to 4e's `.lrp` reader. Documented
  here so a worker does not build a delta encoder the Rust model does not need.
- **Both-sides-drift blind spot in a pure round-trip.** A self-checking record↔replay gate passes even if
  the sampler-serializer and the parser share a *symmetric* bug. Mitigation: the §6b committed corpus,
  whose `state_hash` column is produced by the **sim** (independent of the input pipeline), pins absolute
  values; plus §6 step 5's hand-authored decoded-word spot-checks.
- **Metadata completeness / initial-state fidelity.** If the serializer drops any state-affecting field
  (`weapon` ammo, `game_mode`, a `worm` field), replay's `scenario::load` yields a different tick-0 and
  the whole series diverges. Mitigation: a `parse(to_text(s)) == s` round-trip property test (T0) over
  scenarios exercising **every** directive.
- **Worm iteration order.** `process_frame` interleaves per-worm in `worms` order (`state.rs:1405-1409`);
  the `[ControlState; N]` array is positional by worm index. The recorder captures the whole array in
  index order and the serializer writes `input <t> <w0> <w1>` in worm-index order — preserved by
  construction; a unit assertion pins `w0`↔worm-0.
- **Flush robustness.** In-memory buffer + single flush on exit loses the recording on a hard crash
  (§4.2). Acceptable for a dev artifact; periodic flush deferred unless a need appears.

## 10. Open questions for the controller (max 3)

1. **Home of the scenario serializer.** Recommendation: `Scenario::to_text` + a `with_recorded_inputs`
   builder in the **`scenario`** crate (round-trip-tested against the parser, Bevy-free, on the fast CI
   job, reused by `game`/`shot`/Step 5). Alternative: assemble the text ad-hoc in `game` — rejected
   (spreads a file-format concern into the Bevy crate and off the fast test job). Accept the `scenario`
   home?
2. **Replay CLI surface.** Recommendation: a distinct `--replay <path>` flag that loads an arbitrary path
   with the guard/loop off, keeping the positional `<name>` strictly for committed goldens. Alternative:
   let the positional arg accept a path too — rejected (muddies the golden-name validation that keeps the
   scripted self-check honest). Accept `--replay`?
3. **Commit the recorded corpus, or generate it in-test only?** Recommendation: commit **one** small
   `record_slice4b_*` artifact + its sim-produced `state_hash` sidecar (§6b) — the drift backstop and the
   shared `shot`/Step-5 artifact — while the round-trip gate itself stays golden-free (§6). Alternative:
   keep everything ephemeral in-test — rejected (loses the both-sides-drift backstop and the concrete
   downstream artifact). Accept the one committed corpus?
