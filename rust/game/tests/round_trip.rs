//! MILESTONE — the non-vacuous record→replay round-trip determinism gate
//! (Slice 4b, T3; spec §6). Step 4's headline hard gate: **determinism survives
//! real input**.
//!
//! The trap the overview names is `Scripted == Scripted`: record *through the
//! Scripted source* and replay it, which proves nothing about the live path or
//! the recorder. This gate avoids it by three independent asymmetries any bug in
//! the pipeline breaks (spec §6):
//!
//! 1. **Different code paths on the two sides.** The record side samples a
//!    synthetic keyboard through the **real** [`InputSource::Live`] sampler
//!    (bindings + Dig chord + `pack`); the replay side drives
//!    [`InputSource::Scripted`] (`Scenario::input` + `unpack`). A `pack`/`unpack`
//!    disagreement or a binding/chord error diverges the two series.
//! 2. **A stream no committed scenario contains.** The per-tick key sequence is
//!    authored inline here — distinct from every `render_slice3b_*` golden — so
//!    the recorded `input` words are specific to this test and hand-verifiable,
//!    not silently equal to a pre-existing golden.
//! 3. **A real serialize→parse hop.** `Recorder::build().to_text()` →
//!    `Scenario::parse` routes through **text**, so a serializer bug (wrong
//!    sparse encoding, dropped metadata, wrong `ticks`) turns the gate red.
//!
//! Both series are the per-tick `hash_game_state` time series, driven from the
//! *same* `blood` tick-0 state (seed / level / worms carried verbatim through the
//! recording). The gate asserts them equal tick-for-tick (incl. tick 0), plus
//! spot-checks the decoded `input` words / raw text lines against hand-authored
//! expectations (so a *symmetric* bit swap is caught, not just self-consistency),
//! plus a non-vacuosity guard proving the synthetic stream actually drives the
//! sim. Headless (no Bevy app/window — `ButtonInput` is a plain mutable
//! resource), self-checking (no committed golden), and CI-wired under
//! `cargo test -p game` since 4a.

use std::path::{Path, PathBuf};

use bevy::input::keyboard::KeyCode;
use bevy::input::ButtonInput;

use game::input::{default_bindings, replay_state_series, InputSource, Recorder, N_WORMS};
use scenario::Scenario;
use sim::hash::hash_game_state;
use sim::state::ControlState;

/// Original-Liero TC data root — the same `CARGO_MANIFEST_DIR`-relative
/// resolution `main.rs` / `tests/passthrough.rs` use, so `cargo test` works from
/// any CWD.
const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
/// Committed golden dir (mirrors `tests/passthrough.rs::read_golden`).
const GOLDEN_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../oracle-tests/golden");

/// The Dig chord's packed word: Left OR Right, set together. Dig is
/// **unbound by default** (spec §2 / 4a §3), so a worm digs by holding its Left
/// and Right movement keys together — the produced 7-bit word is `LEFT | RIGHT`.
const DIG_CHORD: u32 = (1 << ControlState::LEFT) | (1 << ControlState::RIGHT);

// -- Default-binding key handles, named for the synthetic stream below. These
//    mirror `default_bindings()` (spec §2): worm 0 is the R/F/D/G diamond, worm 1
//    the arrow cluster. Holding a worm's Left+Right keys together is its Dig.
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

/// Read the committed `blood` scenario — its seed / level / worms are the base
/// tick-0 metadata; its committed `input` lines are irrelevant (the recorder
/// replaces them wholesale with the synthetic stream via `with_recorded_inputs`).
fn base_blood_scenario() -> Scenario {
    let path = format!("{GOLDEN_DIR}/render_slice3b_blood_scenario.txt");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    Scenario::parse(&text).expect("blood scenario parses")
}

/// Drive a persistent `ButtonInput<KeyCode>` to hold **exactly** `wanted` this
/// tick: release every currently-pressed key that is no longer wanted (this is
/// where the stream's key *releases* exercise `ButtonInput::release`), then press
/// every newly-wanted key. The sampler reads level-triggered `pressed`, so only
/// the resulting held-set matters — but routing through real press/release makes
/// the record side a faithful stand-in for the live keyboard.
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

