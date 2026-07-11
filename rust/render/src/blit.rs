//! Sprite/line blit primitives. Ports `blit.cpp:239-729` verbatim in math,
//! idiomatic in API. Every image blit shares `CLIP_IMAGE` (`macros.hpp:3-22`);
//! index 0 is the transparent hole (`blit.cpp:252`).
//!
//! **API deviation from the brief.** The brief assumed `assets::sprite::Sprite`
//! with `.mem/.width/.height/.pitch`. The `assets` crate (slices 1-2) has no
//! per-sprite `Sprite` view — only `SpriteSet` (`sprite.rs`): fields
//! `width/height/count/data`, accessor `sprite(frame) -> &[u8]` returning the
//! `width*height` contiguous indices of one frame. A single sprite's `pitch`
//! therefore equals `width` (frames are stored back-to-back, each row-major and
//! gap-free). So the blits take `(spr: &SpriteSet, frame: usize)` and derive
//! `mem = spr.sprite(frame)`, `width/height = spr.width/height`, `pitch = width`.
//!
//! **`pal` is an explicit arg** (not stored on `Bitmap`): the C++ `scr.pal32`
//! member becomes a `pal: &Pal32` parameter, matching 3a's `set_pixel(.., pal)`
//! convention (`bitmap.rs`). All T1-T5 blits carry `pal: &Pal32`.

use crate::bitmap::{Bitmap, Pal32, Rect};
use crate::shadow_query::ShadowQuery;
use assets::sprite::SpriteSet;

/// `macros.hpp:3-22` CLIP_IMAGE: clamp a `w x h` blit at `(x,y)` to `clip`,
/// sliding the source start index. Returns `None` if fully clipped, else
/// `(x, y, w, h, src_start)`.
///
/// This is the same clamp `draw_level` (`level_draw.rs:24-46`) inlines for the
/// terrain surface; ported once here as the shared image-blit clip.
fn clip_image(
    clip: Rect,
    mut x: i32,
    mut y: i32,
    mut w: i32,
    mut h: i32,
    pitch: i32,
) -> Option<(i32, i32, i32, i32, i32)> {
    let mut src = 0i32;
    let top = y - clip.y1;
    if top < 0 {
        src += -top * pitch;
        h += top;
        y = clip.y1;
    }
    let bottom = y + h - clip.y2;
    if bottom > 0 {
        h -= bottom;
    }
    let left = x - clip.x1;
    if left < 0 {
        src -= left;
        w += left;
        x = clip.x1;
    }
    let right = x + w - clip.x2;
    if right > 0 {
        w -= right;
    }
    if w <= 0 || h <= 0 {
        None
    } else {
        Some((x, y, w, h, src))
    }
}

/// `blit.cpp:239-262` BlitImage: index-0-transparent sprite blit; writes
/// `pal[c]` for every source index `c != 0`.
pub fn blit_image(scr: &mut Bitmap, pal: &Pal32, spr: &SpriteSet, frame: usize, x: i32, y: i32) {
    let pitch = spr.width;
    let (x, y, w, h, src) = match clip_image(scr.clip, x, y, spr.width, spr.height, pitch) {
        Some(v) => v,
        None => return,
    };
    let mem = spr.sprite(frame);
    let scr_pitch = scr.pitch;
    for dy in 0..h {
        for dx in 0..w {
            let c = mem[(src + dy * pitch + dx) as usize];
            if c != 0 {
                scr.pixels[((y + dy) * scr_pitch + (x + dx)) as usize] = pal[c as usize];
            }
        }
    }
}

/// `blit.cpp:264-287` BlitImageTrans: as `blit_image`, plus a checkerboard gate.
/// The C++ `(x ^ y ^ phase) & 1` uses the *loop-local* `x`/`y` counters, which
/// restart at 0 each blit AFTER `CLIP_IMAGE` has already advanced `mem`/the
/// screen pointer — so the checker phase is relative to the CLIPPED sprite
/// origin, not the screen. `dx`/`dy` here are exactly those post-clip counters.
pub fn blit_image_trans(
    scr: &mut Bitmap,
    pal: &Pal32,
    spr: &SpriteSet,
    frame: usize,
    x: i32,
    y: i32,
    phase: i32,
) {
    let pitch = spr.width;
    let (x, y, w, h, src) = match clip_image(scr.clip, x, y, spr.width, spr.height, pitch) {
        Some(v) => v,
        None => return,
    };
    let mem = spr.sprite(frame);
    let scr_pitch = scr.pitch;
    for dy in 0..h {
        for dx in 0..w {
            let c = mem[(src + dy * pitch + dx) as usize];
            if c != 0 && ((dx ^ dy ^ phase) & 1) != 0 {
                scr.pixels[((y + dy) * scr_pitch + (x + dx)) as usize] = pal[c as usize];
            }
        }
    }
}

