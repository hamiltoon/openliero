//! [`LieroEnv`] — the pure-Rust RL episode core (design §5).
//!
//! One `LieroEnv` owns a single Liero match. [`reset`](LieroEnv::reset) rebuilds a
//! tick-0 [`SimState`] from the embedded `rl_default_match` fixture with a
//! caller-supplied seed; [`step`](LieroEnv::step) applies one `ControlState` per
//! worm through the real [`SimState::process_frame`] tick and reports the episode
//! `terminated`/`truncated` flags. Determinism is **per seed**: the same
//! `(seed, action stream)` yields an identical [`wide_rollback_checksum`] trace
//! (design §1.4), which the tests assert directly.

use std::path::PathBuf;

use scenario::{load, Loaded, Scenario};
use sim::state::{ControlState, SimState, WormState};
use sim::wide_checksum::wide_rollback_checksum;

/// Worms per match — the symmetric 1v1 the harness trains (design §5). Fixed at
/// two: one action and one obs/reward per agent per tick.
pub const N_WORMS: usize = 2;

/// The episode fixture, embedded so `reset` needs no filesystem lookup for the
/// scenario text (only the TC assets `scenario::load` reads). KillEmAll, two
/// visible worms, `lives 1`, DART loadout — see the file header.
const RL_DEFAULT_MATCH: &str = include_str!("../scenarios/rl_default_match.txt");

/// The total-conversion asset root `scenario::load` reads (level, sprites, tc.cfg,
/// weapon/object configs). `rust/liero-env` → `../../data/TC/openliero`.
const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");

/// The per-`step` episode signal (design §5's `(terminated, truncated)`).
///
/// `terminated` is per agent but, in the symmetric 1v1 KillEmAll round, both flip
/// together the moment either worm exhausts its lives ("the round is over" —
/// design §5), so the array is broadcast. `truncated` fires independently when the
/// tick budget is spent. `checksum` is the per-tick [`wide_rollback_checksum`]
/// fingerprint the determinism contract is asserted over.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StepOutcome {
    /// Per-agent round-over flag (broadcast in 1v1 KillEmAll, design §5).
    pub terminated: [bool; N_WORMS],
    /// `true` once the tick counter reaches `max_ticks` (design §5). Essential —
    /// Liero rounds can otherwise stall indefinitely.
    pub truncated: bool,
    /// The wide-rollback checksum of the post-`step` state (design §1.4).
    pub checksum: u32,
}

/// A worm has no lives left, so under KillEmAll it can never respawn (the
/// game-mode/lives gate at `state.rs:1917` skips a `lives <= 0` worm's body every
/// subsequent tick). `lives` only reaches `<= 0` via a death, which the death
/// block also marks by clearing `visible` and decrementing `lives`
/// (`worm_death`, `state.rs:2891-2919`) — so this predicate captures exactly
/// design §5's "an agent dead with no lives left".
fn worm_out_of_lives(w: &WormState) -> bool {
    w.lives <= 0
}

/// One Liero match, driven as a reinforcement-learning environment.
pub struct LieroEnv {
    tc_root: PathBuf,
    /// The parsed fixture with its placeholder seed; `reset` clones it and
    /// overrides `seed` per episode (so the base parse happens once).
    base: Scenario,
    /// The live tick-0-onward state (plus viewports/scene `scenario::load`
    /// returns; the env only reads `.state`).
    loaded: Loaded,
    /// Sim frames processed since the last `reset` (`frame_skip` per `step`).
    tick: u32,
    /// Truncation budget in sim frames (design §5's `max_ticks`).
    max_ticks: u32,
    /// Sim frames advanced per `step` (action-repeat, design §2). `>= 1`.
    frame_skip: u32,
    /// The previous tick's applied 7-bit control words — the XOR baseline
    /// `wide_rollback_checksum` folds per worm (`prev_control_states.istate`,
    /// wide_checksum.rs module doc). `[0; N]` at reset, matching the tick-0
    /// convention (`oracle-tests` `wide_checksum_tick0`).
    prev_istates: [u32; N_WORMS],
}