/// The record→serialize→parse→replay outcome for one synthetic stream.
struct RoundTrip {
    /// Record-time `hash_game_state` series (Live sampler → live sim), index = tick.
    record_series: Vec<u32>,
    /// Replay-time `hash_game_state` series (parsed scenario → Scripted), index = tick.
    replay_series: Vec<u32>,
    /// The parsed recording (post serialize→parse hop) — its `input` words are
    /// spot-checked against hand-authored expectations.
    parsed: Scenario,
    /// The raw serialized scenario text — specific `input` lines are asserted.
    text: String,
    /// The replay series for the SAME base with an all-empty input stream — the
    /// non-vacuosity guard: the synthetic stream must move the sim off this.
    empty_series: Vec<u32>,
}

/// One full round-trip over a synthetic per-tick `(worm0_keys, worm1_keys)`
/// stream, authored in the caller. Records through the **real** `Live` sampler,
/// collects the record-time hash series off a live `blood` sim, serializes to
/// text, parses it back, and replays through `Scripted` (`replay_state_series`).
fn round_trip(stream: &[(Vec<KeyCode>, Vec<KeyCode>)]) -> RoundTrip {
    let base = base_blood_scenario();
    let tc_root = Path::new(TC_ROOT);

    // Record side: the real Live sampler over a synthetic keyboard, taps into a
    // Recorder AND drives a live sim to collect the record-time hash series.
    let live = InputSource::Live(default_bindings());
    let mut recorder = Recorder::new(base.clone(), PathBuf::from("/does/not/flush/in/test"));
    let mut keys = ButtonInput::<KeyCode>::default();
    let mut state = scenario::load(tc_root, &base).state;

    let mut record_series = Vec::with_capacity(stream.len() + 1);
    // Tick 0: hashed BEFORE any process_frame — matches replay_state_series.
    record_series.push(hash_game_state(&state));

    for (t, (keys0, keys1)) in stream.iter().enumerate() {
        let mut held: Vec<KeyCode> = keys0.clone();
        held.extend_from_slice(keys1);
        set_held(&mut keys, &held);

        // The tick's sampled [ControlState; N_WORMS] — Dig chord already resolved.
        let inputs: [ControlState; N_WORMS] = live.sample(t as u32, &keys);
        recorder.record(&inputs);
        state.process_frame(&inputs);
        record_series.push(hash_game_state(&state));
    }

    // Serialize → parse: the genuine on-disk-format hop (in memory; T4 commits the
    // file variant). A serializer bug turns the replay series red here.
    let text = recorder.build().to_text();
    let parsed = Scenario::parse(&text).expect("recorded scenario re-parses");

    // Replay side: drive the parsed scenario through Scripted, headless.
    let replay_series = replay_state_series(tc_root, &parsed);

    // Non-vacuosity guard: the SAME base, driven with an all-empty input stream of
    // the same length, must yield a DIFFERENT series — proving the synthetic
    // stream actually influences the sim (not a no-op that trivially agrees).
    let empty = base.with_recorded_inputs(&vec![(0, 0); stream.len()]);
    let empty_series = replay_state_series(tc_root, &empty);

    RoundTrip {
        record_series,
        replay_series,
        parsed,
        text,
        empty_series,
    }
}

