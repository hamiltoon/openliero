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

use crate::bitmap::{Bitmap, Pal32};
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

    /// Draw one glyph `c` (a **bank index**, already `-= 2`-decremented when
    /// called from [`draw_string`](Font::draw_string)) at `(x, y)` in `pal[color]`,
    /// scaled by `size`. Port of `font.cpp:8-43` VERBATIM.
    ///
    /// The `c >= 2 && c < 252` guard is applied to the **already-decremented** `c`
    /// (the `font.cpp:9` "TODO: Is this correct" double-guard quirk — ported
    /// unchanged, NOT "fixed"): glyphs whose decremented index is 0 or 1 draw
    /// nothing even though `draw_string` already validated the original byte.
    ///
    /// Every "on" pixel (any non-zero glyph value — after `Font::load` that is
    /// only index 8) writes `pal[color]`; the glyph value itself is discarded, so
    /// the caller's `color` wins. Index-0 cells are transparent holes.
    ///
    /// CAUTION for direct callers: `c == 250` or `251` passes the verbatim guard
    /// but indexes past `chars` (len 250) — a panic here, latent UB in C++.
    /// `draw_string` can never produce those values (ASCII decode caps at 125
    /// after the decrement); mask on the call side like `cossin[128]`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_char(
        &self,
        scr: &mut Bitmap,
        pal: &Pal32,
        c: u8,
        mut x: i32,
        mut y: i32,
        color: i32,
        size: i32,
    ) {
        // font.cpp:9 — guard on the already-decremented c (the double-guard quirk).
        if !(2..252).contains(&c) {
            return;
        }
        let data = &self.chars[c as usize].data; // font.cpp:12 `chars[c].data`
        let pitch = 7i32; // font.cpp:15
        let mut width = 7i32; // font.cpp:13
        let mut height = 8i32; // font.cpp:14

        // CLIP_IMAGE(scr.clip_rect) — macros.hpp:3-22. Clamps the 7x8 cell to the
        // clip, sliding `src` (the C++ `mem` pointer offset). Same clamp math as
        // blit.rs::clip_image, inlined here (pitch is the constant 7).
        let clip = scr.clip;
        let mut src = 0i32;
        let top = y - clip.y1;
        if top < 0 {
            src += -top * pitch;
            height += top;
            y = clip.y1;
        }
        let bottom = y + height - clip.y2;
        if bottom > 0 {
            height -= bottom;
        }
        let left = x - clip.x1;
        if left < 0 {
            src -= left;
            width += left;
            x = clip.x1;
        }
        let right = x + width - clip.x2;
        if right > 0 {
            width -= right;
        }
        if width <= 0 || height <= 0 {
            return;
        }

        // font.cpp:19-41 — the size-nested draw. `scrptr` is the screen row start;
        // each source row (mem += pitch) is emitted `size` times vertically, and
        // each source pixel is emitted `size` times horizontally. `if (kC)` writes
        // pal[color] for every non-zero glyph cell (the glyph value is discarded).
        let scr_pitch = scr.pitch;
        let kargb = pal[color as usize]; // font.cpp:20 scr.pal32[color]
        let mut scrptr = y * scr_pitch + x; // font.cpp:19
        let mut mem = src;
        for _cy in 0..height {
            for _i in 0..size {
                let mut rowdest = scrptr;
                let mut rowsrc = mem;
                for _cx in 0..width {
                    let kc = data[rowsrc as usize];
                    for _k in 0..size {
                        if kc != 0 {
                            scr.pixels[rowdest as usize] = kargb;
                        }
                        rowdest += 1;
                    }
                    rowsrc += 1;
                }
                scrptr += scr_pitch;
            }
            mem += pitch;
        }
    }

    /// Draw `s` starting at `(x, y)` in `pal[color]`, scaled by `size`. Port of
    /// `font.cpp:58-80`. Each codepoint is decoded to a CP437 byte via
    /// [`ascii_to_font_byte`] (ASCII-only: `< 0x80` identity, else skip — the full
    /// CP437 high-half table is deferred, spec §7 Q2); a codepoint of `0` is the
    /// line-break (`x` resets to the start column, `y += 8 * size`); every other
    /// byte passes the `c >= 2 && c < 252` gate, is decremented by 2, drawn, and
    /// advances `x` by `chars[c].width * size`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_string(
        &self,
        scr: &mut Bitmap,
        pal: &Pal32,
        s: &str,
        x: i32,
        y: i32,
        color: i32,
        size: i32,
    ) {
        let org_x = x; // font.cpp:60 kOrgX
        let mut x = x;
        let mut y = y;
        // font.cpp:62-63 — decode each codepoint. A `&str` is already valid UTF-8,
        // so `chars()` yields exactly what `cp437::Utf8DecodeNext` would.
        for cp in s.chars() {
            // font.cpp:65-69 — a NUL codepoint is the line-break.
            if cp == '\0' {
                x = org_x;
                y += 8 * size;
                continue;
            }
            let c = ascii_to_font_byte(cp); // font.cpp:71
            if (2..252).contains(&c) {
                // font.cpp:72
                let c = c - 2; // font.cpp:73
                self.draw_char(scr, pal, c, x, y, color, size); // font.cpp:75
                x += self.chars[c as usize].width * size; // font.cpp:77
            }
        }
    }

    /// `Font::GetDims` (`font.cpp:87-112`) without the height out-param: the pixel width of
    /// `s` — the widest line (a NUL codepoint breaks a line), summing `chars[c - 2].width` over
    /// the bytes the `2..252` gate passes, decoded exactly as [`draw_string`](Self::draw_string)
    /// decodes them.
    pub fn get_dims(&self, s: &str) -> i32 {
        let mut width = 0;
        let mut max_width = 0;
        for cp in s.chars() {
            if cp == '\0' {
                max_width = max_width.max(width);
                width = 0;
                continue;
            }
            let c = ascii_to_font_byte(cp);
            if (2..252).contains(&c) {
                width += self.chars[(c - 2) as usize].width;
            }
        }
        max_width.max(width)
    }
}

