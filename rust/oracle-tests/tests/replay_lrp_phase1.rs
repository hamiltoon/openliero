//! Slice-4e Phase-1 gate — THE MILESTONE: a real Liero `.lrp` container + per-worm
//! XOR-delta input stream + embedded `WideRollbackChecksum` reads **bit-exact**.
//!
//! This is the strictly *harder* gate than the earlier `HashGameState` goldens
//! (spec §0): `WideRollbackChecksum` folds the ENTIRE rollback inventory — RNG,
//! cycles, every worm field, all four projectile/bonus pools, AND the whole
//! `material_id` buffer (incl. the terrain dug by 1120 ticks of firing). Matching
//! the word the C++ recorder embedded at cycle 1050 proves the Rust sim tracked
//! the real `Game::ProcessFrame` trajectory bit-for-bit over > 1050 ticks AND that
//! the reader decoded the container/delta stream faithfully.
//!
//! ## The fold point (load-bearing, spec §1)
//!
//! The C++ recorder folds the checksum inside `RecordFrame`, which the
//! `LocalController` runs AFTER setting each worm's `control_states` from the
//! tick's input but BEFORE `ProcessFrame` (`lrp_gen.cpp:325-329`,
//! `replay.cpp:369`). So at a `cycles % 1050 == 0` boundary the folded state is
//! the post-(N·ProcessFrame) state with `control_states = frames[N].inputs` and
//! `prev_control_states = frames[N].prev_inputs` (the previous tick's inputs, the
//! reader-tracked XOR baseline). This harness mirrors that exactly: it sets
//! `control_states` from `frames[t].inputs`, folds at the boundary, THEN calls
//! `process_frame(frames[t].inputs)` — the same order (`process_frame` re-sets
//! `control_states` from the same slice, `state.rs:1893`, so the manual set is
//! consistent, not a divergence).
//!
//! ## Cross-isolation (the first-differing-tick tripwire)
//!
//! Besides the two 1050-spaced checksum words, every tick's Rust `hash_game_state`
//! is cross-checked against the committed `_sim.txt` sidecar (produced independently
//! by the C++ `lrp_gen` tool's HashGameState dump over the SAME `.lrp` — byte-identical
//! `.lrp`, verified at generation). Any input-decode drift surfaces at the FIRST
//! differing tick rather than lumped at the 1050 boundary.
//!
//! ## No C++ at test time
//!
//! Like the 4b/4d goldens, this gate is self-checking: the `.lrp` corpus + its
//! sidecar are committed, so the test needs no C++ build (the C++ oracle ran once,
//! at corpus generation). The negative test in this file proves the gate is not a
//! no-op: corrupting one embedded word makes it fail.

use replay::{Replay, CHECKSUM_PERIOD};
use scenario::{load, Scenario};
use sim::hash::hash_game_state;
use sim::state::ControlState;
use sim::wide_checksum::wide_rollback_checksum;
use std::path::Path;

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/golden");

/// The two `WideRollbackChecksum` words embedded in `lrp_slice4e_long.lrp`
/// (T0-committed corpus): cycle 0 and — the HARD one — cycle 1050, which folds the
/// full inventory incl. the dug-out material buffer after 1050 ticks of firing.
const CYCLE0_WORD: u32 = 0x0310_57d7;
const CYCLE1050_WORD: u32 = 0xf621_1087;

fn read(name: &str) -> Vec<u8> {
    std::fs::read(format!("{GOLDEN}/{name}")).unwrap_or_else(|e| panic!("read {name}: {e}"))
}

fn scenario(name: &str) -> Scenario {
    let text = std::fs::read_to_string(format!("{GOLDEN}/{name}"))
        .unwrap_or_else(|e| panic!("read {name}: {e}"));
    Scenario::parse(&text).unwrap_or_else(|e| panic!("parse {name}: {e}"))
}

/// Parse the committed `_sim.txt` sidecar into the per-tick `HashGameState` master
/// series (column 1 of the 11-column `sim_physics_dump`/`lrp_gen` format).
fn sim_master(name: &str) -> Vec<u32> {
    std::fs::read_to_string(format!("{GOLDEN}/{name}"))
        .unwrap_or_else(|e| panic!("read {name}: {e}"))
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('#')
        })
        .map(|l| u32::from_str_radix(l.split_whitespace().nth(1).unwrap(), 16).unwrap())
        .collect()
}

