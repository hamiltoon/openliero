"""`liero_env` — the RL harness's user-facing Python package (plan T4, design §6).

This package is the ecosystem glue over the raw PyO3 surface. The compiled
extension (`RawEnv`, plus the `OBS_DIM` / `N_ACTION_BITS` / `N_WORMS` layout
constants) is built by maturin as the **private** submodule `._liero_env` and
re-exported here; on top of it this module adds:

- `LieroParallelEnv` — a PettingZoo `ParallelEnv` over the two symmetric
  worm-agents (`possible_agents == ["worm_0", "worm_1"]`), the design's **core
  API**. Action space `MultiBinary(7)`, observation space `Box(float32, (43,))`,
  both exposed as per-agent **methods** (PettingZoo 1.26.1 idiom — see the T0
  research note's API warning: `observation_space(agent)` / `action_space(agent)`
  are methods, not attributes).
- `LieroGymEnv` — a single-agent Gymnasium `Env` where the opponent worm is
  driven by a **frozen policy callable** (default `RandomOpponent`, a
  deterministic PRNG). This is the one-line Stable-Baselines3 on-ramp; the
  frozen-opponent seam is where self-play rotation slots in later (see
  `LieroGymEnv.__init__`'s `opponent` argument).
- `RandomOpponent` — a deterministic `MultiBinary(7)` policy used as the default
  frozen opponent.

The hot path stays in Rust: only compact `float32` obs arrays cross the FFI
boundary (design §8); these wrappers are thin re-shaping over `RawEnv`.
"""

import functools

import numpy as np
from gymnasium import Env
from gymnasium.spaces import Box, MultiBinary
from pettingzoo import ParallelEnv

# The compiled PyO3 extension (private submodule, built by maturin as
# `liero_env._liero_env`; see pyproject.toml `module-name`). Re-exported so the
# raw surface stays reachable as `liero_env.RawEnv` for the T3 smoke test and any
# direct consumer.
from ._liero_env import (  # noqa: F401  (re-export)
    N_ACTION_BITS,
    N_WORMS,
    OBS_DIM,
    RawEnv,
    scenario_parses,
)

__all__ = [
    "RawEnv",
    "OBS_DIM",
    "N_ACTION_BITS",
    "N_WORMS",
    "RandomOpponent",
    "LieroParallelEnv",
    "LieroGymEnv",
    "AGENT_NAMES",
    "scenario_parses",
]

#: The two symmetric worm-agents' PettingZoo IDs (index 0 → worm 0, 1 → worm 1).
AGENT_NAMES = [f"worm_{i}" for i in range(N_WORMS)]

#: Largest seed the Rust `RawEnv.reset` accepts (it takes a `u32`).
_SEED_MODULUS = 1 << 32


def _coerce_seed(seed):
    """Map an optional caller seed to a valid `RawEnv` `u32` seed.

    A `None` seed draws a fresh one (training variety, design §5 — determinism is
    *per seed*, so a drawn seed is still fully reproducible if captured). A given
    seed is reduced mod 2**32 so any Python/NumPy integer is accepted while the
    Rust `u32` bound is never overflowed.
    """
    if seed is None:
        return int(np.random.SeedSequence().generate_state(1, dtype=np.uint32)[0])
    return int(seed) % _SEED_MODULUS


def _observation_space():
    """The shared per-agent observation space: a flat `float32` `Box` of length
    `OBS_DIM`.

    Bounds are `(-inf, +inf)`: the obs vector is normalized to *≈unit scale* but
    is deliberately **not** hard-clamped (a worm past 0 health or moving unusually
    fast legitimately reads outside `[-1, 1]` — that is signal, see the Rust
    `obs.rs` module docs). Unbounded-but-finite `Box` bounds keep every extracted
    obs `contained()` (the PettingZoo/Gymnasium checkers assert containment) while
    faithfully declaring the space is not pre-normalized.
    """
    return Box(low=-np.inf, high=np.inf, shape=(OBS_DIM,), dtype=np.float32)


def _action_space():
    """The shared per-agent action space: `MultiBinary(N_ACTION_BITS)` — the sim's
    native 7-bit `ControlState` word (design §2), consumed natively by SB3-PPO's
    Bernoulli head (no re-encoding)."""
    return MultiBinary(N_ACTION_BITS)