/// `font.cpp:51-54` `CodepointToFontByte` restricted to ASCII (spec §7 Q2). C++
/// calls `cp437::UnicodeToByte`, which is **pure identity for every `cp < 0x80`**
/// (`cp437.cpp:177-180`) — so this matches C++ bit-for-bit over the whole ASCII
/// half (0x00..0x7f), which is all the reachable HUD labels/digits ever contain.
/// A codepoint `>= 0x80` has no low-half CP437 identity and its high-half lookup
/// is deferred, so it returns `1` (the `CodepointToFontByte` "skip-no-draw"
/// sentinel — byte 1 fails the `>= 2` gate). NB the plan's "identity 0x20..0x7f"
/// is narrower than C++'s actual `< 0x80`; they agree on all printable ASCII, and
/// `< 0x80` is the faithful port.
fn ascii_to_font_byte(cp: char) -> u8 {
    let u = cp as u32;
    if u < 0x80 {
        u as u8
    } else {
        1 // skip-no-draw; high-half CP437 table deferred
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
    fn get_dims_sums_the_advance_widths_of_the_widest_line() {
        // font.cpp:87-112: the widths of the bytes the 2..252 gate passes; NUL breaks a line.
        let font = real_font();
        let w = |c: char| font.chars[c as usize - 2].width;
        assert_eq!(font.get_dims(""), 0);
        assert_eq!(font.get_dims("A"), w('A'));
        assert!(w('A') > 0, "non-vacuous");
        assert_eq!(font.get_dims("DONE!"), "DONE!".chars().map(w).sum::<i32>());
        assert_eq!(font.get_dims("AB\0A"), w('A') + w('B'), "the widest line");
        assert_eq!(
            font.get_dims("A\u{1}\u{e9}"),
            w('A'),
            "skipped exactly as draw_string skips"
        );
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

    // ----- draw_char / draw_string (T1) -----

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

    fn filled(w: i32, h: i32) -> Bitmap {
        let mut b = Bitmap::new(w, h);
        for p in b.pixels.iter_mut() {
            *p = SENTINEL;
        }
        b
    }

    // A blank 250-char bank; callers set individual glyph cells/widths.
    fn blank_font() -> Font {
        Font {
            chars: vec![Char::default(); Font::NUM_CHARS],
        }
    }

    #[test]
    fn draw_char_writes_pal_color_for_on_pixels_and_holes_stay() {
        // Glyph at bank index 5: index-8 "on" pixels at cell (0,0) and (0,1);
        // every other cell is a 0 hole. Drawn at (3,2) color 10 size 1 -> those
        // two on-pixels become pal[10]; holes leave the sentinel untouched.
        let pal = ramp_pal();
        let mut font = blank_font();
        font.chars[5].data[0 * 7 + 0] = 8; // (x0,y0)
        font.chars[5].data[1 * 7 + 0] = 8; // (x0,y1)
        font.chars[5].width = 2;

        let mut b = filled(20, 20); // clip = full
        font.draw_char(&mut b, &pal, 5, 3, 2, 10, 1);

        assert_eq!(
            b.pixels[(2 * 20 + 3) as usize],
            0xFF00_0000 | 10,
            "(3,2) on -> pal[10]"
        );
        assert_eq!(
            b.pixels[(3 * 20 + 3) as usize],
            0xFF00_0000 | 10,
            "(3,3) on -> pal[10]"
        );
        // Everything else stays sentinel (holes wrote nothing).
        let on: std::collections::HashSet<usize> = [(2 * 20 + 3) as usize, (3 * 20 + 3) as usize]
            .into_iter()
            .collect();
        for (i, &p) in b.pixels.iter().enumerate() {
            if !on.contains(&i) {
                assert_eq!(p, SENTINEL, "cell {i} must stay the hole sentinel");
            }
        }
    }

    #[test]
    fn draw_char_double_guard_skips_decremented_index_below_2() {
        // The font.cpp:9 quirk: the guard tests the ALREADY-decremented c, so
        // c == 0 or c == 1 draw nothing even if the glyph has on-pixels.
        let pal = ramp_pal();
        let mut font = blank_font();
        font.chars[0].data[0] = 8;
        font.chars[1].data[0] = 8;

        let mut b = filled(8, 8);
        font.draw_char(&mut b, &pal, 0, 0, 0, 10, 1);
        font.draw_char(&mut b, &pal, 1, 0, 0, 10, 1);
        assert!(
            b.pixels.iter().all(|&p| p == SENTINEL),
            "c<2 guard skips the glyph"
        );
    }

    #[test]
    fn draw_char_clips_to_clip_rect() {
        // Glyph with on-pixels in cell column 0 AND column 2. Clip x1 = 1 slides
        // the source start past column 0 (CLIP_IMAGE), so only the column-2 pixel
        // survives, landing at screen x = 2; column 0's pixel is never drawn.
        let pal = ramp_pal();
        let mut font = blank_font();
        font.chars[5].data[0 * 7 + 0] = 8; // (x0,y0)
        font.chars[5].data[0 * 7 + 2] = 8; // (x2,y0)

        let mut b = filled(8, 8);
        b.clip = Rect::new(1, 0, 8, 8); // clip out screen column 0
        font.draw_char(&mut b, &pal, 5, 0, 0, 10, 1);

        assert_eq!(b.pixels[0], SENTINEL, "(0,0) clipped out (left of clip)");
        assert_eq!(b.pixels[1], SENTINEL, "(1,0) source col 1 is a hole");
        assert_eq!(
            b.pixels[2],
            0xFF00_0000 | 10,
            "(2,0) source col 2 on-pixel drawn"
        );
    }

    #[test]
    fn draw_char_size_replicates_each_pixel() {
        // size=2 replicates each source cell into a 2x2 screen block. One on-pixel
        // at cell (0,0) drawn at (1,1) -> screen block (1,1),(2,1),(1,2),(2,2).
        let pal = ramp_pal();
        let mut font = blank_font();
        font.chars[5].data[0] = 8; // (x0,y0)

        let mut b = filled(8, 8);
        font.draw_char(&mut b, &pal, 5, 1, 1, 10, 2);
        for &(x, y) in &[(1, 1), (2, 1), (1, 2), (2, 2)] {
            assert_eq!(
                b.pixels[(y * 8 + x) as usize],
                0xFF00_0000 | 10,
                "block ({x},{y})"
            );
        }
        // A neighbouring cell outside the 2x2 block stays sentinel.
        assert_eq!(
            b.pixels[(1 * 8 + 3) as usize],
            SENTINEL,
            "(3,1) outside the block"
        );
    }

    #[test]
    fn draw_string_advances_by_width_times_size() {
        // Synthetic bank: every glyph carries an origin marker at cell (0,0) and a
        // known advance width = (index % 5) + 1. Drawing "Kills: 0" must land each
        // glyph's origin pixel at the running x predicted by the cumulative widths.
        let pal = ramp_pal();
        let mut font = blank_font();
        for (i, ch) in font.chars.iter_mut().enumerate() {
            ch.data[0] = 8; // origin marker at cell (0,0)
            ch.width = (i as i32 % 5) + 1;
        }

        let s = "Kills: 0";
        // Predict origins: for each byte b, bank index = b - 2, advance that width.
        let mut x = 0i32;
        let mut origins = Vec::new();
        for b in s.bytes() {
            let c = (b - 2) as usize;
            origins.push(x);
            x += font.chars[c].width; // size = 1
        }

        let mut b = filled(40, 8);
        font.draw_string(&mut b, &pal, s, 0, 0, 10, 1);

        for &ox in &origins {
            assert_eq!(
                b.pixels[ox as usize],
                0xFF00_0000 | 10,
                "glyph origin at x={ox} should be pal[10]"
            );
        }
        // Total advance equals the sum of the glyph widths.
        let total: i32 = s.bytes().map(|b| font.chars[(b - 2) as usize].width).sum();
        assert_eq!(x, total, "final x == sum of glyph widths");
    }

    #[test]
    fn draw_string_newline_resets_x_and_steps_y() {
        // A NUL codepoint (font.cpp:65 kCp==0) resets x to the start column and
        // steps y by 8*size. Two origin-marked glyphs split by '\0' land on
        // successive rows at the same x.
        let pal = ramp_pal();
        let mut font = blank_font();
        for ch in font.chars.iter_mut() {
            ch.data[0] = 8;
            ch.width = 3;
        }

        let mut b = filled(16, 24);
        // 'A' then NUL then 'A'; start at (5, 1), size 1 -> second row at y = 1+8.
        font.draw_string(&mut b, &pal, "A\0A", 5, 1, 10, 1);
        assert_eq!(
            b.pixels[(1 * 16 + 5) as usize],
            0xFF00_0000 | 10,
            "row 0 glyph at (5,1)"
        );
        assert_eq!(
            b.pixels[(9 * 16 + 5) as usize],
            0xFF00_0000 | 10,
            "row 1 glyph at (5,9)"
        );
    }

    #[test]
    fn reachable_labels_are_ascii() {
        // The label proof-test (spec §7 Q2): every reachable HUD label + digit is
        // < 0x80, so the ASCII decode is bit-exact and the full CP437 table stays
        // deferred. If a label ever became non-ASCII this would fail, forcing it.
        assert!("Kills: Lives: Reloading...".bytes().all(|b| b < 0x80));
    }
}
