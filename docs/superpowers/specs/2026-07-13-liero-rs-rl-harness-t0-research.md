# Liero-rs — RL harness T0 research note (versions + FFI bench + workspace + no-sink)

Status: **RESEARCH — T0 findings, answers the design's open questions** · 2026-07-13
Plan: `docs/superpowers/plans/2026-07-13-liero-rs-rl-harness-plan.md` (task T0)
Design: `docs/superpowers/specs/2026-07-13-liero-rs-rl-harness-pettingzoo-design.md`

Environment of record: macOS 15 (Darwin 25.5) · Apple Silicon **M2 Max** (arm64) ·
Rust **1.97.0** (2026-07-07) · CPython **3.13.1** · project-local venv at
`rust/liero-env/.venv` (gitignored). All numbers below are from a `maturin develop
--release` (optimized) build of a throwaway PyO3 spike, since discarded per the plan
("no env code yet"); the pins live in `rust/liero-env/constraints.txt`.

---

## 1. Version pins + API verification

### 1.1 Pinned versions (installed and exercised, not guessed)

| Package | Pin | Notes |
|---|---|---|
| `stable-baselines3` | **2.9.0** | PPO; consumes `MultiBinary` natively (below) |
| `pettingzoo` | **1.26.1** | `ParallelEnv` core API + `parallel_api_test` |
| `gymnasium` | **1.3.0** | `Box`/`MultiBinary` spaces + `check_env` |
| `torch` | **2.13.0** | arm64 wheel, **MPS built + available** (no CUDA) |
| `numpy` | **2.5.1** | obs array bridge (T3) |
| `maturin` | **1.14.1** | build backend |
| `pyo3` (Rust) | **0.29** | `extension-module` gated OFF by default; `abi3-py39` |
| `numpy` (Rust) | **0.29** | PyO3-0.29-compatible; for the T3 obs bridge (not yet a dep) |

Transitive pins (cloudpickle 3.1.2, Farama-Notifications 0.0.6, filelock 3.29.7,
networkx 3.6.1, sympy 1.14.0, typing_extensions 4.16.0) are captured in
`constraints.txt` for reproducibility.

`torch.backends.mps.is_available()` and `.is_built()` are both **True** — the design's
"PyTorch MPS on the M2 Max, no big-GPU dependency" holds with the stock wheel.

### 1.2 API assumptions — verified against the pinned packages

The June exploration's API shapes were unverified. Checked by introspection against
the versions above:

