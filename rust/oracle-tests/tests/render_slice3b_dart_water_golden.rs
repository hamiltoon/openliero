//! Slice-3b DART-OVER-WATER frame-hash golden — the POSITIVE `BlitImageR` path.
//!
//! Identical DART directives to `render_slice3b_dart`, but on `water_stage.lev`
//! whose sky band is water-range palette 160. The `small_explosion` sobject
//! straddles the water/dirt boundary at ~t26; `BlitImageR` (the sole water-gated
//! sobject drawer) paints its upper rows over the water pixels. Non-vacuity: at
//! peak sobject occupancy, emptying the sobjects pool changes the frame hash —
//! the strongest available proof that `BlitImageR` actually painted (over plain
//! render_stage sky it would paint nothing, so this is the meaningful control).

mod render_slice3b_common;
use render_slice3b_common::{modified_frame_hash, run, Drop};

#[test]
fn render_slice3b_dart_water_frame_hash_matches_cpp_oracle() {
    let r = run("dart_water");

    let t = r.peak_tick(Drop::Sobjects);
    let dropped = modified_frame_hash("dart_water", t, None, Drop::Sobjects);
    assert_ne!(
        r.frame_hashes[t as usize], dropped,
        "tick {t}: emptying the sobjects pool must change the frame over water \
         (BlitImageR paints the explosion over the water band)"
    );
}
