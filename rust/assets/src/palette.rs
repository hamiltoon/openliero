//! Palette loading for `.lev` POWERLEVEL blocks and `modern.pal` files.
//! Byte transforms mirror C++ `Palette::Read`/`ReadFull`/`ExpandToFullRange`
//! (`src/game/gfx/palette.cpp`); the implementation is idiomatic Rust.

/// One palette entry. Mirrors the on-disk RGB triple; C++'s 4th `unused`
/// byte is an in-memory padding detail we do not keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// A 256-entry palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    pub entries: [Color; 256],
}

/// Why a palette failed to load.
#[derive(Debug, PartialEq, Eq)]
pub enum PaletteError {
    /// Fewer than 768 bytes (256 RGB triples) available.
    Truncated,
}

const PALETTE_BYTES: usize = 256 * 3;

impl Palette {
    /// VGA 6-bit read: `(v & 63) << 2` per channel. (C++ `Palette::Read`,
    /// `palette.cpp:61`.) Consumes the first 768 bytes of `bytes`.
    pub fn load_vga(bytes: &[u8]) -> Result<Palette, PaletteError> {
        if bytes.len() < PALETTE_BYTES {
            return Err(PaletteError::Truncated);
        }
        let mut entries = [Color::default(); 256];
        for (i, e) in entries.iter_mut().enumerate() {
            let o = i * 3;
            e.r = (bytes[o] & 63) << 2;
            e.g = (bytes[o + 1] & 63) << 2;
            e.b = (bytes[o + 2] & 63) << 2;
        }
        Ok(Palette { entries })
    }

    /// Full 8-bit read: channels verbatim. (C++ `Palette::ReadFull`,
    /// `palette.cpp:81`.) Consumes the first 768 bytes of `bytes`.
    pub fn load_full(bytes: &[u8]) -> Result<Palette, PaletteError> {
        if bytes.len() < PALETTE_BYTES {
            return Err(PaletteError::Truncated);
        }
        let mut entries = [Color::default(); 256];
        for (i, e) in entries.iter_mut().enumerate() {
            let o = i * 3;
            e.r = bytes[o];
            e.g = bytes[o + 1];
            e.b = bytes[o + 2];
        }
        Ok(Palette { entries })
    }

    /// Expand a VGA-grid palette to the full 8-bit range in place:
    /// `e |= e >> 6` per channel. (C++ `Palette::ExpandToFullRange`,
    /// `palette.cpp:106`.)
    pub fn expand_to_full_range(&mut self) {
        for e in self.entries.iter_mut() {
            e.r |= e.r >> 6;
            e.g |= e.g >> 6;
            e.b |= e.b >> 6;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 768 bytes where channel value = (index) % `modulo`.
    fn buf(modulo: usize) -> Vec<u8> {
        (0..PALETTE_BYTES).map(|i| (i % modulo) as u8).collect()
    }

    #[test]
    fn load_vga_masks_and_shifts() {
        let p = Palette::load_vga(&buf(256)).unwrap();
        // entry 0: bytes 0,1,2 -> (0&63)<<2, (1&63)<<2, (2&63)<<2
        assert_eq!(p.entries[0], Color { r: 0, g: 4, b: 8 });
        // a byte > 63 is masked: byte 200 -> (200 & 63) << 2 = (8) << 2 = 32
        // find an offset whose raw value is 200: 200 (offset 200) is channel
        // r of entry 66 (200/3 = 66, 66*3 = 198 -> r=198,g=199,b=200).
        assert_eq!(p.entries[66].b, (200u8 & 63) << 2);
    }

    #[test]
    fn load_full_keeps_channels() {
        let p = Palette::load_full(&buf(256)).unwrap();
        assert_eq!(p.entries[0], Color { r: 0, g: 1, b: 2 });
        assert_eq!(p.entries[85], Color { r: 255, g: 0, b: 1 }); // 85*3 = 255
    }

    #[test]
    fn expand_maps_vga_white_to_255() {
        // load_vga of 0x3f (63) -> (63 & 63) << 2 = 252; expand -> 252|3 = 255.
        let raw = vec![63u8; PALETTE_BYTES];
        let mut p = Palette::load_vga(&raw).unwrap();
        assert_eq!(p.entries[0], Color { r: 252, g: 252, b: 252 });
        p.expand_to_full_range();
        assert_eq!(p.entries[0], Color { r: 255, g: 255, b: 255 });
    }

    #[test]
    fn rejects_short_buffer() {
        assert_eq!(Palette::load_vga(&[0u8; 767]), Err(PaletteError::Truncated));
        assert_eq!(Palette::load_full(&[0u8; 0]), Err(PaletteError::Truncated));
    }
}
