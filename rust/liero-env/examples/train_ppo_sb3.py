"""SB3-PPO training over ``LieroGymEnv`` — the plan-T6 milestone script, extended
for **run 2** with reward annealing and checkpoint resume.

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
   episodes, logs their mean reward (``eval/mean_reward``) plus **kill / death /
   episode-length** stats (``eval/kill_rate`` etc.), and taps ONE of them to
   ``<run>/eval_gen<N>.txt`` via the env's ``start_recording`` /
   ``save_recording`` passthrough (plan T5). That file plays in the Bevy window:

       cargo run -p game -- --replay <run>/eval_gen<N>.txt

Reward annealing (run 2 — the T6 note's deferred next step)
-----------------------------------------------------------
The T6 default shaping (`w_damage_dealt=1.0, w_damage_taken=0.5, w_kill=10,
w_death=10`) makes a full HP bar of chip damage (=100) dominate the sparse ±10
kill/death terminal by an order of magnitude, so the policy learns to *win the
damage race* but not to *finish kills* (T6 note §4). Reward weights live in the
env (they are passed to ``RawEnv``'s constructor), so they cannot be mutated
mid-episode — annealing is therefore **stepwise across phases**: each phase gets
a fresh vec-env with lower damage weights and higher kill/death weights, and the
model continues from the previous phase's weights (``model.set_env`` in-process
for ``--anneal``, or ``PPO.load`` across processes for ``--resume``). The
schedule of record is :data:`ANNEAL_PHASES`.

**Absolute reward is not comparable across phases** — the weights change its
scale. Compare ``ep_rew_mean`` *within* a phase; compare *behavior* across phases
with the weight-independent proxies: ``ep_len_mean`` (episodes end on a death, so
a falling length ⇒ decisive combat / kills happening) and the eval kill/death
rates.

Backward-compatible: with no ``--anneal``/``--resume``/``--reward-weights`` the
script is exactly the T6 single-phase default-weights run.

Run (from the repo root, in the pinned venv after ``maturin develop``):

    rust/liero-env/.venv/bin/python \
        rust/liero-env/examples/train_ppo_sb3.py --total-steps 400000

Everything it writes (model, CSV progress log, eval recordings) lands under
``--run-dir`` (default ``runs/ppo_<timestamp>``), which is gitignored — only the
example, the training note, and a few representative eval recordings are
committed.
"""

from __future__ import annotations

import argparse
import os
import time
from pathlib import Path

import numpy as np

# The reward-annealing schedule of record (run 2). Each phase is a fresh vec-env
# with these weights; the model continues across phases. ``weights=None`` means
# "the Rust ``RewardConfig::default``" (the T6 shaped weights). Only keys given
# are overridden; omitted keys keep the default (`w_time=0.001` throughout).
#
# The arc: shaped damage race (phase 1, = T6) -> balanced (phase 2) -> sparse,
# kill-dominant (phase 3, where a full HP bar of chip damage = 100*0.2 = 20 is
# worth LESS than a single kill = 25, inverting the T6 dominance so finishing
# pays off).
ANNEAL_PHASES = [
    {"name": "phase1_shaped", "weights": None},
    {"name": "phase2_mid",
     "weights": {"w_damage_dealt": 0.6, "w_damage_taken": 0.3,
                 "w_kill": 17.0, "w_death": 17.0}},
    {"name": "phase3_sparse",
     "weights": {"w_damage_dealt": 0.2, "w_damage_taken": 0.1,
                 "w_kill": 25.0, "w_death": 25.0}},
]


def parse_reward_weights(spec: str | None) -> dict | None:
    """Parse a ``"w_kill=25,w_damage_dealt=0.2"`` CLI string into a reward-config
    dict (or ``None`` for the default weights). Unknown keys are rejected early so
    a typo does not silently train the default config."""
    if not spec:
        return None
    valid = {"w_damage_dealt", "w_damage_taken", "w_kill", "w_death", "w_time"}
    out: dict = {}
    for tok in spec.split(","):
        tok = tok.strip()
        if not tok:
            continue
        k, _, v = tok.partition("=")
        k = k.strip()
        if k not in valid:
            raise ValueError(f"unknown reward weight {k!r}; valid: {sorted(valid)}")
        out[k] = float(v)
    return out or None


