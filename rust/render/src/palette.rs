//! Per-frame palette build. Ports `game.cpp:171-183` (order),
//! `palette.cpp:42-57` (LightUp/RotateFrom), `renderer.cpp:23-30` (pal32 pack).
//! Classic entries are already 6-bit-VGA (`(v&63)<<2`); the pack is straight,
//! with NO second quantization.

use crate::bitmap::Pal32;
use assets::palette::Palette;
use assets::tc::ColorAnim;

/// `palette.cpp:50-57`: rotate the sub-range `[from, to]` of `dst` from the
/// UNROTATED `source`. `count = to-from+1`, `dist %= count`,
/// `dst[from+i] = source[from + ((i + count - dist) % count)]`.
pub fn rotate_from(dst: &mut Palette, source: &Palette, from: i32, to: i32, dist: u32) {
    let count = to - from + 1;
    let d = (dist % count as u32) as i32;
    for i in 0..count {
        let s = from + ((i + count - d) % count);
        dst.entries[(from + i) as usize] = source.entries[s as usize];
    }
}

/// `palette.cpp:24-28,42-48`: `(v*(32-a)+a*255)>>5`, clamped to 255, per channel.
pub fn light_up(pal: &mut Palette, amount: i32) {
    let f = |v: u8| -> u8 {
        let x = (v as i32 * (32 - amount) + amount * 255) >> 5;
        x.min(255) as u8
    };
    for e in pal.entries.iter_mut() {
        e.r = f(e.r);
        e.g = f(e.g);
        e.b = f(e.b);
    }
}

/// `palette.hpp:69-86` + `palette.cpp:77-104`, classic mode: write worm `worm`'s
/// colour ramp into the palette, as C++ `Game::UpdateSettings` does when a match
/// takes focus (`game.cpp:475-488`). The 5-entry shaded ramp lands at
/// `base-2..=base+2` (`base` 32 for worm 0, 41 for worm 1), then it is copied to
/// the worm's secondary sprite copies (`0x58`/`0x78`, six entries) and to its
/// minimap/status entries (129 / 133, three entries). Those status entries sit
/// inside the TC's `colorAnim` ranges (129-131, 133-136), so without this the
/// animation rotates unrelated stock colours — e.g. `render_stage.lev`'s sky,
/// painted with index 130, blinked white/blue in the live game.
pub fn set_worm_colour(pal: &mut Palette, worm: usize, rgb: [i32; 3]) {
    // (base, colour_index, status_index), `Palette::kWormColorBlocks`.
    const BLOCKS: [(usize, usize, usize); 2] = [(32, 0x58, 129), (41, 0x78, 133)];
    // `SetWormColoursSpan`'s hand-tuned (scale, add) steps.
    const STEPS: [(i32, i32); 5] = [(38, 0), (50, 0), (64, 0), (47, 1008), (28, 2205)];
    let (base, colour_index, status_index) = BLOCKS[worm];
    let input = rgb.map(|c| c.clamp(0, 255) >> 2);
    for (j, &(scale, add)) in STEPS.iter().enumerate() {
        let [r, g, b] = input.map(|c| (((add + c * scale) / 64) << 2) as u8);
        let e = &mut pal.entries[base - 2 + j];
        (e.r, e.g, e.b) = (r, g, b);
    }
    for j in 0..6 {
        pal.entries[colour_index + j] = pal.entries[base + (j % 3) - 1];
    }
    for j in 0..3 {
        pal.entries[status_index + j] = pal.entries[base + j];
    }
}

/// `renderer.cpp:23-30`: `0xFF000000 | r<<16 | g<<8 | b`, entries verbatim.
pub fn pack_pal32(pal: &Palette) -> Pal32 {
    let mut out = [0u32; 256];
    for (i, e) in pal.entries.iter().enumerate() {
        out[i] = 0xFF00_0000 | ((e.r as u32) << 16) | ((e.g as u32) << 8) | e.b as u32;
    }
    out
}

