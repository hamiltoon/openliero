//! Live viewport stepping around the sim's `process_frame` (Slice 4d, T2).
//!
//! C++ fuses six phases into one `Game::ProcessFrame` (`game.cpp:267-471`);
//! three of them touch the per-viewport shake/banner state and the sim's
//! `screen_flash`. The Rust sim runs `process_frame` as ONE atomic call, so the
//! `game` layer reproduces the C++ phase order by splitting the viewport work
//! **around** that call (design §3):
//!
//! ```text
//! // BEFORE process_frame (reads the PREVIOUS tick's shake, the pre-`++cycles`
//! // `cycles`, and the pre-worm-loop `killed_timer`):
//! step_before_frame(viewports, cycles, worms)   // shake decrement + banner walk
//!
//! sim.process_frame(inputs)   // phase-1 screen_flash-- + phases 2-6; emits shake events
//!
//! // AFTER process_frame (C++ phase-2 explosion-max; no shake reader ran in between,
//! // the render-time RNG at phase 5 is the only reader):
//! apply_shake_events(viewports, sim.drain_shake_events())
//! ```
//!
//! The render (`frame::draw`) then runs `Viewport::process` (C++ phase 5:
//! centering + shake-RNG + clamp), reading the POST-worm-loop worm state and the
//! post-tick `sim.screen_flash` for the palette `LightUp`.
//!
//! # Why the split is bit-exact (design §3, §9 risk 1)
//!
//! The ONLY reader of `shake` is the phase-5 render RNG. So the render-time value
//! is `max( prev_shake − 4000 (clamped by the `> 0` guard), itof(amount) )` — the
//! decrement (phase 1) strictly BEFORE the explosion-max (phase 2), with no reader
//! between. Reordering (max before decrement, or reading a post-`++cycles`
//! `cycles`/post-worm-loop `killed_timer` for the banner walk) desyncs the frame.
//!
//! This driver is exercised ONLY by the `game` binary's live tick loop. The 3b
//! injection path (`render_slice3b_common`) and the `shot` harness are SEPARATE
//! drivers that call `process_frame` directly and set `viewports[vp].shake`
//! by hand around a single draw — they never call this module, so the live
//! stepping and the injection can never double-apply (design §5).

use render::viewport::Viewport;
use sim::shake::ShakeEvent;
use sim::state::{ControlState, SimState, WormState};
use sim_core::fixed::itof;

/// The per-tick viewport shake decrement + `4000` step (`game.cpp:275-285`).
const SHAKE_DECREMENT: i32 = 4000;
/// The `banner_y` low/high stops (`game.cpp:298-311`): the banner walks in
/// `[-8, 2]`, resting hidden at `-8`.
const BANNER_LOW: i32 = -8;
const BANNER_HIGH: i32 = 2;
/// The `killed_timer` threshold above which the death banner walks DOWN into
/// view (`game.cpp:302`).
const BANNER_KILLED_TIMER: i32 = 16;

/// C++ `Game::ProcessFrame`'s top-of-frame viewport stepping
/// (`game.cpp:271-332`), run **before** `process_frame`. Uses the PREVIOUS
/// tick's viewport `shake`, the pre-`++cycles` `cycles`, and the pre-worm-loop
/// `killed_timer` — all supplied by the caller reading `sim` before the frame.
///
/// * **Shake decrement** (`game.cpp:275-285`): every viewport with `shake > 0`
///   loses `4000`. There is NO floor at 0 — the `> 0` guard is the only gate, so
///   a `shake` in `(0, 4000]` goes slightly negative (`Ftoi(negative) <= 0`
///   disables the render RNG next tick, exactly as C++).
/// * **Banner walk** (`game.cpp:292-311`), only on `(cycles & 1) == 0` (every
///   OTHER cycle): `banner_y` steps toward `+2` when this viewport's worm has
///   `killed_timer > 16` (a death is showing), else toward `-8` (hidden).
pub fn step_before_frame(viewports: &mut [Viewport], cycles: i32, worms: &[WormState]) {
    for vp in viewports.iter_mut() {
        if vp.shake > 0 {
            vp.shake -= SHAKE_DECREMENT;
        }
    }

    if (cycles & 1) == 0 {
        for vp in viewports.iter_mut() {
            let down = worms[vp.worm_idx].killed_timer > BANNER_KILLED_TIMER;
            if down {
                if vp.banner_y < BANNER_HIGH {
                    vp.banner_y += 1;
                }
            } else if vp.banner_y > BANNER_LOW {
                vp.banner_y -= 1;
            }
        }
    }
}

/// Apply the sim's drained explosion-shake events to the live viewports
/// (`sobject.cpp:27-33`, moved game-side — design §3b), run **after**
/// `process_frame` with NO `shake` reader between the phase-1 decrement and this
/// phase-2 max (the render RNG at phase 5 is the only reader).
///
/// For every viewport whose CURRENT world window `[vp.x, vp.x + width) x [vp.y,
/// vp.y + height)` contains the RAW blast `(x, y)` — the C++ strict-`>`/`<`
/// containment test against `v.x`/`v.rect.Width()` — `shake = max(itof(amount),
/// shake)`. `amount` is the raw `type.shake`; the fixed-point `itof` matches
/// `Itof(shake)` in the C++ write.
pub fn apply_shake_events(viewports: &mut [Viewport], events: &[ShakeEvent]) {
    for ev in events {
        for vp in viewports.iter_mut() {
            let contains = ev.x > vp.x
                && ev.x < vp.x + vp.rect.width()
                && ev.y > vp.y
                && ev.y < vp.y + vp.rect.height();
            if contains {
                vp.shake = vp.shake.max(itof(ev.amount));
            }
        }
    }
}

/// One live tick of the game layer: the C++ `Game::ProcessFrame` phase order,
/// reproduced by splitting the viewport work around the sim's atomic
/// `process_frame` (design §3). This is the SINGLE owner of the ordering — the
/// binary's tick loop calls it, so the decrement-before / max-after invariant
/// lives in exactly one place.
///
/// After this returns, `sim` holds the post-tick state (its `sound_events` are
/// still populated for the caller's audio drain) and each viewport carries its
/// stepped `shake`/`banner_y`; the render's `Viewport::process` then does the
/// phase-5 centering + shake-RNG using the post-worm-loop worm state.
pub fn tick_viewports(viewports: &mut [Viewport], sim: &mut SimState, inputs: &[ControlState]) {
    // BEFORE: previous shake, pre-`++cycles` cycles, pre-worm-loop killed_timer.
    step_before_frame(viewports, sim.cycles, &sim.worms);
    // The atomic sim tick: phase-1 screen_flash-- and phases 2-6; explosion
    // sobject creation emits (x, y, amount) shake events into sim.shake_events.
    sim.process_frame(inputs);
    // AFTER: the phase-2 explosion-max, no shake reader having run in between.
    let events = sim.drain_shake_events();
    apply_shake_events(viewports, &events);
}
