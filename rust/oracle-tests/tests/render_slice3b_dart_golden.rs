//! Slice-3b DART frame-hash golden — nobject debris + terrain carve.
//!
//! worm0 lobs a DART that explodes on the floor at ~t26, spraying nobject debris
//! and carving the level. Over the non-water render_stage sky the sobject's
//! `BlitImageR` paints nothing (its POSITIVE water path is proven separately in
//! `render_slice3b_dart_water`), so the on-screen motion is the nobject debris.
//! Non-vacuity: at peak nobject occupancy, emptying the nobjects pool changes the
//! frame hash — the debris sprites provably paint.

mod render_slice3b_common;
use render_slice3b_common::{modified_frame_hash, run, Drop};

#[test]
fn render_slice3b_dart_frame_hash_matches_cpp_oracle() {
    let r = run("dart");

    let t = r.peak_tick(Drop::Nobjects);
    let dropped = modified_frame_hash("dart", t, None, Drop::Nobjects);
    assert_ne!(
        r.frame_hashes[t as usize], dropped,
        "tick {t}: emptying the nobjects pool must change the frame (debris paints)"
    );
}
