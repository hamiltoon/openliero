# Liero-rs RL harness — T6 milestone training note

Status: **MILESTONE MET** · 2026-07-13 · track RL-HARNESS, task T6
Plan: `docs/superpowers/plans/2026-07-13-liero-rs-rl-harness-plan.md` (T6)
Design: `docs/superpowers/specs/2026-07-13-liero-rs-rl-harness-pettingzoo-design.md` (§6, §9)
Script: `rust/liero-env/examples/train_ppo_sb3.py`
Eval sample (committed, replayable): `docs/superpowers/eval_samples/2026-07-13-t6-ppo-eval-gen10.txt`

The T6 gate: an off-the-shelf **SB3-PPO** trains a single worm against the frozen
`RandomOpponent` through the Gymnasium wrapper, and (1) its **reward curve rises**
above the random-policy baseline, (2) **one eval recording replays in the window**
(`--replay`). Both demonstrated below with captured numbers, not asserted-by-hope.

---

## 1. Training setup

| Knob | Value | Note |
|---|---|---|
| Algo / policy | PPO / `MlpPolicy` | SB3 2.9.0, torch 2.13.0 (CPU device) |
| Action space | `MultiBinary(7)` | consumed natively by PPO's Bernoulli head (no re-encoding) |
| Obs space | `Box(f32, (43,))` | Rust extractor, ≈unit scale — **no `VecNormalize`** (see §4) |
| Reward | `RewardConfig::default` | `w_damage_dealt=1.0, w_damage_taken=0.5, w_kill=10, w_death=10, w_time=0.001` |
| Envs | 8 × `DummyVecEnv` + `VecMonitor` | `VecMonitor` logs **raw** episodic reward |
| Episode | `max_ticks=1200`, `frame_skip=4` | ≤ 300 steps/episode; episodes end on a death (KillEmAll) or truncate |
| `n_steps` / batch / epochs | 512 / 256 / 10 | 4096 transitions per update, ~98 updates |
| lr / γ / gae_λ / ent_coef / clip | 3e-4 / 0.99 / 0.95 / 0.01 / 0.2 | SB3 defaults + a small entropy bonus for the Bernoulli head |
| Total steps | **400 000** | **3.2 min** wall, **~2100 steps/s** (single M2 Max, CPU) |
| Seed | 0 | reproducible; opponent + env seeds separate from `SimState.rand` (design §5) |

Command (from the repo root, in the pinned venv after `maturin develop`):

```
rust/liero-env/.venv/bin/python \
    rust/liero-env/examples/train_ppo_sb3.py --timesteps 400000 --run-dir runs/ppo_milestone
```

**Random-policy baseline** (same env config, 30 episodes, `--baseline-only`):
`ep_rew mean = 38.72, std = 31.67`. The baseline is already positive because the
default shaping is asymmetric on purpose — dealing damage is worth `1.0`/HP while
taking it costs only `0.5`/HP, so even random trading nets positive (design §4's
"trading damage is a net win"). The bar the learned curve must clear is therefore
**~39**, not 0; the variance (±32) is high, so read the *trend*, not any single row.

---

## 2. The reward curve (rises) — proof

`rollout/ep_rew_mean` (RAW reward via `VecMonitor`), from `runs/ppo_milestone/progress.csv`:

```
  t=   4096  ep_rew_mean= 41.95  ep_len_mean= 154.3   <- start ≈ random baseline (38.7)
  t=  20480  ep_rew_mean= 44.20  ep_len_mean= 159.1
  t=  40960  ep_rew_mean= 57.45  ep_len_mean= 179.7
  t=  61440  ep_rew_mean= 61.75  ep_len_mean= 185.5
  t=  81920  ep_rew_mean= 69.78  ep_len_mean= 193.7
  t= 102400  ep_rew_mean= 67.55  ep_len_mean= 202.5
  t= 122880  ep_rew_mean= 75.66  ep_len_mean= 200.1
  t= 143360  ep_rew_mean= 76.07  ep_len_mean= 207.2
  t= 163840  ep_rew_mean= 72.77  ep_len_mean= 221.5
  t= 184320  ep_rew_mean= 78.37  ep_len_mean= 222.9
  t= 204800  ep_rew_mean= 81.22  ep_len_mean= 209.1
  t= 266240  ep_rew_mean= 82.06  ep_len_mean= 213.2
  t= 348160  ep_rew_mean= 82.92  ep_len_mean= 213.7
  t= 401408  ep_rew_mean= 79.52  ep_len_mean= 202.6   <- end
```

**Trend: ~42 → ~80, roughly doubling and clearly above the ~39 random baseline.**
Shape is the textbook rise-then-plateau: a steep climb over the first ~120k steps,
then a soft plateau in the high-70s/low-80s (a weak random opponent caps how much
there is left to learn). Not monotone — there are dips (e.g. 82→73 around
t=286k) inside the noise band — but the start-to-end direction is unambiguous.

Deterministic eval (`EvalRecordCallback`, 8 episodes every 40k steps, `eval/mean_reward`):

