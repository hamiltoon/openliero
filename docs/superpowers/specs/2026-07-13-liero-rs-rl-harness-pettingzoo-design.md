# Liero-rs — RL harness (PettingZoo) DESIGN

Status: **DESIGN — committed direction for the ✨ NEW-track RL harness** · 2026-07-13
Track: RL-HARNESS (train an AI to play Liero via PettingZoo)
John-decision: **PettingZoo `ParallelEnv` is the core API.**
Supersedes the recommendation half of `2026-06-27-liero-rs-rl-self-play-exploration.md`
(the June exploration). Where June said "PPO via CleanRL/SB3, action space = engine's
0..56 `InputState` alphabet, obs = MLP-first", this design **re-adjudicates against
today's actual Rust surface** (steps 2–4 landed) and commits concrete shapes.

> **What changed since June.** June reasoned about a *C++* engine and a hypothetical
> Rust port. Steps 2–4 are now real: a deterministic headless `SimState::process_frame`,
> a Bevy-free `scenario` crate with a tick-0 `load()` builder and a `to_text` /
> `with_recorded_inputs` recorder, an `InputSource{Scripted,Live}` seam in `game`, a
> `wide_rollback_checksum`, and bounded thread-local side-channels. The harness is now a
> *thin PyO3 consumer of these existing surfaces* — no new sim code. This doc pins the
> action/observation/reward representations, the crate layout, the episode contract, the
> eval-recording path, and the `InputSource::Agent` adjudication.

---

## 0. TL;DR (the commitments)

- **Core API: PettingZoo `ParallelEnv`** (two symmetric worm-agents, one action per agent
  per tick). A **Gymnasium** single-agent wrapper (frozen opponent) sits on top for a
  one-line Stable-Baselines3 (SB3) start.
- **Action: `MultiBinary(7)`** — the 7 raw `ControlState` bits (Up/Down/Left/Right/Fire/
  Change/Jump). It maps 1:1 onto the sim's native 7-bit word via `ControlState::set`,
  is SB3-PPO-native, and is maximally expressive. `Discrete(128)` and the engine's reduced
  57-symbol alphabet are trivially derivable *wrappers* offered later, not the core.
- **Observation: a flat `Box(float32)` egocentric per-agent vector** (self + opponent
  kinematics/health/weapon + relative geometry), extracted **Rust-side, fixed→float,
  one-directional**. A local terrain patch (small CNN) and full pixels (headless `render`)
  are documented v2/v3 upgrades behind the same `observe()` seam.
- **Reward: shaped-then-annealed.** Start: `+damage_dealt −k·damage_taken +KILL −DEATH
  −tiny·time`, all read from `health`/`kills`/`last_killed_by_idx` deltas. Anneal toward the
  sparse `±1` kill/death objective. Start in **KillEmAll** mode (cleanest signal).
- **Episode:** `reset(seed)` rebuilds a `SimState` from a `default_match`-like fixture via
  `scenario::load`, with the seed randomized for variety (determinism holds *per seed*).
  `terminated` = an agent dead with no lives left; `truncated` = `max_ticks` reached.
- **New crate: `rust/liero-env`** (PyO3 + maturin). Depends only on the Bevy-free
  `sim`/`scenario`/`sim-core`/`assets`. **No Bevy, no `render`, no `game` dep.** The sim
  crate is untouched — the harness only *reads*.
- **Eval recording is in scope from day one:** the trainer taps each tick's `(w0,w1)`
  7-bit words, builds a `Scenario` via `with_recorded_inputs`, writes it with `to_text`, and
  replays it in the real window with `cargo run -p game -- --replay <file>`.
- **`InputSource::Agent` (real-time human-vs-bot in the Bevy app) is a SEPARATE deferred
  slice, NOT part of this harness.** The eval-recording path already delivers "watch the
  agent play" with zero new engine code (see §7).

---

## 1. What the harness stands on (verified surfaces)