- **PettingZoo `ParallelEnv`** (`pettingzoo.utils.env.ParallelEnv`):
  - `reset(self, seed=None, options=None) -> (dict[AgentID, ObsType], dict[AgentID, dict])`
  - `step(self, actions: dict[AgentID, ActionType]) -> (obs, rewards, terminated,
    truncated, infos)` — the **5-tuple of per-agent dicts**. Matches the design's §5
    episode contract exactly.
  - **`observation_space(agent)` and `action_space(agent)` are METHODS** (take an
    `AgentID`), not attributes. Minor deviation from the design's prose ("`action_space
    = MultiBinary(7)`"): T4 must implement them as methods (the common idiom is
    `@functools.lru_cache` returning the per-agent space, plus the
    `observation_spaces`/`action_spaces` dict mirrors). No shape change — just where the
    space object hangs.
  - `parallel_api_test` (`pettingzoo.test.parallel_api_test`) imports — the T4 gate.
- **SB3 + `MultiBinary(7)`**: `stable_baselines3.common.distributions.
  make_proba_distribution(MultiBinary(7))` returns a **`BernoulliDistribution`** — i.e.
  PPO's policy head handles `MultiBinary` natively, **no action re-encoding** in the
  Gymnasium wrapper (design §2 confirmed).
- **Gymnasium `check_env`** (`gymnasium.utils.env_checker.check_env`) imports — the T4
  single-agent-wrapper gate. `MultiBinary(7).shape == (7,)`, `Box(float32)` available.

**No blocking API deviations.** The one adjustment: PettingZoo spaces are per-agent
**methods** (T4 note), not class attributes.

---

## 2. FFI benchmark (single env, release build)

Throwaway PyO3 spike: `Bench(tc_root, scenario_text)` loads the committed
`render_slice3b_blood` fixture via `scenario::load`, then drives
`SimState::process_frame(&[cs0, cs1])` with the scenario's per-tick 7-bit inputs
(cycled). Three paths timed over 2,000,000 ticks each on the M2 Max:

| Path | ns / tick | M ticks/s | vs pure-Rust |
|---|---|---|---|
| noop FFI round-trip (no work) | 24.2 ns/**call** | 41.4 M calls/s | — |
| **pure-Rust tick ceiling** (no boundary/tick) | 1183.7 | 0.845 | 1.00× |
| **per-step FFI** (1 tick / `step()` call) | 1215.0 | 0.823 | 0.97× |
| batched FFI (4 ticks/call) | 1200.9 | 0.833 | 0.99× |
| batched FFI (8 / 16 / 64 / 256) | ~1185 | ~0.845 | ~1.00× |

**FFI overhead per step ≈ 31 ns ≈ 2.6 % of one tick.** A bare boundary crossing is
~24 ns; a full Liero tick (2 worms, real physics/weapons) is ~**1.18 µs**, i.e. ~40×
the crossing.

### 2.1 Batching decision — **single-env, per-step; NO batched VecEnv for the milestone**

The tick dominates the boundary by ~40×, so amortizing the crossing (design §8's
batched-`step`/`VecEnv` escape hatch) recovers at most ~2.6 % — it does **not** demand
a batched API. Concretely:

- **T3/T4 build the single-env per-step `RawEnv.step()`** (design §6 division of
  labour). This is Risk #2 (FFI swamping the tick) **retired by measurement**.
- Throughput scaling for training is **process-level parallelism across cores**
  (SB3 `SubprocVecEnv` / N independent envs), not in-FFI tick batching. At ~0.85 M
  ticks/s/core the M2 Max's performance cores give several M ticks/s — ample for the
  reward-rise milestone.
- Caveat: this measures the **tick only**; T2/T3 add a ~30–40-scalar `f32` obs
  extraction (Rust-side, cheap) plus one small contiguous array returned per step. That
  copy is tiny next to the 1.18 µs tick, and keeping the extractor Rust-side (design §8)
  means only a compact float array crosses. Re-measure once obs lands if paranoid, but
  the boundary is provably not the bottleneck. Batched `VecEnv` stays **deferred**
  (plan's deferral list) unless a later profile contradicts this.

---

## 3. Workspace membership decision — **feature-gate `extension-module` (option a)**

Tested empirically by adding `liero-env` (crate-type `["cdylib","rlib"]`, `pyo3 = { ...
features = ["abi3-py39"] }`, with `extension-module` behind an **off-by-default cargo
feature**) as a real workspace member and running the suites:

- **`cargo test --workspace -p liero-env` (feature OFF, default): GREEN.** pyo3 links
  libpython, so the crate's `rlib` unit test (a determinism check over the fixture)
  compiles and runs inside the workspace.
- **`cargo test -p liero-env --features extension-module`: LINK FAILURE** — even on
  macOS/arm64. The test executable pulls `libpyo3.rlib` whose symbols
  (`_Py_IncRef`, `_Py_InitializeEx`, `_Py_IsInitialized`, `__Py_NoneStruct`, …) are
  deliberately left unresolved by `extension-module` (the extension is meant to borrow
  them from the host interpreter at import time). `ld: symbol(s) not found for
  architecture arm64`. This is exactly the design §6 "workspace-link caveat", and it
  reproduces here — so the gate is not hypothetical.
- **`cargo test --workspace --exclude game` with `liero-env` as a member (feature OFF):
  GREEN** across all 40+ test binaries.

**Decision (design's preferred option a):** `extension-module` is a cargo feature that
is **OFF by default** and **maturin turns ON** (`[tool.maturin] features =
["extension-module"]` in `pyproject.toml`). Then:

- plain `cargo test --workspace` builds `liero-env` **without** `extension-module`, links
  libpython, and runs its `rlib` unit tests — no `[workspace] exclude` needed, so the
  env's pure-Rust logic (env core, obs, action, reward) is workspace-tested like every
  other crate;
- `maturin develop`/`build` flips the feature on to produce the importable extension
  (verified: `maturin develop --release` built `liero_env-0.1.0-cp39-abi3-
  macosx_11_0_arm64.whl` and `import liero_env` works).

`abi3-py39` is pinned so the wheel is forward-compatible across CPython ≥ 3.9 (one wheel,
no per-minor rebuild).

### 3.1 Dependency-tree note (minor design correction)

Design §6/§1.2 say `liero-env` "depends only on `sim`/`scenario`/`sim-core`/`assets`,
**no `render`**". In fact **`scenario` depends on `render`** (`scenario/Cargo.toml`), so
`liero-env` transitively pulls `render` in. This does **not** violate the load-bearing
constraint: `render` is **Bevy-free** (its deps are only `sim-core`/`assets`/`sim` —
`render/Cargo.toml`), so the env still links **no Bevy / no GPU / no wgpu**. Only `game`
pulls Bevy. So the "thin, Bevy-free consumer" property holds; the note is just that the
transitive set is `{sim, scenario, render, sim-core, assets}`, not `{sim, scenario,
sim-core, assets}`. No action needed beyond recording it.

---

## 4. Side-channel no-sink re-verification (code-read)

Re-verified the design §1.7 / plan claim that a long headless tick loop that never
drains the side-channels stays **bounded** — each channel is overwritten every tick.
File:line evidence in the worktree:

- **`sound_events`** (`SimState.sound_events: Vec<SoundEvent>`, `sim/src/state.rs:1234`):
  `process_frame` calls `sound::begin_frame(..)` at the **top**
  (`state.rs:1497`), which `FRAME.clear()`s the thread-local buffer
  (`sim/src/sound.rs` `begin_frame`); at the **bottom** `*sound_events =
  sound::take_frame()` (`state.rs:2263`) `std::mem::take`s the buffer and **replaces**
  the field wholesale. Peak = one tick's events (a handful), reset every tick. No sink,
  no growth.
- **`shake_events`** (`SimState.shake_events: Vec<ShakeEvent>`, `state.rs:1245`):
  symmetric — `shake::begin_frame()` clears at the top (`state.rs:1508`), `*shake_events
  = shake::take_frame()` **replaces** at the bottom (`state.rs:2270`). `drain_shake_events`
  (`state.rs:1426`) exists for the `game` layer but is **not** required by a headless
  consumer; even undrained, the next tick's `take_frame` overwrites. Bounded.
- **`screen_flash`** (`SimState.screen_flash: i32`, `state.rs:919`): a **single scalar
  cell**, not a collection. `flash::begin_frame(*screen_flash)` seeds the thread-local at
  the top (`state.rs:1634`, after the `> 0` decrement at `:1626-1627`), `raise()` folds a
  `max`, `*screen_flash = flash::take_frame()` reads it back (`state.rs:2276`). It only
  ever holds one `i32`; it cannot grow.

**Determinism-inertness confirmed too:** none of the three is walked by the hashes —
`hash.rs` and `wide_checksum.rs` reference these fields **only** in struct-construction
initializers (`hash.rs:218,281,282`; `wide_checksum.rs:155,218,219`), never in the fold;
and the in-tree tests `sound_events_and_hooks_are_hash_inert`
(`state.rs:3944`), `screen_flash_is_hash_inert` (`state.rs:4007`), and
`shake_events_are_hash_inert` (`state.rs:4041`) assert populate/clear does not move the
master or component hashes. So a headless RL harness that simply **ignores** all three
incurs no unbounded growth and no determinism effect. **No sink needed** — as designed.

---

## 5. Deliverables / outcomes

- **Pins:** `rust/liero-env/constraints.txt` (this commit) + Rust pins recorded above.
- **venv:** `rust/liero-env/.venv` (gitignored via `rust/liero-env/.gitignore`), with the
  full stack installed and the API assumptions exercised.
- **Spike:** discarded (not committed), per the plan. Numbers captured in §2.
- **Suites (baseline, committed state — no `liero-env` member):**
  `cargo test --workspace --exclude game` GREEN; `cargo test -p game` GREEN; goldens
  untouched (no `sim`/`scenario`/`render` source edited).
- **Go/no-go for T1:** GREEN. Single-env per-step; `extension-module` feature-gate; env
  transitively links Bevy-free `render` (fine); side-channels need no sink.
