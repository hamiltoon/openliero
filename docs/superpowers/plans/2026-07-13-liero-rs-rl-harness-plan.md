# Liero-rs — RL harness (PettingZoo) PLAN (T0..T7)

Status: **PLAN — TDD task breakdown for the RL-HARNESS track** · 2026-07-13
Design: `docs/superpowers/specs/2026-07-13-liero-rs-rl-harness-pettingzoo-design.md`
Track: RL-HARNESS · John-decision: PettingZoo `ParallelEnv` = core API.

Builds a thin PyO3 harness over the landed Bevy-free `sim`/`scenario` surfaces so an
SB3-PPO agent can train against a random opponent (reward rises) and an eval recording
replays in the real window. **8 tasks, T0..T7.** Each task is TDD (RED → GREEN → review).
Model tier per task: `[Opus]` = hard ports / all reviewers / integration; `[Sonnet]` =
mechanical, well-specified.

---

## Global constraints (inherited + Python tooling)

- **No new deps in `sim`/`render`/`scenario`/`sim-core`/`assets`.** The env only *reads*
  them. New third-party crates (pyo3, numpy-bridge) live in `rust/liero-env` only.
- **No C++ / oracle-dumper changes.** The RL track is pure Rust+Python, strictly downstream.
- **`sim` crate untouched** — consumer only. All obs/reward fields are already `pub`
  (verified in the design §1.3); if an edit to `sim` seems needed, STOP and re-scope.
- **Determinism preserved:** fixed→float one-directional (no float re-enters the sim);
  policy/exploration RNG separate from `SimState.rand`.
- **Python tooling:** a project-local `venv` + `maturin develop` (editable extension).
  No system-Python pollution. All Python versions pinned in T0 (a committed lock/constraints
  file). No network assumptions beyond the initial `pip install` of pinned versions.
- **Determinism gate every task that touches the tick loop:** same `(seed, action stream)`
  → identical `wide_rollback_checksum` trace (`sim/src/wide_checksum.rs:45`).
- **Docs/comments English.** Follow existing crate conventions (rustdoc on public items).
- **Workspace stays green:** `cargo test --workspace` must pass after every Rust task
  (T0 resolves the PyO3 `extension-module` link caveat so this holds).

---

## T0 — Research + FFI bench + toolchain decisions `[Opus]`

**The research task. No env code yet.** Answers every design open question so later tasks
are mechanical.

- Pin exact versions (committed constraints file): `pyo3`, `maturin`, `pettingzoo`,
  `gymnasium`, `stable-baselines3`, `torch` (MPS). Verify PettingZoo `ParallelEnv` +
  Gymnasium wrapper API shapes against **current** docs (the June exploration's APIs are
  unverified assumptions).
- Spike a throwaway PyO3 module that calls `scenario::load` + `process_frame` in a loop;
  **benchmark headless ticks/sec** single-env (pure Rust) vs per-`step` through PyO3.
  Record numbers; decide single-env vs batched-`VecEnv` for the milestone.
- Decide **workspace membership**: `extension-module` feature-gate (preferred) vs
  `[workspace] exclude`, such that `cargo test --workspace` stays green with a PyO3 cdylib.
- Re-verify (code-read) the **side-channel no-sink** reasoning under a long headless run:
  `sound_events`/`shake_events` peak is one-tick-bounded (overwritten by `take_frame` each
  tick), `flash` is a reseeded scalar cell — no growth, no sink needed.
- **Deliverable:** a short findings note (versions + bench numbers + membership decision +
  go/no-go on batching) appended to the plan or a T0 scratch doc; the pinned constraints
  file; the spike discarded (not committed) or kept as a `benches/` micro-bench.

## T1 — Rust env core (`rust/liero-env`, pure Rust) `[Opus]`

Create the crate and the `LieroEnv` core with **no PyO3 yet** — workspace-testable Rust.

- `LieroEnv::reset(seed)`: parse the embedded `rl_default_match` fixture → `Scenario`,
  override `seed`, `scenario::load` → hold `Loaded.state`. `step(&[cs0,cs1])`:
  `process_frame` (× frame_skip), advance tick counter. `terminated`/`truncated` logic
  (design §5).
- **RED:** determinism test — same `(seed, action stream)` yields an identical
  `wide_rollback_checksum` trace across two runs; a `truncated` test at `max_ticks`; a
  `terminated` test on a scripted kill.
- Depends only on `sim`/`scenario`/`sim-core`/`assets`. No Bevy/render/game.
- `cargo test --workspace` green.

## T2 — Observation + action + reward (Rust) `[Sonnet]`

The three pure Rust value-mappers, fully specified by design §2–§4.

- `action.rs`: `pack7([bool;7]) -> ControlState` / `unpack7` inverse (MultiBinary(7) ↔ 7-bit
  word). Tests: round-trip, each bit maps to the right control.