```
  gen1  t=  40960  eval/mean_reward= 68.14
  gen2  t=  81920  eval/mean_reward= 64.33
  gen3  t= 122880  eval/mean_reward= 54.75   <- eval variance is high (8 eps, fixed seeds)
  gen4  t= 163840  eval/mean_reward= 86.62
  gen5  t= 200704  eval/mean_reward= 79.65
  gen6  t= 241664  eval/mean_reward= 63.17
  gen7  t= 282624  eval/mean_reward= 74.25
  gen8  t= 323584  eval/mean_reward= 78.39
  gen9  t= 360448  eval/mean_reward= 84.06
  gen10 t= 401408  eval/mean_reward= 76.10
```

Eval mean also lands well above baseline (mid-70s/80s), noisier than the rollout
mean because it averages only 8 fixed-seed episodes against the ±32-variance signal.

---

## 3. Eval-replay verification

Every eval generation records episode 0 (fixed seed 7777) to
`runs/ppo_milestone/eval_gen<N>.txt` via the env's `start_recording` /
`save_recording` passthrough (plan T5). The final one (`eval_gen10.txt`, 1036 ticks)
is committed as `docs/superpowers/eval_samples/2026-07-13-t6-ppo-eval-gen10.txt`.

- **Parses through the real grammar:** `liero_env.scenario_parses(...) == True`.
- **Replays in the `game` window, no panic:**

  ```
  perl -e 'alarm 6; exec @ARGV' rust/target/debug/game --replay <eval_gen10.txt>
  # -> exit 142 (SIGALRM after 6 s), zero panic lines on stdout/stderr
  ```

  Exit 142 = the Bevy app ran the replay for the full 6 s and was killed by the
  alarm, i.e. it loaded and played the recorded match without crashing.

Watch it live:
`cargo run -p game -- --replay docs/superpowers/eval_samples/2026-07-13-t6-ppo-eval-gen10.txt`

---

## 4. Observed behavior (headless `shot` render of the eval replay)

Rendered the committed recording headlessly at ticks 150/400/700/1000
(`shot --scenario-path <rec> --tick ... --hud`) and eyeballed the frames:

- **t=150** — the learning agent (P0, green worm, left) is at the surface firing;
  the opponent (P1, right) is across the map. Both health bars near full.
- **t=700** — P0's health bar is still long/green; **P1's bar has gone red and
  short** — the agent has out-damaged the random opponent substantially. Heavy
  projectile/blood spray around P0; the terrain is being carved.
- **t=1000** — both worms have **burrowed into the terrain**; P0 keeps a full green
  bar, P1's stays depleted. `Kills: 0` for both throughout.

**Honest read:** the policy learned exactly what the *dense damage shaping* rewards
and no more — maximize `damage_dealt`, minimize `damage_taken` by digging into cover
and spraying fire. It reliably wins the damage race but **does not close out kills**
(`Kills: 0` even with the opponent's HP near zero). This is the design §9
chip-damage-farming / camping tendency showing through, and it is expected
pre-annealing: the `default` config's damage term (a full HP bar = 100) dominates
the sparse `±10` kill/death signal by an order of magnitude (see `reward.rs`
`RewardConfig::default` docs). The curve rises because the agent genuinely gets
better at the *shaped* objective — which is the milestone — but "better at the
shaped objective" is not yet "better at winning".

---

## 5. Deviations & what the next iteration should change

- **No `VecNormalize`.** Kept out on purpose: the Rust obs is already ≈unit-scale
  (empirically `obs ∈ ~[-1, 1]`, a few dims tighter), so `MlpPolicy` trains directly
  and the eval-recording env is the *same* `LieroGymEnv` with no shared normalization
  stats to thread from train → watch. One fewer moving part; the curve rose fine
  without it.
- **No tensorboard dependency.** `tensorboard` is not pinned in `constraints.txt`,
  so the logger is `stdout` + `csv` only; `progress.csv` is the committed-note's
  data source. (The script exposes `--run-dir`; tensorboard can be re-added locally.)
- **Next iteration (deferred, tracked):**
  1. **Anneal the shaping toward the sparse kill/death signal** (lower
     `w_damage_dealt`/`w_damage_taken`, keep `w_kill`/`w_death`) once the agent can
     deal damage — to convert the damage-race winner into a *finisher* and kill the
     camping tendency. This is the design §4 "shaped-then-annealed" path, Python-side.
  2. **A stronger opponent** (scripted aggressor, or self-play rotation — the
     `examples/self_play.py` sketch's frozen-snapshot seam) to lift the plateau; the
     random opponent caps learning in the high-70s.
  3. Longer run / `SubprocVecEnv` for true parallelism if throughput ever binds
     (it did not here — 2100 steps/s over 8 `DummyVecEnv` envs was ample).

## 6. Artifacts

- **Committed:** `examples/train_ppo_sb3.py`, this note, `.gitignore` (`runs/`),
  and one replayable eval recording (`docs/superpowers/eval_samples/2026-07-13-t6-ppo-eval-gen10.txt`).
- **Gitignored** (`rust/liero-env/runs/`): the PPO model (`ppo_final.zip`),
  `progress.csv`, and all `eval_gen<N>.txt` — a run is reproducible from the script.
