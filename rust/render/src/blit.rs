//! Sprite/line blit primitives. Ports `blit.cpp:239-729` verbatim in math,
//! idiomatic in API. Every image blit shares `CLIP_IMAGE` (`macros.hpp:3-22`);
//! index 0 is the transparent hole (`blit.cpp:252`).
//!
//! **API deviation from the brief.** The brief assumed `assets::sprite::Sprite`
//! with `.mem/.width/.height/.pitch`. The `assets` crate (slices 1-2) has no
//! per-sprite `Sprite` view — only `SpriteSet` (`sprite.rs`): fields
//! `width/height/count/data`, accessor `sprite(frame) -> &[u8]` returning the
//! `width*height` contiguous indices of one frame. A single sprite's `pitch`
//! therefore equals `width` (frames are stored back-to-back, each row-major and
//! gap-free). So the blits take `(spr: &SpriteSet, frame: usize)` and derive
//! `mem = spr.sprite(frame)`, `width/height = spr.width/height`, `pitch = width`.
//!
//! **`pal` is an explicit arg** (not stored on `Bitmap`): the C++ `scr.pal32`
//! member becomes a `pal: &Pal32` parameter, matching 3a's `set_pixel(.., pal)`
//! convention (`bitmap.rs`). All T1-T5 blits carry `pal: &Pal32`.

use crate::bitmap::{Bitmap, Pal32, Rect};
use assets::sprite::SpriteSet;

/// `macros.hpp:3-22` CLIP_IMAGE: clamp a `w x h` blit at `(x,y)` to `clip`,
/// sliding the source start index. Returns `None` if fully clipped, else
/// `(x, y, w, h, src_start)`.
///
/// This is the same clamp `draw_level` (`level_draw.rs:24-46`) inlines for the
/// terrain surface; ported once here as the shared image-blit clip.
fn clip_image(
    clip: Rect,
    mut x: i32,
    mut y: i32,
    mut w: i32,
    mut h: i32,
    pitch: i32,
) -> Option<(i32, i32, i32, i32, i32)> {
    let mut src = 0i32;
    let top = y - clip.y1;
    if top < 0 {
        src += -top * pitch;
        h += top;
        y = clip.y1;
    }
    let bottom = y + h - clip.y2;
    if bottom > 0 {
        h -= bottom;
    }
    let left = x - clip.x1;
    if left < 0 {
        src -= left;
        w += left;
        x = clip.x1;
    }
    let right = x + w - clip.x2;
    if right > 0 {
        w -= right;
    }
    if w <= 0 || h <= 0 {
        None
    } else {
        Some((x, y, w, h, src))
    }
}

/// `blit.cpp:239-262` BlitImage: index-0-transparent sprite blit; writes
/// `pal[c]` for every source index `c != 0`.
pub fn blit_image(scr: &mut Bitmap, pal: &Pal32, spr: &SpriteSet, frame: usize, x: i32, y: i32) {
    let pitch = spr.width;
    let (x, y, w, h, src) = match clip_image(scr.clip, x, y, spr.width, spr.height, pitch) {
        Some(v) => v,
        None => return,
    };
    let mem = spr.sprite(frame);
    let scr_pitch = scr.pitch;
    for dy in 0..h {
        for dx in 0..w {
            let c = mem[(src + dy * pitch + dx) as usize];
            if c != 0 {
                scr.pixels[((y + dy) * scr_pitch + (x + dx)) as usize] = pal[c as usize];
            }
        }
    }
}

