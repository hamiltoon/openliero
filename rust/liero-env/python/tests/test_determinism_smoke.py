"""Python determinism / shape / reward smoke for the `liero_env` PyO3 extension (plan T3).

Runs the T1 determinism contract from the Python side of the FFI boundary: the same
`(seed, action stream)` through `RawEnv.step` must trace an identical per-tick
`checksum` series (design §1.4), and a different seed must diverge. Also pins the obs
shape (`OBS_DIM == 43`, `float32`) and a reward-sign sanity check (idle steps are
non-positive; the `reward_config` dict override actually moves the reward).

No pytest dependency (the pinned venv has none — constraints.txt owns the deps): each
`test_*` is a plain function, run by the `__main__` block below. Invoke with the
project venv:

    rust/liero-env/.venv/bin/python rust/liero-env/python/tests/test_determinism_smoke.py

The extension must be built first: `maturin develop` in that venv (plan T3).
"""

import random

import numpy as np

import liero_env
from liero_env import RawEnv


def _action_stream(n, seed=20260713):
    """A deterministic list of `n` per-step `(action0, action1)` pairs.

    Uses Python's RNG (fixed `seed`) purely to script the *actions* — this is the
    policy/exploration RNG the design keeps strictly separate from the sim RNG
    (design §5 seeding discipline); it never touches `SimState.rand`. Each action is a
    length-7 list of 0/1 (a `MultiBinary(7)` word).
    """
    rng = random.Random(seed)
    return [
        (
            [rng.randint(0, 1) for _ in range(liero_env.N_ACTION_BITS)],
            [rng.randint(0, 1) for _ in range(liero_env.N_ACTION_BITS)],
        )
        for _ in range(n)
    ]


def _rollout_checksums(seed, actions, max_ticks=10_000, frame_skip=1):
    """Reset with `seed`, step the whole action stream, return the checksum trace
    `[reset_checksum, step_1, step_2, ...]`."""
    env = RawEnv(max_ticks=max_ticks, frame_skip=frame_skip)
    env.reset(seed)
    trace = [env.checksum]
    for a0, a1 in actions:
        _obs, _rewards, _term, _trunc, info = env.step(a0, a1)
        trace.append(info["checksum"])
    return trace


def test_module_constants():
    """The layout constants Python needs to size spaces (T4) are exposed and match
    the design (`OBS_DIM == 43`, `MultiBinary(7)`, symmetric 1v1)."""
    assert liero_env.OBS_DIM == 43, liero_env.OBS_DIM
    assert liero_env.N_ACTION_BITS == 7, liero_env.N_ACTION_BITS
    assert liero_env.N_WORMS == 2, liero_env.N_WORMS


def test_reset_returns_obs_pair_of_shape_obs_dim():
    """`reset` returns two per-agent observations, each a `float32` array of length
    `OBS_DIM`, all finite."""
    env = RawEnv()
    obs0, obs1 = env.reset(1234)
    for i, obs in enumerate((obs0, obs1)):
        assert isinstance(obs, np.ndarray), (i, type(obs))
        assert obs.shape == (liero_env.OBS_DIM,), (i, obs.shape)
        assert obs.dtype == np.float32, (i, obs.dtype)
        assert np.all(np.isfinite(obs)), (i, obs)


def test_step_returns_obs_pair_and_five_tuple():
    """`step` returns `((obs0, obs1), (r0, r1), (term0, term1), truncated, info)` with
    the documented shapes/types, and `info` carries a `checksum`."""
    env = RawEnv()
    env.reset(1234)
    obs, rewards, terminated, truncated, info = env.step([0] * 7, [0] * 7)
    obs0, obs1 = obs
    assert obs0.shape == (liero_env.OBS_DIM,) and obs0.dtype == np.float32
    assert obs1.shape == (liero_env.OBS_DIM,) and obs1.dtype == np.float32
    assert len(rewards) == 2 and all(np.isfinite(rewards))
    assert len(terminated) == 2 and all(isinstance(t, bool) for t in terminated)
    assert isinstance(truncated, bool)
    assert "checksum" in info and isinstance(info["checksum"], int)


def test_same_seed_same_actions_identical_checksum_trace():
    """The core determinism contract, from Python: same `(seed, action stream)` ⇒
    identical checksum trace across two independent envs — and the trace must actually
    evolve (else the equality is vacuous)."""
    actions = _action_stream(120)
    a = _rollout_checksums(4242, actions)
    b = _rollout_checksums(4242, actions)
    assert a == b, "same seed + same actions must trace identical checksums"
    assert any(x != y for x, y in zip(a, a[1:])), "checksum trace must evolve"


def test_different_seed_diverges():
    """Non-vacuity: a different sim seed drives the RNG down a different path, so the
    checksum trace must diverge (design §5)."""
    actions = _action_stream(120)
    a = _rollout_checksums(1, actions)
    b = _rollout_checksums(2, actions)
    assert a != b, "different seeds must produce a divergent checksum trace"


def test_numpy_action_arrays_are_accepted():
    """A `MultiBinary(7)`-style numpy sample (`int8` array) is a valid action, matching
    the same-seed trace built from plain lists — confirms the binding's cross-dtype
    action extraction (T4 passes numpy samples)."""
    actions = _action_stream(30)
    list_trace = _rollout_checksums(77, actions)

    env = RawEnv(max_ticks=10_000, frame_skip=1)
    env.reset(77)
    np_trace = [env.checksum]
    for a0, a1 in actions:
        _o, _r, _t, _tr, info = env.step(
            np.array(a0, dtype=np.int8), np.array(a1, dtype=np.int8)
        )
        np_trace.append(info["checksum"])
    assert np_trace == list_trace, "numpy int8 actions must match list actions"


def test_reward_sign_sanity_and_config_override():
    """Reward-sign sanity: an idle (all-zero) early step yields a non-positive, finite
    reward for both agents (no damage/kills → only the tiny time penalty). And the
    `reward_config` dict must actually move the reward — a large `w_time` makes the
    idle step strictly more negative, confirming the Python→Rust weight plumbing."""
    default_env = RawEnv()
    default_env.reset(5)
    _o, default_rewards, _t, _tr, _i = default_env.step([0] * 7, [0] * 7)
    assert all(np.isfinite(default_rewards)), default_rewards
    assert all(r <= 0.0 for r in default_rewards), default_rewards

    heavy_env = RawEnv(reward_config={"w_time": 100.0})
    heavy_env.reset(5)
    _o, heavy_rewards, _t, _tr, _i = heavy_env.step([0] * 7, [0] * 7)
    assert all(np.isfinite(heavy_rewards)), heavy_rewards
    for hr, dr in zip(heavy_rewards, default_rewards):
        assert hr < dr, ("w_time override must make idle reward more negative", hr, dr)


TESTS = [
    test_module_constants,
    test_reset_returns_obs_pair_of_shape_obs_dim,
    test_step_returns_obs_pair_and_five_tuple,
    test_same_seed_same_actions_identical_checksum_trace,
    test_different_seed_diverges,
    test_numpy_action_arrays_are_accepted,
    test_reward_sign_sanity_and_config_override,
]


def main():
    failures = 0
    for t in TESTS:
        try:
            t()
        except AssertionError as exc:
            failures += 1
            print(f"FAIL {t.__name__}: {exc}")
        else:
            print(f"ok   {t.__name__}")
    if failures:
        raise SystemExit(f"\n{failures}/{len(TESTS)} smoke tests failed")
    print(f"\nall {len(TESTS)} smoke tests passed")


if __name__ == "__main__":
    main()
