//! Random level generation — port of C++ `Level::GenerateRandom` and friends
//! (`src/game/level.cpp:11-193`, `:397-429`) plus `BlitStone` (`gfx/blit.cpp:462-532`).
//! Step 4½, slice 4½b.
//!
//! Bit-exact vs C++ for the same `Rand` state, TC assets and dimensions — gated by
//! `oracle-tests/tests/levelgen_golden.rs` against `oracle_dump_levelgen`, which hashes
//! `material_id` and `rand.last` after every stage. Integer-only; no I/O; no `SimState`.
//!
//! * **The generator never constructs a `Rand`.** Every entry point takes `&mut Rand`; the
//!   game shell seeds a dedicated one from the match seed (overview locked decision 6,
//!   design §2), the oracle drives it from any seeded state.
//! * **Flags are read live** from `material_flags[material_id[i]]` — equivalent to the C++
//!   `materials[]` cache because every generation writer keeps that cache in sync and the
//!   field writes every cell before any flag is read (design F6).
//! * **C++ `rand()` returns `u32`**: `rand(n) - 8`, `height - 1 - rand(20)` wrap and convert
//!   back to `int`, which is exactly `rand.bound(n) as i32 - 8` here (`bound < n <= 4096`,
//!   design F7). No C++ expression here holds two `rand()` calls, so there is no
//!   unspecified evaluation order to reproduce.

use assets::sprite::SpriteSet;
use sim_core::rng::Rand;

use crate::state::LevelSim;

/// Largest level side C++ accepts (`level.cpp:232`, `assets/src/level.rs:51`).
const MAX_DIM: i32 = 4096;

/// `Level::Resize` (`level.cpp:218-227`) on a fresh level: a zero-filled `width × height`
/// material map carrying the TC flag table. Precondition `1 <= width, height <= 4096`
/// (the menu range is 64..=4096 step 8, `gfx.cpp:1295-1297`, but a hand-edited setup file
/// can hold any value).
pub fn new_level(width: i32, height: i32, material_flags: &[u8; 256]) -> LevelSim {
    debug_assert!(
        (1..=MAX_DIM).contains(&width) && (1..=MAX_DIM).contains(&height),
        "level size {width}x{height} outside 1..=4096"
    );
    LevelSim {
        width,
        height,
        material_id: vec![0u8; (width * height) as usize],
        material_flags: *material_flags,
    }
}

/// The diffusion noise field — first block of `GenerateDirtPattern` (`level.cpp:12-26`).
/// Draws exactly `width * height` values: the corner `rand(7)+12`; the first COLUMN, each
/// `(rand(7)+12 + above) >> 1`; the first ROW, each `(rand(7)+12 + left) >> 1`; the
/// interior row-major, each `(left + up + rand(8)+12) / 3`. Values stay in 12..=18 (the
/// tc.cfg dirt shades).
pub fn generate_dirt_field(level: &mut LevelSim, rand: &mut Rand) {
    let w = level.width;
    let h = level.height;
    let at = |x: i32, y: i32| (x + y * w) as usize;
    let corner = rand.bound(7) + 12;
    level.set_material(0, corner as u8);
    for y in 1..h {
        let up = level.material_id[at(0, y - 1)] as u32;
        let v = (rand.bound(7) + 12 + up) >> 1;
        level.set_material(at(0, y), v as u8);
    }
    for x in 1..w {
        let left = level.material_id[at(x - 1, 0)] as u32;
        let v = (rand.bound(7) + 12 + left) >> 1;
        level.set_material(at(x, 0), v as u8);
    }
    for y in 1..h {
        for x in 1..w {
            let left = level.material_id[at(x - 1, y)] as u32;
            let up = level.material_id[at(x, y - 1)] as u32;
            let v = (left + up + rand.bound(8) + 12) / 3;
            level.set_material(at(x, y), v as u8);
        }
    }
}

