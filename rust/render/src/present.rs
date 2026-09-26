//! Presentation (Step 4½d): the composition fade C++ applies when it copies the back buffer to
//! the window. `Gfx::Flip` → `Gfx::Draw` → `ScaleDraw(..., renderer.fade_value)`
//! (`gfx.cpp:1033-1038`, `blit.cpp:798-826`): at `fade >= 32` the pixels are copied verbatim;
//! below it every pixel goes through `FadeArgb` (`blit.cpp:788-796`), `(v * fade) >> 5` per
//! channel with alpha forced to 0xFF. The same arithmetic as the frame hash's `fade_channel`
//! (`hash.rs`), so `hash_frame(bmp, f)` is the hash of what the window shows.

use crate::bitmap::Bitmap;
use crate::hash::fade_channel;

/// `FadeArgb` (`blit.cpp:788-796`) as `ScaleDraw` applies it (`:801-815`): `fade >= 32` is the
/// verbatim copy, anything below fades each channel and forces alpha to 0xFF.
pub fn fade_argb(c: u32, amount: i32) -> u32 {
    if amount >= 32 {
        return c;
    }
    let r = fade_channel(((c >> 16) & 0xFF) as u8, amount) as u32;
    let g = fade_channel(((c >> 8) & 0xFF) as u8, amount) as u32;
    let b = fade_channel((c & 0xFF) as u8, amount) as u32;
    0xFF00_0000 | (r << 16) | (g << 8) | b
}

/// The window image of `bmp` at `amount`, as RGBA bytes (row-major, `bmp.w * bmp.h * 4`): what
/// the live game uploads (Step 4½d: the first live fade).
pub fn fade_into_rgba(bmp: &Bitmap, amount: i32, out: &mut [u8]) {
    debug_assert_eq!(out.len(), (bmp.w * bmp.h * 4) as usize);
    for y in 0..bmp.h {
        for x in 0..bmp.w {
            let px = fade_argb(bmp.pixels[(y * bmp.pitch + x) as usize], amount);
            let o = ((y * bmp.w + x) * 4) as usize;
            out[o] = ((px >> 16) & 0xFF) as u8;
            out[o + 1] = ((px >> 8) & 0xFF) as u8;
            out[o + 2] = (px & 0xFF) as u8;
            out[o + 3] = ((px >> 24) & 0xFF) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::hash_frame;

    #[test]
    fn fade_argb_is_fadeargb() {
        assert_eq!(
            fade_argb(0x12C8_6420, 33),
            0x12C8_6420,
            "fade >= 32: verbatim, alpha kept"
        );
        assert_eq!(fade_argb(0x12C8_6420, 32), 0x12C8_6420);
        assert_eq!(
            fade_argb(0x00C8_6420, 16),
            0xFF64_3210,
            "(v * 16) >> 5, alpha 0xFF"
        );
        assert_eq!(fade_argb(0xFFFF_FFFF, 0), 0xFF00_0000, "fade 0: black");
    }

    #[test]
    fn the_hash_of_the_faded_frame_is_hash_frame_at_that_fade() {
        let mut bmp = Bitmap::new(7, 3);
        for (i, p) in bmp.pixels.iter_mut().enumerate() {
            *p = 0xFF00_0000 | (i as u32 * 0x0102_03);
        }
        for fade in [0, 1, 17, 31, 32, 33] {
            let mut faded = bmp.clone();
            for p in faded.pixels.iter_mut() {
                *p = fade_argb(*p, fade);
            }
            assert_eq!(
                hash_frame(&faded, 33),
                hash_frame(&bmp, fade),
                "fade {fade}"
            );
        }
    }

    #[test]
    fn fade_into_rgba_writes_the_faded_pixels_as_rgba() {
        let mut bmp = Bitmap::new(2, 1);
        bmp.pixels = vec![0xFF10_2030, 0xFFFF_FFFF];
        let mut out = [0u8; 8];
        fade_into_rgba(&bmp, 16, &mut out);
        assert_eq!(out, [8, 16, 24, 255, 127, 127, 127, 255]);
    }
}