class RandomOpponent:
    """A deterministic uniform-random `MultiBinary(N_ACTION_BITS)` policy — the
    default **frozen opponent** for `LieroGymEnv`.

    Determinism is the point: seeded from a single `u32`, it produces a fixed
    action stream, so a `(env seed, opponent seed)` pair yields a fully
    reproducible single-agent trajectory (the determinism gate exercises this
    through the wrapper). Its exploration RNG is entirely separate from the sim
    RNG (design §5 seeding discipline) — it never touches `SimState.rand`.

    The callable signature `opponent(obs) -> action` is the frozen-opponent
    **seam**: swap in any `callable(np.ndarray) -> length-7 action` (e.g. a frozen
    snapshot of a trained policy) for self-play rotation, no wrapper change.
    """

    def __init__(self, seed=0):
        self._seed = int(seed) % _SEED_MODULUS
        self._rng = np.random.default_rng(self._seed)

    def reset(self, seed=None):
        """Re-seed the action PRNG for a fresh episode. `None` keeps the last
        seed (so a fixed construction seed stays reproducible across resets); an
        explicit seed rebinds it (the wrapper passes the episode seed here so the
        opponent's stream is deterministic per episode)."""
        if seed is not None:
            self._seed = int(seed) % _SEED_MODULUS
        self._rng = np.random.default_rng(self._seed)

    def __call__(self, obs):  # noqa: ARG002  (obs unused — a random policy ignores it)
        return self._rng.integers(0, 2, size=N_ACTION_BITS, dtype=np.int8)


class LieroParallelEnv(ParallelEnv):
    """PettingZoo `ParallelEnv` over the symmetric 1v1 (design's **core API**).

    Both worm-agents act every tick (`actions` is a dict keyed by
    `possible_agents`); `step` returns the PettingZoo 5-tuple of per-agent dicts
    `(observations, rewards, terminations, truncations, infos)`. In the symmetric
    KillEmAll round both agents terminate together (design §5); on termination or
    truncation `self.agents` is emptied, per PettingZoo's episode contract.

    Thin over `RawEnv`: obs/reward/action packing all happen Rust-side. Each
    agent's `info` carries the per-tick `wide_rollback_checksum` (`"checksum"`),
    the determinism fingerprint the gate traces.
    """

    metadata = {"render_modes": [], "name": "liero_parallel_v0"}

    def __init__(self, max_ticks=None, frame_skip=1, reward_config=None, render_mode=None):
        self.possible_agents = list(AGENT_NAMES)
        self.render_mode = render_mode
        self._frame_skip = frame_skip
        self._reward_config = reward_config
        # `None` max_ticks → the Rust `RawEnv` default (DEFAULT_MAX_TICKS = 3000).
        self._max_ticks = max_ticks
        self._raw = self._make_raw()
        self.agents = []
        # Dict mirrors of the per-agent spaces (T0 research note: PettingZoo wants
        # `observation_space(agent)` / `action_space(agent)` as METHODS, plus the
        # `observation_spaces` / `action_spaces` dict mirrors for convenience).
        self.observation_spaces = {a: _observation_space() for a in self.possible_agents}
        self.action_spaces = {a: _action_space() for a in self.possible_agents}

    def _make_raw(self):
        if self._max_ticks is None:
            return RawEnv(frame_skip=self._frame_skip, reward_config=self._reward_config)
        return RawEnv(
            max_ticks=self._max_ticks,
            frame_skip=self._frame_skip,
            reward_config=self._reward_config,
        )

    # PettingZoo 1.26.1: the spaces are per-agent METHODS (T0 research note), not
    # attributes. `lru_cache` returns a stable per-agent object (the checkers
    # assume identity stability across calls).
    @functools.lru_cache(maxsize=None)  # noqa: B019  (bounded: two agents)
    def observation_space(self, agent):
        return self.observation_spaces[agent]

    @functools.lru_cache(maxsize=None)  # noqa: B019
    def action_space(self, agent):
        return self.action_spaces[agent]

    def reset(self, seed=None, options=None):
        """Start a fresh episode. Returns `(observations, infos)` — the PettingZoo
        2-tuple — keyed by all agents (both are always present at tick 0)."""
        self.agents = list(self.possible_agents)
        obs_pair = self._raw.reset(_coerce_seed(seed))
        observations = {a: obs_pair[i] for i, a in enumerate(self.possible_agents)}
        infos = {a: {"checksum": self._raw.checksum} for a in self.possible_agents}
        return observations, infos

    def step(self, actions):
        """Advance one env step from a per-agent `actions` dict. Returns the
        PettingZoo 5-tuple of per-agent dicts; empties `self.agents` once the round
        terminates or truncates."""
        if not actions:
            # PettingZoo contract: stepping with no agents ends the episode.
            self.agents = []
            return {}, {}, {}, {}, {}

        a0 = actions[self.possible_agents[0]]
        a1 = actions[self.possible_agents[1]]
        obs_pair, rewards, terminated, truncated, info = self._raw.step(a0, a1)

        observations = {a: obs_pair[i] for i, a in enumerate(self.possible_agents)}
        rewards = {a: float(rewards[i]) for i, a in enumerate(self.possible_agents)}
        terminations = {a: bool(terminated[i]) for i, a in enumerate(self.possible_agents)}
        truncations = {a: bool(truncated) for a in self.possible_agents}
        infos = {a: {"checksum": info["checksum"]} for a in self.possible_agents}

        if any(terminations.values()) or any(truncations.values()):
            self.agents = []

        return observations, rewards, terminations, truncations, infos

    def render(self):
        # Headless RL harness: no render path (design §3.2). The eval-recording /
        # `--replay` watch path lives in the `game` binary (design §1.5, T5).
        return None

    def close(self):
        return None

    def start_recording(self):
        """Start tapping this episode's per-tick control words (design §1.5,
        plan T5). Call after `reset()`; `save_recording` writes the tapped
        stream as a scenario file replayable via `cargo run -p game --
        --replay <path>`."""
        self._raw.start_recording()

    def save_recording(self, path):
        """Write the current recording to `path` (see `start_recording`)."""
        self._raw.save_recording(str(path))