impl LieroEnv {
    /// A sensible default truncation budget: 3000 sim frames (~43 s at Liero's
    /// ~70 Hz tick) — long enough for a KillEmAll round to resolve, short enough
    /// to bound a stalled episode.
    pub const DEFAULT_MAX_TICKS: u32 = 3000;

    /// Build an env over the embedded `rl_default_match` fixture. `frame_skip` is
    /// clamped to `>= 1`. The fixture is parsed and loaded once here so the struct
    /// always holds a valid state; `reset` reloads it with the episode seed.
    pub fn new(max_ticks: u32, frame_skip: u32) -> Self {
        let tc_root = PathBuf::from(TC_ROOT);
        let base =
            Scenario::parse(RL_DEFAULT_MATCH).expect("embedded rl_default_match fixture parses");
        assert_eq!(
            base.worms.len(),
            N_WORMS,
            "the RL fixture must define exactly {N_WORMS} worms"
        );
        let loaded = load(&tc_root, &base);
        LieroEnv {
            tc_root,
            base,
            loaded,
            tick: 0,
            max_ticks,
            frame_skip: frame_skip.max(1),
            prev_istates: [0; N_WORMS],
        }
    }

    /// Build an env with [`DEFAULT_MAX_TICKS`](Self::DEFAULT_MAX_TICKS) and
    /// `frame_skip = 1`.
    pub fn with_defaults() -> Self {
        Self::new(Self::DEFAULT_MAX_TICKS, 1)
    }

    /// Rebuild the tick-0 state for a fresh episode with `seed`, returning the
    /// tick-0 [`wide_rollback_checksum`]. Determinism is per seed: two envs reset
    /// with the same seed and driven by the same action stream trace an identical
    /// checksum series; different seeds diverge (design §5).
    pub fn reset(&mut self, seed: u32) -> u32 {
        let mut scenario = self.base.clone();
        // Inject the episode seed before load — `SimState::new` seeds `rand` from
        // `scenario.seed` (loader.rs:156), so this is what makes different seeds
        // diverge while a fixed seed stays reproducible (design §5).
        scenario.seed = seed;
        self.loaded = load(&self.tc_root, &scenario);
        self.tick = 0;
        self.prev_istates = [0; N_WORMS];
        wide_rollback_checksum(&self.loaded.state, &self.prev_istates)
    }

    /// Advance the episode by one env step: apply `actions[i]` to worm `i` for
    /// `frame_skip` sim frames, then report the episode flags and the post-step
    /// checksum. `actions` are absolute per-tick control words (T2's `pack7`
    /// produces them from `MultiBinary(7)`; T1 takes `ControlState` directly).
    pub fn step(&mut self, actions: &[ControlState; N_WORMS]) -> StepOutcome {
        let words = [actions[0].pack(), actions[1].pack()];
        for _ in 0..self.frame_skip {
            self.loaded.state.process_frame(actions);
            self.tick += 1;
        }
        let checksum = wide_rollback_checksum(&self.loaded.state, &self.prev_istates);
        self.prev_istates = words;

        let round_over = self.loaded.state.worms.iter().any(worm_out_of_lives);
        StepOutcome {
            terminated: [round_over; N_WORMS],
            truncated: self.tick >= self.max_ticks,
            checksum,
        }
    }

    /// Sim frames processed since the last `reset`.
    pub fn ticks(&self) -> u32 {
        self.tick
    }

    /// The current post-step [`wide_rollback_checksum`] fingerprint.
    pub fn checksum(&self) -> u32 {
        wide_rollback_checksum(&self.loaded.state, &self.prev_istates)
    }

    /// Borrow the live simulation state (obs extraction, T2, reads this).
    pub fn state(&self) -> &SimState {
        &self.loaded.state
    }

