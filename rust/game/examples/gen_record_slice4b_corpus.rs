//! Regeneration script for the committed slice-4b recorded corpus (T4; spec
//! §6b). Writes `record_slice4b_blood_scenario.txt` (the recorded artifact)
//! and `record_slice4b_blood.txt` (its sim-produced `state_hash` sidecar) into
//! `rust/oracle-tests/golden/`.
//!
//! **Reproducible by construction:** the synthetic per-tick key stream below is
//! the SAME stream `game/tests/round_trip.rs::record_replay_round_trip_is_bit_exact`
//! already gates non-vacuous (T3) — multi-tick held movement, fire, weapon
//! change, key releases, and both worms' Dig chord. Recording it again through
//! the real `InputSource::Live` sampler always reproduces byte-identical output
//! (the sim and the recorder are both deterministic), so re-running this script
//! is a no-op against a clean tree. Re-run after any recorder/serializer change
//! to confirm the corpus still regenerates identically (or to intentionally
//! update it):
//!
//! ```text
//! cargo run -p game --example gen_record_slice4b_corpus
//! ```
//!
//! The sidecar's `state_hash` column is produced by [`game::input::replay_state_series`]
//! driven over the FRESHLY RE-PARSED committed scenario text (not the in-memory
//! `Recorder::build()` value) — so it is independent of the record-time
//! serialization path, matching spec §6b's both-sides-drift backstop rationale.
//! `replay_state_series` is headless (no render pass), so unlike the
//! `render_slice3b_*` goldens this sidecar carries no `frame_hash` column —
//! see `game/tests/record_regression.rs` for the (2-column) reader.

use std::path::{Path, PathBuf};

use bevy::input::keyboard::KeyCode;
use bevy::input::ButtonInput;

use game::input::{default_bindings, replay_state_series, InputSource, Recorder, N_WORMS};
use scenario::Scenario;
use sim::state::ControlState;

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
const GOLDEN_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../oracle-tests/golden");

// Default-binding key handles — identical to `tests/round_trip.rs`'s `w0`/`w1`
// modules (duplicated rather than shared: this example and that integration
// test compile as separate crates/targets).
mod w0 {
    use bevy::input::keyboard::KeyCode;
    pub const UP: KeyCode = KeyCode::KeyR;
    pub const LEFT: KeyCode = KeyCode::KeyD;
    pub const RIGHT: KeyCode = KeyCode::KeyG;
    pub const FIRE: KeyCode = KeyCode::ControlLeft;
    pub const CHANGE: KeyCode = KeyCode::ShiftLeft;
    pub const JUMP: KeyCode = KeyCode::AltLeft;
}
mod w1 {
    use bevy::input::keyboard::KeyCode;
    pub const UP: KeyCode = KeyCode::ArrowUp;
    pub const LEFT: KeyCode = KeyCode::ArrowLeft;
    pub const RIGHT: KeyCode = KeyCode::ArrowRight;
    pub const FIRE: KeyCode = KeyCode::ControlRight;
    pub const JUMP: KeyCode = KeyCode::ShiftRight;
}

/// Drive a persistent `ButtonInput<KeyCode>` to hold **exactly** `wanted` this
/// tick (release what's no longer wanted, press what's newly wanted) — same
/// helper as `tests/round_trip.rs::set_held`.
fn set_held(keys: &mut ButtonInput<KeyCode>, wanted: &[KeyCode]) {
    let current: Vec<KeyCode> = keys.get_pressed().copied().collect();
    for k in current {
        if !wanted.contains(&k) {
            keys.release(k);
        }
    }
    for &k in wanted {
        if !keys.pressed(k) {
            keys.press(k);
        }
    }
}

/// The committed `blood` scenario's tick-0 metadata (seed/level/worms) — the
/// base the recording's metadata is carried from verbatim.
fn base_blood_scenario() -> Scenario {
    let path = format!("{GOLDEN_DIR}/render_slice3b_blood_scenario.txt");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    Scenario::parse(&text).expect("blood scenario parses")
}

