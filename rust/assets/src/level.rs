//! `.lev` level loader — material map + optional POWERLEVEL palette (display is a later slice).

use crate::palette::Palette;

/// The parsed level data the simulation needs: dimensions + the per-pixel
/// palette-index material map (row-major, `width*height` bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelData {
    pub width: i32,
    pub height: i32,
    pub material_id: Vec<u8>,
    /// Custom palette from a trailing POWERLEVEL block, if present.
    pub palette: Option<Palette>,
    /// Display layers (sprite/graphics data) — filled in Task 4 (slice 1d).
    pub display: Option<DisplayLayers>,
}

/// Placeholder for the display-layer data parsed from MODERNLV blocks.
/// Implemented in Task 4 (slice 1d); `load` currently always sets `display` to `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayLayers {}

/// Why a `.lev` failed to load. C++ returns `bool`; we use a typed error.
#[derive(Debug, PartialEq, Eq)]
pub enum LevelError {
    /// The buffer ended before the header or material bytes were complete.
    Truncated,
    /// OLLEVEL2 width/height outside the valid `1..=4096` range.
    BadDimensions(i32, i32),
}

const SIZED_MAGIC: &[u8; 8] = b"OLLEVEL2";
const LEGACY_WIDTH: i32 = 504;
const LEGACY_HEIGHT: i32 = 350;
const MAX_DIM: i32 = 4096;
const SIZED_HEADER_LEN: usize = 13; // magic(8) + version(1) + w(2) + h(2)

/// Load a `.lev` byte buffer into its material map. Mirrors the C++
/// `Level::load` format detection (`level.cpp:229`): an `OLLEVEL2` magic selects
/// the sized format; otherwise the bytes are a legacy 504×350 material map.
/// An optional trailing POWERLEVEL block carries a custom palette (C++ always
/// parses it). MODERNLV/display blocks are handled in a later slice.
pub fn load(bytes: &[u8]) -> Result<LevelData, LevelError> {
    let (width, height, mat_start) = if bytes.len() >= 8 && &bytes[0..8] == SIZED_MAGIC {
        let (w, h) = parse_sized_header(bytes)?;
        (w, h, SIZED_HEADER_LEN)
    } else {
        (LEGACY_WIDTH, LEGACY_HEIGHT, 0usize)
    };
    let cells = width as usize * height as usize;
    let mat_end = mat_start + cells;
    if bytes.len() < mat_end {
        return Err(LevelError::Truncated);
    }
    let material_id = bytes[mat_start..mat_end].to_vec();

    // Cursor now points just past the material map; optional blocks follow.
    let mut cursor = mat_end;
    let palette = parse_powerlevel(bytes, &mut cursor);

    Ok(LevelData {
        width,
        height,
        material_id,
        palette,
        display: None, // filled in Task 4
    })
}

// OLLEVEL2 header: magic(8) + version(1) + w(2 LE) + h(2 LE). Returns (w, h).
fn parse_sized_header(bytes: &[u8]) -> Result<(i32, i32), LevelError> {
    if bytes.len() < SIZED_HEADER_LEN {
        return Err(LevelError::Truncated);
    }
    let width = u16::from_le_bytes([bytes[9], bytes[10]]) as i32;
    let height = u16::from_le_bytes([bytes[11], bytes[12]]) as i32;
    if width < 1 || width > MAX_DIM || height < 1 || height > MAX_DIM {
        return Err(LevelError::BadDimensions(width, height));
    }
    Ok((width, height))
}

