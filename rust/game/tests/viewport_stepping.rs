//! Live viewport stepping gate (Slice 4d, T2) — the ordering-critical
//! shake / banner / flash split around `process_frame`.
//!
//! `game` is a Bevy **binary**, so its live tick loop cannot be driven by an
//! integration test directly; the ordering-owning core is factored into the
//! Bevy-free `game::viewport_step` (reached through `lib.rs`) so it CAN be gated
//! headlessly here — exactly the `passthrough.rs` posture for the input sampler.
//!
//! Three ordering properties are pinned (design §3, §9 risk 1):
//! 1. the per-tick shake decrement runs strictly BEFORE the explosion-max
//!    (the central bit-exactness risk — a 2-tick explosion distinguishes the
//!    two orders);
//! 2. the banner walk runs only on `(cycles & 1) == 0` and steps toward the
//!    `killed_timer`-selected stop, reading the values the caller supplies from
//!    the PRE-frame sim;
//! 3. the explosion-shake events land only on viewports whose current WORLD
//!    window (strictly) contains the raw blast.
//!
//! Plus the golden-vakt: driving a real scenario (`blood`) through
//! `tick_viewports` reproduces its committed `hash_game_state` series
//! byte-identically (live stepping is hash-neutral) while the real tick-10
//! explosion drives a live viewport shake (non-vacuity). The 3b injection path
//! (`render_slice3b_common`) and the `shot` harness are SEPARATE drivers that
//! never call `tick_viewports`, so the live stepping and the injection can never
//! double-apply — the injection-bearing goldens (e.g. `shake`) are produced by a
//! different driver and are unaffected (their state hash is injection-neutral,
//! covered by `passthrough.rs`).

use std::path::Path;

use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;

use game::input::InputSource;
use game::viewport_step::{apply_shake_events, step_before_frame, tick_viewports};
use render::bitmap::Rect;
use render::viewport::Viewport;
use scenario::Scenario;
use sim::hash::hash_game_state;
use sim::shake::ShakeEvent;
use sim::state::WormState;
use sim_core::fixed::itof;

/// Original-Liero TC data root (same `CARGO_MANIFEST_DIR`-relative resolution as
/// `passthrough.rs`, so `cargo test` works from any CWD).
const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");

fn read_golden(name: &str, suffix: &str) -> String {
    let path = format!(
        "{}/../oracle-tests/golden/render_slice3b_{name}{suffix}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

/// The committed frame sidecar's per-tick `state_hash` column (index = tick) —
/// same grammar as `passthrough::golden_state_hashes`.
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
        let _frame_hash = it.next().expect("frame_hash column");
        let state_hash = it.next().expect("state_hash column");
        states.push(u32::from_str_radix(state_hash, 16).expect("state_hash hex"));
    }
    states
}

/// The two-viewport player layout, built directly (no TC needed) for the pure
/// stepping tests.
fn two_viewports() -> [Viewport; 2] {
    [
        Viewport::new(Rect::new(0, 0, 158, 158), 0),
        Viewport::new(Rect::new(160, 0, 318, 158), 1),
    ]
}

/// A minimal 2-worm slice for the banner walk (only `killed_timer` is read):
/// load the real `blood` worms and let the caller override `killed_timer`.
fn loaded_worms() -> Vec<WormState> {
    let scenario = Scenario::parse(&read_golden("blood", "_scenario.txt")).expect("parses");
    scenario::load(Path::new(TC_ROOT), &scenario).state.worms
}

