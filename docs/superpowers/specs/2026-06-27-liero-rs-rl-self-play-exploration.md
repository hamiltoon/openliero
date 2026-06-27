# Liero-rs — reinforcement learning / self-play AI (EXPLORATION)

Status: **EXPLORATION — forward-looking, NOT a committed plan, NOT a spec** · 2026-06-27
Part of: `2026-06-26-liero-rs-roadmap.md` (the "deliberately deferred / new capabilities" direction)
Builds on: `2026-06-26-liero-rs-steps2-5-preliminary-breakdown.md` (step 2 sim, step 4 input model)
and `2026-06-26-liero-rs-interactive-iteration-exploration.md` (headless + replay + checksum reuse)

> **Read this first.** This document explores *how we could use AI — reinforcement
> learning and self-play — to learn to play Liero* over many iterations, and what
> the parts of such an agent would be, grounded in *this* engine. It is an
> **exploration**: it maps the design space, gives concrete Liero-specific shapes,
> recommends a pragmatic starting point, and is honest about difficulty. It
> **commits nothing**, writes no code, and is independent of (and later than) the
> core roadmap. The roadmap lists "new capabilities" as possible only *after* a
> deterministic core exists; this is one of those capabilities. Exact NN sizes,
> hyperparameters, and library versions here are illustrative and must be
> re-validated when/if the work is actually undertaken.

---

## TL;DR

- **Liero-rs is an unusually good RL substrate** *because of the crown jewel*: a
  deterministic, fixed-point, **headless** sim (step 2) is exactly a fast,
  reproducible, massively-parallel RL environment — and it needs **no rendering
  (step 3) and no netcode (step 5)**. RL becomes feasible the moment step 2 (the
  deterministic headless tick) plus the step-4 input/control model exist.
- The game is **naturally self-play**: symmetric 1v1, an existing built-in AI to
  use as a free sparring partner / imitation target, and a packed 7-bit
  `ControlState` that is already a clean, tiny action space.
