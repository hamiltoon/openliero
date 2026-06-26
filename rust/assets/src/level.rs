//! `.lev` level loader — material map only (palette/display/sprites are later slices).

/// The parsed level data the simulation needs: dimensions + the per-pixel
/// palette-index material map (row-major, `width*height` bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelData {
    pub width: i32,
    pub height: i32,
    pub material_id: Vec<u8>,
}

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
/// Trailing POWERLEVEL/MODERNLV blocks are ignored — only the material map matters.
pub fn load(bytes: &[u8]) -> Result<LevelData, LevelError> {
    if bytes.len() >= 8 && &bytes[0..8] == SIZED_MAGIC {
        load_sized(bytes)
    } else {
        load_legacy(bytes)
    }
}

fn load_sized(bytes: &[u8]) -> Result<LevelData, LevelError> {
    if bytes.len() < SIZED_HEADER_LEN {
        return Err(LevelError::Truncated);
    }
    let width = u16::from_le_bytes([bytes[9], bytes[10]]) as i32;
    let height = u16::from_le_bytes([bytes[11], bytes[12]]) as i32;
    if width < 1 || width > MAX_DIM || height < 1 || height > MAX_DIM {
        return Err(LevelError::BadDimensions(width, height));
    }
    let cells = width as usize * height as usize;
    let end = SIZED_HEADER_LEN + cells;
    if bytes.len() < end {
        return Err(LevelError::Truncated);
    }
    Ok(LevelData {
        width,
        height,
        material_id: bytes[SIZED_HEADER_LEN..end].to_vec(),
    })
}

fn load_legacy(bytes: &[u8]) -> Result<LevelData, LevelError> {
    let cells = LEGACY_WIDTH as usize * LEGACY_HEIGHT as usize; // 176400
    if bytes.len() < cells {
        return Err(LevelError::Truncated);
    }
    Ok(LevelData {
        width: LEGACY_WIDTH,
        height: LEGACY_HEIGHT,
        material_id: bytes[0..cells].to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