    /// Mutably borrow the live simulation state. Used by tests to inject a scripted
    /// lethal hit; obs/reward paths (T2) read through the shared [`state`](Self::state).
    pub fn state_mut(&mut self) -> &mut SimState {
        &mut self.loaded.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::state::ControlState;

    /// A tiny deterministic action stream (LCG over 7-bit words) — the "same
    /// action sequence" the determinism contract quantifies over. It is entirely
    /// separate from the sim RNG (design §5's seeding discipline): it never touches
    /// `SimState.rand`.
    fn action_stream(len: usize) -> Vec<[ControlState; N_WORMS]> {
        let mut s: u32 = 0x1234_5678;
        let mut next = || {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            ControlState::unpack(s >> 9) // mask to 7 bits inside unpack
        };
        (0..len).map(|_| [next(), next()]).collect()
    }

    /// Roll a full episode: reset with `seed`, step the action stream, and collect
    /// the `[reset, step_1, step_2, …]` checksum trace.
    fn trace(seed: u32, actions: &[[ControlState; N_WORMS]]) -> Vec<u32> {
        let mut env = LieroEnv::new(10_000, 1);
        let mut out = vec![env.reset(seed)];
        for a in actions {
            out.push(env.step(a).checksum);
        }
        out
    }

    /// The core determinism contract (design §1.4, plan T1 RED): same
    /// `(seed, action stream)` ⇒ identical `wide_rollback_checksum` trace across two
    /// independent env instances.
    #[test]
    fn same_seed_same_actions_identical_checksum_trace() {
        let actions = action_stream(120);
        let a = trace(4242, &actions);
        let b = trace(4242, &actions);
        assert_eq!(
            a, b,
            "same seed + same actions must trace identical checksums"
        );
        // Non-triviality: the trace actually evolves (not a constant), so the
        // equality above is meaningful.
        assert!(
            a.windows(2).any(|w| w[0] != w[1]),
            "the checksum trace must evolve, else determinism is vacuous"
        );
    }

    /// Non-vacuity (plan T1 RED): a *different* seed drives the sim RNG down a
    /// different path, so the checksum trace must diverge. This is the test the
    /// seed-injection in `reset` exists to satisfy.
    #[test]
    fn different_seed_diverges() {
        let actions = action_stream(120);
        let a = trace(1, &actions);
        let b = trace(2, &actions);
        assert_ne!(
            a, b,
            "different seeds must produce a divergent checksum trace"
        );
    }

    /// `truncated` fires exactly when the tick counter reaches `max_ticks`, and not
    /// before (design §5).
    #[test]
    fn truncates_at_max_ticks() {
        let max = 5;
        let mut env = LieroEnv::new(max, 1);
        env.reset(7);
        let noop = [ControlState::new(), ControlState::new()];
        for t in 1..max {
            let out = env.step(&noop);
            assert!(
                !out.truncated,
                "must not truncate before max_ticks (tick {t})"
            );
        }
        let out = env.step(&noop);
        assert!(out.truncated, "must truncate once tick == max_ticks");
        assert_eq!(env.ticks(), max);
    }

    /// `terminated` on a scripted kill (plan T1 RED): inject a lethal hit
    /// (`health = 0`) into worm 0 via `state_mut`, then one `step` runs the sim's
    /// real death block (`worm_death`: `--lives` → 0, `visible = false`). With the
    /// fixture's `lives 1`, that exhausts worm 0's lives, so the KillEmAll round is
    /// over and both agents terminate together (design §5).
    #[test]
    fn terminates_on_scripted_kill() {
        let mut env = LieroEnv::new(10_000, 1);
        env.reset(11);
        assert!(
            env.state().worms.iter().all(|w| w.lives > 0),
            "both worms start with lives"
        );
        // Scripted lethal damage: drive worm 0's health to 0 the tick before the
        // step, so the visible arm's death block fires.
        env.state_mut().worms[0].health = 0;

        let out = env.step(&[ControlState::new(), ControlState::new()]);
        assert_eq!(
            env.state().worms[0].lives,
            0,
            "the killed worm is out of lives"
        );
        assert!(
            !env.state().worms[0].visible,
            "the killed worm is hidden on death"
        );
        assert!(
            env.state().worms[1].lives > 0,
            "the survivor still has lives"
        );
        assert_eq!(
            out.terminated,
            [true, true],
            "round over => both agents terminate together (design §5)"
        );
        assert!(!out.truncated, "terminated, not truncated");
    }
}
