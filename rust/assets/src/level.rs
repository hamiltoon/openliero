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
    /// True-color display layers from a trailing MODERNLV block, if present.
    pub display: Option<DisplayLayers>,
}

/// True-color display layers from a `.lev` MODERNLV block. Read byte-exact;
/// the per-tick colour resolve is rendering (step 3), out of scope here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayLayers {
    /// `cells` ARGB values (per-pixel phase offset when animated).
    pub data: Vec<u32>,
    /// `cells` flags: 1 = authored colour, 0 = fall back to palette.
    pub valid: Vec<u8>,
    /// Animation ramps; empty unless a valid animation block followed.
    pub ramps: Vec<ArgbRamp>,
    /// `cells` ramp indices (0 = static, N = ramp N-1); empty unless `ramps`.
    pub anim: Vec<u8>,
}

/// One animation ramp: a colour cycle advanced by `shift`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgbRamp {
    pub shift: u8,
    pub colors: Vec<u32>,
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
/// An optional trailing POWERLEVEL block carries a custom palette (C++ always
/// parses it). MODERNLV/display blocks are handled in a later slice.
///
/// Trailing-block truncation is handled more leniently than C++: where C++'s
/// `Reader::Get` throws and rejects the whole level, we degrade gracefully (a
/// truncated POWERLEVEL palette yields `palette: None`; a truncated MODERNLV
/// body yields `display: None`), keeping what parsed. These blocks are
/// non-simulation rendering data, so dropping them is charter-permitted.
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
    let display = parse_modernlv(bytes, &mut cursor, cells);

    Ok(LevelData {
        width,
        height,
        material_id,
        palette,
        display,
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

const MAX_RAMP_COLORS: usize = 4096;

// If `bytes[cursor..]` starts with "MODERNLV", parse display layers + optional
// animation, advancing `cursor`. Animation degrades gracefully: any malformed
// or short part drops ramps+anim while keeping display data (matches C++).
fn parse_modernlv(bytes: &[u8], cursor: &mut usize, cells: usize) -> Option<DisplayLayers> {
    const MAGIC: &[u8; 8] = b"MODERNLV";
    let start = *cursor;
    if bytes.len() < start + MAGIC.len() || &bytes[start..start + MAGIC.len()] != MAGIC {
        return None;
    }
    let mut pos = start + MAGIC.len();

    // display_data: cells * u32 LE
    let dd_end = pos + cells * 4;
    if bytes.len() < dd_end {
        return None; // C++ Get() would fail; treat as no MODERNLV block
    }
    let data: Vec<u32> = bytes[pos..dd_end]
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    pos = dd_end;

    // display_valid: cells * u8
    let dv_end = pos + cells;
    if bytes.len() < dv_end {
        return None;
    }
    let valid = bytes[pos..dv_end].to_vec();
    pos = dv_end;
    *cursor = pos; // display layers committed; cursor past them

    // Optional animation extension (graceful degrade on any shortfall).
    let (ramps, anim) = parse_animation(bytes, &mut pos, cells).unwrap_or_default();
    if !ramps.is_empty() {
        *cursor = pos; // animation consumed too
    }

    Some(DisplayLayers { data, valid, ramps, anim })
}

// Returns Some((ramps, anim)) only when the full, valid animation parses;
// None on any shortfall/violation (caller keeps display, drops animation).
fn parse_animation(
    bytes: &[u8],
    pos: &mut usize,
    cells: usize,
) -> Option<(Vec<ArgbRamp>, Vec<u8>)> {
    let mut p = *pos;
    let ramp_count = *bytes.get(p)?; // EOF here -> no animation
    p += 1;
    if ramp_count == 0 {
        return None;
    }

    let mut ramps = Vec::with_capacity(ramp_count as usize);
    for _ in 0..ramp_count {
        let shift = *bytes.get(p)?;
        p += 1;
        let cc_lo = *bytes.get(p)?;
        let cc_hi = *bytes.get(p + 1)?;
        p += 2;
        let color_count = u16::from_le_bytes([cc_lo, cc_hi]) as usize;
        if color_count == 0 || color_count > MAX_RAMP_COLORS {
            return None;
        }
        let end = p + color_count * 4;
        if bytes.len() < end {
            return None;
        }
        let colors: Vec<u32> = bytes[p..end]
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        p = end;
        ramps.push(ArgbRamp { shift, colors });
    }

    let anim_end = p + cells;
    if bytes.len() < anim_end {
        return None;
    }
    let anim = bytes[p..anim_end].to_vec();
    // Every index must be <= ramp_count (C++ rejects `> ramp_count`).
    if anim.iter().any(|&idx| idx > ramp_count) {
        return None;
    }
    p = anim_end;

    *pos = p;
    Some((ramps, anim))
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

    #[test]
    fn no_palette_when_trailing_block_not_powerlevel() {
        // A trailing block whose first bytes are not "POWERLEVEL" -> palette None.
        let mut buf = make_ollevel2(2, 2, |_| 1);
        buf.extend_from_slice(b"NOTAPOWER_anything");
        assert!(load(&buf).unwrap().palette.is_none());
    }

    #[test]
    fn no_palette_when_powerlevel_palette_truncated() {
        // "POWERLEVEL" magic but fewer than 768 palette bytes -> palette None.
        let mut buf = make_ollevel2(2, 2, |_| 1);
        buf.extend_from_slice(b"POWERLEVEL");
        buf.extend_from_slice(&[7u8; 100]); // < 768 palette bytes
        assert!(load(&buf).unwrap().palette.is_none());
    }

    // Build a MODERNLV block for `cells` pixels. `anim` = None -> no animation;
    // Some((ramps, anim_indices)) appends the animation extension.
    fn modernlv_block(
        cells: usize,
        anim: Option<(Vec<(u8, Vec<u32>)>, Vec<u8>)>,
    ) -> Vec<u8> {
        let mut b = b"MODERNLV".to_vec();
        for i in 0..cells {
            b.extend_from_slice(&((0x11223300u32).wrapping_add(i as u32)).to_le_bytes());
        }
        for i in 0..cells {
            b.push((i % 2) as u8); // display_valid
        }
        if let Some((ramps, anim_idx)) = anim {
            b.push(ramps.len() as u8);
            for (shift, colors) in &ramps {
                b.push(*shift);
                b.extend_from_slice(&(colors.len() as u16).to_le_bytes());
                for c in colors {
                    b.extend_from_slice(&c.to_le_bytes());
                }
            }
            for idx in &anim_idx {
                b.push(*idx);
            }
        }
        b
    }

    #[test]
    fn parses_modernlv_without_animation() {
        let mut buf = make_ollevel2(2, 2, |_| 7);
        buf.extend_from_slice(&modernlv_block(4, None));
        let d = load(&buf).unwrap().display.expect("display");
        assert_eq!(d.data.len(), 4);
        assert_eq!(d.valid, vec![0, 1, 0, 1]);
        assert_eq!(d.data[0], 0x11223300);
        assert!(d.ramps.is_empty());
        assert!(d.anim.is_empty());
    }

    #[test]
    fn parses_modernlv_with_good_animation() {
        let ramps = vec![(3u8, vec![0xAABBCCDDu32, 0x01020304])];
        let anim = vec![0u8, 1, 1, 0]; // all <= ramp_count (1)
        let mut buf = make_ollevel2(2, 2, |_| 7);
        buf.extend_from_slice(&modernlv_block(4, Some((ramps, anim.clone()))));
        let d = load(&buf).unwrap().display.unwrap();
        assert_eq!(d.ramps.len(), 1);
        assert_eq!(d.ramps[0].shift, 3);
        assert_eq!(d.ramps[0].colors, vec![0xAABBCCDD, 0x01020304]);
        assert_eq!(d.anim, anim);
    }

    #[test]
    fn modernlv_bad_ramp_index_degrades_gracefully() {
        // anim index 2 > ramp_count 1 -> C++ drops ramps+anim, keeps display.
        let ramps = vec![(3u8, vec![0xAABBCCDDu32])];
        let anim = vec![0u8, 2, 0, 0];
        let mut buf = make_ollevel2(2, 2, |_| 7);
        buf.extend_from_slice(&modernlv_block(4, Some((ramps, anim))));
        let d = load(&buf).unwrap().display.unwrap();
        assert_eq!(d.data.len(), 4); // display kept
        assert!(d.ramps.is_empty()); // animation dropped
        assert!(d.anim.is_empty());
    }

    #[test]
    fn modernlv_truncated_animation_degrades_gracefully() {
        // ramp_count=1 but stream ends before the ramp body -> drop animation.
        let mut buf = make_ollevel2(2, 2, |_| 7);
        let mut block = modernlv_block(4, None);
        block.push(1); // ramp_count = 1, then EOF
        buf.extend_from_slice(&block);
        let d = load(&buf).unwrap().display.unwrap();
        assert!(d.ramps.is_empty());
        assert!(d.anim.is_empty());
    }

    #[test]
    fn no_display_when_no_modernlv() {
        let buf = make_ollevel2(2, 2, |_| 7);
        assert!(load(&buf).unwrap().display.is_none());
    }

    #[test]
    fn powerlevel_then_modernlv_both_parsed() {
        let mut buf = make_ollevel2(2, 2, |_| 1);
        buf.extend_from_slice(&powerlevel_block());
        buf.extend_from_slice(&modernlv_block(4, None));
        let lvl = load(&buf).unwrap();
        assert!(lvl.palette.is_some());
        assert!(lvl.display.is_some());
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
