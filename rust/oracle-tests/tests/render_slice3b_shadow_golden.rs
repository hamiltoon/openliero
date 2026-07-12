//! Slice-3b SHADOW frame-hash golden — the shadow pass over kSeeShadow terrain.
//!
//! `render_shadow` flips the draw-time shadow pass on. Non-vacuity: the same
//! tick rendered with `draw_shadow=false` yields a DIFFERENT hash — proving the
//! shadow pixels actually landed (not a no-op behind the directive).

mod render_slice3b_common;
use render_slice3b_common::{modified_frame_hash, run, Drop};

#[test]
fn render_slice3b_shadow_frame_hash_matches_cpp_oracle() {
    let r = run("shadow");

    // Non-vacuity: shadow ON (the golden) vs the SAME state with the shadow pass
    // forced OFF must differ — the shadow pixels are real.
    let off_t1 = modified_frame_hash("shadow", 1, Some(false), Drop::None);
    assert_ne!(
        r.frame_hashes[1], off_t1,
        "tick 1 shadow-ON hash must differ from the shadow-OFF control render"
    );
}