/// `blit.cpp:264-287` BlitImageTrans: as `blit_image`, plus a checkerboard gate.
/// The C++ `(x ^ y ^ phase) & 1` uses the *loop-local* `x`/`y` counters, which
/// restart at 0 each blit AFTER `CLIP_IMAGE` has already advanced `mem`/the
/// screen pointer — so the checker phase is relative to the CLIPPED sprite
/// origin, not the screen. `dx`/`dy` here are exactly those post-clip counters.
pub fn blit_image_trans(
    scr: &mut Bitmap,
    pal: &Pal32,
    spr: &SpriteSet,
    frame: usize,
    x: i32,
    y: i32,
    phase: i32,
) {
    let pitch = spr.width;
    let (x, y, w, h, src) = match clip_image(scr.clip, x, y, spr.width, spr.height, pitch) {
        Some(v) => v,
        None => return,
    };
    let mem = spr.sprite(frame);
    let scr_pitch = scr.pitch;
    for dy in 0..h {
        for dx in 0..w {
            let c = mem[(src + dy * pitch + dx) as usize];
            if c != 0 && ((dx ^ dy ^ phase) & 1) != 0 {
                scr.pixels[((y + dy) * scr_pitch + (x + dx)) as usize] = pal[c as usize];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitmap::{Bitmap, Rect};

    const SENTINEL: u32 = 0xDEAD_BEEF;

    // pal[i] = 0xFF000000 | i (distinct per index so a pixel reveals its index).
    fn ramp_pal() -> Pal32 {
        let mut p = [0u32; 256];
        for (i, e) in p.iter_mut().enumerate() {
            *e = 0xFF00_0000 | i as u32;
        }
        p
    }

    fn sprite(width: i32, height: i32, data: Vec<u8>) -> SpriteSet {
        assert_eq!(data.len() as i32, width * height);
        SpriteSet { width, height, count: 1, data }
    }

    fn filled(w: i32, h: i32) -> Bitmap {
        let mut b = Bitmap::new(w, h);
        for p in b.pixels.iter_mut() {
            *p = SENTINEL;
        }
        b
    }

    #[test]
    fn blit_image_index0_is_transparent_and_uses_pal32() {
        // 2x2 sprite: [1, 0 / 0, 2]; index 0 must NOT write.
        let pal = ramp_pal();
        let spr = sprite(2, 2, vec![1, 0, 0, 2]);
        let mut b = filled(2, 2); // clip = full
        blit_image(&mut b, &pal, &spr, 0, 0, 0);
        assert_eq!(b.pixels[0], 0xFF00_0001, "(0,0) = pal[1]");
        assert_eq!(b.pixels[1], SENTINEL, "(1,0) index 0 transparent");
        assert_eq!(b.pixels[2], SENTINEL, "(0,1) index 0 transparent");
        assert_eq!(b.pixels[3], 0xFF00_0002, "(1,1) = pal[2]");
    }

    #[test]
    fn blit_image_clips_to_clip_rect() {
        // 2x2 all-index-1 sprite at (1,1); clip = [0,2)x[0,2). CLIP_IMAGE clamps
        // to the single inside cell (1,1); the outside cells stay sentinel
        // (clamp, not whole-sprite skip).
        let pal = ramp_pal();
        let spr = sprite(2, 2, vec![1, 1, 1, 1]);
        let mut b = filled(4, 4);
        b.clip = Rect::new(0, 0, 2, 2);
        blit_image(&mut b, &pal, &spr, 0, 1, 1);
        assert_eq!(b.pixels[1 * 4 + 1], 0xFF00_0001, "(1,1) inside clip");
        assert_eq!(b.pixels[1 * 4 + 2], SENTINEL, "(2,1) clipped out");
        assert_eq!(b.pixels[2 * 4 + 1], SENTINEL, "(1,2) clipped out");
        assert_eq!(b.pixels[2 * 4 + 2], SENTINEL, "(2,2) clipped out");
    }

    #[test]
    fn blit_image_trans_checkerboard_phase() {
        // Full (all-index-1) 2x2 sprite, clip full: (dx ^ dy ^ phase) & 1 gate.
        let pal = ramp_pal();
        let spr = sprite(2, 2, vec![1, 1, 1, 1]);

        // phase 0 writes (1,0) and (0,1); the two diagonal cells stay sentinel.
        let mut b = filled(2, 2);
        blit_image_trans(&mut b, &pal, &spr, 0, 0, 0, 0);
        assert_eq!(b.pixels[0], SENTINEL, "phase0 (0,0) checker off");
        assert_eq!(b.pixels[1], 0xFF00_0001, "phase0 (1,0) checker on");
        assert_eq!(b.pixels[2], 0xFF00_0001, "phase0 (0,1) checker on");
        assert_eq!(b.pixels[3], SENTINEL, "phase0 (1,1) checker off");

        // phase 1 inverts the checker.
        let mut b = filled(2, 2);
        blit_image_trans(&mut b, &pal, &spr, 0, 0, 0, 1);
        assert_eq!(b.pixels[0], 0xFF00_0001, "phase1 (0,0) checker on");
        assert_eq!(b.pixels[1], SENTINEL, "phase1 (1,0) checker off");
        assert_eq!(b.pixels[2], SENTINEL, "phase1 (0,1) checker off");
        assert_eq!(b.pixels[3], 0xFF00_0001, "phase1 (1,1) checker on");

        // Index 0 stays transparent even where the checker bit is set:
        // sprite [1,0 / 1,0], phase 0 sets the checker at (1,0) and (0,1).
        // (1,0) is index 0 -> must stay sentinel; (0,1) is index 1 -> writes.
        let spr2 = sprite(2, 2, vec![1, 0, 1, 0]);
        let mut b = filled(2, 2);
        blit_image_trans(&mut b, &pal, &spr2, 0, 0, 0, 0);
        assert_eq!(b.pixels[1], SENTINEL, "checker-on but index 0 -> transparent");
        assert_eq!(b.pixels[2], 0xFF00_0001, "checker-on and index 1 -> writes");
    }

    #[test]
    fn blit_image_trans_checker_uses_post_clip_counters() {
        // CRITICAL (brief): the checker phase is relative to the CLIPPED sprite
        // origin, not the screen. 3x3 all-index-1 sprite at (0,0); clip clamps
        // the LEFT edge by 1 (x -> 1, y stays 0). Post-clip counters give
        // (dx ^ dy) & 1; screen-absolute ((x+dx) ^ (y+dy)) would INVERT it
        // (x=1 shifts parity), so this distinguishes the two.
        let pal = ramp_pal();
        let spr = sprite(3, 3, vec![1; 9]);
        let mut b = filled(4, 4);
        b.clip = Rect::new(1, 0, 4, 4);
        blit_image_trans(&mut b, &pal, &spr, 0, 0, 0, 0);
        // Post-clip local checker: (0,0) off, (1,0) on, (0,1) on, ...
        assert_eq!(b.pixels[0 * 4 + 1], SENTINEL, "(1,0) local (0,0) checker off");
        assert_eq!(b.pixels[0 * 4 + 2], 0xFF00_0001, "(2,0) local (1,0) checker on");
        assert_eq!(b.pixels[1 * 4 + 1], 0xFF00_0001, "(1,1) local (0,1) checker on");
        assert_eq!(b.pixels[1 * 4 + 2], SENTINEL, "(2,1) local (1,1) checker off");
    }
}
