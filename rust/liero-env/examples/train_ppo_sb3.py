"""Minimal SB3-PPO training example over ``LieroGymEnv`` — the plan-T6 milestone.

This is the harness's **runnable proof**, not a tuned agent: an off-the-shelf
Stable-Baselines3 PPO (``MlpPolicy``) trains a single worm against the default
frozen ``RandomOpponent`` through the Gymnasium wrapper (design §6). Two things
are demonstrated, both observable rather than asserted-by-hope (design §9):

1. **The reward curve rises.** SB3's ``Monitor`` logs raw episodic reward
   (``rollout/ep_rew_mean``); over the run it trends up above the random-policy
   baseline (measure the baseline with ``--baseline-only``). The full series is
   written to ``<run>/progress.csv`` (SB3's CSV logger) for the training note.
2. **An eval recording replays in the real window.** Every ``--eval-every``
   steps, :class:`EvalRecordCallback` runs a handful of *deterministic* eval
   episodes, logs their mean reward (``eval/mean_reward``), and taps ONE of them
   to ``<run>/eval_gen<N>.txt`` via the env's ``start_recording`` /
   ``save_recording`` passthrough (plan T5). That file plays in the Bevy window:

       cargo run -p game -- --replay <run>/eval_gen<N>.txt

Design choices kept deliberately plain so the example stays a readable on-ramp:

- **No ``VecNormalize``.** The Rust obs extractor already emits ≈unit-scale
  ``float32`` (design §3.1; verified empirically in the T6 note), so the default
  ``MlpPolicy`` trains directly and — crucially — the eval-recording env is the
  *same* ``LieroGymEnv`` type with no shared normalization statistics to thread
  through. One fewer moving part between "train" and "watch what it learned".
- **``MultiBinary(7)`` action space consumed natively** by PPO's Bernoulli head
  (design §2 / T0 research note) — no action re-encoding.
- **Separate policy/exploration RNG from the sim RNG** (design §5): the frozen
  opponent and env seeds are explicit; nothing here re-enters ``SimState.rand``.

Run (from the repo root, in the pinned venv after ``maturin develop``):

    rust/liero-env/.venv/bin/python \
        rust/liero-env/examples/train_ppo_sb3.py --timesteps 400000

Everything it writes (model, CSV progress log, eval recordings) lands under
``--run-dir`` (default ``runs/ppo_<timestamp>``), which is gitignored — only the
example, the training note, and one representative eval recording are committed.
"""

from __future__ import annotations

import argparse
import os
import time
from pathlib import Path

import numpy as np


def make_env(max_ticks: int, frame_skip: int, seed: int | None = None):
    """Factory for one ``LieroGymEnv`` vs the default frozen ``RandomOpponent``.

    Imported lazily inside the factory so ``SubprocVecEnv`` subprocesses each
    import the compiled extension cleanly.
    """
    from liero_env import LieroGymEnv

    def _init():
        env = LieroGymEnv(max_ticks=max_ticks, frame_skip=frame_skip)
        if seed is not None:
            env.reset(seed=seed)
        return env

    return _init


def measure_baseline(max_ticks, frame_skip, n_eps=30, seed0=1_000_000):
    """Random-policy episodic-reward baseline — the bar the learned curve must
    clear (design §9). Same env config as training, uniform-random actions."""
    from liero_env import LieroGymEnv

    env = LieroGymEnv(max_ticks=max_ticks, frame_skip=frame_skip)
    returns = []
    for ep in range(n_eps):
        env.reset(seed=seed0 + ep)
        total, done = 0.0, False
        while not done:
            _o, r, term, trunc, _ = env.step(env.action_space.sample())
            total += r
            done = term or trunc
        returns.append(total)
    return float(np.mean(returns)), float(np.std(returns))


