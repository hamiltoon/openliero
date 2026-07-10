//! Sprite-bank loading from OpenLiero's TGA files. The TGA dialect and the
//! bottom-to-top pixel de-flip mirror C++ `ReadSpriteTga` (`common.cpp:242`);
//! the implementation is idiomatic Rust, not a port of the streaming reader.

use crate::palette::{Color, Palette};

/// A parsed sprite TGA: the colour map + the de-flipped pixel buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tga {
    pub width: i32,
    pub height: i32,
    /// 256-entry colour map, BGR→RGB with the low 2 bits dropped (`& 0xfc`).
    pub palette: Palette,
    /// `width * height` palette indices, top-to-bottom row-major.
    pub pixels: Vec<u8>,
}

/// A sprite bank: `count` sprites of `width × height` palette indices, stored
/// back-to-back (`sprite N` at `data[N*width*height ..]`).
///
/// `Default` is an **empty** bank (zero dimensions, no pixels): the sim unit
/// tests + slices 1-4a — which never index `large_sprites` — construct one this
/// way instead of loading a real TGA. Indexing an empty bank with `sprite()`
/// would panic, which is exactly the desired "you forgot to load the bank"
/// signal for code paths that DO dig.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SpriteSet {
    pub width: i32,
    pub height: i32,
    pub count: i32,
    pub data: Vec<u8>,
}

/// Why a sprite TGA failed to load.
#[derive(Debug, PartialEq, Eq)]
pub enum SpriteError {
    /// A header field had a value OpenLiero's loader rejects.
    BadHeader,
    /// The buffer ended before the header, colour map, or pixels were complete.
    Truncated,
    /// The TGA dimensions did not match the requested bank layout.
    DimensionMismatch { want: (i32, i32), got: (i32, i32) },
}

const HEADER_LEN: usize = 18;
const PALETTE_BYTES: usize = 256 * 3;

fn le_u16(bytes: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([bytes[o], bytes[o + 1]])
}

impl Tga {
    /// Parse an OpenLiero sprite TGA (`ReadSpriteTga`, `common.cpp:242`).
    pub fn load(bytes: &[u8]) -> Result<Tga, SpriteError> {
        if bytes.len() < HEADER_LEN {
            return Err(SpriteError::Truncated);
        }
        let id_len = bytes[0] as usize;
        // Validate every fixed header field, exactly as the C++ CHECK(...) chain.
        if bytes[1] != 1            // colour-map type
            || bytes[2] != 1        // image type (uncompressed indexed)
            || le_u16(bytes, 3) != 0    // colour-map first entry
            || le_u16(bytes, 5) != 256  // colour-map length
            || bytes[7] != 24       // colour-map entry size (bits)
            || le_u16(bytes, 8) != 0    // x origin
            || le_u16(bytes, 10) != 0   // y origin
            || bytes[16] != 8       // bits per pixel
            || bytes[17] != 0       // image descriptor
        {
            return Err(SpriteError::BadHeader);
        }
        let width = le_u16(bytes, 12) as i32;
        let height = le_u16(bytes, 14) as i32;

        let mut pos = HEADER_LEN + id_len;

        // Colour map: 256 × BGR, stored RGB with the low 2 bits dropped.
        if bytes.len() < pos + PALETTE_BYTES {
            return Err(SpriteError::Truncated);
        }
        let mut entries = [Color::default(); 256];
        for e in entries.iter_mut() {
            e.b = bytes[pos] & 0xfc;
            e.g = bytes[pos + 1] & 0xfc;
            e.r = bytes[pos + 2] & 0xfc;
            pos += 3;
        }
        let palette = Palette { entries };

        // Pixels: bottom-to-top. The first `width` bytes are the bottom row.
        let cells = width as usize * height as usize;
        if bytes.len() < pos + cells {
            return Err(SpriteError::Truncated);
        }
        let mut pixels = vec![0u8; cells];
        let w = width as usize;
        for y in (0..height as usize).rev() {
            pixels[y * w..y * w + w].copy_from_slice(&bytes[pos..pos + w]);
            pos += w;
        }

        Ok(Tga { width, height, palette, pixels })
    }
}

impl SpriteSet {
    /// View a parsed TGA as a bank of `count` sprites of `sprite_width ×
    /// sprite_height`. Requires `tga.width == sprite_width` and
    /// `tga.height == count * sprite_height`.
    pub fn from_tga(
        tga: &Tga,
        sprite_width: i32,
        sprite_height: i32,
        count: i32,
    ) -> Result<SpriteSet, SpriteError> {
        let want = (sprite_width, count * sprite_height);
        if tga.width != want.0 || tga.height != want.1 {
            return Err(SpriteError::DimensionMismatch {
                want,
                got: (tga.width, tga.height),
            });
        }
        Ok(SpriteSet {
            width: sprite_width,
            height: sprite_height,
            count,
            data: tga.pixels.clone(),
        })
    }

