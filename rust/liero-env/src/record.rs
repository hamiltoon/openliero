//! Eval recording (design §1.5, plan T5) — the harness's "watch it play" path.
//!
//! [`Recording`] taps the per-tick `(w0, w1)` 7-bit [`sim::state::ControlState::pack`]
//! words [`LieroEnv::step`](crate::env::LieroEnv::step) applies while recording is
//! on, one pair per underlying sim tick (a `frame_skip > 1` env-step taps the same
//! action word once per repeated sim tick, so the recorded stream lines up 1:1
//! with the sim ticks `--replay` drives, not with the env's action-repeat steps).
//! On save, [`Recording::to_scenario`] builds a replayable [`Scenario`] via
//! `Scenario::with_recorded_inputs` over the episode's **base fixture with its
//! seed already injected** (`LieroEnv`'s `episode_base` — the RL fixture, not a
//! placeholder), so the file's tick-0 state matches the episode that was
//! actually played; `to_text` serializes it, and `cargo run -p game -- --replay
//! <path>` plays it back through the existing `Scripted`/`Mode::Replay` path
//! (design §1.5, §7) — no new engine code.

use std::io;
use std::path::Path;

use scenario::Scenario;

/// Accumulates one episode's per-tick applied control words.
#[derive(Default)]
pub struct Recording {
    /// Positional per-tick `(worm0_7bit, worm1_7bit)` words, in tick order —
    /// the same shape [`Scenario::with_recorded_inputs`] takes.
    per_tick: Vec<(u32, u32)>,
}

impl Recording {
    /// An empty recording, ready for [`push`](Self::push).
    pub fn new() -> Self {
        Recording::default()
    }

    /// Tap one sim tick's applied control words, in tick order.
    pub fn push(&mut self, w0: u32, w1: u32) {
        self.per_tick.push((w0, w1));
    }

    /// Sim ticks recorded so far.
    pub fn len(&self) -> usize {
        self.per_tick.len()
    }

    /// `true` iff no tick has been recorded yet.
    pub fn is_empty(&self) -> bool {
        self.per_tick.is_empty()
    }

    /// Build the replayable [`Scenario`]: `base`'s tick-0 metadata (seed/level/
    /// worms/weapons — the episode's actual fixture, seed already injected by
    /// the caller) plus the recorded per-tick input stream, via
    /// `Scenario::with_recorded_inputs` (sparse; all-zero ticks omitted; the
    /// result round-trips through `to_text`/`parse` unchanged).
    pub fn to_scenario(&self, base: &Scenario) -> Scenario {
        base.with_recorded_inputs(&self.per_tick)
    }

    /// Build the scenario and write its `to_text` form to `path` — the file
    /// `cargo run -p game -- --replay <path>` opens in the real window (design
    /// §1.5).
    pub fn save(&self, base: &Scenario, path: &Path) -> io::Result<()> {
        std::fs::write(path, self.to_scenario(base).to_text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny synthetic base — no TC assets touched, `Recording` only reads/
    /// clones `Scenario` metadata, never loads it.
    fn base() -> Scenario {
        Scenario::parse(
            "seed 1\nlevel Levels/x.lev\nticks 0\nworm 0 0 0 100 1 0 1\nworm 1 0 0 100 1 218 1\n",
        )
        .expect("synthetic base scenario parses")
    }

    /// `to_scenario` sets `ticks` to the recorded length and stores the stream
    /// sparsely (design §1.5) — an all-zero tick decodes back to `(0, 0)` from
    /// absence, matching `Scenario::with_recorded_inputs`'s own contract.
    #[test]
    fn to_scenario_sets_ticks_and_sparse_inputs() {
        let mut rec = Recording::new();
        assert!(rec.is_empty());
        rec.push(5, 0);
        rec.push(0, 0);
        rec.push(0, 3);
        assert_eq!(rec.len(), 3);

        let scenario = rec.to_scenario(&base());
        assert_eq!(scenario.ticks, 3);
        assert_eq!(scenario.input(0, 0), 5);
        assert_eq!(scenario.input(0, 1), 0);
        assert_eq!(scenario.input(1, 0), 0);
        assert_eq!(scenario.input(1, 1), 0);
        assert_eq!(scenario.input(2, 0), 0);
        assert_eq!(scenario.input(2, 1), 3);
    }

    /// `save` writes a file whose text re-parses through the real grammar and
    /// carries the same recorded stream (the on-disk half of the round-trip
    /// gate; the full "drive a fresh sim from it" gate lives in `env.rs`).
    #[test]
    fn save_writes_parsable_to_text() {
        let mut rec = Recording::new();
        rec.push(1, 2);
        rec.push(0, 0);

        let path = std::env::temp_dir().join(format!(
            "liero_env_record_unit_{}_{}.txt",
            std::process::id(),
            "save_writes_parsable_to_text"
        ));
        rec.save(&base(), &path).expect("save writes the file");
        let text = std::fs::read_to_string(&path).expect("file exists");
        std::fs::remove_file(&path).ok();

        let parsed = Scenario::parse(&text).expect("recorded text parses");
        assert_eq!(parsed.ticks, 2);
        assert_eq!(parsed.input(0, 0), 1);
        assert_eq!(parsed.input(0, 1), 2);
        assert_eq!(parsed.input(1, 0), 0);
        assert_eq!(parsed.input(1, 1), 0);
    }
}
