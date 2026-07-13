//! Shaped per-agent reward over `(prev, next)` [`WormState`] deltas (design §4).
//!
//! `reward()` is a small pure function: given a *before* snapshot
//! ([`WormSnapshot`], captured before `LieroEnv::step`) and the *after*
//! [`SimState`] (the live state right after `step`), plus Python-tunable
//! [`RewardConfig`] weights, it returns one agent's scalar reward for that
//! step. Keeping the weights out of the Rust code means reward tuning /
//! annealing (design §4's "shaped-then-annealed") is a Python-side experiment
//! with no recompile (plan T2).
//!
//! ## Attribution in the symmetric 1v1
//! The sim exposes exact **kill** attribution (`WormState::kills` is
//! incremented on the credited killer worm itself inside `process_frame`, not
//! derived here) and exact **own-health** deltas, but it does not expose a
//! per-hit *damage source* for ordinary (non-lethal) chip damage — only
//! [`WormState::last_killed_by_idx`] attributes the final, lethal hit. In the
//! symmetric two-agent match this crate is scoped to (design §5, `N_WORMS ==
//! 2`), "opponent health decrease" (design §4's `damage_dealt`) is read
//! directly off the opponent's own health delta: any decrease in worm B's
//! health is credited to worm A's `damage_dealt` reward term (and
//! symmetrically for B). Self-inflicted splash damage is a known, documented
//! edge case this simplification does not separate out — the design's own
//! per-hit attribution table does not exist in the current sim surface, so
//! this is the minimal reading that needs no `sim` changes (a hard constraint,
//! design §6).
//!
//! ## `frame_skip` batches deltas over the WHOLE step, not per tick (T1 review)
//! `LieroEnv::step` may advance `frame_skip > 1` sim ticks per env step
//! (action-repeat, design §2). `reward()` only ever sees the snapshot from
//! *before* that whole batch and the state from *after* it — never the
//! intermediate per-tick states. This is deliberately the CORRECT aggregation,
//! not a gap to patch: health decreases monotonically-or-holds across a death
//! (no mid-batch healing in KillEmAll without a respawn), so summing via a
//! single before/after subtraction gives the exact total damage across
//! however many ticks the batch spanned, and a `health <= 0` transition is
//! detected exactly once no matter how many of the batch's ticks come after
//! the lethal one (see
//! [`reward_aggregates_multi_tick_damage_across_a_frame_skip_batch_including_death`]
//! below for the RED test pinning this).

use sim::state::{SimState, WormState};

/// The minimal per-worm snapshot `reward()` needs to compute deltas from —
/// `health`/`lives`/`kills`, captured before a `step` (design §4: "a small
/// pure Rust function over `(prev, next)` `WormState` snapshots").
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct WormSnapshot {
    pub health: i32,
    pub lives: i32,
    pub kills: i32,
}

impl From<&WormState> for WormSnapshot {
    fn from(w: &WormState) -> Self {
        WormSnapshot {
            health: w.health,
            lives: w.lives,
            kills: w.kills,
        }
    }
}

/// Snapshot every worm in `state` — the `prev` argument `reward()` expects,
/// captured by the caller (the env's `step` wrapper, T3) before advancing the
/// sim.
pub fn snapshot_all(state: &SimState) -> Vec<WormSnapshot> {
    state.worms.iter().map(WormSnapshot::from).collect()
}

/// Reward-shaping weights (design §4), Python-tunable (plan T2/T3) — no
/// recompile needed to retune. `Default` gives small, documented starting
/// values; annealing (shifting weight toward the sparse `±1` kill/death
/// objective as training matures) is a Python-side concern (design §4).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RewardConfig {
    /// Weight on the opponent's health decrease this step (design §4:
    /// `+ w_dmg · damage_dealt`).
    pub w_damage_dealt: f32,
    /// Weight on this agent's own health decrease this step (design §4:
    /// `− w_taken · damage_taken`, `w_taken < w_dmg` so trading damage is a net
    /// win — chip-damage farming pitfall, design §4).
    pub w_damage_taken: f32,
    /// Flat bonus the step this agent's `kills` counter increments (design §4:
    /// `+ KILL`).
    pub w_kill: f32,
    /// Flat penalty the step this agent's own health transitions to `<= 0`
    /// (design §4: `− DEATH`).
    pub w_death: f32,
    /// Tiny flat per-step penalty (design §4: `− w_time`) — forces engagement,
    /// bounds stalling.
    pub w_time: f32,
}

