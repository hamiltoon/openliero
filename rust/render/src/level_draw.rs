//! Terrain draw: paints `AppearanceAt` into the surface at `(x, y)`. Port of
//! `DrawLevel` (`blit.cpp:194-213`) + the `CLIP_IMAGE` clamp (`macros.hpp:3-22`).
//! Classic arm = `pal32[material_id[idx]]` (`level.hpp:59-64`); the Modern arm is
//! deferred (overview Classic-vs-Modern).

use crate::bitmap::{Bitmap, ColorMode, Pal32};
use sim::state::LevelSim;

pub fn draw_level(dst: &mut Bitmap, lvl: &LevelSim, pal: &Pal32, x: i32, y: i32, mode: ColorMode) {
    match mode {
        ColorMode::Classic => {}
        ColorMode::Modern => unimplemented!("Modern AppearanceAt deferred past 3a"),
    }
    // CLIP_IMAGE (macros.hpp:3-22): clamp width/height/x/y and slide the source
    // index. `mem` in C++ is a pointer into material_id; we track a flat index.
    let mut x = x;
    let mut y = y;
    let mut width = lvl.width;
    let mut height = lvl.height;
    let pitch = lvl.width; // source (level) pitch
    let clip = dst.clip;
    let mut src_idx: i32 = 0;

    let top = y - clip.y1;
    if top < 0 {
        src_idx += -top * pitch;
        height += top;
        y = clip.y1;
    }
    let bottom = y + height - clip.y2;
    if bottom > 0 {
        height -= bottom;
    }
    let left = x - clip.x1;
    if left < 0 {
        src_idx -= left;
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

    let scr_pitch = dst.pitch;
    let mut scr_row = y * scr_pitch + x;
    let mut idx = src_idx;
    for _dy in 0..height {
        for dx in 0..width {
            let mat = lvl.material_id[(idx + dx) as usize];
            dst.pixels[(scr_row + dx) as usize] = pal[mat as usize];
        }
        scr_row += scr_pitch;
        idx += pitch;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitmap::{Bitmap, ColorMode, Rect};
    use sim::state::LevelSim;

    fn ramp_pal() -> [u32; 256] {
        let mut p = [0u32; 256];
        for (i, e) in p.iter_mut().enumerate() {
            *e = 0xFF00_0000 | i as u32;
        }
        p
    }
    // 2x2 level, material_id = [10, 11, 12, 13] row-major.
    fn lvl() -> LevelSim {
        LevelSim { width: 2, height: 2, material_id: vec![10, 11, 12, 13], material_flags: [0u8; 256] }
    }

    #[test]
    fn draw_level_resolves_material_through_pal32_at_offset() {
        let pal = ramp_pal();
        let mut b = Bitmap::new(4, 4); // clip = full
        draw_level(&mut b, &lvl(), &pal, 1, 1, ColorMode::Classic);
        // level (0,0)->screen (1,1); material_id[0]=10.
        assert_eq!(b.pixels[1 * 4 + 1], 0xFF00_000A);
        assert_eq!(b.pixels[1 * 4 + 2], 0xFF00_000B); // (1,0)->(2,1) mat 11
        assert_eq!(b.pixels[2 * 4 + 1], 0xFF00_000C); // (0,1)->(1,2) mat 12
        assert_eq!(b.pixels[2 * 4 + 2], 0xFF00_000D); // (1,1)->(2,2) mat 13
        // Untouched pixel stays 0.
        assert_eq!(b.pixels[0], 0);
    }

    #[test]
    fn draw_level_clips_to_clip_rect() {
        let pal = ramp_pal();
        let mut b = Bitmap::new(4, 4);
        b.clip = Rect::new(2, 2, 4, 4); // only the bottom-right quadrant
        draw_level(&mut b, &lvl(), &pal, 1, 1, ColorMode::Classic);
        // Only level (1,1)->screen (2,2) is inside the clip.
        assert_eq!(b.pixels[2 * 4 + 2], 0xFF00_000D);
        assert_eq!(b.pixels[1 * 4 + 1], 0, "clipped out");
        assert_eq!(b.pixels[1 * 4 + 2], 0, "clipped out");
    }

    #[test]
    fn draw_level_negative_offset_clamps_source() {
        let pal = ramp_pal();
        let mut b = Bitmap::new(2, 2); // clip = full 2x2
        // Offset (-1,-1): level (1,1)->screen(0,0); the rest is off-surface.
        draw_level(&mut b, &lvl(), &pal, -1, -1, ColorMode::Classic);
        assert_eq!(b.pixels[0], 0xFF00_000D, "source clamped by top/left");
        assert!(b.pixels[1..].iter().all(|&p| p == 0));
    }
}