- **The parts of the AI** (the user's core question): an *environment/episode API*
  (gym-style `step(action) -> (obs, reward, done)` over the headless sim), an
  *observation space* (worm/opponent kinematics + weapon/ammo + a local terrain
  crop from the material map), an *action space* (the `ControlState` bits, or the
  engine's existing reduced 0..56 input alphabet), a *reward* (damage/kills/
  survival, sparse vs shaped), a *policy/value network* (small MLP, or CNN if the
  terrain crop is used), an *algorithm* (**PPO** as the baseline), *self-play +
  curriculum* (start vs the built-in AI, graduate to an opponent league), a
  *vectorized training loop* (thousands of headless matches), and *evaluation*
  (win-rate vs built-in AI and past checkpoints, Elo over a league).
- **Recommended implementation path:** keep the sim authoritative in Rust and
  expose a thin headless API; **train in Python via PyO3 bindings + a mature RL
  stack (CleanRL / Stable-Baselines3, PyTorch).** This buys the rich RL ecosystem
  at the cost of one well-contained language boundary. Rust-native (`burn`/`candle`)
  is a viable "one language, fastest" alternative to revisit once the env is proven.
- **Roadmap fit:** earliest feasible point is **right after step 2 + the step-4
  control model**, before/independent of rendering and netcode. The sim needs only
  to *expose* (not change) four hooks: `step(ControlState)`, `reset(seed, scenario)`,
  a `state -> observation` extractor, and a `reward/termination` readout. RL is a
  **consumer** of the sim, never a modification of it.
- **Minimal first milestone:** *a PPO agent that beats the built-in `DumbLieroAI`
  1v1 on one fixed level with one weapon.* Stepping stones before that: imitation-
  learn the built-in AI, and single-agent target practice before any self-play.

---

## 1. Why Liero-rs is an unusually good RL substrate

RL's practical cost is almost always **environment throughput and reproducibility**.
Liero-rs, by construction of the roadmap, is strong on exactly those axes.

- **Deterministic, by design and proven.** The whole project premise is a
  bit-exact tick: `Game::ProcessFrame` is a pure function of (state, input), RNG is
  part of state (`game.rand`), no floats in the sim, fixed iteration order. The
  per-tick checksum `HashGameState` (`src/game/stateHash.hpp`) already hashes
  worm `pos/vel/aiming_angle/health/lives/kills`, weapon ammo, ninjarope, terrain
  `material_id`, `rand.last`, and `cycles`. For RL this means **every episode is
  exactly reproducible from (seed, scenario, action stream)** — flaky-env debugging,
  the bane of RL, mostly disappears. A divergent training run can be replayed
  tick-for-tick.
- **Headless and rendering-free.** Step 2 is the sim in ECS with *no* rendering
  (step 3) and *no* audio. Training never needs a GPU for graphics, a window, or
  the wasm path. This is the key insight: **RL consumes the same headless sim the
  determinism oracle already drives** (the interactive-iteration doc's "run N ticks,
  checksum" harness is the same machinery an RL rollout uses).
- **Fast and cheap to parallelize.** Rust + integer math + no rendering → a single
  tick is very cheap; a 1v1 round is short. There is no shared mutable global state
  that prevents running **many independent envs** in parallel processes/threads —
  the natural shape for vectorized RL (thousands of concurrent matches).
- **Reproducible episodes for free.** `GameSnapshot` / `SaveWormSimState` /
  `RestoreWormSimState` (`src/game/serialization/fast_snapshot.hpp`) already define
  the complete sim-state set and can save/restore it. That is `reset()`, fixed
  starting positions, and even mid-episode branching — and it is the *same* state
  inventory step 5 will roll back. RL gets episode reset and exact restart from the
  rollback machinery.
- **Natural self-play.** Liero 1v1 is **symmetric and zero-sum-ish** (your kill is
  their death): the ideal setting for self-play, where the opponent improves as you
  do and provides an automatic curriculum.

### The existing hand-tuned AI — what it is and its ceiling

The engine ships **two** AIs, both useful as baselines/oracles:

- **`DumbLieroAI`** (`src/game/worm.cpp:477`) — the classic Liero AI. It finds the
  nearest worm, computes an effective firing distance from the current weapon's
  speed/`time_to_explo`, snaps its aim toward the target via the `cossin_table`,
  and then **toggles each control bit with a per-control probability** drawn from
  `common.ai_params.k[on/off][control]` (e.g. `rand(k[kFire][kFire]) == 0` flips
  fire). It is a **stochastic reactive controller with hand-tuned constants** — no
  planning, no learning, no terrain reasoning beyond line-of-sight aiming. Its
  ceiling is fixed: it cannot discover tactics, use cover, lead targets with
  lobbed weapons, or manage ammo. It is a *great free sparring partner and
  imitation target* precisely because it is cheap and beatable.
- **`predictive_ai` (`FollowAI` / `SimpleAI`, `src/game/ai/predictive_ai.{hpp,cpp}`)**
  — a much more sophisticated *search* AI that already exploits the deterministic
  sim: it builds a Dijkstra navigation level, maintains a presence/damage grid
  (`AiContext`), evaluates **populations of candidate plans** (`CandPlan`) by
  forward-simulating them, and even carries a **learned transition model**
  (`TransModel`, a probability table over an abstracted input alphabet indexed by an
  `InputContext`). This is essentially **model-based planning with a learned prior**
  — and it is the strongest existing evidence that "use the deterministic sim as a
  forward model for AI" *already works here*. An RL/self-play agent is the natural
  successor: amortize that per-frame search into a trained policy network, and let
  self-play (not hand-tuned weights) discover the value function.

Two facts from `predictive_ai.hpp` are directly reusable for RL design (see §2):

- **A ready-made reduced action alphabet.** `InputState` already collapses the raw
  controls into **0..56 discrete actions** — `kMoveJumpFire` (48 combos),
  `kChangeWeapon` (5), `kRopeUpDown` (3) — via `Compose`/`Decompose`. This is a
  hand-validated, game-meaningful action discretization we can borrow.
- **A ready-made context featurization.** `InputContext::Pack()` encodes
  `(ninjarope_out, facing_enemy, current_state)` into `kSize = 56*2*2 = 224`
  contexts. It hints at which discrete features matter most for action selection.

---

## 2. The anatomy of such an AI — the parts

This is the core of the user's question: *what parts make up an RL agent for Liero,
and how do they fit together?* Each part below is grounded in concrete engine
structures.

```
                 ┌──────────────────────────────────────────────────────┐
                 │                  TRAINING (Python or Rust)             │
                 │                                                        │
   obs ──────────┼──►  Policy/Value NN  ──► action (ControlState/InputState)
    ▲            │           ▲                          │                 │
    │            │           │ gradients                │                 │
    │            │     PPO (rollout buffer, GAE,        │                 │
    │            │      clipped objective)              │                 │
    │            │           ▲                          │                 │
    │  reward,done│          │ returns/advantages       │                 │
    └────────────┼──────────┴──────────────────────────┼─────────────────┘
                 │                                       ▼
        ┌────────┴───────────────────────────────────────────────┐
        │  ENV WRAPPER  step(action) -> (obs, reward, done)        │
        │  reset(seed,scenario)   ── over N parallel envs ──       │
        └────────┬───────────────────────────────────────────────┘
                 ▼
        ┌────────────────────────────────────────────────────────┐
        │  HEADLESS DETERMINISTIC SIM (step 2)                     │
        │  ProcessFrame(ControlState) · GameSnapshot reset/restore │
        │  HashGameState (reproducibility) · material_id (terrain) │
        └────────────────────────────────────────────────────────┘
```

### 2.1 Environment / episode API (the gym wrapper)

A thin, **read-mostly** wrapper over the headless sim implementing the standard
contract:

- `reset(seed, scenario) -> obs`: install a deterministic starting state. Two
  realizations already exist in-engine — fresh spawn (`BeginRespawn`, RNG-driven
  spawn rect `WormSpawnRect*`) or restore a fixed `GameSnapshot`
  (`RestoreWormSimState`). Use snapshot-restore for *exactly fixed* starts (best for
  reproducible eval), and seeded fresh spawns for *training variety*.
- `step(action) -> (obs, reward, done, info)`: map the action to a `ControlState`,
  call one (or `frame_skip`) `ProcessFrame`, then read the new state out.
- **Episode = one round.** Terminate on death (`health <= 0`, see `DoDamageDirect`),
  on round win/`lives` exhausted, or on a `max_steps` cap (essential — Liero rounds
  can stall; a step budget bounds episode length and forces engagement).
- **Seeding.** The sim RNG (`game.rand`) is part of state and restorable; the env
  seeds it explicitly so an episode is `f(seed, scenario, actions)`. The agent's own
  exploration RNG is separate from the sim RNG (never let policy sampling perturb the
  sim's RNG stream — that would break the determinism contract).

### 2.2 Observation space

Recommendation: **state-based, not pixels.** The engine exposes clean structured
state (this is rare and valuable); rendering (step 3) isn't even built yet at the
feasible point. Pixels would force the GPU/render path RL otherwise avoids and add a
hard perception problem for no benefit.

Per-agent observation, drawn from `WormSimState` / `HashGameState` fields:

- **Self kinematics:** `pos` (x,y), `vel` (x,y), `aiming_angle`, `aiming_speed`,
  `direction`, `able_to_jump`/`able_to_dig`, `fire_cone`, ninjarope `out`/`pos`.
- **Weapon/ammo state:** `current_weapon`, and per-weapon `ammo`, `delay_left`,
  `loading_left` (from `WormWeapon`) — so the agent learns reload/ammo discipline.
- **Opponent state:** the same kinematics for the nearest/other worm, plus a
  `visible` flag and a relative vector (`opp.pos - self.pos`) and distance — note
  `DumbLieroAI` and `InputContext.facing_enemy` already show relative geometry is the
  key signal.
- **Nearby terrain — a local crop of the material map.** The full level
  `material_id[width*height]` (the destructible terrain) is the spatial context.
  Feed the policy an **egocentric crop** (e.g. an NxN window of material classes —
  dirt / rock / background — centered on the worm, optionally downsampled) so the
  agent can learn to use cover, dig, and avoid walls. This is the one part that may
  warrant a small CNN (§2.5).
- **Projectiles / objects:** positions/velocities of nearby `WObject`/`NObject`
  (incoming fire, grenades). Variable count → either a fixed-K "nearest objects"
  list or rasterize them into an extra channel of the terrain crop.

**Fixed-point → float featurization is one-directional.** The sim stays in 16.16
`fixed`/`fixedvec`; the *extractor* converts to normalized `f32` for the network
only. Float never flows back into the sim (the same discipline step 2/3 enforce for
rendering). Normalize positions by level size, velocities/angles to roughly unit
scale, healths to [0,1].

### 2.3 Action space

The sim consumes a **7-bit `ControlState`** (`worm.hpp:150`): bits for
`kUp, kDown, kLeft, kRight, kFire, kChange, kJump` (`Pack()`/`Unpack()`, masked to
`0x7f`). Three viable encodings, increasing structure:

- **MultiDiscrete(7 binary buttons)** — the most expressive, closest to a human;
  the policy outputs 7 independent Bernoulli logits → assemble a `ControlState`.
  128 raw combinations, many nonsensical (left+right), but the policy learns to
  avoid those. Best long-term.
- **Discrete(128)** — one categorical over all bit combos. Simple, but mixes the
  meaningful and the absurd.
- **Discrete(57) — the engine's own `InputState` alphabet** (`predictive_ai.hpp`):
  `kMoveJumpFire`/`kChangeWeapon`/`kRopeUpDown`, the 0..56 space the existing search
  AI already uses. **Strongly recommended as the starting action space** — it is
  game-validated, compact (fast to learn), and bridges directly to the existing AI
  for imitation. Graduate to MultiDiscrete(7) later if the alphabet limits tactics.
- **Frame-skip / action repeat:** Liero ticks fast; holding a control for k ticks
  (e.g. k=2–4) shortens the effective horizon and speeds learning. Aiming changes
  per tick (`aiming_speed`), so don't skip so coarsely that aim overshoots — a tunable.

### 2.4 Reward design

Grounded in the damage/kill model (`Game::DoDamage`/`DoDamageDirect`, `game.cpp:546`):

- **Sparse, true objective:** +1 on kill (opponent `health <= 0`, you are
  `last_killed_by_idx`), −1 on own death; episode = round. Correct but hard to learn
  (long credit-assignment, rare signal).
- **Shaped (recommended to bootstrap):** reward **damage dealt** (the per-hit
  `amount` in `DoDamage`) and penalize **damage taken**, scaled smaller than the
  kill bonus; small **survival** bonus; small **ammo-efficiency**/wasted-shot
  penalty (fire with no hit). Optionally tiny "face/approach enemy" shaping early,
  removed later (shaping can teach degenerate habits — keep it light and anneal it
  out).
- **The credit-assignment challenge is the real difficulty.** A grenade thrown now
  explodes seconds later; a dig now enables an escape later. Damage-based shaping
  shortens the gap; PPO's GAE (γ, λ) propagates credit; frame-skip shortens the
  horizon. Be honest: reward shaping is where most of the tuning pain will live
  (§5).
- **Game-mode coupling:** rewards must match the mode. `DoHealing`/`ScalesOfJustice`
  (`kGmScalesOfJustice`) *redistributes* health on self-damage — naive
  "damage taken is bad" can be gamed. Start with **Kill-em-all** (simplest,
  cleanest signal); generalize modes later.

### 2.5 Policy / value model

Small networks suffice — this is not ImageNet.

- **If state-only obs (no terrain crop):** a **2–3 layer MLP** (e.g. 256-wide) with
  separate policy and value heads (shared trunk). Input ≈ a few dozen scalars (self
  + opponent kinematics + weapon/ammo + relative geometry); output = action logits
  (57 for the `InputState` alphabet, or 7 Bernoulli for MultiDiscrete) + a scalar
  value.
- **If terrain crop included:** a **small CNN** (2–3 conv layers) over the NxN
  material crop (+ object channel), flattened and concatenated with the scalar
  features into the MLP trunk. Keep the crop small (e.g. 32–64 px) and downsampled
  to keep it cheap.
- **Recurrence:** likely unnecessary if obs are Markov enough (they nearly are,
  given full state access); add a small GRU only if partial observability (off-screen
  opponent) hurts.

### 2.6 Algorithm

- **PPO is the recommended baseline.** On-policy, robust to hyperparameters,
  excellent with massively parallel envs (which we have cheaply), and the de-facto
  standard for self-play game agents. It tolerates the non-stationarity of a moving
  self-play opponent better than value-based methods, and the clipped objective is
  forgiving. CleanRL/SB3 PPO is a near-drop-in for a gym env.
- **DQN / off-policy** (with the Discrete(57) action space) is *possible* and
  sample-efficient via replay, but is more brittle under self-play non-stationarity
  and needs a replay buffer; treat as a later comparison, not the start.
- **Actor-critic family generally:** PPO (A2C's robust descendant) is the sweet
  spot. The determinism dividend is orthogonal to the algorithm — it makes *any*
  choice debuggable.
- **Why PPO fits *here* specifically:** cheap parallel envs (PPO loves throughput),
  symmetric self-play (PPO handles the non-stationary opponent), and a small
  discrete/multi-discrete action space (clean categorical/Bernoulli heads).

### 2.7 Self-play & curriculum

- **Start with a fixed opponent: the built-in `DumbLieroAI`.** Free, deterministic-
  enough, beatable — a perfect first sparring partner and an unambiguous evaluation
  target. (Its stochastic toggles draw from the sim RNG; keep that accounted for in
  reproducibility.)
- **Graduate to self-play** once the agent reliably beats `DumbLieroAI`: the
  opponent becomes a copy of (a past checkpoint of) the policy. Symmetric 1v1 means
  one network can play both sides.
- **Opponent pool / league** to avoid the classic self-play failure of cyclic
  forgetting (beating only your latest self): sample opponents from a pool of past
  checkpoints (and keep `DumbLieroAI`/`FollowAI` in the pool as anchors). A
  lightweight AlphaStar-style league is the mature version; a frozen-snapshot pool is
  the cheap, effective start.
- **Curriculum (start simple, widen):** one weapon → a small weapon set → full
  arsenal; one open level → levels with cover/tunnels; fixed spawns → random spawns;
  fixed opponent → league. Each axis can be widened independently and regression-
  tested (determinism makes "did widening break the easy case?" a cheap check).

### 2.8 Training loop & scale

- **Vectorized parallel envs:** run N headless sims (processes or threads) and batch
  their observations through the policy. Throughput is the lever; Rust + no rendering
  makes each env cheap, so N can be large on one machine.
- **Rollout → update:** collect fixed-length rollouts across the vector, compute GAE
  advantages, run PPO epochs, repeat for many iterations (millions of env steps).
- **Checkpoints:** snapshot the policy periodically (for the opponent pool and for
  regression/eval). Log win-rate, episode length, reward components.
- **The determinism dividend, restated:** any training episode (incl. a
  catastrophic one) can be **replayed exactly** from (seed, scenario, action stream)
  via the same `.lrp`-style replay + `HashGameState` checksum the roadmap already
  plans. "Why did the agent suicide at tick 812?" is answerable, not guesswork —
  a luxury most RL setups lack.

### 2.9 Evaluation

- **Win-rate vs the built-in AI** (`DumbLieroAI`, and later `FollowAI`) on a fixed
  eval scenario set — the primary, interpretable metric and the first-milestone gate.
- **Win-rate vs past checkpoints** + **Elo over the league** — measures *real*
  progress under self-play (raw self-play reward is misleading because the opponent
  moves).
- **Policy regression:** because eval scenarios are deterministic, a *fixed eval
  suite* (seeds + opponents) gives a stable score; a checkpoint's score is
  reproducible, so "did this change make the agent worse?" is objective — the same
  posture the interactive-iteration doc takes for sim regression, applied to policies.
- **Behavioral sanity / qualitative:** once rendering (step 3) exists, replay a
  match to video (`videotool` analogue) to *watch* what the agent learned — but
  rendering is not required for training or for the quantitative gate.

---

## 3. Implementation paths & trade-offs

The invariant across all paths: **the sim stays authoritative and unmodified in
Rust; RL is a consumer.** Determinism must hold across whatever boundary is chosen
(the action stream in and the state out are the only crossings; the sim's
fixed-point/RNG never leave Rust).

### (a) Rust-native RL (`burn` / `candle`)
- **Pros:** one language, no FFI/serialization cost, the env and the trainer share
  the exact same types, easiest to keep determinism honest, best raw throughput,
  trivial deployment (a single binary; a trained policy could even run *inside* the
  game as a new AI controller, wasm-friendly).
- **Cons:** the RL ecosystem is **much thinner** than Python's — fewer batteries
  (PPO impls, vectorized-env utilities, loggers, hyperparameter tooling), more to
  build/maintain yourself, smaller community to copy from. You'd likely port a PPO
  reference by hand.

### (b) Python via PyO3/FFI bindings to the headless sim  ⟵ **recommended start**
- **Pros:** the **mature RL ecosystem** — Stable-Baselines3 / CleanRL PPO, Gymnasium
  vectorized envs, Weights & Biases logging, the whole research toolchain — works
  out of the box. Fastest path to a *working agent* and to *iterating on reward/
  algorithm*. The sim is exposed as a small `step/reset/obs/reward` extension module
  (PyO3) wrapping the same headless `sim-core`/step-2 tick the oracle uses.
- **Cons:** one language boundary; per-step FFI + observation serialization cost
  (mitigated by **batching** — step many vectorized envs per call, return obs as a
  single contiguous array — and by keeping the obs extractor on the Rust side so only
  compact float arrays cross). A second toolchain (Python) enters the repo.
- **Determinism note:** safe *as long as* the boundary only carries actions in and
  derived observations/rewards out; the sim's fixed-point state and RNG stay entirely
  in Rust. Featurization (fixed→float) happens Rust-side, one-directional.

### (c) Process / socket boundary
- **Pros:** maximal isolation/language-agnosticism; trivial to scale envs across
  machines; clean crash isolation.
- **Cons:** highest per-step latency/overhead (serialization + IPC) — wasteful for a
  tick this cheap; more moving parts. Best reserved for *large-scale distributed*
  training later, not for bring-up.

### Recommendation
**Start with (b): Python + PyO3 bindings over the headless step-2 sim, PPO via
CleanRL/SB3.** It minimizes time-to-first-learning-agent and lets us lean on a
battle-tested RL stack while the *interesting* engine-specific work (obs/action/
reward design) is what we actually iterate on. Keep the binding **thin and batched**,
and the obs extractor in Rust. **Revisit (a) Rust-native** once the env + reward +
baseline are proven and we want a single-binary, in-game-deployable, wasm-friendly
policy (a trained policy as a new `WormAI` alongside `DumbLieroAI`/`FollowAI` is a
genuinely attractive end state). (c) is a later scale-out option, not a starting
point.

---

## 4. Where it fits the roadmap & prerequisites

- **Earliest feasible point: right after step 2 (deterministic headless sim) plus
  the step-4 input/control model** (the `ControlState` mapping and the fixed-timestep
  driver). RL needs the *headless deterministic tick* and the *control model* — it
  does **not** need step 3 (rendering) or step 5 (netcode). So RL is **independent of
  and parallel to** the back half of the roadmap, and slots cleanly under the
  roadmap's "new capabilities become possible after a deterministic core exists."
- **It is a consumer, not a modification.** The sim must only *expose* four hooks,
  all of which already exist in some form on the C++ side and are natural to mirror in
  the Rust sim:
  1. **`step(ControlState) -> ()`** — one deterministic tick (the step-2 tick,
     callable headlessly with injected per-worm controls; the step-4 input model
     defines the `ControlState` injection point).
  2. **`reset(seed, scenario) -> ()`** — seed `rand` and install a start state
     (fresh `BeginRespawn` spawn, or `RestoreWormSimState`/`GameSnapshot` for fixed
     starts). The snapshot machinery from step 5's lineage gives this for free.
  3. **`state -> observation`** — a read-only extractor over `WormSimState` +
     `material_id` (the §2.2 features), fixed→float, one-directional.
  4. **`reward / termination readout`** — read `health`/`lives`/`kills`/
     `last_killed_by_idx` deltas and round-over state (the §2.4 signals).
- **Reuses existing infrastructure** the roadmap already plans: the headless sim and
  per-tick `HashGameState` checksum (oracle/step 2), the `.lrp`-style replay (step 4 /
  interactive-iteration doc), and `GameSnapshot` save/restore (step 5). RL adds a
  wrapper and a trainer; it does **not** touch determinism-critical code.
- **Non-negotiable constraint (same as the rest of the roadmap):** nothing here may
  complicate steps 0–2, and nothing here may introduce float-into-sim or
  RNG-out-of-order regressions. RL lives strictly *outside* the `sim-core` boundary.

---

## 5. Realistic assessment

**Be honest: this is the hard, open-ended kind of work, and "deferred" is correct.**

- **Difficulty.** The *substrate* is unusually favorable (deterministic, headless,
  parallel — that's the easy 30%). The *RL* is the hard 70%: reward shaping that
  doesn't induce degenerate behavior, credit assignment over long horizons (delayed
  explosions, terrain plays), self-play stability (cyclic forgetting, opponent
  collapse), and the usual hyperparameter sensitivity. None of that is made easy by
  determinism — determinism only makes it *debuggable and reproducible*.
- **Compute.** Game-playing PPO agents typically need **millions to tens of millions
  of env steps**. The Rust headless sim makes steps cheap, but this is still hours-
  to-days of training per serious experiment, and self-play multiplies the
  experiment count. Plan for many runs, not one.
- **Reward-shaping pitfalls (call them out up front):** agents will exploit any
  shaping loophole — farming "damage dealt" by self-safe chip damage, camping for a
  survival bonus, ammo-dumping if firing is rewarded directly, or gaming
  `ScalesOfJustice` health redistribution. Mitigations: keep shaping light and
  anneal it toward the sparse true objective; start in Kill-em-all; watch replays of
  "high-reward" episodes for cheese.
- **Self-play pitfalls:** non-transitive strategies and forgetting → use an opponent
  **pool/league** with anchors (`DumbLieroAI`), and measure progress by Elo/win-rate
  vs a *fixed* eval set, never by raw self-play return.

### Minimal first milestone
> **A PPO agent beats the built-in `DumbLieroAI` 1v1, on one fixed level, with one
> weapon, from a fixed spawn, in Kill-em-all mode — win-rate clearly above 50% on a
> deterministic eval suite.**

Small, objective, reproducible (determinism makes the win-rate a stable number), and
it exercises every part (env, obs, action, reward, policy, PPO, eval) without any
self-play, multi-weapon, or terrain complexity yet.

### Smaller stepping stones (de-risk before the milestone)
1. **Build & validate the env wrapper alone** — random/scripted actions, confirm
   `step`/`reset` are deterministic (same seed+actions → identical `HashGameState`
   trace). This proves the substrate before any learning.
2. **Imitation-learn the built-in AI first.** Collect (obs, action) pairs by running
   `DumbLieroAI` (or `FollowAI`) headlessly, train the policy network with supervised
   behavioral cloning to reproduce it. This validates the obs/action/network design
   *and* gives a strong PPO initialization — far cheaper than learning from scratch.
3. **Single-agent target practice before self-play.** A static or scripted target;
   reward = damage dealt. Learns aiming + firing in isolation (the `aiming_angle`/
   `fire_cone` mechanics) before adding a moving, shooting opponent.
4. **Then the milestone (vs `DumbLieroAI`), then self-play + league, then curriculum
   widening** (weapons, levels, spawns, modes).

End state to aspire to (not commit to): a **trained policy that ships as a new
`WormAI`** alongside `DumbLieroAI`/`FollowAI` — a stronger, learned bot — ideally in
Rust-native form so it runs in-engine and in wasm.

---

## Open questions for the controller

- **Worth doing at all, and when?** This is explicitly a deferred "new capability."
  Confirm it stays *after* step 2 + step-4 control model and *never* blocks the core
  roadmap.
- **Python-boundary tolerance.** Is adding a Python toolchain + PyO3 bindings to the
  repo acceptable for the RL ecosystem payoff, or is the "one language" value of
  Rust-native (`burn`/`candle`) strong enough to start there despite the thinner RL
  ecosystem?
- **Action space to commit to first:** the engine's existing `InputState` 0..56
  alphabet (game-validated, fast) vs raw MultiDiscrete(7) `ControlState` (most
  expressive)?
- **Observation scope:** start state-only (MLP, simplest) and add the terrain crop
  (CNN) only if needed, or include the crop from the start?
- **Is "a learned `WormAI` that ships in-game" a real goal**, or is the agent purely
  a research/experimentation artifact? (Affects whether Rust-native deployment is a
  requirement and how much determinism the policy itself must preserve.)
- **Relationship to `predictive_ai`:** treat RL as a *replacement* for the search AI,
  or as a *policy/value prior* that guides its plan search (an AlphaZero-style
  marriage of the existing forward-search with a learned net)?

---

*Exploration only. No code written, no commits, `rust/` and `src/` untouched —
read-only inspection of the C++ engine and the existing roadmap/spec docs.*
