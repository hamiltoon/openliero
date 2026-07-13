//! [`LieroEnv`] — the pure-Rust RL episode core (design §5).
//!
//! One `LieroEnv` owns a single Liero match. [`reset`](LieroEnv::reset) rebuilds a
//! tick-0 [`SimState`] from the embedded `rl_default_match` fixture with a
//! caller-supplied seed; [`step`](LieroEnv::step) applies one `ControlState` per
//! worm through the real [`SimState::process_frame`] tick and reports the episode
//! `terminated`/`truncated` flags. Determinism is **per seed**: the same
//! `(seed, action stream)` yields an identical [`wide_rollback_checksum`] trace
//! (design §1.4), which the tests assert directly.

use std::io;
use std::path::{Path, PathBuf};

use scenario::{load, Loaded, Scenario};
use sim::state::{ControlState, SimState, WormState};
use sim::wide_checksum::wide_rollback_checksum;

use crate::record::Recording;

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
    /// The CURRENT episode's tick-0 metadata: `base` cloned with `reset`'s
    /// `seed` injected (design §1.5). This — not `base` — is what a recording
    /// is built over, so a saved file's tick-0 state matches the episode that
    /// was actually played, not the placeholder-seed fixture.
    episode_base: Scenario,
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
    /// The checksum computed by the most recent `reset`/`step` call (T1 review
    /// fix, see [`checksum`](Self::checksum)) — cached rather than recomputed so
    /// the accessor always agrees with the last-returned `StepOutcome.checksum`.
    last_checksum: u32,
    /// The active eval recording (design §1.5, plan T5), or `None` if
    /// `start_recording` hasn't been called since the last `reset`. `reset`
    /// always clears this — a recording's base fixture is tied to the episode
    /// it was started under.
    recording: Option<Recording>,
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
        let prev_istates = [0; N_WORMS];
        let last_checksum = wide_rollback_checksum(&loaded.state, &prev_istates);
        let episode_base = base.clone();
        LieroEnv {
            tc_root,
            base,
            episode_base,
            loaded,
            tick: 0,
            max_ticks,
            frame_skip: frame_skip.max(1),
            prev_istates,
            last_checksum,
            recording: None,
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
        // `scenario` is no longer needed after `load` (it only borrowed it) —
        // keep it as this episode's recording base (design §1.5): the seed the
        // episode actually ran with, not the placeholder in `self.base`.
        self.episode_base = scenario;
        self.tick = 0;
        self.prev_istates = [0; N_WORMS];
        // A fresh episode invalidates any in-progress recording — its base
        // fixture is tied to the episode `start_recording` was called under.
        self.recording = None;
        self.last_checksum = wide_rollback_checksum(&self.loaded.state, &self.prev_istates);
        self.last_checksum
    }

    /// Advance the episode by one env step: apply `actions[i]` to worm `i` for
    /// `frame_skip` sim frames, then report the episode flags and the post-step
    /// checksum. `actions` are absolute per-tick control words (T2's `pack7`
    /// produces them from `MultiBinary(7)`; T1 takes `ControlState` directly).
    pub fn step(&mut self, actions: &[ControlState; N_WORMS]) -> StepOutcome {
        let words = [actions[0].pack(), actions[1].pack()];
        for _ in 0..self.frame_skip {
            // Tap once per underlying sim tick (design §1.5) — a `frame_skip >
            // 1` step taps the same action word `frame_skip` times, so the
            // recorded stream indexes 1:1 by sim tick, exactly what `--replay`
            // (`InputSource::Scripted::sample(tick, _)`) expects, not by env step.
            if let Some(rec) = self.recording.as_mut() {
                rec.push(words[0], words[1]);
            }
            self.loaded.state.process_frame(actions);
            self.tick += 1;
        }
        let checksum = wide_rollback_checksum(&self.loaded.state, &self.prev_istates);
        self.prev_istates = words;
        self.last_checksum = checksum;

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

    /// The current [`wide_rollback_checksum`] fingerprint. Returns the **cached**
    /// value set by the last `reset`/`step` call (T1 review fix): recomputing it
    /// here from `self.prev_istates` would be wrong, because `step` overwrites
    /// `prev_istates` with the just-applied action word AFTER folding the
    /// checksum against the *prior* baseline — so a fresh
    /// `wide_rollback_checksum(&self.loaded.state, &self.prev_istates)` call at
    /// this point uses a different (post-update) baseline than the one
    /// `StepOutcome.checksum` was computed with, and disagrees with it whenever
    /// the two baselines differ (any non-zero action). Caching sidesteps the
    /// baseline mismatch entirely: `checksum()` always agrees with the
    /// most-recently-returned `StepOutcome.checksum` / `reset` value by
    /// construction.
    pub fn checksum(&self) -> u32 {
        self.last_checksum
    }

    /// Start tapping this episode's per-tick control words into a fresh
    /// [`Recording`] (design §1.5, plan T5). Replaces any recording already in
    /// progress. Ticks recorded before this call (earlier in the same episode)
    /// are NOT retroactively captured — only ticks from this point on.
    pub fn start_recording(&mut self) {
        self.recording = Some(Recording::new());
    }

    /// `true` while a recording is active (since the last `start_recording`,
    /// not yet cleared by a `reset`).
    pub fn is_recording(&self) -> bool {
        self.recording.is_some()
    }

    /// Build the active recording into a replayable [`Scenario`] (this
    /// episode's `episode_base` — seed already injected — plus the tapped
    /// input stream) and write its `to_text` form to `path`; the file
    /// `cargo run -p game -- --replay <path>` opens in the real window (design
    /// §1.5, §7). Errors with no active recording (call
    /// [`start_recording`](Self::start_recording) first) — never writes a
    /// silent empty/garbage file — or if `path` cannot be written.
    pub fn save_recording(&self, path: &Path) -> io::Result<()> {
        match &self.recording {
            Some(rec) => rec.save(&self.episode_base, path),
            None => Err(io::Error::other(
                "save_recording: no active recording (call start_recording() first)",
            )),
        }
    }

    /// Borrow the live simulation state (obs extraction, T2, reads this).
    pub fn state(&self) -> &SimState {
        &self.loaded.state
    }

    /// Mutably borrow the live simulation state. Test-only (T1 review fix): this
    /// must never reach the Python binding — obs/reward extraction (T2) reads
    /// through the immutable [`state`](Self::state) accessor, which is all they
    /// need. `#[cfg(test)]` makes a non-test caller a compile error.
    #[cfg(test)]
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

    /// T1-review RED test: `checksum()` must agree with the checksum on the
    /// `StepOutcome` the immediately preceding `step` call returned. Before the
    /// fix, `checksum()` recomputes `wide_rollback_checksum` against
    /// `self.prev_istates`, but `step` has already overwritten that field with
    /// the just-applied action word (the POST-update baseline) by the time
    /// `checksum()` runs — a different baseline than the one `StepOutcome.checksum`
    /// was folded against, so the two disagree whenever the applied action is
    /// non-zero (`0 != 0` never differs, so an all-noop action would pass
    /// vacuously; this test uses non-zero actions to actually exercise the bug).
    #[test]
    fn checksum_accessor_matches_last_step_outcome() {
        let mut env = LieroEnv::new(1000, 1);
        env.reset(99);
        let mut cs0 = ControlState::new();
        cs0.press(ControlState::RIGHT);
        let mut cs1 = ControlState::new();
        cs1.press(ControlState::LEFT);
        let out = env.step(&[cs0, cs1]);
        assert_eq!(
            env.checksum(),
            out.checksum,
            "checksum() must match the just-returned StepOutcome.checksum"
        );
    }

    /// T1-review RED test companion: `checksum()` must also agree with the
    /// checksum `reset` itself returns (the baseline case, already true before
    /// the fix — kept as a regression guard once `checksum()` switches to a
    /// cached value).
    #[test]
    fn checksum_accessor_matches_reset_checksum() {
        let mut env = LieroEnv::new(1000, 1);
        let reset_checksum = env.reset(99);
        assert_eq!(env.checksum(), reset_checksum);
    }

    /// T5 RED/GATE (plan T5, design §1.5): record a scripted episode, save it,
    /// `Scenario::parse` it back, and drive a FRESH sim from the parsed file
    /// exactly as `--replay` does (per-tick `ControlState::unpack(input(tick,
    /// i))` fed into `process_frame`, mirroring `game::input::replay_state_series`
    /// — reimplemented here since `liero-env` must not depend on `game`). The
    /// replayed checksum trace must match the original episode's bit-for-bit —
    /// this is the round-trip contract the whole eval-recording path rests on.
    #[test]
    fn recorded_episode_round_trips_through_replay() {
        let mut env = LieroEnv::new(500, 1);
        let mut original_trace = vec![env.reset(2024)];
        assert!(
            !env.is_recording(),
            "a fresh env must not be recording before start_recording"
        );
        env.start_recording();
        assert!(env.is_recording(), "start_recording must flip is_recording");

        let actions = action_stream(50);
        for a in &actions {
            original_trace.push(env.step(a).checksum);
        }

        let path = std::env::temp_dir().join(format!(
            "liero_env_round_trip_{}_{}.txt",
            std::process::id(),
            "recorded_episode_round_trips_through_replay"
        ));
        env.save_recording(&path)
            .expect("save_recording writes the file while recording is active");

        let text = std::fs::read_to_string(&path).expect("recorded file exists");
        std::fs::remove_file(&path).ok();
        let parsed = Scenario::parse(&text).expect("recorded scenario parses");
        assert_eq!(
            parsed.ticks as usize,
            actions.len(),
            "ticks must equal the recorded action-stream length"
        );

        let tc_root = PathBuf::from(TC_ROOT);
        let mut replay_state = load(&tc_root, &parsed).state;
        let mut prev = [0u32; N_WORMS];
        let mut replay_trace = vec![wide_rollback_checksum(&replay_state, &prev)];
        for t in 0..parsed.ticks {
            let w0 = parsed.input(t, 0);
            let w1 = parsed.input(t, 1);
            replay_state.process_frame(&[ControlState::unpack(w0), ControlState::unpack(w1)]);
            replay_trace.push(wide_rollback_checksum(&replay_state, &prev));
            prev = [w0, w1];
        }

        assert_eq!(
            original_trace, replay_trace,
            "round-tripped recording must trace an identical checksum series"
        );
    }

    /// `save_recording` before `start_recording` (or after a `reset` cleared a
    /// prior recording) must error loudly and write nothing — never a silent
    /// empty/garbage file.
    #[test]
    fn save_recording_without_active_recording_errors() {
        let mut env = LieroEnv::new(10, 1);
        env.reset(1);
        let path = std::env::temp_dir().join(format!(
            "liero_env_no_recording_{}_{}.txt",
            std::process::id(),
            "save_recording_without_active_recording_errors"
        ));
        let _ = std::fs::remove_file(&path);
        assert!(
            env.save_recording(&path).is_err(),
            "save_recording must error with no active recording"
        );
        assert!(!path.exists(), "no file must be written when not recording");
    }

    /// `reset` starts a fresh episode, so it must clear any in-progress
    /// recording (design §1.5 — a recording's base fixture is the episode it was
    /// started under; carrying it across a reset would silently record a
    /// mismatched episode).
    #[test]
    fn reset_clears_active_recording() {
        let mut env = LieroEnv::new(10, 1);
        env.reset(1);
        env.start_recording();
        assert!(env.is_recording());
        env.reset(2);
        assert!(
            !env.is_recording(),
            "reset must clear any in-progress recording"
        );
    }
}
