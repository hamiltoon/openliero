//! Slice-3b BLOOD frame-hash golden — the bobject blood pool (`SetPixel`).
//!
//! A DART into worm0's feet wounds it (health 100->95, survives) and sprays a
//! blood pool that drips into the `bobjects` pool for ~30 ticks. Non-vacuity: at
//! peak bobject occupancy, emptying the bobjects pool changes the frame hash —
//! the blood pixels provably paint.

mod render_slice3b_common;
use render_slice3b_common::{modified_frame_hash, run, Drop};

#[test]
fn render_slice3b_blood_frame_hash_matches_cpp_oracle() {
    let r = run("blood");

    let t = r.peak_tick(Drop::Bobjects);
    let dropped = modified_frame_hash("blood", t, None, Drop::Bobjects);
    assert_ne!(
        r.frame_hashes[t as usize], dropped,
        "tick {t}: emptying the bobjects pool must change the frame (blood paints)"
    );
}
