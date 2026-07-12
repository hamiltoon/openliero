//! Pure, Bevy-free helpers for the 3c demo: the ARGB→RGBA blit (the one
//! byte-order-sensitive piece) and the deterministic loop step. Unit-tested
//! without a window.
use render::bitmap::Bitmap;

/// Convert `render`'s `Bitmap` (`Vec<u32>` packed `0xAARRGGBB`, `bitmap.rs`)
/// into tightly-packed RGBA8 bytes `[R,G,B,A]` for a Bevy `Image`. `out` must
/// be `w*h*4` long. Honors `pitch` (source stride in pixels) != `w`.
pub fn blit_surface_into_bytes(bmp: &Bitmap, out: &mut [u8]) {
    debug_assert_eq!(out.len(), (bmp.w * bmp.h * 4) as usize);
    for y in 0..bmp.h {
        for x in 0..bmp.w {
            let px = bmp.pixels[(y * bmp.pitch + x) as usize];
            let o = ((y * bmp.w + x) * 4) as usize;
            out[o] = ((px >> 16) & 0xff) as u8; // R
            out[o + 1] = ((px >> 8) & 0xff) as u8; // G
            out[o + 2] = (px & 0xff) as u8; // B
            out[o + 3] = ((px >> 24) & 0xff) as u8; // A
        }
    }
}

/// Loop step for the demo. Returns `(tick+1, false)` until the increment would
/// pass `ticks`, then `(0, true)` — the caller rebuilds from the loader at 0.
pub fn next_tick(tick: u32, ticks: u32) -> (u32, bool) {
    if tick + 1 > ticks {
        (0, true)
    } else {
        (tick + 1, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use render::bitmap::Bitmap;

    #[test]
    fn argb_u32_becomes_rgba_byte_order() {
        // One pixel 0xFF_2A_00_00 (A=FF, R=2A, G=00, B=00) -> [0x2A,0,0,0xFF].
        let mut bmp = Bitmap::new(1, 1);
        bmp.pixels[0] = 0xFF_2A_00_00;
        let mut out = vec![0u8; 4];
        blit_surface_into_bytes(&bmp, &mut out);
        assert_eq!(out, [0x2A, 0x00, 0x00, 0xFF], "[R,G,B,A]");
    }

    #[test]
    fn non_ff_alpha_and_all_channels_round_trip() {
        // 0x80_11_22_33 -> R=0x11 G=0x22 B=0x33 A=0x80.
        let mut bmp = Bitmap::new(1, 1);
        bmp.pixels[0] = 0x80_11_22_33;
        let mut out = vec![0u8; 4];
        blit_surface_into_bytes(&bmp, &mut out);
        assert_eq!(out, [0x11, 0x22, 0x33, 0x80]);
    }

    #[test]
    fn respects_pitch_greater_than_width() {
        // 2x2 image with pitch 4 (stride padding). A single-row (h=1) fixture
        // can't distinguish `y*pitch+x` from a buggy `y*w+x` — with h=1, y is
        // always 0 and both formulas collapse to the same index. h=2 forces
        // row 1's source read through the padded stride: row 0 = real px, pad,
        // pad; row 1 = real px, pad, pad. Pad columns get sentinel values
        // (0x99/0xAA/0xBB/0xCC) that don't match any expected output byte, so
        // if the implementation ever indexes by `w` instead of `pitch`, row 1
        // reads from row 0's pad cells and the assertion below fails loudly.
        let mut bmp = Bitmap::new(2, 2);
        bmp.pitch = 4; // stride wider than width (guard the w-vs-pitch bug)
        bmp.pixels = vec![
            0xFF_11_00_00, 0xFF_22_00_00, 0xFF_99_00_00, 0xFF_AA_00_00, // row0: px0, px1, pad, pad
            0xFF_33_00_00, 0xFF_44_00_00, 0xFF_BB_00_00, 0xFF_CC_00_00, // row1: px0, px1, pad, pad
        ];
        let mut out = vec![0u8; 16];
        blit_surface_into_bytes(&bmp, &mut out);
        assert_eq!(
            out,
            [
                0x11, 0, 0, 0xFF, // (0,0)
                0x22, 0, 0, 0xFF, // (1,0)
                0x33, 0, 0, 0xFF, // (0,1) — from source row 1, not row 0's pad
                0x44, 0, 0, 0xFF, // (1,1) — from source row 1, not row 0's pad
            ]
        );
    }

    #[test]
    fn next_tick_wraps_at_ticks() {
        assert_eq!(next_tick(0, 40), (1, false));
        assert_eq!(next_tick(39, 40), (40, false));
        assert_eq!(next_tick(40, 40), (0, true), "passing ticks signals reload");
    }
}
