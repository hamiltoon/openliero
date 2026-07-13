//! Slice-4d LIVE flash/shake/banner/centering golden — THE MILESTONE.
//!
//! The scenario (`render_slice4d_live_scenario.txt`) drives worm0's EXPLOSIVES
//! into the low-health worm1 (the 5d death+respawn geometry) with the two
//! viewports wired LIVE, so the render-only side effects Step 3 deferred now run
//! from real events: the `large_explosion` (flash=8, shake=4) blips the palette
//! and jitters worm1's centred viewport, worm1's death walks its `banner_y`, and
//! the respawn makes the camera follow the live worm as it falls. A second worm0
//! fire (tick 155, reloaded) lands a clean explosion during the countdown so the
//! shake window overlaps the post-respawn camera-follow.
//!
//! [`run`] gates the **bit-exactness**: every tick's `frame_hash` == the C++
//! `render_live` sidecar, `state_hash` == `hash_game_state` == the sim-golden
//! master (the isolation triple), plus the folded `total` and row count. This
//! test adds the **non-vacuity** proofs — that each effect is LIVE pixels/state,
//! not a golden that passed while the effect did nothing:
//!   * FLASH: suppressing `screen_flash` on a flash tick changes the frame.
//!   * SHAKE: a shaking tick's viewport `(x,y)` differs from the shake-suppressed
//!     centering-only re-render (the viewport-local RNG jitter is live).
//!   * BANNER: `banner_y` walks the full `[-8, 2]` range and the death resets it
//!     to `-8` in a single tick (the `killed_timer==150` reset, not the ±1 walk).
//!   * CENTERING: the shake-suppressed (pure-centering) camera `y` moves across
//!     the post-respawn fall — the camera follows the live worm — while worm0's
//!     viewport (never dead/respawned) stays pinned at the origin.

mod render_slice4d_common;
use render_slice4d_common::{modified, run};

#[test]
fn render_slice4d_live_frame_hash_matches_cpp_oracle() {
    // The MILESTONE gate: `run` asserts, per tick, frame_hash == sidecar,
    // state_hash == hash_game_state == sim master (triple isolation), the folded
    // `total`, and the row count. A green `run` IS the bit-exact live golden.
    let r = run("live");

    // ---- FLASH non-vacuity ----------------------------------------------------
    // The first `large_explosion` raises screen_flash=8; it decays one/tick. On a
    // flash tick the palette LightUp brightens every non-black pixel, so dropping
    // it to 0 must change the frame.
    let flash_tick = 88usize;
    assert!(
        r.screen_flash[flash_tick] > 0,
        "tick {flash_tick}: the live screen_flash must be raised by the explosion"
    );
    let (fh_no_flash, _) = modified("live", flash_tick as u32, Some(0), false);
    assert_ne!(
        r.frame_hashes[flash_tick], fh_no_flash,
        "tick {flash_tick}: flash-on frame must differ from screen_flash=0 (the LightUp blip is live)"
    );

    // ---- SHAKE non-vacuity ----------------------------------------------------
    // The second explosion (~tick 206) sets vp1.shake = itof(4); the viewport-local
    // RNG then offsets (x,y) by +/-Ftoi(shake) each tick. A shake-suppressed
    // re-render of the same tick is pure centering (no jitter) — the two (x,y)
    // must differ.
    let shake_tick = 207usize;
    assert!(
        r.vps[shake_tick][1].shake >= 1 << 16,
        "tick {shake_tick}: vp1.shake must be >= itof(1) so the render RNG draws"
    );
    let live_xy = (r.vps[shake_tick][1].x, r.vps[shake_tick][1].y);
    let (_, centering_only) = modified("live", shake_tick as u32, None, true);
    assert_ne!(
        live_xy, centering_only[1],
        "tick {shake_tick}: vp1 (x,y) with shake must differ from the centering-only \
         (shake-suppressed) re-render — the shake RNG jitter is live"
    );

    // ---- BANNER walk + death reset --------------------------------------------
    // worm1's viewport banner_y walks toward +2 while its worm's killed_timer>16
    // and toward -8 otherwise, stepping +/-1 every OTHER cycle — so it spans the
    // full [-8, 2]. worm1's death resets it to -8 in ONE tick (the
    // killed_timer==KILLED_TIMER_INITIAL reset in Viewport::process), a jump the
    // gradual +/-1 walk can never make.
    let bmin = (0..=r.ticks)
        .map(|t| r.vps[t as usize][1].banner_y)
        .min()
        .unwrap();
    let bmax = (0..=r.ticks)
        .map(|t| r.vps[t as usize][1].banner_y)
        .max()
        .unwrap();
    assert_eq!(
        (bmin, bmax),
        (-8, 2),
        "vp1 banner_y walks the full [-8, 2] range"
    );
    let reset = (1..=r.ticks).any(|t| {
        let prev = r.vps[(t - 1) as usize][1].banner_y;
        let cur = r.vps[t as usize][1].banner_y;
        cur == -8 && prev - cur > 1 // a >1 drop to -8 == the death reset, not the walk
    });
    assert!(
        reset,
        "vp1 banner_y must reset to -8 in a single >1 step at the death (killed_timer==150)"
    );

    // ---- CENTERING non-vacuity (camera follows the live worm) -----------------
    // After respawn worm1 is alive+visible with killed_timer<=0, so the
    // alive-visible arm SetCenters on it as it falls. Compare the PURE-centering
    // (shake-suppressed) camera y at two post-respawn ticks: the worm fell, so y
    // moves — the live camera follow.
    let (_, early) = modified("live", 240, None, true);
    let (_, late) = modified("live", 254, None, true);
    assert_ne!(
        early[1].1, late[1].1,
        "vp1 pure-centering y must move across the post-respawn fall (camera follows the worm)"
    );

    // Control: worm0 never dies/respawns (killed_timer stays 150), so neither
    // centering arm fires and its viewport stays pinned at the world origin for
    // every tick — proving the centering above is a real, worm-specific follow.
    for t in 0..=r.ticks as usize {
        assert_eq!(
            (r.vps[t][0].x, r.vps[t][0].y),
            (0, 0),
            "tick {t}: worm0's viewport is fixed at the origin (never centers)"
        );
        assert_eq!(
            r.vps[t][0].shake, 0,
            "tick {t}: worm0's viewport never shakes"
        );
    }
}