/// Drive the sim from a scenario-reconstructed tick-0 state through a decoded
/// `.lrp`, asserting per tick: `hash_game_state` == the `_sim.txt` sidecar, and at
/// every `cycles % 1050 == 0` boundary `wide_rollback_checksum` == the embedded
/// word. Returns the number of checksum boundaries actually verified.
///
/// `corrupt_cycle1050`: for the negative test — perturb the state right before the
/// cycle-1050 fold so the gate must fail. `false` in the real gate.
fn drive_gate(
    scenario_file: &str,
    lrp_file: &str,
    sim_file: &str,
    corrupt_cycle1050: bool,
) -> usize {
    let sc = scenario(scenario_file);
    let num_worms = sc.worms.len();

    let replay = Replay::parse(&read(lrp_file), num_worms)
        .unwrap_or_else(|e| panic!("parse {lrp_file}: {e}"));

    // The stream must decode to exactly `scenario.ticks` frames and end on 0x83
    // (an early/late end would leave `frames.len()` off). The reader stops on the
    // end tag, so a full decode == `ticks` frames is the "stream ends on 0x83" pin.
    assert_eq!(
        replay.ticks(),
        sc.ticks as usize,
        "decoded tick count must match the scenario"
    );

    let master = sim_master(sim_file);
    assert_eq!(
        master.len(),
        sc.ticks as usize + 1,
        "sidecar has one HashGameState line per tick 0..=ticks"
    );

    let mut state = load(Path::new(TC_ROOT), &sc).state;

    // Tick 0: the sidecar's first line is the FRESH state (control_states still the
    // default 0, before any input is applied — `lrp_gen.cpp:318` dumps before the
    // loop). Check it before the loop touches control_states.
    assert_eq!(state.cycles, 0, "fresh state is at cycle 0");
    assert_eq!(
        hash_game_state(&state),
        master[0],
        "tick 0: hash_game_state == sidecar"
    );

    let mut boundaries = 0usize;

    for t in 0..replay.ticks() {
        let frame = &replay.frames[t];

        // Mirror `RecordFrame`: set each worm's control_states from this tick's
        // decoded input BEFORE the fold (the C++ order, `lrp_gen.cpp:325-328`).
        for (w, worm) in state.worms.iter_mut().enumerate() {
            worm.control_states = ControlState::unpack(frame.inputs[w]);
        }

        // At a 1050 boundary the recorder embedded a checksum word; fold the same
        // state (control_states just set, prev = the reader-tracked baseline).
        let cycle = state.cycles as u64;
        if cycle % CHECKSUM_PERIOD == 0 {
            let embedded = *replay
                .checksums
                .get(&cycle)
                .unwrap_or_else(|| panic!("no embedded checksum word at cycle {cycle}"));

            if corrupt_cycle1050 && cycle == 1050 {
                // Negative-test perturbation: nudge one folded field (worm0's
                // aiming_angle) so the wide checksum must diverge from the word.
                state.worms[0].aiming_angle = state.worms[0].aiming_angle.wrapping_add(1);
            }

            let got = wide_rollback_checksum(&state, &frame.prev_inputs);
            assert_eq!(
                got, embedded,
                "cycle {cycle}: WideRollbackChecksum {got:#010x} != embedded {embedded:#010x}"
            );
            boundaries += 1;
        }

        // Advance the sim (process_frame re-sets control_states from the same
        // slice, so this is the identical trajectory the C++ recorder drove).
        let inputs: Vec<ControlState> = frame
            .inputs
            .iter()
            .map(|&i| ControlState::unpack(i))
            .collect();
        state.process_frame(&inputs);

        // Cross-isolation: the post-ProcessFrame HashGameState must equal the
        // sidecar for THIS tick (line t+1). Divergence localises to the first tick.
        assert_eq!(
            hash_game_state(&state),
            master[t + 1],
            "tick {}: hash_game_state == sidecar (first-differing-tick tripwire)",
            t + 1
        );
    }

    boundaries
}

