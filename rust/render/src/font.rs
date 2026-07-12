//! CPU font bank ported from C++ `Font`/`Font::Char` (`font.hpp:10-14`) plus the
//! `font.tga` post-process (`common.cpp:414-433`).
//!
//! `font.tga` is **not** a `SpriteSet`: C++ reads a raw 7-wide × `chars*8`-tall
//! index buffer (the same de-flipping [`assets::sprite::Tga::load`] performs) and
//! post-processes each 7×8 cell into a glyph whose "on" pixels are palette index
//! **8** and whose advance `width` is auto-detected by the first pixel that is
//! neither 0 nor 50 (a sentinel column). The transform is a pure asset step (no
//! sim, no RNG), but the pixel gate hashes the drawn glyphs, so the `0/50→0/8`
//! remap and the width detection must be **bit-exact** — hence `Font` lives in
//! `render` while the raw TGA read reuses the generic `assets` loader.

use assets::sprite::Tga;

/// One 7×8 glyph cell + its auto-detected advance width (`Font::Char`,
/// `font.hpp:11-14`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Char {
    /// 7 wide × 8 tall palette indices, row-major (`data[y*7 + x]`). After the
    /// post-process only 0 (transparent hole) and 8 (the single "on" index the
    /// renderer resolves via `pal32[color]`) ever appear; cells past a row's
    /// sentinel keep the zero default (matching the C++ `Char` zero-init).
    pub data: [u8; 7 * 8],
    /// Advance width in pixels (the sentinel column `x`; 0 if the glyph has none).
    pub width: i32,
}

impl Default for Char {
    fn default() -> Self {
        Char {
            data: [0u8; 7 * 8],
            width: 0,
        }
    }
}

/// The 250-glyph font bank (`Font`, `font.hpp:10-16` — `chars(250)`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Font {
    pub chars: Vec<Char>,
}

impl Font {
    /// Glyph count (`Font() : chars(250)`, `font.hpp:16`).
    pub const NUM_CHARS: usize = 250;

    /// Post-process a raw `font.tga` buffer into the glyph bank
    /// (`common.cpp:414-433`). `tga` must be the 7 × (250*8) de-flipped font
    /// buffer produced by [`Tga::load`] (same loader C++ feeds `ReadSpriteTga`).
    pub fn load(tga: &Tga) -> Font {
        // C++ `common.cpp:409-433` (font.tga post-process):
        //   std::vector<uint8_t> data(font.chars.size() * 7 * 8, 10);
        //   ReadSpriteTga(r, 7, chars*8, chars, data.data(), nullptr);   // fills `data`
        //   for (i in 0..chars) {
        //     Font::Char& ch = font.chars[i];
        //     uint8_t const* dest = &data[i * 7 * 8];
        //     ch.width = 0;
        //     for (y in 0..8) for (x in 0..7) {
        //       auto p = dest[y*7 + x];
        //       if (p == 0)       ch.data[y*7+x] = 0;
        //       else if (p == 50) ch.data[y*7+x] = 8;
        //       else            { ch.width = x; break; }   // sentinel column ends the row
        //     }
        //   }
        // `data`'s fill value (10) is irrelevant: ReadSpriteTga overwrites every
        // byte with the whole de-flipped image, which is exactly `tga.pixels`.
        let mut chars = vec![Char::default(); Font::NUM_CHARS];
        for (i, ch) in chars.iter_mut().enumerate() {
            let base = i * 7 * 8;
            ch.width = 0;
            for y in 0..8usize {
                for x in 0..7usize {
                    let p = tga.pixels[base + y * 7 + x];
                    if p == 0 {
                        ch.data[y * 7 + x] = 0;
                    } else if p == 50 {
                        ch.data[y * 7 + x] = 8;
                    } else {
                        ch.width = x as i32;
                        break;
                    }
                }
            }
        }
        Font { chars }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FONT_TGA: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/TC/openliero/sprites/font.tga"
    );

    fn real_font() -> Font {
        let bytes = std::fs::read(FONT_TGA).expect("read font.tga");
        let tga = Tga::load(&bytes).expect("font.tga parses");
        Font::load(&tga)
    }

    #[test]
    fn loads_250_chars() {
        let font = real_font();
        assert_eq!(font.chars.len(), 250);
    }

    #[test]
    fn known_glyph_k_has_width() {
        // 'K' = CP437 byte 75; DrawString does `c -= 2`, so its bank slot is 73.
        let font = real_font();
        assert!(
            font.chars[75 - 2].width > 0,
            "'K' glyph should have a nonzero advance"
        );
    }

    #[test]
    fn glyph_data_is_only_hole_or_on() {
        // Post-process maps every cell to 0 (hole) or 8 (on); nothing else.
        let font = real_font();
        for (i, ch) in font.chars.iter().enumerate() {
            for (j, &p) in ch.data.iter().enumerate() {
                assert!(p == 0 || p == 8, "char {i} cell {j} = {p}, expected 0 or 8");
            }
        }
    }
}
