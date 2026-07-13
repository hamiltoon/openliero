"""Frozen-copy-rotation self-play **sketch** (plan T6) — documented, NOT a tuned league.

The T6 milestone trains against a fixed `RandomOpponent` (see `train_ppo_sb3.py`);
this file sketches the one seam that turns that into self-play, so the path is
concrete without committing a tuned training run. It is intentionally minimal and
is **not** part of the milestone evidence — the random-opponent run is.

The whole idea rests on the frozen-opponent seam already built into
`LieroGymEnv` (design §6, `__init__`'s `opponent` argument): the opponent is any

    callable(np.ndarray obs) -> length-7 MultiBinary action

The default is `RandomOpponent`. Self-play just swaps in a **frozen snapshot of a
past policy** as that callable, and periodically refreshes the snapshot to the
current policy. No wrapper change, no engine change.

    ┌─ current policy (PPO, learning) ──────────────┐
    │                                               │  every `refresh_every` steps:
    │   trains in LieroGymEnv(opponent = frozen) ───┼──►  frozen := deepcopy(current)
    └───────────────────────────────────────────────┘

Why a *frozen copy* and not the live policy: training against a moving target
(the live net updating every step) is unstable; a periodically-refreshed frozen
snapshot is the standard cheap self-play recipe (a 1-deep league). A real league
(a pool of snapshots, prioritized sampling, exploitability tracking) is the
deferred `self-play league` track — explicitly out of this plan.

This sketch does NOT run a long training job; run it only to see the seam wired.
"""

from __future__ import annotations

import numpy as np


class FrozenPolicyOpponent:
    """Wrap a frozen SB3 policy snapshot as a `LieroGymEnv` opponent callable.

    The snapshot is held in a **dedicated second PPO** whose weights are copied
    from the live model via SB3's own `get_parameters` / `set_parameters` (which
    clone the tensors) — NOT `copy.deepcopy(model.policy)`, which raises on a
    policy holding non-leaf tensors after a forward pass. Because the snapshot is
    a separate model, later updates to the live policy do not leak into the
    opponent (the frozen-target stability point above) until an explicit
    `refresh`. `deterministic=True` keeps the opponent's stream a fixed function
    of the observation — reproducible episodes.
    """

    def __init__(self, snapshot_model, live_model=None):
        self._model = snapshot_model
        if live_model is not None:
            self.refresh(live_model)

    def refresh(self, live_model):
        """Rotate the frozen opponent forward to the live policy's current weights."""
        self._model.set_parameters(live_model.get_parameters(), exact_match=True)

    def __call__(self, obs):
        action, _ = self._model.predict(obs, deterministic=True)
        return np.asarray(action, dtype=np.int8)

    # A no-op `reset` so `LieroGymEnv.reset` can re-seed opponents that want it;
    # a frozen deterministic policy needs no per-episode reseeding.
    def reset(self, seed=None):  # noqa: ARG002
        return None


def sketch():
    """Wire the seam end-to-end for a handful of steps (proof-of-plumbing only).

    Bootstrap note: the very first opponent snapshot is a freshly-initialized
    (random-ish) policy — equivalent to the milestone's `RandomOpponent` start —
    and each interval re-freezes the improving current policy into it.
    """
    from stable_baselines3 import PPO
    from stable_baselines3.common.vec_env import DummyVecEnv, VecMonitor

    from liero_env import LieroGymEnv

    def _env(**kw):
        return LieroGymEnv(max_ticks=400, frame_skip=4, **kw)

    # A dedicated snapshot model holds the frozen opponent's weights. Its throwaway
    # env just fixes the obs/action spaces; it is never trained.
    frozen_model = PPO("MlpPolicy", DummyVecEnv([_env]), n_steps=64, batch_size=64,
                       verbose=0)
    opponent = FrozenPolicyOpponent(frozen_model)  # bootstrap = untrained policy

    venv = VecMonitor(DummyVecEnv([lambda: _env(opponent=opponent)]))
    model = PPO("MlpPolicy", venv, n_steps=256, batch_size=64, verbose=0)

    total_iters = 3
    refresh_every_steps = 2048
    for it in range(total_iters):
        model.learn(total_timesteps=refresh_every_steps, reset_num_timesteps=False)
        opponent.refresh(model)  # rotate: current policy becomes the next opponent
        print(f"[self-play] iter {it}: refreshed frozen opponent to current policy")

    print("self-play sketch wired OK (this is a sketch, not a tuned league).")


if __name__ == "__main__":
    sketch()
