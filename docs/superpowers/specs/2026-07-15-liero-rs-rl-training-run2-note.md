# Liero-rs RL — run 2 training note: 3M steps with reward annealing

Status: **DONE** · 2026-07-15 · track RL-HARNESS, follow-on to T6
Previous: `docs/superpowers/specs/2026-07-13-liero-rs-rl-harness-t6-training-note.md`
Script: `rust/liero-env/examples/train_ppo_sb3.py` (extended: `--total-steps`, `--resume`, `--anneal`, `--reward-weights`, eval kill/death metrics)
Eval samples (committed, replayable): `docs/superpowers/eval_samples/2026-07-15-run2-eval-{early-t500k,mid-t2200k,late-t3000k}.txt`

Run 2 executes the T6 note's §5 recommendation #1: **continue training to ~3M
total steps and anneal the dense damage shaping toward the sparse kill/death
signal**, to convert the T6 damage-race winner (which plateaued at ep_rew ~80
with `Kills: 0` in its recorded eval and a terrain-burrowing camping habit)
into a *finisher*.

**Headline result: the agent now kills.** Final-phase eval is 8/8 kills, 0/8
deaths across all generations, and the recorded final eval episode closes out
the opponent in 112 sim-ticks (vs 1036 in the T6 recording). The burrowing
degeneration is gone from the recorded evals.

---

## 1. Setup — stepwise annealing across three phases

Reward weights are constructor-time state in `RawEnv` (they cannot change
mid-run), so annealing is **stepwise**: each phase is a separate process with
its own weights, resumed from the previous phase's checkpoint
(`PPO.load(..., env=new_venv)`, `reset_num_timesteps=False` — timesteps keep
counting across phases). Everything else is the T6 configuration unchanged
(8×DummyVecEnv + VecMonitor, MlpPolicy, MultiBinary(7), n_steps 512, batch 256,
lr 3e-4, ent_coef 0.01, γ 0.99, max_ticks 1200, frame_skip 4, seed 0).
Eval every 100k steps: 8 deterministic episodes, fixed seeds 7777..7784 —
identical across all generations and phases, so generations are comparable.

| Phase | Steps (cumulative t) | w_damage_dealt | w_damage_taken | w_kill | w_death | w_time |
|---|---|---|---|---|---|---|
| T6 (prior run) | 0 → 0.4M | 1.0 | 0.5 | 10 | 10 | 0.001 |
| 1 `shaped` (resume T6) | 0.4M → 1.4M | 1.0 | 0.5 | 10 | 10 | 0.001 |
| 2 `mid` | 1.4M → 2.2M | 0.6 | 0.3 | 17 | 17 | 0.001 |
| 3 `sparse` | 2.2M → 3.0M | 0.2 | 0.1 | 25 | 25 | 0.001 |

The arc inverts the T6 dominance: in phase 1 a full HP bar of chip damage
(=100) is worth 10× a kill; in phase 3 it is worth 20 — *less than one kill
(25)* — so finishing, not farming, is what pays. Total new steps: 2.6M;
total with T6: **3.0M**. Wall time ~23 min (~1900 steps/s, M2 Max CPU).
The schedule also ships as `--anneal` (in-process phases via `model.set_env`);
this run used the equivalent per-process `--resume` + `--reward-weights` path.

**Scale warning:** absolute reward changes scale at each phase boundary
(the weights define the scale). Compare `ep_rew_mean` only *within* a phase.
The weight-independent cross-phase signals are `ep_len_mean` (episodes end on
a death, so shorter ⇒ kills are happening) and the eval kill/death rates
(new in this run's eval callback — attribution via the terminal reward's sign:
`+w_kill` vs `−w_death` dominates the final step).

## 2. Curves per phase (from each phase's `progress.csv`)

