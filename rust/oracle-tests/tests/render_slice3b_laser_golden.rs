//! Slice-3b LASER frame-hash golden — the viewport-RNG trap.
//!
//! Two stationary RIFLE (laserSight=true) worms. In the palette-constant window
//! ticks 1..7 the ONLY per-tick mover is `DrawLaserSight`'s per-viewport `rand`,
//! so the frame hash advances tick-to-tick even though the worms are motionless.
//! Non-vacuity: >=5 distinct hashes in t1..7 (a renderer that no-ops the laser
//! yields ONE), and BOTH viewports' RNGs actually advanced (draws > 0).

mod render_slice3b_common;
use render_slice3b_common::{distinct, run};

#[test]
fn render_slice3b_laser_frame_hash_matches_cpp_oracle() {
    let r = run("laser");

    // Non-vacuity 1: the palette-constant window t1..=7 holds >=5 distinct frame
    // hashes — the per-viewport RNG is provably live (FAN yields a single hash).
    let window = &r.frame_hashes[1..=7];
    assert!(
        distinct(window) >= 5,
        "laser t1..7 must hold >=5 distinct frame hashes (RNG live); saw {} in {window:02x?}",
        distinct(window)
    );

    // Non-vacuity 2: both viewport RNGs left their default (draws taken). A laser
    // that silently no-ops would leave both at 0.
    assert!(
        r.vp_rand_draws[0] > 0 && r.vp_rand_draws[1] > 0,
        "both viewport RNGs must advance (worms both hold RIFLE); draws = {:?}",
        r.vp_rand_draws
    );
}
