//! Terrain destruction: the `DrawDirtEffect` port (C++ `gfx/blit.cpp:534-622`).
//!
//! `DrawDirtEffect` stamps a 16x16 "crater" into the level by walking a *mask*
//! sprite (whose cell values select an action) and, for the "fill" cases,
//! copying texels from a *fill* sprite tiled in **level** coordinates. It is the
//! function that mutates `material_id`, so it is what turns the level hash into a
//! time series.
//!
//! Faithful port notes:
//!
//! * **RNG is consumed FIRST** — the fill frame is `s_frame + rand(r_frame)`,
//!   drawn before any pixel is touched (`blit.cpp:537`). Determinism depends on
//!   this ordering relative to the rest of the frame.
//! * The 16x16 window is clipped with the `CLIP_IMAGE` macro (`gfx/macros.hpp`)
//!   against `Rect(0, 0, width, height - 1)` — note the **`height - 1`**, which
//!   excludes the level's bottom row (`blit.cpp:545`). The clamp adjusts the
//!   start `(x, y)`, the extent `(width, height)`, and the offset into the mask
//!   sprite (`mem`) so exactly the same cells are visited as in C++.
//! * The destination/mask walk reproduces the `BLITL` macro (`blit.cpp:323`):
//!   row-major, `mem` advances by `pitch` (16) per row, mask cell is
//!   `mask[mem + y_*16 + x_]`, dest level index is `(y+y_)*width + (x+x_)`.
//! * The fill wrap uses **level** coords: `fill[((my & 15) << 4) + (mx & 15)]`
//!   with `mx = x + x_, my = y + y_` (`blit.cpp:559/593`).
//! * Writes touch `material_id` only (via [`LevelSim::set_material`]). The C++
//!   also updates a derived `materials` flag cache, a `display_valid` byte, and a
//!   dirty-rect — all render/derived state the Rust port omits and the hash never
//!   reads. The `Background()/AnyDirt()/Dirt()/Dirt2()` guards become the Task-0
//!   flag predicates reading `material_flags[material_id[idx]]` live; this is
//!   equivalent because the double loop visits each level cell **at most once per
//!   call**, so there is never a within-call re-read of a just-written cell.

use assets::sprite::SpriteSet;
use assets::tc::Texture;
use sim_core::rng::Rand;

use crate::state::LevelSim;