// If `bytes[cursor..]` starts with "POWERLEVEL", consume the 10-byte magic and
// a 768-byte VGA palette, advancing `cursor`. C++ always parses this block
// (equivalent to load_powerlevel_palette = true; see the spec).
fn parse_powerlevel(bytes: &[u8], cursor: &mut usize) -> Option<Palette> {
    const MAGIC: &[u8; 10] = b"POWERLEVEL";
    let start = *cursor;
    if bytes.len() < start + MAGIC.len() || &bytes[start..start + MAGIC.len()] != MAGIC {
        return None;
    }
    let pal_start = start + MAGIC.len();
    match Palette::load_vga(&bytes[pal_start..]) {
        Ok(pal) => {
            *cursor = pal_start + 256 * 3;
            Some(pal)
        }
        Err(_) => None, // truncated palette: leave cursor, no custom palette
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::Color;

    fn powerlevel_block() -> Vec<u8> {
        let mut b = b"POWERLEVEL".to_vec();
        // 768 VGA bytes: channel value = offset % 64.
        for i in 0..256 * 3 {
            b.push((i % 64) as u8);
        }
        b
    }

    #[test]
    fn parses_powerlevel_palette() {
        let mut buf = make_ollevel2(4, 4, |_| 1);
        buf.extend_from_slice(&powerlevel_block());
        let lvl = load(&buf).unwrap();
        let pal = lvl.palette.expect("expected custom palette");
        // VGA: (0&63)<<2, (1&63)<<2, (2&63)<<2
        assert_eq!(pal.entries[0], Color { r: 0, g: 4, b: 8 });
    }

    #[test]
    fn no_palette_when_no_powerlevel() {
        let buf = make_ollevel2(4, 4, |_| 1);
        assert!(load(&buf).unwrap().palette.is_none());
    }

    // OLLEVEL2: magic(8) + version(1) + w(2 LE) + h(2 LE) + w*h material bytes.
    fn make_ollevel2(w: i32, h: i32, fill: impl Fn(usize) -> u8) -> Vec<u8> {
        let mut b = b"OLLEVEL2".to_vec();
        b.push(0); // version
        b.extend_from_slice(&(w as u16).to_le_bytes());
        b.extend_from_slice(&(h as u16).to_le_bytes());
        for i in 0..(w as usize * h as usize) {
            b.push(fill(i));
        }
        b
    }

    #[test]
    fn loads_ollevel2_dimensions_and_materials() {
        let buf = make_ollevel2(13, 11, |i| ((i * 5 + 2) % 256) as u8);
        let lvl = load(&buf).unwrap();
        assert_eq!(lvl.width, 13);
        assert_eq!(lvl.height, 11);
        assert_eq!(lvl.material_id.len(), 13 * 11);
        assert_eq!(lvl.material_id[0], 2);
        assert_eq!(lvl.material_id[3], ((3 * 5 + 2) % 256) as u8);
    }

    #[test]
    fn loads_legacy_504x350_when_no_magic() {
        // 176400 bytes, first 8 != "OLLEVEL2" so it is legacy.
        let buf: Vec<u8> = (0..504 * 350).map(|i| (i % 251) as u8).collect();
        let lvl = load(&buf).unwrap();
        assert_eq!(lvl.width, 504);
        assert_eq!(lvl.height, 350);
        assert_eq!(lvl.material_id.len(), 504 * 350);
        assert_eq!(lvl.material_id[0], 0);
        assert_eq!(lvl.material_id[7], 7);
        assert_eq!(lvl.material_id[176399], ((176399 % 251) as u8));
    }

    #[test]
    fn rejects_truncated_sized_header() {
        let buf = b"OLLEVEL2\x00\x0d".to_vec(); // magic + version + 1 byte (header incomplete)
        assert_eq!(load(&buf), Err(LevelError::Truncated));
    }

    #[test]
    fn rejects_truncated_sized_body() {
        let mut buf = make_ollevel2(4, 4, |_| 0);
        buf.truncate(SIZED_HEADER_LEN + 5); // fewer than 16 material bytes
        assert_eq!(load(&buf), Err(LevelError::Truncated));
    }

    #[test]
    fn rejects_zero_dimensions() {
        let buf = make_ollevel2(0, 10, |_| 0); // width 0 is invalid
        assert_eq!(load(&buf), Err(LevelError::BadDimensions(0, 10)));
    }

    #[test]
    fn rejects_truncated_legacy() {
        let buf = vec![1u8; 1000]; // far fewer than 176400, no magic
        assert_eq!(load(&buf), Err(LevelError::Truncated));
    }
}
