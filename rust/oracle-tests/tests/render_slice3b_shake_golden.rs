//! Slice-3b SHAKE + FLASH frame-hash golden — draw-only injection (spec O4).
//!
//! `render_shake 4 0 6` shifts viewport 0 at tick 4 (2 `rand` draws + a positive
//! camera offset); because the origin-pinned camera is never re-centred the shift
//! PERSISTS (t4==t5) — shake is a STEP, not a blip (T7 concern 2). `render_flash
//! 6 40` is a clean one-tick `LightUp` palette blip at tick 6.

mod render_slice3b_common;
use render_slice3b_common::run;

#[test]
fn render_slice3b_shake_frame_hash_matches_cpp_oracle() {
    let r = run("shake");
    let f = &r.frame_hashes;

    // Shake step semantics: tick 4 (shake injected) differs from tick 3, and the
    // shift persists into tick 5 (camera never re-centred) => t4 == t5.
    assert_ne!(
        f[4], f[3],
        "tick 4 shake shifts the viewport (differs from t3)"
    );
    assert_eq!(
        f[5], f[4],
        "shake persists: t5 == t4 (camera not re-centred)"
    );

    // Flash is a clean one-tick blip: tick 6 differs from BOTH neighbours, then
    // the palette restores (t7 == t5 == t4, the persisted-shake baseline).
    assert_ne!(f[6], f[5], "tick 6 flash blip differs from t5");
    assert_ne!(f[6], f[7], "tick 6 flash blip differs from t7");
    assert_eq!(f[7], f[5], "flash restores: t7 == t5 (one-tick blip only)");
}