class LieroGymEnv(Env):
    """Single-agent Gymnasium `Env` — the Stable-Baselines3 on-ramp (design §0/§6).

    One worm (`agent_index`, default 0) is the learning agent; the other is driven
    by a **frozen opponent** policy callable (default `RandomOpponent`). This
    reduces the two-agent `ParallelEnv` to the classic single-agent Gymnasium API
    SB3-PPO trains against directly — `action_space` / `observation_space` are
    plain attributes (Gymnasium style), and PPO consumes `MultiBinary(7)` natively.

    **Frozen-opponent seam (self-play later):** `opponent` is any
    `callable(np.ndarray) -> length-7 action`. The default random policy is the
    milestone baseline (design §9); passing a frozen snapshot of a trained policy
    here — with no wrapper change — is how self-play opponent rotation (T6
    `self_play.py` sketch) plugs in. If the opponent exposes a `reset(seed)`, it is
    re-seeded from the episode seed so the single-agent trajectory is fully
    reproducible per seed.
    """

    metadata = {"render_modes": [], "name": "liero_gym_v0"}

    def __init__(
        self,
        max_ticks=None,
        frame_skip=1,
        reward_config=None,
        opponent=None,
        opponent_seed=0,
        agent_index=0,
        render_mode=None,
    ):
        super().__init__()
        if agent_index not in (0, 1):
            raise ValueError(f"agent_index must be 0 or 1 (the 1v1), got {agent_index}")
        self.observation_space = _observation_space()
        self.action_space = _action_space()
        self.render_mode = render_mode

        self._frame_skip = frame_skip
        self._reward_config = reward_config
        self._max_ticks = max_ticks
        self._raw = self._make_raw()

        self._agent = agent_index
        self._opp = 1 - agent_index
        self._opponent = opponent if opponent is not None else RandomOpponent(opponent_seed)
        self._last_opp_obs = None

    def _make_raw(self):
        if self._max_ticks is None:
            return RawEnv(frame_skip=self._frame_skip, reward_config=self._reward_config)
        return RawEnv(
            max_ticks=self._max_ticks,
            frame_skip=self._frame_skip,
            reward_config=self._reward_config,
        )

    def reset(self, *, seed=None, options=None):
        """Reset to a fresh episode. Draws/coerces a `u32` sim seed, re-seeds the
        frozen opponent from the same seed (so its action stream is deterministic
        per episode), and returns `(obs, info)` for the learning agent."""
        super().reset(seed=seed)
        sim_seed = _coerce_seed(seed)
        if hasattr(self._opponent, "reset"):
            self._opponent.reset(sim_seed)
        obs_pair = self._raw.reset(sim_seed)
        self._last_opp_obs = obs_pair[self._opp]
        info = {"checksum": self._raw.checksum}
        return obs_pair[self._agent], info

    def step(self, action):
        """Advance one tick: the frozen opponent acts from its last observation,
        both actions go through `RawEnv.step`, and the learning agent's
        `(obs, reward, terminated, truncated, info)` is returned."""
        opp_action = self._opponent(self._last_opp_obs)
        if self._agent == 0:
            a0, a1 = action, opp_action
        else:
            a0, a1 = opp_action, action

        obs_pair, rewards, terminated, truncated, info = self._raw.step(a0, a1)
        self._last_opp_obs = obs_pair[self._opp]
        return (
            obs_pair[self._agent],
            float(rewards[self._agent]),
            bool(terminated[self._agent]),
            bool(truncated),
            info,
        )

    def render(self):
        return None

    def close(self):
        return None

    def start_recording(self):
        """Start tapping this episode's per-tick control words (both agents —
        the learning agent's `step` action and the frozen opponent's, design
        §1.5, plan T5). Call after `reset()`."""
        self._raw.start_recording()

    def save_recording(self, path):
        """Write the current recording to `path` (see `start_recording`)."""
        self._raw.save_recording(str(path))