/// Port of `BlitStone(common, level, p1 = false, mem, x, y)` (`gfx/blit.cpp:462-532`).
/// `CLIP_IMAGE` (`gfx/macros.hpp:3-22`) against **`Rect(0, 0, width, height)` — the full
/// height** (unlike `DrawDirtEffect`'s `height - 1`), then every NON-ZERO texel overwrites
/// the destination (the `p1 = false` branch has no material test, `:505-531`). Every
/// C++ caller is the generator and passes `p1 = false`; the `p1 = true` branch has no caller
/// and is not ported.
///
/// The clipping block duplicates its twin in `draw_dirt_effect` (`sim/src/blit.rs:58-90`,
/// which clips to `height - 1`); the two are to be merged into one `CLIP_IMAGE` helper
/// after slice 4½a lands (controller ruling R10 — `blit.rs` is not touched here).
pub fn blit_stone(level: &mut LevelSim, large_sprites: &SpriteSet, frame: usize, x: i32, y: i32) {
    let sprite = large_sprites.sprite(frame);
    let pitch: i32 = 16;
    let mut w: i32 = 16;
    let mut h: i32 = 16;
    let mut mem: i32 = 0;
    let mut x = x;
    let mut y = y;
    let (cx1, cy1, cx2, cy2) = (0i32, 0i32, level.width, level.height);
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
    let level_width = level.width;
    for y_ in 0..h {
        for x_ in 0..w {
            let c = sprite[(mem + y_ * pitch + x_) as usize];
            if c != 0 {
                level.set_material(((y + y_) * level_width + x + x_) as usize, c);
            }
        }
    }
}

/// One large-sprite splat (`level.cpp:38-70`): rows `my >= height` BREAK, `my < 0`
/// CONTINUE (same for columns) — a literal port, not a clamp. A non-zero texel over a
/// destination in 177..=179 (`> 176 && < 180`) blends to `(src + dest) / 2`, otherwise it
/// overwrites.
fn splat_sprite(level: &mut LevelSim, image: &[u8], kx: i32, ky: i32) {
    for cy in 0..16 {
        let my = cy + ky;
        if my >= level.height {
            break;
        }
        if my < 0 {
            continue;
        }
        for cx in 0..16 {
            let mx = cx + kx;
            if mx >= level.width {
                break;
            }
            if mx < 0 {
                continue;
            }
            let src = image[((cy << 4) + cx) as usize];
            if src > 0 {
                let idx = (mx + my * level.width) as usize;
                let pix = level.material_id[idx];
                let v = if pix > 176 && pix < 180 {
                    ((src as u32 + pix as u32) / 2) as u8
                } else {
                    src
                };
                level.set_material(idx, v);
            }
        }
    }
}

/// `GenerateDirtPattern`'s splat block (`level.cpp:30-71`): `count = rand(100)`, then per
/// splat `x = rand(w)-8`, `y = rand(h)-8`, frame `rand(4)+69`. Draws `1 + 3*count`.
pub fn splat_large_sprites(level: &mut LevelSim, large_sprites: &SpriteSet, rand: &mut Rand) {
    let count = rand.bound(100) as i32;
    for _ in 0..count {
        let kx = rand.bound(level.width as u32) as i32 - 8;
        let ky = rand.bound(level.height as u32) as i32 - 8;
        let frame = rand.bound(4) as usize + 69;
        splat_sprite(level, large_sprites.sprite(frame), kx, ky);
    }
}

/// `GenerateDirtPattern`'s stone block (`level.cpp:73-82`): `count = rand(15)`, then per
/// stone `x = rand(w)-8`, `y = rand(h)-8`, frame `rand(4)+56`, [`blit_stone`]. Draws
/// `1 + 3*count`.
pub fn scatter_stones(level: &mut LevelSim, large_sprites: &SpriteSet, rand: &mut Rand) {
    let count = rand.bound(15) as i32;
    for _ in 0..count {
        let kx = rand.bound(level.width as u32) as i32 - 8;
        let ky = rand.bound(level.height as u32) as i32 - 8;
        let frame = rand.bound(4) as usize + 56;
        blit_stone(level, large_sprites, frame, kx, ky);
    }
}