def make_env(max_ticks: int, frame_skip: int, reward_config: dict | None = None,
             seed: int | None = None):
    """Factory for one ``LieroGymEnv`` vs the default frozen ``RandomOpponent``.

    Imported lazily inside the factory so ``SubprocVecEnv`` subprocesses each
    import the compiled extension cleanly. ``reward_config`` selects the phase's
    reward weights (``None`` = the Rust default).
    """
    from liero_env import LieroGymEnv

    def _init():
        env = LieroGymEnv(max_ticks=max_ticks, frame_skip=frame_skip,
                          reward_config=reward_config)
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
                       reward_config=None, gen_prefix="", n_eval_episodes=8,
                       record_seed=7_777):
    """Build the periodic deterministic-eval callback (plan T5/T6).

    Every ``eval_every`` steps it (a) runs ``n_eval_episodes`` deterministic
    episodes, logging their mean reward (``eval/mean_reward``) plus the
    weight-independent behavior proxies ``eval/mean_ep_len``, ``eval/kill_rate``,
    ``eval/death_rate`` and ``eval/term_rate``, and (b) records ONE of them (fixed
    seed, so generations are visually comparable in ``--replay``) to
    ``<run>/<gen_prefix>eval_gen<N>.txt``.

    Kill / death attribution needs no Rust change: an episode ends only on a death
    (KillEmAll) or a max-tick truncation. On a death step exactly one terminal
    term fires for the learning agent — ``+w_kill`` if it killed the opponent,
    ``−w_death`` if it died — and that term dominates the tiny single-step damage,
    so the SIGN of the terminal reward classifies kill vs death.
    """
    from stable_baselines3.common.callbacks import BaseCallback
    from liero_env import LieroGymEnv

    class _Cb(BaseCallback):
        def __init__(self):
            super().__init__()
            self._eval_env = LieroGymEnv(max_ticks=max_ticks, frame_skip=frame_skip,
                                         reward_config=reward_config)
            self._next_eval = None  # set on first step, relative to num_timesteps
            self._gen = 0
            # (timesteps, mean_reward, mean_ep_len, kill_rate, death_rate, path)
            self.history = []

        def _run_episode(self, seed, record_path=None):
            obs, _ = self._eval_env.reset(seed=seed)
            if record_path is not None:
                self._eval_env.start_recording()
            total, steps, done = 0.0, 0, False
            r = 0.0
            term = trunc = False
            while not done:
                action, _ = self.model.predict(obs, deterministic=True)
                obs, r, term, trunc, _ = self._eval_env.step(action)
                total += r
                steps += 1
                done = term or trunc
            if record_path is not None:
                self._eval_env.save_recording(record_path)
            # term (not trunc) ⇒ a death occurred; sign of the terminal reward
            # says whether the learning agent killed (+) or died (−).
            killed = bool(term) and not bool(trunc) and r > 0.0
            died = bool(term) and not bool(trunc) and r <= 0.0
            return total, steps, killed, died

        def _evaluate(self):
            self._gen += 1
            rec_path = os.path.join(run_dir, f"{gen_prefix}eval_gen{self._gen}.txt")
            rewards, lengths, kills, deaths = [], [], 0, 0
            # Episode 0 of the eval batch is the one we record.
            for i in range(n_eval_episodes):
                rp = rec_path if i == 0 else None
                total, steps, killed, died = self._run_episode(record_seed + i, rp)
                rewards.append(total)
                lengths.append(steps)
                kills += int(killed)
                deaths += int(died)
            mean_r = float(np.mean(rewards))
            mean_len = float(np.mean(lengths))
            kill_rate = kills / n_eval_episodes
            death_rate = deaths / n_eval_episodes
            term_rate = (kills + deaths) / n_eval_episodes
            self.logger.record("eval/mean_reward", mean_r)
            self.logger.record("eval/mean_ep_len", mean_len)
            self.logger.record("eval/kill_rate", kill_rate)
            self.logger.record("eval/death_rate", death_rate)
            self.logger.record("eval/term_rate", term_rate)
            self.logger.record("eval/gen", self._gen)
            self.history.append((int(self.num_timesteps), mean_r, mean_len,
                                 kill_rate, death_rate, rec_path))
            print(f"[eval] gen{self._gen} t={self.num_timesteps} "
                  f"mean_reward={mean_r:.2f} mean_ep_len={mean_len:.1f} "
                  f"kills={kills}/{n_eval_episodes} deaths={deaths}/{n_eval_episodes} "
                  f"recorded={os.path.basename(rec_path)}")

        def _on_step(self) -> bool:
            # Anchor the first eval boundary to wherever training starts (a
            # resumed model's num_timesteps is already high) so a cross-process
            # resume does not fire a burst of catch-up evals.
            if self._next_eval is None:
                self._next_eval = self.num_timesteps + eval_every
            if self.num_timesteps >= self._next_eval:
                self._evaluate()
                self._next_eval += eval_every
            return True

    return _Cb()


