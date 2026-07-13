"""T4 gates: PettingZoo `ParallelEnv` + Gymnasium frozen-opponent wrapper.

Runs the plan's two hard gates plus a determinism smoke through both wrapper
layers:

  1. `pettingzoo.test.parallel_api_test(LieroParallelEnv())` — the core-API gate.
  2. `gymnasium.utils.env_checker.check_env(LieroGymEnv())` — the SB3 on-ramp gate.
  3. Determinism smoke: same seed (+ same action stream / frozen opponent) traces
     an identical per-tick `checksum` series through BOTH the ParallelEnv and the
     Gymnasium wrapper, and a different seed diverges (design §1.4 / §5).

No pytest dependency (the pinned venv has none — constraints.txt owns the deps):
each `test_*` is a plain function, run by the `__main__` block. Invoke with the
project venv after `maturin develop`:

    rust/liero-env/.venv/bin/python rust/liero-env/python/tests/test_t4_wrappers.py
"""

import random

import numpy as np
from gymnasium.spaces import Box, MultiBinary
from gymnasium.utils.env_checker import check_env
from pettingzoo.test import parallel_api_test

import liero_env
from liero_env import (
    AGENT_NAMES,
    LieroGymEnv,
    LieroParallelEnv,
    RandomOpponent,
)


# --------------------------------------------------------------------------- #
# 1. PettingZoo ParallelEnv API gate + shape/space pins
# --------------------------------------------------------------------------- #
def test_parallel_api_test_passes():
    """The plan's core-API gate: PettingZoo's own conformance test over the
    `ParallelEnv`. A short `max_ticks` so truncation (and the agents-emptying
    episode-end path) is actually exercised within the test's cycle budget."""
    env = LieroParallelEnv(max_ticks=200)
    parallel_api_test(env, num_cycles=1000)


def test_parallel_env_spaces_are_per_agent_methods():
    """T0 API warning pinned: `observation_space(agent)` / `action_space(agent)`
    are METHODS returning `Box(float32, (OBS_DIM,))` / `MultiBinary(7)`, with the
    two symmetric worm-agents as `possible_agents`."""
    env = LieroParallelEnv()
    assert env.possible_agents == ["worm_0", "worm_1"] == AGENT_NAMES
    for agent in env.possible_agents:
        obs_space = env.observation_space(agent)
        act_space = env.action_space(agent)
        assert isinstance(obs_space, Box), obs_space
        assert obs_space.shape == (liero_env.OBS_DIM,), obs_space.shape
        assert obs_space.dtype == np.float32, obs_space.dtype
        assert isinstance(act_space, MultiBinary), act_space
        assert act_space.n == liero_env.N_ACTION_BITS, act_space.n
        # Identity stability (the checkers assume it): same object each call.
        assert env.observation_space(agent) is obs_space


def test_parallel_env_reset_and_step_shapes():
    """`reset` → `(obs, infos)` keyed by both agents; `step` → the 5-tuple of
    per-agent dicts; agents empty on truncation."""
    env = LieroParallelEnv(max_ticks=3)
    obs, infos = env.reset(seed=7)
    assert set(obs) == set(env.possible_agents)
    assert set(infos) == set(env.possible_agents)
    for a in env.possible_agents:
        assert obs[a].shape == (liero_env.OBS_DIM,) and obs[a].dtype == np.float32
        assert env.observation_space(a).contains(obs[a])

    noop = {a: np.zeros(liero_env.N_ACTION_BITS, dtype=np.int8) for a in env.agents}
    truncated_seen = False
    for _ in range(3):
        if not env.agents:
            break
        o, r, term, trunc, info = env.step({a: noop[a] for a in env.agents})
        for a in o:
            assert env.observation_space(a).contains(o[a])
            assert np.isfinite(r[a])
            assert "checksum" in info[a]
        if any(trunc.values()):
            truncated_seen = True
    assert truncated_seen, "max_ticks=3 must truncate within 3 steps"
    assert env.agents == [], "agents must empty once the round truncates"


# --------------------------------------------------------------------------- #
# 2. Gymnasium wrapper gate + frozen opponent
# --------------------------------------------------------------------------- #
def test_gym_check_env_passes():
    """The plan's SB3 on-ramp gate: Gymnasium's `check_env` over the single-agent
    frozen-opponent wrapper."""
    env = LieroGymEnv()
    check_env(env)


def test_gym_spaces_are_gymnasium_style_attributes():
    """Gymnasium style: `observation_space` / `action_space` are ATTRIBUTES
    (`Box(float32, (OBS_DIM,))` / `MultiBinary(7)`)."""
    env = LieroGymEnv()
    assert isinstance(env.observation_space, Box)
    assert env.observation_space.shape == (liero_env.OBS_DIM,)
    assert env.observation_space.dtype == np.float32
    assert isinstance(env.action_space, MultiBinary)
    assert env.action_space.n == liero_env.N_ACTION_BITS