Everything below is a real, landed surface in the worktree. The harness composes them; it
adds no simulation logic.

### 1.1 The tick: `SimState::process_frame(&[ControlState])`
`rust/sim/src/state.rs:1491`. One deterministic tick. Takes a slice of per-worm
`ControlState` (two worms → `&[cs0, cs1]`). Fully integer/fixed-point, RNG in state
(`SimState.rand`). This is `step()`.

### 1.2 Reset: `scenario::load(tc_root, &Scenario) -> Loaded`
`rust/scenario/src/loader.rs:100`. Builds a complete tick-0 `SimState` (`Loaded.state`,
`loader.rs:87`) from a parsed `Scenario` — resolves weapons, worm inits (pos/health/lives/
visible), seeds `rand` from `scenario.seed`, sets game mode, loads TC constants. This is
`reset()`: parse a base fixture, override `scenario.seed`, call `load`, keep `.state`.
The whole crate is **Bevy-free by construction** (`scenario/src/lib.rs:4-8`), so the env
links it without pulling Bevy.

### 1.3 The observable worm fields: `WormState`
`rust/sim/src/state.rs:263`. Hashed, load-bearing fields the extractor reads:
`pos`/`vel` (`Vec2`, 16.16 fixed), `aiming_angle` (`Fixed`), `aiming_speed`, `direction`,
`health`/`lives`/`kills` (`i32`), `visible` (`bool`), `current_weapon`,
`weapons: [WormWeapon; NUM_WEAPONS]` (each `ty`/`ammo`/`delay_left`/`loading_left`,
`state.rs:173`), `ninjarope`, and `last_killed_by_idx` (`state.rs:284`, death attribution —
who to credit a kill to). Everything the reward and obs need is here.

### 1.4 Determinism oracle for the env: `wide_rollback_checksum(&SimState, &[u32])`
`rust/sim/src/wide_checksum.rs:45`. Folds a *wider* set than the master hash (incl. prev
control states). The env's determinism test asserts: same `(seed, action stream)` →
identical per-tick checksum trace. This is the "flaky-env debugging disappears" dividend,
made concrete.

### 1.5 Eval recording: `Scenario::with_recorded_inputs` + `to_text`
`rust/scenario/src/parser.rs:424` and `:353`. `with_recorded_inputs(&[(u32,u32)])` builds a
new `Scenario` whose per-tick `(w0,w1)` 7-bit inputs are the recorded stream (sparse:
all-zero ticks omitted, round-trips through `parse`). `to_text` serializes it. The `game`
binary already replays such files via `--replay <path>` (`game/src/main.rs:265-275`,
`Mode::Replay`). So the trainer produces a watchable artifact with **existing** machinery.

### 1.6 The input seam: `InputSource{Scripted,Live}`
`rust/game/src/input.rs:125`. `Scripted(Scenario)` is a literal pass-through of recorded
inputs; `Live(InputMap)` polls the keyboard. `sample(tick, keys) -> [ControlState; N_WORMS]`
(`input.rs:278`). A future `Agent` arm would slot here — adjudicated **out of harness scope**
in §7.

### 1.7 Side-channels are headless-safe (verified, no sink needed)
The sim emits sound/shake/flash through **bounded per-tick thread-locals** that
`process_frame` clears at the top and drains into `SimState` at the bottom:

- `sound.rs` / `shake.rs`: `begin_frame()` clears a thread-local `Vec` at tick top
  (`state.rs:1497,1508`); `take_frame()` moves it into `SimState.sound_events` /
  `.shake_events` at tick bottom (`state.rs:2263,2270`), *replacing* the prior Vec.
- `flash.rs`: a thread-local reseeded every `begin_frame(screen_flash)` (`state.rs:1634`);
  non-draining, overwritten next tick.