/// THE MILESTONE — `lrp_slice4e_long.lrp` (1120 ticks) plays back bit-exact: both
/// embedded `WideRollbackChecksum` words match (cycle 0 AND the hard cycle-1050
/// word folding the dug material buffer), and every tick's `hash_game_state`
/// matches the committed sidecar.
#[test]
fn long_corpus_replays_bit_exact_including_cycle_1050() {
    let replay = Replay::parse(
        &read("lrp_slice4e_long.lrp"),
        scenario("lrp_slice4e_long_scenario.txt").worms.len(),
    )
    .expect("parse long corpus");

    // Non-vacuity FIRST: the corpus must exceed 1050 ticks so >= 1 checksum word is
    // actually consumed at a NON-cycle-0 boundary (a corpus of <= 1050 ticks would
    // pass the checksum gate vacuously — only the free cycle-0 word).
    assert!(
        replay.ticks() > CHECKSUM_PERIOD as usize,
        "corpus must exceed 1050 ticks (non-vacuous cycle-1050 fold)"
    );
    assert_eq!(replay.checksums.len(), 2, "two embedded checksum words");
    assert_eq!(replay.checksums.get(&0), Some(&CYCLE0_WORD), "cycle-0 word");
    assert_eq!(
        replay.checksums.get(&1050),
        Some(&CYCLE1050_WORD),
        "the HARD cycle-1050 word"
    );

    let boundaries = drive_gate(
        "lrp_slice4e_long_scenario.txt",
        "lrp_slice4e_long.lrp",
        "lrp_slice4e_long_sim.txt",
        false,
    );
    assert_eq!(
        boundaries, 2,
        "both the cycle-0 and cycle-1050 checksum folds were verified"
    );
}

/// Secondary gate — the SHORT 4d fixture (`lrp_render_slice4d_live.lrp`, 320 ticks)
/// cross-checked over its first 60 ticks (the T2 ≤60-tick scope: after tick 150 the
/// fixture's inputs provoke a `ProcessFrame` control-state mutation, past which a
/// pure stream reader's XOR baseline drifts, spec §1 caveat — irrelevant to the
/// long corpus's held-fire inputs). The sim trajectory (driven by the decoded
/// inputs) is cross-checked against `render_slice4d_live_sim.txt`, and the cycle-0
/// checksum word is verified — a second, independent container+state fixture.
#[test]
fn short_corpus_cross_checks_first_60_ticks() {
    let sc = scenario("render_slice4d_live_scenario.txt");
    let num_worms = sc.worms.len();
    let replay =
        Replay::parse(&read("lrp_render_slice4d_live.lrp"), num_worms).expect("parse short corpus");

    // A 320-tick replay crosses only the cycle-0 boundary.
    assert_eq!(
        replay.checksums.keys().copied().collect::<Vec<_>>(),
        vec![0],
        "one checksum word at cycle 0"
    );

    let master = sim_master("render_slice4d_live_sim.txt");
    let mut state = load(Path::new(TC_ROOT), &sc).state;

    assert_eq!(
        hash_game_state(&state),
        master[0],
        "tick 0: hash_game_state == sidecar"
    );

    // Cycle-0 fold: control_states = frames[0].inputs, prev = frames[0].prev_inputs.
    for (w, worm) in state.worms.iter_mut().enumerate() {
        worm.control_states = ControlState::unpack(replay.frames[0].inputs[w]);
    }
    assert_eq!(
        wide_rollback_checksum(&state, &replay.frames[0].prev_inputs),
        *replay.checksums.get(&0).unwrap(),
        "cycle-0 WideRollbackChecksum matches the embedded word"
    );

    // Cross-check the first 60 ticks' HashGameState against the sidecar.
    for t in 0..60 {
        let frame = &replay.frames[t];
        let inputs: Vec<ControlState> = frame
            .inputs
            .iter()
            .map(|&i| ControlState::unpack(i))
            .collect();
        state.process_frame(&inputs);
        assert_eq!(
            hash_game_state(&state),
            master[t + 1],
            "tick {}: short-fixture hash_game_state == sidecar",
            t + 1
        );
    }
}

/// Non-vacuity / negative — perturbing one folded field right before the cycle-1050
/// fold makes the gate FAIL. Proves the milestone assertion is a real gate, not a
/// no-op that would pass regardless of the state.
#[test]
#[should_panic(expected = "WideRollbackChecksum")]
fn corrupting_a_folded_field_fails_the_gate() {
    drive_gate(
        "lrp_slice4e_long_scenario.txt",
        "lrp_slice4e_long.lrp",
        "lrp_slice4e_long_sim.txt",
        true,
    );
}
