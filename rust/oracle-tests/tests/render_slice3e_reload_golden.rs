//! Slice-3e RELOAD frame-hash golden — the ammo-bar RELOAD arm (the growing
//! loading bar + the blinking "Reloading" text, `viewport.cpp:110-128`).
//!
//! worm0 wields the single-ammo GRENADE, fires once (tick 22), empties the
//! magazine and reloads for the rest of the window; both worms stay visible with
//! full life bars. The C++ dumper (full sim) generated the `render_slice3e_reload{,
//! _sim}` goldens; the per-tick differential in `run` proves the whole player view
//! pixel-exact vs C++, and its non-vacuity witness is that two adjacent settled
//! ticks (50 vs 51) carry different frame hashes (the loading bar steps + the
//! "Reloading" text blinks while the world is at rest).
//!
//! ## GRENADE is shotType=0 (ST_NORMAL) — fully ported [T7b]
//!
//! The earlier pick, RIFLE, was `shotType = 4` == `ST_LASER` (`rifle.cfg:39`), and
//! the Rust sim's `weapon::wobject_process` (`sim/src/weapon.rs:319`) deliberately
//! defers the laser do-loop (`debug_assert!(shot_type != ST_LASER, ...)`): the
//! moment the shot entered `wobject_process` the sim panicked and the differential
//! could not advance past the fire, so the test was `#[ignore]`d. GRENADE
//! (`grenade.cfg:39`) is `shotType = 0` (ST_NORMAL), which shares the plain ported
//! flight — the differential drives the whole run.
//!
//! Two GRENADE properties keep the reload window clean: `timeToExplo = 115` means
//! the shot does NOT explode inside the 70-tick window (fire @22 -> would explode
//! ~@128), and its in-flight worm-hit arm is INERT (`hitDamage = blowAway =
//! bloodOnHit = 0`, `wormCollide = false`, so the hit gate `weapon.cpp:290-291`
//! never fires) — hence neither worm can be wounded regardless of trajectory. The
//! grenade bounces to rest downrange by ~tick 39; from there only its (unrendered)
//! `time_to_explo` countdown ticks, so the RENDERED world is identical frame to
//! frame and the settled-window witness (50 vs 51) sees only the loading bar + the
//! "Reloading" text move.
//!
//! ## The reload HUD arm is ALSO proven by `render_slice3e_death`
//!
//! EXPLOSIVES is another `ammo = 1` weapon with a ported shot_type; in the death
//! scenario worm0 fires it, empties the magazine, and takes the SAME reload arm
//! (`viewport.cpp:110-128`). GRENADE keeps this scenario independent of that one.

mod render_slice3e_common;
use render_slice3e_common::run;

#[test]
fn render_slice3e_reload_frame_hash_matches_cpp_oracle() {
    let r = run("reload");

    // Non-vacuity: two adjacent ticks deep in the settled reload window. The world
    // is at rest here (the grenade came to rest by ~tick 39; only its unrendered
    // time_to_explo counter ticks), so the ONLY moving pixels are the loading bar
    // (stepping ~1px) and the blinking "Reloading" text — so the frames must differ.
    assert_ne!(
        r.frame_hashes[50], r.frame_hashes[51],
        "settled reload window: ticks 50 vs 51 must differ (loading bar steps + \
         Reloading text blinks)"
    );
}