def build_venv(args, reward_config):
    """Build the vectorized, monitored training env for one phase's weights."""
    from stable_baselines3.common.vec_env import DummyVecEnv, VecMonitor

    venv = DummyVecEnv([
        make_env(args.max_ticks, args.frame_skip, reward_config=reward_config,
                 seed=args.seed + i)
        for i in range(args.n_envs)
    ])
    return VecMonitor(venv)


def new_model(venv, args):
    """Fresh PPO with the harness's hyperparameters (design §6, T6 note §1)."""
    from stable_baselines3 import PPO

    return PPO(
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


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    # --total-steps is the run-2 name; --timesteps kept as a back-compat alias.
    ap.add_argument("--total-steps", "--timesteps", dest="total_steps",
                    type=int, default=400_000,
                    help="Total env steps for this invocation (split across "
                         "phases when --anneal).")
    ap.add_argument("--resume", type=str, default=None,
                    help="Path to a saved model .zip to continue from "
                         "(reset_num_timesteps=False).")
    ap.add_argument("--anneal", action="store_true",
                    help="Run the built-in ANNEAL_PHASES reward schedule "
                         "in-process (fresh vec-env + model.set_env per phase).")
    ap.add_argument("--reward-weights", type=str, default=None,
                    help="Single-phase reward-weight overrides, e.g. "
                         "'w_kill=25,w_damage_dealt=0.2'. Ignored with --anneal.")
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

    from stable_baselines3 import PPO
    from stable_baselines3.common.logger import configure

    run_dir = args.run_dir or os.path.join(
        "runs", f"ppo_{time.strftime('%Y%m%d_%H%M%S')}")
    Path(run_dir).mkdir(parents=True, exist_ok=True)
    print(f"run_dir: {run_dir}")

    t0 = time.time()

    if args.anneal:
        # In-process stepwise annealing: fresh vec-env per phase, one model
        # carried across phases via set_env; timesteps keep counting.
        phases = ANNEAL_PHASES
        per_phase = args.total_steps // len(phases)
        model = None
        eval_cb = None
        for i, phase in enumerate(phases):
            rc = phase["weights"]
            venv = build_venv(args, rc)
            if model is None:
                if args.resume:
                    print(f"resume from {args.resume}")
                    model = PPO.load(args.resume, env=venv)
                else:
                    model = new_model(venv, args)
                model.set_logger(configure(run_dir, ["stdout", "csv"]))
                eval_cb = make_eval_callback(
                    run_dir=run_dir, eval_every=args.eval_every,
                    max_ticks=args.max_ticks, frame_skip=args.frame_skip,
                    reward_config=rc)
            else:
                model.set_env(venv)
                # Eval env must track the current phase's weights.
                eval_cb._eval_env = eval_cb._eval_env.__class__(
                    max_ticks=args.max_ticks, frame_skip=args.frame_skip,
                    reward_config=rc)
            print(f"\n=== phase {i+1}/{len(phases)}: {phase['name']} "
                  f"weights={rc or 'default'} steps={per_phase} ===")
            model.learn(total_timesteps=per_phase, callback=eval_cb,
                        reset_num_timesteps=(i == 0 and not args.resume),
                        progress_bar=False)
    else:
        # Single-phase run (optionally resumed). This is the T6-compatible path.
        reward_config = parse_reward_weights(args.reward_weights)
        venv = build_venv(args, reward_config)
        if args.resume:
            print(f"resume from {args.resume}")
            model = PPO.load(args.resume, env=venv)
        else:
            model = new_model(venv, args)
        model.set_logger(configure(run_dir, ["stdout", "csv"]))
        eval_cb = make_eval_callback(
            run_dir=run_dir, eval_every=args.eval_every,
            max_ticks=args.max_ticks, frame_skip=args.frame_skip,
            reward_config=reward_config)
        print(f"\n=== single phase: weights={reward_config or 'default'} "
              f"steps={args.total_steps} resume={bool(args.resume)} ===")
        model.learn(total_timesteps=args.total_steps, callback=eval_cb,
                    reset_num_timesteps=not args.resume, progress_bar=False)

    dt = time.time() - t0

    model.save(os.path.join(run_dir, "ppo_final"))
    print(f"\ntrained {args.total_steps} steps in {dt/60:.1f} min "
          f"({args.total_steps/dt:.0f} steps/s)")
    print(f"model:   {os.path.join(run_dir, 'ppo_final.zip')}")
    print(f"csv:     {os.path.join(run_dir, 'progress.csv')}")
    if eval_cb is not None and eval_cb.history:
        last = eval_cb.history[-1]
        print(f"last eval recording: {last[5]}  (mean_reward={last[1]:.2f}, "
              f"kill_rate={last[3]:.2f})")
        print("watch it:  cargo run -p game -- --replay " + last[5])


if __name__ == "__main__":
    main()
