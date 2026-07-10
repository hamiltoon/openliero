//! Shadow/material queries resolved against the **level** (not the screen).
//! Port of `shadow_query.hpp:16-69`. `screen + world_offset = level` coords
//! (C++ builds `world_offset = -kOffs`, so screen→world is a plain add). The
//! Classic arm returns the *darkened-terrain* palette entry (`pal32[kP+4]`) for
//! a `SeeShadow` cell; the Modern display-halve arm (`shadow_query.hpp:62-66`)
//! is deferred (`ColorMode::Modern` is an unimplemented `match` arm, per the
//! Classic-only Global Constraint).
//!
//! Because the query reads the level, overlapping shadow blits are
//! **idempotent**: a second shadow over an already-shadowed pixel reads the
//! same terrain material and writes the same darkened value — there is no
//! double-darkening (see `blit::blit_shadow_image` and its idempotency test).

use crate::bitmap::{ColorMode, Pal32};
use sim::state::LevelSim;

/// `Material::kSeeShadow` (`material.hpp:11`): the flag bit marking a material
/// that shadows fall on. Queried via `LevelSim::material_flags` (the flag byte
/// per palette index), not through any `Material` struct.
pub const MAT_SEE_SHADOW: u8 = 1 << 4;

/// Queries shadow/material against the level. `pixel_at`/`shadowed_index`/
/// `shadowed_argb` mirror `shadow_query.hpp`. `mode`/`cycles` are carried for
/// the deferred Modern arm; the Classic path ignores `cycles`.
pub struct ShadowQuery<'a> {
    pub level: &'a LevelSim,
    pub pal32: &'a Pal32,
    pub world_offset_x: i32,
    pub world_offset_y: i32,
    pub mode: ColorMode,
    pub cycles: i32,
}

impl<'a> ShadowQuery<'a> {
    /// `shadow_query.hpp:27-34`: palette/material index of the level pixel under
    /// screen `(sx, sy)`, or `-1` outside the level. The bounds test ports
    /// `level.Inside` (`level.hpp:132-135`), whose `static_cast<unsigned>` trick
    /// is exactly half-open `[0,width) x [0,height)`.
    pub fn pixel_at(&self, sx: i32, sy: i32) -> i32 {
        let wx = sx + self.world_offset_x;
        let wy = sy + self.world_offset_y;
        if wx < 0 || wy < 0 || wx >= self.level.width || wy >= self.level.height {
            return -1;
        }
        self.level.material_id[(wx + wy * self.level.width) as usize] as i32
    }

    /// `shadow_query.hpp:40`: `kP >= 0 && common.materials[kP].SeeShadow()`.
    /// SeeShadow is read from `material_flags[material_id]` — here `m` is already
    /// the material id (`pixel_at`'s return), so we index the flag table directly.
    fn see_shadow(&self, m: i32) -> bool {
        m >= 0 && (self.level.material_flags[m as usize] & MAT_SEE_SHADOW) != 0
    }

    /// `shadow_query.hpp:38-46`: the level pixel shifted to its darkened palette
    /// entry (`kP + 4`), or `-1` if no shadow falls there. The `+4` can leave
    /// the 256-entry palette for materials 252-255, so it is **clamped to the
    /// unshifted index** (`kP + 4 < 256 ? kP + 4 : kP`) — NOT wrapped.
    pub fn shadowed_index(&self, sx: i32, sy: i32) -> i32 {
        let p = self.pixel_at(sx, sy);
        if !self.see_shadow(p) {
            return -1;
        }
        if p + 4 < 256 {
            p + 4
        } else {
            p
        }
    }