/// **Ordering RED (design §9 risk 1).** Rig a 2-tick explosion on viewport 0 and
/// prove the shake decrement runs STRICTLY BEFORE the explosion-max. Both ticks
/// fire an `amount = 2` explosion (`itof(2) = 131072`) whose blast lands inside
/// vp0's window; `cycles` is odd so the banner walk is skipped (worms unused).
///
/// Tick 2 is the discriminator: with `shake = 131072` carried from tick 1,
/// - CORRECT (decrement THEN max): `131072 − 4000 = 127072`, then
///   `max(131072, 127072) = 131072`;
/// - WRONG (max THEN decrement): `max(131072, 131072) = 131072`, then
///   `131072 − 4000 = 127072`.
///
/// Reversing the two `viewport_step` calls (the RED run) yields `127072` and
/// trips the tick-2 assert; the committed order yields `131072` (GREEN).
#[test]
fn shake_decrement_runs_strictly_before_the_explosion_max() {
    let mut vps = two_viewports();
    let blast = ShakeEvent {
        x: 40,
        y: 40,
        amount: 2,
    }; // inside vp0 [0,158)x[0,158)

    // Tick 1: shake 0 -> decrement is a no-op -> max(131072, 0) = 131072.
    step_before_frame(&mut vps, 1, &[]);
    apply_shake_events(&mut vps, &[blast]);
    assert_eq!(
        vps[0].shake,
        itof(2),
        "tick 1 sets shake to itof(2) = 131072"
    );

    // Tick 2 (CORRECT order): decrement 131072 -> 127072, then max(131072, 127072).
    step_before_frame(&mut vps, 1, &[]);
    apply_shake_events(&mut vps, &[blast]);
    assert_eq!(
        vps[0].shake,
        itof(2),
        "the fresh explosion re-maxes above the DECREMENTED carry only if the \
         decrement ran first; the reversed order yields 127072"
    );
}

/// The shake decrement itself (`game.cpp:275-285`): subtract 4000 while `> 0`,
/// with NO floor at 0 (a shake in `(0, 4000]` goes slightly negative — the `> 0`
/// guard is the only gate, so `Ftoi(negative) <= 0` disables the render RNG).
#[test]
fn shake_decrement_has_no_zero_floor() {
    let mut vps = two_viewports();
    vps[0].shake = 3000; // in (0, 4000]
    step_before_frame(&mut vps, 1, &[]); // odd cycle: banner skipped
    assert_eq!(
        vps[0].shake, -1000,
        "3000 - 4000 goes negative (only the > 0 guard)"
    );
    // A second step must NOT decrement again (guard is `> 0`, and -1000 is not).
    step_before_frame(&mut vps, 1, &[]);
    assert_eq!(vps[0].shake, -1000, "the > 0 guard stops further decrement");
}

/// The banner walk (`game.cpp:292-311`): runs only on `(cycles & 1) == 0`, steps
/// `banner_y` toward `+2` when the viewport's worm has `killed_timer > 16`, else
/// toward `-8`, capped at both stops. The caller supplies `cycles`/`worms` from
/// the PRE-frame sim, so this pins the semantics the ordering depends on.
#[test]
fn banner_walk_steps_on_even_cycles_toward_the_killed_timer_stop() {
    let mut worms = loaded_worms();
    let mut vps = two_viewports();

    // killed_timer > 16 => walk DOWN into view (toward +2).
    worms[0].killed_timer = 20;
    vps[0].banner_y = -8;
    step_before_frame(&mut vps, 0, &worms); // even cycle
    assert_eq!(
        vps[0].banner_y, -7,
        "even cycle + killed_timer>16 -> +1 toward 2"
    );

    // Odd cycle: the walk is skipped entirely (every OTHER cycle).
    step_before_frame(&mut vps, 1, &worms);
    assert_eq!(vps[0].banner_y, -7, "odd cycle -> banner frozen");

    // killed_timer <= 16 => retreat (toward -8).
    worms[0].killed_timer = 5;
    vps[0].banner_y = 0;
    step_before_frame(&mut vps, 0, &worms);
    assert_eq!(
        vps[0].banner_y, -1,
        "even cycle + killed_timer<=16 -> -1 toward -8"
    );

    // Capped at -8.
    vps[0].banner_y = -8;
    step_before_frame(&mut vps, 0, &worms);
    assert_eq!(vps[0].banner_y, -8, "floored at -8");

    // Capped at +2.
    worms[0].killed_timer = 20;
    vps[0].banner_y = 2;
    step_before_frame(&mut vps, 0, &worms);
    assert_eq!(vps[0].banner_y, 2, "capped at 2");
}