fn main() {
    // Identical to `round_trip.rs`'s primary 12-tick stream (spec §6b: "one
    // recorded artifact... produced from a synthetic stream" — reusing the
    // already-gated T3 stream keeps the corpus's coverage independently
    // verified rather than inventing new, unverified coverage math).
    let stream: Vec<(Vec<KeyCode>, Vec<KeyCode>)> = vec![
        (vec![w0::UP], vec![]),
        (vec![w0::UP, w0::RIGHT], vec![w1::UP]),
        (vec![w0::UP, w0::FIRE], vec![w1::UP]),
        (vec![w0::RIGHT], vec![w1::UP, w1::LEFT]),
        (vec![], vec![w1::LEFT]),
        (vec![w0::CHANGE], vec![]),
        (vec![w0::LEFT, w0::RIGHT], vec![]),
        (vec![w0::LEFT, w0::RIGHT], vec![w1::LEFT, w1::RIGHT]),
        (vec![], vec![w1::RIGHT]),
        (vec![w0::UP], vec![w1::UP, w1::FIRE]),
        (vec![], vec![]),
        (vec![w0::JUMP], vec![w1::JUMP]),
    ];

    let base = base_blood_scenario();
    let tc_root = Path::new(TC_ROOT);
    let scenario_path = format!("{GOLDEN_DIR}/record_slice4b_blood_scenario.txt");
    let sidecar_path = format!("{GOLDEN_DIR}/record_slice4b_blood.txt");

    // Record through the REAL Live sampler (spec §6b: the artifact is produced
    // the same non-vacuous way the T3 gate proves — never through Scripted).
    let live = InputSource::Live(default_bindings());
    let mut recorder = Recorder::new(base, PathBuf::from(&scenario_path));
    let mut keys = ButtonInput::<KeyCode>::default();
    for (t, (keys0, keys1)) in stream.iter().enumerate() {
        let mut held: Vec<KeyCode> = keys0.clone();
        held.extend_from_slice(keys1);
        set_held(&mut keys, &held);
        let inputs: [ControlState; N_WORMS] = live.sample(t as u32, &keys);
        recorder.record(&inputs);
    }

    let text = recorder.build().to_text();
    std::fs::write(&scenario_path, &text).unwrap_or_else(|e| panic!("write {scenario_path}: {e}"));
    println!("wrote {scenario_path} ({} bytes)", text.len());

    // Sidecar: re-read + re-parse the COMMITTED bytes (not the in-memory
    // `Recorder::build()` value) so the state_hash column is produced from
    // exactly what future test runs will parse — then drive replay_state_series
    // (headless, sim-produced, independent of the recorder/serializer path).
    let committed_text = std::fs::read_to_string(&scenario_path)
        .unwrap_or_else(|e| panic!("read back {scenario_path}: {e}"));
    let committed = Scenario::parse(&committed_text).expect("committed scenario re-parses");
    let series = replay_state_series(tc_root, &committed);

    let mut sidecar = String::new();
    sidecar.push_str(
        "# record_slice4b_blood — sim-produced state_hash sidecar (slice 4b, T4).\n\
         # Column 2 is hash_game_state(tick), produced by replay_state_series driving\n\
         # InputSource::Scripted over record_slice4b_blood_scenario.txt (headless — no\n\
         # render pass, so unlike render_slice3b_* there is no frame_hash column).\n\
         # This is the drift backstop of spec §6b: the round-trip gate (T3) is a pure\n\
         # self-check (record == replay) and is blind to a bug that shifts BOTH sides\n\
         # identically; this sidecar pins ABSOLUTE sim-produced values against a frozen\n\
         # committed scenario, so a symmetric drift still turns record_regression.rs red.\n\
         # Regenerate via: cargo run -p game --example gen_record_slice4b_corpus\n",
    );
    for (tick, hash) in series.iter().enumerate() {
        sidecar.push_str(&format!("{tick} {hash:08x}\n"));
    }
    sidecar.push_str(&format!("total {}\n", series.len()));

    std::fs::write(&sidecar_path, &sidecar).unwrap_or_else(|e| panic!("write {sidecar_path}: {e}"));
    println!("wrote {sidecar_path} ({} ticks)", series.len());
}
