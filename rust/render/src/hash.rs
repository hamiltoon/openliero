//! FNV-1a frame hash over the ARGB buffer, R,G,B order, alpha dropped, with a
//! composition-time fade. Port of `framehash_main.cpp:26-50`. The regression
//! primitive: two renderers agree iff their frame hashes agree.

use crate::bitmap::Bitmap;

/// `framehash_main.cpp:26`.
pub const FNV_OFFSET: u64 = 1469598103934665603;
/// `framehash_main.cpp:27`.
pub const FNV_PRIME: u64 = 1099511628211;

/// `framehash_main.cpp:32-34`: identity at `amount>=32`, else `(v*amount)>>5`.
pub fn fade_channel(v: u8, amount: i32) -> u8 {
    if amount >= 32 {
        v
    } else {
        ((v as i32 * amount) >> 5) as u8
    }
}

/// `framehash_main.cpp:38-50`: FNV-1a over each pixel's R,G,B (alpha dropped),
/// faded, row-major. `fade` is 0 on frame 0 (black), 33 afterwards (identity).
pub fn hash_frame(bmp: &Bitmap, fade: i32) -> u64 {
    let mut h = FNV_OFFSET;
    for y in 0..bmp.h {
        for x in 0..bmp.w {
            let c = bmp.pixels[(y * bmp.pitch + x) as usize];
            let r = fade_channel(((c >> 16) & 0xFF) as u8, fade);
            let g = fade_channel(((c >> 8) & 0xFF) as u8, fade);
            let b = fade_channel((c & 0xFF) as u8, fade);
            h = (h ^ r as u64).wrapping_mul(FNV_PRIME);
            h = (h ^ g as u64).wrapping_mul(FNV_PRIME);
            h = (h ^ b as u64).wrapping_mul(FNV_PRIME);
        }
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fade_channel_matches_framehash() {
        // fade>=32 is identity.
        assert_eq!(fade_channel(200, 32), 200);
        assert_eq!(fade_channel(200, 33), 200);
        // fade<32: (v*amount)>>5. v=200, a=16 -> (3200)>>5 = 100.
        assert_eq!(fade_channel(200, 16), 100);
        // fade 0 -> black.
        assert_eq!(fade_channel(255, 0), 0);
    }

    #[test]
    fn hash_frame_fnv1a_rgb_order_one_pixel() {
        use crate::bitmap::{Bitmap, Rect};
        // 1x1 surface, pixel = 0xFF_11_22_33 (A=FF,R=11,G=22,B=33).
        let b = Bitmap {
            w: 1,
            h: 1,
            pitch: 1,
            pixels: vec![0xFF11_2233],
            clip: Rect::new(0, 0, 1, 1),
            cycles: 0,
        };
        // Hand FNV-1a over bytes 0x11,0x22,0x33 (R,G,B), fade=33 (identity).
        let mut h = FNV_OFFSET;
        for byte in [0x11u8, 0x22, 0x33] {
            h = (h ^ byte as u64).wrapping_mul(FNV_PRIME);
        }
        assert_eq!(hash_frame(&b, 33), h);
    }

    #[test]
    fn hash_frame_fade_zero_is_all_black_constant() {
        use crate::bitmap::{Bitmap, Rect};
        // Two DIFFERENT 2x1 frames hash to the SAME value at fade=0 (all channels ->0).
        let a = Bitmap {
            w: 2,
            h: 1,
            pitch: 2,
            pixels: vec![0xFFAA_BBCC, 0xFF01_0203],
            clip: Rect::new(0, 0, 2, 1),
            cycles: 0,
        };
        let b = Bitmap {
            w: 2,
            h: 1,
            pitch: 2,
            pixels: vec![0xFF99_8877, 0xFF44_5566],
            clip: Rect::new(0, 0, 2, 1),
            cycles: 0,
        };
        assert_eq!(hash_frame(&a, 0), hash_frame(&b, 0));
        // And it equals FNV over 6 zero bytes.
        let mut h = FNV_OFFSET;
        for _ in 0..6 {
            h = (h ^ 0u64).wrapping_mul(FNV_PRIME);
        }
        assert_eq!(hash_frame(&a, 0), h);
    }

    #[test]
    fn hash_frame_ignores_alpha() {
        use crate::bitmap::{Bitmap, Rect};
        let a = Bitmap {
            w: 1,
            h: 1,
            pitch: 1,
            pixels: vec![0x0011_2233],
            clip: Rect::new(0, 0, 1, 1),
            cycles: 0,
        };
        let b = Bitmap {
            w: 1,
            h: 1,
            pitch: 1,
            pixels: vec![0xFF11_2233],
            clip: Rect::new(0, 0, 1, 1),
            cycles: 0,
        };
        assert_eq!(hash_frame(&a, 33), hash_frame(&b, 33), "alpha dropped");
    }
}