/// Port of `DrawDirtEffect` (`gfx/blit.cpp:534-622`). Stamps the 16x16 dirt
/// effect `dirt_effect` (an index into `textures`) at level coords `(x, y)`,
/// writing `material_id` via [`LevelSim::set_material`]. Consumes exactly one
/// `rand(r_frame)` draw, **before** any pixel write.
pub fn draw_dirt_effect(
    level: &mut LevelSim,
    large_sprites: &SpriteSet,
    textures: &[Texture],
    dirt_effect: i32,
    x: i32,
    y: i32,
    rand: &mut Rand,
) {
    debug_assert!(dirt_effect >= 0 && (dirt_effect as usize) < textures.len());
    let tex = &textures[dirt_effect as usize];

    // RNG FIRST (blit.cpp:537): pick the fill frame before touching any pixel.
    let fill = large_sprites.sprite((tex.sframe + rand.bound(tex.rframe as u32) as i32) as usize);
    let mask = large_sprites.sprite(tex.mframe as usize);

    // CLIP_IMAGE(Rect(0, 0, width, height - 1)) — gfx/macros.hpp + blit.cpp:545.
    // `mem` is the byte offset into `mask` (C++ advances the `mem` pointer).
    let pitch: i32 = 16;
    let mut w: i32 = 16;
    let mut h: i32 = 16;
    let mut mem: i32 = 0;
    let mut x = x;
    let mut y = y;
    let (cx1, cy1, cx2, cy2) = (0i32, 0i32, level.width, level.height - 1);

    let top = y - cy1;
    if top < 0 {
        mem += -top * pitch;
        h += top;
        y = cy1;
    }
    let bottom = y + h - cy2;
    if bottom > 0 {
        h -= bottom;
    }
    let left = x - cx1;
    if left < 0 {
        mem -= left;
        w += left;
        x = cx1;
    }
    let right = x + w - cx2;
    if right > 0 {
        w -= right;
    }
    if w <= 0 || h <= 0 {
        return;
    }

    // BLITL walk (blit.cpp:323): row-major over the clipped window; the mask cell
    // is `mask[mem + y_*pitch + x_]`, the dest level index is `my*width + mx`.
    let level_width = level.width;
    if tex.ndrawback {
        // Carving over Dirt/Dirt2 (blit.cpp:551-583): dig path (4c/4d).
        for y_ in 0..h {
            for x_ in 0..w {
                let c = mask[(mem + y_ * pitch + x_) as usize];
                let mx = x + x_;
                let my = y + y_;
                let idx = (my * level_width + mx) as usize;
                match c {
                    6 => {
                        if level.any_dirt(mx, my) {
                            let texel = fill[(((my & 15) << 4) + (mx & 15)) as usize];
                            level.set_material(idx, texel);
                        }
                    }
                    1 => {
                        if level.dirt2(mx, my) {
                            level.set_material(idx, 2);
                        } else if level.dirt(mx, my) {
                            level.set_material(idx, 1);
                        }
                    }
                    _ => {}
                }
            }
        }
    } else {
        // Additive over Background (blit.cpp:584-621): greenball path.
        for y_ in 0..h {
            for x_ in 0..w {
                let c = mask[(mem + y_ * pitch + x_) as usize];
                let mx = x + x_;
                let my = y + y_;
                let idx = (my * level_width + mx) as usize;
                match c {
                    10 | 6 => {
                        if level.background(mx, my) {
                            let texel = fill[(((my & 15) << 4) + (mx & 15)) as usize];
                            level.set_material(idx, texel);
                        }
                    }
                    2 => {
                        if level.background(mx, my) {
                            level.set_material(idx, 2);
                        }
                    }
                    1 => {
                        if level.background(mx, my) {
                            level.set_material(idx, 1);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{MAT_BACKGROUND, MAT_DIRT, MAT_DIRT2, MAT_ROCK};

    const SIZE: usize = 256; // 16 x 16

    // Build a SpriteSet of `count` 16x16 sprites, applying each (index, bytes)
    // override on top of an all-zero bank.
    fn make_sprites(count: i32, overrides: &[(usize, Vec<u8>)]) -> SpriteSet {
        let mut data = vec![0u8; count as usize * SIZE];
        for (idx, bytes) in overrides {
            assert_eq!(bytes.len(), SIZE);
            data[idx * SIZE..idx * SIZE + SIZE].copy_from_slice(bytes);
        }
        SpriteSet { width: 16, height: 16, count, data }
    }

    // A mask sprite (256 bytes) with the given (offset, value) cells set; the
    // offset is row-major `y_*16 + x_`. Everything else is 0 ("other" / no-op).
    fn mask_cells(cells: &[(usize, u8)]) -> Vec<u8> {
        let mut m = vec![0u8; SIZE];
        for (off, v) in cells {
            m[*off] = *v;
        }
        m
    }

    // A fill sprite whose every texel equals its own index + base (so a written
    // value reveals which wrap index was sampled).
    fn fill_indexed(base: u8) -> Vec<u8> {
        (0..SIZE).map(|i| base.wrapping_add(i as u8)).collect()
    }

    // A fill sprite of a single constant value.
    fn fill_const(v: u8) -> Vec<u8> {
        vec![v; SIZE]
    }

    // An all-background level: material 0 carries the background flag and every
    // pixel is material 0.
    fn bg_level(width: i32, height: i32) -> LevelSim {
        let mut material_flags = [0u8; 256];
        material_flags[0] = MAT_BACKGROUND;
        LevelSim {
            width,
            height,
            material_id: vec![0u8; (width * height) as usize],
            material_flags,
        }
    }

    fn seeded(seed: u32) -> Rand {
        let mut r = Rand::new();
        r.seed(seed);
        r
    }

    // The shipped greenball texture frames (tc.cfg greenball / brief).
    fn greenball() -> Texture {
        Texture { sframe: 82, rframe: 2, mframe: 38, ndrawback: false }
    }

    // ---- Step 1: RNG drawn first + fill/mask selection ----------------------
    #[test]
    fn rng_consumed_first_and_selects_fill_frame() {
        // Greenball: rframe=2, so one rand(2) picks fill in {82,83}. Sprite 82 is
        // all 200, sprite 83 all 201; a single case-6 mask cell at (0,0) over an
        // all-background level writes fill[wrap=0] -> reveals which frame was used.
        let sprites = make_sprites(
            84,
            &[
                (38, mask_cells(&[(0, 6)])),
                (82, fill_const(200)),
                (83, fill_const(201)),
            ],
        );
        let textures = [greenball()];
        let mut level = bg_level(16, 17); // height-1 = 16 covers the full window

        const SEED: u32 = 0x9e37_79b9;
        // Oracle: an identically seeded Rand tells us the expected first draw.
        let mut oracle = seeded(SEED);
        let draw = oracle.bound(2);
        let expected_last = oracle.last();

        let mut rand = seeded(SEED);
        draw_dirt_effect(&mut level, &sprites, &textures, 0, 0, 0, &mut rand);

        // Exactly one rand(2) was consumed, and before the write (the written
        // texel is the fill frame the first draw selected).
        assert_eq!(rand.last(), expected_last, "exactly one rand(2) consumed");
        assert_eq!(
            level.material_id[0],
            200u8.wrapping_add(draw as u8),
            "fill frame = 82 + rand(2); RNG drawn before the pixel write"
        );
    }

    // ---- Step 2: ndrawback=false cases over Background ----------------------
    #[test]
    fn ndrawback_false_cases_over_background() {
        // rframe=1 => rand(1)==0 deterministically => fill frame = sframe (82).
        let tex = Texture { sframe: 82, rframe: 1, mframe: 38, ndrawback: false };
        let mask = mask_cells(&[
            (0, 6),   // (0,0): fill
            (1, 10),  // (1,0): fill (case 10 aliases 6)
            (2, 2),   // (2,0): -> material 2
            (3, 1),   // (3,0): -> material 1
            (4, 99),  // (4,0): other -> unchanged
            (5, 6),   // (5,0): case 6 but level cell is NON-background -> untouched
        ]);
        // fill[i] = 100 + i, so case-6/10 writes are distinguishable from the 0
        // background and from the literal 1/2 writes.
        let sprites = make_sprites(83, &[(38, mask), (82, fill_indexed(100))]);

        let mut level = bg_level(16, 17);
        // Cell (5,0): make it rock (no background flag) so the guard skips it.
        level.material_flags[3] = MAT_ROCK;
        level.material_id[5] = 3;

        let mut rand = seeded(1);
        draw_dirt_effect(&mut level, &sprites, &tex_slice(&tex), 0, 0, 0, &mut rand);

        assert_eq!(level.material_id[0], 100, "case 6 -> fill[wrap 0]");
        assert_eq!(level.material_id[1], 101, "case 10 -> fill[wrap 1]");
        assert_eq!(level.material_id[2], 2, "case 2 -> material 2");
        assert_eq!(level.material_id[3], 1, "case 1 -> material 1");
        assert_eq!(level.material_id[4], 0, "other -> unchanged");
        assert_eq!(level.material_id[5], 3, "non-Background cell left untouched");
    }

    // ---- Step 3: ndrawback=true carving ------------------------------------
    #[test]
    fn ndrawback_true_carving_over_dirt() {
        let tex = Texture { sframe: 82, rframe: 1, mframe: 38, ndrawback: true };
        let mask = mask_cells(&[
            (0, 6), // AnyDirt -> fill
            (1, 6), // background (not AnyDirt) -> unchanged
            (2, 1), // dirt2 -> 2
            (3, 1), // dirt -> 1
            (4, 1), // neither -> unchanged
        ]);
        let sprites = make_sprites(83, &[(38, mask), (82, fill_indexed(100))]);

        let mut level = bg_level(16, 17);
        // material ids: 10 = dirt, 11 = dirt2.
        level.material_flags[10] = MAT_DIRT;
        level.material_flags[11] = MAT_DIRT2;
        level.material_id[0] = 10; // dirt -> AnyDirt
        // cell 1 stays background (0)
        level.material_id[2] = 11; // dirt2
        level.material_id[3] = 10; // dirt
        // cell 4 stays background (0) -> neither dirt

        let mut rand = seeded(1);
        draw_dirt_effect(&mut level, &sprites, &tex_slice(&tex), 0, 0, 0, &mut rand);

        assert_eq!(level.material_id[0], 100, "case 6 AnyDirt -> fill[wrap 0]");
        assert_eq!(level.material_id[1], 0, "case 6 over background -> unchanged");
        assert_eq!(level.material_id[2], 2, "case 1 dirt2 -> 2");
        assert_eq!(level.material_id[3], 1, "case 1 dirt -> 1");
        assert_eq!(level.material_id[4], 0, "case 1 neither -> unchanged");
    }

    // ---- Step 4: texture wrap uses LEVEL coords ----------------------------
    #[test]
    fn texture_wrap_uses_level_coords_not_window_offsets() {
        // Window at (x=5, y=3); a single case-6 mask cell at window offset
        // (x_=2, y_=1). Level coords: mx=7, my=4. wrap = (4<<4)+7 = 71.
        // fill[i] = i, so the written value is 71 (NOT (1<<4)+2 = 18).
        let tex = Texture { sframe: 82, rframe: 1, mframe: 38, ndrawback: false };
        let mask = mask_cells(&[(1 * 16 + 2, 6)]); // y_=1, x_=2
        let sprites = make_sprites(83, &[(38, mask), (82, fill_indexed(0))]);

        let width = 20;
        let mut level = bg_level(width, 20);
        let mut rand = seeded(1);
        draw_dirt_effect(&mut level, &sprites, &tex_slice(&tex), 0, 5, 3, &mut rand);

        let dest = (4 * width + 7) as usize; // my*width + mx
        assert_eq!(level.material_id[dest], 71, "wrap = ((my&15)<<4)+(mx&15)");
        assert_ne!(
            level.material_id[dest], 18,
            "must NOT use window offsets (y_<<4 + x_)"
        );
    }

    // ---- Step 5: clip at edges (CLIP_IMAGE clamp, height-1, no OOB) ---------
    #[test]
    fn clips_window_straddling_right_and_bottom_edges() {
        // width=20, height=10 => clip Rect(0,0,20,9): bottom row y=9 excluded.
        // Window at (18,8), 16x16: clamps to x in [18,20), y in [8,9) -> 2x1.
        let tex = Texture { sframe: 82, rframe: 1, mframe: 38, ndrawback: false };
        let sprites = make_sprites(83, &[(38, vec![6u8; SIZE]), (82, fill_const(150))]);

        let width = 20;
        let height = 10;
        let mut level = bg_level(width, height);
        let before = level.material_id.clone();

        let mut rand = seeded(1);
        // No panic = no OOB write.
        draw_dirt_effect(&mut level, &sprites, &tex_slice(&tex), 0, 18, 8, &mut rand);

        // Exactly (18,8) and (19,8) changed; everything else (incl. row 9 and
        // col 17) is untouched.
        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                let expect_write = y == 8 && (x == 18 || x == 19);
                if expect_write {
                    assert_eq!(level.material_id[idx], 150, "({x},{y}) filled");
                } else {
                    assert_eq!(
                        level.material_id[idx], before[idx],
                        "({x},{y}) must be untouched (clip)"
                    );
                }
            }
        }
    }

    #[test]
    fn clips_window_past_top_left_corner() {
        // Window at (-3,-2), 16x16, level 20x10 (clip Rect(0,0,20,9)). Clamps to
        // x in [0,13), y in [0,9). Mask all case-6, fill const 175.
        let tex = Texture { sframe: 82, rframe: 1, mframe: 38, ndrawback: false };
        let sprites = make_sprites(83, &[(38, vec![6u8; SIZE]), (82, fill_const(175))]);

        let width = 20;
        let height = 10;
        let mut level = bg_level(width, height);

        let mut rand = seeded(1);
        draw_dirt_effect(&mut level, &sprites, &tex_slice(&tex), 0, -3, -2, &mut rand);

        let at = |x: i32, y: i32| level.material_id[(y * width + x) as usize];
        assert_eq!(at(0, 0), 175, "top-left covered cell filled");
        assert_eq!(at(12, 8), 175, "bottom-right covered cell filled");
        assert_eq!(at(13, 8), 0, "x=13 outside clamped width -> untouched");
        assert_eq!(at(0, 9), 0, "y=9 (height-1) excluded -> untouched");
    }

    // Helper: wrap a single texture in a slice so calls read `textures[0]`.
    fn tex_slice(t: &Texture) -> [Texture; 1] {
        [t.clone()]
    }
}