impl Default for RewardConfig {
    /// `w_damage_dealt > w_damage_taken` (trading damage nets positive, but
    /// taking it is discouraged, design §4). Note the shaped **damage** term
    /// deliberately DOMINATES early: a full health bar of chip damage is worth
    /// `100 * w_damage_dealt = 100`, an order of magnitude above a single
    /// `w_kill`/`w_death = 10` terminal event — so dense damage shaping is what
    /// drives learning at the start, by design. The sparse kill/death objective
    /// only takes over as the caller **anneals** the shaping weights down toward
    /// the `±1` kill/death signal (design §4's "shaped-then-annealed"); that
    /// annealing is the Python-side path, not a property of these starting
    /// values. `w_time` is deliberately tiny — one full stalled 3000-tick
    /// episode (`LieroEnv::DEFAULT_MAX_TICKS`) costs `3000 * 0.001 = 3.0`, well
    /// under a single kill/death.
    fn default() -> Self {
        RewardConfig {
            w_damage_dealt: 1.0,
            w_damage_taken: 0.5,
            w_kill: 10.0,
            w_death: 10.0,
            w_time: 0.001,
        }
    }
}

/// This crate's fixed 1v1: the opponent of worm `agent_idx` (design §5,
/// `N_WORMS == 2`).
fn opponent_idx(agent_idx: usize) -> usize {
    debug_assert!(
        agent_idx < 2,
        "reward() is scoped to the symmetric 1v1 (N_WORMS == 2)"
    );
    1 - agent_idx
}