    /// Palette indices for sprite `frame` (`width*height` bytes).
    pub fn sprite(&self, frame: usize) -> &[u8] {
        let size = self.width as usize * self.height as usize;
        &self.data[frame * size..frame * size + size]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Build a minimal valid sprite TGA: `width × height`, id_len bytes of ID,
    // a 256×BGR colour map (channel = index%modulo), and bottom-to-top pixels
    // where the byte value encodes its file order (so the de-flip is testable).
    fn make_tga(width: i32, height: i32, id_len: u8) -> Vec<u8> {
        let mut b = vec![
            id_len, 1, 1, // id_len, cmap type, image type
            0, 0, // cmap first
            0, 1, // cmap length = 256 (LE)
            24, // cmap entry size
            0, 0, 0, 0, // x/y origin
        ];
        b.extend_from_slice(&(width as u16).to_le_bytes());
        b.extend_from_slice(&(height as u16).to_le_bytes());
        b.push(8); // bpp
        b.push(0); // descriptor
        for _ in 0..id_len {
            b.push(0xEE); // ID field (skipped)
        }
        for i in 0..256 * 3 {
            b.push((i % 64) as u8); // colour map BGR
        }
        // Pixels in FILE order (bottom row first). Encode file index in value.
        for i in 0..(width as usize * height as usize) {
            b.push(i as u8);
        }
        b
    }

    #[test]
    fn parses_and_deflips_pixels() {
        // 2 wide × 4 tall. File order rows: bottom..top. After de-flip, the
        // first file row (values 0,1) lands at the LAST in-memory row.
        let buf = make_tga(2, 4, 0);
        let tga = Tga::load(&buf).unwrap();
        assert_eq!((tga.width, tga.height), (2, 4));
        assert_eq!(tga.pixels.len(), 8);
        // file bytes: [0,1, 2,3, 4,5, 6,7] read bottom-to-top.
        // row y=3 gets file bytes 0,1; y=2 -> 2,3; y=1 -> 4,5; y=0 -> 6,7.
        assert_eq!(tga.pixels, vec![6, 7, 4, 5, 2, 3, 0, 1]);
    }

    #[test]
    fn palette_is_bgr_to_rgb_masked() {
        let tga = Tga::load(&make_tga(2, 2, 0)).unwrap();
        // entry 0: file bytes 0,1,2 = b,g,r -> r=(2&0xfc),g=(1&0xfc),b=(0&0xfc)
        assert_eq!(tga.palette.entries[0], Color { r: 0, g: 0, b: 0 });
        // entry 1: file bytes 3,4,5 -> b=3&0xfc=0, g=4&0xfc=4, r=5&0xfc=4
        assert_eq!(tga.palette.entries[1], Color { r: 4, g: 4, b: 0 });
    }

    #[test]
    fn id_field_is_skipped() {
        // Same pixels regardless of id_len.
        let a = Tga::load(&make_tga(2, 2, 0)).unwrap();
        let b = Tga::load(&make_tga(2, 2, 5)).unwrap();
        assert_eq!(a.pixels, b.pixels);
        assert_eq!(a.palette, b.palette);
    }

    #[test]
    fn from_tga_splits_into_banks() {
        // 2×4 image = 2 sprites of 2×2.
        let tga = Tga::load(&make_tga(2, 4, 0)).unwrap();
        let set = SpriteSet::from_tga(&tga, 2, 2, 2).unwrap();
        assert_eq!(set.count, 2);
        assert_eq!(set.data.len(), 8);
        assert_eq!(set.sprite(0), &[6, 7, 4, 5]); // top sprite
        assert_eq!(set.sprite(1), &[2, 3, 0, 1]); // bottom sprite
    }

    #[test]
    fn rejects_bad_header_field() {
        let mut buf = make_tga(2, 2, 0);
        buf[2] = 9; // image type 9 (RLE) not supported
        assert_eq!(Tga::load(&buf), Err(SpriteError::BadHeader));
    }

    #[test]
    fn rejects_truncated() {
        let buf = make_tga(2, 2, 0);
        assert_eq!(Tga::load(&buf[..10]), Err(SpriteError::Truncated));
        // header+palette OK but pixels cut short:
        assert_eq!(
            Tga::load(&buf[..HEADER_LEN + PALETTE_BYTES + 1]),
            Err(SpriteError::Truncated)
        );
    }

    #[test]
    fn rejects_dimension_mismatch() {
        let tga = Tga::load(&make_tga(2, 4, 0)).unwrap();
        assert_eq!(
            SpriteSet::from_tga(&tga, 7, 7, 130),
            Err(SpriteError::DimensionMismatch { want: (7, 910), got: (2, 4) })
        );
    }
}
