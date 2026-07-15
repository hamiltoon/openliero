# Liero-rs RL — run 3 training note: self-play, frozen-snapshot rotation (gen 0 → 7)

Status: **DONE** · 2026-07-15 · track RL-HARNESS, follow-on to run 2
Previous: `docs/superpowers/specs/2026-07-15-liero-rs-rl-training-run2-note.md`
Script: `rust/liero-env/examples/self_play.py` (the T6 sketch, now a runnable 1-deep league)
Eval samples (committed, replayable): `docs/superpowers/eval_samples/2026-07-15-run3-gen{0-round0,3-round3,6-round6}.txt`

Run 3 executes the run-2 note's §5.1 recommendation: **replace the exhausted
`RandomOpponent` with self-play** — freeze the run-2 final policy as the
opponent, train against it, and rotate the snapshot forward when the learner
wins decisively. It answers the run-2 ceiling ("8/8 kills, 0/8 deaths for the
last 2M steps — nothing above this is measurable against random") by giving the
agent an opponent that fights back.

**Headline result — with an important asterisk.** The final policy (gen 7) is
**genuinely stronger than the run-2 baseline**: against a *stochastic* (non-
exploitable) run-2 final it wins **60/0** (`win_rate 1.00`). But the
per-generation promotion signal (**7 promotions in 7 rounds, mostly 20/0**)
**overstates** progress — it is contaminated by a worm-slot spawn advantage and
by learned exploitation of the *deterministic* eval opponent, and the
intermediate ladder is **non-transitive** (gen 2 is actually a regression on
gen 0). The matches themselves look **real and healthy**: two competent worms
duelling at the surface, no burrowing/camping/kite-loops.

---

## 1. Setup — a runnable 1-deep league over the frozen-opponent seam

`LieroGymEnv(opponent=...)` takes any `callable(obs) -> MultiBinary(7)` (design
§6). `FrozenPolicyOpponent` (in `self_play.py`) wraps a frozen SB3 snapshot as
that callable. The rotation:

- **Live policy** starts from `runs/run2_phase3/ppo_final.zip` (continual — NOT
  scratch; it must *improve relative to* run 2, not relearn to walk).
- **Gen-0 opponent** = the same run-2 final, frozen.
- Each round: train the live policy `--steps-per-gen` steps against the frozen
  opponent → eval win/loss/draw → if `win_rate > 0.60`, freeze the current live
  weights as the next-generation opponent.

**Obs seam verified against `src/obs.rs`.** `observe(state, agent_idx)` is fully
egocentric (SELF block reads worm `agent_idx`, OPPONENT block reads
`1-agent_idx`, relative geometry `opp-self` in that worm's frame — pinned by
`observe_is_egocentric_per_agent`). `LieroGymEnv` feeds the opponent callable
`obs_pair[self._opp]` — worm 1's *own* egocentric obs — so a policy trained as
worm 0 on `obs_pair[0]` drops in as the opponent on `obs_pair[1]` unchanged and
behaves correctly. The seam is symmetric; no wrapper or engine change was needed.

**Stochastic training opponent, deterministic eval.** The training opponent
samples its Bernoulli head (`deterministic=False`): a `deterministic=True`
opponent emits one fixed action stream per seed, which the learner overfits to
*exploit* rather than learning robust play; sampling gives a varied-but-competent
target (the standard self-play robustness choice). Eval uses `deterministic=True`
on both sides for a clean, reproducible, comparable win-rate. (§4 shows this eval
choice is itself a confound — see the recommendations.)

Reward weights: the run-2 phase-3 **sparse** schedule throughout
(`w_damage_dealt=0.2, w_damage_taken=0.1, w_kill=25, w_death=25`, `w_time` 0.001)
— proven stable in run 2 and the currency self-play is judged in. Everything else
is the run-2 configuration (8×DummyVecEnv + VecMonitor, MlpPolicy, MultiBinary(7),
n_steps 512, batch 256, lr 3e-4, ent_coef 0.01, γ 0.99, max_ticks 1200,
frame_skip 4, seed 0). 20 fixed-seed eval episodes (7777..7796) per round,
identical across rounds so win-rates are comparable. `--promote-threshold 0.60`.

Total: **7 rounds × 250k = 1.75M new steps** (cumulative with run 2: **4.75M**),
~21 min wall (~1350–1590 steps/s, M-series CPU; the opponent forward pass costs
~15% vs run 2's random opponent). The script is **per-generation resumable**
(state + artifacts persisted to `--run-dir` each round), so the run split across
three foreground training calls.

## 2. Per-generation results (from `runs/run3_selfplay/state.json`)

| round | opponent | W / L / D | win-rate | ep_len (env-steps) | promoted |
|---|---|---|---|---|---|
| 0 | gen 0 (run-2 final) | 20 / 0 / 0 | 1.00 | 45.2 | ✔ → gen 1 |
| 1 | gen 1 | 18 / 2 / 0 | 0.90 | 31.2 | ✔ → gen 2 |
| 2 | gen 2 | 20 / 0 / 0 | 1.00 | 37.2 | ✔ → gen 3 |
| 3 | gen 3 | 20 / 0 / 0 | 1.00 | 23.9 | ✔ → gen 4 |
| 4 | gen 4 | 20 / 0 / 0 | 1.00 | 39.5 | ✔ → gen 5 |
| 5 | gen 5 | 20 / 0 / 0 | 1.00 | 20.1 | ✔ → gen 6 |
| 6 | gen 6 | 20 / 0 / 0 | 1.00 | 29.6 | ✔ → gen 7 |

Taken at face value this is a clean monotone ladder: every generation crushes its
predecessor and kills faster (ep_len oscillates 20–45 env-steps ≈ 80–180 sim
ticks, vs run-2's 72 against random). **§4 shows the face value is misleading.**

## 3. Match quality — headless `shot` renders (real matches, no degeneration)

`shot --scenario-path <recording> --hud` renders the recorded eval episodes
headlessly. Both an **early** (round 0, learner vs run-2 final) and a **late**
(round 6, vs gen 6) recording show the same healthy picture:

- **Both worms fight at the surface** — mobile, airborne, aggressive. No
  burrowing, no terrain-camping, no kite-loops (the two failure modes to watch).
  The run-2 anti-burrowing behavior *survived* self-play.
- **Real dart exchanges.** round-0 tick 55: the learner (green, P0) lands a dart
  burst on the opponent (blue, P1) with a visible hit-flash, emptying its health
  bar — a genuine kill, not a farmed timeout.
- Late episodes end fast (~20–30 env-steps) because the learner closes decisively
  on the (deterministic, predictable) opponent.

The matches read as **two competent agents duelling** — the qualitative goal of
self-play was met.

## 4. Honest findings — the promotion signal is contaminated

A round-robin cross-eval (`agent`=row plays worm 0 deterministic, `opponent`=col
worm 1 deterministic, 20 fixed-seed episodes, win-rate for the agent):

```
          opp g0   opp g2   opp g4   opp g7
agent g0   0.45     0.85     0.85     0.50
agent g2   0.20     1.00     1.00     0.95
agent g4   0.95     1.00     1.00     1.00
agent g7   0.70     0.95     1.00     1.00
```

1. **Worm-slot / spawn advantage inflates every win-rate.** The self-play gens
   beat *deterministic copies of themselves* 20/0 (diagonal g2/g4/g7 = 1.00) —
   impossible for genuine equal-skill play. Tellingly **gen 0 vs gen 0 = 0.45**,
   so the run-2 policy did *not* have this; self-play **taught** the agent to
   exploit a predictable worm-1 opponent from the advantaged worm-0 slot. A
   chunk of the "20/0 every round" is this, not skill.

2. **The ladder is non-transitive (it cycles).** `agent g0 vs opp g2 = 0.85`
   while `agent g2 vs opp g0 = 0.20`: **gen 2 is a regression on gen 0**, yet it
   was "promoted" (it beat gen 1's deterministic snapshot). g0 and g4 each "beat"
   the other depending on who holds the deterministic slot — rock-paper-scissors,
   the classic fictitious-self-play cycling signature. Promoting only against the
   immediate predecessor cannot detect this.

3. **But real improvement is there on the robustness axis.** Against a
   **stochastic** (non-exploitable) opponent, the confound clears:

   | matchup | opp deterministic | opp stochastic (3 seed banks) |
   |---|---|---|
   | gen 7 vs gen 0 | 14/3/3 (0.70, ep_len 103) | **60/0/0 (1.00, ep_len 50)** |
   | gen 7 vs gen 4 | 20/0/0 (1.00) | **60/0/0 (1.00)** |

   Against a fair, unpredictable run-2 baseline gen 7 wins **60/0** and kills
   *twice as fast* — genuine skill gain, not just exploitation. (Note the sign
   flip: gen 7 beats the *deterministic* gen 0 only 0.70 — a fixed gen-0 line
   occasionally counters it — but dominates the *stochastic* gen 0 completely.
   The deterministic eval both over- and under-states depending on matchup; it is
   simply the wrong yardstick.)

**Bottom line:** run 3 produced a policy genuinely stronger than run 2 (gen 7 ≫
gen 0 under fair eval), and the matches are healthy real duels — but the
per-generation 20/0 promotion numbers are a contaminated signal (slot advantage +
determinism exploitation) over a non-monotone ladder. The *rotation worked*; the
*measurement* needs fixing before the win-rates can be trusted generation over
generation.

## 5. Recommendations for run 4

1. **Fix the eval/promotion yardstick** (the highest-value change):
   - Promote on win-rate vs a **stochastic** opponent (non-exploitable), and
     **symmetrize the slot** — play the agent as worm 0 *and* worm 1 (or mirror
     seeds) and average, to cancel the spawn advantage that makes a policy beat
     its own clone 20/0.
   - Evaluate against a **pool** of past snapshots, not just the immediate
     predecessor — this is what catches the gen-0-beats-gen-2 cycling.
2. **Train both slots.** The live policy only ever controlled worm 0, so it
   overfits that spawn. Alternating `agent_index` (or a symmetric two-sided
   rollout) would remove the slot bias at the source, not just in eval.
3. **A real league, not 1-deep.** Sample the training opponent from a pool of
   past snapshots (prioritized fictitious self-play) to damp the cycling — the
   deferred `self-play league` track. Keep sparse weights; they held throughout.
4. **Inference bridge (still unblocked, John-facing).** Export the gen-7 (or
   run-2 final) 43→7 MLP and run it Rust-side per tick behind the `game`
   `InputSource` seam, so a human can play the policy in the real window. Gen 7
   is a fine first opponent to ship. Pure glue — nothing architectural blocks it.

## 6. Artifacts

- **Committed:** the runnable `examples/self_play.py` (FrozenPolicyOpponent +
  resumable rotation orchestrator), this note, three replayable eval samples
  under `docs/superpowers/eval_samples/` (`run3-gen0-round0`, `-gen3-round3`,
  `-gen6-round6`). All three parse (`scenario_parses == True`) and replay in the
  game window without panic (`perl -e 'alarm 6; exec @ARGV'
  rust/target/debug/game --replay <sample>` → exit 142, no panic output).
- **Gitignored** (`rust/liero-env/runs/run3_selfplay/`): `live.zip`,
  `opponent_gen{0..7}.zip` (the ladder), `state.json` (metric history), all
  `run3-round<r>-vs-gen<k>.txt` recordings.
- **Suites green:** `cargo test -p liero-env` (32 passed); Python smokes
  (`test_determinism_smoke` 7, `test_t4_wrappers` 8, `test_t5_recording_smoke` 4).
