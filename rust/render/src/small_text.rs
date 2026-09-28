//! `Common::DrawTextSmall` (`common.cpp:227-237`): the 4×4 capital-letter font of the name
//! labels over bonuses, booby traps and a worm holding Change (`viewport.cpp:408-413`,
//! `:466-479`, `:575-581`; Step 4½e-1). The bank is `text.tga` (4×4, 26 frames, `A`..`Z`).

use crate::bitmap::{Bitmap, Pal32};
use crate::blit::blit_image;
use assets::sprite::SpriteSet;

/// `common.cpp:227-237`, verbatim: walk the bytes up to the end (or a NUL, the C string's
/// end); `c = byte - 'A'` as an unsigned char, and blit `bank[c]` (index-0 transparent,
/// `BlitImage`) at `(x, y)` only when `c < 26`; `x` always advances 4 px. Anything that is not
/// an upper-case ASCII letter — a space, a digit, a lower-case letter — only advances.
pub fn draw_text_small(bmp: &mut Bitmap, pal: &Pal32, bank: &SpriteSet, s: &[u8], x: i32, y: i32) {
    let mut x = x;
    for &b in s {
        if b == 0 {
            break; // `for (; *str; ++str)`
        }
        let c = b.wrapping_sub(b'A'); // `unsigned char const kC = *str - 'A'`
        if c < 26 {
            blit_image(bmp, pal, bank, c as usize, x, y);
        }
        x += 4;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SENTINEL: u32 = 0xDEAD_BEEF;

    fn ramp_pal() -> Pal32 {
        let mut p = [0u32; 256];
        for (i, e) in p.iter_mut().enumerate() {
            *e = 0xFF00_0000 | i as u32;
        }
        p
    }

    // A 26-frame 4×4 bank whose frame `f` is a single pixel of index `f + 1` at (0,0).
    fn marker_bank() -> SpriteSet {
        let mut data = vec![0u8; 26 * 16];
        for f in 0..26 {
            data[f * 16] = f as u8 + 1;
        }
        SpriteSet {
            width: 4,
            height: 4,
            count: 26,
            data,
        }
    }

    fn filled(w: i32, h: i32) -> Bitmap {
        let mut b = Bitmap::new(w, h);
        b.pixels.fill(SENTINEL);
        b
    }

    fn drawn(b: &Bitmap) -> Vec<(i32, u32)> {
        (0..b.w)
            .filter_map(|x| {
                let p = b.pixels[(2 * b.pitch + x) as usize];
                (p != SENTINEL).then_some((x, p))
            })
            .collect()
    }

    #[test]
    fn a_space_z_blits_frames_0_and_25_four_pixels_apart() {
        let pal = ramp_pal();
        let bank = marker_bank();
        let mut b = filled(40, 8);
        draw_text_small(&mut b, &pal, &bank, b"A Z", 3, 2);
        // 'A' -> frame 0 at x = 3; ' ' advances only; 'Z' -> frame 25 at x = 3 + 8.
        assert_eq!(drawn(&b), vec![(3, pal[1]), (11, pal[26])]);
        assert_eq!(b.pixels.iter().filter(|&&p| p != SENTINEL).count(), 2);
    }

    #[test]
    fn non_capitals_only_advance() {
        // A lower-case letter, a digit, a byte below 'A' and one >= 0x80 all fail `c < 26`.
        let pal = ramp_pal();
        let bank = marker_bank();
        let mut b = filled(60, 8);
        draw_text_small(&mut b, &pal, &bank, b"a7@\x84B", 0, 2);
        assert_eq!(
            drawn(&b),
            vec![(16, pal[2])],
            "only 'B', after four 4-px advances"
        );
    }

    #[test]
    fn a_nul_ends_the_string() {
        let pal = ramp_pal();
        let bank = marker_bank();
        let mut b = filled(40, 8);
        draw_text_small(&mut b, &pal, &bank, b"A\0B", 0, 2);
        assert_eq!(drawn(&b), vec![(0, pal[1])]);
    }

    #[test]
    fn the_real_text_bank_has_26_letters() {
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/TC/openliero/sprites/text.tga"
        ))
        .expect("read text.tga");
        let tga = assets::sprite::Tga::load(&bytes).expect("text.tga parses");
        let bank = SpriteSet::from_tga(&tga, 4, 4, 26).expect("text bank");
        let pal = ramp_pal();
        let mut b = filled(8, 8);
        draw_text_small(&mut b, &pal, &bank, b"A", 0, 0);
        assert!(
            b.pixels.iter().any(|&p| p != SENTINEL),
            "'A' draws something"
        );
    }
}