/// `blit.cpp:433-460` BlitShadowImage: for every source index `c != 0`, paint
/// `shadow.shadowed_argb(x+dx, y+dy)` **iff** it is non-zero. Shares the same
/// `CLIP_IMAGE` clamp as `blit_image`. Unlike `blit_image`, the value written is
/// a **raw ARGB** from the shadow query (darkened terrain), not `pal[c]` — the
/// sprite pixel is only a *stencil* deciding WHERE a shadow may land; the colour
/// comes from the level.
///
/// Because the query reads the level (not the screen), this blit is
/// **idempotent**: two overlapping shadow blits over the same cell both read the
/// same terrain material and write the same darkened value — no double-darkening
/// (`shadow_query.hpp:8-15`). The write goes straight to the (already clipped)
/// pixel, matching the C++ `*rowdest = kShadowed`.
///
/// Signature follows the T1 sprite convention (`&SpriteSet` + `frame`, source
/// `pitch == width`); the C++ takes a raw `PalIdx*` + explicit `width,height`.
pub fn blit_shadow_image(
    scr: &mut Bitmap,
    shadow: &ShadowQuery,
    spr: &SpriteSet,
    frame: usize,
    x: i32,
    y: i32,
) {
    let pitch = spr.width;
    let (x, y, w, h, src) = match clip_image(scr.clip, x, y, spr.width, spr.height, pitch) {
        Some(v) => v,
        None => return,
    };
    let mem = spr.sprite(frame);
    let scr_pitch = scr.pitch;
    for dy in 0..h {
        for dx in 0..w {
            let c = mem[(src + dy * pitch + dx) as usize];
            if c != 0 {
                let sh = shadow.shadowed_argb(x + dx, y + dy);
                if sh != 0 {
                    scr.pixels[((y + dy) * scr_pitch + (x + dx)) as usize] = sh;
                }
            }
        }
    }
}

/// `blit.cpp:344-373` BlitImageR: sprite blit restricted to the level's special
/// range (classically water, palette indices `[160,168)`). For every source
/// index `c != 0`, writes `pal[c]` **iff** the level pixel under `(x+dx, y+dy)`
/// — read via the shadow query, NOT the screen — is in the half-open range
/// `[160, 168)`. Shares the `CLIP_IMAGE` clamp; the C++ takes a raw `PalIdx*` +
/// explicit `width,height`, so `w`/`h`/`pitch` are the caller's sprite dims
/// (`viewport.cpp:423` always passes `16,16` against `large_sprites`).
#[allow(clippy::too_many_arguments)]
pub fn blit_image_r(
    scr: &mut Bitmap,
    pal: &Pal32,
    shadow: &ShadowQuery,
    spr: &SpriteSet,
    frame: usize,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) {
    let pitch = w;
    let (x, y, w, h, src) = match clip_image(scr.clip, x, y, w, h, pitch) {
        Some(v) => v,
        None => return,
    };
    let mem = spr.sprite(frame);
    let scr_pitch = scr.pitch;
    for dy in 0..h {
        for dx in 0..w {
            let c = mem[(src + dy * pitch + dx) as usize];
            if c != 0 {
                // The level, not the screen, is the material source of truth.
                let p = shadow.pixel_at(x + dx, y + dy);
                if (160..168).contains(&p) {
                    scr.pixels[((y + dy) * scr_pitch + (x + dx)) as usize] = pal[c as usize];
                }
            }
        }
    }
}

/// `blit.cpp:375-407` BlitFireCone: a fixed 16x16 blit with one of four
/// brightness stages selected by `fc` (`w.fire_cone / 2`, `viewport.cpp:539`).
/// Stages 0/1/2 gate on `c > {116,114,112}` (strictly `>`) and write
/// `pal[c - {5,3,1}]` (the offset SUBTRACTS, dimming); the `default` stage is a
/// plain index-0-transparent blit (`c != 0 -> pal[c]`). Uses `clip_image` with
/// `width == height == 16`.
pub fn blit_fire_cone(
    scr: &mut Bitmap,
    pal: &Pal32,
    fc: i32,
    spr: &SpriteSet,
    frame: usize,
    x: i32,
    y: i32,
) {
    let pitch = 16;
    let (x, y, w, h, src) = match clip_image(scr.clip, x, y, 16, 16, pitch) {
        Some(v) => v,
        None => return,
    };
    let mem = spr.sprite(frame);
    let scr_pitch = scr.pitch;
    for dy in 0..h {
        for dx in 0..w {
            let c = mem[(src + dy * pitch + dx) as usize] as i32;
            let idx = match fc {
                0 if c > 116 => Some(c - 5),
                1 if c > 114 => Some(c - 3),
                2 if c > 112 => Some(c - 1),
                fc2 if fc2 != 0 && fc2 != 1 && fc2 != 2 && c != 0 => Some(c),
                _ => None,
            };
            if let Some(i) = idx {
                scr.pixels[((y + dy) * scr_pitch + (x + dx)) as usize] = pal[i as usize];
            }
        }
    }
}

/// `blit.cpp:641-677` DO_LINE. Major-axis Bresenham with error init
/// `c = -(d>>1)`. The loop **steps BEFORE the body**, so the start pixel
/// `(from_x, from_y)` is skipped and the end pixel `(to_x, to_y)` is the LAST
/// body call — it IS drawn, because the terminating `cx != to_x` / `cy != to_y`
/// check only fires *after* the body at the endpoint has already run. A
/// degenerate `from == to` yields zero body calls (the else branch's
/// `cy != to_y` is immediately false). Calls `body(cx, cy)` for each stepped-onto
/// pixel in exact C++ order; a closure keeps the five drawers DRY without a macro.
fn do_line(from_x: i32, from_y: i32, to_x: i32, to_y: i32, mut body: impl FnMut(i32, i32)) {
    fn sign(v: i32) -> i32 {
        if v < 0 {
            -1
        } else if v > 0 {
            1
        } else {
            0
        }
    }
    let (mut cx, mut cy) = (from_x, from_y);
    let sx = sign(to_x - from_x);
    let sy = sign(to_y - from_y);
    let dx = (to_x - from_x).abs();
    let dy = (to_y - from_y).abs();
    if dx > dy {
        let mut c = -(dx >> 1);
        while cx != to_x {
            c += dy;
            cx += sx;
            if c > 0 {
                cy += sy;
                c -= dx;
            }
            body(cx, cy);
        }
    } else {
        let mut c = -(dy >> 1);
        while cy != to_y {
            c += dx;
            cy += sy;
            if c > 0 {
                cx += sx;
                c -= dy;
            }
            body(cx, cy);
        }
    }
}