- `obs.rs`: fixed→float egocentric per-agent `Vec<f32>` (design §3.1 layout), normalized,
  one-directional. Tests: known `WormState` → expected normalized ranges; assert no float
  path writes back into the sim.
- `reward.rs`: shaped reward over `(prev,next)` `WormState` with Python-supplied weights.
  Tests: kill → `+KILL`, death → `−DEATH`, damage-dealt/taken deltas signed correctly,
  time penalty applied.
- `cargo test --workspace` green.

## T3 — PyO3 binding + maturin/venv `[Opus]`

Expose `RawEnv` to Python; stand up the Python build.

- `lib.rs` `#[pymodule]`: `RawEnv.reset(seed)/step(actions)/observe()/record_tick()`,
  returning obs as a contiguous `f32` array (numpy-compatible). Apply the T0 membership
  decision (feature-gate `extension-module`).
- Set up `venv` + `maturin develop`; a Python smoke test: `import liero_env`, `reset`,
  `step` random actions, and a **determinism check** (same seed+actions → identical obs).
- **RED:** the Python smoke/determinism test fails before the binding exists.
- `cargo test --workspace` still green (rlib unit tests unaffected by the cdylib).

## T4 — PettingZoo `ParallelEnv` + Gymnasium wrapper (Python) `[Opus]`

The core API and the SB3 on-ramp.

- `python/liero_env/__init__.py`: `LieroParallelEnv(ParallelEnv)` over the two worm-agents
  (`action_space = MultiBinary(7)`, `observation_space = Box(f32)`), thin over `RawEnv`;
  and a `LieroGymEnv(gymnasium.Env)` single-agent wrapper with a **frozen opponent**
  (random/scripted policy) for SB3.
- **RED/gate:** PettingZoo `parallel_api_test(LieroParallelEnv())` passes; Gymnasium
  `check_env(LieroGymEnv())` passes.

## T5 — Eval recording → `--replay` `[Sonnet]`

The watch path (design §1.5, §7).

- `record.rs`: accumulate `(w0,w1)` per tick; on episode end build a `Scenario` via
  `with_recorded_inputs` + `to_text`; a Python helper writes the `.txt`.
- **RED:** round-trip test — record a scripted episode, write the file, `Scenario::parse` it
  back, assert the input stream matches; and an integration check that
  `cargo run -p game -- --replay <file>` loads and plays it (headless smoke via the `shot`
  CLI or a short run).

## T6 — Training examples + MILESTONE `[Opus]`

The gate. Not a full training project — minimal runnable examples.

- `examples/train_ppo_sb3.py`: SB3-PPO on `LieroGymEnv` vs a random opponent; short run.
- `examples/self_play.py`: frozen-copy-rotation self-play **sketch** (opponent = past
  policy snapshot) — documented, not a tuned league.
- **🎯 MILESTONE:** run the short PPO training and assert the **reward curve rises**
  (mean episodic reward over the first N updates trends up, above a random-policy baseline);
  produce **one eval recording that replays in the window** (`--replay`). Both demonstrated,
  not asserted-by-hope — capture the reward numbers and confirm the replay opens.

## T7 — Broad final review `[Opus]`

Full-track review across T0..T6: constraints held (no sim/C++/render/scenario deps or
edits; determinism one-directional; side-channels no-sink), the PettingZoo `ParallelEnv`
and Gymnasium wrapper are API-conformant, the milestone evidence is real (reward-rise +
replay), `cargo test --workspace` green, Python tests green in the pinned venv, and the eval
recording round-trips. Produce the ship/no-ship report and the deferral track (terrain-patch
obs, pixel obs, reduced 57-action wrapper, self-play league, `InputSource::Agent` slice,
curriculum widening).

---

## Task → tier summary

| Task | What | Tier |
|---|---|---|
| T0 | Research: versions + FFI bench + workspace-membership + no-sink re-verify | Opus |
| T1 | Rust env core (`LieroEnv` reset/step/done), determinism test | Opus |
| T2 | obs + action + reward (pure Rust value-mappers) | Sonnet |
| T3 | PyO3 `RawEnv` binding + maturin/venv + Python determinism smoke | Opus |
| T4 | PettingZoo `ParallelEnv` + Gymnasium frozen-opponent wrapper | Opus |
| T5 | Eval recording → `--replay` round-trip | Sonnet |
| T6 | 🎯 SB3-PPO example: reward rises + eval replay in window | Opus |
| T7 | Broad final review + deferral track | Opus |

**Deferred (tracked, not in this plan):** terrain-patch (CNN) and full-pixel observations;
the reduced 57-symbol action wrapper; imitation of a (not-yet-ported) `DumbLieroAI`;
self-play opponent league; curriculum widening (weapons/levels/spawns/modes); the real-time
`InputSource::Agent` slice in `game`; batched `VecEnv` if T0's bench shows FFI does not
dominate.
