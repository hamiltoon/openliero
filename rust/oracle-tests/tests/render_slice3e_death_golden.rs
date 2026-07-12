//! Slice-3e DEATH frame-hash golden — the DYING life-bar countdown arm
//! (`viewport.cpp:88-95`): worm1 is killed in-sim (~tick 91, `visible=false`,
//! `killed_timer=150`) and its HUD takes the else arm, drawing a countdown bar
//! `100 - (killed_timer*25)/37` that grows ~1px every ~1.5 ticks as the timer
//! ticks down. worm0 stays alive with the normal visible life bar.
//!
//! The per-tick differential in `run` proves the whole player view pixel-exact vs
//! C++ across the death + countdown window (the dying-arm bar is captured by the
//! row match — the worm is dead yet its bar is drawn). It ALSO pins the ammo-bar
//! RELOAD arm: worm0's EXPLOSIVES is `ammo=1, loadingTime=113`, so after its fire
//! (tick 38) worm0 draws the loading bar + blinking "Reloading" text at
//! `stats_x=0` for the rest of the window (`viewport.cpp:110-128`) — the reload
//! HUD arm that `render_slice3e_reload` (RIFLE) can't reach on the Rust side (its
//! ST_LASER shot trips a deferred sim branch; see that file). Non-vacuity:
//!   (a) two adjacent countdown ticks (130 vs 131) differ — the dying bar steps.
//!   (b) suppression: the SAME countdown tick with `draw_hud=false` differs — the
//!       HUD block (dying bar + worm0's bar + text + minimap) is live pixels, not a
//!       no-op behind the directive.

mod render_slice3e_common;
use render_slice3e_common::{modified_frame_hash, run};

#[test]
fn render_slice3e_death_frame_hash_matches_cpp_oracle() {
    let r = run("death");

    // (a) Two adjacent ticks in the dying-bar countdown window. worm1 is dead and
    // its killed_timer steps down, growing the countdown bar — the frames differ.
    assert_ne!(
        r.frame_hashes[130], r.frame_hashes[131],
        "countdown window: ticks 130 vs 131 must differ (the dying life bar steps)"
    );

    // (b) HUD suppression on a countdown tick: draw_hud=false drops the whole HUD
    // block, so the frame must differ from the HUD-on golden frame.
    let hud_off_t130 = modified_frame_hash("death", 130, Some(false), None);
    assert_ne!(
        r.frame_hashes[130], hud_off_t130,
        "tick 130: HUD-on frame must differ from the draw_hud=false control (the \
         dying bar + bars/text + minimap paint)"
    );
}
