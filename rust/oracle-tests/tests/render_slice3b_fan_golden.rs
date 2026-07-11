//! Slice-3b FAN frame-hash golden — the wobject SPRITE arm (`BlitImage`).
//!
//! worm0 fires the FAN; the wobjects fly downrange for ~24 ticks. Non-vacuity:
//! at the peak-occupancy tick, emptying the wobjects pool changes the frame hash
//! — the wobject sprites provably paint pixels.

mod render_slice3b_common;
use render_slice3b_common::{modified_frame_hash, run, Drop};

#[test]
fn render_slice3b_fan_frame_hash_matches_cpp_oracle() {
    let r = run("fan");

    let t = r.peak_tick(Drop::Wobjects);
    let dropped = modified_frame_hash("fan", t, None, Drop::Wobjects);
    assert_ne!(
        r.frame_hashes[t as usize], dropped,
        "tick {t}: emptying the wobjects pool must change the frame (sprites paint)"
    );
}