/// The explosion-shake application (`sobject.cpp:27-33`, game-side): a blast lands
/// on a viewport iff its CURRENT world window `(vp.x, vp.x+width) x (vp.y,
/// vp.y+height)` strictly contains the raw `(x, y)`, then `shake = max(itof, shake)`.
#[test]
fn apply_shake_events_uses_the_viewport_world_window_strictly() {
    let mut vps = two_viewports();
    // Scroll vp0's world window to [100,258) x [50,208).
    vps[0].x = 100;
    vps[0].y = 50;

    // Just inside -> applied.
    apply_shake_events(
        &mut vps,
        &[ShakeEvent {
            x: 101,
            y: 51,
            amount: 3,
        }],
    );
    assert_eq!(
        vps[0].shake,
        itof(3),
        "blast inside the world window sets itof(amount)"
    );

    // On the left edge (x == vp.x): strict `>` excludes it.
    vps[0].shake = 0;
    apply_shake_events(
        &mut vps,
        &[ShakeEvent {
            x: 100,
            y: 60,
            amount: 3,
        }],
    );
    assert_eq!(vps[0].shake, 0, "left edge is exclusive (strict >)");

    // Outside the window entirely -> not applied.
    apply_shake_events(
        &mut vps,
        &[ShakeEvent {
            x: 300,
            y: 60,
            amount: 3,
        }],
    );
    assert_eq!(vps[0].shake, 0, "blast outside the world window is ignored");

    // max, not overwrite: a smaller amount does not lower a larger carried shake.
    vps[0].shake = itof(9);
    apply_shake_events(
        &mut vps,
        &[ShakeEvent {
            x: 101,
            y: 51,
            amount: 1,
        }],
    );
    assert_eq!(vps[0].shake, itof(9), "max keeps the larger carried shake");
}

/// **Golden-vakt + non-vacuity.** Driving the real `blood` scenario through
/// `tick_viewports` (the full pre-decrement -> `process_frame` -> event-max
/// order) reproduces its committed `hash_game_state` series byte-identically —
/// the live stepping is hash-neutral (`screen_flash`/`shake_events` are unhashed
/// side channels) — while the real tick-10 explosion drives a live viewport
/// shake, proving the wired path is non-vacuous.
#[test]
fn blood_live_stepping_is_hash_neutral_and_fires_the_explosion_shake() {
    let scenario = Scenario::parse(&read_golden("blood", "_scenario.txt")).expect("parses");
    let golden = golden_state_hashes("blood");
    assert_eq!(
        golden.len(),
        (scenario.ticks + 1) as usize,
        "one golden state_hash per tick 0..=ticks"
    );

    let loaded = scenario::load(Path::new(TC_ROOT), &scenario);
    let mut state = loaded.state;
    let mut vps: [Viewport; 2] = loaded.viewports;
    let source = InputSource::Scripted(scenario.clone());
    let empty = ButtonInput::<KeyCode>::default();

    // Tick 0: hashed BEFORE any frame, exactly as the sim/3b goldens are.
    assert_eq!(
        hash_game_state(&state),
        golden[0],
        "blood tick 0 state hash"
    );

    let mut fired_shake = false;
    for k in 1..=scenario.ticks {
        let inputs = source.sample(k - 1, &empty);
        tick_viewports(&mut vps, &mut state, &inputs);
        assert_eq!(
            hash_game_state(&state),
            golden[k as usize],
            "blood tick {k}: live stepping perturbed the sim hash (must stay golden-neutral)"
        );
        if vps[0].shake > 0 || vps[1].shake > 0 {
            fired_shake = true;
        }
    }
    assert!(
        fired_shake,
        "blood's real explosion must drive a live viewport shake through tick_viewports"
    );
}