/// `blit.cpp:719-729` DrawLine: plot `pal[color]` at each stepped Bresenham
/// pixel that is `Inside` the clip.
#[allow(clippy::too_many_arguments)]
pub fn draw_line(
    scr: &mut Bitmap,
    pal: &Pal32,
    from_x: i32,
    from_y: i32,
    to_x: i32,
    to_y: i32,
    color: i32,
) {
    let clip = scr.clip;
    let pitch = scr.pitch;
    do_line(from_x, from_y, to_x, to_y, |cx, cy| {
        if clip.inside(cx, cy) {
            scr.pixels[(cy * pitch + cx) as usize] = pal[color as usize];
        }
    });
}

/// `blit.cpp:679-691` DrawNinjarope: the colour cycles over the half-open range
/// `[nr_begin, nr_end)`. The C++ `if (++color == end) color = begin;`
/// **pre-increments before the plot** — port exactly: bump `color`, wrap, THEN
/// clip-gate the write of `pal[color]`. The colour advances every stepped pixel
/// regardless of clip. (`nr_begin`/`nr_end` are threaded as params instead of
/// `LC(NRColourBegin/End)`.)
#[allow(clippy::too_many_arguments)]
pub fn draw_ninjarope(
    scr: &mut Bitmap,
    pal: &Pal32,
    from_x: i32,
    from_y: i32,
    to_x: i32,
    to_y: i32,
    nr_begin: i32,
    nr_end: i32,
) {
    let clip = scr.clip;
    let pitch = scr.pitch;
    let mut color = nr_begin;
    do_line(from_x, from_y, to_x, to_y, |cx, cy| {
        color += 1;
        if color == nr_end {
            color = nr_begin;
        }
        if clip.inside(cx, cy) {
            scr.pixels[(cy * pitch + cx) as usize] = pal[color as usize];
        }
    });
}

/// `blit.cpp:693-703` DrawLaserSight — the viewport-RNG trap. Per stepped
/// Bresenham pixel it draws `rand(5)` (**always**, 1 draw); **iff that is 0**
/// AND the pixel is `Inside` the clip, it then draws `rand(2)` and writes
/// `pal[rand(2) + 83]` at `(cx, cy)`. In C++ (`blit.cpp:698-701`):
/// ```cpp
/// if (rand(5) == 0) {
///   if (clip.Inside(cx, cy)) ptr[cy * kPitch + cx] = scr.pal32[rand(2) + 83];
/// }
/// ```
/// the `rand(2)` call is on the RHS of the assignment inside the `Inside`
/// branch — it is the *controlled statement* of the `if`, so it only
/// evaluates when `Inside` is true. An off-clip zero-pixel therefore does
/// NOT advance the RNG by the `rand(2)` draw. The per-pixel draw count is 1,
/// or 2 iff the first draw was 0 AND the pixel is inside the clip. `rand` is
/// the display-only per-viewport `Rand` (never `game.rand`).
#[allow(clippy::too_many_arguments)]
pub fn draw_laser_sight(
    scr: &mut Bitmap,
    pal: &Pal32,
    rand: &mut sim_core::rng::Rand,
    from_x: i32,
    from_y: i32,
    to_x: i32,
    to_y: i32,
) {
    let clip = scr.clip;
    let pitch = scr.pitch;
    do_line(from_x, from_y, to_x, to_y, |cx, cy| {
        if rand.bound(5) == 0 {
            if clip.inside(cx, cy) {
                let idx = rand.bound(2) as i32 + 83;
                scr.pixels[(cy * pitch + cx) as usize] = pal[idx as usize];
            }
        }
    });
}