/// THE gate. A 12-tick synthetic `blood` stream exercising multi-tick held
/// movement, fire, weapon change, key releases, and BOTH worms' Dig chord
/// (Left+Right held together) — recorded through the real Live sampler, routed
/// through text, replayed through Scripted — must reproduce the identical
/// `hash_game_state` series tick-for-tick.
#[test]
fn record_replay_round_trip_is_bit_exact() {
    // Per-tick (worm0 keys held, worm1 keys held). Words below are `pack()` =
    // raw bitfield: UP=1, DOWN=2, LEFT=4, RIGHT=8, FIRE=16, CHANGE=32, JUMP=64.
    //
    // Each worm holds Up (aim) for at least one tick before it Fires. Historical
    // note: this stream originally tripped the then-unguarded `cossin[128]` OOB
    // (firing un-aimed right after a walk direction-flip) — that reachable edge
    // was escalated out of this task's review and fixed by the H1 hardening
    // (`&0x7f` index masks in worm_fire/ninjarope/dig, sim `weapon.rs`), so the
    // aim-then-fire shape is kept as a realistic-player stream, not a workaround.
    let stream: Vec<(Vec<KeyCode>, Vec<KeyCode>)> = vec![
        // 0: w0 Up(1) aim;                 w1 idle(0)
        (vec![w0::UP], vec![]),
        // 1: w0 Up+Right(1|8=9) aim+walk;  w1 Up(1) aim
        (vec![w0::UP, w0::RIGHT], vec![w1::UP]),
        // 2: w0 Up+Fire(1|16=17) fire;     w1 Up held(1)   <- worm0 fires (aimed)
        (vec![w0::UP, w0::FIRE], vec![w1::UP]),
        // 3: w0 Right(8), Up/Fire released; w1 Up+Left(1|4=5)
        (vec![w0::RIGHT], vec![w1::UP, w1::LEFT]),
        // 4: w0 all released(0);           w1 Left(4)   -> sparse line (0,4)
        (vec![], vec![w1::LEFT]),
        // 5: w0 Change(32);                w1 idle(0)
        (vec![w0::CHANGE], vec![]),
        // 6: w0 DIG chord Left+Right(12);  w1 idle(0)   <- worm0 dig
        (vec![w0::LEFT, w0::RIGHT], vec![]),
        // 7: w0 Dig held(12);             w1 DIG chord Left+Right(12) <- worm1 dig
        (vec![w0::LEFT, w0::RIGHT], vec![w1::LEFT, w1::RIGHT]),
        // 8: w0 idle(0);                   w1 Right(8)
        (vec![], vec![w1::RIGHT]),
        // 9: w0 Up(1);                     w1 Up+Fire(1|16=17)  <- worm1 fires (aimed)
        (vec![w0::UP], vec![w1::UP, w1::FIRE]),
        // 10: both idle(0,0)  -> NO input line at all (fully-sparse tick)
        (vec![], vec![]),
        // 11: w0 Jump(64);                 w1 Jump(64)
        (vec![w0::JUMP], vec![w1::JUMP]),
    ];

    let rt = round_trip(&stream);

    // (1) THE GATE: the two independently-derived series agree tick-for-tick,
    //     including tick 0. Asserted first so a rigged divergence trips it here.
    assert_eq!(
        rt.record_series.len(),
        stream.len() + 1,
        "record series has one hash per tick 0..=ticks"
    );
    assert_eq!(
        rt.replay_series.len(),
        stream.len() + 1,
        "replay series has one hash per tick 0..=ticks"
    );
    assert_eq!(
        rt.record_series, rt.replay_series,
        "record (Live sampler) and replay (Scripted) hash_game_state series must be bit-identical"
    );

    // (2) NON-VACUOSITY GUARD: the synthetic stream actually drives the sim — the
    //     record series must differ from the same base run with no inputs.
    assert_ne!(
        rt.record_series, rt.empty_series,
        "synthetic stream must move the sim off the no-input baseline (else the gate is vacuous)"
    );

    // (3) DECODED-WORD SPOT-CHECKS on the parsed recording — a symmetric bit swap
    //     that keeps the two series self-consistent is still caught here.
    assert_eq!(
        rt.parsed.ticks,
        stream.len() as u32,
        "ticks == stream length"
    );
    assert_eq!(rt.parsed.input(0, 0), 1, "tick 0 worm0 = Up(1)");
    assert_eq!(rt.parsed.input(0, 1), 0, "tick 0 worm1 idle");
    assert_eq!(rt.parsed.input(1, 0), 9, "tick 1 worm0 = Up|Right(9)");
    assert_eq!(rt.parsed.input(2, 0), 17, "tick 2 worm0 = Up|Fire(17)");
    assert_eq!(rt.parsed.input(3, 1), 5, "tick 3 worm1 = Up|Left(5)");
    // The Dig chord decodes to LEFT|RIGHT for each worm:
    assert_eq!(
        rt.parsed.input(6, 0),
        DIG_CHORD,
        "tick 6 worm0 Dig chord = LEFT|RIGHT(12)"
    );
    assert_eq!(DIG_CHORD, 12, "Dig chord word is LEFT|RIGHT = 12");
    assert_eq!(
        rt.parsed.input(7, 1),
        DIG_CHORD,
        "tick 7 worm1 Dig chord = LEFT|RIGHT(12)"
    );
    assert_eq!(rt.parsed.input(11, 0), 64, "tick 11 worm0 = Jump(64)");
    assert_eq!(rt.parsed.input(11, 1), 64, "tick 11 worm1 = Jump(64)");
    // Fully-empty tick 10 decodes to (0,0) from absence (sparse storage).
    assert_eq!(rt.parsed.input(10, 0), 0, "tick 10 worm0 empty");
    assert_eq!(rt.parsed.input(10, 1), 0, "tick 10 worm1 empty");

    // (4) RAW-TEXT SPOT-CHECKS — read the serialized lines directly (spec §6
    //     "verify specific input rows"), incl. the Dig-chord rows and the sparse
    //     omission of the all-zero tick.
    assert!(
        rt.text.contains("input 0 1 0\n"),
        "tick-0 line present: {}",
        rt.text
    );
    assert!(
        rt.text.contains("input 6 12 0\n"),
        "worm0 Dig-chord line (L+R = 12) present: {}",
        rt.text
    );
    assert!(
        rt.text.contains("input 7 12 12\n"),
        "both-worms Dig-chord line present: {}",
        rt.text
    );
    assert!(
        rt.text.contains("input 11 64 64\n"),
        "tick-11 jump line present: {}",
        rt.text
    );
    assert!(
        !rt.text.contains("input 10 "),
        "all-zero tick 10 emits NO input line (sparse): {}",
        rt.text
    );
}