def test_gym_reset_step_and_frozen_opponent_seam():
    """`reset` → `(obs, info)`; `step` → the Gymnasium 5-tuple. A custom opponent
    callable is honored (the frozen-opponent seam) and is actually invoked."""
    calls = {"n": 0}

    def scripted_opponent(obs):
        calls["n"] += 1
        return np.zeros(liero_env.N_ACTION_BITS, dtype=np.int8)

    env = LieroGymEnv(opponent=scripted_opponent, agent_index=0)
    obs, info = env.reset(seed=3)
    assert obs.shape == (liero_env.OBS_DIM,) and obs.dtype == np.float32
    assert env.observation_space.contains(obs)
    for _ in range(5):
        obs, reward, term, trunc, info = env.step(env.action_space.sample())
        assert env.observation_space.contains(obs)
        assert np.isfinite(reward) and isinstance(reward, float)
        assert isinstance(term, bool) and isinstance(trunc, bool)
    assert calls["n"] == 5, "the frozen opponent callable must drive the other worm"


# --------------------------------------------------------------------------- #
# 3. Determinism smoke through BOTH wrapper layers
# --------------------------------------------------------------------------- #
def _parallel_action_stream(n, seed=20260713):
    rng = random.Random(seed)
    return [
        {
            "worm_0": np.array(
                [rng.randint(0, 1) for _ in range(liero_env.N_ACTION_BITS)], dtype=np.int8
            ),
            "worm_1": np.array(
                [rng.randint(0, 1) for _ in range(liero_env.N_ACTION_BITS)], dtype=np.int8
            ),
        }
        for _ in range(n)
    ]


def _parallel_checksums(seed, actions):
    env = LieroParallelEnv(max_ticks=10_000)
    _obs, infos = env.reset(seed=seed)
    trace = [infos["worm_0"]["checksum"]]
    for act in actions:
        _o, _r, _t, _tr, info = env.step(act)
        trace.append(info["worm_0"]["checksum"])
    return trace


def test_parallel_env_determinism_same_seed_and_divergence():
    """Same `(seed, action stream)` ⇒ identical checksum trace through the
    ParallelEnv layer; different seed diverges; the trace actually evolves."""
    actions = _parallel_action_stream(120)
    a = _parallel_checksums(4242, actions)
    b = _parallel_checksums(4242, actions)
    assert a == b, "same seed + same actions must trace identical checksums (ParallelEnv)"
    assert any(x != y for x, y in zip(a, a[1:])), "checksum trace must evolve"
    c = _parallel_checksums(1, actions)
    d = _parallel_checksums(2, actions)
    assert c != d, "different seeds must diverge (ParallelEnv)"


def _gym_agent_action_stream(n, seed=999):
    rng = random.Random(seed)
    return [
        np.array([rng.randint(0, 1) for _ in range(liero_env.N_ACTION_BITS)], dtype=np.int8)
        for _ in range(n)
    ]


def _gym_checksums(seed, agent_actions, opponent_seed=0):
    # A fresh frozen opponent per rollout: re-seeded from the episode seed inside
    # reset(), so both rollouts drive the opposing worm identically.
    env = LieroGymEnv(max_ticks=10_000, opponent=RandomOpponent(opponent_seed))
    _obs, info = env.reset(seed=seed)
    trace = [info["checksum"]]
    for act in agent_actions:
        _o, _r, _t, _tr, info = env.step(act)
        trace.append(info["checksum"])
    return trace


def test_gym_env_determinism_same_seed_and_divergence():
    """Through the Gymnasium wrapper (frozen opponent + agent action stream): same
    seed ⇒ identical checksum trace, different seed diverges. This is the whole
    single-agent trajectory (sim RNG + frozen-opponent PRNG) reproduced per seed."""
    agent_actions = _gym_agent_action_stream(120)
    a = _gym_checksums(4242, agent_actions)
    b = _gym_checksums(4242, agent_actions)
    assert a == b, "same seed ⇒ identical trace through the Gymnasium wrapper"
    assert any(x != y for x, y in zip(a, a[1:])), "checksum trace must evolve"
    c = _gym_checksums(1, agent_actions)
    d = _gym_checksums(2, agent_actions)
    assert c != d, "different seeds must diverge (Gymnasium wrapper)"


TESTS = [
    test_parallel_api_test_passes,
    test_parallel_env_spaces_are_per_agent_methods,
    test_parallel_env_reset_and_step_shapes,
    test_gym_check_env_passes,
    test_gym_spaces_are_gymnasium_style_attributes,
    test_gym_reset_step_and_frozen_opponent_seam,
    test_parallel_env_determinism_same_seed_and_divergence,
    test_gym_env_determinism_same_seed_and_divergence,
]


def main():
    failures = 0
    for t in TESTS:
        try:
            t()
        except Exception as exc:  # noqa: BLE001 (smoke runner: report every failure)
            failures += 1
            print(f"FAIL {t.__name__}: {type(exc).__name__}: {exc}")
        else:
            print(f"ok   {t.__name__}")
    if failures:
        raise SystemExit(f"\n{failures}/{len(TESTS)} T4 tests failed")
    print(f"\nall {len(TESTS)} T4 tests passed")


if __name__ == "__main__":
    main()