/// `blit.cpp:705-717` DrawShadowLine: for each stepped pixel `Inside` the clip,
/// paint `shadow.shadowed_argb(cx, cy)` iff it is non-zero (raw darkened-terrain
/// ARGB, like `blit_shadow_image`).
pub fn draw_shadow_line(
    scr: &mut Bitmap,
    shadow: &ShadowQuery,
    from_x: i32,
    from_y: i32,
    to_x: i32,
    to_y: i32,
) {
    let clip = scr.clip;
    let pitch = scr.pitch;
    do_line(from_x, from_y, to_x, to_y, |cx, cy| {
        if clip.inside(cx, cy) {
            let sh = shadow.shadowed_argb(cx, cy);
            if sh != 0 {
                scr.pixels[(cy * pitch + cx) as usize] = sh;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitmap::{Bitmap, ColorMode, Rect};
    use crate::shadow_query::{ShadowQuery, MAT_SEE_SHADOW};
    use sim::state::LevelSim;
    use sim_core::rng::Rand;

    const SENTINEL: u32 = 0xDEAD_BEEF;

    // pal[i] = 0xFF000000 | i (distinct per index so a pixel reveals its index).
    fn ramp_pal() -> Pal32 {
        let mut p = [0u32; 256];
        for (i, e) in p.iter_mut().enumerate() {
            *e = 0xFF00_0000 | i as u32;
        }
        p
    }

    fn sprite(width: i32, height: i32, data: Vec<u8>) -> SpriteSet {
        assert_eq!(data.len() as i32, width * height);
        SpriteSet { width, height, count: 1, data }
    }

    fn filled(w: i32, h: i32) -> Bitmap {
        let mut b = Bitmap::new(w, h);
        for p in b.pixels.iter_mut() {
            *p = SENTINEL;
        }
        b
    }

    #[test]
    fn blit_image_index0_is_transparent_and_uses_pal32() {
        // 2x2 sprite: [1, 0 / 0, 2]; index 0 must NOT write.
        let pal = ramp_pal();
        let spr = sprite(2, 2, vec![1, 0, 0, 2]);
        let mut b = filled(2, 2); // clip = full
        blit_image(&mut b, &pal, &spr, 0, 0, 0);
        assert_eq!(b.pixels[0], 0xFF00_0001, "(0,0) = pal[1]");
        assert_eq!(b.pixels[1], SENTINEL, "(1,0) index 0 transparent");
        assert_eq!(b.pixels[2], SENTINEL, "(0,1) index 0 transparent");
        assert_eq!(b.pixels[3], 0xFF00_0002, "(1,1) = pal[2]");
    }

    #[test]
    fn blit_image_clips_to_clip_rect() {
        // 2x2 all-index-1 sprite at (1,1); clip = [0,2)x[0,2). CLIP_IMAGE clamps
        // to the single inside cell (1,1); the outside cells stay sentinel
        // (clamp, not whole-sprite skip).
        let pal = ramp_pal();
        let spr = sprite(2, 2, vec![1, 1, 1, 1]);
        let mut b = filled(4, 4);
        b.clip = Rect::new(0, 0, 2, 2);
        blit_image(&mut b, &pal, &spr, 0, 1, 1);
        assert_eq!(b.pixels[1 * 4 + 1], 0xFF00_0001, "(1,1) inside clip");
        assert_eq!(b.pixels[1 * 4 + 2], SENTINEL, "(2,1) clipped out");
        assert_eq!(b.pixels[2 * 4 + 1], SENTINEL, "(1,2) clipped out");
        assert_eq!(b.pixels[2 * 4 + 2], SENTINEL, "(2,2) clipped out");
    }

    #[test]
    fn blit_image_trans_checkerboard_phase() {
        // Full (all-index-1) 2x2 sprite, clip full: (dx ^ dy ^ phase) & 1 gate.
        let pal = ramp_pal();
        let spr = sprite(2, 2, vec![1, 1, 1, 1]);

        // phase 0 writes (1,0) and (0,1); the two diagonal cells stay sentinel.
        let mut b = filled(2, 2);
        blit_image_trans(&mut b, &pal, &spr, 0, 0, 0, 0);
        assert_eq!(b.pixels[0], SENTINEL, "phase0 (0,0) checker off");
        assert_eq!(b.pixels[1], 0xFF00_0001, "phase0 (1,0) checker on");
        assert_eq!(b.pixels[2], 0xFF00_0001, "phase0 (0,1) checker on");
        assert_eq!(b.pixels[3], SENTINEL, "phase0 (1,1) checker off");

        // phase 1 inverts the checker.
        let mut b = filled(2, 2);
        blit_image_trans(&mut b, &pal, &spr, 0, 0, 0, 1);
        assert_eq!(b.pixels[0], 0xFF00_0001, "phase1 (0,0) checker on");
        assert_eq!(b.pixels[1], SENTINEL, "phase1 (1,0) checker off");
        assert_eq!(b.pixels[2], SENTINEL, "phase1 (0,1) checker off");
        assert_eq!(b.pixels[3], 0xFF00_0001, "phase1 (1,1) checker on");

        // Index 0 stays transparent even where the checker bit is set:
        // sprite [1,0 / 1,0], phase 0 sets the checker at (1,0) and (0,1).
        // (1,0) is index 0 -> must stay sentinel; (0,1) is index 1 -> writes.
        let spr2 = sprite(2, 2, vec![1, 0, 1, 0]);
        let mut b = filled(2, 2);
        blit_image_trans(&mut b, &pal, &spr2, 0, 0, 0, 0);
        assert_eq!(b.pixels[1], SENTINEL, "checker-on but index 0 -> transparent");
        assert_eq!(b.pixels[2], 0xFF00_0001, "checker-on and index 1 -> writes");
    }

    #[test]
    fn blit_image_trans_checker_uses_post_clip_counters() {
        // CRITICAL (brief): the checker phase is relative to the CLIPPED sprite
        // origin, not the screen. 3x3 all-index-1 sprite at (0,0); clip clamps
        // the LEFT edge by 1 (x -> 1, y stays 0). Post-clip counters give
        // (dx ^ dy) & 1; screen-absolute ((x+dx) ^ (y+dy)) would INVERT it
        // (x=1 shifts parity), so this distinguishes the two.
        let pal = ramp_pal();
        let spr = sprite(3, 3, vec![1; 9]);
        let mut b = filled(4, 4);
        b.clip = Rect::new(1, 0, 4, 4);
        blit_image_trans(&mut b, &pal, &spr, 0, 0, 0, 0);
        // Post-clip local checker: (0,0) off, (1,0) on, (0,1) on, ...
        assert_eq!(b.pixels[0 * 4 + 1], SENTINEL, "(1,0) local (0,0) checker off");
        assert_eq!(b.pixels[0 * 4 + 2], 0xFF00_0001, "(2,0) local (1,0) checker on");
        assert_eq!(b.pixels[1 * 4 + 1], 0xFF00_0001, "(1,1) local (0,1) checker on");
        assert_eq!(b.pixels[1 * 4 + 2], SENTINEL, "(2,1) local (1,1) checker off");
    }

    // ----- blit_shadow_image (T2) -----

    // 2x2 level. (0,0) material 20 SeeShadow; (1,0) material 30 NOT SeeShadow;
    // (0,1),(1,1) material 0 (not flagged). pal ramp so pal[m] reveals m.
    fn shadow_lvl() -> LevelSim {
        let mut material_id = vec![0u8; 4];
        material_id[0] = 20; // (0,0)
        material_id[1] = 30; // (1,0)
        let mut material_flags = [0u8; 256];
        material_flags[20] = MAT_SEE_SHADOW;
        LevelSim { width: 2, height: 2, material_id, material_flags }
    }

    fn shadow_query<'a>(lvl: &'a LevelSim, pal: &'a Pal32) -> ShadowQuery<'a> {
        ShadowQuery {
            level: lvl,
            pal32: pal,
            world_offset_x: 0,
            world_offset_y: 0,
            mode: ColorMode::Classic,
            cycles: 0,
        }
    }

    #[test]
    fn blit_shadow_image_stencils_shadow_and_respects_hole() {
        // 2x2 sprite [1,0 / 1,1]: index 0 at (1,0) is a hole.
        //   screen(0,0) sprite=1, level(0,0) SeeShadow 20 -> writes pal[24].
        //   screen(1,0) sprite=0 (hole) -> untouched even though level(1,0)...
        //                                  (which isn't SeeShadow anyway).
        //   screen(0,1) sprite=1, level(0,1) material 0 not SeeShadow -> nothing.
        //   screen(1,1) sprite=1, level(1,1) material 0 not SeeShadow -> nothing.
        let pal = ramp_pal();
        let lvl = shadow_lvl();
        let q = shadow_query(&lvl, &pal);
        let spr = sprite(2, 2, vec![1, 0, 1, 1]);
        let mut b = filled(2, 2); // clip = full
        blit_shadow_image(&mut b, &q, &spr, 0, 0, 0);
        assert_eq!(b.pixels[0], 0xFF00_0000 | 24, "(0,0) SeeShadow -> pal[20+4]");
        assert_eq!(b.pixels[1], SENTINEL, "(1,0) sprite hole -> untouched");
        assert_eq!(b.pixels[2], SENTINEL, "(0,1) not SeeShadow -> untouched");
        assert_eq!(b.pixels[3], SENTINEL, "(1,1) not SeeShadow -> untouched");
    }

    #[test]
    fn blit_shadow_image_solid_over_nonseeshadow_writes_nothing() {
        // Solid sprite pixel over the NOT-SeeShadow cell (1,0) writes nothing.
        let pal = ramp_pal();
        let lvl = shadow_lvl();
        let q = shadow_query(&lvl, &pal);
        let spr = sprite(1, 1, vec![1]);
        let mut b = filled(2, 2);
        blit_shadow_image(&mut b, &q, &spr, 0, 1, 0); // target screen (1,0)
        assert_eq!(b.pixels[1], SENTINEL, "solid over non-SeeShadow cell -> nothing");
    }

    #[test]
    fn blit_shadow_image_is_idempotent_reads_level_not_screen() {
        // Two overlapping shadow blits over the SeeShadow cell (0,0) write the
        // SAME darkened value both times — the query reads the level, so there
        // is no double-darkening (the second blit does not shadow the first's
        // output). Matches C++ BlitShadowImage reading ShadowedArgb off level.
        let pal = ramp_pal();
        let lvl = shadow_lvl();
        let q = shadow_query(&lvl, &pal);
        let spr = sprite(1, 1, vec![1]);
        let mut b = filled(2, 2);
        blit_shadow_image(&mut b, &q, &spr, 0, 0, 0);
        let after_first = b.pixels[0];
        blit_shadow_image(&mut b, &q, &spr, 0, 0, 0);
        assert_eq!(b.pixels[0], after_first, "second shadow blit is idempotent");
        assert_eq!(b.pixels[0], 0xFF00_0000 | 24, "still the single-darkened value");
    }

    #[test]
    fn blit_shadow_image_clips_like_blit_image() {
        // Sprite at (0,0), clip = [1,2)x[0,2): only screen (1,y) survives clip.
        // But (1,0) level material 30 is not SeeShadow, so still nothing writes;
        // this pins that CLIP_IMAGE is applied (no OOB, no left-column write).
        let pal = ramp_pal();
        let lvl = shadow_lvl();
        let q = shadow_query(&lvl, &pal);
        let spr = sprite(2, 2, vec![1, 1, 1, 1]);
        let mut b = filled(2, 2);
        b.clip = Rect::new(1, 0, 2, 2);
        blit_shadow_image(&mut b, &q, &spr, 0, 0, 0);
        assert_eq!(b.pixels[0], SENTINEL, "(0,0) clipped out (left of clip)");
        assert_eq!(b.pixels[2], SENTINEL, "(0,1) clipped out");
        assert_eq!(b.pixels[1], SENTINEL, "(1,0) inside clip but not SeeShadow");
        assert_eq!(b.pixels[3], SENTINEL, "(1,1) inside clip but not SeeShadow");
    }

    // ----- do_line (DO_LINE Bresenham) -----

    fn record(fx: i32, fy: i32, tx: i32, ty: i32) -> Vec<(i32, i32)> {
        let mut v = Vec::new();
        do_line(fx, fy, tx, ty, |cx, cy| v.push((cx, cy)));
        v
    }

    #[test]
    fn do_line_exact_short_lines() {
        // Hand-traced against DO_LINE (blit.cpp:644-677), c = -(d>>1), step
        // BEFORE body -> start skipped, endpoint drawn last.
        // (0,0)->(3,1): dx>dy, c0=-1.
        assert_eq!(record(0, 0, 3, 1), vec![(1, 0), (2, 1), (3, 1)]);
        // (0,0)->(1,3): dy>=dx branch, c0=-1.
        assert_eq!(record(0, 0, 1, 3), vec![(0, 1), (1, 2), (1, 3)]);
        // (3,0)->(0,2): negative-x slope, dx>dy, sx=-1.
        assert_eq!(record(3, 0, 0, 2), vec![(2, 1), (1, 1), (0, 2)]);
        // Degenerate from==to: no body calls (start skipped, loop never runs).
        assert_eq!(record(5, 5, 5, 5), Vec::<(i32, i32)>::new());
        // Pure horizontal (dy=0): every step advances x, y constant.
        assert_eq!(record(0, 2, 3, 2), vec![(1, 2), (2, 2), (3, 2)]);
        // Pure vertical (dx=0): dy>=dx branch.
        assert_eq!(record(2, 0, 2, 3), vec![(2, 1), (2, 2), (2, 3)]);
    }

    #[test]
    fn do_line_all_octants_start_skipped_end_is_terminator() {
        // One endpoint per octant (4 x-major, 4 y-major), both slope signs.
        let ends = [
            (5, 2),
            (2, 5),
            (-2, 5),
            (-5, 2),
            (-5, -2),
            (-2, -5),
            (2, -5),
            (5, -2),
        ];
        for &(tx, ty) in &ends {
            let seq = record(0, 0, tx, ty);
            assert!(!seq.is_empty(), "octant ({tx},{ty}) non-empty");
            assert_ne!(seq[0], (0, 0), "octant ({tx},{ty}) start pixel skipped");
            assert_eq!(*seq.last().unwrap(), (tx, ty), "octant ({tx},{ty}) endpoint drawn last");
            // Major axis advances by its sign EVERY step; minor axis never
            // overshoots its sign.
            let x_major = tx.abs() > ty.abs();
            let sx = tx.signum();
            let sy = ty.signum();
            let (mut px, mut py) = (0, 0);
            for &(cx, cy) in &seq {
                if x_major {
                    assert_eq!(cx - px, sx, "x-major: x steps by sx each pixel");
                    assert!((cy - py) == 0 || (cy - py) == sy, "y-major minor step in {{0,sy}}");
                } else {
                    assert_eq!(cy - py, sy, "y-major: y steps by sy each pixel");
                    assert!((cx - px) == 0 || (cx - px) == sx, "x minor step in {{0,sx}}");
                }
                px = cx;
                py = cy;
            }
        }
    }

    // ----- draw_line -----

    #[test]
    fn draw_line_plots_pal_color_inside_clip() {
        let pal = ramp_pal();
        let mut b = filled(4, 4);
        draw_line(&mut b, &pal, 0, 0, 3, 0, 7);
        // start (0,0) skipped; (1,0),(2,0),(3,0) = pal[7].
        assert_eq!(b.pixels[0], SENTINEL, "start pixel skipped");
        assert_eq!(b.pixels[1], 0xFF00_0007);
        assert_eq!(b.pixels[2], 0xFF00_0007);
        assert_eq!(b.pixels[3], 0xFF00_0007);
    }

    #[test]
    fn draw_line_clips_out_of_bounds_pixels() {
        // Line runs off the right edge of a narrow clip; off-clip pixels dropped,
        // no panic (Inside gate, blit.cpp:725).
        let pal = ramp_pal();
        let mut b = filled(4, 4);
        b.clip = Rect::new(0, 0, 2, 4); // x in [0,2)
        draw_line(&mut b, &pal, 0, 0, 3, 0, 7);
        assert_eq!(b.pixels[1], 0xFF00_0007, "(1,0) inside clip");
        assert_eq!(b.pixels[2], SENTINEL, "(2,0) clipped out");
        assert_eq!(b.pixels[3], SENTINEL, "(3,0) clipped out");
    }

    // ----- draw_ninjarope -----

    #[test]
    fn draw_ninjarope_pre_increments_and_cycles_color() {
        // Range [10,13): colours cycle 11,12,10,11,12,... The pre-increment runs
        // BEFORE the plot (blit.cpp:687), so the FIRST drawn pixel is begin+1=11.
        let pal = ramp_pal();
        let mut b = filled(8, 1);
        // 5-pixel horizontal line (0,0)->(5,0): pixels (1..=5,0).
        draw_ninjarope(&mut b, &pal, 0, 0, 5, 0, 10, 13);
        assert_eq!(b.pixels[1], 0xFF00_0000 | 11, "1st pixel: begin+1=11");
        assert_eq!(b.pixels[2], 0xFF00_0000 | 12, "2nd: 12");
        assert_eq!(b.pixels[3], 0xFF00_0000 | 10, "3rd: wraps to begin=10");
        assert_eq!(b.pixels[4], 0xFF00_0000 | 11, "4th: 11");
        assert_eq!(b.pixels[5], 0xFF00_0000 | 12, "5th: 12");
    }

    #[test]
    fn draw_ninjarope_color_advances_even_when_clipped() {
        // The colour cycles per STEPPED pixel regardless of clip (the ++color is
        // outside the Inside gate, blit.cpp:687-689). Clip out the first two
        // pixels; the third pixel must show the colour as if all three advanced.
        let pal = ramp_pal();
        let mut b = filled(8, 1);
        b.clip = Rect::new(3, 0, 8, 1); // only x>=3 visible
        draw_ninjarope(&mut b, &pal, 0, 0, 5, 0, 10, 13);
        // colours per pixel: (1,0)=11 clipped, (2,0)=12 clipped, (3,0)=10 shown.
        assert_eq!(b.pixels[1], SENTINEL, "(1,0) clipped");
        assert_eq!(b.pixels[2], SENTINEL, "(2,0) clipped");
        assert_eq!(b.pixels[3], 0xFF00_0000 | 10, "(3,0) colour advanced to 10 despite clip");
    }

    // ----- draw_laser_sight (the viewport-RNG pin) -----

    // Oracle that models blit.cpp:698-701 with an INDEPENDENT default Rand:
    // returns (writes:(cx,cy,idx), total_draws). rand(2) is the controlled
    // statement of `if (clip.Inside(cx, cy))`, so it is only drawn when
    // rand(5)==0 AND the pixel is inside the clip (off-clip zero-pixels draw
    // only the rand(5)).
    fn laser_oracle(path: &[(i32, i32)], clip: Rect) -> (Vec<(i32, i32, usize)>, u64) {
        let mut r = Rand::new();
        let mut writes = Vec::new();
        let mut draws = 0u64;
        for &(cx, cy) in path {
            let z = r.bound(5);
            draws += 1;
            if z == 0 && clip.inside(cx, cy) {
                let idx = r.bound(2) as usize + 83;
                draws += 1;
                writes.push((cx, cy, idx));
            }
        }
        (writes, draws)
    }

    #[test]
    fn draw_laser_sight_rng_order_count_and_writes() {
        let pal = ramp_pal();
        let (fx, fy, tx, ty) = (0, 0, 20, 4); // x-major, 20 stepped pixels
        let path = record(fx, fy, tx, ty);
        assert_eq!(path.len(), 20, "known Bresenham length");
        let clip = Rect::new(0, 0, 40, 40);
        let (writes, expected_draws) = laser_oracle(&path, clip);
        // The two-draw branch (rand(5)==0 -> rand(2)) must actually fire, so this
        // pins the rand(2) draw, not just the rand(5) stream.
        assert!(
            expected_draws > path.len() as u64,
            "at least one rand(5)==0 must occur so the rand(2) branch is exercised"
        );
        assert!(!writes.is_empty(), "some pixels are written");
        // Hard concrete guards against a silent RNG-impl change (default seed
        // 0x1337): exact draw count and the first written pixel/index.
        assert_eq!(expected_draws, 23, "seed 0x1337: 20 pixels + 3 zeros -> 23 draws");
        assert_eq!(writes.len(), 3, "seed 0x1337: exactly 3 sparks over this span");
        assert_eq!(writes[0], (3, 1, 84), "first laser spark pixel+index");

        // Real drawer with a fresh default Rand.
        let mut b = filled(40, 40);
        b.clip = clip;
        let mut rand = Rand::new();
        draw_laser_sight(&mut b, &pal, &mut rand, fx, fy, tx, ty);
        assert_eq!(rand.draws(), expected_draws, "draw count == N + #zeros");
        for &(cx, cy, idx) in &writes {
            assert_eq!(
                b.pixels[(cy * b.pitch + cx) as usize],
                pal[idx],
                "spark at ({cx},{cy}) = pal[{idx}] (idx in [83,85))"
            );
            assert!((83..85).contains(&idx), "index is rand(2)+83 in [83,85)");
        }
        // Non-spark path pixels stay sentinel.
        let written: std::collections::HashSet<(i32, i32)> =
            writes.iter().map(|&(x, y, _)| (x, y)).collect();
        for &(cx, cy) in &path {
            if !written.contains(&(cx, cy)) {
                assert_eq!(b.pixels[(cy * b.pitch + cx) as usize], SENTINEL, "({cx},{cy}) not a spark");
            }
        }
    }

    #[test]
    fn draw_laser_sight_rand2_gated_behind_clip_inside() {
        // THE subtlety (blit.cpp:698-701): `rand(2)` sits on the RHS of the
        // assignment that is the controlled statement of `if (clip.Inside(cx,
        // cy))` — it only evaluates when the pixel is inside the clip. So an
        // empty clip (Inside always false) must draw ONLY the rand(5) per
        // stepped pixel (20 draws for this 20-pixel span), while a full clip
        // additionally draws rand(2) for each of the 3 rand(5)==0 hits pinned
        // by seed 0x1337 in the sibling test (23 draws). If rand(2) were
        // drawn unconditionally (the old, inverted-order bug), the empty-clip
        // run would ALSO draw 23, not 20.
        let pal = ramp_pal();
        let (fx, fy, tx, ty) = (0, 0, 20, 4);
        let mut full = filled(40, 40);
        full.clip = Rect::new(0, 0, 40, 40);
        let mut ra = Rand::new();
        draw_laser_sight(&mut full, &pal, &mut ra, fx, fy, tx, ty);

        let mut empty = filled(40, 40);
        empty.clip = Rect::new(0, 0, 0, 0); // Inside always false
        let mut rb = Rand::new();
        draw_laser_sight(&mut empty, &pal, &mut rb, fx, fy, tx, ty);

        assert_eq!(rb.draws(), 20, "empty clip: only the rand(5) per pixel, no rand(2)");
        assert_eq!(ra.draws(), 23, "full clip: 20 rand(5) + 3 rand(2) (seed 0x1337)");
        assert!(empty.pixels.iter().all(|&p| p == SENTINEL), "empty clip writes nothing");
    }

    // ----- blit_image_r (water range [160,168)) -----

    // 4x1 level: material_id per cell picks water/non-water indices.
    //   (0,0)=159 (just below range), (1,0)=160 (low edge, in),
    //   (2,0)=167 (high edge, in), (3,0)=168 (just above range, out).
    fn water_lvl() -> LevelSim {
        LevelSim {
            width: 4,
            height: 1,
            material_id: vec![159, 160, 167, 168],
            material_flags: [0u8; 256],
        }
    }

    #[test]
    fn blit_image_r_draws_only_over_water_range() {
        let pal = ramp_pal();
        let lvl = water_lvl();
        let q = shadow_query_at(&lvl, &pal);
        // 4x1 solid sprite (index 5 everywhere); frame indexes the water range.
        let spr = sprite(4, 1, vec![5, 5, 5, 5]);
        let mut b = filled(4, 1);
        blit_image_r(&mut b, &pal, &q, &spr, 0, 0, 0, 4, 1);
        assert_eq!(b.pixels[0], SENTINEL, "(0,0) level 159 < 160 -> out");
        assert_eq!(b.pixels[1], 0xFF00_0005, "(1,0) level 160 in [160,168) -> pal[5]");
        assert_eq!(b.pixels[2], 0xFF00_0005, "(2,0) level 167 in range -> pal[5]");
        assert_eq!(b.pixels[3], SENTINEL, "(3,0) level 168 >= 168 -> out (half-open)");
    }

    #[test]
    fn blit_image_r_index0_transparent_over_water() {
        // Even over a water cell, a source index 0 writes nothing.
        let pal = ramp_pal();
        let lvl = water_lvl();
        let q = shadow_query_at(&lvl, &pal);
        let spr = sprite(4, 1, vec![0, 0, 0, 0]);
        let mut b = filled(4, 1);
        blit_image_r(&mut b, &pal, &q, &spr, 0, 0, 0, 4, 1);
        assert!(b.pixels.iter().all(|&p| p == SENTINEL), "index 0 never writes");
    }

    fn shadow_query_at<'a>(lvl: &'a LevelSim, pal: &'a Pal32) -> ShadowQuery<'a> {
        ShadowQuery {
            level: lvl,
            pal32: pal,
            world_offset_x: 0,
            world_offset_y: 0,
            mode: ColorMode::Classic,
            cycles: 0,
        }
    }

    // ----- blit_fire_cone (4 stages) -----

    #[test]
    fn blit_fire_cone_four_stages_thresholds_and_offsets() {
        let pal = ramp_pal();
        // 16x16 sprite: row 0 carries probe indices, rest are 0. Probes chosen to
        // straddle each stage threshold (strict >), plus a high value common to all.
        let mut data = vec![0u8; 16 * 16];
        // columns: 0->112, 1->113, 2->115, 3->117, 4->200, 5->0
        data[0] = 112;
        data[1] = 113;
        data[2] = 115;
        data[3] = 117;
        data[4] = 200;
        data[5] = 0;
        let spr = sprite(16, 16, data);

        // Stage 0: c > 116 -> pal[c-5]. Only 117 and 200 pass.
        let mut b = filled(16, 16);
        blit_fire_cone(&mut b, &pal, 0, &spr, 0, 0, 0);
        assert_eq!(b.pixels[0], SENTINEL, "112 !> 116");
        assert_eq!(b.pixels[3], 0xFF00_0000 | (117 - 5), "117 -> pal[112]");
        assert_eq!(b.pixels[4], 0xFF00_0000 | (200 - 5), "200 -> pal[195]");
        assert_eq!(b.pixels[5], SENTINEL, "index 0");

        // Stage 1: c > 114 -> pal[c-3]. 115,117,200 pass; 113,112 fail.
        let mut b = filled(16, 16);
        blit_fire_cone(&mut b, &pal, 1, &spr, 0, 0, 0);
        assert_eq!(b.pixels[1], SENTINEL, "113 !> 114");
        assert_eq!(b.pixels[2], 0xFF00_0000 | (115 - 3), "115 -> pal[112]");
        assert_eq!(b.pixels[3], 0xFF00_0000 | (117 - 3), "117 -> pal[114]");

        // Stage 2: c > 112 -> pal[c-1]. 113,115,117,200 pass; 112 fails.
        let mut b = filled(16, 16);
        blit_fire_cone(&mut b, &pal, 2, &spr, 0, 0, 0);
        assert_eq!(b.pixels[0], SENTINEL, "112 !> 112");
        assert_eq!(b.pixels[1], 0xFF00_0000 | (113 - 1), "113 -> pal[112]");

        // Default stage (fc=3): plain index-0-transparent blit, no offset.
        let mut b = filled(16, 16);
        blit_fire_cone(&mut b, &pal, 3, &spr, 0, 0, 0);
        assert_eq!(b.pixels[0], 0xFF00_0000 | 112, "default: pal[112], no offset");
        assert_eq!(b.pixels[4], 0xFF00_0000 | 200, "default: pal[200]");
        assert_eq!(b.pixels[5], SENTINEL, "default: index 0 transparent");
    }

    #[test]
    fn draw_shadow_line_paints_shadowed_argb() {
        // 4x1 level, (1,0) and (2,0) SeeShadow so the line paints darkened
        // terrain there; endpoints outside SeeShadow stay sentinel.
        let pal = ramp_pal();
        let mut material_id = vec![0u8; 4];
        material_id[1] = 20; // SeeShadow
        material_id[2] = 20;
        let mut material_flags = [0u8; 256];
        material_flags[20] = MAT_SEE_SHADOW;
        let lvl = LevelSim { width: 4, height: 1, material_id, material_flags };
        let q = shadow_query_at(&lvl, &pal);
        let mut b = filled(4, 1);
        // (0,0)->(3,0): pixels (1,0),(2,0),(3,0).
        draw_shadow_line(&mut b, &q, 0, 0, 3, 0);
        assert_eq!(b.pixels[1], 0xFF00_0000 | 24, "(1,0) SeeShadow 20 -> pal[24]");
        assert_eq!(b.pixels[2], 0xFF00_0000 | 24, "(2,0) SeeShadow 20 -> pal[24]");
        assert_eq!(b.pixels[3], SENTINEL, "(3,0) material 0 not SeeShadow -> untouched");
    }
}
