//! Headless pass-through determinism gate (Slice 4a, T3).
//!
//! The objective 4a gate (spec §5, done-when 3): driving the sim through the
//! **new** input sampler in *scripted* mode must reproduce every committed
//! scenario's `hash_game_state` time series **bit-exact**. Because
//! `InputSource::Scripted` touches no Bevy/window, the whole gate runs
//! **headless** under `cargo test -p game` — the direct ancestor of 4b's
//! record→replay round-trip.
//!
//! Crucially the gate drives the **real** [`game::input::InputSource::Scripted`]
//! (reached through the crate's thin `lib.rs` surface), not a local
//! re-implementation of the recorded-input read: a regression in the sampler's
//! pass-through — index swap, wrong `unpack`, keys leaking in — turns this red.
//!
//! Per scenario the sim is driven exactly as the 3b frame-hash harness drives it
//! (`render_slice3b_common::run`): tick 0 is asserted **before** any
//! `process_frame`, then tick `k` (1..=ticks) feeds `source.sample(k-1, &empty)`
//! into `process_frame` and asserts `hash_game_state == golden.state_hash[k]`.
//! An empty `ButtonInput` is passed because Scripted ignores live keys (the
//! pass-through carries the recorded stream). The committed goldens are the C++
//! dumper's output — a red here means the sampler DRIFTED; fix the sampler,
//! never the golden.

use std::path::Path;

use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;

use game::input::InputSource;
use scenario::Scenario;
use sim::hash::hash_game_state;

/// Original-Liero TC data root (relative to this crate's manifest — the same
/// `CARGO_MANIFEST_DIR`-relative resolution `main.rs` and `shot`'s golden test
/// use, so `cargo test` works from any CWD).
const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");

/// Read a committed `render_slice3b_<name><suffix>` golden from the
/// `oracle-tests` crate (mirrors `shot::tests::golden::read_golden`).
fn read_golden(name: &str, suffix: &str) -> String {
    let path = format!(
        "{}/../oracle-tests/golden/render_slice3b_{name}{suffix}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

/// The committed frame sidecar's per-tick `state_hash` column (3rd column, hex
/// `u32`; blank / `#` / `total` lines skipped) — index = tick. Same grammar as
/// `main.rs::load_golden_hashes` / `render_slice3b_common::parse_frames`
/// (`<tick> <frame_hash_hex16> <state_hash_hex8>`), reading only the state
/// column the determinism gate asserts.
fn golden_state_hashes(name: &str) -> Vec<u32> {
    let text = read_golden(name, ".txt");
    let mut states = Vec::new();
    for l in text.lines() {
        let t = l.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let mut it = t.split_whitespace();
        if it.next() == Some("total") {
            continue;
        }
        // Columns after <tick>: <frame_hash_hex16> <state_hash_hex8>.
        let _frame_hash = it.next().expect("frame_hash column");
        let state_hash = it.next().expect("state_hash column");
        states.push(u32::from_str_radix(state_hash, 16).expect("state_hash hex"));
    }
    states
}

/// Drive one committed scenario headlessly through the **real**
/// `InputSource::Scripted` and lock every per-tick `hash_game_state` to the
/// golden `state_hash` column.
fn assert_passthrough_deterministic(name: &str) {
    let scenario_text = read_golden(name, "_scenario.txt");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");

    let golden = golden_state_hashes(name);
    assert_eq!(
        golden.len(),
        (scenario.ticks + 1) as usize,
        "{name}: one golden state_hash per tick 0..=ticks"
    );

    // Tick-0 state via the same `scenario::load` path main.rs and the 3b harness
    // use (level + worms + RNG); the render surface is irrelevant here.
    let mut state = scenario::load(Path::new(TC_ROOT), &scenario).state;

    // The gate feeds the REAL sampler. Scripted ignores keys, but `sample` still
    // takes a `ButtonInput`, so hand it an empty one — no window, fully headless.
    let source = InputSource::Scripted(scenario.clone());
    let empty = ButtonInput::<KeyCode>::default();

    // Tick 0: hashed BEFORE the first `process_frame` (as the sim/3b goldens are).
    assert_eq!(
        hash_game_state(&state),
        golden[0],
        "{name} tick 0: state hash must match the golden before any process_frame"
    );

    // Tick k (1..=ticks): the recorded input for tick k-1 flows through the
    // sampler, one frame advances, the state hash is locked to the golden. This
    // index alignment mirrors the 3b harness exactly (input(k-1) -> state k).
    for k in 1..=scenario.ticks {
        let inputs = source.sample(k - 1, &empty);
        state.process_frame(&inputs);
        assert_eq!(
            hash_game_state(&state),
            golden[k as usize],
            "{name} tick {k}: state hash diverged from golden through InputSource::Scripted"
        );
    }
    // The loop above ran through `scenario.ticks` unconditionally (no early
    // return), and the length assert above already pins `golden.len() ==
    // scenario.ticks + 1` — so reaching here already proves the full series
    // was driven through the final tick; a second length assert here would be
    // tautological.
}

// One test per committed 3b scenario so a drift localises to the family that
// broke (blood/dart pin the base sim; the rest broaden the input/RNG coverage).

#[test]
fn passthrough_blood() {
    assert_passthrough_deterministic("blood");
}

#[test]
fn passthrough_dart() {
    assert_passthrough_deterministic("dart");
}

#[test]
fn passthrough_dart_water() {
    assert_passthrough_deterministic("dart_water");
}

#[test]
fn passthrough_fan() {
    assert_passthrough_deterministic("fan");
}

#[test]
fn passthrough_laser() {
    assert_passthrough_deterministic("laser");
}

#[test]
fn passthrough_shadow() {
    assert_passthrough_deterministic("shadow");
}

#[test]
fn passthrough_shake() {
    assert_passthrough_deterministic("shake");
}