def make_eval_callback(run_dir, eval_every, max_ticks, frame_skip,
                       n_eval_episodes=8, record_seed=7_777):
    """Build the periodic deterministic-eval callback (plan T5/T6).

    Every ``eval_every`` steps it (a) logs mean episodic reward over
    ``n_eval_episodes`` deterministic episodes (``eval/mean_reward``) and
    (b) records ONE of them to a replayable scenario file. Implemented against
    SB3's ``BaseCallback`` (imported here, not at module load, so
    ``--baseline-only`` stays torch-free-fast). The recorded episode uses a
    fixed seed so successive generations are visually comparable in ``--replay``.
    """
    from stable_baselines3.common.callbacks import BaseCallback
    from liero_env import LieroGymEnv

    class _Cb(BaseCallback):
        def __init__(self):
            super().__init__()
            self._eval_env = LieroGymEnv(max_ticks=max_ticks, frame_skip=frame_skip)
            self._next_eval = eval_every
            self._gen = 0
            self.history = []  # (timesteps, mean_reward, recording_path)

        def _run_episode(self, seed, record_path=None):
            obs, _ = self._eval_env.reset(seed=seed)
            if record_path is not None:
                self._eval_env.start_recording()
            total, done = 0.0, False
            while not done:
                action, _ = self.model.predict(obs, deterministic=True)
                obs, r, term, trunc, _ = self._eval_env.step(action)
                total += r
                done = term or trunc
            if record_path is not None:
                self._eval_env.save_recording(record_path)
            return total

        def _evaluate(self):
            self._gen += 1
            rec_path = os.path.join(run_dir, f"eval_gen{self._gen}.txt")
            # Episode 0 of the eval batch is the one we record.
            rewards = [self._run_episode(record_seed, rec_path)]
            for i in range(1, n_eval_episodes):
                rewards.append(self._run_episode(record_seed + i))
            mean_r = float(np.mean(rewards))
            self.logger.record("eval/mean_reward", mean_r)
            self.logger.record("eval/gen", self._gen)
            self.history.append((int(self.num_timesteps), mean_r, rec_path))
            print(f"[eval] gen{self._gen} t={self.num_timesteps} "
                  f"mean_reward={mean_r:.2f} recorded={os.path.basename(rec_path)}")

        def _on_step(self) -> bool:
            if self.num_timesteps >= self._next_eval:
                self._evaluate()
                self._next_eval += eval_every
            return True

    return _Cb()


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--timesteps", type=int, default=400_000)
    ap.add_argument("--n-envs", type=int, default=8)
    ap.add_argument("--n-steps", type=int, default=512)
    ap.add_argument("--batch-size", type=int, default=256)
    ap.add_argument("--n-epochs", type=int, default=10)
    ap.add_argument("--lr", type=float, default=3e-4)
    ap.add_argument("--ent-coef", type=float, default=0.01)
    ap.add_argument("--gamma", type=float, default=0.99)
    ap.add_argument("--max-ticks", type=int, default=1200)
    ap.add_argument("--frame-skip", type=int, default=4)
    ap.add_argument("--eval-every", type=int, default=40_000)
    ap.add_argument("--seed", type=int, default=0)
    ap.add_argument("--run-dir", type=str, default=None)
    ap.add_argument("--baseline-only", action="store_true",
                    help="Print the random-policy baseline and exit (no training).")
    args = ap.parse_args()

    if args.baseline_only:
        mean_r, std_r = measure_baseline(args.max_ticks, args.frame_skip)
        print(f"random baseline ep_rew: mean={mean_r:.2f} std={std_r:.2f}")
        return

    # Imports deferred so --baseline-only stays torch-free-fast.
    from stable_baselines3 import PPO
    from stable_baselines3.common.logger import configure
    from stable_baselines3.common.monitor import Monitor
    from stable_baselines3.common.vec_env import DummyVecEnv, VecMonitor

    run_dir = args.run_dir or os.path.join(
        "runs", f"ppo_{time.strftime('%Y%m%d_%H%M%S')}")
    Path(run_dir).mkdir(parents=True, exist_ok=True)
    print(f"run_dir: {run_dir}")

    # Vectorized training envs; VecMonitor records RAW episodic reward
    # (rollout/ep_rew_mean) before any wrapping — the curve we report.
    venv = DummyVecEnv([
        make_env(args.max_ticks, args.frame_skip, seed=args.seed + i)
        for i in range(args.n_envs)
    ])
    venv = VecMonitor(venv)

    model = PPO(
        "MlpPolicy",
        venv,
        n_steps=args.n_steps,
        batch_size=args.batch_size,
        n_epochs=args.n_epochs,
        learning_rate=args.lr,
        ent_coef=args.ent_coef,
        gamma=args.gamma,
        seed=args.seed,
        verbose=1,
    )
    # stdout + CSV logger so progress.csv holds the full ep_rew_mean series.
    # (Tensorboard is intentionally not a harness dependency — it is not pinned
    # in constraints.txt; the CSV is the committed-note's data source.)
    model.set_logger(configure(run_dir, ["stdout", "csv"]))

    eval_cb = make_eval_callback(
        run_dir=run_dir,
        eval_every=args.eval_every,
        max_ticks=args.max_ticks,
        frame_skip=args.frame_skip,
    )

    t0 = time.time()
    model.learn(total_timesteps=args.timesteps, callback=eval_cb, progress_bar=False)
    dt = time.time() - t0

    model.save(os.path.join(run_dir, "ppo_final"))
    print(f"\ntrained {args.timesteps} steps in {dt/60:.1f} min "
          f"({args.timesteps/dt:.0f} steps/s)")
    print(f"model:   {os.path.join(run_dir, 'ppo_final.zip')}")
    print(f"csv:     {os.path.join(run_dir, 'progress.csv')}")
    if eval_cb.history:
        last = eval_cb.history[-1]
        print(f"last eval recording: {last[2]}  (mean_reward={last[1]:.2f})")
        print("watch it:  cargo run -p game -- --replay " + last[2])


if __name__ == "__main__":
    main()
