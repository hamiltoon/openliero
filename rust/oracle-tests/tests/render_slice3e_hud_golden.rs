//! Slice-3e HUD frame-hash golden — the STATIC HUD elements over the full player
//! view (life bar + kills/lives text + 52×36 minimap with worm dots).
//!
//! Reuses the 3b BLOOD inputs (worm0 DARTs its own feet → health 95, both worms
//! stay visible; KillEmAll) and adds `render_hud`. The per-tick differential in
//! `run` already proves the whole 320×200 surface — world + HUD + minimap — is
//! pixel-exact vs C++ for every tick. Non-vacuity proves each reachable element
//! actually paints:
//!   (a) HUD-on ≠ frozen world-only: the 3e tick-1 frame hash differs from
//!       `render_slice3b_blood.txt`'s tick-1 (the HUD/minimap paint extra pixels).
//!   (b) minimap on/off: the same tick with `map=false` (HUD still on) differs —
//!       the minimap terrain + worm dots are real pixels, not a no-op behind the
//!       directive.
//! The worm-DOT *movement* is covered by the per-tick full-window match in `run`:
//! both worms move across the 40-tick window, so a mislocated dot on ANY tick
//! would break that tick's frame hash. (a)+(b) is the simplest sufficient proof —
//! the sharper "dot pair" form buys nothing over the exhaustive per-tick gate.

mod render_slice3e_common;
use render_slice3e_common::{modified_frame_hash, read_slice3b_frame_hash, run};

#[test]
fn render_slice3e_hud_frame_hash_matches_cpp_oracle() {
    let r = run("hud");

    // (a) HUD-on (this golden) vs the FROZEN world-only 3b BLOOD frame at the same
    // tick. Tick 1 is the witness: tick 0 is hashed with fade=0 (all-black) so it
    // is identical across scenarios; the HUD's paint first shows at tick 1. Read
    // the 3b hash from disk (never hard-coded) so the assert stays honest.
    let blood_t1 = read_slice3b_frame_hash("blood", 1);
    assert_ne!(
        r.frame_hashes[1], blood_t1,
        "tick 1: HUD-on frame must differ from the frozen world-only 3b BLOOD frame \
         (the HUD bars/text + minimap paint extra pixels)"
    );

    // (b) minimap suppression: the SAME state at tick 1 with `map=false` (HUD still
    // on) must differ — the 52×36 minimap terrain + worm dots are live pixels.
    let map_off_t1 = modified_frame_hash("hud", 1, None, Some(false));
    assert_ne!(
        r.frame_hashes[1], map_off_t1,
        "tick 1: map-ON frame must differ from the map-OFF control (minimap + worm \
         dots paint)"
    );
}