**Reasoning verified:** a headless RL harness that never calls `drain_shake_events`/never
reads `sound_events` incurs **no unbounded growth** — each tick's `take_frame` overwrites
the previous per-tick Vec (peak = one tick's events, a handful), and `flash` is a single
reseeded scalar cell. No sink, no leak, no determinism effect (the channels are unhashed).
The env simply ignores them. `wide_rollback_checksum` is available if the env wants a
cheap per-episode integrity fingerprint.

---

## 2. Action space — UTRED → recommendation

**Question:** raw 7-bit word `Discrete(128)` vs `MultiBinary(7)` vs `Dict`/`MultiDiscrete`,
optimizing for SB3 compatibility through the Gymnasium wrapper.

**Recommendation: `MultiBinary(7)` as the core action space.**

| Encoding | Maps to `ControlState` | SB3-PPO native? | Expressive? | Verdict |
|---|---|---|---|---|
| **`MultiBinary(7)`** | 1:1 — bit `n` → `cs.set(n, on)` | **yes** (Bernoulli head) | full (all 128 combos) | **CHOSEN** |
| `Discrete(128)` | word → `ControlState::from_bits` | yes (categorical) | full but mixes sane/absurd | derivable wrapper |
| Reduced 57-symbol (`InputState`) | via a decode table | yes (categorical) | game-validated, compact | optional wrapper later |
| `MultiDiscrete([2;7])` | same as MultiBinary | yes | full | equivalent, less idiomatic |
| `Dict` | — | needs flatten | overkill | rejected |

Rationale:
- `MultiBinary(7)` is the **exact shape of the sim's native 7-bit `ControlState`** — the
  policy's 7 independent Bernoulli logits assemble directly, no lossy discretization.
- **SB3 PPO consumes `MultiBinary` natively** (Bernoulli policy), so the Gymnasium wrapper
  needs no action re-encoding — the wrapper's `action_space` *is* `MultiBinary(7)`.
- Absurd combos (Left+Right) are learnable-away; the sim already tolerates them (the control
  path `Unpack`s each tick, `control.rs:562`).
- June preferred the engine's 0..56 alphabet "to bridge imitation of the built-in AI." We
  **defer** that: (a) there is no ported `DumbLieroAI` in Rust yet, so imitation isn't the
  first milestone; (b) `MultiBinary(7)` is strictly more expressive; (c) the 57-symbol space
  is trivially offered later as a *reduced-action wrapper* over the same env if learning
  proves slow. Starting expressive-and-native beats starting reduced-and-bridged.
- **Frame-skip / action-repeat** (hold action k ticks) is a wrapper knob, not an action-space
  change; k∈2..4 shortens the horizon. Keep it tunable; don't skip so coarsely that
  `aiming_speed` overshoots.

Rust side: `action.rs` exposes `pack7(bits: [bool;7]) -> ControlState` and the inverse for
recording.

---

## 3. Observation space — UTRED → recommendation

**Question:** structured float vector vs pixels (headless `render`), and whether to include a
local terrain patch from the start.

**Recommendation: start with a flat `Box(float32)` egocentric per-agent vector, no terrain
patch. Reserve terrain-patch (CNN) as v2 and full pixels as v3, behind the same `observe()`
seam.**

### 3.1 v1 vector (per agent), extracted Rust-side from `WormState`
All normalized to ≈unit scale; **fixed→float is one-directional** (float never re-enters the
sim — the same discipline steps 2/3 enforce). Proposed layout (~30–40 scalars):

- **Self:** `pos.x/W, pos.y/H`, `vel.x, vel.y` (scaled), `sin(aim), cos(aim)`
  (from `aiming_angle` via `cossin`), `aiming_speed`, `direction` (±1),
  `health/settings_health`, `lives` (scaled), `visible` (0/1),
  `able_to_jump`/`able_to_dig`, `ninjarope.out` (0/1) + rope rel-pos if out.
- **Weapons:** `current_weapon` (one-hot over `NUM_WEAPONS`) + per-slot
  `ammo/max`, `delay_left`, `loading_left` (normalized) — teaches reload/ammo discipline.
- **Opponent:** the same kinematics block, plus **relative geometry** — `(opp.pos−self.pos)`
  normalized, distance, and a `visible` flag. June and `InputContext.facing_enemy` both flag
  relative geometry as the key signal; make it explicit rather than let the net rederive it.

The observation is **Markov enough** given near-full state access → no recurrence in v1 (add a
small GRU only if off-screen opponent partial-observability later hurts).

### 3.2 Why not pixels first
- The `render` crate is headless-capable, but pixel obs forces the render path RL otherwise
  avoids, is far costlier per step (throughput is the whole game), and adds a perception
  problem for no early benefit.
- Structured state is *rare and valuable* — Liero exposes it cleanly. Use it.

### 3.3 The upgrade path (same seam)
`observe()` returns a Rust-built contiguous `f32` array. v2 = concat an NxN egocentric crop of
`level.material_id` (dirt/rock/background classes) → small CNN branch. v3 = full pixels via
`render` for generality. All three keep the extractor Rust-side so only compact arrays cross
the FFI boundary (see §8).

---

## 4. Reward — UTRED → basdesign

**Recommendation: shaped-then-annealed, computed Rust-side from per-tick state deltas, in
KillEmAll mode.**

Per agent, per tick (deltas vs previous tick):
- `+ w_dmg · damage_dealt` — opponent `health` decrease attributable to this agent.
- `− w_taken · damage_taken` — this agent's `health` decrease (`w_taken < w_dmg`).
- `+ KILL` on `kills` increment (opponent died crediting this agent via `last_killed_by_idx`).
- `− DEATH` on own `health <= 0` transition.
- `− w_time` tiny per-tick — forces engagement, bounds stalling.
- (optional, early only) `+ tiny·approach` shaping, **annealed out** — shaping teaches
  degenerate habits; keep it light.

Design for iteration: the reward is a small pure Rust function over `(prev, next)` `WormState`
snapshots, with weights passed from Python (so reward tuning is a Python-side experiment, no
recompile). **Anneal** `w_dmg`/`w_taken`/shaping toward the sparse `±1` kill/death objective
as training matures.

Pitfalls (call out, per June §5): damage-farming chip damage, camping for survival, ammo-dump
if firing is directly rewarded (it is **not** here — only *damage dealt* is), and
**ScalesOfJustice** (`game_mode==3`) redistributes health on self-damage → "damage taken is
bad" is gameable. **Start KillEmAll (`game_mode==0`)**; generalize modes later. `do_damage`
(`state.rs:493`) confirms only step-1 direct damage runs in KillEmAll — clean signal.

---

## 5. Episode contract

- **`reset(seed) -> obs`:** parse the embedded `default_match`-like fixture text →
  `Scenario`, override `scenario.seed = seed` for variety, `scenario::load(tc_root, &scn)` →
  keep `Loaded.state`. Determinism is **per seed** (same seed+actions → identical checksum
  trace, §1.4); seed-randomization gives training variety without breaking reproducibility.
- **`step([a0,a1]) -> (obs, rewards, terminated, truncated, info)`:** pack each agent's
  `MultiBinary(7)` → `ControlState`, `process_frame(&[cs0,cs1])` (× frame_skip), extract obs,
  compute per-agent reward.
- **`terminated`:** an agent reaches `health <= 0` with `lives` exhausted (round over). In the
  symmetric 1v1 both agents terminate together.
- **`truncated`:** `tick >= max_ticks` (essential — Liero rounds can stall).
- **Seeding discipline:** the sim RNG (`SimState.rand`) is seeded from the scenario and lives
  entirely in Rust; the policy's exploration RNG is **separate** and never perturbs the sim
  stream (would break the determinism contract).
- **Fixture:** a committed `scenarios/rl_default_match.txt` (KillEmAll, two worms, one open
  level, a starting weapon), analogous to the existing slice fixtures. Curriculum widening
  (weapons → levels-with-cover → random spawns → opponent league) is future work along
  independent, regression-testable axes.

---

## 6. Crate / directory structure

Repo convention: **all Rust crates live under `rust/` as workspace members**
(`rust/Cargo.toml:3`). Follow it.

```
rust/liero-env/                     ← NEW workspace member (PyO3 + maturin)
  Cargo.toml                        crate-type = ["cdylib", "rlib"];
                                    deps: sim, scenario, sim-core, assets
                                    (NO bevy, NO render, NO game)
                                    pyo3 (extension-module gated — see T0)
  pyproject.toml                    maturin build backend, mixed Rust/Python layout
  src/lib.rs                        #[pymodule]: exposes RawEnv to Python
  src/env.rs                        LieroEnv: reset(seed)/step(&[cs;2])/done logic
  src/obs.rs                        fixed→float extractor (§3.1), one-directional
  src/action.rs                     pack7 / unpack7  (MultiBinary(7) ↔ ControlState)
  src/reward.rs                     shaped reward over (prev,next) WormState
  src/record.rs                     tap (w0,w1) → Scenario::with_recorded_inputs → to_text
  python/liero_env/__init__.py      PettingZoo ParallelEnv + Gymnasium wrapper (thin)
  examples/train_ppo_sb3.py         minimal SB3-PPO vs frozen opponent
  examples/self_play.py             frozen-copy-rotation self-play sketch
scenarios/rl_default_match.txt      NEW episode fixture (KillEmAll, open level)
```

**Division of labour (Rust vs Python):** Rust owns the *hot path* — sim tick, obs extraction,
reward, action packing, eval recording — exposed as a raw `RawEnv` (`reset/step/observe/record`)
returning contiguous `f32` arrays. Python owns the *ecosystem glue* — the PettingZoo
`ParallelEnv` and the Gymnasium frozen-opponent wrapper are thin Python over `RawEnv`, so the
whole SB3/PettingZoo/Gymnasium battery works out of the box while nothing perf-critical crosses
the boundary per scalar.

**Workspace-membership caveat (→ T0):** a PyO3 `cdylib` with `extension-module` can fail to
link under a plain `cargo test --workspace` (missing Python symbols at test-link). T0 decides:
either (a) gate `extension-module` behind an off-by-default feature that maturin enables, or
(b) `[workspace] exclude`. Prefer (a) so the crate's `rlib` unit tests run in the workspace.

**Global constraints (inherited from the step-2/3/4 plans + Python tooling):**
- **No new dependencies in `sim`/`render`/`scenario`/`sim-core`/`assets`.** The env only
  *reads* them; it adds no upstream deps.
- **No C++ changes**, no oracle-dumper changes. The RL track is pure Rust+Python, downstream.
- **`sim` crate untouched** — the harness is a consumer. If a needed `WormState` field is
  `pub` it is readable already (all obs/reward fields verified `pub` in §1.3); no sim edits.
- **Determinism preserved:** fixed→float one-directional, policy RNG separate from sim RNG.
- **Python tooling:** a project-local `venv` + `maturin develop` (editable extension build).
  No system-Python pollution. Pin versions in T0.

---

## 7. `InputSource::Agent` — adjudication (OUT of harness scope)

June listed "an `InputSource::Agent` arm (live-watching + play against the agent)" as a
possible slice. **Adjudication: it is a SEPARATE, deferred `game` slice, NOT part of the RL
harness — and it is not even needed for "watch the agent play."**

Reasoning:
- **Watching the agent is already free via eval-recording (§1.5).** The trainer runs an eval
  episode, taps `(w0,w1)` per tick, `with_recorded_inputs` + `to_text` → a scenario file,
  and `cargo run -p game -- --replay <file>` plays it in the real Bevy window through the
  existing `Scripted`/`Mode::Replay` path. Zero new engine code. This is the recommended
  "see what it learned" loop and is **in harness scope from day one**.
- **Real-time `InputSource::Agent`** (the policy driving a worm *live* in the Bevy app, e.g.
  human-vs-bot) is a genuinely different concern: it needs *inference inside the app* — either
  an exported policy run in-process (a Rust ONNX/`tract`-style runtime, or a Rust-native
  `burn`/`candle` reimplementation of the trained net) or an FFI hop to Python at frame time.
  That pulls a model-runtime dependency into `game` and couples the Bevy loop to a policy
  format. It belongs to its own future slice, decided when a trained policy actually exists
  and the "ship a learned `WormAI`" question (June's open question) is answered.

So: **eval-recording (Scripted/--replay) is the harness's watch path; `InputSource::Agent` is
deferred and tracked, not built here.**

---

## 8. Performance & FFI (T0 research question)

- **Throughput is the whole game.** Rust + integer sim + no rendering makes a tick cheap; the
  risk is **per-`step` FFI + obs-serialization overhead** across the Python boundary.
- **T0 measures:** headless ticks/sec for a single env (pure Rust loop) and the same through
  one PyO3 `step` call, to quantify FFI overhead per step.
- **Batching plan (if FFI dominates):** expose a **batched `step`** that advances N vectorized
  envs per FFI call and returns a single contiguous `(N, obs_dim)` `f32` array — amortizing the
  boundary crossing. Keep the obs extractor Rust-side so only compact float arrays cross.
  Build single-env first, measure, add batched `VecEnv` only if the numbers demand it.
- **Local-first (unchanged from June):** target the author's M2 Max — CPU-bound Rust envs
  across cores, small state-based net on CPU or PyTorch MPS. No CUDA/big-GPU dependency for the
  first milestone.

---

## 9. Milestone & risks

**Milestone (the plan's gate):** an SB3-PPO agent trains against a **random/scripted opponent**
via the Gymnasium wrapper and its **reward curve clearly rises**, AND one **eval recording
replays in the real window** (`--replay`). Small, objective, reproducible; exercises every part
(reset/obs/action/reward/step/record) without self-play or terrain complexity yet.

**Top-3 risks:**
1. **PyO3/maturin workspace-link friction** — `extension-module` breaking `cargo test
   --workspace`, venv/maturin-develop toolchain setup, version skew across
   pyo3/maturin/pettingzoo/gymnasium/SB3/torch-MPS. → T0 pins versions and decides the
   membership pattern before any env code.
2. **FFI per-step overhead swamping the cheap tick** — a Python `step` that costs more than the
   sim tick kills throughput. → T0 benchmarks; §8 batched-VecEnv is the escape hatch.
3. **Reward shaping induces degenerate behavior** (chip-damage farming, camping, ScalesOfJustice
   gaming) → start KillEmAll, keep shaping light + annealed, watch high-reward eval replays for
   cheese (the recording path makes this a first-class debugging tool).

**T0 research questions (all answered before env code):**
- Pin exact versions: `pyo3`, `maturin`, `pettingzoo`, `gymnasium`, `stable-baselines3`,
  `torch` (MPS). Confirm PettingZoo `ParallelEnv` + Gymnasium wrapper API shapes against
  current docs (the exploration's assumed APIs are unverified).
- Benchmark: headless ticks/sec single-env (Rust) and per-`step` FFI overhead (PyO3). Decide
  single-env vs batched-VecEnv for the first milestone.
- Decide workspace membership: `extension-module` feature-gate vs `[workspace] exclude`, such
  that `cargo test --workspace` stays green.
- Re-verify (cheap, code-read): side-channel no-sink reasoning under a long headless run (§1.7)
  — confirm `sound_events`/`shake_events` peak is one-tick-bounded and `flash` is a scalar cell.

---

*Design only. Writes exclusively under `docs/superpowers/`. No code, no commits, no git-state
changes. `rust/` and `src/` inspected read-only; the two ocommitted step-5 docs untouched.*