/// `Level::GenerateDirtPattern` (`level.cpp:11-83`) = field, splats, stones.
pub fn generate_dirt_pattern(level: &mut LevelSim, large_sprites: &SpriteSet, rand: &mut Rand) {
    generate_dirt_field(level, rand);
    splat_large_sprites(level, large_sprites, rand);
    scatter_stones(level, large_sprites, rand);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::MAT_ROCK;

    fn seeded(seed: u32) -> Rand {
        let mut r = Rand::new();
        r.seed(seed);
        r
    }

    #[test]
    fn new_level_is_zero_filled_with_dims_and_flags() {
        let mut flags = [0u8; 256];
        flags[7] = MAT_ROCK;
        let level = new_level(5, 3, &flags);
        assert_eq!((level.width, level.height), (5, 3));
        assert_eq!(level.material_id, vec![0u8; 15]);
        assert_eq!(level.material_flags, flags);
    }

    /// level.cpp:12-26 restated statement by statement with an identically seeded
    /// reference `Rand`: corner, then COLUMN x=0 (y=1..h), then ROW y=0 (x=1..w), then the
    /// interior row-major. Swapping the column and row loops shifts every later draw.
    #[test]
    fn dirt_field_follows_level_cpp_12_26_draw_order() {
        let (w, h) = (9i32, 7i32);
        let mut r = seeded(7);
        let mut want = vec![0u8; (w * h) as usize];
        let at = |x: i32, y: i32| (x + y * w) as usize;
        want[0] = (r.bound(7) + 12) as u8;
        for y in 1..h {
            want[at(0, y)] = ((r.bound(7) + 12 + want[at(0, y - 1)] as u32) >> 1) as u8;
        }
        for x in 1..w {
            want[at(x, 0)] = ((r.bound(7) + 12 + want[at(x - 1, 0)] as u32) >> 1) as u8;
        }
        for y in 1..h {
            for x in 1..w {
                let left = want[at(x - 1, y)] as u32;
                let up = want[at(x, y - 1)] as u32;
                want[at(x, y)] = ((left + up + r.bound(8) + 12) / 3) as u8;
            }
        }

        let mut level = new_level(w, h, &[0u8; 256]);
        let mut rand = seeded(7);
        generate_dirt_field(&mut level, &mut rand);
        assert_eq!(level.material_id, want);
        assert_eq!(rand.last(), r.last());
        assert_eq!(rand.draws(), r.draws());
    }

    #[test]
    fn dirt_field_draws_w_times_h_and_stays_in_12_to_18() {
        let mut level = new_level(17, 9, &[0u8; 256]);
        let mut rand = seeded(42);
        generate_dirt_field(&mut level, &mut rand);
        assert_eq!(rand.draws(), 17 * 9);
        assert!(level.material_id.iter().all(|&p| (12..=18).contains(&p)));
    }

    /// A bank of `count` 16x16 sprites, all zero except the `(frame, 256 bytes)` overrides.
    fn bank(count: i32, overrides: &[(usize, Vec<u8>)]) -> SpriteSet {
        let mut data = vec![0u8; count as usize * 256];
        for (frame, bytes) in overrides {
            assert_eq!(bytes.len(), 256);
            data[frame * 256..frame * 256 + 256].copy_from_slice(bytes);
        }
        SpriteSet {
            width: 16,
            height: 16,
            count,
            data,
        }
    }

    #[test]
    fn blit_stone_overwrites_nonzero_texels_and_clips_to_full_height() {
        // texel (x_, y_) = 100 + y_*16 + x_, except texel (1, 0) = 0 (transparent).
        let mut spr: Vec<u8> = (0..256).map(|i| 100u8.wrapping_add(i as u8)).collect();
        spr[1] = 0;
        let sprites = bank(4, &[(3, spr)]);
        let mut level = new_level(20, 10, &[0u8; 256]);
        level.material_id.iter_mut().for_each(|p| *p = 7);
        blit_stone(&mut level, &sprites, 3, 15, 5); // clipped to x 15..20, y 5..10
        let at = |x: i32, y: i32| level.material_id[(x + y * 20) as usize];
        assert_eq!(at(15, 5), 100, "texel (0,0)");
        assert_eq!(at(16, 5), 7, "zero texel is transparent");
        assert_eq!(
            at(19, 9),
            100 + 4 * 16 + 4,
            "bottom row IS written: clip is Rect(0,0,w,h)"
        );
        assert_eq!(at(14, 5), 7, "left of the window untouched");
    }

    #[test]
    fn blit_stone_negative_origin_offsets_the_source_like_clip_image() {
        let spr: Vec<u8> = (0..256).map(|i| 1u8.wrapping_add(i as u8)).collect();
        let sprites = bank(1, &[(0, spr)]);
        let mut level = new_level(20, 10, &[0u8; 256]);
        blit_stone(&mut level, &sprites, 0, -3, -2); // visible window: x 0..13, y 0..14∩0..10
        assert_eq!(level.material_id[0], 1 + 2 * 16 + 3, "(0,0) <- texel (3,2)");
        assert_eq!(
            level.material_id[12],
            1 + 2 * 16 + 15,
            "(12,0) <- texel (15,2)"
        );
        assert_eq!(
            level.material_id[13], 0,
            "(13,0) is outside the clipped window"
        );
    }

    #[test]
    fn splat_blends_177_to_179_and_overwrites_everything_else() {
        // All-150 image except texel (5,0) = 0; row 0 destination = 176,177,178,179,180,55.
        let mut img = vec![150u8; 256];
        img[5] = 0;
        let mut level = new_level(16, 16, &[0u8; 256]);
        for (x, v) in [176u8, 177, 178, 179, 180, 55].iter().enumerate() {
            level.material_id[x] = *v;
        }
        splat_sprite(&mut level, &img, 0, 0);
        // (150+177)/2 = 163, (150+178)/2 = 164, (150+179)/2 = 164 (level.cpp:63-64).
        assert_eq!(&level.material_id[0..6], &[150, 163, 164, 164, 150, 55]);
        assert_eq!(level.material_id[16], 150, "(0,1) overwritten");
    }

    #[test]
    fn splat_clips_with_break_and_continue_at_every_edge() {
        let img = vec![9u8; 256];
        let mut a = new_level(10, 6, &[0u8; 256]);
        splat_sprite(&mut a, &img, -8, -8); // rand(w)-8 can be -8: cells 0..8 x 0..8, h=6
        let at = |l: &LevelSim, x: i32, y: i32| l.material_id[(x + y * 10) as usize];
        assert_eq!(at(&a, 0, 0), 9);
        assert_eq!(at(&a, 7, 5), 9);
        assert_eq!(at(&a, 8, 5), 0, "x = kX + 16 is past the sprite");
        let mut b = new_level(10, 6, &[0u8; 256]);
        splat_sprite(&mut b, &img, 5, 3); // breaks at x = 10 and y = 6
        assert_eq!(at(&b, 9, 5), 9);
        assert_eq!(at(&b, 4, 3), 0);
        assert_eq!(at(&b, 5, 2), 0);
    }

    #[test]
    fn splats_draw_one_count_plus_three_per_splat() {
        let sprites = bank(73, &[]); // frames 69..=72 must exist
        let mut level = new_level(40, 30, &[0u8; 256]);
        let mut rand = seeded(5);
        splat_large_sprites(&mut level, &sprites, &mut rand);
        let mut r = seeded(5);
        let count = r.bound(100) as u64;
        assert_eq!(rand.draws(), 1 + 3 * count);
    }

    #[test]
    fn stones_draw_one_count_plus_three_per_stone_from_frames_56_to_59() {
        let solid = |v: u8| vec![v; 256];
        let sprites = bank(
            60,
            &[
                (56, solid(56)),
                (57, solid(57)),
                (58, solid(58)),
                (59, solid(59)),
            ],
        );
        let mut level = new_level(64, 48, &[0u8; 256]);
        let mut rand = seeded(11);
        scatter_stones(&mut level, &sprites, &mut rand);
        let mut r = seeded(11);
        let count = r.bound(15) as u64;
        assert_eq!(rand.draws(), 1 + 3 * count);
        assert!(level
            .material_id
            .iter()
            .all(|&p| p == 0 || (56..=59).contains(&p)));
    }

    #[test]
    fn dirt_pattern_is_field_then_splats_then_stones() {
        let sprites = bank(73, &[(70, vec![200; 256]), (57, vec![30; 256])]);
        let mut a = new_level(50, 40, &[0u8; 256]);
        let mut ra = seeded(3);
        generate_dirt_pattern(&mut a, &sprites, &mut ra);
        let mut b = new_level(50, 40, &[0u8; 256]);
        let mut rb = seeded(3);
        generate_dirt_field(&mut b, &mut rb);
        splat_large_sprites(&mut b, &sprites, &mut rb);
        scatter_stones(&mut b, &sprites, &mut rb);
        assert_eq!(a, b);
        assert_eq!(ra.last(), rb.last());
    }
}