    /// `shadow_query.hpp:51-68`: the ARGB to paint at screen `(sx, sy)` if a
    /// shadow falls there, else 0. Classic returns `pal32[kP+4]` (clamped); the
    /// Modern display-halve arm (`:62-66`) is deferred.
    pub fn shadowed_argb(&self, sx: i32, sy: i32) -> u32 {
        let p = self.pixel_at(sx, sy);
        if !self.see_shadow(p) {
            return 0;
        }
        match self.mode {
            ColorMode::Classic => {
                self.pal32[if p + 4 < 256 { (p + 4) as usize } else { p as usize }]
            }
            // `shadow_query.hpp:62-66` display-halve arm — deferred (Classic-only
            // Global Constraint). Reached only if a caller sets `mode = Modern`,
            // which no 3b scenario does.
            ColorMode::Modern => unimplemented!("Modern display-halve shadow deferred"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // pal[i] = 0xFF000000 | i so a returned ARGB reveals which index was read.
    fn ramp_pal() -> Pal32 {
        let mut p = [0u32; 256];
        for (i, e) in p.iter_mut().enumerate() {
            *e = 0xFF00_0000 | i as u32;
        }
        p
    }

    // 4x4 level. material_id picks a few cells; the rest are material 0.
    // material_flags marks a couple of indices SeeShadow.
    //   (0,0) idx0 -> material 20 (SeeShadow)
    //   (1,0) idx1 -> material 30 (NOT SeeShadow)
    //   (2,0) idx2 -> material 254 (SeeShadow; +4 would overflow -> clamp)
    //   (3,0) idx3 -> material 0   (material 0 NOT SeeShadow here)
    fn lvl() -> LevelSim {
        let mut material_id = vec![0u8; 16];
        material_id[0] = 20;
        material_id[1] = 30;
        material_id[2] = 254;
        let mut material_flags = [0u8; 256];
        material_flags[20] = MAT_SEE_SHADOW;
        material_flags[254] = MAT_SEE_SHADOW;
        // material 30 deliberately NOT flagged; material 0 NOT flagged.
        LevelSim { width: 4, height: 4, material_id, material_flags }
    }

    fn query<'a>(level: &'a LevelSim, pal: &'a Pal32, ox: i32, oy: i32) -> ShadowQuery<'a> {
        ShadowQuery {
            level,
            pal32: pal,
            world_offset_x: ox,
            world_offset_y: oy,
            mode: ColorMode::Classic,
            cycles: 0,
        }
    }

    #[test]
    fn pixel_at_inside_and_outside() {
        let lvl = lvl();
        let pal = ramp_pal();
        // world_offset = (-1,-1): screen (1,1) -> world (0,0).
        let q = query(&lvl, &pal, -1, -1);
        assert_eq!(q.pixel_at(1, 1), 20, "screen(1,1)->world(0,0) material 20");
        assert_eq!(q.pixel_at(2, 1), 30, "screen(2,1)->world(1,0) material 30");
        // Outside the level in every direction -> -1.
        assert_eq!(q.pixel_at(0, 0), -1, "world(-1,-1) outside");
        assert_eq!(q.pixel_at(1, 0), -1, "world(0,-1) outside (y<0)");
        assert_eq!(q.pixel_at(0, 1), -1, "world(-1,0) outside (x<0)");
        assert_eq!(q.pixel_at(5, 5), -1, "world(4,4) outside (>= width/height)");
    }

    #[test]
    fn shadowed_index_plus4_and_clamp() {
        let lvl = lvl();
        let pal = ramp_pal();
        let q = query(&lvl, &pal, 0, 0); // screen == world
        // SeeShadow cell material 20 -> 24.
        assert_eq!(q.shadowed_index(0, 0), 24, "SeeShadow 20 -> 20+4");
        // NOT-SeeShadow cell material 30 -> -1.
        assert_eq!(q.shadowed_index(1, 0), -1, "not SeeShadow -> -1");
        // material 0 (not flagged) -> -1.
        assert_eq!(q.shadowed_index(3, 0), -1, "material 0 not SeeShadow -> -1");
        // material 254 SeeShadow: +4 would be 258 >= 256 -> clamp to 254 (unshifted).
        assert_eq!(q.shadowed_index(2, 0), 254, "254+4 overflows -> unshifted 254, not wrap");
        // Outside the level -> -1.
        assert_eq!(q.shadowed_index(9, 9), -1, "outside -> -1");
    }

    #[test]
    fn shadowed_argb_reads_pal_kp_plus4_or_zero() {
        let lvl = lvl();
        let pal = ramp_pal();
        let q = query(&lvl, &pal, 0, 0);
        // SeeShadow 20 -> pal32[24].
        assert_eq!(q.shadowed_argb(0, 0), 0xFF00_0000 | 24, "pal32[20+4]");
        // NOT SeeShadow -> 0.
        assert_eq!(q.shadowed_argb(1, 0), 0, "not SeeShadow -> 0 ARGB");
        // material 0 -> 0.
        assert_eq!(q.shadowed_argb(3, 0), 0, "material 0 -> 0 ARGB");
        // material 254 SeeShadow clamped -> pal32[254].
        assert_eq!(q.shadowed_argb(2, 0), 0xFF00_0000 | 254, "clamped -> pal32[254]");
        // Outside -> 0.
        assert_eq!(q.shadowed_argb(9, 9), 0, "outside -> 0 ARGB");
    }

    #[test]
    fn world_offset_is_screen_plus_offset() {
        let lvl = lvl();
        let pal = ramp_pal();
        // offset (2,3): screen (sx,sy) -> world (sx+2, sy+3).
        let q = query(&lvl, &pal, 2, 3);
        // world(0,0)=material 20 is reached from screen(-2,-3).
        assert_eq!(q.pixel_at(-2, -3), 20, "screen + offset = world");
        assert_eq!(q.shadowed_index(-2, -3), 24, "shadow query keyed at world(0,0)");
    }
}
