//! ARGB8888 destination surface + clip rect. Port of `bitmap.hpp:12-54`,
//! `blit.cpp:20-41`. `pitch` is in PIXELS (`bitmap.hpp:15`); addressing is
//! `y*pitch + x`. Drawing resolves an 8-bit index through `pal32` at write time.

/// Colour mode of the owning renderer. Classic resolves terrain via
/// `pal32[material_id]`; Modern (deferred, `level.hpp:220`) returns authored
/// ARGB. Kept in the API so Modern is a later `match` arm, not a refactor.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ColorMode {
    #[default]
    Classic,
    Modern,
}

/// The 256-entry ARGB lookup the renderer packs each frame (`renderer.cpp:23`).
pub type Pal32 = [u32; 256];

/// Integer rectangle. Port of `BasicRect<int>` (`math/rect.hpp:60-113`); only
/// the members 3a needs. Half-open: `inside` is `[x1,x2) x [y1,y2)`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rect {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
}

impl Rect {
    pub fn new(x1: i32, y1: i32, x2: i32, y2: i32) -> Rect {
        Rect { x1, y1, x2, y2 }
    }
    /// `rect.hpp:70`.
    pub fn width(&self) -> i32 {
        self.x2 - self.x1
    }
    /// `rect.hpp:71`.
    pub fn height(&self) -> i32 {
        self.y2 - self.y1
    }
    /// `rect.hpp:75` `Ul()`.
    pub fn ul(&self) -> (i32, i32) {
        (self.x1, self.y1)
    }
    /// `rect.hpp:77-81`: `dx>=0 && dx<width && dy>=0 && dy<height`.
    pub fn inside(&self, x: i32, y: i32) -> bool {
        let dx = x - self.x1;
        let dy = y - self.y1;
        dx >= 0 && dx < self.width() && dy >= 0 && dy < self.height()
    }
}

/// ARGB8888 surface. `bitmap.hpp:12-24`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Bitmap {
    pub w: i32,
    pub h: i32,
    /// In PIXELS, not bytes (`bitmap.hpp:15`).
    pub pitch: i32,
    pub pixels: Vec<u32>,
    pub clip: Rect,
    /// Frame counter for animated terrain (`bitmap.hpp:23`); unused by the
    /// Classic `draw_level` arm but kept for 3b/Modern parity.
    pub cycles: i32,
}

impl Bitmap {
    /// Allocate a `w x h` surface (pitch = w), zero-filled, clip = full.
    /// Mirrors `Bitmap::Alloc` (`bitmap.hpp:31-46`).
    pub fn new(w: i32, h: i32) -> Bitmap {
        Bitmap {
            w,
            h,
            pitch: w,
            pixels: vec![0u32; (w * h) as usize],
            clip: Rect::new(0, 0, w, h),
            cycles: 0,
        }
    }

    /// `bitmap.hpp:50-54`: clip-gated write of `pal32[idx]` at `y*pitch + x`.
    pub fn set_pixel(&mut self, x: i32, y: i32, idx: u8, pal: &Pal32) {
        if self.clip.inside(x, y) {
            self.pixels[(y * self.pitch + x) as usize] = pal[idx as usize];
        }
    }

    /// `blit.cpp:39-41`: repaint the WHOLE buffer through the LUT. C++ `Fill`
    /// ignores `clip_rect` (it fills `pixels .. pixels + pitch*h`); 3a relies
    /// on this to paint the HUD gap as `pal32[0]`.
    pub fn fill(&mut self, idx: u8, pal: &Pal32) {
        let argb = pal[idx as usize];
        for p in self.pixels.iter_mut() {
            *p = argb;
        }
    }

    /// `blit.cpp:20-37`: clip-CLAMPED rectangle fill (used by later slices; kept
    /// here so the clamp math is unit-tested now).
    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, idx: u8, pal: &Pal32) {
        let mut x0 = x.max(self.clip.x1);
        let mut y0 = y.max(self.clip.y1);
        let x1 = (x + w).min(self.clip.x2);
        let y1 = (y + h).min(self.clip.y2);
        if x1 > x0 {
            let argb = pal[idx as usize];
            while y0 < y1 {
                let mut cx = x0;
                while cx < x1 {
                    self.pixels[(y0 * self.pitch + cx) as usize] = argb;
                    cx += 1;
                }
                y0 += 1;
            }
            let _ = &mut x0; // silence unused-mut if refactored
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // pal[i] = 0xFF000000 | i (distinct per index so a pixel reveals its index).
    fn ramp_pal() -> Pal32 {
        let mut p = [0u32; 256];
        for (i, e) in p.iter_mut().enumerate() {
            *e = 0xFF00_0000 | i as u32;
        }
        p
    }

    #[test]
    fn set_pixel_writes_inside_and_uses_pitch() {
        let pal = ramp_pal();
        // pitch != w so a wrong `w`-vs-`pitch` index is caught.
        let mut b = Bitmap {
            w: 4,
            h: 3,
            pitch: 6,
            pixels: vec![0u32; 6 * 3],
            clip: Rect::new(0, 0, 4, 3),
            cycles: 0,
        };
        b.set_pixel(2, 1, 7, &pal);
        // index = y*pitch + x = 1*6 + 2 = 8.
        assert_eq!(b.pixels[8], 0xFF00_0007);
        assert_eq!(b.pixels[2 + 1 * 4], 0, "must NOT index by w");
    }

    #[test]
    fn set_pixel_drops_outside_clip() {
        let pal = ramp_pal();
        let mut b = Bitmap::new(4, 4);
        b.clip = Rect::new(1, 1, 3, 3);
        b.set_pixel(0, 0, 5, &pal); // outside clip
        b.set_pixel(2, 2, 5, &pal); // inside clip
        assert_eq!(b.pixels[0], 0, "outside-clip write dropped");
        assert_eq!(b.pixels[2 * 4 + 2], 0xFF00_0005, "inside-clip write kept");
    }

    #[test]
    fn fill_ignores_clip_and_paints_whole_buffer() {
        let pal = ramp_pal();
        let mut b = Bitmap::new(3, 2);
        b.clip = Rect::new(1, 1, 2, 2); // narrow clip must NOT limit fill
        b.fill(9, &pal);
        assert!(b.pixels.iter().all(|&p| p == 0xFF00_0009), "Fill is whole-buffer");
    }

    #[test]
    fn fill_rect_clamps_to_clip() {
        let pal = ramp_pal();
        let mut b = Bitmap::new(5, 5);
        b.clip = Rect::new(1, 1, 4, 4);
        b.fill_rect(0, 0, 10, 10, 3, &pal); // overspills on every side
        // Corners outside clip stay 0; a pixel inside clip is painted.
        assert_eq!(b.pixels[0], 0, "(0,0) outside clip");
        assert_eq!(b.pixels[4 * 5 + 4], 0, "(4,4) outside clip");
        assert_eq!(b.pixels[1 * 5 + 1], 0xFF00_0003, "(1,1) inside clip");
        assert_eq!(b.pixels[3 * 5 + 3], 0xFF00_0003, "(3,3) inside clip");
    }
}