Phase 1 — shaped (resume from T6's plateau ~80):

```
  t=  405504  ep_rew=  82.72  ep_len= 187.9   <- T6 end state
  t=  614400  ep_rew=  83.60  ep_len= 176.1
  t=  753664  ep_rew=  93.77  ep_len= 164.9
  t=  892928  ep_rew=  96.70  ep_len= 138.3
  t= 1032192  ep_rew=  97.19  ep_len= 116.0
  t= 1171456  ep_rew= 103.23  ep_len= 102.0
  t= 1310720  ep_rew= 103.27  ep_len=  97.0
  t= 1404928  ep_rew= 102.49  ep_len=  98.2   <- phase end
```

Phase 2 — mid weights (reward re-scales, ~89 → settles ~73):

```
  t= 1409024  ep_rew=  89.28  ep_len=  94.8
  t= 1642496  ep_rew=  70.53  ep_len=  82.9
  t= 1875968  ep_rew=  70.09  ep_len=  78.2
  t= 2109440  ep_rew=  71.30  ep_len=  78.8
  t= 2207744  ep_rew=  73.40  ep_len=  74.2   <- phase end
```

Phase 3 — sparse weights (reward re-scales again, settles ~36):

```
  t= 2211840  ep_rew=  53.36  ep_len=  69.7
  t= 2445312  ep_rew=  35.49  ep_len=  76.7
  t= 2678784  ep_rew=  35.75  ep_len=  71.1
  t= 2912256  ep_rew=  37.71  ep_len=  74.6
  t= 3010560  ep_rew=  36.10  ep_len=  72.1   <- end (3.0M total)
```

The cross-phase story is in **ep_len_mean: 188 → 97 → 74 → ~72**, monotone
down across 2.6M steps — episodes end four times faster than at the T6
plateau because the agent closes them with kills. In phase 3 (~36 mean reward
against a max of ~25+damage−death terms) the level holds flat with no
regression: the kill behavior survives having its damage crutch removed,
which is the point of the anneal.

Deterministic eval (8 fixed-seed episodes / 100k steps; kill/death = new metrics):

```
  phase 1: t= 501416  rew= 73.81  len=251.9  kills=4/8  deaths=0/8
           t= 901416  rew=103.15  len= 98.9  kills=8/8  deaths=0/8
           t=1401416  rew=105.43  len= 72.1  kills=8/8  deaths=0/8
  phase 2: t=1504936  rew= 64.47  len= 62.9  kills=8/8  deaths=0/8
           t=2204936  rew= 73.14  len= 73.8  kills=8/8  deaths=0/8
  phase 3: t=2307752  rew= 36.01  len= 66.1  kills=8/8  deaths=0/8
           t=2607752  rew= 36.79  len=107.1  kills=8/8  deaths=0/8
           t=3007752  rew= 39.71  len= 69.9  kills=8/8  deaths=0/8
```

## 3. Honest findings

1. **The kill breakthrough happened in phase 1, under the UNCHANGED default
   weights.** Kill rate went 4/8 → 8/8 between t=0.5M and t=0.9M with the T6
   shaping still in place. T6's "plateau" at 400k was simply an early stop —
   more steps alone broke it. The anneal did not *create* the kill behavior.
2. **What the anneal did do:** it verified and consolidated the behavior under
   progressively sparser reward — kill rate stayed 8/8 and ep_len kept
   trending down (95→74 in phase 2) as the damage shaping was cut 5× and the
   kill stake more than doubled. A policy that was only chip-damage-farming
   would have degraded in phase 3; this one held flat.
3. **T6's "Kills: 0" was partly a sampling artifact.** The new kill-rate
   metric shows the resumed T6 model already scored 4/8 kills on the eval
   seeds; the T6 note judged from ONE recorded episode (seed 7777) that
   happened to be a burrow-and-camp episode. Per-generation kill/death
   counters now make this visible instead of anecdotal.
4. **RandomOpponent is exhausted.** 8/8 kills, 0/8 deaths, everywhere, for
   the last 2M steps. Nothing above this ceiling is measurable against it.

## 4. Behavior — headless `shot` renders of the committed samples

- **early-t500k** (phase 1 gen1, 716 ticks): the old T6 habit on full display —
  P0 fights from *inside* the terrain (burrowed, tick 500 render), wins the
  damage race, and the kill only lands at the very end of a long episode.
- **mid-t2200k** (phase 2 end, 368 ticks): P0 fights *at the surface*, airborne
  and aggressive, heavy sustained fire; opponent's bar is emptied by mid-episode
  and the episode ends in half the early sample's time. No burrowing.
- **late-t3000k** (phase 3 end, 112 ticks): immediate engagement at the
  surface, opponent dead inside 112 ticks — HUD shows **Kills: 1 / opponent
  Lives: 0**. No burrowing at any rendered tick.

(HUD kill counters update the tick after a death, so a final-tick screenshot of
the early/mid samples can show `Kills: 0` even though the kill landed; the
late sample's final frame is past that boundary and shows `Kills: 1`.)

All three samples parse (`liero_env.scenario_parses == True`) and replay in the
game window without panic (`perl -e 'alarm 6; exec @ARGV'
rust/target/debug/game --replay <sample>` → exit 142, no panic output):

```
cargo run -p game -- --replay docs/superpowers/eval_samples/2026-07-15-run2-eval-late-t3000k.txt
```

## 5. Recommendation for run 3

1. **Stronger opponent — the only remaining axis.** Self-play rotation via the
   `LieroGymEnv(opponent=...)` seam: freeze the run-2 final policy as the
   opponent (a small MLP forward pass as the callable), train against it,
   rotate periodically. `examples/self_play.py` sketches this. Start from the
   run-2 final checkpoint with the phase-3 sparse weights (they are proven
   stable now); expect ep_rew to drop hard at first (the opponent finally
   fights back) — judge by kill/death rate and ep_len, not reward level.
2. **Optional, John-facing:** a live inference bridge so a human can play
   against the policy in the `game` window (export the 43→7 MLP, run it
   Rust-side per tick, e.g. via tract/ort, feeding the existing
   `InputSource` seam). Nothing blocks this architecturally; it is pure glue.
3. Keep `--eval-every 100000` with the kill/death metrics — they, not reward,
   are the cross-run comparison currency from here on.

## 6. Artifacts

- **Committed:** the extended `examples/train_ppo_sb3.py`, this note, three
  replayable eval samples under `docs/superpowers/eval_samples/` (early/mid/late).
- **Gitignored** (`rust/liero-env/runs/run2_phase{1,2,3}/`): per-phase
  `ppo_final.zip` checkpoints, `progress.csv` series, all `eval_gen<N>.txt`.
  The final model is `runs/run2_phase3/ppo_final.zip` — run 3's resume point.
