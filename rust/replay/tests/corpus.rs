//! Corpus test — parse BOTH committed `.lrp` fixtures byte-faithfully.
//!
//! The fixtures were recorded by the C++ `lrp_gen` tool (T0) driving the real
//! `Game::ProcessFrame` over the committed scenarios, so the decoded per-tick
//! inputs must reproduce the scenario grammar exactly (the facit) and the
//! embedded `WideRollbackChecksum` words must land at the right cycles.
//!
//! This is a T2 (reader) test: it checks the *stream decode*, not the sim. The
//! sim gate (Rust `wide_rollback_checksum` == each embedded word) is the T3
//! milestone in `oracle-tests`.
//!
//! Facit (extracted from the committed corpus by the T0 reviewer):
//!   * `lrp_slice4e_long.lrp`      — 1120 ticks, 2 worms, checksum words
//!     `0x031057d7` @ cycle 0 and `0xf6211087` @ cycle 1050.
//!   * `lrp_render_slice4d_live.lrp` — 320 ticks, 2 worms, one checksum @ cycle 0.

use replay::{inflate, Replay, REPLAY_MAGIC};
use scenario::Scenario;

const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../oracle-tests/golden");

fn read(name: &str) -> Vec<u8> {
    std::fs::read(format!("{GOLDEN}/{name}")).unwrap_or_else(|e| panic!("read {name}: {e}"))
}

fn scenario(name: &str) -> Scenario {
    let text = std::fs::read_to_string(format!("{GOLDEN}/{name}"))
        .unwrap_or_else(|e| panic!("read {name}: {e}"));
    Scenario::parse(&text).unwrap_or_else(|e| panic!("parse {name}: {e}"))
}

/// Spec §4 empirical pin: the whole `.lrp` is one zlib-wrapped deflate stream
/// (miniz default framing), so `ZlibDecoder` inflates it and the inflated bytes
/// start with the big-endian `LRPF` magic.
#[test]
fn long_corpus_inflates_to_lrpf_magic() {
    let raw = inflate(&read("lrp_slice4e_long.lrp")).expect("inflate long corpus");
    assert!(!raw.is_empty(), "inflated stream is non-empty");
    assert_eq!(
        u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]),
        REPLAY_MAGIC,
        "inflated bytes start with the big-endian LRPF magic"
    );
}

#[test]
fn long_corpus_decodes_byte_faithfully() {
    let sc = scenario("lrp_slice4e_long_scenario.txt");
    assert_eq!(sc.worms.len(), 2);
    assert_eq!(sc.ticks, 1120);

    let replay =
        Replay::parse(&read("lrp_slice4e_long.lrp"), sc.worms.len()).expect("parse long corpus");

    assert_eq!(replay.version, 9, "kMyReplayVersion");
    assert_eq!(replay.num_worms, 2);
    assert_eq!(replay.ticks(), 1120, "the writer recorded `ticks` frames");

    // Checksum words at cycles 0 and 1050 (the > 1050 boundary is crossed).
    assert_eq!(replay.checksums.len(), 2, "two embedded checksum words");
    assert_eq!(replay.checksums.get(&0), Some(&0x0310_57d7), "cycle-0 word");
    assert_eq!(
        replay.checksums.get(&1050),
        Some(&0xf621_1087),
        "cycle-1050 word"
    );

    // The decoded per-tick inputs reproduce the scenario grammar exactly for
    // every tick (the facit) — both the explicit Down / Down+Fire rows and the
    // implicit idle (0) tail.
    for (tick, frame) in replay.frames.iter().enumerate() {
        for worm in 0..sc.worms.len() {
            assert_eq!(
                frame.inputs[worm],
                sc.input(tick as u32, worm),
                "tick {tick} worm {worm}: decoded input must match the scenario"
            );
        }
    }
}

#[test]
fn short_corpus_container_and_leading_inputs() {
    // The 4d fixture exercises the CONTAINER at a different length/state (320
    // ticks, a single cycle-0 checksum). Unlike the purpose-built long corpus, it
    // was recorded for RENDER testing (worm death + respawn), so its inputs later
    // provoke a `ProcessFrame` control-state mutation (worm1's one-shot Fire at
    // tick 150 clears the bit in the writer's `prev` baseline, whereas the long
    // corpus's held auto-fire never does). A PURE Phase-1 reader tracks
    // `prev = decoded` rather than the sim's post-process value, so it can only
    // reproduce the raw inputs up to that first mutation (spec §1 caveat) — the
    // full-stream input facit belongs to the long corpus. Here we pin the
    // container framing + the leading (pre-mutation) inputs.
    let sc = scenario("render_slice4d_live_scenario.txt");
    assert_eq!(sc.worms.len(), 2);
    assert_eq!(sc.ticks, 320);

    let replay = Replay::parse(&read("lrp_render_slice4d_live.lrp"), sc.worms.len())
        .expect("parse short corpus");

    assert_eq!(replay.version, 9);
    assert_eq!(replay.ticks(), 320);

    // A 320-tick replay crosses only the cycle-0 boundary.
    assert_eq!(
        replay.checksums.keys().copied().collect::<Vec<_>>(),
        vec![0]
    );

    // The opening inputs (well before the tick-150 mutation) decode faithfully
    // against the scenario grammar.
    for (tick, frame) in replay.frames.iter().take(60).enumerate() {
        for worm in 0..sc.worms.len() {
            assert_eq!(
                frame.inputs[worm],
                sc.input(tick as u32, worm),
                "tick {tick} worm {worm}: leading decoded input must match the scenario"
            );
        }
    }
}
