"""Frozen-snapshot self-play rotation (run 3) — a **runnable 1-deep league**.

Run 2 exhausted the fixed `RandomOpponent`: 8/8 kills, 0/8 deaths for its last
2M steps (run-2 note §3.4). The only remaining axis is a *stronger opponent*, so
this script turns the `LieroGymEnv(opponent=...)` seam (design §6) into real
self-play: the live PPO trains against a **frozen snapshot of a past policy**,
and whenever it beats that snapshot decisively the snapshot is rotated forward to
the current policy. That is the standard cheap self-play recipe — a 1-deep league
(a full pool-of-snapshots league stays the deferred `self-play league` track).

    generation 0 opponent := run-2 final policy (runs/run2_phase3/ppo_final.zip)
    live policy           := run-2 final policy  (continual — NOT scratch)

    repeat:
        train live `--steps-per-gen` steps vs the frozen opponent
        eval  live (deterministic) vs opponent (deterministic), N fixed-seed eps
        if win-rate > `--promote-threshold`:
            freeze current live weights as the next-generation opponent

Why the opponent obs seam is correct (verified against `src/obs.rs`)
--------------------------------------------------------------------
`observe(state, agent_idx)` is fully **egocentric**: the SELF block reads worm
`agent_idx`, the OPPONENT block reads worm `1-agent_idx`, relative geometry is
`opp - self` in that worm's frame (pinned by `observe_is_egocentric_per_agent`).
`LieroGymEnv` feeds the opponent callable `obs_pair[self._opp]` — worm 1's own
egocentric obs — so a policy trained as worm 0 on `obs_pair[0]` drops in as the
opponent on `obs_pair[1]` unchanged and behaves correctly (it always sees itself
as "self"). The seam is symmetric; no wrapper or engine change is needed.

Stochastic opponent while training, deterministic for eval
----------------------------------------------------------
The **training** opponent samples its Bernoulli action head
(`deterministic=False`). Motivation: a `deterministic=True` opponent emits a
single fixed action stream per seed, which the learner can overfit to *exploit*
(memorise one exact sequence) rather than learning robust play; sampling gives a
varied-but-competent target, the standard self-play robustness choice. **Eval**
uses `deterministic=True` on both sides so the win/loss/draw win-rate is a clean,
reproducible, comparable number across generations.

Reward weights: the run-2 phase-3 **sparse** schedule throughout
(`w_damage_dealt=0.2, w_damage_taken=0.1, w_kill=25, w_death=25`, `w_time` at its
0.001 default) — proven stable in run 2 and the currency self-play is judged in
(kill/death, not damage farming). Absolute `ep_rew` is expected to *drop hard* at
gen 0 (the opponent finally fights back); judge by win-rate / ep_len, not reward.

Per-generation resumable
------------------------
Each generation persists everything to `--run-dir` immediately — the live model
(`live.zip`), any promoted opponent snapshots (`opponent_gen<k>.zip`), the metric
history and rotation state (`state.json`), and one fixed-seed eval recording
(`run3-round<r>-vs-gen<k>.txt`, replayable via `cargo run -p game -- --replay`).
`--gens` controls how many generations one invocation runs; re-invoking with the
same `--run-dir` continues from the last completed generation (so a long rotation
splits across several foreground training calls). The first invocation seeds both
the live model and the gen-0 opponent from `--init`.

Run (from repo root, in the pinned venv after `maturin develop`):

    rust/liero-env/.venv/bin/python rust/liero-env/examples/self_play.py \
        --run-dir runs/run3_selfplay \
        --init    runs/run2_phase3/ppo_final.zip \
        --steps-per-gen 150000 --gens 3

Everything it writes lands under `--run-dir` (gitignored); only this example, the
training note, and a few representative eval recordings are committed.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import time
from pathlib import Path

import numpy as np

# Run-2 phase-3 sparse reward weights — the proven-stable schedule self-play runs
# under throughout (run-2 note §5.1). Only these keys are overridden; `w_time`
# keeps its 0.001 default.
SPARSE_WEIGHTS = {
    "w_damage_dealt": 0.2,
    "w_damage_taken": 0.1,
    "w_kill": 25.0,
    "w_death": 25.0,
}


class FrozenPolicyOpponent:
    """Wrap a frozen SB3 policy snapshot as a `LieroGymEnv` opponent callable.

    The snapshot is a dedicated, never-trained PPO loaded from a saved `.zip`
    (`from_zip`). Because it is a separate model, updates to the live policy do
    not leak into the opponent — the frozen-target stability the self-play recipe
    relies on. It is fed the opponent worm's own egocentric observation (see the
    module docstring) and returns a length-`N_ACTION_BITS` `MultiBinary` action.

    `deterministic` selects the action head mode: `False` (sampled) for a
    robust-but-varied training target, `True` (argmax) for reproducible eval.
    """

    def __init__(self, model, deterministic=False):
        self._model = model
        self._deterministic = deterministic

    @classmethod
    def from_zip(cls, path, deterministic=False):
        """Load a frozen opponent from a saved SB3 model `.zip` (CPU — the
        opponent is a per-tick forward pass, no training, so GPU is pointless
        and CPU keeps it colocated with the DummyVecEnv worker)."""
        from stable_baselines3 import PPO

        return cls(PPO.load(path, device="cpu"), deterministic=deterministic)

    def refresh(self, live_model):
        """Rotate the frozen opponent forward to the live policy's weights via
        SB3's own `get_parameters` / `set_parameters` (which clone the tensors —
        NOT `copy.deepcopy`, which raises on a policy holding non-leaf tensors
        after a forward pass)."""
        self._model.set_parameters(live_model.get_parameters(), exact_match=True)

    def __call__(self, obs):
        action, _ = self._model.predict(obs, deterministic=self._deterministic)
        return np.asarray(action, dtype=np.int8)

    def reset(self, seed=None):  # noqa: ARG002
        # A frozen policy needs no per-episode reseeding; a no-op keeps
        # `LieroGymEnv.reset`'s opponent-reset passthrough happy.
        return None


def make_env(max_ticks, frame_skip, opponent, seed=None):
    """Factory for one `LieroGymEnv` vs the given frozen opponent, sparse weights.

    Imported lazily inside the closure so `SubprocVecEnv` (if ever used) imports
    the compiled extension cleanly per worker; `DummyVecEnv` is unaffected.
    """
    from liero_env import LieroGymEnv

    def _init():
        env = LieroGymEnv(max_ticks=max_ticks, frame_skip=frame_skip,
                          reward_config=dict(SPARSE_WEIGHTS), opponent=opponent)
        if seed is not None:
            env.reset(seed=seed)
        return env

    return _init


def build_venv(args, opponent):
    """The vectorized, monitored training env — all sub-envs share one frozen
    opponent object (DummyVecEnv steps them sequentially in-process, and the
    opponent call is stateless, so sharing is safe and avoids N model copies)."""
    from stable_baselines3.common.vec_env import DummyVecEnv, VecMonitor

    venv = DummyVecEnv([
        make_env(args.max_ticks, args.frame_skip, opponent, seed=args.seed + i)
        for i in range(args.n_envs)
    ])
    return VecMonitor(venv)


def new_model(venv, args):
    """Fresh PPO with the harness's run-2 hyperparameters (only used if `--init`
    is absent — normally self-play continues the run-2 final policy)."""
    from stable_baselines3 import PPO

    return PPO("MlpPolicy", venv, n_steps=args.n_steps, batch_size=args.batch_size,
               n_epochs=args.n_epochs, learning_rate=args.lr, ent_coef=args.ent_coef,
               gamma=args.gamma, seed=args.seed, verbose=1)


def evaluate(live_model, opponent_path, args, record_path):
    """Play `--eval-episodes` fixed-seed episodes of the live policy
    (deterministic) vs a deterministic view of the current frozen opponent, and
    classify each as win / loss / draw.

    An episode ends only on a death (terminated) or a max-tick truncation
    (draw). On a death the learning agent's terminal reward is `+w_kill` if it
    killed the opponent, `−w_death` if it died — that sparse terminal dominates
    the tiny per-step damage/time terms, so the SIGN classifies win vs loss
    (same attribution the run-2 eval callback proved). Episode 0 (fixed seed) is
    recorded to `record_path` for `--replay` inspection.
    """
    from liero_env import LieroGymEnv

    opp = FrozenPolicyOpponent.from_zip(opponent_path, deterministic=True)
    env = LieroGymEnv(max_ticks=args.max_ticks, frame_skip=args.frame_skip,
                      reward_config=dict(SPARSE_WEIGHTS), opponent=opp)

    wins = losses = draws = 0
    lengths = []
    for i in range(args.eval_episodes):
        obs, _ = env.reset(seed=args.eval_seed0 + i)
        if i == 0 and record_path is not None:
            env.start_recording()
        r, term, trunc, steps, done = 0.0, False, False, 0, False
        while not done:
            action, _ = live_model.predict(obs, deterministic=True)
            obs, r, term, trunc, _ = env.step(action)
            steps += 1
            done = term or trunc
        if i == 0 and record_path is not None:
            env.save_recording(record_path)
        lengths.append(steps)
        if term and not trunc:
            if r > 0.0:
                wins += 1
            else:
                losses += 1
        else:
            draws += 1

    n = args.eval_episodes
    return {
        "episodes": n,
        "wins": wins, "losses": losses, "draws": draws,
        "win_rate": wins / n, "loss_rate": losses / n, "draw_rate": draws / n,
        "mean_ep_len": float(np.mean(lengths)),
        # kills == wins, deaths == losses under the sparse terminal semantics.
        "kills": wins, "deaths": losses,
    }


def load_state(run_dir):
    p = os.path.join(run_dir, "state.json")
    if os.path.exists(p):
        with open(p) as f:
            return json.load(f)
    return None


def save_state(run_dir, state):
    p = os.path.join(run_dir, "state.json")
    tmp = p + ".tmp"
    with open(tmp, "w") as f:
        json.dump(state, f, indent=2)
    os.replace(tmp, p)


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--run-dir", type=str, default=None,
                    help="Rotation state + artifacts dir (default runs/run3_<ts>).")
    ap.add_argument("--init", type=str, default="runs/run2_phase3/ppo_final.zip",
                    help="Initial checkpoint seeding BOTH the live model and the "
                         "gen-0 opponent (first invocation only).")
    ap.add_argument("--steps-per-gen", type=int, default=150_000,
                    help="Env steps of training per generation.")
    ap.add_argument("--gens", type=int, default=1,
                    help="Generations to run THIS invocation (resumable across "
                         "invocations via --run-dir).")
    ap.add_argument("--promote-threshold", type=float, default=0.60,
                    help="Win-rate over the frozen opponent above which the live "
                         "policy is frozen as the next-generation opponent.")
    ap.add_argument("--eval-episodes", type=int, default=20)
    ap.add_argument("--eval-seed0", type=int, default=7_777,
                    help="First eval-episode seed (fixed across gens => "
                         "comparable win-rates; episode 0 is the recorded one).")
    ap.add_argument("--n-envs", type=int, default=8)
    ap.add_argument("--n-steps", type=int, default=512)
    ap.add_argument("--batch-size", type=int, default=256)
    ap.add_argument("--n-epochs", type=int, default=10)
    ap.add_argument("--lr", type=float, default=3e-4)
    ap.add_argument("--ent-coef", type=float, default=0.01)
    ap.add_argument("--gamma", type=float, default=0.99)
    ap.add_argument("--max-ticks", type=int, default=1200)
    ap.add_argument("--frame-skip", type=int, default=4)
    ap.add_argument("--seed", type=int, default=0)
    ap.add_argument("--sketch", action="store_true",
                    help="Run the tiny plumbing smoke (a few steps) and exit.")
    args = ap.parse_args()

    if args.sketch:
        sketch()
        return

    from stable_baselines3 import PPO

    run_dir = args.run_dir or os.path.join(
        "runs", f"run3_{time.strftime('%Y%m%d_%H%M%S')}")
    Path(run_dir).mkdir(parents=True, exist_ok=True)

    state = load_state(run_dir)
    if state is None:
        # First invocation: seed live + gen-0 opponent from --init.
        init_path = args.init
        if not os.path.exists(init_path):
            raise SystemExit(f"--init checkpoint not found: {init_path}")
        live_path = os.path.join(run_dir, "live.zip")
        opp0_path = os.path.join(run_dir, "opponent_gen0.zip")
        shutil.copyfile(init_path, live_path)
        shutil.copyfile(init_path, opp0_path)
        state = {
            "run_dir": run_dir,
            "round": 0,          # completed training rounds
            "opp_gen": 0,        # generation of the current frozen opponent
            "live_path": live_path,
            "opponent_path": opp0_path,
            "history": [],       # per-round metric dicts
        }
        save_state(run_dir, state)
        print(f"[init] live+gen0 opponent := {init_path}")

    print(f"run_dir: {run_dir}  (starting at round {state['round']}, "
          f"opponent gen {state['opp_gen']})")

    for _ in range(args.gens):
        r = state["round"]
        opp_gen = state["opp_gen"]
        opp_path = state["opponent_path"]

        # --- train the live policy against the frozen opponent (stochastic) ---
        train_opp = FrozenPolicyOpponent.from_zip(opp_path, deterministic=False)
        venv = build_venv(args, train_opp)
        model = PPO.load(state["live_path"], env=venv, device="cpu")
        print(f"\n=== round {r}: train {args.steps_per_gen} steps vs opponent "
              f"gen{opp_gen} ===")
        t0 = time.time()
        model.learn(total_timesteps=args.steps_per_gen, reset_num_timesteps=False,
                    progress_bar=False)
        dt = time.time() - t0
        model.save(state["live_path"])
        sps = args.steps_per_gen / dt

        # --- eval live vs the same opponent (both deterministic) ---
        rec_path = os.path.join(run_dir, f"run3-round{r}-vs-gen{opp_gen}.txt")
        metrics = evaluate(model, opp_path, args, rec_path)
        metrics.update({"round": r, "opp_gen": opp_gen,
                        "train_steps": args.steps_per_gen,
                        "steps_per_s": round(sps, 1),
                        "recording": os.path.basename(rec_path)})

        print(f"[eval] round{r} vs gen{opp_gen}: "
              f"W/L/D={metrics['wins']}/{metrics['losses']}/{metrics['draws']} "
              f"win_rate={metrics['win_rate']:.2f} "
              f"ep_len={metrics['mean_ep_len']:.1f} "
              f"({sps:.0f} steps/s, {dt/60:.1f} min)")

        # --- rotate the snapshot if the live policy beat it decisively ---
        promoted = metrics["win_rate"] > args.promote_threshold
        if promoted:
            new_gen = opp_gen + 1
            new_opp_path = os.path.join(run_dir, f"opponent_gen{new_gen}.zip")
            shutil.copyfile(state["live_path"], new_opp_path)
            state["opp_gen"] = new_gen
            state["opponent_path"] = new_opp_path
            print(f"[promote] win_rate {metrics['win_rate']:.2f} > "
                  f"{args.promote_threshold:.2f} -> froze gen{new_gen} opponent")
        else:
            print(f"[hold] win_rate {metrics['win_rate']:.2f} <= "
                  f"{args.promote_threshold:.2f} -> opponent stays gen{opp_gen}")
        metrics["promoted"] = promoted

        state["round"] = r + 1
        state["history"].append(metrics)
        save_state(run_dir, state)

    print(f"\n[done] {args.gens} generation(s) this invocation. "
          f"round={state['round']} opp_gen={state['opp_gen']}")
    print(f"state:   {os.path.join(run_dir, 'state.json')}")
    if state["history"]:
        last = state["history"][-1]
        print("watch last eval:  cargo run -p game -- --replay "
              + os.path.join(run_dir, last["recording"]))


def sketch():
    """Tiny plumbing smoke: seam wired for a few steps (proof-of-plumbing only)."""
    from stable_baselines3 import PPO
    from stable_baselines3.common.vec_env import DummyVecEnv, VecMonitor

    from liero_env import LieroGymEnv

    def _env(**kw):
        return LieroGymEnv(max_ticks=200, frame_skip=4, **kw)

    frozen = PPO("MlpPolicy", DummyVecEnv([_env]), n_steps=64, batch_size=64,
                 verbose=0)
    opponent = FrozenPolicyOpponent(frozen, deterministic=False)
    venv = VecMonitor(DummyVecEnv([lambda: _env(opponent=opponent)]))
    model = PPO("MlpPolicy", venv, n_steps=128, batch_size=64, verbose=0)
    for it in range(2):
        model.learn(total_timesteps=256, reset_num_timesteps=False)
        opponent.refresh(model)
        print(f"[self-play sketch] iter {it}: refreshed frozen opponent")
    print("self-play sketch wired OK.")


if __name__ == "__main__":
    main()
