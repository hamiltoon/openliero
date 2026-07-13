//! Committed-corpus drift backstop (Slice 4b, T4; spec §6b).
//!
//! The T3 round-trip gate (`tests/round_trip.rs`) is a pure self-check: it
//! records through `Live`, replays through `Scripted`, and asserts the two
//! independently-derived series agree. That is blind to exactly one class of
//! bug — a **symmetric** drift that shifts the recorder's serialization and the
//! parser's reading *identically* (spec §9's both-sides-drift finding), which
//! would still pass a pure round-trip.
//!
//! This test closes that hole with a **frozen, committed** artifact
//! (`record_slice4b_blood_scenario.txt`) plus a `state_hash` sidecar
//! (`record_slice4b_blood.txt`) whose column was produced by the **sim**
//! (`replay_state_series`, driven over the committed text at generation time —
//! see `game/examples/gen_record_slice4b_corpus.rs`) — independent of whatever
//! the input pipeline does *today*. Because the scenario file is committed
//! (frozen), a later change to `to_text`/`Scenario::parse`/`ControlState::pack`/
//! `unpack` semantics that drifts both sides in lockstep still changes what
//! `replay_state_series` computes for THIS file today vs. what the sidecar
//! pinned at generation time — so the drift turns this test red even though it
//! would leave `round_trip.rs` green.

use std::path::Path;

use game::input::replay_state_series;
use scenario::Scenario;

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
const GOLDEN_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../oracle-tests/golden");

/// Read the committed sidecar's per-tick `state_hash` column (2nd whitespace
/// token, hex `u32`; blank / `#` / `total` lines skipped) — index = tick.
/// Deliberately a 2-column grammar (no `frame_hash`): the sidecar is produced
/// by the headless `replay_state_series`, which never renders (see the
/// generator's doc comment for the full rationale).
fn sidecar_state_hashes(name: &str) -> Vec<u32> {
    let path = format!("{GOLDEN_DIR}/record_slice4b_{name}.txt");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    let mut states = Vec::new();
    for l in text.lines() {
        let t = l.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let mut it = t.split_whitespace();
        let first = it.next().expect("tick column");
        if first == "total" {
            continue;
        }
        let _tick: u32 = first.parse().expect("tick column is a u32");
        let hash = it.next().expect("state_hash column");
        states.push(u32::from_str_radix(hash, 16).expect("state_hash is hex"));
    }
    states
}

/// Drive the committed `record_slice4b_<name>` scenario through
/// `replay_state_series` and assert every tick equals the committed sidecar
/// column — mirrors `tests/passthrough.rs::assert_passthrough_deterministic`'s
/// shape, just against a recorded (not hand-authored) scenario.
fn assert_record_regression(name: &str) {
    let scenario_path = format!("{GOLDEN_DIR}/record_slice4b_{name}_scenario.txt");
    let text = std::fs::read_to_string(&scenario_path)
        .unwrap_or_else(|e| panic!("read {scenario_path}: {e}"));
    let scenario = Scenario::parse(&text).expect("committed recorded scenario parses");

    let golden = sidecar_state_hashes(name);
    assert_eq!(
        golden.len(),
        (scenario.ticks + 1) as usize,
        "{name}: one committed state_hash per tick 0..=ticks"
    );

    let series = replay_state_series(Path::new(TC_ROOT), &scenario);
    assert_eq!(
        series, golden,
        "{name}: replay_state_series diverged from the committed sim-produced sidecar \
         (a symmetric recorder+parser drift — spec §6b)"
    );
}

#[test]
fn record_slice4b_blood_matches_committed_sidecar() {
    assert_record_regression("blood");
}
