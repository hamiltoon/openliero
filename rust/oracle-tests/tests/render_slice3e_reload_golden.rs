//! Slice-3e RELOAD frame-hash golden — the ammo-bar RELOAD arm (the growing
//! loading bar + the blinking "Reloading" text, `viewport.cpp:110-128`).
//!
//! worm0 wields the single-ammo RIFLE, fires once (tick 22), empties the magazine
//! and reloads for the rest of the window; both worms stay visible with full life
//! bars. The C++ dumper (full sim) generated valid `render_slice3e_reload{,_sim}`
//! goldens. The per-tick differential in `run` would prove the whole player view
//! pixel-exact vs C++, and its non-vacuity witness is that two adjacent settled
//! ticks (50 vs 51) carry different frame hashes (the loading bar steps + the
//! "Reloading" text blinks while the world is at rest).
//!
//! ## BLOCKED — deferred Rust-sim laser do-loop (NOT a HUD/render bug) [`#[ignore]`]
//!
//! RIFLE is `shotType = 4` == `ST_LASER` (`rifle.cfg:39`). The Rust sim's
//! `weapon::wobject_process` (`sim/src/weapon.rs:319`) deliberately defers the
//! laser do-loop (`debug_assert!(shot_type != ST_LASER, "laser do-loop Process
//! branch deferred")`), a standing deferral from the sim slices — the HUD slice
//! did not touch it. So the moment worm0's RIFLE shot enters `wobject_process`
//! (the fire tick, ~k=23) the Rust sim panics; the differential, which MUST drive
//! the Rust `SimState` tick-by-tick to match the sidecar, cannot advance past the
//! fire. This is a Rust sim-completeness gap, not a HUD/render defect — the C++
//! goldens are correct and stay committed as `facit`.
//!
//! This test is preserved (not deleted) and stays a loud tripwire: it will start
//! passing unchanged the moment the laser do-loop lands in the sim. It is
//! `#[ignore]`d only so the workspace suite stays green.
//!
//! ## The reload HUD arm IS proven non-vacuously today — by `render_slice3e_death`
//!
//! EXPLOSIVES is also `ammo = 1` with `loadingTime = 113` (`explosives.cfg`), and
//! it uses a PORTED shot_type. In the death scenario worm0 fires it (tick 38),
//! empties the magazine, and takes the SAME reload arm (`viewport.cpp:110-128`):
//! the loading bar + blinking "Reloading" text at `stats_x=0` for ticks ~39..140.
//! The death golden's row-by-row match (a GREEN test) therefore already pins the
//! reload/loading-bar arm pixel-exact vs C++ — the reload HUD element is proven
//! live regardless of this RIFLE scenario being ignored.

mod render_slice3e_common;
use render_slice3e_common::run;

#[test]
#[ignore = "RIFLE shotType=4 (ST_LASER) trips the deferred laser do-loop in \
            sim::weapon::wobject_process (weapon.rs:319); the Rust sim cannot \
            process the fire. Reload HUD arm is meanwhile proven by \
            render_slice3e_death (worm0 EXPLOSIVES ammo=1 reload). Un-ignore when \
            the laser do-loop lands in the sim."]
fn render_slice3e_reload_frame_hash_matches_cpp_oracle() {
    let r = run("reload");

    // Non-vacuity: two adjacent ticks deep in the settled reload window. The world
    // is at rest here; the ONLY moving pixels are the loading bar (stepping ~1px)
    // and the blinking "Reloading" text — so the frames must differ.
    assert_ne!(
        r.frame_hashes[50], r.frame_hashes[51],
        "settled reload window: ticks 50 vs 51 must differ (loading bar steps + \
         Reloading text blinks)"
    );
}