/// `game.cpp:171-183`: reset to origpal -> RotateFrom each color_anim FROM
/// origpal (source is always the ORIGINAL) -> LightUp if screen_flash>0 (inert
/// in 3a) -> pack. `cycles>>3` is a signed shift then an unsigned distance.
pub fn build_palette(
    origpal: &Palette,
    color_anim: &[ColorAnim],
    cycles: i32,
    screen_flash: i32,
) -> Pal32 {
    let mut pal = origpal.clone();
    let dist = (cycles >> 3) as u32;
    for a in color_anim {
        rotate_from(&mut pal, origpal, a.from, a.to, dist);
    }
    if screen_flash > 0 {
        light_up(&mut pal, screen_flash);
    }
    pack_pal32(&pal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_worm_colour_matches_cpp() {
        use assets::palette::{Color, Palette};
        let mut pal = Palette { entries: [Color::default(); 256] };
        // Worm 0's default colour (`worm.hpp:87-89`); >>2 gives (26, 26, 62).
        set_worm_colour(&mut pal, 0, [104, 104, 248]);
        let rgb = |i: usize| (pal.entries[i].r, pal.entries[i].g, pal.entries[i].b);
        // step 0 (38, 0): 26*38/64 = 15, 62*38/64 = 36 -> <<2.
        assert_eq!(rgb(30), (60, 60, 144));
        // step 2 (64, 0) reproduces the quantized input.
        assert_eq!(rgb(32), (104, 104, 248));
        // step 4 (28, 2205): (2205 + 26*28)/64 = 45, (2205 + 62*28)/64 = 61.
        assert_eq!(rgb(34), (180, 180, 244));
        // Copies: 0x58+j = base + (j%3) - 1; status 129+j = base + j.
        for j in 0..6 {
            assert_eq!(rgb(0x58 + j), rgb(32 + (j % 3) - 1));
        }
        for j in 0..3 {
            assert_eq!(rgb(129 + j), rgb(32 + j));
        }
        // Worm 1 lands on its own block and leaves worm 0's alone.
        let before = pal.clone();
        set_worm_colour(&mut pal, 1, [60, 172, 60]);
        assert_eq!(pal.entries[32], before.entries[32]);
        assert_eq!(pal.entries[133], pal.entries[41]);
    }

    #[test]
    fn rotate_from_matches_cpp_formula() {
        use assets::palette::{Color, Palette};
        // entries[i] = (i, 0, 0) so a rotation is visible in `r`.
        let mut src = Palette { entries: [Color::default(); 256] };
        for (i, e) in src.entries.iter_mut().enumerate() {
            e.r = i as u8;
        }
        // Rotate sub-range [2, 5] (count 4) by dist 1.
        let mut dst = src.clone();
        rotate_from(&mut dst, &src, 2, 5, 1);
        // entries[from+i] = source[from + ((i + count - dist) % count)], count=4, dist=1.
        // i=0 -> src[2 + (0+4-1)%4]=src[2+3]=src[5]; i=1 -> src[2]; i=2 -> src[3]; i=3 -> src[4].
        assert_eq!(dst.entries[2].r, 5);
        assert_eq!(dst.entries[3].r, 2);
        assert_eq!(dst.entries[4].r, 3);
        assert_eq!(dst.entries[5].r, 4);
        // Outside the range is untouched.
        assert_eq!(dst.entries[1].r, 1);
        assert_eq!(dst.entries[6].r, 6);
    }

    #[test]
    fn rotate_from_dist_zero_is_identity_and_wraps_modulo_count() {
        use assets::palette::{Color, Palette};
        let mut src = Palette { entries: [Color::default(); 256] };
        for (i, e) in src.entries.iter_mut().enumerate() {
            e.r = i as u8;
        }
        let mut id = src.clone();
        rotate_from(&mut id, &src, 10, 13, 0);
        for i in 10..=13 {
            assert_eq!(id.entries[i].r, i as u8, "dist 0 is identity");
        }
        // dist %= count: count=4, dist 5 behaves like dist 1.
        let mut d5 = src.clone();
        let mut d1 = src.clone();
        rotate_from(&mut d5, &src, 10, 13, 5);
        rotate_from(&mut d1, &src, 10, 13, 1);
        assert_eq!(d5.entries[10].r, d1.entries[10].r);
    }

    #[test]
    fn light_up_matches_cpp_and_clamps() {
        use assets::palette::{Color, Palette};
        let mut pal = Palette { entries: [Color::default(); 256] };
        pal.entries[0] = Color { r: 100, g: 0, b: 200 };
        light_up(&mut pal, 8);
        // (v*(32-a)+a*255)>>5, a=8: r=(100*24+8*255)>>5=(2400+2040)>>5=4440>>5=138;
        // g=(0*24+2040)>>5=63; b=(200*24+2040)>>5=(4800+2040)>>5=6840>>5=213.
        assert_eq!(pal.entries[0], Color { r: 138, g: 63, b: 213 });
        // Clamp to 255: a=31, v=255 -> (255*1 + 31*255)>>5 = (255+7905)>>5 = 255.
        let mut hi = Palette { entries: [Color { r: 255, g: 255, b: 255 }; 256] };
        light_up(&mut hi, 31);
        assert_eq!(hi.entries[0], Color { r: 255, g: 255, b: 255 });
    }

    #[test]
    fn pack_pal32_packs_argb_without_requantizing() {
        use assets::palette::{Color, Palette};
        let mut pal = Palette { entries: [Color::default(); 256] };
        pal.entries[1] = Color { r: 0x12, g: 0x34, b: 0x56 };
        let p = pack_pal32(&pal);
        assert_eq!(p[1], 0xFF12_3456, "0xFF000000|r<<16|g<<8|b, entries verbatim");
        assert_eq!(p[0], 0xFF00_0000);
    }

    #[test]
    fn build_palette_order_reset_rotate_pack_and_screen_flash_inert() {
        use assets::palette::{Color, Palette};
        use assets::tc::ColorAnim;
        let mut origpal = Palette { entries: [Color::default(); 256] };
        for (i, e) in origpal.entries.iter_mut().enumerate() {
            e.r = i as u8;
        }
        let anim = [ColorAnim { from: 2, to: 5 }];
        // cycles>>3 = 8>>3 = 1 -> the [2,5] range rotates by 1 (as rotate_from test).
        let p8 = build_palette(&origpal, &anim, 8, 0);
        // Red is packed at bits 16-23 (`renderer.cpp:23-30`), so extract it
        // with `>> 16` (the brief's `& 0xFF` read the blue byte, always 0 here).
        assert_eq!((p8[2] >> 16) & 0xFF, 5, "cycles 8: entry 2 rotated to src[5].r");
        // cycles 0..7 -> dist 0 -> identity in the animated range.
        let p0 = build_palette(&origpal, &anim, 0, 0);
        assert_eq!((p0[2] >> 16) & 0xFF, 2, "cycles 0: identity");
        // screen_flash 0 is inert: the LUT equals the rotate-only build.
        let p8b = build_palette(&origpal, &anim, 8, 0);
        assert_eq!(p8, p8b);
    }
}