/// A second, longer synthetic stream with BOTH worms active across most ticks, so
/// the gate is not a single-vector fluke (plan T3). Same three asymmetries; here
/// we only assert the series equality + non-vacuosity (the decoded-word coverage
/// lives in the primary test above).
#[test]
fn record_replay_round_trip_longer_dual_worm_stream() {
    let stream: Vec<(Vec<KeyCode>, Vec<KeyCode>)> = vec![
        (vec![w0::UP], vec![w1::UP]),
        (vec![w0::UP, w0::FIRE], vec![w1::UP, w1::FIRE]),
        (vec![w0::LEFT], vec![w1::RIGHT]),
        (vec![w0::LEFT, w0::FIRE], vec![w1::RIGHT, w1::FIRE]),
        (vec![w0::RIGHT], vec![w1::LEFT]),
        (vec![w0::LEFT, w0::RIGHT], vec![w1::LEFT, w1::RIGHT]), // both dig
        (vec![w0::CHANGE], vec![w1::UP]),
        (vec![w0::FIRE], vec![w1::FIRE]),
        (vec![w0::JUMP], vec![w1::JUMP]),
        (vec![w0::UP, w0::RIGHT], vec![w1::UP, w1::LEFT]),
        (vec![w0::RIGHT, w0::FIRE], vec![w1::LEFT, w1::FIRE]),
        (vec![w0::LEFT, w0::RIGHT], vec![]), // worm0 dig, worm1 idle
        (vec![], vec![w1::LEFT, w1::RIGHT]), // worm0 idle, worm1 dig
        (vec![w0::UP], vec![w1::UP]),
        (vec![w0::UP, w0::CHANGE], vec![w1::UP, w1::FIRE]),
        (vec![w0::FIRE], vec![w1::RIGHT]),
        (vec![w0::JUMP, w0::FIRE], vec![w1::JUMP, w1::FIRE]),
        (vec![w0::RIGHT], vec![w1::LEFT]),
    ];

    let rt = round_trip(&stream);

    assert_eq!(
        rt.record_series, rt.replay_series,
        "longer dual-worm stream: record and replay series must be bit-identical"
    );
    assert_ne!(
        rt.record_series, rt.empty_series,
        "longer dual-worm stream must move the sim off the no-input baseline"
    );
    assert_eq!(rt.parsed.ticks, stream.len() as u32);
}