/// One agent's shaped reward for the step that moved the match from `prev`
/// (captured before the step) to `state` (the live state right after).
/// `agent_idx` is `0` or `1` (design §5's symmetric 1v1). See the module docs
/// for the attribution/aggregation caveats.
pub fn reward(
    prev: &[WormSnapshot],
    state: &SimState,
    agent_idx: usize,
    cfg: &RewardConfig,
) -> f32 {
    let opp_idx = opponent_idx(agent_idx);
    let prev_self = prev[agent_idx];
    let prev_opp = prev[opp_idx];
    let cur_self = &state.worms[agent_idx];
    let cur_opp = &state.worms[opp_idx];

    // Only a DECREASE counts as damage — healing (a respawn's `health =
    // settings_health` reset, or a health bonus) must never register as
    // negative damage, so this clamps at 0 rather than reading the signed delta.
    let damage_dealt = (prev_opp.health - cur_opp.health).max(0) as f32;
    let damage_taken = (prev_self.health - cur_self.health).max(0) as f32;

    // Kill attribution is already resolved inside the sim (the killer's
    // `kills` field, not `last_killed_by_idx`, is what increments) — reward
    // just watches the counter, no re-derivation.
    let kill = if cur_self.kills > prev_self.kills {
        1.0
    } else {
        0.0
    };

    // A `health <= 0` transition, detected once per batch regardless of how
    // many ticks inside it were already-dead (T1 review, module docs).
    let death = if prev_self.health > 0 && cur_self.health <= 0 {
        1.0
    } else {
        0.0
    };

    cfg.w_damage_dealt * damage_dealt - cfg.w_damage_taken * damage_taken + cfg.w_kill * kill
        - cfg.w_death * death
        - cfg.w_time
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{two_worm_state as build_state, worm_init};
    use sim_core::vec::Vec2;

    /// Build a minimal two-worm `SimState` for reward tests: only `health` /
    /// `lives` / `kills` are load-bearing for `reward()`, so both worms start
    /// visible at the origin on a throwaway 100x100 level.
    fn two_worm_state(health: [i32; 2], lives: [i32; 2], kills: [i32; 2]) -> SimState {
        let w0 = worm_init(0, Vec2::zero(), health[0], lives[0], true);
        let w1 = worm_init(1, Vec2::zero(), health[1], lives[1], true);
        let mut state = build_state(100, 100, w0, w1);
        state.worms[0].kills = kills[0];
        state.worms[1].kills = kills[1];
        state
    }

    fn snap(health: i32, lives: i32, kills: i32) -> WormSnapshot {
        WormSnapshot {
            health,
            lives,
            kills,
        }
    }

    /// RED (plan T2): damage dealt to the opponent is rewarded, signed
    /// positive, scaled by `w_damage_dealt`, with everything else quiescent
    /// (net of the tiny time penalty).
    #[test]
    fn damage_dealt_to_opponent_is_rewarded_positively() {
        let prev = vec![snap(100, 1, 0), snap(100, 1, 0)];
        let state = two_worm_state([100, 70], [1, 1], [0, 0]); // opponent (idx 1) took 30
        let cfg = RewardConfig {
            w_damage_dealt: 2.0,
            w_damage_taken: 0.0,
            w_kill: 0.0,
            w_death: 0.0,
            w_time: 0.0,
        };
        let r = reward(&prev, &state, 0, &cfg);
        assert_eq!(r, 2.0 * 30.0, "30 opponent damage * w_damage_dealt=2.0");
    }

    /// RED (plan T2): damage taken by the agent itself is a negative reward,
    /// scaled by `w_damage_taken` — the sign is the opposite of damage dealt.
    #[test]
    fn damage_taken_by_self_is_rewarded_negatively() {
        let prev = vec![snap(100, 1, 0), snap(100, 1, 0)];
        let state = two_worm_state([60, 100], [1, 1], [0, 0]); // self (idx 0) took 40
        let cfg = RewardConfig {
            w_damage_dealt: 0.0,
            w_damage_taken: 3.0,
            w_kill: 0.0,
            w_death: 0.0,
            w_time: 0.0,
        };
        let r = reward(&prev, &state, 0, &cfg);
        assert_eq!(r, -3.0 * 40.0, "40 self damage * -w_damage_taken=3.0");
    }

    /// RED (plan T2): healing (a health INCREASE, e.g. a respawn reset or a
    /// bonus) must never register as damage in either direction.
    #[test]
    fn health_increase_is_not_damage_either_direction() {
        let prev = vec![snap(40, 1, 0), snap(40, 1, 0)];
        let state = two_worm_state([100, 100], [1, 1], [0, 0]); // both healed
        let cfg = RewardConfig {
            w_damage_dealt: 5.0,
            w_damage_taken: 5.0,
            w_kill: 0.0,
            w_death: 0.0,
            w_time: 0.0,
        };
        let r = reward(&prev, &state, 0, &cfg);
        assert_eq!(r, 0.0, "healing is neither damage_dealt nor damage_taken");
    }

    /// RED (plan T2): a kill (own `kills` counter incrementing) is a flat
    /// `+w_kill` bonus, independent of any simultaneous health deltas.
    #[test]
    fn kill_increment_gives_flat_kill_bonus() {
        let prev = vec![snap(100, 1, 0), snap(0, 0, 0)];
        let state = two_worm_state([100, 0], [1, 0], [1, 0]); // agent 0's kills: 0 -> 1
        let cfg = RewardConfig {
            w_damage_dealt: 0.0,
            w_damage_taken: 0.0,
            w_kill: 7.0,
            w_death: 0.0,
            w_time: 0.0,
        };
        let r = reward(&prev, &state, 0, &cfg);
        assert_eq!(r, 7.0);
    }

    /// RED (plan T2): the agent's own `health <= 0` transition is a flat
    /// `-w_death` penalty.
    #[test]
    fn own_death_transition_gives_flat_death_penalty() {
        let prev = vec![snap(20, 1, 0), snap(100, 1, 0)];
        let state = two_worm_state([0, 100], [0, 1], [0, 0]); // self (idx 0) dies
        let cfg = RewardConfig {
            w_damage_dealt: 0.0,
            w_damage_taken: 0.0, // isolate the death term from the damage term
            w_kill: 0.0,
            w_death: 9.0,
            w_time: 0.0,
        };
        let r = reward(&prev, &state, 0, &cfg);
        assert_eq!(r, -9.0);
    }

    /// RED (plan T2): already-dead this step (prev health already `<= 0`) must
    /// NOT re-fire the death penalty — it is a transition, not a level check.
    #[test]
    fn death_penalty_does_not_refire_once_already_dead() {
        let prev = vec![snap(0, 0, 0), snap(100, 1, 0)];
        let state = two_worm_state([0, 100], [0, 1], [0, 0]);
        let cfg = RewardConfig {
            w_damage_dealt: 0.0,
            w_damage_taken: 0.0,
            w_kill: 0.0,
            w_death: 9.0,
            w_time: 0.0,
        };
        let r = reward(&prev, &state, 0, &cfg);
        assert_eq!(r, 0.0, "already dead => no new death transition this step");
    }

    /// RED (plan T2): the tiny per-step time penalty applies even when nothing
    /// else changed.
    #[test]
    fn time_penalty_applies_when_nothing_else_changes() {
        let prev = vec![snap(100, 1, 0), snap(100, 1, 0)];
        let state = two_worm_state([100, 100], [1, 1], [0, 0]);
        let cfg = RewardConfig {
            w_damage_dealt: 0.0,
            w_damage_taken: 0.0,
            w_kill: 0.0,
            w_death: 0.0,
            w_time: 0.25,
        };
        let r = reward(&prev, &state, 0, &cfg);
        assert_eq!(r, -0.25);
    }

    /// RED (plan T2, T1-review "frame_skip spans death"): a `frame_skip > 1`
    /// batch can span several ticks of chip damage AND the lethal one, all
    /// folded into a single before/after snapshot pair. The before/after
    /// subtraction must still report the FULL summed damage across the whole
    /// batch (not just the last tick's), and the death flag must fire exactly
    /// once (not once per post-lethal tick still inside the batch) even though
    /// health has gone well past zero by the time the batch ends.
    #[test]
    fn reward_aggregates_multi_tick_damage_across_a_frame_skip_batch_including_death() {
        // Simulates: worm 0 started the batch at 100 health and, across several
        // ticks folded into ONE env step (frame_skip > 1), took 60 + 70 = 130
        // total damage — ending the batch well past 0 (-30), because more hits
        // landed on it after it was already dead.
        let prev = vec![snap(100, 1, 0), snap(100, 1, 0)];
        let state = two_worm_state([-30, 100], [0, 1], [0, 0]);
        let cfg = RewardConfig {
            w_damage_dealt: 0.0,
            w_damage_taken: 1.0,
            w_kill: 0.0,
            w_death: 9.0,
            w_time: 0.0,
        };
        let r = reward(&prev, &state, 0, &cfg);
        // damage_taken = 130 (the FULL batch total, not clamped at the health
        // bar's nominal 0 floor) => -130, plus exactly one death charge => -9.
        assert_eq!(
            r,
            -130.0 - 9.0,
            "the whole batch's damage must sum exactly once, and death must fire exactly once"
        );
    }

    /// RED (plan T2): reward is symmetric — the same step scores the mirror
    /// image for the other agent (its damage_dealt is agent 0's damage_taken).
    #[test]
    fn reward_is_symmetric_between_the_two_agents() {
        let prev = vec![snap(100, 1, 0), snap(100, 1, 0)];
        let state = two_worm_state([60, 80], [1, 1], [0, 0]); // 0 took 40, 1 took 20
        let cfg = RewardConfig {
            w_damage_dealt: 1.0,
            w_damage_taken: 1.0,
            w_kill: 0.0,
            w_death: 0.0,
            w_time: 0.0,
        };
        let r0 = reward(&prev, &state, 0, &cfg); // dealt 20, took 40
        let r1 = reward(&prev, &state, 1, &cfg); // dealt 40, took 20
        assert_eq!(r0, 20.0 - 40.0);
        assert_eq!(r1, 40.0 - 20.0);
    }
}
